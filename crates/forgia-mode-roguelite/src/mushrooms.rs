//! mushrooms.rs — story-625 : champignons lumineux des Cryptes de l'Enclume.
//!
//! Bible `crypts_of_anvil.md` : "Petits champignons lumineux bleus, jaunes, roses
//! (touche mignonne contre la noirceur)" + "mignonification". L'identité Cult of
//! the Lamb : sinistre digestible, glissé de touches lumineuses.
//!
//! Décor **émissif** data-driven, garde-fou-first (modèle `render_quality.rs`) :
//! - Genome `roguelite_mushrooms.toml` (hot-reload mtime 1Hz) = clusters + params.
//!   `enabled = false` → coupe la feature sans rebuild.
//! - Capteur `forgia2_mushrooms.json` = vérité runtime (counts + health).
//! - Spawn idempotent quand l'arène est prête (anchor `PlayerSpawn` présent, même
//!   signal que `decor.rs`), respawn live sur hot-reload, `DespawnOnExit(Roguelite)`.
//!
//! Perf : 1 `PointLight` PAR CLUSTER (pas par champignon) ; les champignons sont
//! des meshes **émissifs** (glow gratuit via bloom, 0 light). Spawn one-shot
//! (reconcile gardé) — aucun coût par-frame hors poll 1Hz + capteur 1Hz.

use bevy::prelude::*;
use forgia_anchor::{AnchorKind, AnchorPoint};
use forgia_core::prelude::*;
use serde::Deserialize;
use std::fs;
use std::time::SystemTime;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_mushrooms.toml";
const SENSOR_PATH: &str = "forgia2_mushrooms.json";
const POLL_PERIOD_SEC: f32 = 1.0;
/// Plafond lumen (convention atmosphère, cf `decor.rs`/`poi.rs`) — anti-cramage.
const MUSHROOM_LUMEN_CAP: f32 = 8_000.0;
// Dimensions de mesh = internes rendu (non exposées au créateur, cf no-hardcode
// exception "rendering internals"). Le champignon = chapeau (sphère aplatie) + pied.
const CAP_RADIUS: f32 = 0.5;
const STEM_RADIUS: f32 = 0.12;
const STEM_HEIGHT: f32 = 0.7;

// ─── TOML schema ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ClusterToml {
    pos: [f32; 3],
    color: [f32; 3],
    count: Option<u32>,
    radius: Option<f32>,
}

#[derive(Deserialize, Default)]
struct MushroomsGenomeToml {
    enabled: Option<bool>,
    mushroom_size: Option<f32>,
    light_intensity: Option<f32>,
    light_range: Option<f32>,
    emissive_boost: Option<f32>,
    #[serde(default)]
    clusters: Vec<ClusterToml>,
}

// ─── Config (Resource) ──────────────────────────────────────────────────────

/// Un amas de champignons : position monde, couleur (srgb), nombre, rayon de dispersion.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MushroomCluster {
    pub pos: [f32; 3],
    pub color: [f32; 3],
    pub count: u32,
    pub radius: f32,
}

/// Config champignons — pilotée par `roguelite_mushrooms.toml` (hot-reload).
#[derive(Resource, Clone, PartialEq, Debug)]
pub struct MushroomsConfig {
    pub enabled: bool,
    pub mushroom_size: f32,
    pub light_intensity: f32,
    pub light_range: f32,
    pub emissive_boost: f32,
    pub clusters: Vec<MushroomCluster>,
}

impl Default for MushroomsConfig {
    fn default() -> Self {
        // Clusters = CONTENU → vivent dans le TOML (no-hardcode). Sans fichier :
        // params sains + 0 cluster (feature inerte mais non-cassante, capteur le dit).
        Self {
            enabled: true,
            mushroom_size: 0.4,
            light_intensity: 1_800.0,
            light_range: 8.0,
            emissive_boost: 5.0,
            clusters: Vec::new(),
        }
    }
}

impl MushroomsConfig {
    /// Pur — testable. Merge le TOML sur les défauts.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: MushroomsGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let d = Self::default();
        Self {
            enabled: parsed.enabled.unwrap_or(d.enabled),
            mushroom_size: parsed.mushroom_size.unwrap_or(d.mushroom_size).clamp(0.1, 3.0),
            light_intensity: parsed
                .light_intensity
                .unwrap_or(d.light_intensity)
                .clamp(0.0, MUSHROOM_LUMEN_CAP),
            light_range: parsed.light_range.unwrap_or(d.light_range).clamp(0.5, 40.0),
            emissive_boost: parsed.emissive_boost.unwrap_or(d.emissive_boost).clamp(0.0, 30.0),
            clusters: parsed
                .clusters
                .into_iter()
                .map(|c| MushroomCluster {
                    pos: c.pos,
                    color: c.color,
                    count: c.count.unwrap_or(5).clamp(1, 40),
                    radius: c.radius.unwrap_or(2.5).clamp(0.2, 12.0),
                })
                .collect(),
        }
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(GENOME_PATH) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

/// Suivi mtime + état runtime (capteur).
#[derive(Resource, Default, Debug)]
pub struct MushroomsWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
    pub mushrooms_spawned: u32,
    pub lights_spawned: u32,
}

/// Handles partagés (créés une fois) : meshes chapeau/pied + matériau pied.
#[derive(Resource)]
pub struct MushroomAssets {
    pub cap_mesh: Handle<Mesh>,
    pub stem_mesh: Handle<Mesh>,
    pub stem_mat: Handle<StandardMaterial>,
}

/// Marqueur sur la racine d'un champignon OU d'une lumière de cluster (despawn).
#[derive(Component, Debug, Clone, Copy)]
pub struct MushroomProp;

// ─── Dispersion déterministe (pur, testable) ────────────────────────────────

/// Offset local (x, z) du i-ème champignon dans un cluster — spirale de Vogel
/// (angle d'or) : disque rempli uniformément, déterministe, sans RNG.
pub fn scatter_offset(i: u32, count: u32, radius: f32) -> Vec2 {
    const GOLDEN_ANGLE: f32 = 2.399_963_2; // rad
    let idx = i as f32 + 0.5;
    let r = radius * (idx / count.max(1) as f32).sqrt();
    let a = idx * GOLDEN_ANGLE;
    Vec2::new(r * a.cos(), r * a.sin())
}

// ─── Spawn ────────────────────────────────────────────────────────────────────

/// (Re)pose tous les clusters. Retourne (champignons, lumières) spawnés.
fn spawn_clusters(
    commands: &mut Commands,
    materials: &mut Assets<StandardMaterial>,
    cfg: &MushroomsConfig,
    assets: &MushroomAssets,
) -> (u32, u32) {
    let mut mushrooms = 0u32;
    let mut lights = 0u32;
    let size = cfg.mushroom_size;
    for cluster in &cfg.clusters {
        let col = cluster.color;
        let cap_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(col[0], col[1], col[2]),
            emissive: LinearRgba::rgb(
                col[0] * cfg.emissive_boost,
                col[1] * cfg.emissive_boost,
                col[2] * cfg.emissive_boost,
            ),
            perceptual_roughness: 0.5,
            ..default()
        });
        // 1 PointLight par cluster (perf : pas par champignon), légèrement surélevée.
        commands.spawn((
            MushroomProp,
            DespawnOnExit(GameMode::Roguelite),
            Name::new("MushroomLight"),
            PointLight {
                color: Color::srgb(col[0], col[1], col[2]),
                intensity: cfg.light_intensity.min(MUSHROOM_LUMEN_CAP),
                range: cfg.light_range,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(Vec3::from(cluster.pos) + Vec3::Y * (size * 2.0)),
        ));
        lights += 1;

        let count = cluster.count.max(1);
        for i in 0..count {
            let off = scatter_offset(i, count, cluster.radius);
            let base = Vec3::new(
                cluster.pos[0] + off.x,
                cluster.pos[1],
                cluster.pos[2] + off.y,
            );
            let yaw = i as f32 * 1.1;
            commands
                .spawn((
                    MushroomProp,
                    DespawnOnExit(GameMode::Roguelite),
                    Name::new("Mushroom"),
                    Transform::from_translation(base)
                        .with_rotation(Quat::from_rotation_y(yaw))
                        .with_scale(Vec3::splat(size)),
                    Visibility::default(),
                ))
                .with_children(|p| {
                    p.spawn((
                        Mesh3d(assets.stem_mesh.clone()),
                        MeshMaterial3d(assets.stem_mat.clone()),
                        Transform::from_xyz(0.0, STEM_HEIGHT * 0.5, 0.0),
                    ));
                    p.spawn((
                        Mesh3d(assets.cap_mesh.clone()),
                        MeshMaterial3d(cap_mat.clone()),
                        Transform::from_xyz(0.0, STEM_HEIGHT, 0.0)
                            .with_scale(Vec3::new(1.0, 0.6, 1.0)),
                    ));
                });
            mushrooms += 1;
        }
    }
    (mushrooms, lights)
}

// ─── Systems ────────────────────────────────────────────────────────────────

pub fn sys_init_mushroom_genome(mut commands: Commands) {
    let cfg = MushroomsConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    let clusters = cfg.clusters.len();
    commands.insert_resource(cfg);
    commands.insert_resource(MushroomsWatch {
        last_mtime: mtime,
        ..Default::default()
    });
    info!("[mushrooms] genome chargé — {clusters} clusters");
}

pub fn sys_init_mushroom_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cap_mesh = meshes.add(Sphere::new(CAP_RADIUS));
    let stem_mesh = meshes.add(Cylinder::new(STEM_RADIUS, STEM_HEIGHT));
    // Pied pâle, faiblement émissif (lisible dans la pénombre sans voler la vedette).
    let stem_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.83, 0.78),
        emissive: LinearRgba::rgb(0.15, 0.14, 0.12),
        perceptual_roughness: 0.8,
        ..default()
    });
    commands.insert_resource(MushroomAssets {
        cap_mesh,
        stem_mesh,
        stem_mat,
    });
}

pub fn sys_hot_reload_mushroom_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<MushroomsConfig>,
    mut watch: ResMut<MushroomsWatch>,
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
    let new_cfg = MushroomsConfig::parse_toml(&content);
    if new_cfg != *cfg {
        *cfg = new_cfg; // ResMut muté → is_changed() → reconcile respawn.
        watch.reload_count = watch.reload_count.saturating_add(1);
        info!("[mushrooms] HOT-RELOADED — {} clusters", cfg.clusters.len());
    }
}

/// Spawn idempotent : pose les clusters quand l'arène est prête (anchor PlayerSpawn),
/// respawn si la config a changé (hot-reload), despawn si désactivé.
pub fn sys_reconcile_mushrooms(
    mut commands: Commands,
    cfg: Option<Res<MushroomsConfig>>,
    assets: Option<Res<MushroomAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut watch: ResMut<MushroomsWatch>,
    q_anchors: Query<&AnchorPoint>,
    q_existing: Query<Entity, With<MushroomProp>>,
) {
    let (Some(cfg), Some(assets)) = (cfg, assets) else {
        return;
    };
    // Arène prête = même signal que decor.rs (PlayerSpawn posé par le stage).
    if !q_anchors.iter().any(|a| a.kind == AnchorKind::PlayerSpawn) {
        return;
    }
    let already = q_existing.iter().next().is_some();

    if !cfg.enabled {
        if already {
            for e in &q_existing {
                commands.entity(e).despawn();
            }
            watch.mushrooms_spawned = 0;
            watch.lights_spawned = 0;
        }
        return;
    }
    // (Re)spawn quand : rien posé encore, OU config changée (hot-reload live).
    if already && !cfg.is_changed() {
        return;
    }
    if already {
        for e in &q_existing {
            commands.entity(e).despawn();
        }
    }
    let (m, l) = spawn_clusters(&mut commands, &mut materials, &cfg, &assets);
    watch.mushrooms_spawned = m;
    watch.lights_spawned = l;
    info!("[mushrooms] posés — {m} champignons, {l} lumières");
}

/// Pur — severity/next_step du capteur.
pub fn severity_for_mushrooms(
    enabled: bool,
    arena_ready: bool,
    clusters: usize,
    spawned: u32,
) -> (&'static str, &'static str) {
    if !enabled {
        return ("info", "Champignons coupés (roguelite_mushrooms.toml enabled=false).");
    }
    if clusters == 0 {
        return (
            "info",
            "0 cluster défini — ajouter des [[clusters]] dans roguelite_mushrooms.toml.",
        );
    }
    if arena_ready && spawned == 0 {
        return (
            "warn",
            "Clusters définis mais 0 champignon posé en arène — vérifier le spawn (anchor PlayerSpawn ?).",
        );
    }
    ("ok", "Champignons posés.")
}

pub fn sys_write_mushroom_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    cfg: Option<Res<MushroomsConfig>>,
    watch: Option<Res<MushroomsWatch>>,
    q_anchors: Query<&AnchorPoint>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;
    let (Some(cfg), Some(watch)) = (cfg, watch) else {
        return;
    };
    let arena_ready = q_anchors.iter().any(|a| a.kind == AnchorKind::PlayerSpawn);
    let (severity, next_step) = severity_for_mushrooms(
        cfg.enabled,
        arena_ready,
        cfg.clusters.len(),
        watch.mushrooms_spawned,
    );
    let json = format!(
        r#"{{"id":"mushrooms","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"enabled":{},"clusters":{},"mushrooms_spawned":{},"lights_spawned":{},"emissive_boost":{:.2},"reload_count":{}}}"#,
        time.elapsed_secs(),
        cfg.enabled,
        cfg.clusters.len(),
        watch.mushrooms_spawned,
        watch.lights_spawned,
        cfg.emissive_boost,
        watch.reload_count,
    );
    if let Err(e) = fs::write(SENSOR_PATH, &json) {
        warn!("[mushrooms] sensor write failed: {e}");
    }
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

pub struct ForgiaRogueliteMushroomsPlugin;

impl Plugin for ForgiaRogueliteMushroomsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (sys_init_mushroom_genome, sys_init_mushroom_assets));
        app.add_systems(
            Update,
            (sys_hot_reload_mushroom_genome, sys_reconcile_mushrooms)
                .chain()
                .in_set(GameSet::Effects)
                .run_if(in_state(GameMode::Roguelite)),
        );
        app.add_systems(
            Update,
            sys_write_mushroom_sensor
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
        assert_eq!(MushroomsConfig::parse_toml(""), MushroomsConfig::default());
    }

    #[test]
    fn default_enabled_with_no_clusters() {
        let d = MushroomsConfig::default();
        assert!(d.enabled);
        assert!(d.clusters.is_empty());
    }

    #[test]
    fn parse_overrides_and_clusters() {
        let toml = r#"
enabled = false
emissive_boost = 8.0
[[clusters]]
pos = [1.0, 0.0, 2.0]
color = [1.0, 0.4, 0.7]
count = 6
radius = 3.0
"#;
        let c = MushroomsConfig::parse_toml(toml);
        assert!(!c.enabled);
        assert!((c.emissive_boost - 8.0).abs() < 1e-6);
        assert_eq!(c.clusters.len(), 1);
        assert_eq!(c.clusters[0].count, 6);
        assert_eq!(c.clusters[0].color, [1.0, 0.4, 0.7]);
    }

    #[test]
    fn parse_clamps_and_cluster_defaults() {
        let toml = r#"
light_intensity = 99999.0
[[clusters]]
pos = [0.0, 0.0, 0.0]
color = [0.0, 0.5, 1.0]
"#;
        let c = MushroomsConfig::parse_toml(toml);
        assert!(c.light_intensity <= MUSHROOM_LUMEN_CAP);
        // count/radius absents → défauts appliqués
        assert_eq!(c.clusters[0].count, 5);
        assert!((c.clusters[0].radius - 2.5).abs() < 1e-6);
    }

    #[test]
    fn scatter_within_radius_and_deterministic() {
        let r = 3.0;
        for i in 0..10 {
            let o = scatter_offset(i, 10, r);
            assert!(o.length() <= r + 1e-3, "offset {i} hors rayon");
        }
        // déterministe : même entrée → même sortie
        assert_eq!(scatter_offset(3, 10, r), scatter_offset(3, 10, r));
    }

    #[test]
    fn severity_paths() {
        assert_eq!(severity_for_mushrooms(false, true, 5, 0).0, "info");
        assert_eq!(severity_for_mushrooms(true, true, 0, 0).0, "info");
        assert_eq!(severity_for_mushrooms(true, true, 5, 0).0, "warn");
        assert_eq!(severity_for_mushrooms(true, true, 5, 30).0, "ok");
        // pas encore en arène → pas de warn (transitoire)
        assert_eq!(severity_for_mushrooms(true, false, 5, 0).0, "ok");
    }
}
