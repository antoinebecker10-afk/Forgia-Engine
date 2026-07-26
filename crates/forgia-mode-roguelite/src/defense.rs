//! defense.rs — Story-640 P0-2 : données + systèmes de la défense tri-couche.
//!
//! Le **mécanisme** (`DefenseLayer`, absorption Bouclier→Armure→Vie, régén) vit dans
//! `forgia-damage` (crate atomique partagé joueur/ennemis). Ce module côté mode fournit :
//! - `DefenseConfig` : les **valeurs** par archétype + joueur (genome `roguelite_defense.toml`,
//!   hot-reload mtime, `Default` = miroir exact) — même séparation que `elements.rs`.
//! - la **régénération** du bouclier hors combat (ennemis + joueur),
//! - l'**attache** de la couche au joueur (idempotent, retirée OnExit),
//! - le **sensor** `forgia2_shield.json` + health alert.
//!
//! L'attache de la couche aux ENNEMIS se fait au spawn (`waves.rs`, consomme ce config).

use bevy::prelude::*;
use forgia_damage::DefenseLayer;
use forgia_player::Player;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::enemies::EnemyArchetype;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_defense.toml";
const POLL_PERIOD_SEC: f32 = 1.0;
const SENSOR_PATH: &str = "forgia2_shield.json";

/// Capacités défensives d'une entité (bouclier + armure max). Vie = ailleurs.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct DefensePool {
    pub shield_max: f32,
    pub armor_max: f32,
}

/// Paramètres de régénération du bouclier (partagés ennemis + joueur).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct RegenParams {
    /// Points de bouclier restaurés par seconde hors combat.
    pub rate: f32,
    /// Secondes sans coup avant le début de la régén.
    pub delay: f32,
}

/// Config de la défense tri-couche — Resource data-driven (genome TOML, hot-reload).
/// `Default` = miroir EXACT de `roguelite_defense.toml` → zéro régression si absent.
#[derive(Resource, Deserialize, Clone, Debug, PartialEq)]
pub struct DefenseConfig {
    pub tank: DefensePool,
    pub runner: DefensePool,
    pub sniper: DefensePool,
    pub boss: DefensePool,
    pub player: DefensePool,
    pub regen: RegenParams,
}

impl Default for DefenseConfig {
    fn default() -> Self {
        Self {
            tank: DefensePool {
                shield_max: 0.0,
                armor_max: 80.0,
            },
            runner: DefensePool {
                shield_max: 30.0,
                armor_max: 0.0,
            },
            sniper: DefensePool {
                shield_max: 40.0,
                armor_max: 0.0,
            },
            boss: DefensePool {
                shield_max: 200.0,
                armor_max: 150.0,
            },
            player: DefensePool {
                shield_max: 50.0,
                armor_max: 0.0,
            },
            regen: RegenParams {
                rate: 20.0,
                delay: 3.0,
            },
        }
    }
}

impl DefenseConfig {
    /// Pool défensif d'un archétype (data live).
    pub fn pool_for(&self, a: EnemyArchetype) -> DefensePool {
        match a {
            EnemyArchetype::Tank => self.tank,
            EnemyArchetype::Runner => self.runner,
            EnemyArchetype::Sniper => self.sniper,
            EnemyArchetype::Boss => self.boss,
        }
    }

    /// Construit une `DefenseLayer` pleine pour un archétype (appelé au spawn ennemi).
    pub fn layer_for(&self, a: EnemyArchetype) -> DefenseLayer {
        let p = self.pool_for(a);
        DefenseLayer::new(p.shield_max, p.armor_max, self.regen.rate, self.regen.delay)
    }

    /// Construit la `DefenseLayer` du joueur.
    pub fn player_layer(&self) -> DefenseLayer {
        DefenseLayer::new(
            self.player.shield_max,
            self.player.armor_max,
            self.regen.rate,
            self.regen.delay,
        )
    }

    /// Pur — testable headless. Fallback `Default` sur toute erreur de parse.
    pub fn parse_toml(content: &str) -> Self {
        toml::from_str(content).unwrap_or_else(|_| Self::default())
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

/// Watch mtime du TOML (hot-reload, miroir `EnemyGenomeWatch`).
#[derive(Resource, Default, Debug)]
pub struct DefenseGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

// ─── Systems : genome load + hot-reload ─────────────────────────────────────

/// Startup : charge le TOML + init le watch mtime.
pub fn sys_init_defense_genome(mut commands: Commands) {
    let cfg = DefenseConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    info!(
        "[defense] genome chargé — tank(sh{:.0}/ar{:.0}) runner(sh{:.0}) boss(sh{:.0}/ar{:.0}) joueur(sh{:.0}) regen {:.0}/s après {:.1}s",
        cfg.tank.shield_max, cfg.tank.armor_max, cfg.runner.shield_max,
        cfg.boss.shield_max, cfg.boss.armor_max, cfg.player.shield_max,
        cfg.regen.rate, cfg.regen.delay,
    );
    commands.insert_resource(cfg);
    commands.insert_resource(DefenseGenomeWatch {
        last_mtime: mtime,
        ..default()
    });
}

/// Poll mtime 1Hz, re-parse si changé (hot-reload). Ne re-baigne PAS les couches
/// déjà spawnées (les nouvelles valeurs s'appliquent aux prochains spawns + à la
/// régén via `regen.rate/delay` lus live) — cohérent avec `enemies.rs`.
pub fn sys_hot_reload_defense_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<DefenseConfig>,
    mut watch: ResMut<DefenseGenomeWatch>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let Ok(meta) = fs::metadata(GENOME_PATH) else {
        return;
    };
    let Ok(mtime) = meta.modified() else {
        return;
    };
    if watch.last_mtime == Some(mtime) {
        return;
    }
    let Ok(content) = fs::read_to_string(GENOME_PATH) else {
        return;
    };
    let new_cfg = DefenseConfig::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_cfg != *cfg {
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!(
            "[defense] HOT-RELOADED #{} — regen {:.0}/s après {:.1}s",
            watch.reload_count, new_cfg.regen.rate, new_cfg.regen.delay,
        );
        *cfg = new_cfg;
    }
}

// ─── Régénération du bouclier (ennemis + joueur) ────────────────────────────

/// FixedUpdate — régénère le bouclier de toute `DefenseLayer` hors combat
/// (`since_hit >= regen_delay`). Cadence déterministe (comme le DoT élémentaire,
/// story-634). O(entités défendues) ≈ ≤ waves + 1 joueur → coût négligeable.
pub fn sys_regen_defense(time: Res<Time>, mut q: Query<&mut DefenseLayer>) {
    let dt = time.delta_secs();
    for mut dl in &mut q {
        dl.regen(dt);
    }
}

// ─── Attache / détache de la couche joueur ──────────────────────────────────

/// Attache la `DefenseLayer` au joueur (idempotent : `Without<DefenseLayer>`).
/// Le joueur est spawné par `forgia-player` (ordre cross-plugin non garanti) → on
/// tag en Update comme `sys_tag_player_as_collector`. Valeurs live depuis le genome.
pub fn sys_attach_player_defense(
    mut commands: Commands,
    config: Res<DefenseConfig>,
    q_player: Query<Entity, (With<Player>, Without<DefenseLayer>)>,
) {
    for e in &q_player {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.insert(config.player_layer());
            info!(
                "[defense] Player {e:?} → DefenseLayer (bouclier {:.0}, armure {:.0})",
                config.player.shield_max, config.player.armor_max
            );
        }
    }
}

/// OnExit Roguelite — retire la `DefenseLayer` du joueur (sinon Arena/RPG en hérite,
/// miroir de `sys_remove_player_health_guard`).
pub fn sys_remove_player_defense(
    mut commands: Commands,
    q_player: Query<Entity, (With<Player>, With<DefenseLayer>)>,
) {
    for e in &q_player {
        commands.entity(e).remove::<DefenseLayer>();
    }
}

// ─── Sensor forgia2_shield.json ─────────────────────────────────────────────

/// Écrit `forgia2_shield.json` 1Hz : bouclier/armure agrégés ennemis + joueur, en
/// régén. Health alert `warn` si des ennemis existent mais aucun ne porte de
/// `DefenseLayer` alors que le genome prévoit des couches (wiring spawn cassé).
pub fn sys_write_shield_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    config: Res<DefenseConfig>,
    q_enemy: Query<&DefenseLayer, With<EnemyArchetype>>,
    q_enemy_all: Query<(), With<EnemyArchetype>>,
    q_player: Query<&DefenseLayer, With<Player>>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;

    let mut e_shield = 0.0f32;
    let mut e_shield_max = 0.0f32;
    let mut e_armor = 0.0f32;
    let mut e_armor_max = 0.0f32;
    let mut e_regen = 0u32;
    let mut e_defended = 0u32;
    for dl in &q_enemy {
        e_shield += dl.shield;
        e_shield_max += dl.shield_max;
        e_armor += dl.armor;
        e_armor_max += dl.armor_max;
        if dl.is_regenerating() {
            e_regen += 1;
        }
        e_defended += 1;
    }
    let enemies_total = q_enemy_all.iter().count() as u32;

    let player = q_player.iter().next();
    let (p_shield, p_shield_max, p_armor, p_armor_max, p_regen, p_present) = match player {
        Some(dl) => (
            dl.shield,
            dl.shield_max,
            dl.armor,
            dl.armor_max,
            dl.is_regenerating(),
            true,
        ),
        None => (0.0, 0.0, 0.0, 0.0, false, false),
    };

    // Le genome prévoit-il une couche pour au moins un archétype ?
    let config_has_layers = [config.tank, config.runner, config.sniper, config.boss]
        .iter()
        .any(|p| p.shield_max > 0.0 || p.armor_max > 0.0);
    let (severity, next_step) = if enemies_total > 0 && e_defended == 0 && config_has_layers {
        (
            "warn",
            "0 ennemi porte DefenseLayer alors que le genome prévoit des couches — spawn_live/attach cassé (waves.rs)",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"shield","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"regen":{{"rate":{:.1},"delay":{:.1}}},"enemies":{{"total":{enemies_total},"defended":{e_defended},"shield":{e_shield:.0},"shield_max":{e_shield_max:.0},"armor":{e_armor:.0},"armor_max":{e_armor_max:.0},"regenerating":{e_regen}}},"player":{{"present":{p_present},"shield":{p_shield:.0},"shield_max":{p_shield_max:.0},"armor":{p_armor:.0},"armor_max":{p_armor_max:.0},"regenerating":{p_regen}}}}}"#,
        time.elapsed_secs(),
        config.regen.rate,
        config.regen.delay,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[defense] sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pools_match_gunfire_lite_design() {
        let c = DefenseConfig::default();
        assert_eq!(
            c.tank.armor_max, 80.0,
            "Tank = armure lourde, pas de bouclier"
        );
        assert_eq!(c.tank.shield_max, 0.0);
        assert_eq!(
            c.runner.shield_max, 30.0,
            "Runner = bouclier léger régénérant"
        );
        assert!(
            c.boss.shield_max > 0.0 && c.boss.armor_max > 0.0,
            "Boss = deux couches"
        );
        assert_eq!(c.player.shield_max, 50.0);
    }

    #[test]
    fn layer_for_builds_full_layer() {
        let c = DefenseConfig::default();
        let l = c.layer_for(EnemyArchetype::Boss);
        assert_eq!(l.shield, 200.0);
        assert_eq!(l.shield_max, 200.0);
        assert_eq!(l.armor, 150.0);
        assert_eq!(l.regen_rate, c.regen.rate);
        assert_eq!(l.regen_delay, c.regen.delay);
    }

    #[test]
    fn player_layer_uses_player_pool() {
        let c = DefenseConfig::default();
        let l = c.player_layer();
        assert_eq!(l.shield_max, c.player.shield_max);
        assert_eq!(l.armor_max, c.player.armor_max);
    }

    #[test]
    fn parse_garbage_falls_back_to_default() {
        assert_eq!(
            DefenseConfig::parse_toml("pas du TOML valide [[["),
            DefenseConfig::default()
        );
    }

    #[test]
    fn parse_roundtrip_overrides_default() {
        let toml = r#"
[tank]
shield_max = 100.0
armor_max = 100.0
[runner]
shield_max = 30.0
armor_max = 0.0
[sniper]
shield_max = 40.0
armor_max = 0.0
[boss]
shield_max = 200.0
armor_max = 150.0
[player]
shield_max = 50.0
armor_max = 0.0
[regen]
rate = 20.0
delay = 3.0
"#;
        let c = DefenseConfig::parse_toml(toml);
        assert_eq!(c.tank.shield_max, 100.0, "override tank shield lu du TOML");
        assert_ne!(
            c,
            DefenseConfig::default(),
            "diff du Default → hot-reload déclenchable"
        );
    }
}
