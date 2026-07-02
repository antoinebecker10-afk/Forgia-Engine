//! render_quality.rs — story-625 Tier 4 : SSAO + contrôle/observabilité du rendu.
//!
//! Garde-fou-first (leçon du crash outline 2026-06-27) : la config + le capteur
//! `forgia2_render_fx.json` sont la source de vérité runtime ; chaque effet est
//! **toggleable en data** (`roguelite_render.toml`, hot-reload mtime 1Hz) → si un
//! effet casse en jeu, on le coupe sans rebuild.
//!
//! Contrairement à l'outline (2 passes FullscreenMaterial → crash wgpu), le SSAO
//! est un effet compute natif Bevy (`ScreenSpaceAmbientOcclusion`, plugin déjà
//! dans `PbrPlugin`) qui ne touche pas au render-graph fullscreen → pas de dual-pass.
//!
//! Pièces :
//! - `RogueliteRenderConfig` : toggles SSAO + atmosphère (brume/ambiante).
//! - SSAO attaché/détaché sur la `Camera3d` principale (hors ViewmodelCamera).
//! - Atmosphère (DistanceFog + AmbientLight) lue depuis cette config (cf
//!   `atmosphere.rs`) → densité/brillance/toggle pilotables en data.
//! - Capteur 1Hz + health check (SSAO demandé mais 0 caméra, etc).

use bevy::pbr::{ScreenSpaceAmbientOcclusion, ScreenSpaceAmbientOcclusionQualityLevel};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::view::{Hdr, Msaa};
use forgia_core::prelude::*;
use forgia_player::prelude::ViewmodelCamera;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_render.toml";
const SENSOR_PATH: &str = "forgia2_render_fx.json";
const POLL_PERIOD_SEC: f32 = 1.0;

// ─── TOML schema (genes plats, pattern toon_config) ─────────────────────────

#[derive(Deserialize)]
struct GeneToml {
    id: String,
    #[serde(default)]
    default: f32,
}

#[derive(Deserialize)]
struct RenderGenomeToml {
    #[serde(default)]
    genes: Vec<GeneToml>,
}

// ─── Resource ───────────────────────────────────────────────────────────────

/// Config rendu Roguelite — pilotée par `roguelite_render.toml` (hot-reload).
/// Consommée par `sys_apply_ssao` (SSAO) + `atmosphere.rs` (brume/ambiante).
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct RogueliteRenderConfig {
    pub ssao_enabled: bool,
    /// 0 Low · 1 Medium · 2 High · 3 Ultra.
    pub ssao_quality: u8,
    pub ssao_thickness: f32,
    /// Story-647 : glow des émissifs HDR (muzzle/tracers/sparks/hit-flash).
    pub bloom_enabled: bool,
    pub bloom_intensity: f32,
    pub fog_enabled: bool,
    pub fog_density: f32,
    pub ambient_brightness: f32,
}

impl Default for RogueliteRenderConfig {
    fn default() -> Self {
        // Miroir de l'état pré-Tier-4 : atmosphere.rs hardcodait density=0.008,
        // ambient=300. SSAO ON par défaut (effet sûr, désactivable si besoin).
        Self {
            ssao_enabled: true,
            ssao_quality: 2, // High (défaut Bevy)
            ssao_thickness: 0.25,
            bloom_enabled: true,
            bloom_intensity: 0.15, // préset NATURAL — subtil, pas de glow runaway
            fog_enabled: true,
            fog_density: 0.008,
            ambient_brightness: 300.0,
        }
    }
}

impl RogueliteRenderConfig {
    /// Pur — testable.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: RenderGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut c = Self::default();
        for gene in &parsed.genes {
            match gene.id.as_str() {
                "roguelite_ssao_enabled" => c.ssao_enabled = gene.default >= 0.5,
                "roguelite_ssao_quality" => c.ssao_quality = gene.default.clamp(0.0, 3.0) as u8,
                "roguelite_ssao_thickness" => c.ssao_thickness = gene.default.clamp(0.01, 2.0),
                "roguelite_bloom_enabled" => c.bloom_enabled = gene.default >= 0.5,
                "roguelite_bloom_intensity" => {
                    c.bloom_intensity = gene.default.clamp(0.0, 0.5);
                }
                "roguelite_fog_enabled" => c.fog_enabled = gene.default >= 0.5,
                "roguelite_fog_density" => c.fog_density = gene.default.clamp(0.0, 0.1),
                "roguelite_ambient_brightness" => {
                    c.ambient_brightness = gene.default.clamp(0.0, 2000.0);
                }
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

    pub fn ssao_quality_level(self) -> ScreenSpaceAmbientOcclusionQualityLevel {
        match self.ssao_quality {
            0 => ScreenSpaceAmbientOcclusionQualityLevel::Low,
            1 => ScreenSpaceAmbientOcclusionQualityLevel::Medium,
            3 => ScreenSpaceAmbientOcclusionQualityLevel::Ultra,
            _ => ScreenSpaceAmbientOcclusionQualityLevel::High,
        }
    }

    pub fn to_ssao(self) -> ScreenSpaceAmbientOcclusion {
        ScreenSpaceAmbientOcclusion {
            quality_level: self.ssao_quality_level(),
            constant_object_thickness: self.ssao_thickness,
        }
    }

    /// Story-647 : préset NATURAL (energy-conserving) + intensité genome.
    pub fn to_bloom(self) -> Bloom {
        Bloom {
            intensity: self.bloom_intensity,
            ..Bloom::NATURAL
        }
    }
}

/// Suivi mtime + état runtime (capteur).
#[derive(Resource, Default, Debug)]
pub struct RenderGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
    pub ssao_attached: u32,
    pub bloom_attached: u32,
}

// ─── Systems : genome ───────────────────────────────────────────────────────

pub fn sys_init_render_genome(mut commands: Commands) {
    let cfg = RogueliteRenderConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(cfg);
    commands.insert_resource(RenderGenomeWatch {
        last_mtime: mtime,
        ..Default::default()
    });
    info!(
        "[roguelite-render] genome loaded — ssao({} q{} th{:.2}) fog({} d{:.3}) ambient({:.0})",
        cfg.ssao_enabled, cfg.ssao_quality, cfg.ssao_thickness, cfg.fog_enabled, cfg.fog_density, cfg.ambient_brightness
    );
}

pub fn sys_hot_reload_render_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<RogueliteRenderConfig>,
    mut watch: ResMut<RenderGenomeWatch>,
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
    let new_cfg = RogueliteRenderConfig::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_cfg != *cfg {
        *cfg = new_cfg;
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!(
            "[roguelite-render] HOT-RELOADED — ssao({} q{}) fog({} d{:.3})",
            new_cfg.ssao_enabled, new_cfg.ssao_quality, new_cfg.fog_enabled, new_cfg.fog_density
        );
    }
}

// ─── Systems : SSAO apply/detach ────────────────────────────────────────────

fn apply_ssao_to(commands: &mut Commands, e: Entity, cfg: &RogueliteRenderConfig) {
    if cfg.ssao_enabled {
        commands.entity(e).insert(cfg.to_ssao());
        // SSAO et MSAA sont MUTUELLEMENT EXCLUSIFS dans Bevy (sinon ERROR b0004 à
        // chaque frame + SSAO inopérant). On force `Msaa::Off` sur la caméra qui
        // porte le SSAO. `apply_msaa_to_cameras` (forgia-ui-lib) ignore les cams
        // avec SSAO et restaure le MSAA utilisateur dès que le SSAO est retiré.
        commands.entity(e).insert(Msaa::Off);
    } else {
        commands.entity(e).remove::<ScreenSpaceAmbientOcclusion>();
    }
}

/// Attache/détache `ScreenSpaceAmbientOcclusion` sur la Camera3d principale (hors
/// ViewmodelCamera) quand la config change OU qu'une caméra apparaît. Le require
/// Bevy ajoute DepthPrepass+NormalPrepass automatiquement.
pub fn sys_apply_ssao(
    mut commands: Commands,
    cfg: Res<RogueliteRenderConfig>,
    mut watch: ResMut<RenderGenomeWatch>,
    q_all: Query<Entity, (With<Camera3d>, Without<ViewmodelCamera>)>,
    q_new: Query<Entity, (Added<Camera3d>, Without<ViewmodelCamera>)>,
) {
    if cfg.is_changed() {
        let mut n = 0u32;
        for e in &q_all {
            apply_ssao_to(&mut commands, e, &cfg);
            if cfg.ssao_enabled {
                n += 1;
            }
        }
        watch.ssao_attached = n;
    } else {
        for e in &q_new {
            apply_ssao_to(&mut commands, e, &cfg);
            if cfg.ssao_enabled {
                watch.ssao_attached = watch.ssao_attached.saturating_add(1);
            }
        }
    }
}

/// OnEnter(Roguelite) — force re-apply (la caméra préexiste depuis un autre mode,
/// la config n'a pas "changé" donc sys_apply_ssao ne tirerait pas).
pub fn sys_force_apply_ssao(
    mut commands: Commands,
    cfg: Res<RogueliteRenderConfig>,
    mut watch: ResMut<RenderGenomeWatch>,
    q_cam: Query<Entity, (With<Camera3d>, Without<ViewmodelCamera>)>,
) {
    let mut n = 0u32;
    for e in &q_cam {
        apply_ssao_to(&mut commands, e, &cfg);
        if cfg.ssao_enabled {
            n += 1;
        }
    }
    watch.ssao_attached = n;
    info!("[roguelite-render] force-applied SSAO (enabled={}) to {n} cam", cfg.ssao_enabled);
}

/// OnExit(Roguelite) — retire le SSAO (les autres modes retrouvent leur rendu).
/// DepthPrepass/NormalPrepass (ajoutés par le require) restent — inoffensifs.
pub fn sys_detach_ssao(
    mut commands: Commands,
    q_cam: Query<Entity, With<ScreenSpaceAmbientOcclusion>>,
    mut watch: ResMut<RenderGenomeWatch>,
) {
    for e in &q_cam {
        commands.entity(e).remove::<ScreenSpaceAmbientOcclusion>();
    }
    watch.ssao_attached = 0;
}

// ─── Systems : Bloom apply/detach (story-647) ───────────────────────────────

fn apply_bloom_to(commands: &mut Commands, e: Entity, cfg: &RogueliteRenderConfig) {
    if cfg.bloom_enabled {
        commands.entity(e).insert(cfg.to_bloom());
    } else {
        commands.entity(e).remove::<Bloom>();
    }
}

/// Attache/détache `Bloom` sur les Camera3d **déjà Hdr** (hors ViewmodelCamera).
/// Le `Hdr` lui-même est posé À LA CRÉATION de la FpsCamera (forgia-player,
/// leçon cyber_city 2026-06-16 : Hdr post-hoc = passe principale cassée) —
/// attacher/retirer `Bloom` à chaud sur une cam déjà HDR est sûr, lui.
///
/// COUPLAGE (audit QA story-647) : `spawn_player` n'est PAS gaté par GameMode →
/// la FpsCamera porte Hdr aussi en RPG. Sans effet aujourd'hui car forgia-rpg
/// (`character.rs`, exclusivité caméra) force `is_active=false` sur la FpsCamera
/// en RPG. Si cette exclusivité change, prévoir un sys_apply_bloom côté RPG.
pub fn sys_apply_bloom(
    mut commands: Commands,
    cfg: Res<RogueliteRenderConfig>,
    mut watch: ResMut<RenderGenomeWatch>,
    q_all: Query<Entity, (With<Camera3d>, With<Hdr>, Without<ViewmodelCamera>)>,
    q_new: Query<Entity, (Added<Camera3d>, With<Hdr>, Without<ViewmodelCamera>)>,
) {
    if cfg.is_changed() {
        let mut n = 0u32;
        for e in &q_all {
            apply_bloom_to(&mut commands, e, &cfg);
            if cfg.bloom_enabled {
                n += 1;
            }
        }
        watch.bloom_attached = n;
    } else {
        for e in &q_new {
            apply_bloom_to(&mut commands, e, &cfg);
            if cfg.bloom_enabled {
                watch.bloom_attached = watch.bloom_attached.saturating_add(1);
            }
        }
    }
}

/// OnEnter(Roguelite) — force re-apply (même raison que le SSAO : la caméra
/// préexiste depuis un autre mode, la config n'a pas "changé").
pub fn sys_force_apply_bloom(
    mut commands: Commands,
    cfg: Res<RogueliteRenderConfig>,
    mut watch: ResMut<RenderGenomeWatch>,
    q_cam: Query<Entity, (With<Camera3d>, With<Hdr>, Without<ViewmodelCamera>)>,
) {
    let mut n = 0u32;
    for e in &q_cam {
        apply_bloom_to(&mut commands, e, &cfg);
        if cfg.bloom_enabled {
            n += 1;
        }
    }
    watch.bloom_attached = n;
    info!(
        "[roguelite-render] force-applied Bloom (enabled={} intensity={:.2}) to {n} cam",
        cfg.bloom_enabled, cfg.bloom_intensity
    );
}

/// OnExit(Roguelite) — retire le Bloom (les autres modes retrouvent leur rendu).
/// Le `Hdr` de la FpsCamera reste : inoffensif seul, et le retirer serait
/// justement la mutation post-hoc interdite.
pub fn sys_detach_bloom(
    mut commands: Commands,
    q_cam: Query<Entity, With<Bloom>>,
    mut watch: ResMut<RenderGenomeWatch>,
) {
    for e in &q_cam {
        commands.entity(e).remove::<Bloom>();
    }
    watch.bloom_attached = 0;
}

// ─── Sensor ─────────────────────────────────────────────────────────────────

/// Pur — severity/next_step du capteur rendu.
///
/// `ssao_msaa_conflict` : une cam porte SSAO **et** un MSAA actif (≠ Off). C'est
/// l'unique combinaison qui spamme `ERROR b0004` chaque frame + rend le SSAO
/// inopérant (incompatibilité Bevy). Health-check prioritaire : sans lui, le
/// conflit n'est visible que dans les erreurs wgpu brutes, pas dans un capteur.
pub fn severity_for_render(
    ssao_enabled: bool,
    ssao_attached: u32,
    bloom_enabled: bool,
    bloom_attached: u32,
    camera_present: bool,
    ssao_msaa_conflict: bool,
) -> (&'static str, &'static str) {
    if ssao_msaa_conflict {
        (
            "warn",
            "Conflit SSAO+MSAA sur la meme camera (ERROR b0004 chaque frame, SSAO inoperant). apply_ssao_to doit forcer Msaa::Off ; apply_msaa_to_cameras doit exclure les cams SSAO.",
        )
    } else if ssao_enabled && camera_present && ssao_attached == 0 {
        (
            "warn",
            "SSAO active en config mais 0 camera attachee — effet invisible. Verifier la Camera3d Roguelite.",
        )
    } else if bloom_enabled && camera_present && bloom_attached == 0 {
        (
            "warn",
            "Bloom actif en config mais 0 camera Hdr attachee — glow invisible. Verifier que la FpsCamera a Hdr A LA CREATION (story-647, jamais post-hoc).",
        )
    } else {
        (
            "ok",
            "Rendu OK. Si crash GPU/perf : couper roguelite_ssao_enabled dans roguelite_render.toml (hot-reload).",
        )
    }
}

pub fn sys_write_render_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cfg: Option<Res<RogueliteRenderConfig>>,
    watch: Option<Res<RenderGenomeWatch>>,
    q_cam: Query<(), (With<Camera3d>, Without<ViewmodelCamera>)>,
    q_fog: Query<(), With<DistanceFog>>,
    q_ssao_msaa: Query<&Msaa, With<ScreenSpaceAmbientOcclusion>>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let (Some(cfg), Some(watch)) = (cfg, watch) else {
        return;
    };
    let camera_count = q_cam.iter().count() as u32;
    let fog_attached = q_fog.iter().count() as u32;
    // Conflit = une cam SSAO avec un MSAA ≠ Off (b0004 + SSAO inopérant).
    let ssao_msaa_conflict = q_ssao_msaa.iter().any(|m| !matches!(m, Msaa::Off));
    let (severity, next_step) = severity_for_render(
        cfg.ssao_enabled,
        watch.ssao_attached,
        cfg.bloom_enabled,
        watch.bloom_attached,
        camera_count > 0,
        ssao_msaa_conflict,
    );

    let json = format!(
        r#"{{"id":"render_fx","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"camera_count":{},"ssao_enabled":{},"ssao_attached":{},"ssao_msaa_conflict":{},"ssao_quality":{},"ssao_thickness":{:.2},"bloom_enabled":{},"bloom_attached":{},"bloom_intensity":{:.2},"fog_enabled":{},"fog_density":{:.3},"fog_attached":{},"ambient_brightness":{:.0},"reload_count":{}}}"#,
        time.elapsed_secs(),
        camera_count,
        cfg.ssao_enabled,
        watch.ssao_attached,
        ssao_msaa_conflict,
        cfg.ssao_quality,
        cfg.ssao_thickness,
        cfg.bloom_enabled,
        watch.bloom_attached,
        cfg.bloom_intensity,
        cfg.fog_enabled,
        cfg.fog_density,
        fog_attached,
        cfg.ambient_brightness,
        watch.reload_count,
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[roguelite-render] sensor write failed: {e}");
    }
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

pub struct ForgiaRogueliteRenderPlugin;

impl Plugin for ForgiaRogueliteRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, sys_init_render_genome);
        app.add_systems(
            OnEnter(GameMode::Roguelite),
            (sys_force_apply_ssao, sys_force_apply_bloom),
        );
        app.add_systems(
            OnExit(GameMode::Roguelite),
            (sys_detach_ssao, sys_detach_bloom),
        );
        app.add_systems(
            Update,
            (sys_hot_reload_render_genome, sys_apply_ssao, sys_apply_bloom)
                .chain()
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_write_render_sensor
                .in_set(GameSet::Sensors)
                .run_if(in_state(GameMode::Roguelite)),
        );
    }
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_returns_default() {
        assert_eq!(
            RogueliteRenderConfig::parse_toml(""),
            RogueliteRenderConfig::default()
        );
    }

    #[test]
    fn parse_overrides_and_clamps() {
        let toml = r#"
[[genes]]
id = "roguelite_ssao_enabled"
default = 0.0
[[genes]]
id = "roguelite_ssao_quality"
default = 9.0
[[genes]]
id = "roguelite_fog_density"
default = 0.05
"#;
        let c = RogueliteRenderConfig::parse_toml(toml);
        assert!(!c.ssao_enabled);
        assert_eq!(c.ssao_quality, 3); // clamp 9 -> 3
        assert!((c.fog_density - 0.05).abs() < 1e-6);
    }

    #[test]
    fn quality_level_mapping() {
        let mut c = RogueliteRenderConfig::default();
        c.ssao_quality = 0;
        assert_eq!(c.ssao_quality_level(), ScreenSpaceAmbientOcclusionQualityLevel::Low);
        c.ssao_quality = 3;
        assert_eq!(c.ssao_quality_level(), ScreenSpaceAmbientOcclusionQualityLevel::Ultra);
        c.ssao_quality = 2;
        assert_eq!(c.ssao_quality_level(), ScreenSpaceAmbientOcclusionQualityLevel::High);
    }

    #[test]
    fn severity_warn_when_enabled_but_unattached() {
        assert_eq!(severity_for_render(true, 0, false, 0, true, false).0, "warn");
        assert_eq!(severity_for_render(true, 1, false, 0, true, false).0, "ok");
        assert_eq!(severity_for_render(false, 0, false, 0, true, false).0, "ok");
        // pas de caméra encore → pas de warn (transitoire boot)
        assert_eq!(severity_for_render(true, 0, false, 0, false, false).0, "ok");
    }

    #[test]
    fn severity_warn_on_ssao_msaa_conflict() {
        // SSAO + MSAA sur la même cam = b0004 + SSAO inopérant → warn prioritaire.
        let (sev, next) = severity_for_render(true, 1, false, 0, true, true);
        assert_eq!(sev, "warn");
        assert!(next.contains("b0004"));
        // Conflit prioritaire même si le reste paraît "ok" (SSAO attaché, cam présente).
        assert_eq!(severity_for_render(false, 0, false, 0, false, true).0, "warn");
    }

    #[test]
    fn severity_warn_when_bloom_unattached() {
        // Story-647 : bloom demandé mais aucune cam Hdr → glow invisible = warn.
        let (sev, next) = severity_for_render(false, 0, true, 0, true, false);
        assert_eq!(sev, "warn");
        assert!(next.contains("Hdr"));
        assert_eq!(severity_for_render(false, 0, true, 1, true, false).0, "ok");
        // pas de caméra encore → transitoire boot, pas de warn.
        assert_eq!(severity_for_render(false, 0, true, 0, false, false).0, "ok");
    }

    #[test]
    fn parse_bloom_genes_and_clamps() {
        let toml = r#"
[[genes]]
id = "roguelite_bloom_enabled"
default = 0.0
[[genes]]
id = "roguelite_bloom_intensity"
default = 3.0
"#;
        let c = RogueliteRenderConfig::parse_toml(toml);
        assert!(!c.bloom_enabled);
        assert!((c.bloom_intensity - 0.5).abs() < 1e-6, "clamp 3.0 -> 0.5");
        // défauts : bloom ON, intensité NATURAL.
        let d = RogueliteRenderConfig::default();
        assert!(d.bloom_enabled);
        assert!((d.bloom_intensity - 0.15).abs() < 1e-6);
    }

    #[test]
    fn default_mirrors_pre_tier4_atmosphere() {
        let c = RogueliteRenderConfig::default();
        assert!((c.fog_density - 0.008).abs() < 1e-6);
        assert!((c.ambient_brightness - 300.0).abs() < 1e-3);
    }
}
