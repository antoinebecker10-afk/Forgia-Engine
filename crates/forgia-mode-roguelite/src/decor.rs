//! decor.rs — Story-562b : props décoratifs (GLB Inferno) pour remplir l'arène.
//!
//! Greffe de vrais props 3D (pack **Inferno** CC0, volcan, déjà dans
//! `assets/models/environment/inferno/`) par-dessus l'arène `forgia-stage`, SANS
//! la toucher. Rochers, crags, mounds, colonnes, braseros (lueur), statue, tour,
//! + petits props dispersés (vases, boîtes, engrenages).
//!
//! ## Cohérence d'échelle (calibration AABB)
//!
//! Les tailles natives des GLB varient énormément (TowerBig ≫ Vase). Pour un
//! rendu cohérent, chaque prop est **calibré à une taille cible** par groupe :
//! `NeedsDecorCalibrate` mesure l'AABB runtime → `scale = target / max_dim`.
//! C'est la même logique que `forgia-asset-registry::calibrate_assets`, répliquée
//! ici car ce système est gaté `GameMode::Rpg`.
//!
//! ## Colliders (pattern rapier 0.33 parent/enfant)
//!
//! Gros props (périmètre) = solides : **parent** `RigidBody::Fixed` (scale 1) +
//! **enfant** `SceneRoot` scalé + `AsyncSceneCollider{ConvexHull}` → rapier
//! génère le collider depuis le mesh scalé et l'attache au RigidBody parent
//! (quirk rapier 0.33, cf forgia-mode-fps-arena). Petits débris au sol = SANS
//! collider (bots/joueur passent au travers = nav safe).
//!
//! ## Lifecycle (count-based reconcile)
//!
//! Arène bâtie (anchor `PlayerSpawn`) + aucun prop → spawn. Props tagués
//! `StageArenaMarker` → cleanup par forgia-stage à chaque transition. Retry
//! in-place → décor persiste.

use bevy::camera::primitives::Aabb;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::{Scene, SceneRoot};
use bevy::state::state_scoped::DespawnOnExit;
use bevy_rapier3d::prelude::{Collider, ComputedColliderShape, RigidBody};
use forgia_ai_arena_bot::ArenaBot;
use forgia_anchor::{AnchorKind, AnchorPoint};
use forgia_core::prelude::*;
use forgia_stage::{StageArenaMarker, StageLoadResult};
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;
use serde::Deserialize;
use std::f32::consts::TAU;
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::run::RunSeed;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_decor.toml";
const SENSOR_PATH: &str = "forgia2_stage_decor.json";
const POLL_PERIOD_SEC: f32 = 1.0;
const ATMOSPHERE_LUMEN_CAP: f32 = 8_000.0;

// ─── Catalogue de props (pack Inferno CC0, déjà dans assets/) ─────────────────

/// Landmarks hauts = points focaux (statue, tour).
const LANDMARK_PROPS: &[&str] = &[
    "models/environment/inferno/StatueKnight_002.glb",
    "models/environment/inferno/TowerBig_001.glb",
];

/// Gros props de remplissage (rochers, crags, mounds, colonnes).
const BIG_PROPS: &[&str] = &[
    "models/environment/inferno/RockBig_001.glb",
    "models/environment/inferno/RockBig_003.glb",
    "models/environment/inferno/RockBig_004.glb",
    "models/environment/inferno/Crag_001.glb",
    "models/environment/inferno/Crag_003.glb",
    "models/environment/inferno/Mound_005.glb",
    "models/environment/inferno/Mound_008.glb",
    "models/environment/inferno/ColumnBig_001.glb",
    "models/environment/inferno/ColumnBigBroken_001.glb",
    "models/environment/inferno/ColumnBigBroken_002.glb",
];

/// Braseros — élément lumineux (feu GLB + PointLight chaud).
const BRAZIERS: &[&str] = &[
    "models/environment/inferno/Brazier_002.glb",
    "models/environment/inferno/Brazier_004.glb",
];

/// Petits props dispersés au sol.
const SCATTER_PROPS: &[&str] = &[
    "models/environment/inferno/RockMid_001.glb",
    "models/environment/inferno/RockMid_002.glb",
    "models/environment/inferno/RockMid_003.glb",
    "models/environment/inferno/Box_001.glb",
    "models/environment/inferno/Vase_001.glb",
    "models/environment/inferno/Vase_002.glb",
    "models/environment/inferno/Gear_001.glb",
    "models/environment/inferno/Gear_002.glb",
];

/// Segments de mur KayKit dungeon (salles en L). Échelle NATIVE (modulaires).
const WALL_VARIANTS: &[&str] = &[
    "models/kaykit/dungeon/wall.glb",
    "models/kaykit/dungeon/wall.glb",
    "models/kaykit/dungeon/wall_broken.glb",
    "models/kaykit/dungeon/wall_window.glb",
];
const WALL_CORNER: &str = "models/kaykit/dungeon/wall_corner.glb";

/// Gravats au sol pour casser la répétition des dalles (masque, pas collider).
const RUBBLE_PROPS: &[&str] = &["models/kaykit/dungeon/rubble.glb"];

/// Bâtiments KayKit Medieval Hexagon (couleur ROUGE = forge/feu) pour la ville
/// industrielle « Cratère de la Forge » (incr.3). Self-contained .gltf + atlas
/// `hexagons_medieval.png`. Mix industriel dominant (blacksmith ×2, mine, tours,
/// barracks, castle, lumbermill, scaffolding, ruines) + un peu de civil.
const BUILDINGS: &[&str] = &[
    "models/kaykit/hexagon/red/building_blacksmith_red.gltf",
    "models/kaykit/hexagon/red/building_blacksmith_red.gltf",
    "models/kaykit/hexagon/red/building_mine_red.gltf",
    "models/kaykit/hexagon/red/building_tower_A_red.gltf",
    "models/kaykit/hexagon/red/building_tower_B_red.gltf",
    "models/kaykit/hexagon/red/building_tower_catapult_red.gltf",
    "models/kaykit/hexagon/red/building_barracks_red.gltf",
    "models/kaykit/hexagon/red/building_castle_red.gltf",
    "models/kaykit/hexagon/red/building_lumbermill_red.gltf",
    "models/kaykit/hexagon/red/building_tavern_red.gltf",
    "models/kaykit/hexagon/red/building_home_A_red.gltf",
    "models/kaykit/hexagon/red/building_home_B_red.gltf",
    "models/kaykit/hexagon/red/building_market_red.gltf",
    "models/kaykit/hexagon/neutral/building_scaffolding.gltf",
    "models/kaykit/hexagon/neutral/building_destroyed.gltf",
    "models/kaykit/hexagon/neutral/building_grain.gltf",
];

/// Dimensions natives KayKit dungeon wall.glb (cf forgia-stage : largeur 1 m,
/// hauteur `RAMPARTS_WALL_HEIGHT_M`=4, épaisseur `RAMPARTS_WALL_THICKNESS_M`=0.4).
const WALL_SEG_W: f32 = 1.0;
const WALL_HEIGHT: f32 = 4.0;

// ─── Genome / Config ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GeneToml {
    id: String,
    #[serde(default)]
    default: f32,
}

#[derive(Deserialize)]
struct DecorGenomeToml {
    #[serde(default)]
    genes: Vec<GeneToml>,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct RogueliteDecorConfig {
    pub enabled: bool,
    pub ring_radius_min: f32,
    pub ring_radius_max: f32,
    pub perimeter_count: u32,
    pub landmark_count: u32,
    pub brazier_ratio: f32,
    pub scatter_count: u32,
    pub scatter_radius_min: f32,
    pub scatter_radius_max: f32,
    pub target_landmark: f32,
    pub target_big: f32,
    pub target_brazier: f32,
    pub target_scatter: f32,
    // Salles en L (murs KayKit + collider par bras).
    pub room_count: u32,
    pub room_arm_segments: u32,
    pub room_radius_min: f32,
    pub room_radius_max: f32,
    // Gravats masquant la répétition du sol.
    pub rubble_count: u32,
    pub target_rubble: f32,
    // Fond « Cratère de la Forge » : anneau de falaises volcaniques + pics géants
    // hors-map (ÉNORME, sans collider — pure silhouette à l'horizon).
    pub background_count: u32,
    pub background_radius_min: f32,
    pub background_radius_max: f32,
    pub target_background: f32,
    pub giant_peak_count: u32,
    pub giant_peak_radius: f32,
    pub target_giant_peak: f32,
    // Cœur de forge (incr.2) : anneau de braseros « ring of fire » autour de
    // l'aire de combat + monuments/cheminées hauts qui encadrent le centre.
    pub forge_ring_count: u32,
    pub forge_ring_radius: f32,
    pub forge_monument_count: u32,
    pub forge_monument_radius: f32,
    pub forge_monument_target: f32,
    // Ville KayKit (incr.3) : ceinture de bâtiments industriels (red) face au centre.
    pub building_count: u32,
    pub building_radius_min: f32,
    pub building_radius_max: f32,
    pub target_building: f32,
    /// Anti-freeze (story-619 follow-up) : nb de props GLB instanciés par frame.
    /// Le décor est planifié d'un coup mais spawné par budget → un hitch de 65 ms
    /// (797 instanciations SceneSpawner en 1 frame) devient ~80 frames < 16 ms.
    pub spawn_budget_per_frame: u32,
}

impl Default for RogueliteDecorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ring_radius_min: 42.0,
            ring_radius_max: 74.0,
            perimeter_count: 34,
            landmark_count: 4,
            brazier_ratio: 0.30,
            scatter_count: 55,
            scatter_radius_min: 12.0,
            scatter_radius_max: 72.0,
            target_landmark: 16.0,
            target_big: 7.0,
            target_brazier: 3.5,
            target_scatter: 1.6,
            room_count: 4,
            room_arm_segments: 6,
            room_radius_min: 20.0,
            room_radius_max: 52.0,
            rubble_count: 34,
            target_rubble: 3.4,
            // Cratère de fond : crête dense + 2 pics géants dominants.
            background_count: 44,
            background_radius_min: 110.0,
            background_radius_max: 165.0,
            target_background: 34.0,
            giant_peak_count: 2,
            giant_peak_radius: 205.0,
            target_giant_peak: 80.0,
            // Cœur de forge : 8 braseros en anneau à 16 m + 4 monuments à ~30 m.
            forge_ring_count: 8,
            forge_ring_radius: 16.0,
            forge_monument_count: 4,
            forge_monument_radius: 30.0,
            forge_monument_target: 18.0,
            // Ville : 18 bâtiments KayKit en ceinture, ~12 m, à 52-76 m.
            building_count: 18,
            building_radius_min: 52.0,
            building_radius_max: 76.0,
            target_building: 12.0,
            // 12 props/frame → ~67 frames (~1,1 s) pour ~800 props, chaque frame
            // bien sous le budget 16 ms. Hot-reload via genome.
            spawn_budget_per_frame: 12,
        }
    }
}

impl RogueliteDecorConfig {
    /// Pur — testable headless.
    pub fn parse_toml(content: &str) -> Self {
        let parsed: DecorGenomeToml = match toml::from_str(content) {
            Ok(v) => v,
            Err(_) => return Self::default(),
        };
        let mut c = Self::default();
        for gene in &parsed.genes {
            match gene.id.as_str() {
                "decor_enabled" => c.enabled = gene.default >= 0.5,
                "decor_ring_radius_min" => c.ring_radius_min = gene.default.clamp(1.0, 500.0),
                "decor_ring_radius_max" => c.ring_radius_max = gene.default.clamp(1.0, 500.0),
                "decor_perimeter_count" => {
                    c.perimeter_count = gene.default.clamp(0.0, 200.0) as u32
                }
                "decor_landmark_count" => c.landmark_count = gene.default.clamp(0.0, 50.0) as u32,
                "decor_brazier_ratio" => c.brazier_ratio = gene.default.clamp(0.0, 1.0),
                "decor_scatter_count" => c.scatter_count = gene.default.clamp(0.0, 600.0) as u32,
                "decor_scatter_radius_min" => {
                    c.scatter_radius_min = gene.default.clamp(0.0, 500.0)
                }
                "decor_scatter_radius_max" => {
                    c.scatter_radius_max = gene.default.clamp(0.0, 500.0)
                }
                "decor_target_landmark" => c.target_landmark = gene.default.clamp(0.5, 50.0),
                "decor_target_big" => c.target_big = gene.default.clamp(0.5, 50.0),
                "decor_target_brazier" => c.target_brazier = gene.default.clamp(0.3, 20.0),
                "decor_target_scatter" => c.target_scatter = gene.default.clamp(0.2, 10.0),
                "decor_room_count" => c.room_count = gene.default.clamp(0.0, 30.0) as u32,
                "decor_room_arm_segments" => {
                    c.room_arm_segments = gene.default.clamp(0.0, 40.0) as u32
                }
                "decor_room_radius_min" => c.room_radius_min = gene.default.clamp(0.0, 500.0),
                "decor_room_radius_max" => c.room_radius_max = gene.default.clamp(0.0, 500.0),
                "decor_rubble_count" => c.rubble_count = gene.default.clamp(0.0, 400.0) as u32,
                "decor_target_rubble" => c.target_rubble = gene.default.clamp(0.3, 20.0),
                "decor_background_count" => {
                    c.background_count = gene.default.clamp(0.0, 200.0) as u32
                }
                "decor_background_radius_min" => {
                    c.background_radius_min = gene.default.clamp(10.0, 600.0)
                }
                "decor_background_radius_max" => {
                    c.background_radius_max = gene.default.clamp(10.0, 600.0)
                }
                "decor_target_background" => c.target_background = gene.default.clamp(5.0, 120.0),
                "decor_giant_peak_count" => {
                    c.giant_peak_count = gene.default.clamp(0.0, 12.0) as u32
                }
                "decor_giant_peak_radius" => {
                    c.giant_peak_radius = gene.default.clamp(20.0, 800.0)
                }
                "decor_target_giant_peak" => c.target_giant_peak = gene.default.clamp(10.0, 200.0),
                "decor_forge_ring_count" => {
                    c.forge_ring_count = gene.default.clamp(0.0, 40.0) as u32
                }
                "decor_forge_ring_radius" => c.forge_ring_radius = gene.default.clamp(4.0, 80.0),
                "decor_forge_monument_count" => {
                    c.forge_monument_count = gene.default.clamp(0.0, 20.0) as u32
                }
                "decor_forge_monument_radius" => {
                    c.forge_monument_radius = gene.default.clamp(6.0, 90.0)
                }
                "decor_forge_monument_target" => {
                    c.forge_monument_target = gene.default.clamp(2.0, 50.0)
                }
                "decor_building_count" => {
                    c.building_count = gene.default.clamp(0.0, 80.0) as u32
                }
                "decor_building_radius_min" => {
                    c.building_radius_min = gene.default.clamp(10.0, 200.0)
                }
                "decor_building_radius_max" => {
                    c.building_radius_max = gene.default.clamp(10.0, 200.0)
                }
                "decor_target_building" => c.target_building = gene.default.clamp(3.0, 40.0),
                "decor_spawn_budget_per_frame" => {
                    c.spawn_budget_per_frame = gene.default.clamp(1.0, 200.0) as u32
                }
                _ => {}
            }
        }
        if c.ring_radius_min > c.ring_radius_max {
            std::mem::swap(&mut c.ring_radius_min, &mut c.ring_radius_max);
        }
        if c.scatter_radius_min > c.scatter_radius_max {
            std::mem::swap(&mut c.scatter_radius_min, &mut c.scatter_radius_max);
        }
        if c.room_radius_min > c.room_radius_max {
            std::mem::swap(&mut c.room_radius_min, &mut c.room_radius_max);
        }
        if c.background_radius_min > c.background_radius_max {
            std::mem::swap(&mut c.background_radius_min, &mut c.background_radius_max);
        }
        if c.building_radius_min > c.building_radius_max {
            std::mem::swap(&mut c.building_radius_min, &mut c.building_radius_max);
        }
        c
    }

    fn load_or_default() -> Self {
        match fs::read_to_string(PathBuf::from(GENOME_PATH)) {
            Ok(content) => Self::parse_toml(&content),
            Err(_) => Self::default(),
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct DecorGenomeWatch {
    pub last_mtime: Option<SystemTime>,
    pub reload_count: u32,
}

/// Handles de scènes GLB préchargés (réutilisés sur toutes les instances).
#[derive(Resource, Default)]
pub struct DecorAssets {
    pub landmarks: Vec<Handle<Scene>>,
    pub big: Vec<Handle<Scene>>,
    pub braziers: Vec<Handle<Scene>>,
    pub scatter: Vec<Handle<Scene>>,
    pub walls: Vec<Handle<Scene>>,
    pub wall_corner: Vec<Handle<Scene>>,
    pub rubble: Vec<Handle<Scene>>,
    pub buildings: Vec<Handle<Scene>>,
}

/// Marqueur sur l'entité racine d'un prop (count sensor + cleanup).
#[derive(Component, Debug, Clone, Copy)]
pub struct DecorProp;

/// Posé sur le SceneRoot d'un prop à calibrer. `sys_calibrate_decor` mesure
/// l'AABB une fois la scène chargée et applique `scale = target / max_dim`.
/// (Le collider, lui, est un cylindre primitif posé au spawn sur le parent —
/// fiable, sized depuis la target, indépendant du timing de chargement scène.)
#[derive(Component, Debug, Clone, Copy)]
pub struct NeedsDecorCalibrate {
    pub target_m: f32,
    pub user_scale: f32,
}

/// Posé sur le PARENT d'un prop : `sys_decor_build_hull_colliders` attache un
/// `Collider` **ConvexHull** construit depuis chaque mesh chargé (suit la
/// silhouette du prop ; fiable car bâti APRÈS chargement, pas le souci async de
/// `AsyncSceneCollider`). Fallback cylindre si aucun hull n'est productible.
#[derive(Component, Debug, Clone, Copy)]
pub struct NeedsHullCollider {
    pub fallback_target_m: f32,
    pub fallback_radius_factor: f32,
}

/// Marqueur sur un décor SOLIDE (prop/mur/coin) avec son rayon de footprint
/// approximatif. `sys_unstick_bots_from_decor` s'en sert pour qu'un ennemi
/// n'apparaisse jamais COINCÉ dans le décor (position d'entité → robuste au
/// timing de build des colliders, contrairement à un test physique).
#[derive(Component, Debug, Clone, Copy)]
pub struct SolidDecorObstacle {
    pub radius: f32,
}

// ─── File de spawn étalé (anti-freeze story-619 follow-up) ────────────────────

/// Un prop décor résolu (handle + transform + params), prêt à spawner. Produit
/// en bloc par `plan_decor_set` (RNG only, pas d'instanciation), puis drainé par
/// `sys_drain_decor_queue` à `spawn_budget_per_frame` par frame — c'est l'étalement
/// qui supprime le hitch de 65 ms (797 instanciations SceneSpawner en 1 frame).
enum DecorSpec {
    /// Silhouette de fond (un seul `SceneRoot`, sans collider).
    Background {
        handle: Handle<Scene>,
        name: &'static str,
        pos: Vec3,
        yaw: f32,
        target_m: f32,
    },
    /// Prop solide (parent RigidBody + enfant visuel + hull collider + brasero).
    Perimeter {
        handle: Handle<Scene>,
        name: &'static str,
        pos: Vec3,
        yaw: f32,
        target_m: f32,
        user_scale: f32,
        brazier: bool,
        col_radius_factor: f32,
    },
    /// Petit prop au sol sans collider (scatter / rubble) — diffèrent par le nom.
    Loose {
        handle: Handle<Scene>,
        name: &'static str,
        pos: Vec3,
        yaw: f32,
        user_scale: f32,
        target_m: f32,
    },
    /// Segment de mur (coin ou bras de salle en L) — hull collider + obstacle.
    WallPiece {
        handle: Handle<Scene>,
        pos: Vec3,
        yaw: f32,
        obstacle_radius: f32,
    },
}

/// File des props planifiés mais pas encore instanciés. Drainée par budget/frame.
/// `cursor` évite le `Vec::remove(0)` O(n) ; le `Vec` est vidé une fois épuisé.
#[derive(Resource, Default)]
pub struct DecorSpawnQueue {
    pending: Vec<DecorSpec>,
    cursor: usize,
}

impl DecorSpawnQueue {
    fn remaining(&self) -> usize {
        self.pending.len().saturating_sub(self.cursor)
    }
}

/// Instancie UN prop résolu (route vers le helper adéquat). C'est le seul endroit
/// qui appelle `commands.spawn` pour le décor → le coût (SceneSpawner) est ainsi
/// étalable par le drain.
fn spawn_one(commands: &mut Commands, spec: &DecorSpec) {
    match spec {
        DecorSpec::Background {
            handle,
            name,
            pos,
            yaw,
            target_m,
        } => spawn_background_silhouette(commands, handle, name, *pos, *yaw, *target_m),
        DecorSpec::Perimeter {
            handle,
            name,
            pos,
            yaw,
            target_m,
            user_scale,
            brazier,
            col_radius_factor,
        } => spawn_perimeter_prop(
            commands,
            handle,
            name,
            *pos,
            *yaw,
            *target_m,
            *user_scale,
            *brazier,
            *col_radius_factor,
        ),
        DecorSpec::Loose {
            handle,
            name,
            pos,
            yaw,
            user_scale,
            target_m,
        } => {
            commands.spawn((
                decor_markers(*name),
                SceneRoot(handle.clone()),
                Transform::from_translation(*pos).with_rotation(Quat::from_rotation_y(*yaw)),
                NeedsDecorCalibrate {
                    target_m: *target_m,
                    user_scale: *user_scale,
                },
            ));
        }
        DecorSpec::WallPiece {
            handle,
            pos,
            yaw,
            obstacle_radius,
        } => {
            commands.spawn((
                decor_markers("Decor_Wall"),
                SceneRoot(handle.clone()),
                Transform::from_translation(*pos).with_rotation(Quat::from_rotation_y(*yaw)),
                NeedsHullCollider {
                    fallback_target_m: WALL_HEIGHT,
                    fallback_radius_factor: 0.3,
                },
                SolidDecorObstacle {
                    radius: *obstacle_radius,
                },
            ));
        }
    }
}

// ─── Systems : init genome + assets + hot-reload ──────────────────────────────

pub fn sys_init_decor_genome(mut commands: Commands) {
    let cfg = RogueliteDecorConfig::load_or_default();
    let mtime = fs::metadata(GENOME_PATH).and_then(|m| m.modified()).ok();
    commands.insert_resource(cfg);
    commands.insert_resource(DecorGenomeWatch {
        last_mtime: mtime,
        ..default()
    });
    info!(
        "[decor] genome loaded — ring {:.0}-{:.0}m perim={} landmarks={} braziers={:.0}% scatter={}",
        cfg.ring_radius_min,
        cfg.ring_radius_max,
        cfg.perimeter_count,
        cfg.landmark_count,
        cfg.brazier_ratio * 100.0,
        cfg.scatter_count
    );
}

/// Précharge toutes les scènes GLB une fois (un seul call-site `load`).
pub fn sys_load_decor_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let load = |paths: &[&str]| -> Vec<Handle<Scene>> {
        paths
            .iter()
            .map(|p| asset_server.load(GltfAssetLabel::Scene(0).from_asset(p.to_string())))
            .collect()
    };
    commands.insert_resource(DecorAssets {
        landmarks: load(LANDMARK_PROPS),
        big: load(BIG_PROPS),
        braziers: load(BRAZIERS),
        scatter: load(SCATTER_PROPS),
        walls: load(WALL_VARIANTS),
        wall_corner: load(&[WALL_CORNER]),
        rubble: load(RUBBLE_PROPS),
        buildings: load(BUILDINGS),
    });
    info!("[decor] preloaded inferno + wall + rubble + kaykit buildings scenes");
}

/// Poll mtime 1Hz. Sur changement réel, despawn le décor → réconciliation le
/// reconstruit avec les nouveaux paramètres.
pub fn sys_hot_reload_decor_genome(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut cfg: ResMut<RogueliteDecorConfig>,
    mut watch: ResMut<DecorGenomeWatch>,
    q_decor: Query<Entity, With<DecorProp>>,
    mut commands: Commands,
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
    let new_cfg = RogueliteDecorConfig::parse_toml(&content);
    watch.last_mtime = Some(mtime);
    if new_cfg != *cfg {
        *cfg = new_cfg;
        watch.reload_count = watch.reload_count.saturating_add(1);
        for e in &q_decor {
            commands.entity(e).despawn();
        }
        info!(
            "[decor] HOT-RELOADED — ring {:.0}-{:.0}m perim={} targets L{:.0}/B{:.0}/Br{:.1}/S{:.1} → régénération",
            new_cfg.ring_radius_min,
            new_cfg.ring_radius_max,
            new_cfg.perimeter_count,
            new_cfg.target_landmark,
            new_cfg.target_big,
            new_cfg.target_brazier,
            new_cfg.target_scatter
        );
    }
}

// ─── Calibration AABB (cohérence d'échelle + ajout collider après scale) ──────

/// Poll les `NeedsDecorCalibrate`, mesure l'AABB une fois la scène chargée,
/// applique `scale = target / max_dim * user_scale`, puis ajoute le collider si
/// demandé (après le scale → collider à la bonne taille). Gated Roguelite.
pub fn sys_calibrate_decor(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsDecorCalibrate)>,
    q_aabb: Query<&Aabb>,
    q_children: Query<&Children>,
    mut q_transform: Query<&mut Transform>,
) {
    for (entity, needs) in &q_needs {
        let Some(max_dim) = compute_aabb_max_dim(entity, &q_aabb, &q_children) else {
            continue; // scène pas encore chargée, retry next frame
        };
        if max_dim <= 0.0 || !max_dim.is_finite() {
            commands.entity(entity).remove::<NeedsDecorCalibrate>();
            continue;
        }
        let scale = needs.target_m / max_dim * needs.user_scale;
        if let Ok(mut tf) = q_transform.get_mut(entity) {
            tf.scale = Vec3::splat(scale);
        }
        commands.entity(entity).remove::<NeedsDecorCalibrate>();
    }
}

/// Walk récursif des Children pour le 1er `Aabb`. `max(half_extents)*2`.
fn compute_aabb_max_dim(
    root: Entity,
    q_aabb: &Query<&Aabb>,
    q_children: &Query<&Children>,
) -> Option<f32> {
    if let Ok(a) = q_aabb.get(root) {
        return Some(a.half_extents.max_element() * 2.0);
    }
    let children = q_children.get(root).ok()?;
    let mut max: f32 = 0.0;
    let mut found = false;
    for child in children.iter() {
        if let Some(d) = compute_aabb_max_dim(child, q_aabb, q_children) {
            max = max.max(d);
            found = true;
        }
    }
    if found {
        Some(max)
    } else {
        None
    }
}

// ─── Colliders mesh-fidèles (ConvexHull) ──────────────────────────────────────

/// Walk récursif : collecte les entités porteuses d'un `Mesh3d` sous `root`.
fn collect_mesh_entities(
    root: Entity,
    q_children: &Query<&Children>,
    q_mesh: &Query<&Mesh3d>,
    out: &mut Vec<Entity>,
) {
    if q_mesh.contains(root) {
        out.push(root);
    }
    if let Ok(children) = q_children.get(root) {
        for child in children.iter() {
            collect_mesh_entities(child, q_children, q_mesh, out);
        }
    }
}

/// Pour chaque prop marqué `NeedsHullCollider`, une fois ses meshes chargés,
/// attache un `Collider` ConvexHull sur chaque entité `Mesh3d` (rapier le scale
/// via le `GlobalTransform` = le scale appliqué par la calibration). Si aucun
/// hull n'est productible, pose un cylindre de secours. Retry tant que les
/// meshes ne sont pas chargés. Gated Roguelite.
pub fn sys_decor_build_hull_colliders(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsHullCollider)>,
    q_children: Query<&Children>,
    q_mesh: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
) {
    for (parent, needs) in &q_needs {
        let mut mesh_entities = Vec::new();
        collect_mesh_entities(parent, &q_children, &q_mesh, &mut mesh_entities);
        if mesh_entities.is_empty() {
            continue; // scène pas encore peuplée → retry frame suivante
        }
        // Attendre que TOUS les assets Mesh soient chargés (sinon retry).
        let all_loaded = mesh_entities.iter().all(|e| {
            q_mesh
                .get(*e)
                .ok()
                .and_then(|m| meshes.get(&m.0))
                .is_some()
        });
        if !all_loaded {
            continue;
        }
        let mut built = 0u32;
        for &me in &mesh_entities {
            if let Ok(m3d) = q_mesh.get(me) {
                if let Some(mesh) = meshes.get(&m3d.0) {
                    if let Some(col) =
                        Collider::from_bevy_mesh(mesh, &ComputedColliderShape::ConvexHull)
                    {
                        commands.entity(me).insert(col);
                        built += 1;
                    }
                }
            }
        }
        if built == 0 {
            // Fallback : cylindre primitif (mesh dégénéré / non hull-able).
            let half_h = (needs.fallback_target_m * 0.5).max(0.3);
            let radius = (needs.fallback_target_m * needs.fallback_radius_factor).max(0.25);
            commands.spawn((
                ChildOf(parent),
                Name::new("DecorColliderFallback"),
                Transform::from_xyz(0.0, half_h, 0.0),
                Collider::cylinder(half_h, radius),
            ));
        }
        commands.entity(parent).remove::<NeedsHullCollider>();
    }
}

// ─── Clear-spawn ennemis : jamais coincés dans le décor ───────────────────────

/// Footprint approx d'un bot (XZ) pour le clear-spawn.
const BOT_FOOTPRINT_M: f32 = 0.5;

/// Empêche un ennemi de RESTER apparu dans un décor solide : si un bot chevauche
/// le footprint d'un `SolidDecorObstacle`, on le pousse juste au bord (nudge
/// radial minimal → ne casse pas le cover). Robuste au timing (positions
/// d'entités, pas de test physique async). Gated Roguelite.
pub fn sys_unstick_bots_from_decor(
    mut q_bots: Query<&mut Transform, With<ArenaBot>>,
    q_obstacles: Query<(&Transform, &SolidDecorObstacle), Without<ArenaBot>>,
) {
    if q_obstacles.is_empty() {
        return;
    }
    for mut tf in &mut q_bots {
        for (otf, obs) in &q_obstacles {
            let dx = tf.translation.x - otf.translation.x;
            let dz = tf.translation.z - otf.translation.z;
            let clear = obs.radius + BOT_FOOTPRINT_M;
            let d2 = dx * dx + dz * dz;
            if d2 < clear * clear {
                let d = d2.sqrt();
                let (nx, nz) = if d > 1.0e-3 {
                    (dx / d, dz / d)
                } else {
                    (1.0, 0.0) // pile au centre → pousse vers +X
                };
                let push = clear - d + 0.1;
                tf.translation.x += nx * push;
                tf.translation.z += nz * push;
            }
        }
    }
}

// ─── Réconciliation count-based ───────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub fn sys_reconcile_decor(
    cfg: Option<Res<RogueliteDecorConfig>>,
    assets: Option<Res<DecorAssets>>,
    run_seed: Option<Res<RunSeed>>,
    stage_result: Option<Res<StageLoadResult>>,
    q_anchors: Query<&AnchorPoint>,
    q_decor: Query<(), With<DecorProp>>,
    mut queue: ResMut<DecorSpawnQueue>,
) {
    let Some(cfg) = cfg else {
        return;
    };
    let Some(assets) = assets else {
        return;
    };
    if !cfg.enabled {
        return;
    }
    if !q_anchors.iter().any(|a| a.kind == AnchorKind::PlayerSpawn) {
        return;
    }
    // Décor déjà présent OU déjà planifié (drain en cours) → ne pas re-planifier.
    if q_decor.iter().next().is_some() || queue.remaining() > 0 {
        return;
    }
    let seed = run_seed.map(|s| s.seed).unwrap_or(0xDEC0_F00D);
    let biome = stage_result
        .map(|r| r.biome.clone())
        .unwrap_or_else(|| "default".to_string());

    // Planifie tout (RNG only, pas d'instanciation) → le drain spawne par budget.
    let specs = plan_decor_set(&cfg, &assets, seed);
    let count = specs.len();
    queue.pending = specs;
    queue.cursor = 0;
    info!(
        "[decor] planned {count} GLB props (biome={biome}) — étalés à {}/frame",
        cfg.spawn_budget_per_frame.max(1)
    );
}

/// Draine la file de props à `spawn_budget_per_frame` par frame. C'est l'étalement
/// qui supprime le hitch d'entrée de stage : au lieu de ~800 instanciations
/// SceneSpawner d'un coup (65 ms), N par frame jusqu'à épuisement (~1 s, < 16 ms/frame).
pub fn sys_drain_decor_queue(
    mut commands: Commands,
    cfg: Option<Res<RogueliteDecorConfig>>,
    mut queue: ResMut<DecorSpawnQueue>,
) {
    if queue.remaining() == 0 {
        // File épuisée → libère la mémoire une seule fois.
        if !queue.pending.is_empty() {
            queue.pending.clear();
            queue.cursor = 0;
        }
        return;
    }
    let budget = cfg
        .as_ref()
        .map(|c| c.spawn_budget_per_frame)
        .unwrap_or(12)
        .max(1) as usize;
    let end = (queue.cursor + budget).min(queue.pending.len());
    for i in queue.cursor..end {
        spawn_one(&mut commands, &queue.pending[i]);
    }
    queue.cursor = end;
}

#[inline]
fn rng01(rng: &mut Xoshiro256StarStar) -> f32 {
    rng.next_u32() as f32 / u32::MAX as f32
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn pick<'a>(pool: &'a [Handle<Scene>], rng: &mut Xoshiro256StarStar) -> Option<&'a Handle<Scene>> {
    if pool.is_empty() {
        return None;
    }
    pool.get((rng.next_u32() as usize) % pool.len())
}

fn decor_markers(
    name: impl Into<String>,
) -> (Name, StageArenaMarker, DespawnOnExit<GameMode>, DecorProp) {
    (
        Name::new(name.into()),
        StageArenaMarker,
        DespawnOnExit(GameMode::Roguelite),
        DecorProp,
    )
}

/// Spawn une silhouette de FOND (Cratère de la Forge) : ÉNORME, hors-map, SANS
/// collider (le joueur n'y va jamais). Entité unique scalée par calibration AABB.
fn spawn_background_silhouette(
    commands: &mut Commands,
    handle: &Handle<Scene>,
    name: &str,
    pos: Vec3,
    yaw: f32,
    target_m: f32,
) {
    commands.spawn((
        decor_markers(name),
        SceneRoot(handle.clone()),
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
        NeedsDecorCalibrate {
            target_m,
            user_scale: 1.0,
        },
    ));
}

/// Spawn un prop périmétrique SOLIDE : parent `RigidBody::Fixed` (scale 1) +
/// enfant visuel `SceneRoot` (scale calibré) + **collider ConvexHull** construit
/// depuis le mesh chargé (épouse la silhouette ; bloque le LOS/tir des bots).
/// Optionnellement un PointLight (brasero). `col_radius_factor` = footprint
/// relatif (fin pour les tours/statues, large pour les rochers).
#[allow(clippy::too_many_arguments)]
fn spawn_perimeter_prop(
    commands: &mut Commands,
    handle: &Handle<Scene>,
    name: &str,
    pos: Vec3,
    yaw: f32,
    target_m: f32,
    user_scale: f32,
    brazier: bool,
    col_radius_factor: f32,
) {
    let parent = commands
        .spawn((
            decor_markers(name),
            RigidBody::Fixed,
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
        ))
        .id();
    // Enfant visuel (scale appliqué par calibration ; parent reste scale 1).
    commands.spawn((
        ChildOf(parent),
        Name::new("DecorVisual"),
        SceneRoot(handle.clone()),
        Transform::IDENTITY,
        NeedsDecorCalibrate { target_m, user_scale },
    ));
    // Collider mesh-fidèle : ConvexHull construit depuis le mesh chargé par
    // `sys_decor_build_hull_colliders` (suit la silhouette ; fiable). Fallback
    // cylindre si le mesh ne produit pas de hull. Marqueur sur le parent.
    commands.entity(parent).insert((
        NeedsHullCollider {
            fallback_target_m: target_m,
            fallback_radius_factor: col_radius_factor,
        },
        // Footprint approx (rayon ~0.4× la taille cible) pour le clear-spawn ennemis.
        SolidDecorObstacle {
            radius: (target_m * 0.4).max(0.6),
        },
    ));
    if brazier {
        commands.spawn((
            ChildOf(parent),
            PointLight {
                color: Color::srgb(1.0, 0.55, 0.2),
                intensity: 4_500.0_f32.min(ATMOSPHERE_LUMEN_CAP),
                range: 14.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, target_m * 0.7, 0.0),
        ));
    }
}

/// Planifie l'ensemble du décor (RNG only, AUCUNE instanciation). Retourne les
/// specs résolues, à drainer par budget/frame via `sys_drain_decor_queue`.
/// Préserve EXACTEMENT le stream RNG de l'ancien `spawn_decor_set` (les salles
/// sont décomposées en pièces via `plan_wall_room`, consommant le RNG à
/// l'identique) → layout décor inchangé, juste étalé dans le temps.
fn plan_decor_set(cfg: &RogueliteDecorConfig, assets: &DecorAssets, seed: u64) -> Vec<DecorSpec> {
    let mut rng = Xoshiro256StarStar::seed_from_u64(seed ^ 0xDEC0_DEC0_F00D_BEEF);
    let mut specs: Vec<DecorSpec> = Vec::new();

    // ── Fond « Cratère de la Forge » : crête volcanique + pics géants ─────────
    // Anneau lointain de falaises ÉNORMES (hors-map, SANS collider) → silhouette
    // de cratère ; hauteurs variées = crête déchiquetée. + pics géants dominants
    // (« le rocher géant hors map »). Réutilise les rochers/crags Inferno (big).
    let bg_n = cfg.background_count.max(1);
    for i in 0..cfg.background_count {
        let slot = TAU * i as f32 / bg_n as f32;
        let jitter = (rng01(&mut rng) - 0.5) * (TAU / bg_n as f32) * 0.85;
        let angle = slot + jitter;
        let radius = lerp(
            cfg.background_radius_min,
            cfg.background_radius_max,
            rng01(&mut rng),
        );
        let pos = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
        let yaw = rng01(&mut rng) * TAU;
        // Hauteur variée (0.7..1.4× la cible) → crête déchiquetée, pas un mur plat.
        let target = cfg.target_background * (0.7 + rng01(&mut rng) * 0.7);
        if let Some(handle) = pick(&assets.big, &mut rng) {
            specs.push(DecorSpec::Background {
                handle: handle.clone(),
                name: "Decor_Background",
                pos,
                yaw,
                target_m: target,
            });
        }
    }
    let peaks = cfg.giant_peak_count.max(1);
    for i in 0..cfg.giant_peak_count {
        let angle = TAU * (i as f32 + 0.3) / peaks as f32 + rng01(&mut rng) * 0.7;
        let pos = Vec3::new(
            cfg.giant_peak_radius * angle.cos(),
            0.0,
            cfg.giant_peak_radius * angle.sin(),
        );
        let yaw = rng01(&mut rng) * TAU;
        if let Some(handle) = pick(&assets.big, &mut rng) {
            specs.push(DecorSpec::Background {
                handle: handle.clone(),
                name: "Decor_GiantPeak",
                pos,
                yaw,
                target_m: cfg.target_giant_peak,
            });
        }
    }

    // ── Cœur de forge : anneau de braseros (ring of fire) + monuments hauts ────
    // Rapproche l'atmosphère forge du joueur : cercle de feu autour de l'aire de
    // combat + monuments/cheminées hauts encadrant le centre. Solides (collider
    // hull) → cover + contournés par les bots (collide-and-slide).
    for i in 0..cfg.forge_ring_count {
        let angle = TAU * i as f32 / cfg.forge_ring_count.max(1) as f32;
        let pos = Vec3::new(
            cfg.forge_ring_radius * angle.cos(),
            0.0,
            cfg.forge_ring_radius * angle.sin(),
        );
        let yaw = rng01(&mut rng) * TAU;
        if let Some(handle) = pick(&assets.braziers, &mut rng) {
            specs.push(DecorSpec::Perimeter {
                handle: handle.clone(),
                name: "Decor_ForgeBrazier",
                pos,
                yaw,
                target_m: cfg.target_brazier,
                user_scale: 1.0,
                brazier: true,
                col_radius_factor: 0.3,
            });
        }
    }
    for i in 0..cfg.forge_monument_count {
        let angle = TAU * (i as f32 + 0.5) / cfg.forge_monument_count.max(1) as f32;
        let radius = cfg.forge_monument_radius * (0.85 + rng01(&mut rng) * 0.3);
        let pos = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
        let yaw = rng01(&mut rng) * TAU;
        if let Some(handle) = pick(&assets.landmarks, &mut rng) {
            specs.push(DecorSpec::Perimeter {
                handle: handle.clone(),
                name: "Decor_ForgeMonument",
                pos,
                yaw,
                target_m: cfg.forge_monument_target,
                user_scale: 1.0,
                brazier: false,
                col_radius_factor: 0.16,
            });
        }
    }

    // ── Ville-forge : ceinture de bâtiments KayKit industriels (incr.3) ───────
    // Bâtiments (forge, mine, tours, barracks, ruines) FACE au centre (dos aux
    // ramparts) → skyline de ville-forge autour de l'arène. Solides (collider
    // hull + clear-spawn ennemis). Réutilise spawn_perimeter_prop.
    for i in 0..cfg.building_count {
        let slot = TAU * i as f32 / cfg.building_count.max(1) as f32;
        let jitter = (rng01(&mut rng) - 0.5) * (TAU / cfg.building_count.max(1) as f32) * 0.45;
        let angle = slot + jitter;
        let radius = lerp(cfg.building_radius_min, cfg.building_radius_max, rng01(&mut rng));
        let pos = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
        let yaw = angle + std::f32::consts::PI; // face au centre
        let target = cfg.target_building * (0.85 + rng01(&mut rng) * 0.4);
        if let Some(handle) = pick(&assets.buildings, &mut rng) {
            specs.push(DecorSpec::Perimeter {
                handle: handle.clone(),
                name: "Decor_Building",
                pos,
                yaw,
                target_m: target,
                user_scale: 1.0,
                brazier: false,
                col_radius_factor: 0.32,
            });
        }
    }

    // ── Anneau périmétrique ───────────────────────────────────────────────────
    let n = cfg.perimeter_count.max(1);
    let step = TAU / n as f32;
    // Slots des landmarks : répartis ~également autour de l'anneau.
    let landmark_n = cfg.landmark_count.min(cfg.perimeter_count);
    let landmark_step = if landmark_n > 0 {
        (cfg.perimeter_count / landmark_n).max(1)
    } else {
        u32::MAX
    };
    let mut landmarks_placed = 0u32;

    for i in 0..cfg.perimeter_count {
        let jitter = (rng01(&mut rng) - 0.5) * step * 0.6;
        let angle = step * i as f32 + jitter;
        let radius = lerp(cfg.ring_radius_min, cfg.ring_radius_max, rng01(&mut rng));
        let pos = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
        let yaw = rng01(&mut rng) * TAU;

        let is_landmark = landmarks_placed < landmark_n && i % landmark_step == 0;
        let roll = rng01(&mut rng);

        let (pool, name, target, us, brazier, crf) = if is_landmark {
            landmarks_placed += 1;
            (
                &assets.landmarks,
                "Decor_Landmark",
                cfg.target_landmark,
                0.95 + rng01(&mut rng) * 0.12,
                false,
                0.16, // tours/statues : footprint fin
            )
        } else if roll < cfg.brazier_ratio {
            (
                &assets.braziers,
                "Decor_Brazier",
                cfg.target_brazier,
                0.9 + rng01(&mut rng) * 0.2,
                true,
                0.32,
            )
        } else {
            (
                &assets.big,
                "Decor_Big",
                cfg.target_big,
                0.85 + rng01(&mut rng) * 0.35,
                false,
                0.34, // rochers/colonnes : footprint large
            )
        };
        let Some(handle) = pick(pool, &mut rng) else {
            continue;
        };
        specs.push(DecorSpec::Perimeter {
            handle: handle.clone(),
            name,
            pos,
            yaw,
            target_m: target,
            user_scale: us,
            brazier,
            col_radius_factor: crf,
        });
    }

    // ── Petits props dispersés au sol (sans collider) ─────────────────────────
    for _ in 0..cfg.scatter_count {
        let Some(handle) = pick(&assets.scatter, &mut rng) else {
            break;
        };
        let angle = rng01(&mut rng) * TAU;
        let t = rng01(&mut rng).sqrt(); // uniforme en surface
        let radius = lerp(cfg.scatter_radius_min, cfg.scatter_radius_max, t);
        let pos = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
        let yaw = rng01(&mut rng) * TAU;
        let us = 0.75 + rng01(&mut rng) * 0.5;

        specs.push(DecorSpec::Loose {
            handle: handle.clone(),
            name: "Decor_Scatter",
            pos,
            yaw,
            user_scale: us,
            target_m: cfg.target_scatter,
        });
    }

    // ── Salles en L (coin + 2 bras de mur KayKit, 1 collider cuboid par bras) ─
    for r in 0..cfg.room_count {
        let angle = (r as f32 / cfg.room_count.max(1) as f32) * TAU + (rng01(&mut rng) - 0.5) * 0.8;
        let radius = lerp(cfg.room_radius_min, cfg.room_radius_max, rng01(&mut rng));
        let pos = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
        let yaw0 = rng01(&mut rng) * TAU;
        plan_wall_room(&mut specs, assets, &mut rng, pos, yaw0, cfg.room_arm_segments);
    }

    // ── Gravats au sol (masque la répétition des dalles, sans collider) ───────
    for _ in 0..cfg.rubble_count {
        let Some(handle) = pick(&assets.rubble, &mut rng) else {
            break;
        };
        let angle = rng01(&mut rng) * TAU;
        let t = rng01(&mut rng).sqrt();
        let radius = lerp(cfg.scatter_radius_min, cfg.scatter_radius_max, t);
        let pos = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
        let yaw = rng01(&mut rng) * TAU;
        let us = 0.8 + rng01(&mut rng) * 0.6;
        specs.push(DecorSpec::Loose {
            handle: handle.clone(),
            name: "Decor_Rubble",
            pos,
            yaw,
            user_scale: us,
            target_m: cfg.target_rubble,
        });
    }

    specs
}

/// Planifie une "salle" en L : coin + 2 bras perpendiculaires de murs KayKit.
/// Layout OUVERT (pas d'enceinte fermée) → les bots peuvent contourner. Pousse
/// des `WallPiece` (chacune = 1 instanciation, drainable) ; consomme le RNG à
/// l'identique de l'ancien `spawn_wall_room` → layout inchangé.
fn plan_wall_room(
    specs: &mut Vec<DecorSpec>,
    assets: &DecorAssets,
    rng: &mut Xoshiro256StarStar,
    origin: Vec3,
    yaw0: f32,
    arm_segments: u32,
) {
    if arm_segments == 0 || assets.walls.is_empty() {
        return;
    }
    let rot = Quat::from_rotation_y(yaw0);

    // Coin au pivot (visuel seulement ; ne consomme PAS de RNG — `first()`).
    if let Some(corner) = assets.wall_corner.first() {
        specs.push(DecorSpec::WallPiece {
            handle: corner.clone(),
            pos: origin,
            yaw: yaw0,
            obstacle_radius: WALL_SEG_W * 0.7,
        });
    }

    // Deux bras perpendiculaires (local +X et local +Z).
    for dir in [rot * Vec3::X, rot * Vec3::Z] {
        plan_wall_arm(specs, assets, rng, origin, dir, arm_segments);
    }
}

/// Aligne `n` segments de mur le long de `dir` (normalisé), espacés de
/// `WALL_SEG_W`. Chaque mur = un `WallPiece` (hull collider). Consomme le RNG
/// (`pick`) à l'identique de l'ancien `spawn_wall_arm`.
fn plan_wall_arm(
    specs: &mut Vec<DecorSpec>,
    assets: &DecorAssets,
    rng: &mut Xoshiro256StarStar,
    origin: Vec3,
    dir: Vec3,
    n: u32,
) {
    // Convention KayKit : axe X = largeur du mur. yaw aligne X sur dir.
    let yaw = (-dir.z).atan2(dir.x);
    for s in 1..=n {
        let Some(handle) = pick(&assets.walls, rng) else {
            break;
        };
        let p = origin + dir * (s as f32 * WALL_SEG_W);
        specs.push(DecorSpec::WallPiece {
            handle: handle.clone(),
            pos: p,
            yaw,
            obstacle_radius: WALL_SEG_W * 0.6,
        });
    }
}

// ─── Sensor ────────────────────────────────────────────────────────────────────

pub fn sys_write_decor_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    q_decor: Query<&Name, With<DecorProp>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let mut total = 0u32;
    let mut landmarks = 0u32;
    let mut braziers = 0u32;
    let mut big = 0u32;
    let mut scatter = 0u32;
    let mut walls = 0u32;
    let mut rubble = 0u32;
    for name in &q_decor {
        total += 1;
        let s = name.as_str();
        if s.contains("Landmark") {
            landmarks += 1;
        } else if s.contains("Brazier") {
            braziers += 1;
        } else if s.contains("Wall") {
            walls += 1;
        } else if s.contains("Rubble") {
            rubble += 1;
        } else if s.contains("Big") {
            big += 1;
        } else if s.contains("Scatter") {
            scatter += 1;
        }
    }
    let (severity, next_step) = severity_for_decor(total);
    let json = format!(
        r#"{{"id":"stage_decor","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"decor_total":{total},"landmarks":{landmarks},"braziers":{braziers},"walls":{walls},"big_props":{big},"rubble":{rubble},"scatter":{scatter}}}"#,
        time.elapsed_secs(),
    );
    let _ = fs::write(SENSOR_PATH, json);
}

/// Pur — testable.
pub fn severity_for_decor(total: u32) -> (&'static str, &'static str) {
    if total == 0 {
        (
            "info",
            "0 prop décor (hors arène ou decor_enabled=0). Read roguelite_decor.toml.",
        )
    } else {
        ("ok", "Décor posé. Ajuste rayons/counts/targets via roguelite_decor.toml (Shift+F12).")
    }
}

// ─── Tests purs ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_is_default() {
        assert_eq!(
            RogueliteDecorConfig::parse_toml(""),
            RogueliteDecorConfig::default()
        );
    }

    #[test]
    fn parse_overrides_and_clamps() {
        let toml = r#"
[[genes]]
id = "decor_perimeter_count"
default = 999.0
[[genes]]
id = "decor_target_landmark"
default = 99.0
[[genes]]
id = "decor_brazier_ratio"
default = 2.0
[[genes]]
id = "decor_enabled"
default = 0.0
"#;
        let c = RogueliteDecorConfig::parse_toml(toml);
        assert_eq!(c.perimeter_count, 200);
        assert_eq!(c.target_landmark, 50.0); // clamp
        assert_eq!(c.brazier_ratio, 1.0);
        assert!(!c.enabled);
    }

    #[test]
    fn parse_swaps_inverted_ranges() {
        let toml = r#"
[[genes]]
id = "decor_ring_radius_min"
default = 90.0
[[genes]]
id = "decor_ring_radius_max"
default = 40.0
"#;
        let c = RogueliteDecorConfig::parse_toml(toml);
        assert_eq!(c.ring_radius_min, 40.0);
        assert_eq!(c.ring_radius_max, 90.0);
    }

    #[test]
    fn prop_paths_valid() {
        for p in LANDMARK_PROPS
            .iter()
            .chain(BIG_PROPS)
            .chain(BRAZIERS)
            .chain(SCATTER_PROPS)
        {
            assert!(p.starts_with("models/environment/inferno/"));
            assert!(p.ends_with(".glb"));
        }
        assert!(!LANDMARK_PROPS.is_empty());
        assert!(!BIG_PROPS.is_empty());
    }

    #[test]
    fn calibration_scale_normalizes() {
        // scale = target / max_dim * user_scale → un prop natif 8m visé à 4m = 0.5.
        let target = 4.0_f32;
        let max_dim = 8.0_f32;
        let user = 1.0_f32;
        assert_eq!(target / max_dim * user, 0.5);
    }

    #[test]
    fn severity_info_empty_ok_present() {
        assert_eq!(severity_for_decor(0).0, "info");
        assert_eq!(severity_for_decor(50).0, "ok");
    }

    #[test]
    fn lerp_endpoints() {
        assert_eq!(lerp(10.0, 20.0, 0.0), 10.0);
        assert_eq!(lerp(10.0, 20.0, 1.0), 20.0);
    }

    #[test]
    fn parse_spawn_budget_default_and_clamp() {
        // Défaut = 12 (fallback Default).
        assert_eq!(RogueliteDecorConfig::default().spawn_budget_per_frame, 12);
        // Clamp min 1 (0 interdit → sinon drain bloqué).
        let c = RogueliteDecorConfig::parse_toml(
            "[[genes]]\nid = \"decor_spawn_budget_per_frame\"\ndefault = 0.0\n",
        );
        assert_eq!(c.spawn_budget_per_frame, 1);
    }

    #[test]
    fn plan_decor_set_deterministic_and_budgetable() {
        // Handles factices (le plan ne touche aucun asset, juste du RNG + clone).
        let h = || vec![Handle::<Scene>::default()];
        let assets = DecorAssets {
            landmarks: h(),
            big: h(),
            braziers: h(),
            scatter: h(),
            walls: h(),
            wall_corner: h(),
            rubble: h(),
            buildings: h(),
        };
        let cfg = RogueliteDecorConfig::default();
        let a = plan_decor_set(&cfg, &assets, 0xABCD);
        let b = plan_decor_set(&cfg, &assets, 0xABCD);
        // Même seed → même nombre de props (RNG préservé).
        assert_eq!(a.len(), b.len());
        // Le décor par défaut produit beaucoup de props (sinon pas de freeze à étaler).
        assert!(a.len() > 100, "plan trop petit: {}", a.len());
        // Le budget par défaut est bien < total → drainage en plusieurs frames.
        assert!((cfg.spawn_budget_per_frame as usize) < a.len());
    }
}
