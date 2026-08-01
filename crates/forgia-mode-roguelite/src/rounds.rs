//! rounds.rs — la BOUCLE DE ROUNDS et son mur (story-677).
//!
//! Des arènes qui s'enchaînent, sans carte ni choix de porte. La difficulté monte
//! plus vite que ce que le joueur gagne s'il ne fait rien : passer un round exige
//! de monter ses perfs.
//!
//! ## Pourquoi la courbe change
//!
//! L'ancien scaling était **linéaire** : `1 + round × 0,35`. Une droite finit
//! toujours par se faire rattraper par un joueur qui compose ses multiplicateurs
//! — la trempe seule vaut ×2,01 (5 paliers × +15 % multiplicatifs), et elle
//! rattrape un ×1,35/round vers le round 3. Passé là, la pression s'évapore.
//!
//! Maintenant : **géométrique + paliers**. La croissance continue tient la
//! pression sur toute la montée ; les paliers donnent le sursaut qu'on ressent.
//! Gunfire Reborn monte par paliers, Risk of Rain 2 en continu — on prend les deux.
//!
//! ## Le mur se CALCULE, il ne s'espère pas
//!
//! Un round se passe si la vague est nettoyée dans le budget de temps :
//!
//! ```text
//! ttk(r) = pv_vague(r) / (dps_base × puissance(r))
//! mur    = premier r où ttk(r) > budget_temps_round_s
//! ```
//!
//! Deux murs en découlent, et l'écart entre eux EST la valeur de la progression :
//! celui d'un joueur qui ne prend rien, et celui d'un joueur qui prend tout. Les
//! tests vérifient que cet écart existe — sinon monter ses perfs ne sert à rien
//! et la boucle n'est qu'un compte à rebours.

use bevy::prelude::*;
use serde::Deserialize;
use std::fs;
use std::time::SystemTime;

pub(crate) const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_rounds.toml";
const POLL_PERIOD_SEC: f32 = 1.0;

/// Borne de recherche du mur. Un mur au-delà de 500 rounds n'est pas un mur ;
/// la fonction renvoie alors `None` et le dit, plutôt que de boucler.
const WALL_SEARCH_LIMIT: u32 = 500;

// ─── TOML ───────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct RoundsToml {
    #[serde(default)]
    boucle: BoucleToml,
    #[serde(default)]
    menace: MenaceToml,
    #[serde(default)]
    mur: MurToml,
    #[serde(default)]
    recompense: RecompenseToml,
}

#[derive(Deserialize, Default)]
struct BoucleToml {
    enabled: Option<bool>,
    max_rounds: Option<u32>,
}

#[derive(Deserialize, Default)]
struct MenaceToml {
    pv_par_round: Option<f32>,
    degats_par_round: Option<f32>,
    palier_tous_les: Option<u32>,
    palier_pv: Option<f32>,
    palier_degats: Option<f32>,
}

#[derive(Deserialize, Default)]
struct MurToml {
    budget_temps_round_s: Option<f32>,
    dps_reference: Option<f32>,
    efficacite_tir: Option<f32>,
    pv_vague_reference: Option<f32>,
    gain_puissance_par_round: Option<f32>,
}

#[derive(Deserialize, Default)]
struct RecompenseToml {
    boon_par_round: Option<u32>,
    equipement_tous_les: Option<u32>,
    repit_tous_les: Option<u32>,
}

// ─── Config ─────────────────────────────────────────────────────────────────

/// Multiplicateurs de menace d'un round.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Threat {
    pub hp: f32,
    pub damage: f32,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct RoundsConfig {
    pub enabled: bool,
    /// 0 = rounds infinis.
    pub max_rounds: u32,
    pub hp_growth: f32,
    pub damage_growth: f32,
    pub tier_every: u32,
    pub tier_hp_step: f32,
    pub tier_damage_step: f32,
    pub round_time_budget_s: f32,
    pub dps_reference: f32,
    /// Part du round réellement passée à toucher. Un FPS ne délivre jamais son
    /// DPS théorique : déplacement, rechargement, visée, approche des ennemis.
    pub fire_uptime: f32,
    pub wave_hp_reference: f32,
    pub power_gain_per_round: f32,
    pub boon_per_round: u32,
    pub equipment_every: u32,
    pub respite_every: u32,
}

impl Default for RoundsConfig {
    fn default() -> Self {
        // Miroir EXACT de assets/genomes/roguelite/roguelite_rounds.toml.
        Self {
            enabled: true,
            max_rounds: 0,
            hp_growth: 1.16,
            damage_growth: 1.07,
            tier_every: 3,
            tier_hp_step: 1.22,
            tier_damage_step: 1.10,
            round_time_budget_s: 90.0,
            dps_reference: 168.0,
            fire_uptime: 0.35,
            wave_hp_reference: 1355.0,
            power_gain_per_round: 0.34,
            boon_per_round: 1,
            equipment_every: 2,
            respite_every: 5,
        }
    }
}

impl RoundsConfig {
    /// PUR — testable. Bornes appliquées à la lecture : un génome édité à la main
    /// ne doit pas pouvoir produire une courbe décroissante (round 5 plus facile
    /// que round 1) ni une explosion qui rend le round 3 injouable.
    pub fn parse_toml(content: &str) -> Self {
        let t: RoundsToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(e) => {
                warn!("[rounds] génome illisible ({e}) — MIROIR RUST utilisé");
                return Self::default();
            }
        };
        let d = Self::default();
        Self {
            enabled: t.boucle.enabled.unwrap_or(d.enabled),
            max_rounds: t.boucle.max_rounds.unwrap_or(d.max_rounds).min(10_000),
            // ≥ 1.0 : une croissance < 1 rendrait le round 10 plus facile que le 1.
            hp_growth: t
                .menace
                .pv_par_round
                .unwrap_or(d.hp_growth)
                .clamp(1.0, 2.0),
            damage_growth: t
                .menace
                .degats_par_round
                .unwrap_or(d.damage_growth)
                .clamp(1.0, 2.0),
            // ≥ 1 : un palier tous les 0 rounds serait une division par zéro.
            tier_every: t.menace.palier_tous_les.unwrap_or(d.tier_every).max(1),
            tier_hp_step: t.menace.palier_pv.unwrap_or(d.tier_hp_step).clamp(1.0, 5.0),
            tier_damage_step: t
                .menace
                .palier_degats
                .unwrap_or(d.tier_damage_step)
                .clamp(1.0, 5.0),
            round_time_budget_s: t
                .mur
                .budget_temps_round_s
                .unwrap_or(d.round_time_budget_s)
                .clamp(5.0, 3_600.0),
            dps_reference: t
                .mur
                .dps_reference
                .unwrap_or(d.dps_reference)
                .clamp(1.0, 100_000.0),
            // > 0 : une efficacité nulle donnerait un mur au round 0.
            fire_uptime: t
                .mur
                .efficacite_tir
                .unwrap_or(d.fire_uptime)
                .clamp(0.01, 1.0),
            wave_hp_reference: t
                .mur
                .pv_vague_reference
                .unwrap_or(d.wave_hp_reference)
                .clamp(1.0, 1_000_000.0),
            power_gain_per_round: t
                .mur
                .gain_puissance_par_round
                .unwrap_or(d.power_gain_per_round)
                .clamp(0.0, 10.0),
            boon_per_round: t.recompense.boon_par_round.unwrap_or(d.boon_per_round),
            equipment_every: t
                .recompense
                .equipement_tous_les
                .unwrap_or(d.equipment_every)
                .max(1),
            respite_every: t
                .recompense
                .repit_tous_les
                .unwrap_or(d.respite_every)
                .max(1),
        }
    }

    pub fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(c) => Self::parse_toml(&c),
            Err(e) => {
                warn!("[rounds] {GENOME_PATH} illisible ({e}) — courbe par défaut");
                Self::default()
            }
        }
    }

    /// La menace d'un round : croissance continue × sursaut de palier.
    ///
    /// Round 0 = ×1,0, toujours — c'est la référence de calibration de tout le reste.
    pub fn threat(&self, round: u32) -> Threat {
        if !self.enabled {
            return Threat {
                hp: 1.0,
                damage: 1.0,
            };
        }
        let tiers = (round / self.tier_every) as i32;
        Threat {
            hp: self.hp_growth.powi(round as i32) * self.tier_hp_step.powi(tiers),
            damage: self.damage_growth.powi(round as i32)
                * self.tier_damage_step.powi(tiers),
        }
    }

    /// `true` si ce round est un palier (le round où le sursaut tombe).
    ///
    /// Round 0 n'en est pas un : c'est le départ, pas une marche.
    pub fn is_tier_round(&self, round: u32) -> bool {
        round > 0 && round.is_multiple_of(self.tier_every)
    }

    /// Round de RESPIRATION — pas d'ennemis, marchand et soin.
    ///
    /// Le rythme d'une run a besoin de relâche : deux salles de combat de suite
    /// sans respiration se lisent comme une seule salle trop longue
    /// (`map-design-intention.md` §4.1).
    pub fn is_respite_round(&self, round: u32) -> bool {
        round > 0 && round.is_multiple_of(self.respite_every)
    }

    /// Round qui offre un choix d'équipement en plus du boon.
    pub fn grants_equipment(&self, round: u32) -> bool {
        round > 0 && round.is_multiple_of(self.equipment_every)
    }

    /// Puissance du joueur au round `r`, comme fraction de son DPS de départ.
    ///
    /// `uptake` ∈ \[0, 1\] = la part des récompenses qu'il prend réellement.
    /// 0 = il ne prend rien, 1 = il prend tout. C'est le paramètre qui rend la
    /// progression falsifiable : sans lui, on ne peut pas montrer qu'elle sert.
    pub fn player_power(&self, round: u32, uptake: f32) -> f32 {
        1.0 + self.power_gain_per_round * uptake.clamp(0.0, 1.0) * round as f32
    }

    /// Temps estimé pour nettoyer la vague du round `r` (s).
    pub fn time_to_clear(&self, round: u32, uptake: f32) -> f32 {
        let wave_hp = self.wave_hp_reference * self.threat(round).hp;
        let dps = self.dps_reference * self.fire_uptime * self.player_power(round, uptake);
        if dps <= f32::EPSILON {
            return f32::INFINITY;
        }
        wave_hp / dps
    }

    /// **Le mur** : premier round dont la vague ne se nettoie plus dans le budget.
    ///
    /// `None` = aucun mur trouvé sous `WALL_SEARCH_LIMIT` rounds. Ce n'est pas
    /// « pas de mur », c'est « le mur est hors de portée » — le distinguer évite
    /// de lire un `0` comme un succès.
    pub fn wall_round(&self, uptake: f32) -> Option<u32> {
        (0..WALL_SEARCH_LIMIT)
            .find(|&r| self.time_to_clear(r, uptake) > self.round_time_budget_s)
    }

    /// Marge du round courant : `1 - ttk / budget`. Négatif = le mur est franchi.
    pub fn margin(&self, round: u32, uptake: f32) -> f32 {
        1.0 - self.time_to_clear(round, uptake) / self.round_time_budget_s.max(f32::EPSILON)
    }
}

// ─── Plomberie ──────────────────────────────────────────────────────────────

#[derive(Resource, Default, Debug)]
pub struct RoundsWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

pub fn sys_init_rounds(mut commands: Commands) {
    let cfg = RoundsConfig::load_or_default();
    let lazy = cfg.wall_round(0.0);
    let diligent = cfg.wall_round(1.0);
    info!(
        "[rounds] boucle {} — menace pv ×{:.2}/round, palier ×{:.2} tous les {} ; \
         mur estimé : round {} sans progression, round {} en prenant tout",
        if cfg.enabled { "ACTIVE" } else { "inactive" },
        cfg.hp_growth,
        cfg.tier_hp_step,
        cfg.tier_every,
        lazy.map_or("∞".to_string(), |r| r.to_string()),
        diligent.map_or("∞".to_string(), |r| r.to_string()),
    );
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(cfg);
    commands.insert_resource(RoundsWatch {
        last_mtime: mtime,
        reload_count: 0,
    });
}

pub fn sys_hot_reload_rounds(
    time: Res<Time<Real>>,
    mut cfg: ResMut<RoundsConfig>,
    mut watch: ResMut<RoundsWatch>,
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
    let next = RoundsConfig::parse_toml(&content);
    if next == *cfg {
        return;
    }
    *cfg = next;
    watch.reload_count = watch.reload_count.saturating_add(1);
    info!(
        "[rounds] génome HOT-RELOADED (#{}) — mur sans progression : round {:?}",
        watch.reload_count,
        cfg.wall_round(0.0)
    );
}
// ─── Le rythme MESURÉ ───────────────────────────────────────────────────────

/// Nombre de rounds gardés en mémoire pour lire une TENDANCE.
///
/// Trois : deux points donnent une droite, jamais une tendance. Trois permettent
/// de distinguer « j'ai galéré une fois » de « je décroche ».
pub const PACE_HISTORY: usize = 3;

/// Où en est le joueur par rapport au mur — la lecture directe, en trois états.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    /// Marge confortable : le round se nettoie bien avant le budget.
    Holding,
    /// Le budget se consomme : ça passe encore, mais ça se resserre.
    Pressured,
    /// Le budget est dépassé — c'est le mur, ici et maintenant.
    Falling,
}

impl Pace {
    /// Libellé écran. Les mots comptent : « TU DÉCROCHES » dit au joueur QUOI
    /// faire (monter ses perfs), là où un pourcentage ne dit rien.
    pub const fn label(self) -> &'static str {
        match self {
            Pace::Holding => "TU TIENS",
            Pace::Pressured => "SOUS PRESSION",
            Pace::Falling => "TU DÉCROCHES",
        }
    }
}

/// Seuil entre « tu tiens » et « sous pression », en fraction du budget.
///
/// 0,6 : au-delà de 60 % du budget consommé, il ne reste plus de quoi absorber
/// un imprévu (une vague qui traîne, une mort évitée de justesse). C'est le
/// moment utile pour prévenir — prévenir à 99 % ne sert à rien.
const PRESSURE_THRESHOLD: f32 = 0.6;

/// Décide l'état à partir du temps de combat ÉCOULÉ — pas d'un DPS estimé.
///
/// C'est le point important : le mur est défini par « la vague se nettoie-t-elle
/// dans le budget ? ». On mesure donc exactement cette grandeur, au lieu de la
/// reconstruire depuis un DPS théorique et un facteur d'efficacité supposé.
pub fn pace_from_elapsed(elapsed_s: f32, budget_s: f32) -> Pace {
    if budget_s <= f32::EPSILON {
        return Pace::Holding;
    }
    let ratio = elapsed_s / budget_s;
    if ratio >= 1.0 {
        Pace::Falling
    } else if ratio >= PRESSURE_THRESHOLD {
        Pace::Pressured
    } else {
        Pace::Holding
    }
}

/// Temps de combat effectif du round en cours + les derniers rounds nettoyés.
///
/// « Effectif » = hors break et hors respiration : le budget porte sur le temps
/// passé à se battre, pas sur le temps passé à choisir un boon.
#[derive(Resource, Debug, Clone, Default)]
pub struct RoundPace {
    /// Round en cours de chronométrage.
    pub round: u8,
    /// Secondes de combat écoulées dans ce round.
    pub combat_secs: f32,
    /// Temps de nettoyage des derniers rounds, du plus récent au plus ancien.
    pub cleared: Vec<f32>,
}

impl RoundPace {
    /// Enregistre le round qui vient d'être nettoyé et repart à zéro.
    pub fn finish_round(&mut self, next_round: u8) {
        if self.combat_secs > 0.0 {
            self.cleared.insert(0, self.combat_secs);
            self.cleared.truncate(PACE_HISTORY);
        }
        self.round = next_round;
        self.combat_secs = 0.0;
    }

    /// La TENDANCE : le joueur décroche-t-il, ou tient-il le rythme ?
    ///
    /// `None` tant qu'on n'a pas assez de points — et c'est important de le
    /// dire plutôt que d'inventer une tendance sur un seul round.
    pub fn trend(&self, budget_s: f32) -> Option<Pace> {
        if self.cleared.len() < PACE_HISTORY || budget_s <= f32::EPSILON {
            return None;
        }
        // Moyenne des temps de nettoyage récents rapportée au budget.
        let avg = self.cleared.iter().sum::<f32>() / self.cleared.len() as f32;
        Some(pace_from_elapsed(avg, budget_s))
    }
}

/// Chronomètre le temps de combat du round. Ne tourne PAS pendant les breaks ni
/// les respirations : le budget porte sur le combat, pas sur les temps morts.
pub fn sys_track_round_pace(
    time: Res<Time>,
    cfg: Res<RoundsConfig>,
    wave: Res<crate::waves::RogueliteWave>,
    mut pace: ResMut<RoundPace>,
) {
    if wave.stage != pace.round {
        pace.finish_round(wave.stage);
        return;
    }
    if !cfg.enabled || wave.in_break || cfg.is_respite_round(u32::from(wave.stage)) {
        return;
    }
    pace.combat_secs += time.delta_secs();
}

// ─── Capteur ────────────────────────────────────────────────────────────────

/// Chemin du capteur — `observability-required.md` : une feature qu'on ne voit
/// pas quand l'user dit « regarde » est incomplète.
pub const SENSOR_PATH: &str = "forgia2_rounds.json";
const SENSOR_PERIOD_SEC: f32 = 1.0;

/// Écrit l'état de la boucle : où en est le round, quelle menace il porte, et
/// **à quelle distance du mur** on se trouve.
///
/// La marge est la lecture qui compte : positive = le round est nettoyable dans
/// le budget, négative = le mur est franchi et il faut monter ses perfs. C'est
/// exactement la question que la boucle pose au joueur, rendue lisible.
pub fn sys_write_rounds_sensor(
    time: Res<Time<Real>>,
    cfg: Res<RoundsConfig>,
    wave: Res<crate::waves::RogueliteWave>,
    pace: Res<RoundPace>,
    mut cooldown: Local<f32>,
) {
    *cooldown -= time.delta_secs();
    if *cooldown > 0.0 {
        return;
    }
    *cooldown = SENSOR_PERIOD_SEC;
    let round = u32::from(wave.stage);
    let t = cfg.threat(round);
    // On ne connaît pas la part de récompenses réellement prise : on encadre.
    let margin_lazy = cfg.margin(round, 0.0);
    let margin_full = cfg.margin(round, 1.0);
    let (severity, next_step) = if margin_full < 0.0 {
        (
            "error",
            "MUR FRANCHI même en prenant tout : baisser [menace] pv_par_round ou palier_pv dans roguelite_rounds.toml",
        )
    } else if margin_lazy < 0.0 {
        (
            "info",
            "le round exige de la progression — c'est l'intention (mur sans upgrade)",
        )
    } else {
        ("ok", "-")
    };
    let json = format!(
        r#"{{"id":"roguelite_rounds","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"loop_enabled":{},"round":{round},"is_tier_round":{},"is_respite_round":{},"threat_hp":{:.3},"threat_damage":{:.3},"time_to_clear_lazy_s":{:.1},"time_to_clear_full_s":{:.1},"round_time_budget_s":{:.1},"margin_lazy":{:.3},"margin_full":{:.3},"wall_lazy":{},"wall_full":{},"combat_secs":{:.1},"pace":"{}","pace_trend":"{}","cleared_secs":{:?}}}"#,
        time.elapsed_secs_f64(),
        cfg.enabled,
        cfg.is_tier_round(round),
        cfg.is_respite_round(round),
        t.hp,
        t.damage,
        cfg.time_to_clear(round, 0.0),
        cfg.time_to_clear(round, 1.0),
        cfg.round_time_budget_s,
        margin_lazy,
        margin_full,
        cfg.wall_round(0.0).map_or(-1i64, i64::from),
        cfg.wall_round(1.0).map_or(-1i64, i64::from),
        pace.combat_secs,
        pace_from_elapsed(pace.combat_secs, cfg.round_time_budget_s).label(),
        pace.trend(cfg.round_time_budget_s)
            .map_or("indéterminée", Pace::label),
        pace.cleared,
    );
    let _ = fs::write(SENSOR_PATH, json);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le round 0 est la référence de calibration de tout le reste : s'il n'est
    /// pas à ×1,0, aucun autre chiffre du génome ne veut dire ce qu'il dit.
    #[test]
    fn round_zero_is_the_reference() {
        let c = RoundsConfig::default();
        let t = c.threat(0);
        assert!((t.hp - 1.0).abs() < 1e-5, "pv round 0 = {}", t.hp);
        assert!((t.damage - 1.0).abs() < 1e-5);
    }

    /// La menace doit croître STRICTEMENT — un plateau, c'est un round gratuit,
    /// et c'est le défaut qu'on a déjà payé sur le budget du directeur.
    #[test]
    fn the_threat_grows_strictly_every_single_round() {
        let c = RoundsConfig::default();
        for r in 0..60 {
            let a = c.threat(r);
            let b = c.threat(r + 1);
            assert!(b.hp > a.hp, "plateau de pv entre {r} et {}", r + 1);
            assert!(b.damage > a.damage, "plateau de dégâts entre {r} et {}", r + 1);
        }
    }

    /// Les paliers doivent se SENTIR : le saut de palier doit être nettement plus
    /// grand que le pas ordinaire, sinon c'est une pente déguisée en marche.
    #[test]
    fn a_tier_round_is_a_real_step_not_a_slope() {
        let c = RoundsConfig::default();
        let tier = c.tier_every;
        assert!(c.is_tier_round(tier) && !c.is_tier_round(tier - 1));
        assert!(!c.is_tier_round(0), "le round 0 n'est pas une marche");
        let step_at_tier = c.threat(tier).hp / c.threat(tier - 1).hp;
        let ordinary = c.threat(tier + 1).hp / c.threat(tier).hp;
        assert!(
            step_at_tier > ordinary * 1.15,
            "palier ×{step_at_tier:.3} vs ordinaire ×{ordinary:.3} — on ne le sentira pas"
        );
    }

    /// LE CŒUR DU SUJET : monter ses perfs doit repousser le mur, franchement.
    /// Si l'écart est petit, la progression est décorative et la boucle n'est
    /// qu'un compte à rebours.
    #[test]
    fn upgrading_pushes_the_wall_far_enough_to_matter() {
        let c = RoundsConfig::default();
        let lazy = c.wall_round(0.0).expect("un joueur qui ne prend rien DOIT buter");
        let diligent = c.wall_round(1.0).expect("le mur doit exister même en jouant bien");
        assert!(
            diligent > lazy,
            "prendre les récompenses ne repousse pas le mur ({lazy} → {diligent})"
        );
        assert!(
            diligent >= lazy * 2,
            "l'écart est trop faible pour être senti : {lazy} sans progression, {diligent} avec"
        );
        // Le mur sans progression doit tomber TÔT — la pression doit être
        // ressentie dans les premiers rounds, pas au bout d'une heure.
        assert!(
            (2..=8).contains(&lazy),
            "mur sans progression au round {lazy} — trop tard pour se faire sentir"
        );
        println!(
            "[story-677] mur : round {lazy} sans progression, round {diligent} en prenant tout"
        );
    }

    /// Le mur doit être franchissable *en jouant bien* sur les premiers rounds :
    /// une boucle où le round 1 est déjà un mur n'est pas une boucle.
    #[test]
    fn the_early_rounds_are_clearable_without_any_upgrade() {
        let c = RoundsConfig::default();
        assert!(c.margin(0, 0.0) > 0.0, "le round 0 doit passer les mains nues");
        assert!(c.margin(1, 0.0) > 0.0, "le round 1 aussi");
    }

    /// La marge est la lecture directe du capteur : positive = ça passe.
    #[test]
    fn the_margin_flips_sign_exactly_at_the_wall() {
        let c = RoundsConfig::default();
        let wall = c.wall_round(0.5).expect("mur attendu");
        assert!(c.margin(wall, 0.5) < 0.0, "au mur, la marge doit être négative");
        assert!(
            c.margin(wall - 1, 0.5) >= 0.0,
            "juste avant le mur, la marge doit être positive"
        );
    }

    /// Un génome cassé ne doit jamais produire une courbe décroissante ni une
    /// division par zéro sur le palier.
    #[test]
    fn a_hostile_genome_cannot_produce_an_absurd_curve() {
        let c = RoundsConfig::parse_toml(
            r#"
[menace]
pv_par_round = 0.5
palier_tous_les = 0
palier_pv = 0.1
[mur]
budget_temps_round_s = 0.0
dps_reference = 0.0
"#,
        );
        assert!(c.hp_growth >= 1.0, "croissance < 1 = round 10 plus facile que le 1");
        assert!(c.tier_every >= 1, "palier tous les 0 rounds = division par zéro");
        assert!(c.tier_hp_step >= 1.0);
        assert!(c.dps_reference > 0.0);
        // Et la courbe reste monotone malgré l'hostilité.
        for r in 0..20 {
            assert!(c.threat(r + 1).hp >= c.threat(r).hp);
        }
    }

    #[test]
    fn a_broken_genome_falls_back_to_the_rust_mirror() {
        let c = RoundsConfig::parse_toml("pas du TOML {{{");
        assert_eq!(c, RoundsConfig::default());
    }

    /// Le rythme : respirations et équipements ne tombent jamais au round 0.
    #[test]
    fn the_rhythm_never_fires_on_round_zero() {
        let c = RoundsConfig::default();
        assert!(!c.is_respite_round(0) && !c.grants_equipment(0));
        assert!(c.is_respite_round(c.respite_every));
        assert!(c.grants_equipment(c.equipment_every));
    }

    /// Le génome livré doit produire la même chose que le miroir Rust — sinon
    /// le repli change le jeu au lieu de le préserver.
    #[test]
    fn the_shipped_genome_matches_the_rust_mirror() {
        let content = fs::read_to_string(GENOME_PATH)
            .or_else(|_| fs::read_to_string(format!("../../{GENOME_PATH}")))
            .expect("roguelite_rounds.toml introuvable");
        assert_eq!(
            RoundsConfig::parse_toml(&content),
            RoundsConfig::default(),
            "le TOML livré et le miroir Rust ont divergé"
        );
    }
}

#[cfg(test)]
mod pace_tests {
    use super::*;

    /// Les trois états, aux bornes exactes. Le seuil de pression est à 60 % :
    /// prévenir à 99 % du budget ne servirait à rien.
    #[test]
    fn the_three_states_land_on_their_thresholds() {
        assert_eq!(pace_from_elapsed(0.0, 90.0), Pace::Holding);
        assert_eq!(pace_from_elapsed(53.0, 90.0), Pace::Holding);
        assert_eq!(pace_from_elapsed(54.0, 90.0), Pace::Pressured, "60 % du budget");
        assert_eq!(pace_from_elapsed(89.0, 90.0), Pace::Pressured);
        assert_eq!(pace_from_elapsed(90.0, 90.0), Pace::Falling, "au budget = le mur");
        assert_eq!(pace_from_elapsed(300.0, 90.0), Pace::Falling);
    }

    /// Un budget nul ne doit pas produire un NaN ni un « tu décroches » permanent.
    #[test]
    fn a_zero_budget_does_not_produce_a_permanent_alarm() {
        assert_eq!(pace_from_elapsed(10.0, 0.0), Pace::Holding);
    }

    /// La tendance ne se prononce PAS tant qu'elle n'a pas de quoi. Sur un seul
    /// round, « tu décroches » serait du bruit — et le bruit fait ignorer l'alerte.
    #[test]
    fn the_trend_stays_silent_until_it_has_enough_points() {
        let mut p = RoundPace::default();
        assert!(p.trend(90.0).is_none(), "aucun round : pas de tendance");
        for (i, secs) in [30.0f32, 40.0, 50.0].iter().enumerate() {
            p.combat_secs = *secs;
            p.finish_round(i as u8 + 1);
            if i < PACE_HISTORY - 1 {
                assert!(p.trend(90.0).is_none(), "{} round(s) : trop tôt", i + 1);
            }
        }
        assert_eq!(p.trend(90.0), Some(Pace::Holding), "moyenne 40 s sur 90 = ça tient");
    }

    /// La tendance suit les DERNIERS rounds, pas toute la run : décrocher
    /// maintenant doit se voir même après un bon départ.
    #[test]
    fn the_trend_forgets_the_old_rounds() {
        let mut p = RoundPace::default();
        for (i, secs) in [5.0f32, 5.0, 5.0, 80.0, 85.0, 88.0].iter().enumerate() {
            p.combat_secs = *secs;
            p.finish_round(i as u8 + 1);
        }
        assert_eq!(p.cleared.len(), PACE_HISTORY, "l'historique est borné");
        assert_eq!(
            p.trend(90.0),
            Some(Pace::Pressured),
            "3 rounds à ~84 s sur 90 : la tendance doit alerter malgré le bon départ"
        );
    }

    /// Un round nettoyé instantanément (respiration, salle vide) ne doit pas
    /// polluer l'historique d'un faux « 0 s » qui ferait croire que tout va bien.
    #[test]
    fn a_zero_second_round_is_not_recorded() {
        let mut p = RoundPace::default();
        p.combat_secs = 0.0;
        p.finish_round(1);
        assert!(p.cleared.is_empty(), "une respiration n'est pas une performance");
    }

    /// Les libellés sont le message : ils doivent dire QUOI FAIRE.
    #[test]
    fn the_labels_tell_the_player_what_is_happening() {
        assert_eq!(Pace::Holding.label(), "TU TIENS");
        assert_eq!(Pace::Falling.label(), "TU DÉCROCHES");
    }
}
