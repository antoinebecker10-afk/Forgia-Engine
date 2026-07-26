//! toon_config.rs — Story-544 close (2026-05-29).
//!
//! Hot-reloadable genome `roguelite_toon.toml` + auto-apply `ToonSettings`
//! et `OutlineSettings` sur les `Camera3d` du mode Roguelite + sensor
//! `forgia2_toon.json`.
//!
//! Pattern aligné `forgia-stage::graph::RunGraphConfig` (parse_toml + Default
//! fallback) + poll mtime 1Hz (pas d'AssetServer, le TOML n'est pas un asset
//! Bevy ici).
//!
//! Bible v1 (cartoon family-friendly) : 4 bands + outline Sobel noir 0.8.

use bevy::prelude::*;
use forgia_player::prelude::ViewmodelCamera;
use forgia_postprocess::outline::OutlineSettings;
use forgia_postprocess::toon::ToonSettings;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_toon.toml";
const SENSOR_PATH: &str = "forgia2_toon.json";
const POLL_PERIOD_SEC: f32 = 1.0;

// ─── TOML schema ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GeneToml {
    id: String,
    #[serde(default)]
    default: f32,
}

#[derive(Deserialize)]
struct ToonGenomeToml {
    #[serde(default)]
    genes: Vec<GeneToml>,
}

// ─── Resource ──────────────────────────────────────────────────────────────

/// Config consommée par `sys_apply_toon_settings` pour synchroniser les
/// composants `ToonSettings` / `OutlineSettings` sur Camera3d.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct RogueliteToonConfig {
    pub bands: f32,
    pub strength: f32,
    pub edge_dark: f32,
    pub outline_enabled: bool,
    pub outline_thickness: f32,
    pub outline_threshold: f32,
    pub outline_strength: f32,
}

impl Default for RogueliteToonConfig {
    fn default() -> Self {
        // 2026-05-29 — defaults adoucis : strength=1.0 + edge_dark=0.15 craquaient
        // le rendu Roguelite en noir total (éclairage scène faible, all-pixel
        // tombait dans band[0]). 0.6 + 0.0 = blend lisible + look cartoon.
        Self {
            bands: 4.0,
            strength: 0.6,
            edge_dark: 0.0,
            outline_enabled: true,
            outline_thickness: 1.0,
            outline_threshold: 0.15,
            outline_strength: 0.8,
        }
    }
}

impl RogueliteToonConfig {
    /// Pur — testable.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: ToonGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut c = Self::default();
        for gene in &parsed.genes {
            match gene.id.as_str() {
                "roguelite_toon_bands" => c.bands = gene.default.clamp(2.0, 16.0),
                "roguelite_toon_strength" => c.strength = gene.default.clamp(0.0, 1.0),
                "roguelite_toon_edge_dark" => c.edge_dark = gene.default.clamp(0.0, 1.0),
                "roguelite_outline_enabled" => c.outline_enabled = gene.default >= 0.5,
                "roguelite_outline_thickness" => c.outline_thickness = gene.default.clamp(0.5, 3.0),
                "roguelite_outline_threshold" => {
                    c.outline_threshold = gene.default.clamp(0.05, 0.5)
                }
                "roguelite_outline_strength" => c.outline_strength = gene.default.clamp(0.0, 1.0),
                _ => {}
            }
        }
        c
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }

    pub fn to_toon(self) -> ToonSettings {
        ToonSettings {
            bands: self.bands,
            strength: self.strength,
            edge_dark: self.edge_dark,
            _pad: 0.0,
        }
    }

    pub fn to_outline(self) -> OutlineSettings {
        OutlineSettings {
            thickness: self.outline_thickness,
            threshold: self.outline_threshold,
            strength: if self.outline_enabled {
                self.outline_strength
            } else {
                0.0
            },
            _pad: 0.0,
            edge_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

/// Track mtime du TOML + compteur reload, pour hot-reload + sensor.
#[derive(Resource, Default, Debug)]
pub struct ToonGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
    pub last_reload_secs: f32,
    pub attached_cameras: u32,
}

// ─── Systems ───────────────────────────────────────────────────────────────

/// Startup : charge le TOML + initialise watch mtime.
pub fn sys_init_toon_genome(mut commands: Commands) {
    let cfg = RogueliteToonConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(cfg);
    commands.insert_resource(ToonGenomeWatch {
        last_mtime: mtime,
        ..Default::default()
    });
    info!(
        "[roguelite-toon] genome loaded — bands={:.1} strength={:.2} edge_dark={:.2} outline_strength={:.2}",
        cfg.bands, cfg.strength, cfg.edge_dark, cfg.outline_strength
    );
}

/// Poll mtime 1Hz, re-parse + insert Resource si changé.
/// Mutate-only-on-real-change pour déclencher `DetectChanges` côté apply.
pub fn sys_hot_reload_toon_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<RogueliteToonConfig>,
    mut watch: ResMut<ToonGenomeWatch>,
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
    let new_cfg = RogueliteToonConfig::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_cfg != *cfg {
        *cfg = new_cfg;
        watch.reload_count = watch.reload_count.saturating_add(1);
        watch.last_reload_secs = time.elapsed_secs();
        info!(
            "[roguelite-toon] HOT-RELOADED — bands={:.1} strength={:.2} edge_dark={:.2} outline_strength={:.2}",
            new_cfg.bands, new_cfg.strength, new_cfg.edge_dark, new_cfg.outline_strength
        );
    }
}

/// Apply : sync `ToonSettings` + `OutlineSettings` sur toute Camera3d quand :
/// - config Changed (hot-reload)
/// - nouvelle Camera3d apparaît (Added<Camera3d>)
///
/// Idempotent — overwrite valeurs existantes.
pub fn sys_apply_toon_settings(
    mut commands: Commands,
    cfg: Res<RogueliteToonConfig>,
    mut watch: ResMut<ToonGenomeWatch>,
    // Story-618 : exclure la ViewmodelCamera — le toon sur une 2e caméra rejoue le
    // crash render-graph documenté (dual-pass même surface). Le viewmodel n'est
    // donc pas cel-shadé (caveat assumé v1).
    q_cam_all: Query<Entity, (With<Camera3d>, Without<ViewmodelCamera>)>,
    q_cam_new: Query<Entity, (Added<Camera3d>, Without<ViewmodelCamera>)>,
) {
    let cfg_changed = cfg.is_changed();
    let toon = cfg.to_toon();
    let outline = cfg.to_outline();

    if cfg_changed {
        let mut count = 0u32;
        for e in &q_cam_all {
            commands.entity(e).insert(toon);
            let _ = outline;
            count += 1;
        }
        watch.attached_cameras = count;
    } else {
        for e in &q_cam_new {
            commands.entity(e).insert(toon);
            let _ = outline;
            watch.attached_cameras = watch.attached_cameras.saturating_add(1);
        }
    }
}

/// OnEnter(GameMode::Roguelite) — force re-apply sur toutes Camera3d existantes.
/// Nécessaire car `sys_apply_toon_settings` ne tire que sur Changed/Added,
/// alors qu'à l'entrée dans Roguelite la cfg n'a pas changé et la caméra
/// préexiste depuis un autre mode.
pub fn sys_force_apply_toon_settings(
    mut commands: Commands,
    cfg: Res<RogueliteToonConfig>,
    mut watch: ResMut<ToonGenomeWatch>,
    q_cam: Query<Entity, (With<Camera3d>, Without<ViewmodelCamera>)>,
) {
    let toon = cfg.to_toon();
    let outline = cfg.to_outline();
    let mut count = 0u32;
    for e in &q_cam {
        commands.entity(e).insert(toon);
        let _ = outline;
        count += 1;
    }
    watch.attached_cameras = count;
    info!("[roguelite-toon] force-attached toon+outline to {count} Camera3d(s)");
}

/// OnExit(GameMode::Roguelite) — retire ToonSettings + OutlineSettings.
pub fn sys_detach_toon_from_cameras(
    mut commands: Commands,
    q_cam: Query<Entity, With<Camera3d>>,
    mut watch: ResMut<ToonGenomeWatch>,
) {
    for e in &q_cam {
        commands.entity(e).remove::<ToonSettings>();
    }
    watch.attached_cameras = 0;
}

/// État RÉEL de l'outline Sobel : le plugin est désactivé (crash wgpu
/// `SurfaceAcquireSemaphores`, cf lib.rs bloc commenté) — `to_outline()` est
/// calculé puis jeté (`let _ = outline;`) dans les 3 systèmes apply.
///
/// Story-593 (audit 2026-06-10 P1) : le sensor exportait `outline_enabled` (la
/// CONFIG) sans dire que rien n'est appliqué — une IA diagnostiquait sur un état
/// fictif. Cette const est LA source de vérité runtime ; la flipper à `true`
/// UNIQUEMENT quand le plugin outline est réellement re-branché (fix node_edges).
const OUTLINE_ATTACHED: bool = false;

/// Sensor `forgia2_toon.json` 1Hz — état runtime + dernière config appliquée.
pub fn sys_write_toon_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cfg: Option<Res<RogueliteToonConfig>>,
    watch: Option<Res<ToonGenomeWatch>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let Some(cfg) = cfg else {
        return;
    };
    let Some(watch) = watch else {
        return;
    };

    let outline_ghost = cfg.outline_enabled && !OUTLINE_ATTACHED;
    let (severity, next_step) =
        severity_for_toon(cfg.strength, watch.attached_cameras, outline_ghost);

    let json = format!(
        r#"{{"id":"toon","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"bands":{:.2},"strength":{:.2},"edge_dark":{:.2},"outline_enabled":{},"outline_attached":{},"outline_thickness":{:.2},"outline_threshold":{:.2},"outline_strength":{:.2},"attached_cameras":{},"reload_count":{},"last_reload_secs":{:.1}}}"#,
        time.elapsed_secs(),
        cfg.bands,
        cfg.strength,
        cfg.edge_dark,
        cfg.outline_enabled,
        OUTLINE_ATTACHED,
        cfg.outline_thickness,
        cfg.outline_threshold,
        cfg.outline_strength,
        watch.attached_cameras,
        watch.reload_count,
        watch.last_reload_secs,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[roguelite-toon] sensor write failed: {e}");
    }
}

/// Pur — testable. Severity `warn` si config "on" mais aucune caméra attachée
/// (= post-process invisible runtime malgré activation), `info` si l'outline est
/// demandé en config mais jamais appliqué (plugin désactivé, story-593).
pub fn severity_for_toon(
    strength: f32,
    attached_cameras: u32,
    outline_requested_not_attached: bool,
) -> (&'static str, &'static str) {
    if strength > 0.0 && attached_cameras == 0 {
        (
            "warn",
            "toon strength>0 mais 0 Camera3d attached — post-process invisible",
        )
    } else if outline_requested_not_attached {
        (
            "info",
            "outline_enabled en config mais plugin outline DESACTIVE (crash wgpu) — etat reel = outline_attached:false ; fix = node_edges apres ToonSettings (lib.rs)",
        )
    } else {
        ("ok", "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_default() {
        let cfg = RogueliteToonConfig::parse_toml("");
        assert_eq!(cfg, RogueliteToonConfig::default());
    }

    #[test]
    fn parse_overrides_known_genes() {
        let toml = r#"
[[genes]]
id = "roguelite_toon_bands"
default = 6.0
[[genes]]
id = "roguelite_outline_enabled"
default = 0.0
"#;
        let cfg = RogueliteToonConfig::parse_toml(toml);
        assert_eq!(cfg.bands, 6.0);
        assert!(!cfg.outline_enabled);
    }

    #[test]
    fn parse_clamps_out_of_range() {
        let toml = r#"
[[genes]]
id = "roguelite_toon_bands"
default = 999.0
[[genes]]
id = "roguelite_toon_strength"
default = -5.0
"#;
        let cfg = RogueliteToonConfig::parse_toml(toml);
        assert_eq!(cfg.bands, 16.0);
        assert_eq!(cfg.strength, 0.0);
    }

    #[test]
    fn to_outline_disabled_zeroes_strength() {
        let mut cfg = RogueliteToonConfig::default();
        cfg.outline_enabled = false;
        cfg.outline_strength = 0.9;
        assert_eq!(cfg.to_outline().strength, 0.0);
    }

    #[test]
    fn severity_warn_when_strength_but_no_camera() {
        assert_eq!(severity_for_toon(1.0, 0, false).0, "warn");
        assert_eq!(severity_for_toon(1.0, 1, false).0, "ok");
        assert_eq!(severity_for_toon(0.0, 0, false).0, "ok");
        // Story-593 : outline demandé en config mais plugin désactivé → info
        // (le sensor ne doit plus rapporter un outline fictif comme sain).
        assert_eq!(severity_for_toon(1.0, 1, true).0, "info");
        // La caméra manquante prime sur l'écart outline.
        assert_eq!(severity_for_toon(1.0, 0, true).0, "warn");
    }

    #[test]
    fn parse_real_genome_file() {
        let path = "../../assets/genomes/roguelite/roguelite_toon.toml";
        if let Ok(content) = fs::read_to_string(path) {
            let cfg = RogueliteToonConfig::parse_toml(&content);
            assert_eq!(cfg.bands, 4.0);
            assert_eq!(cfg.strength, 0.6);
            assert_eq!(cfg.edge_dark, 0.0);
            assert!(cfg.outline_enabled);
        }
    }
}
