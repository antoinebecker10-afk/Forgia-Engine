//! color_grading.rs — Color grading filmique par `GameMode` (story-602, 2026-06-17).
//!
//! Chaque mode reçoit un `ColorGrading` (Bevy, lu AVANT la passe tonemapping —
//! pas une passe séparée comme Bloom, donc sûr) qui définit son *mood* :
//! température (chaud/froid), exposition, saturation, contraste midtones.
//!
//! - **Roguelite** : chaud, punchy (forge volcanique cartoon)
//! - **CyberCity** : froid/néon, contrasté
//! - **RPG** : naturel, doux
//! - Menu/Fps/None : neutre (reset → aucune fuite inter-mode, cf audit mode-coupling)
//!
//! La COURBE (Tonemapping) reste le réglage user global (ESC). Le grading est
//! orthogonal (composant distinct) → zéro conflit avec `apply_tonemapping_to_cameras`.
//!
//! Genome `assets/genomes/color_grading.toml` hot-reload (poll mtime 1Hz) →
//! tune les looks live sans rebuild. Pattern aligné `toon_config`.

use bevy::prelude::*;
use bevy::render::view::ColorGrading;
use forgia_core::prelude::*;
use serde::Deserialize;
use std::fs;
use std::time::SystemTime;

const GENOME_PATH: &str = "assets/genomes/color_grading.toml";
const SENSOR_PATH: &str = "forgia2_color_grading.json";
const POLL_PERIOD_SEC: f32 = 1.0;

// ─── Modèle ──────────────────────────────────────────────────────────────────

/// Deltas de grading d'un mode (0 = neutre).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ModeGrade {
    pub exposure: f32,
    pub temperature: f32,
    pub tint: f32,
    pub post_saturation: f32,
    pub contrast: f32,
    pub saturation: f32,
}

impl ModeGrade {
    pub const NEUTRAL: Self = Self {
        exposure: 0.0,
        temperature: 0.0,
        tint: 0.0,
        post_saturation: 0.0,
        contrast: 0.0,
        saturation: 0.0,
    };

    /// Construit le `ColorGrading` Bevy : part du neutre (default) + applique les
    /// deltas (additif pour exposure/temp/tint ; ajouté au 1.0 multiplicatif pour
    /// post_saturation/contrast/saturation). Clampé pour éviter un tune cassé.
    pub fn to_color_grading(self) -> ColorGrading {
        let mut cg = ColorGrading::default();
        cg.global.exposure = self.exposure.clamp(-3.0, 3.0);
        cg.global.temperature = self.temperature.clamp(-1.0, 1.0);
        cg.global.tint = self.tint.clamp(-1.0, 1.0);
        cg.global.post_saturation =
            (cg.global.post_saturation + self.post_saturation).clamp(0.0, 4.0);
        cg.midtones.contrast = (cg.midtones.contrast + self.contrast).clamp(0.0, 4.0);
        cg.midtones.saturation = (cg.midtones.saturation + self.saturation).clamp(0.0, 4.0);
        cg
    }
}

/// Config par mode (Resource), source = genome + Default = looks proposés 2026-06-17.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct ColorGradingConfig {
    pub roguelite: ModeGrade,
    pub cybercity: ModeGrade,
    pub rpg: ModeGrade,
}

impl Default for ColorGradingConfig {
    fn default() -> Self {
        Self {
            roguelite: ModeGrade {
                exposure: 0.0,
                temperature: 0.20,
                tint: 0.0,
                post_saturation: 0.10,
                contrast: 0.08,
                saturation: 0.05,
            },
            cybercity: ModeGrade {
                exposure: 0.0,
                temperature: -0.15,
                tint: 0.05,
                post_saturation: 0.15,
                contrast: 0.12,
                saturation: 0.08,
            },
            rpg: ModeGrade {
                exposure: 0.0,
                temperature: 0.05,
                tint: 0.0,
                post_saturation: 0.0,
                contrast: 0.0,
                saturation: 0.0,
            },
        }
    }
}

impl ColorGradingConfig {
    /// Grade du mode courant. None = mode neutre (Menu/Fps/None → reset).
    pub fn grade_for(&self, mode: &GameMode) -> Option<ModeGrade> {
        match mode {
            GameMode::Roguelite => Some(self.roguelite),
            GameMode::CyberCity => Some(self.cybercity),
            GameMode::Rpg => Some(self.rpg),
            _ => None,
        }
    }

    /// Pur — testable. Merge le TOML (champs présents) sur les défauts.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: GradingToml = toml::from_str(content).unwrap_or_default();
        let base = Self::default();
        Self {
            roguelite: merge(base.roguelite, &parsed.roguelite),
            cybercity: merge(base.cybercity, &parsed.cybercity),
            rpg: merge(base.rpg, &parsed.rpg),
        }
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

#[derive(Deserialize, Default)]
struct ModeGradeToml {
    exposure: Option<f32>,
    temperature: Option<f32>,
    tint: Option<f32>,
    post_saturation: Option<f32>,
    contrast: Option<f32>,
    saturation: Option<f32>,
}

#[derive(Deserialize, Default)]
struct GradingToml {
    roguelite: Option<ModeGradeToml>,
    cybercity: Option<ModeGradeToml>,
    rpg: Option<ModeGradeToml>,
}

fn merge(base: ModeGrade, t: &Option<ModeGradeToml>) -> ModeGrade {
    match t {
        None => base,
        Some(t) => ModeGrade {
            exposure: t.exposure.unwrap_or(base.exposure),
            temperature: t.temperature.unwrap_or(base.temperature),
            tint: t.tint.unwrap_or(base.tint),
            post_saturation: t.post_saturation.unwrap_or(base.post_saturation),
            contrast: t.contrast.unwrap_or(base.contrast),
            saturation: t.saturation.unwrap_or(base.saturation),
        },
    }
}

/// Track mtime + reload + état pour le sensor.
#[derive(Resource, Default, Debug)]
pub struct ColorGradingWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
    pub attached_cameras: u32,
    pub last_mode: String,
}

// ─── Systems ───────────────────────────────────────────────────────────────

fn sys_init(mut commands: Commands) {
    let cfg = ColorGradingConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(cfg);
    commands.insert_resource(ColorGradingWatch {
        last_mtime: mtime,
        ..Default::default()
    });
    info!("[color-grading] genome chargé (roguelite/cybercity/rpg)");
}

/// Poll mtime 1Hz → re-parse si changé (mutate-only-on-change → DetectChanges).
fn sys_hot_reload(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<ColorGradingConfig>,
    mut watch: ResMut<ColorGradingWatch>,
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
    let new_cfg = ColorGradingConfig::parse_toml(&content);
    if new_cfg != *cfg {
        *cfg = new_cfg;
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!("[color-grading] HOT-RELOADED");
    }
}

/// Applique le `ColorGrading` du mode courant sur toutes les `Camera3d` quand :
/// - le mode change (Local last_mode), - le genome a changé (cfg Changed),
/// - une nouvelle Camera3d apparaît (Added). Mode neutre → reset (default).
fn sys_apply(
    mut commands: Commands,
    cfg: Res<ColorGradingConfig>,
    state: Res<State<GameMode>>,
    mut watch: ResMut<ColorGradingWatch>,
    mut last_mode: Local<Option<GameMode>>,
    q_cam_all: Query<Entity, With<Camera3d>>,
    q_cam_new: Query<Entity, Added<Camera3d>>,
) {
    let mode = state.get().clone();
    let cg = cfg.grade_for(&mode).unwrap_or(ModeGrade::NEUTRAL).to_color_grading();

    let mode_changed = last_mode.as_ref() != Some(&mode);
    if mode_changed || cfg.is_changed() {
        let mut count = 0u32;
        for e in &q_cam_all {
            commands.entity(e).insert(cg.clone());
            count += 1;
        }
        watch.attached_cameras = count;
        watch.last_mode = format!("{mode:?}");
        *last_mode = Some(mode);
    } else {
        for e in &q_cam_new {
            commands.entity(e).insert(cg.clone());
            watch.attached_cameras = watch.attached_cameras.saturating_add(1);
        }
    }
}

/// Sensor `forgia2_color_grading.json` 1Hz (observability-required).
fn sys_write_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cfg: Res<ColorGradingConfig>,
    state: Res<State<GameMode>>,
    watch: Res<ColorGradingWatch>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let mode = state.get().clone();
    let g = cfg.grade_for(&mode).unwrap_or(ModeGrade::NEUTRAL);
    let (severity, next_step) = if watch.attached_cameras == 0 {
        (
            "info",
            "aucune Camera3d (menu ?) — grading appliqué dès qu'une caméra 3D existe",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"color_grading","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"mode":"{mode:?}","exposure":{:.3},"temperature":{:.3},"tint":{:.3},"post_saturation":{:.3},"contrast":{:.3},"saturation":{:.3},"attached_cameras":{},"reload_count":{}}}"#,
        time.elapsed_secs(),
        g.exposure,
        g.temperature,
        g.tint,
        g.post_saturation,
        g.contrast,
        g.saturation,
        watch.attached_cameras,
        watch.reload_count,
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[color-grading] sensor write failed: {e}");
    }
}

pub struct ColorGradingPlugin;

impl Plugin for ColorGradingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, sys_init)
            .add_systems(Update, (sys_hot_reload, sys_apply, sys_write_sensor));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_is_default() {
        assert_eq!(ColorGradingConfig::parse_toml(""), ColorGradingConfig::default());
    }

    #[test]
    fn parse_overrides_present_fields_only() {
        let toml = "[cybercity]\ntemperature = -0.5\n";
        let cfg = ColorGradingConfig::parse_toml(toml);
        assert_eq!(cfg.cybercity.temperature, -0.5);
        // champ non spécifié garde le défaut
        assert_eq!(cfg.cybercity.post_saturation, 0.15);
        // autre mode intact
        assert_eq!(cfg.roguelite.temperature, 0.20);
    }

    #[test]
    fn neutral_grade_is_identity_ish() {
        let cg = ModeGrade::NEUTRAL.to_color_grading();
        assert_eq!(cg.global.exposure, 0.0);
        assert_eq!(cg.global.temperature, 0.0);
    }

    #[test]
    fn grade_for_menu_is_none() {
        let cfg = ColorGradingConfig::default();
        assert!(cfg.grade_for(&GameMode::Roguelite).is_some());
        assert!(cfg.grade_for(&GameMode::None).is_none());
    }

    #[test]
    fn delta_clamps() {
        let g = ModeGrade {
            post_saturation: 99.0,
            ..ModeGrade::NEUTRAL
        };
        assert!(g.to_color_grading().global.post_saturation <= 4.0);
    }
}
