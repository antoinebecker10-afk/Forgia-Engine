//! chain.rs — la CHAÎNE : un tir qui saute sur les ennemis voisins (story-680, cran 5).
//!
//! ## Pourquoi ce module existe
//!
//! `PlayerCombatMods.chain_extra_targets` était calculé par `boons_apply` depuis
//! story-558 et **aucun système ne le lisait**. Les deux atouts « Chaîne » du
//! catalogue ne faisaient donc strictement rien — et c'était, avec « Impact »,
//! l'un des deux seuls effets du jeu qui changeait la *façon de jouer* plutôt que
//! de multiplier un nombre.
//!
//! L'audit de progression concluait « tout est vertical ». C'était pire : les
//! deux exceptions étaient mortes.
//!
//! ## Ce que ça change pour le joueur
//!
//! Un atout de chaîne transforme le placement en décision : tirer sur l'ennemi au
//! milieu du groupe vaut mieux que tirer sur celui qui est isolé. C'est une
//! mécanique, pas un multiplicateur.
//!
//! ## Garde-fous
//!
//! - **Pas de récursion.** Les rebonds ne rebondissent pas — sinon un groupe
//!   dense produirait une avalanche exponentielle. Le marqueur est `weapon: None`.
//! - **Seuls les tirs du JOUEUR chaînent.** Un ennemi qui tire ne doit pas
//!   propager sa balle dans ses alliés.
//! - **La cible d'origine est exclue** : la chaîne saute ailleurs, elle ne
//!   double pas le tir.

use bevy::prelude::*;
use forgia_ai_arena_bot::ArenaBot;
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_combat::combat_mods::PlayerCombatMods;
use forgia_combat::Health;
use forgia_player::Player;
use serde::Deserialize;
use std::fs;
use std::time::SystemTime;

pub(crate) const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_chain.toml";
const POLL_PERIOD_SEC: f32 = 1.0;

#[derive(Deserialize, Default)]
struct ChainToml {
    #[serde(default)]
    chain: ChainSection,
}

#[derive(Deserialize, Default)]
struct ChainSection {
    enabled: Option<bool>,
    radius_m: Option<f32>,
    damage_fraction: Option<f32>,
    falloff_per_jump: Option<f32>,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct ChainConfig {
    pub enabled: bool,
    /// Portée d'un saut (m). Au-delà, la chaîne ne trouve personne.
    pub radius_m: f32,
    /// Part des dégâts d'origine transmise au premier rebond.
    pub damage_fraction: f32,
    /// Décroissance par rebond supplémentaire.
    pub falloff_per_jump: f32,
}

impl Default for ChainConfig {
    fn default() -> Self {
        // Miroir EXACT de assets/genomes/roguelite/roguelite_chain.toml.
        Self {
            enabled: true,
            radius_m: 7.0,
            damage_fraction: 0.45,
            falloff_per_jump: 0.7,
        }
    }
}

impl ChainConfig {
    pub fn parse_toml(content: &str) -> Self {
        let t: ChainToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(e) => {
                warn!("[chain] génome illisible ({e}) — MIROIR RUST utilisé");
                return Self::default();
            }
        };
        let d = Self::default();
        Self {
            enabled: t.chain.enabled.unwrap_or(d.enabled),
            radius_m: t.chain.radius_m.unwrap_or(d.radius_m).clamp(0.5, 60.0),
            // Borné à 1.0 : un rebond ne peut pas frapper plus fort que le tir
            // d'origine, sinon viser à côté de sa cible deviendrait optimal.
            damage_fraction: t
                .chain
                .damage_fraction
                .unwrap_or(d.damage_fraction)
                .clamp(0.0, 1.0),
            falloff_per_jump: t
                .chain
                .falloff_per_jump
                .unwrap_or(d.falloff_per_jump)
                .clamp(0.05, 1.0),
        }
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(c) => Self::parse_toml(&c),
            Err(e) => {
                warn!("[chain] {GENOME_PATH} illisible ({e}) — défauts Rust");
                Self::default()
            }
        }
    }

    /// Dégâts du `jump`-ième rebond (1 = premier). PUR — testable.
    pub fn jump_damage(&self, base: f32, jump: u32) -> f32 {
        if jump == 0 {
            return 0.0;
        }
        base * self.damage_fraction * self.falloff_per_jump.powi(jump as i32 - 1)
    }
}

#[derive(Resource, Default, Debug)]
pub struct ChainWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

/// Compteur observable — sans lui, une chaîne qui ne trouve jamais de cible
/// serait indiscernable d'une chaîne absente.
#[derive(Resource, Default, Debug)]
pub struct ChainStats {
    pub jumps_total: u32,
    pub last_jumps: u32,
}

/// Sélectionne les cibles d'une chaîne — PUR, donc testable sans monde Bevy.
///
/// `candidates` = (entité, position) des ennemis, cible d'origine EXCLUE par
/// l'appelant. Rend les `max` plus proches dans le rayon, du plus proche au plus
/// lointain : la chaîne saute au voisin, pas au bout de la salle.
pub fn pick_chain_targets(
    origin: Vec3,
    candidates: &[(Entity, Vec3)],
    radius_m: f32,
    max: u32,
) -> Vec<Entity> {
    if max == 0 || radius_m <= 0.0 {
        return Vec::new();
    }
    let r2 = radius_m * radius_m;
    let mut in_range: Vec<(f32, Entity)> = candidates
        .iter()
        .filter_map(|(e, p)| {
            let d2 = p.distance_squared(origin);
            (d2 <= r2).then_some((d2, *e))
        })
        .collect();
    // Tri par distance, puis par INDEX d'entité. Le départage est explicite
    // exprès : l'ordre naturel de `Entity` n'est pas l'index croissant (il
    // compare d'abord la génération), donc s'y fier ferait dire au commentaire
    // autre chose que ce que fait le code. Deux ennemis à distance égale
    // doivent donner le même ordre d'une frame à l'autre.
    in_range.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.index().cmp(&b.1.index()))
    });
    in_range
        .into_iter()
        .take(max as usize)
        .map(|(_, e)| e)
        .collect()
}

/// Propage les tirs du joueur aux ennemis voisins.
#[allow(clippy::too_many_arguments)]
pub fn sys_apply_chain(
    cfg: Res<ChainConfig>,
    mods: Res<PlayerCombatMods>,
    mut events: MessageReader<CombatHitEvent>,
    q_player: Query<Entity, With<Player>>,
    q_bots: Query<(Entity, &Transform), With<ArenaBot>>,
    mut q_health: Query<&mut Health>,
    mut hits_w: MessageWriter<CombatHitEvent>,
    mut stats: ResMut<ChainStats>,
) {
    stats.last_jumps = 0;
    let extra = mods.chain_extra_targets;
    if !cfg.enabled || extra == 0 {
        // On DOIT quand même drainer, sinon les événements s'accumulent.
        events.read().for_each(|_| ());
        return;
    }
    let Ok(player) = q_player.single() else {
        events.read().for_each(|_| ());
        return;
    };
    // Un seul relevé de positions par frame — pas une requête par impact.
    let bots: Vec<(Entity, Vec3)> = q_bots.iter().map(|(e, t)| (e, t.translation)).collect();

    // Collecte d'abord : `q_health` est emprunté en mutable dans la boucle, on
    // ne peut pas lire les événements en même temps.
    let impacts: Vec<(Entity, Vec3, f32)> = events
        .read()
        // Seuls les tirs du JOUEUR chaînent, et seulement les tirs d'ARME : un
        // rebond porte `weapon: None`, ce qui l'empêche d'en relancer un autre.
        // Sans ce filtre, un groupe dense produirait une avalanche exponentielle.
        .filter(|ev| ev.attacker == Some(player) && ev.weapon.is_some())
        .map(|ev| (ev.target, ev.hit_world_pos, ev.damage))
        .collect();

    for (origin_target, origin_pos, damage) in impacts {
        let candidates: Vec<(Entity, Vec3)> = bots
            .iter()
            .filter(|(e, _)| *e != origin_target)
            .copied()
            .collect();
        for (i, target) in pick_chain_targets(origin_pos, &candidates, cfg.radius_m, extra)
            .into_iter()
            .enumerate()
        {
            let dmg = cfg.jump_damage(damage, i as u32 + 1);
            if dmg <= 0.0 {
                continue;
            }
            let Ok(mut hp) = q_health.get_mut(target) else {
                continue;
            };
            // ENNEMIS = `forgia_combat::Health`, mutation directe (piège mémoire
            // documenté : surtout pas `DamageEvent`, qui est la voie du JOUEUR).
            let before = hp.current;
            hp.current = (hp.current - dmg).max(0.0);
            let killed = before > 0.0 && hp.current <= 0.0;
            hits_w.write(CombatHitEvent {
                target,
                attacker: Some(player),
                damage: dmg,
                is_kill: killed,
                is_headshot: false,
                hit_world_pos: origin_pos,
                // `None` = c'est un rebond. C'est le marqueur qui empêche la
                // récursion, pas une omission.
                weapon: None,
                body_zone: forgia_damage::HitZone::Body,
            });
            stats.jumps_total = stats.jumps_total.saturating_add(1);
            stats.last_jumps = stats.last_jumps.saturating_add(1);
        }
    }
}

pub fn sys_init_chain(mut commands: Commands) {
    let cfg = ChainConfig::load_or_default();
    info!(
        "[chain] rayon {:.1} m, {:.0} % des dégâts au 1er rebond, x{:.2} par saut",
        cfg.radius_m,
        cfg.damage_fraction * 100.0,
        cfg.falloff_per_jump
    );
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(cfg);
    commands.insert_resource(ChainWatch {
        last_mtime: mtime,
        reload_count: 0,
    });
}

pub fn sys_hot_reload_chain(
    time: Res<Time<Real>>,
    mut cfg: ResMut<ChainConfig>,
    mut watch: ResMut<ChainWatch>,
    mut cooldown: Local<f32>,
) {
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = POLL_PERIOD_SEC;
    let Ok(mtime) = fs::metadata(GENOME_PATH).and_then(|m| m.modified()) else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    watch.last_mtime = Some(mtime);
    let Ok(content) = fs::read_to_string(GENOME_PATH) else {
        return;
    };
    let next = ChainConfig::parse_toml(&content);
    if next == *cfg {
        return;
    }
    *cfg = next;
    watch.reload_count = watch.reload_count.saturating_add(1);
    info!("[chain] génome HOT-RELOADED (#{})", watch.reload_count);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ent(i: u32) -> Entity {
        Entity::from_raw_u32(i).expect("index valide")
    }

    /// La chaîne saute au VOISIN, pas au bout de la salle.
    #[test]
    fn the_chain_picks_the_closest_targets_first() {
        let c = vec![
            (ent(1), Vec3::new(10.0, 0.0, 0.0)),
            (ent(2), Vec3::new(2.0, 0.0, 0.0)),
            (ent(3), Vec3::new(5.0, 0.0, 0.0)),
        ];
        assert_eq!(
            pick_chain_targets(Vec3::ZERO, &c, 20.0, 2),
            vec![ent(2), ent(3)]
        );
    }

    /// Hors de portée = pas de saut. Sans ça, la « chaîne » serait un AoE global
    /// et le placement cesserait d'être une décision.
    #[test]
    fn nothing_outside_the_radius_is_hit() {
        let c = vec![(ent(1), Vec3::new(50.0, 0.0, 0.0))];
        assert!(pick_chain_targets(Vec3::ZERO, &c, 7.0, 3).is_empty());
    }

    /// Déterminisme : deux ennemis à distance égale doivent donner le même
    /// ordre d'une frame à l'autre.
    #[test]
    fn ties_are_broken_deterministically() {
        let c = vec![
            (ent(7), Vec3::new(3.0, 0.0, 0.0)),
            (ent(3), Vec3::new(-3.0, 0.0, 0.0)),
        ];
        let a = pick_chain_targets(Vec3::ZERO, &c, 10.0, 2);
        assert_eq!(a, pick_chain_targets(Vec3::ZERO, &c, 10.0, 2));
        assert_eq!(a[0], ent(3), "à distance égale, l'entité la plus basse d'abord");
    }

    /// Zéro cible supplémentaire = aucun saut : un joueur sans atout Chaîne ne
    /// paie pas le coût d'une mécanique qu'il n'a pas.
    #[test]
    fn no_boon_means_no_jump() {
        let c = vec![(ent(1), Vec3::new(1.0, 0.0, 0.0))];
        assert!(pick_chain_targets(Vec3::ZERO, &c, 7.0, 0).is_empty());
    }

    /// Les rebonds décroissent — sinon la chaîne écraserait toute autre
    /// statistique et il ne resterait qu'un seul build viable.
    #[test]
    fn each_jump_hits_softer_than_the_last() {
        let c = ChainConfig::default();
        let (j1, j2, j3) = (
            c.jump_damage(100.0, 1),
            c.jump_damage(100.0, 2),
            c.jump_damage(100.0, 3),
        );
        assert!(j1 > j2 && j2 > j3, "{j1} / {j2} / {j3}");
        assert!(j3 > 0.0, "aucun rebond ne doit être nul");
    }

    /// Un rebond ne frappe JAMAIS plus fort que le tir d'origine : sinon viser
    /// à côté de sa cible deviendrait optimal.
    #[test]
    fn a_jump_never_hits_harder_than_the_original_shot() {
        let hostile =
            ChainConfig::parse_toml("[chain]\ndamage_fraction = 5.0\nfalloff_per_jump = 3.0\n");
        assert!(hostile.damage_fraction <= 1.0);
        assert!(hostile.falloff_per_jump <= 1.0);
        assert!(hostile.jump_damage(100.0, 1) <= 100.0);
    }

    #[test]
    fn a_broken_genome_falls_back_to_the_rust_mirror() {
        assert_eq!(
            ChainConfig::parse_toml("pas du TOML {{{"),
            ChainConfig::default()
        );
    }

    #[test]
    fn the_shipped_genome_matches_the_rust_mirror() {
        let content = fs::read_to_string(GENOME_PATH)
            .or_else(|_| fs::read_to_string(format!("../../{GENOME_PATH}")))
            .expect("roguelite_chain.toml introuvable");
        assert_eq!(ChainConfig::parse_toml(&content), ChainConfig::default());
    }
}
