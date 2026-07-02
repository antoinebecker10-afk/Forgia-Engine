//! # forgia-juice-hit-stop
//!
//! Hit-stop time pause — sur hit confirmé, ralentit `Time<Virtual>` brièvement
//! puis restaure. Pattern Apex / Doom Eternal pour la sensation d'impact.
//!
//! Source de vérité Forgia V2 — extrait depuis `forgia-combat::combat_juice`
//! (Tier 1D refacto fine-grained-crates, 2026-05-17).
//!
//! Story-648 (2026-07-02) — **paliers** : la durée de base PAR-ARME
//! (`hit_stop_duration` du genome arme) est multipliée selon l'issue du tir :
//! hit normal ×1.0 (feel historique inchangé) < crit/weakspot < kill <
//! multikill (≥2 kills dans le MÊME tir — shotgun/sniper perçant). Réfs audit
//! VFX 2026-07-02 : SF2/ULTRAKILL/Vlambeer — le hitstop des KILLS doit être
//! nettement plus long que celui des hits. Multiplicateurs en genome plat
//! hot-reload (`roguelite_gamefeel.toml`) + capteur `forgia2_gamefeel.json`.

use bevy::prelude::*;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

pub mod prelude {
    pub use super::{ForgiaJuiceHitStopPlugin, HitStopState, HitstopStats, HitstopTiers};
}

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_gamefeel.toml";
const SENSOR_PATH: &str = "forgia2_gamefeel.json";
const POLL_PERIOD_SEC: f32 = 1.0;

/// Resource éphémère : insérée par un caller (fire system) pour déclencher
/// un hit-stop. Le system `hitstop_tick_system` la consomme et restaure la
/// vitesse `Time<Virtual>` à `restore_speed` quand `timer.is_finished()`.
#[derive(Resource)]
pub struct HitStopState {
    pub timer: Timer,
    pub restore_speed: f32,
}

/// Multiplicateurs de palier appliqués à la durée de base par-arme.
/// Base 0.05 s → hit 50 ms / crit 75 ms / kill 125 ms / multikill 200 ms
/// (défauts alignés sur les cibles de l'audit VFX 2026-07-02).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct HitstopTiers {
    pub crit_mult: f32,
    pub kill_mult: f32,
    pub multikill_mult: f32,
}

impl Default for HitstopTiers {
    fn default() -> Self {
        Self {
            crit_mult: 1.5,
            kill_mult: 2.5,
            multikill_mult: 4.0,
        }
    }
}

#[derive(Deserialize)]
struct GeneToml {
    id: String,
    #[serde(default)]
    default: f32,
}

#[derive(Deserialize)]
struct GamefeelGenomeToml {
    #[serde(default)]
    genes: Vec<GeneToml>,
}

impl HitstopTiers {
    /// Pur — testable. Genes plats, pattern render_quality/toon_config.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: GamefeelGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut t = Self::default();
        for gene in &parsed.genes {
            match gene.id.as_str() {
                "hitstop_crit_mult" => t.crit_mult = gene.default.clamp(1.0, 8.0),
                "hitstop_kill_mult" => t.kill_mult = gene.default.clamp(1.0, 8.0),
                "hitstop_multikill_mult" => {
                    t.multikill_mult = gene.default.clamp(1.0, 8.0);
                }
                _ => {}
            }
        }
        t
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

/// Pur — choisit (multiplicateur, label de palier) pour un tir résolu.
/// Priorité : multikill (≥2 kills même tir) > kill > crit/weakspot > hit.
pub fn tier_for_shot(tiers: &HitstopTiers, kills: u32, crit_or_head: bool) -> (f32, &'static str) {
    if kills >= 2 {
        (tiers.multikill_mult, "multikill")
    } else if kills == 1 {
        (tiers.kill_mult, "kill")
    } else if crit_or_head {
        (tiers.crit_mult, "crit")
    } else {
        (1.0, "hit")
    }
}

/// Compteurs par palier (capteur). Rempli par le fire system via `record`.
#[derive(Resource, Debug)]
pub struct HitstopStats {
    pub hits: u32,
    pub crits: u32,
    pub kills: u32,
    pub multikills: u32,
    pub last_tier: &'static str,
    pub last_duration_secs: f32,
}

impl Default for HitstopStats {
    fn default() -> Self {
        Self {
            hits: 0,
            crits: 0,
            kills: 0,
            multikills: 0,
            last_tier: "none",
            last_duration_secs: 0.0,
        }
    }
}

impl HitstopStats {
    pub fn record(&mut self, tier: &'static str, duration_secs: f32) {
        match tier {
            "crit" => self.crits = self.crits.saturating_add(1),
            "kill" => self.kills = self.kills.saturating_add(1),
            "multikill" => self.multikills = self.multikills.saturating_add(1),
            _ => self.hits = self.hits.saturating_add(1),
        }
        self.last_tier = tier;
        self.last_duration_secs = duration_secs;
    }
}

/// Suivi mtime genome (hot-reload 1Hz).
#[derive(Resource, Default)]
pub struct GamefeelGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

pub struct ForgiaJuiceHitStopPlugin;

impl Plugin for ForgiaJuiceHitStopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HitstopStats>()
            .add_systems(Startup, sys_init_gamefeel_genome)
            .add_systems(
                Update,
                (
                    hitstop_tick_system.run_if(resource_exists::<HitStopState>),
                    sys_hot_reload_gamefeel_genome,
                    sys_write_gamefeel_sensor,
                ),
            );
    }
}

/// Tick le timer hit-stop. Restaure `Time<Virtual>` à `restore_speed` puis
/// retire la Resource quand fini.
pub fn hitstop_tick_system(
    mut commands: Commands,
    real_time: Res<Time<Real>>,
    mut time: ResMut<Time<Virtual>>,
    mut state: ResMut<HitStopState>,
) {
    state.timer.tick(real_time.delta());
    if state.timer.is_finished() {
        time.set_relative_speed(state.restore_speed);
        commands.remove_resource::<HitStopState>();
    }
}

fn sys_init_gamefeel_genome(mut commands: Commands) {
    let tiers = HitstopTiers::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(tiers);
    commands.insert_resource(GamefeelGenomeWatch {
        last_mtime: mtime,
        ..Default::default()
    });
    info!(
        "[juice-hitstop] gamefeel genome loaded — tiers crit×{:.1} kill×{:.1} multikill×{:.1}",
        tiers.crit_mult, tiers.kill_mult, tiers.multikill_mult
    );
}

fn sys_hot_reload_gamefeel_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    tiers: Option<ResMut<HitstopTiers>>,
    watch: Option<ResMut<GamefeelGenomeWatch>>,
) {
    let (Some(mut tiers), Some(mut watch)) = (tiers, watch) else {
        return;
    };
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
    let new_tiers = HitstopTiers::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_tiers != *tiers {
        *tiers = new_tiers;
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!(
            "[juice-hitstop] HOT-RELOADED — crit×{:.1} kill×{:.1} multikill×{:.1}",
            new_tiers.crit_mult, new_tiers.kill_mult, new_tiers.multikill_mult
        );
    }
}

fn sys_write_gamefeel_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    stats: Option<Res<HitstopStats>>,
    tiers: Option<Res<HitstopTiers>>,
    watch: Option<Res<GamefeelGenomeWatch>>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let (Some(stats), Some(tiers), Some(watch)) = (stats, tiers, watch) else {
        return;
    };
    let json = format!(
        r#"{{"id":"gamefeel","severity":"ok","next_step":"Paliers hitstop tunables dans roguelite_gamefeel.toml (hot-reload 1Hz). Base par-arme = hit_stop_duration (genome arme).","timestamp_secs":{:.1},"hitstop_counts":{{"hit":{},"crit":{},"kill":{},"multikill":{}}},"last_tier":"{}","last_duration_ms":{:.0},"tiers":{{"crit_mult":{:.2},"kill_mult":{:.2},"multikill_mult":{:.2}}},"reload_count":{}}}"#,
        time.elapsed_secs(),
        stats.hits,
        stats.crits,
        stats.kills,
        stats.multikills,
        stats.last_tier,
        stats.last_duration_secs * 1000.0,
        tiers.crit_mult,
        tiers.kill_mult,
        tiers.multikill_mult,
        watch.reload_count,
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[juice-hitstop] sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaJuiceHitStopPlugin;
    }

    #[test]
    fn state_constructible() {
        let _s = HitStopState {
            timer: Timer::from_seconds(0.05, TimerMode::Once),
            restore_speed: 1.0,
        };
    }

    #[test]
    fn tier_priority_multikill_over_all() {
        let t = HitstopTiers::default();
        // multikill > kill > crit > hit, quel que soit le flag crit.
        assert_eq!(tier_for_shot(&t, 2, true).1, "multikill");
        assert_eq!(tier_for_shot(&t, 1, true).1, "kill");
        assert_eq!(tier_for_shot(&t, 0, true).1, "crit");
        assert_eq!(tier_for_shot(&t, 0, false).1, "hit");
    }

    #[test]
    fn tier_hit_keeps_historic_feel() {
        // Hit normal = ×1.0 exactement : le feel existant ne change PAS.
        let t = HitstopTiers::default();
        assert_eq!(tier_for_shot(&t, 0, false).0, 1.0);
    }

    #[test]
    fn tiers_ordered_ascending() {
        let t = HitstopTiers::default();
        assert!(1.0 < t.crit_mult && t.crit_mult < t.kill_mult && t.kill_mult < t.multikill_mult);
    }

    #[test]
    fn parse_overrides_and_clamps() {
        let toml = r#"
[[genes]]
id = "hitstop_kill_mult"
default = 3.0
[[genes]]
id = "hitstop_multikill_mult"
default = 99.0
"#;
        let t = HitstopTiers::parse_toml(toml);
        assert!((t.kill_mult - 3.0).abs() < 1e-6);
        assert!((t.multikill_mult - 8.0).abs() < 1e-6, "clamp 99 -> 8");
        assert!((t.crit_mult - 1.5).abs() < 1e-6, "absent = défaut");
    }

    #[test]
    fn parse_empty_returns_default() {
        assert_eq!(HitstopTiers::parse_toml(""), HitstopTiers::default());
    }

    #[test]
    fn stats_record_routes_tiers() {
        let mut s = HitstopStats::default();
        s.record("kill", 0.125);
        s.record("multikill", 0.2);
        s.record("hit", 0.05);
        s.record("crit", 0.075);
        assert_eq!((s.hits, s.crits, s.kills, s.multikills), (1, 1, 1, 1));
        assert_eq!(s.last_tier, "crit");
    }
}
