//! enemy_scaling.rs — story-658 : scaling des ennemis par **profondeur de salle**.
//!
//! LA PRESSION qui donne son sens à « La Trempe » (story-653/657) : plus la run
//! avance (`RogueliteWave.stage`), plus les ennemis sont durs (vie + défenses +
//! dégâts). Sans elle, monter son arme rendait le jeu trivial (arme qui monte,
//! rien qui résiste). Salle 0 = ×1.0 (référence), salle N = ×(1 + N × per_stage).
//!
//! ## Post-spawn (concept-first + multi-terminal)
//! Le scaling s'applique **après le spawn** via `Added<EnemyArchetype>` sur la racine
//! ennemi (qui porte `Health` + `DefenseLayer` + `BotShootConfig` sur la MÊME entité,
//! cf `waves.rs` parent tuple) — PAS dans `spawn_wave_enemies`. Raison : waves.rs est
//! un fichier chaud multi-terminal ; ce module n'y touche pas (zéro collision). Effet
//! identique : l'ennemi vit ~1 frame à ses stats de base avant scaling (imperceptible,
//! il spawn pendant le break sans combat).
//!
//! Idempotent : `Added<>` ne se déclenche qu'une fois par entité → jamais de
//! double-scaling. Data-driven `roguelite_progression.toml [scaling]` (hot-reload).

use bevy::prelude::*;
use forgia_ai_arena_bot::BotShootConfig;
use forgia_combat::Health;
use forgia_core::prelude::*;
use forgia_damage::DefenseLayer;
use serde::Deserialize;
use std::fs;
use std::time::SystemTime;

use crate::enemies::EnemyArchetype;
use crate::run::RunState;
use crate::waves::RogueliteWave;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_progression.toml";
const SENSOR_PATH: &str = "forgia2_enemy_scaling.json";
const POLL_PERIOD_SEC: f32 = 1.0;

// ─── Config (genome, hot-reload) ─────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct ProgressionToml {
    #[serde(default)]
    scaling: ScalingToml,
}

#[derive(Deserialize, Default)]
struct ScalingToml {
    enabled: Option<bool>,
    hp_per_stage: Option<f32>,
    damage_per_stage: Option<f32>,
}

/// Config du scaling — `[scaling]` de `roguelite_progression.toml` (hot-reload).
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct EnemyScalingConfig {
    pub enabled: bool,
    pub hp_per_stage: f32,
    pub damage_per_stage: f32,
}

impl Default for EnemyScalingConfig {
    fn default() -> Self {
        // Miroir EXACT de assets/genomes/roguelite/roguelite_progression.toml [scaling].
        Self {
            enabled: true,
            hp_per_stage: 0.35,
            damage_per_stage: 0.15,
        }
    }
}

impl EnemyScalingConfig {
    pub fn parse_toml(content: &str) -> Self {
        let parsed: ProgressionToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let s = parsed.scaling;
        let d = Self::default();
        Self {
            enabled: s.enabled.unwrap_or(d.enabled),
            hp_per_stage: s.hp_per_stage.unwrap_or(d.hp_per_stage).clamp(0.0, 10.0),
            damage_per_stage: s
                .damage_per_stage
                .unwrap_or(d.damage_per_stage)
                .clamp(0.0, 10.0),
        }
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }

    /// Multiplicateur de vie/défenses pour une salle : `1 + stage × hp_per_stage`.
    ///
    /// ⚠️ Story-677 — chemin de REPLI. Quand `[boucle] enabled = true` dans
    /// `roguelite_rounds.toml`, c'est `rounds::RoundsConfig::threat()` qui
    /// s'applique : géométrique + paliers, parce qu'une droite finit toujours
    /// par se faire rattraper par les multiplicateurs du joueur.
    pub fn hp_mul_for_stage(&self, stage: u8) -> f32 {
        1.0 + f32::from(stage) * self.hp_per_stage
    }

    /// Multiplicateur de dégâts ennemis pour une salle : `1 + stage × damage_per_stage`.
    pub fn damage_mul_for_stage(&self, stage: u8) -> f32 {
        1.0 + f32::from(stage) * self.damage_per_stage
    }
}

#[derive(Resource, Default, Debug)]
pub struct EnemyScalingWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
    /// Cumul d'ennemis scalés cette run (sensor) + dernier multiplicateur appliqué.
    pub scaled_total: u32,
    pub last_hp_mul: f32,
    pub last_dmg_mul: f32,
    pub last_stage: u8,
}

// ─── Systèmes ────────────────────────────────────────────────────────────────

pub fn sys_init_enemy_scaling(mut commands: Commands) {
    let cfg = EnemyScalingConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(cfg);
    commands.insert_resource(EnemyScalingWatch {
        last_mtime: mtime,
        last_hp_mul: 1.0,
        last_dmg_mul: 1.0,
        ..Default::default()
    });
}

pub fn sys_hot_reload_enemy_scaling(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<EnemyScalingConfig>,
    mut watch: ResMut<EnemyScalingWatch>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
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
    let new_cfg = EnemyScalingConfig::parse_toml(&content);
    if new_cfg != *cfg {
        *cfg = new_cfg;
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!("[enemy_scaling] HOT-RELOADED");
    }
}

/// Applique le scaling à chaque ennemi fraîchement spawné, selon la salle courante.
/// `Added<EnemyArchetype>` = une fois par ennemi. Multiplie Vie + Bouclier + Armure
/// (× hp_mul) et les dégâts de tir (× dmg_mul). Salle 0 → mul 1.0 → no-op silencieux.
#[allow(clippy::type_complexity)]
pub fn sys_scale_enemies_by_depth(
    cfg: Res<EnemyScalingConfig>,
    // Story-677 — quand la boucle de rounds est active, c'est ELLE qui porte la
    // courbe (géométrique + paliers). Le linéaire de `[scaling]` reste le
    // chemin d'avant, utilisé si `[boucle] enabled = false`. UNE seule courbe
    // s'applique à la fois : deux sources de difficulté, c'est une balance
    // qu'on ne sait plus lire.
    rounds: Option<Res<crate::rounds::RoundsConfig>>,
    wave: Res<RogueliteWave>,
    // 2026-08-04 — la menace monte par ROUND (chaque combat), pas par ARÈNE.
    // Avec plusieurs combats par arène, indexer sur `stage` faisait monter la
    // difficulté par bonds pendant que le joueur enchaînait trois combats
    // identiques : le palier ne tombait plus là où le HUD l'annonçait.
    graph_cfg: Res<forgia_stage::graph::RunGraphConfig>,
    // 2026-08-04 — le chapitre courant : la marche ENTRE chapitres.
    chapter: Option<Res<crate::meta_shop::SelectedChapter>>,
    mut watch: ResMut<EnemyScalingWatch>,
    mut q: Query<
        (
            &mut Health,
            Option<&mut DefenseLayer>,
            Option<&mut BotShootConfig>,
        ),
        Added<EnemyArchetype>,
    >,
) {
    if !cfg.enabled {
        return;
    }
    let stage = wave.stage;
    let (hp_mul, dmg_mul) = match rounds.as_deref() {
        Some(r) if r.enabled => {
            // Round 1 = référence ×1.0, comme la salle 0 l'était : `threat`
            // attend un index 0-basé, `round()` rend un compteur 1-basé.
            // Le CHAPITRE compose par-dessus la montée du round : c'est lui qui
            // rend le Livre progressif, et c'est la progression permanente qui
            // doit le combler (la Trempe et les atouts repartent à zéro).
            let chapitre = chapter.map(|c| c.0).unwrap_or(1);
            let t = r.threat_at(
                chapitre,
                wave.round(graph_cfg.waves_per_stage).saturating_sub(1),
            );
            (t.hp, t.damage)
        }
        // Repli hors boucle : le linéaire historique, indexé sur la SALLE — il n'y
        // a pas de notion de round quand c'est le graphe qui pilote.
        _ => (cfg.hp_mul_for_stage(stage), cfg.damage_mul_for_stage(stage)),
    };
    watch.last_stage = stage;
    watch.last_hp_mul = hp_mul;
    watch.last_dmg_mul = dmg_mul;
    // Salle 0 (référence) : rien à multiplier — mais on itère quand même pour
    // consommer `Added` proprement (coût nul, la boucle est vide au stage 0).
    if hp_mul <= 1.0 && dmg_mul <= 1.0 {
        return;
    }
    for (mut hp, defense, shoot) in &mut q {
        // Ennemi fraîchement spawné → current == max ; scaler les deux garde full HP.
        hp.max *= hp_mul;
        hp.current *= hp_mul;
        if let Some(mut d) = defense {
            d.shield *= hp_mul;
            d.shield_max *= hp_mul;
            d.armor *= hp_mul;
            d.armor_max *= hp_mul;
        }
        if let Some(mut s) = shoot {
            s.damage *= dmg_mul;
        }
        watch.scaled_total = watch.scaled_total.saturating_add(1);
    }
}

/// Reset des compteurs de run au retour Lobby (le sensor repart propre).
pub fn sys_reset_enemy_scaling(mut watch: ResMut<EnemyScalingWatch>) {
    watch.scaled_total = 0;
    watch.last_stage = 0;
    watch.last_hp_mul = 1.0;
    watch.last_dmg_mul = 1.0;
}

// ─── Sensor ──────────────────────────────────────────────────────────────────

pub fn sys_write_enemy_scaling_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cfg: Option<Res<EnemyScalingConfig>>,
    watch: Option<Res<EnemyScalingWatch>>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let (Some(cfg), Some(watch)) = (cfg, watch) else {
        return;
    };
    let severity = if cfg.enabled { "ok" } else { "info" };
    let next_step = if cfg.enabled {
        ""
    } else {
        "Scaling ennemi désactivé (enabled=false)."
    };
    let json = format!(
        r#"{{"id":"enemy_scaling","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"enabled":{},"stage":{},"hp_mul":{:.3},"damage_mul":{:.3},"scaled_total":{},"reload_count":{}}}"#,
        time.elapsed_secs(),
        cfg.enabled,
        watch.last_stage,
        watch.last_hp_mul,
        watch.last_dmg_mul,
        watch.scaled_total,
        watch.reload_count,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[enemy_scaling] sensor write failed: {e}");
    }
}

// ─── Plugin ──────────────────────────────────────────────────────────────────

pub struct RogueliteEnemyScalingPlugin;

impl Plugin for RogueliteEnemyScalingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, sys_init_enemy_scaling);
        app.add_systems(OnEnter(RunState::Lobby), sys_reset_enemy_scaling);
        app.add_systems(
            Update,
            (sys_hot_reload_enemy_scaling, sys_scale_enemies_by_depth)
                .chain()
                .in_set(GameSet::Combat)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_write_enemy_scaling_sensor.in_set(GameSet::Sensors),
        );
    }
}

// ─── Tests purs ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_default() {
        assert_eq!(
            EnemyScalingConfig::parse_toml(""),
            EnemyScalingConfig::default()
        );
    }

    #[test]
    fn parse_reads_scaling_section() {
        let c = EnemyScalingConfig::parse_toml("[scaling]\nenabled = false\nhp_per_stage = 0.5\n");
        assert!(!c.enabled);
        assert!((c.hp_per_stage - 0.5).abs() < 1e-6);
        // damage_per_stage non précisé → défaut.
        assert!((c.damage_per_stage - 0.15).abs() < 1e-6);
    }

    #[test]
    fn ignores_trempe_section() {
        // Le fichier partage [trempe] et [scaling] : parser [scaling] ignore [trempe].
        let c = EnemyScalingConfig::parse_toml(
            "[trempe]\ncost_base = 999\n[scaling]\nhp_per_stage = 0.2\n",
        );
        assert!((c.hp_per_stage - 0.2).abs() < 1e-6);
    }

    #[test]
    fn stage_zero_is_reference() {
        let c = EnemyScalingConfig::default();
        assert!((c.hp_mul_for_stage(0) - 1.0).abs() < 1e-6);
        assert!((c.damage_mul_for_stage(0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn scaling_grows_with_stage() {
        let c = EnemyScalingConfig::default(); // hp 0.35, dmg 0.15
        assert!((c.hp_mul_for_stage(2) - 1.70).abs() < 1e-6);
        assert!((c.damage_mul_for_stage(2) - 1.30).abs() < 1e-6);
        assert!(c.hp_mul_for_stage(3) > c.hp_mul_for_stage(2));
    }

    #[test]
    fn parse_garbage_falls_back() {
        assert_eq!(
            EnemyScalingConfig::parse_toml("pas du toml [[["),
            EnemyScalingConfig::default()
        );
    }
}
