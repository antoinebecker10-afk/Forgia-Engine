//! config.rs — Configuration genome du RPG Health Monitor.
//!
//! Chargée depuis `config/genomes/rpg_monitor.toml`.
//! Reloadable via Shift+F12 in-game (sys_reload_config_on_hotkey).

use bevy::prelude::*;
use serde::Deserialize;

// ─────────────────────────── Sub-structs ───────────────────────────

#[derive(Clone, Debug, Deserialize)]
pub struct MetaConfig {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_name() -> String {
    "rpg_monitor".to_string()
}
fn default_version() -> u32 {
    1
}
fn default_true() -> bool {
    true
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            name: default_name(),
            version: default_version(),
            enabled: default_true(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_tick_interval")]
    pub tick_interval_secs: f32,
}

fn default_tick_interval() -> f32 {
    1.0
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            tick_interval_secs: default_tick_interval(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct LivenessConfig {
    #[serde(default = "default_expected_sensors")]
    pub expected_sensors: Vec<String>,
    #[serde(default = "default_stale_secs")]
    pub stale_secs: f32,
}

fn default_expected_sensors() -> Vec<String> {
    // BUG-452-10 fix : combat + health ajoutés pour que CHK-6 fonctionne par défaut.
    // Vague 5 Phase 5b Session A : forgia_health.json → forgia2_health.json
    // (rename canonique sensor Phase 5).
    // Vague 5 Phase 5b Session B (story-467) : perf + entities + memory ajoutés
    // (Tier 2 cross-mode producers).
    vec![
        "forgia_terrain_lod.json".to_string(),
        "forgia_chunks_snapshot.json".to_string(),
        "forgia_anim_layer.json".to_string(),
        "forgia_combat.json".to_string(),
        "forgia2_health.json".to_string(),
        "forgia2_perf.json".to_string(),
        "forgia2_entities.json".to_string(),
        "forgia2_memory.json".to_string(),
        // Story-469 V5 Session C
        "forgia2_lifecycle.json".to_string(),
        "forgia2_watchdog.json".to_string(),
        "forgia2_audio.json".to_string(),
        "forgia2_input.json".to_string(),
        "forgia2_sensor_health.json".to_string(),
    ]
}
fn default_stale_secs() -> f32 {
    30.0
}

impl Default for LivenessConfig {
    fn default() -> Self {
        Self {
            expected_sensors: default_expected_sensors(),
            stale_secs: default_stale_secs(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Lod2DesyncConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_lod2_tolerance")]
    pub tolerance: u64,
}

fn default_lod2_tolerance() -> u64 {
    4
}

impl Default for Lod2DesyncConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tolerance: default_lod2_tolerance(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct BiomeLuminanceConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_lum_floor")]
    pub lum_floor: f32,
    #[serde(default = "default_lum_ceiling")]
    pub lum_ceiling: f32,
}

// BUG-452-02 fix : floor LINÉAIRE (post-fix Rec709 on linear).
// Volcanic 0.22/0.15/0.12 sRGB → linear lum ≈ 0.024 (catch ancien bug).
// Volcanic 0.35/0.27/0.22 sRGB → linear lum ≈ 0.065 (current passe).
// Jungle 0.12/0.32/0.10 → linear lum ≈ 0.063 (current passe).
fn default_lum_floor() -> f32 {
    0.05
}
fn default_lum_ceiling() -> f32 {
    0.95
}

impl Default for BiomeLuminanceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            lum_floor: default_lum_floor(),
            lum_ceiling: default_lum_ceiling(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct LodAsymmetryConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Seuil d'écart (mètres) entre `lod0_y` et `lod2_y` pour déclencher Warn.
    #[serde(default = "default_lod_asym_delta")]
    pub max_delta_m: f32,
    /// Tolérance numérique pour considérer deux Y comme égaux (float compare).
    #[serde(default = "default_lod_asym_epsilon")]
    pub epsilon_m: f32,
}

fn default_lod_asym_delta() -> f32 {
    0.5
}
fn default_lod_asym_epsilon() -> f32 {
    0.001
}

impl Default for LodAsymmetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_delta_m: default_lod_asym_delta(),
            epsilon_m: default_lod_asym_epsilon(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct CriticalAssetsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_asset_paths")]
    pub paths: Vec<String>,
    #[serde(default = "default_asset_min_uptime")]
    pub asset_check_min_uptime_secs: f32,
}

fn default_asset_paths() -> Vec<String> {
    // Story-453 + fix 2026-05-28 : ex-paths "textures-v1/terrain/grass/*"
    // supprimés (legacy V1 git-ignored depuis purge 184e091). Aligné sur
    // `config/genomes/rpg_monitor.toml` qui pointe sur jolcham bark V2
    // (textures réellement câblées via story-486 foliage trunk override).
    vec![
        "textures/pbr/jolcham_oak_bark_01/jolcham_oak_bark_01_diff_2k.jpg".to_string(),
        "textures/pbr/jolcham_oak_bark_01/jolcham_oak_bark_01_nor_gl_2k.jpg".to_string(),
        "textures/pbr/jolcham_oak_bark_01/jolcham_oak_bark_01_arm_2k.jpg".to_string(),
    ]
}
fn default_asset_min_uptime() -> f32 {
    30.0
}

impl Default for CriticalAssetsConfig {
    fn default() -> Self {
        // Story-453 : réactivé par défaut (handles préchargés via
        // CriticalAssetHandles à OnEnter(GameMode::Rpg)).
        Self {
            enabled: true,
            paths: default_asset_paths(),
            asset_check_min_uptime_secs: default_asset_min_uptime(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct HealthConsistencyConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for HealthConsistencyConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

// ─────────────────── RogueliteHealthConfig (story-621, RGL-1/2) ───────────────────

/// Seuils des checks santé Roguelite (forgia2_health.json actif en `GameMode::Roguelite`).
#[derive(Clone, Debug, Deserialize)]
pub struct RogueliteHealthConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// RGL-1 : nombre de Mesh3d présents au-dessus duquel « 0 visible » = écran vide
    /// (aligné sur render_sensor : mesh_total>50 && visible==0).
    #[serde(default = "default_blank_mesh_floor")]
    pub blank_mesh_floor: u64,
    /// RGL-2 : secondes en run actif (in_run/boss), hors break, sans aucun bot vivant,
    /// avant de flaguer une vague figée.
    #[serde(default = "default_stuck_wave_secs")]
    pub stuck_wave_secs: f32,
    /// RGL-2 : au-delà de ce délai sans MAJ de forgia2_roguelite_state.json, le capteur
    /// est stale → on skip (diagnostic invalide, cf multi-terminal §5).
    #[serde(default = "default_state_stale_secs")]
    pub state_stale_secs: f32,
}

fn default_blank_mesh_floor() -> u64 {
    50
}
fn default_stuck_wave_secs() -> f32 {
    8.0
}
fn default_state_stale_secs() -> f32 {
    3.0
}

impl Default for RogueliteHealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            blank_mesh_floor: default_blank_mesh_floor(),
            stuck_wave_secs: default_stuck_wave_secs(),
            state_stale_secs: default_state_stale_secs(),
        }
    }
}

// ─────────────────────────── RpgMonitorConfig ───────────────────────────

const CONFIG_PATH: &str = "config/genomes/rpg_monitor.toml";

#[derive(Resource, Clone, Debug, Default, Deserialize)]
pub struct RpgMonitorConfig {
    #[serde(default)]
    pub meta: MetaConfig,
    #[serde(default)]
    pub global: GlobalConfig,
    #[serde(default)]
    pub liveness: LivenessConfig,
    #[serde(default)]
    pub lod2_desync: Lod2DesyncConfig,
    #[serde(default)]
    pub biome_luminance: BiomeLuminanceConfig,
    #[serde(default)]
    pub lod_asymmetry: LodAsymmetryConfig,
    #[serde(default)]
    pub critical_assets: CriticalAssetsConfig,
    #[serde(default)]
    pub health_consistency: HealthConsistencyConfig,
    #[serde(default)]
    pub roguelite_health: RogueliteHealthConfig,
}

impl RpgMonitorConfig {
    pub fn load_or_default() -> Self {
        match std::fs::read_to_string(CONFIG_PATH) {
            Ok(content) => match toml::from_str::<Self>(&content) {
                Ok(cfg) => {
                    info!("[rpg-monitor] Config loaded from {CONFIG_PATH}");
                    cfg
                }
                Err(e) => {
                    warn!("[rpg-monitor] TOML parse error in {CONFIG_PATH}: {e} — using defaults");
                    Self::default()
                }
            },
            Err(e) => {
                warn!("[rpg-monitor] Cannot read {CONFIG_PATH}: {e} — using defaults");
                Self::default()
            }
        }
    }

    pub fn reload(&mut self) {
        match std::fs::read_to_string(CONFIG_PATH) {
            Ok(content) => {
                match toml::from_str::<Self>(&content) {
                    Ok(new_cfg) => {
                        *self = new_cfg;
                        info!("[rpg-monitor] Config hot-reloaded from {CONFIG_PATH}");
                    }
                    Err(e) => {
                        warn!("[rpg-monitor] Hot-reload TOML parse error: {e} — keeping current config");
                    }
                }
            }
            Err(e) => {
                warn!("[rpg-monitor] Hot-reload read error: {e} — keeping current config");
            }
        }
    }
}

// ─────────────────────────── System hotkey reload ───────────────────────────

pub fn sys_reload_config_on_hotkey(
    keys: Res<ButtonInput<KeyCode>>,
    mut config: ResMut<RpgMonitorConfig>,
) {
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    if shift && keys.just_pressed(KeyCode::F12) {
        config.reload();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = RpgMonitorConfig::default();
        assert!(cfg.global.tick_interval_secs > 0.0);
        assert!(!cfg.liveness.expected_sensors.is_empty());
        assert!(cfg.biome_luminance.lum_floor < cfg.biome_luminance.lum_ceiling);
    }

    #[test]
    fn load_or_default_does_not_panic() {
        // Peut échouer à lire le TOML si absent du CWD — doit tomber sur default sans panic.
        let cfg = RpgMonitorConfig::load_or_default();
        assert!(cfg.global.tick_interval_secs > 0.0);
    }
}
