//! ultimate_config.rs — Genome de tuning des Ultimes (story-596 T4a).
//!
//! Charge `assets/genomes/roguelite/roguelite_ultimate.toml` (hot-reload mtime
//! 1Hz, Shift+F12-like) → permet de régler durées/rayons/dégâts des Ultimes EN
//! LIVE sans rebuild. Le `Default` Rust est le miroir EXACT des `pub const`
//! (`ultimate.rs` + `ultimate_tech.rs`) : si le TOML disparaît ou est invalide,
//! le jeu retombe sur ces défauts (zéro régression). Miroir du pattern
//! `elements.rs` (`ElementConfig` + `ElementGenomeWatch`).

use bevy::prelude::*;
use forgia_combat::ultimate::{UltimateState, ULTIMATE_COOLDOWN_SECS, ULTIMATE_DURATION_SECS};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::ultimate_tech::{
    ULTIMATE_CHAIN_DAMAGE_FACTOR, ULTIMATE_CHAIN_HOP_RADIUS, ULTIMATE_CHAIN_MAX_EXTRA,
    ULTIMATE_EXPLOSION_DAMAGE_FACTOR, ULTIMATE_EXPLOSION_RADIUS, ULTIMATE_FREEZE_AOE_RADIUS,
    ULTIMATE_FREEZE_SECS, ULTIMATE_FREEZE_SLOW, ULTIMATE_PIERCE_DAMAGE_FACTOR,
    ULTIMATE_PIERCE_HALF_WIDTH, ULTIMATE_PIERCE_RANGE,
};

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_ultimate.toml";
const POLL_PERIOD_SEC: f32 = 1.0;

/// Pépin — explosion AOE.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct ExplosionParams {
    pub radius: f32,
    pub damage_factor: f32,
}
impl Default for ExplosionParams {
    fn default() -> Self {
        Self {
            radius: ULTIMATE_EXPLOSION_RADIUS,
            damage_factor: ULTIMATE_EXPLOSION_DAMAGE_FACTOR,
        }
    }
}

/// Bourrasque — chaîne électrique.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct ChainParams {
    pub max_extra: usize,
    pub hop_radius: f32,
    pub damage_factor: f32,
}
impl Default for ChainParams {
    fn default() -> Self {
        Self {
            max_extra: ULTIMATE_CHAIN_MAX_EXTRA,
            hop_radius: ULTIMATE_CHAIN_HOP_RADIUS,
            damage_factor: ULTIMATE_CHAIN_DAMAGE_FACTOR,
        }
    }
}

/// Mme Lenoir — perforation.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct PierceParams {
    pub range: f32,
    pub half_width: f32,
    pub damage_factor: f32,
}
impl Default for PierceParams {
    fn default() -> Self {
        Self {
            range: ULTIMATE_PIERCE_RANGE,
            half_width: ULTIMATE_PIERCE_HALF_WIDTH,
            damage_factor: ULTIMATE_PIERCE_DAMAGE_FACTOR,
        }
    }
}

/// Pompe — gel de zone.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct FreezeParams {
    pub secs: f32,
    pub slow_factor: f32,
    pub aoe_radius: f32,
}
impl Default for FreezeParams {
    fn default() -> Self {
        Self {
            secs: ULTIMATE_FREEZE_SECS,
            slow_factor: ULTIMATE_FREEZE_SLOW,
            aoe_radius: ULTIMATE_FREEZE_AOE_RADIUS,
        }
    }
}

/// Tuning complet des Ultimes (Resource + parsé du TOML, mtime hot-reload).
#[derive(Resource, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default)]
pub struct UltimateConfig {
    pub duration: f32,
    pub cooldown: f32,
    pub explosion: ExplosionParams,
    pub chain: ChainParams,
    pub pierce: PierceParams,
    pub freeze: FreezeParams,
}

impl Default for UltimateConfig {
    fn default() -> Self {
        Self {
            duration: ULTIMATE_DURATION_SECS,
            cooldown: ULTIMATE_COOLDOWN_SECS,
            explosion: ExplosionParams::default(),
            chain: ChainParams::default(),
            pierce: PierceParams::default(),
            freeze: FreezeParams::default(),
        }
    }
}

impl UltimateConfig {
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

/// Watch mtime du TOML (miroir [`crate::elements::ElementGenomeWatch`]).
#[derive(Resource, Default, Debug)]
pub struct UltimateGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

/// Recopie durée/cooldown du genome vers `UltimateState` (forgia-combat). Le
/// changement ne s'applique qu'à la PROCHAINE activation (un cd en cours garde
/// sa valeur calculée). Pas de modif si déjà égal (évite un faux Changed).
fn apply_to_state(cfg: &UltimateConfig, ult: &mut UltimateState) {
    if ult.duration != cfg.duration {
        ult.duration = cfg.duration;
    }
    if ult.cooldown != cfg.cooldown {
        ult.cooldown = cfg.cooldown;
    }
}

/// Startup : charge le TOML, initialise le watch, applique à `UltimateState`.
pub fn sys_init_ultimate_genome(mut commands: Commands, mut ult: ResMut<UltimateState>) {
    let cfg = UltimateConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    apply_to_state(&cfg, &mut ult);
    info!(
        "[ultimate] genome chargé — durée={:.0}s cd={:.0}s | explo r{:.1} | chaîne {}×{:.1}m | perfo {:.0}m | gel {:.1}s r{:.1}",
        cfg.duration, cfg.cooldown, cfg.explosion.radius, cfg.chain.max_extra,
        cfg.chain.hop_radius, cfg.pierce.range, cfg.freeze.secs, cfg.freeze.aoe_radius,
    );
    commands.insert_resource(cfg);
    commands.insert_resource(UltimateGenomeWatch {
        last_mtime: mtime,
        ..default()
    });
}

/// Poll mtime 1Hz, re-parse si changé (hot-reload Shift+F12-like).
pub fn sys_hot_reload_ultimate_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<UltimateConfig>,
    mut watch: ResMut<UltimateGenomeWatch>,
    mut ult: ResMut<UltimateState>,
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
    let new_cfg = UltimateConfig::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_cfg != *cfg {
        watch.reload_count = watch.reload_count.saturating_add(1);
        apply_to_state(&new_cfg, &mut ult);
        info!(
            "[ultimate] HOT-RELOADED #{} — durée={:.0}s cd={:.0}s gel={:.1}s",
            watch.reload_count, new_cfg.duration, new_cfg.cooldown, new_cfg.freeze.secs,
        );
        *cfg = new_cfg;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mirrors_consts() {
        let c = UltimateConfig::default();
        assert_eq!(c.duration, ULTIMATE_DURATION_SECS);
        assert_eq!(c.cooldown, ULTIMATE_COOLDOWN_SECS);
        assert_eq!(c.explosion.radius, ULTIMATE_EXPLOSION_RADIUS);
        assert_eq!(c.chain.max_extra, ULTIMATE_CHAIN_MAX_EXTRA);
        assert_eq!(c.pierce.range, ULTIMATE_PIERCE_RANGE);
        assert_eq!(c.freeze.secs, ULTIMATE_FREEZE_SECS);
    }

    #[test]
    fn parse_full_toml_roundtrip() {
        let toml = r#"
duration = 8.0
cooldown = 20.0
[explosion]
radius = 6.0
damage_factor = 1.0
[chain]
max_extra = 5
hop_radius = 5.5
damage_factor = 0.7
[pierce]
range = 30.0
half_width = 2.0
damage_factor = 0.9
[freeze]
secs = 3.0
slow_factor = 0.2
aoe_radius = 6.5
"#;
        let c = UltimateConfig::parse_toml(toml);
        assert_eq!(c.duration, 8.0);
        assert_eq!(c.chain.max_extra, 5);
        assert_eq!(c.freeze.slow_factor, 0.2);
        assert_eq!(c.pierce.half_width, 2.0);
    }

    #[test]
    fn parse_garbage_falls_back_to_default() {
        assert_eq!(
            UltimateConfig::parse_toml("pas du TOML [[["),
            UltimateConfig::default()
        );
    }

    #[test]
    fn missing_section_uses_default_for_that_section() {
        // Seul [freeze] override → le reste garde les défauts (serde default).
        let toml = "duration = 12.0\n[freeze]\nsecs = 4.0\nslow_factor = 0.0\naoe_radius = 7.0\n";
        let c = UltimateConfig::parse_toml(toml);
        assert_eq!(c.duration, 12.0);
        assert_eq!(c.freeze.secs, 4.0);
        assert_eq!(c.explosion, ExplosionParams::default(), "section absente → défaut");
        assert_eq!(c.chain, ChainParams::default());
    }

    #[test]
    fn apply_to_state_overrides_timers() {
        let mut ult = UltimateState::default();
        let cfg = UltimateConfig {
            duration: 7.0,
            cooldown: 18.0,
            ..UltimateConfig::default()
        };
        apply_to_state(&cfg, &mut ult);
        assert_eq!(ult.duration, 7.0);
        assert_eq!(ult.cooldown, 18.0);
    }
}