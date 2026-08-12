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
use forgia_core::layout::{covers_expected, disc_area, poisson_disk_annulus};
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

/// Hauteur à partir de laquelle un prop CASSE la ligne de vue (m).
///
/// L'œil du joueur est à 1,70 m et il n'y a PAS d'accroupissement : en dessous,
/// un obstacle masque le corps sans masquer la vue — il ne sert à rien
/// (`map-design-patterns.md` §11). Ce n'est pas un réglage, c'est la géométrie
/// du personnage.
const SIGHT_BREAK_H_M: f32 = 1.80;

const GENOME_PATH: &str = "assets/genomes/roguelite/roguelite_decor.toml";
const SENSOR_PATH: &str = "forgia2_stage_decor.json";
const POLL_PERIOD_SEC: f32 = 1.0;
const ATMOSPHERE_LUMEN_CAP: f32 = 8_000.0;

// ─── Catalogues de props : PARTIS EN COUCHE DEFINITION (story-671) ───────────
// Les ex-`const LANDMARK_PROPS / BIG_PROPS / BRAZIERS / SCATTER_PROPS /
// WALL_VARIANTS / WALL_CORNER / RUBBLE_PROPS / BUILDINGS` vivent désormais dans
// `assets/genomes/roguelite/roguelite_palettes.toml` (cf `decor_palettes.rs`),
// une entrée par DIRECTION ARTISTIQUE. Le miroir Rust de la palette historique
// est `DecorPalette::inferno()` : sans génome, le jeu se comporte exactement
// comme avant story-671.

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
    pub landmark_count: u32,
    pub brazier_ratio: f32,
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
    // Story-672 — zones interdites au décor (voir `SpawnKeepout`).
    pub keepout_player_m: f32,
    pub keepout_spawn_margin_m: f32,
    // Story-674 — aménagement dérivé (bruit bleu + compte depuis l'aire).
    pub scatter_spacing_m: f32,
    pub perimeter_spacing_m: f32,
    // Story-688 — le COUVERT de l'aire de combat.
    pub cover_radius_min_m: f32,
    pub cover_spacing_m: f32,
    pub max_props: u32,
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
            landmark_count: 4,
            brazier_ratio: 0.30,
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
            keepout_player_m: 8.0,
            keepout_spawn_margin_m: 3.5,
            scatter_spacing_m: 5.0,
            perimeter_spacing_m: 9.0,
            cover_radius_min_m: 20.0,
            cover_spacing_m: 8.0,
            max_props: 420,
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
                "decor_landmark_count" => c.landmark_count = gene.default.clamp(0.0, 50.0) as u32,
                "decor_brazier_ratio" => c.brazier_ratio = gene.default.clamp(0.0, 1.0),
                "decor_scatter_radius_min" => c.scatter_radius_min = gene.default.clamp(0.0, 500.0),
                "decor_scatter_radius_max" => c.scatter_radius_max = gene.default.clamp(0.0, 500.0),
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
                "decor_keepout_player_m" => c.keepout_player_m = gene.default.clamp(0.0, 60.0),
                "decor_scatter_spacing_m" => c.scatter_spacing_m = gene.default.clamp(0.5, 40.0),
                "decor_cover_radius_min_m" => c.cover_radius_min_m = gene.default.clamp(0.0, 200.0),
                // Bande sourcée 3-10 m (Watch Dogs, Gears) — cf §11.
                "decor_cover_spacing_m" => c.cover_spacing_m = gene.default.clamp(3.0, 10.0),
                "decor_perimeter_spacing_m" => {
                    c.perimeter_spacing_m = gene.default.clamp(0.5, 40.0)
                }
                "decor_max_props" => c.max_props = gene.default.clamp(0.0, 4000.0) as u32,
                "decor_keepout_spawn_margin_m" => {
                    c.keepout_spawn_margin_m = gene.default.clamp(0.0, 30.0)
                }
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
                "decor_giant_peak_radius" => c.giant_peak_radius = gene.default.clamp(20.0, 800.0),
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
                "decor_building_count" => c.building_count = gene.default.clamp(0.0, 80.0) as u32,
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
/// Handles d'une DA (story-671). C'est l'ex-`DecorAssets` : le planificateur ne
/// voit toujours qu'un seul jeu de props à la fois, celui de la salle en cours.
/// Un prop chargé + ses MESURES natives (story-673). Les mesures viennent de
/// `asset_registry.toml` ; sans elles on retombe sur l'estimation d'avant.
#[derive(Clone)]
pub struct DecorAsset {
    pub scene: Handle<Scene>,
    /// Emprise au sol NATIVE (m). 0 = inconnue.
    pub native_footprint_m: f32,
    /// Plus grande dimension native (m) — ce que la calibration ramène à la
    /// taille cible. 0 = inconnue.
    pub native_max_dim_m: f32,
    /// Hauteur NATIVE (m). 0 = inconnue.
    ///
    /// Story-688 — sert à dériver le RÔLE. Attention : c'est la hauteur EN JEU
    /// qui décide, pas celle-ci. Le kit hexagon est en miniatures (un bâtiment
    /// fait 0,93 m nativement) et le décor le recalibre à `target_big` = 7 m.
    /// Filtrer sur le natif conclurait « aucune de ces cartes n'a de couvert »,
    /// ce qui est faux — c'est exactement l'erreur des 1,92 m de story-672.
    pub native_height_m: f32,
}

impl DecorAsset {
    /// Hauteur RÉELLE une fois le prop calibré à `target_m`.
    ///
    /// C'est elle qui décide du rôle : ≥ 1,80 m casse la ligne de vue, donc
    /// c'est du COUVERT (`map-design-patterns.md` §11). En dessous, l'objet
    /// masque le corps sans masquer la vue — il ne sert à rien.
    pub fn height_at(&self, target_m: f32) -> f32 {
        if target_m <= 0.0 || self.native_max_dim_m <= 1.0e-4 {
            return self.native_height_m;
        }
        self.native_height_m * (target_m / self.native_max_dim_m)
    }

    /// Emprise RÉELLE une fois le prop calibré à `target_m`.
    ///
    /// Sans mesure : l'ancienne estimation `0,5 × target`. C'est un repli assumé,
    /// pas un défaut silencieux — le chargement loggue combien d'assets sont sans
    /// mesure.
    pub fn footprint_at(&self, target_m: f32) -> f32 {
        if self.native_max_dim_m > 1.0e-4 && self.native_footprint_m > 0.0 {
            self.native_footprint_m * (target_m / self.native_max_dim_m)
        } else {
            0.5 * target_m
        }
    }
}

#[derive(Default, Clone)]
pub struct DecorPaletteAssets {
    pub landmarks: Vec<DecorAsset>,
    pub big: Vec<DecorAsset>,
    pub braziers: Vec<DecorAsset>,
    pub scatter: Vec<DecorAsset>,
    pub walls: Vec<DecorAsset>,
    pub wall_corner: Vec<DecorAsset>,
    pub rubble: Vec<DecorAsset>,
    pub buildings: Vec<DecorAsset>,
}

/// Toutes les DA préchargées, indexées par id de palette (story-671).
#[derive(Resource, Default)]
pub struct DecorAssets {
    by_palette: std::collections::HashMap<String, DecorPaletteAssets>,
    /// Repli : la DA historique. Garantit qu'une salle n'est JAMAIS sans props.
    fallback: DecorPaletteAssets,
}

impl DecorAssets {
    /// Handles d'une palette, avec repli explicite sur la DA historique.
    pub fn for_palette(&self, id: &str) -> &DecorPaletteAssets {
        self.by_palette.get(id).unwrap_or(&self.fallback)
    }

    pub fn palette_count(&self) -> usize {
        self.by_palette.len()
    }

    /// Tous les handles, TOUTES palettes confondues — pour le préchauffage des
    /// pipelines. Il faut chauffer les 4 DA, pas seulement celle de la salle 1 :
    /// sinon entrer dans une salle d'une DA jamais vue déclenche la spécialisation
    /// PBR en plein combat (cf `reference_pbr_pipeline_warmup_frustum_trap`).
    /// Coût assumé : un premier Lobby plus long, une seule fois par session.
    pub fn all_handles(&self) -> impl Iterator<Item = &Handle<Scene>> {
        self.by_palette.values().flat_map(|p| {
            [
                &p.landmarks,
                &p.big,
                &p.braziers,
                &p.scatter,
                &p.walls,
                &p.wall_corner,
                &p.rubble,
                &p.buildings,
            ]
            .into_iter()
            .flatten()
            .map(|a| &a.scene)
        })
    }
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
/// `Collider` mesh-fidèle construit depuis chaque mesh chargé (bâti APRÈS
/// chargement, pas le souci async de `AsyncSceneCollider`). La forme dépend de
/// `precise`. Fallback cylindre si aucun collider n'est productible.
#[derive(Component, Debug, Clone, Copy)]
pub struct NeedsHullCollider {
    pub fallback_target_m: f32,
    pub fallback_radius_factor: f32,
    /// `true` → **TriMesh** (épouse exactement la géométrie : les creux sont
    /// préservés, on peut passer entre les jambes d'une statue). Réservé aux gros
    /// props concaves (statues, colonnes brisées). `false` → **ConvexHull**
    /// (enveloppe convexe : plus léger + fallback cylindre plus sûr, pour les murs
    /// qui n'ont pas de creux à préserver).
    pub precise: bool,
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
        /// Emprise au sol RÉELLE (m), mesurée puis calibrée. Story-673.
        footprint_m: f32,
        /// Hauteur EN JEU (m), une fois calibré à `target_m`. Story-690 — c'est
        /// elle qui décide si le prop est un abri (≥ 1,80 m) ou un obstacle
        /// décoratif, et le capteur d'arène ne peut pas la deviner après coup.
        height_m: f32,
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
        /// Emprise au sol RÉELLE (m), mesurée puis calibrée. Story-673.
        footprint_m: f32,
        name: &'static str,
        pos: Vec3,
        yaw: f32,
        user_scale: f32,
        target_m: f32,
    },
    /// Segment de mur (coin ou bras de salle en L) — hull collider + obstacle.
    WallPiece {
        handle: Handle<Scene>,
        /// Emprise au sol RÉELLE (m). Story-673.
        footprint_m: f32,
        /// Hauteur EN JEU (m). Story-690 — les murs sont posés à l'échelle
        /// native, donc c'est leur hauteur mesurée telle quelle.
        height_m: f32,
        pos: Vec3,
        yaw: f32,
        obstacle_radius: f32,
    },
}

/// Rayon d'emprise au sol d'un prop (m), pour le test de zone interdite.
///
/// Volontairement GÉNÉREUX : la moitié de la dimension cible, sans tenir compte
/// de la forme réelle du mesh. Un prop rejeté à tort coûte un trou dans le décor ;
/// un prop accepté à tort coûte un joueur qui apparaît dedans. Les deux erreurs
/// n'ont pas le même prix.
impl DecorSpec {
    fn ground_pos(&self) -> Vec2 {
        let p = match self {
            DecorSpec::Background { pos, .. }
            | DecorSpec::Perimeter { pos, .. }
            | DecorSpec::Loose { pos, .. }
            | DecorSpec::WallPiece { pos, .. } => *pos,
        };
        Vec2::new(p.x, p.z)
    }

    fn footprint_radius(&self) -> f32 {
        match self {
            // Le fond est hors-map et sans collider : il ne bloque personne.
            DecorSpec::Background { .. } => 0.0,
            DecorSpec::Perimeter { footprint_m, .. }
            | DecorSpec::Loose { footprint_m, .. }
            | DecorSpec::WallPiece { footprint_m, .. } => *footprint_m,
        }
    }

    /// Le fond n'est jamais filtré : il est hors de l'enceinte par construction.
    fn is_background(&self) -> bool {
        matches!(self, DecorSpec::Background { .. })
    }

    /// Hauteur en jeu (m) — 0 pour ce qui ne bloque rien.
    ///
    /// Story-690 : c'est elle qui fait la différence entre un abri et un caillou,
    /// et elle n'est connue qu'ICI, au plan, quand l'asset et sa taille cible
    /// sont encore en main.
    fn height_m(&self) -> f32 {
        match self {
            DecorSpec::Perimeter { height_m, .. } | DecorSpec::WallPiece { height_m, .. } => {
                *height_m
            }
            DecorSpec::Background { .. } | DecorSpec::Loose { .. } => 0.0,
        }
    }

    /// Porte-t-il un collider ? C'est le seul critère qui décide si un mob peut
    /// s'y coincer. `Loose` = semis au sol sans collider, traversable.
    fn is_solid(&self) -> bool {
        matches!(
            self,
            DecorSpec::Perimeter { .. } | DecorSpec::WallPiece { .. }
        )
    }
}

/// Story-672 — LA CARTE DES OBSTACLES SOLIDES, publiée dès la PLANIFICATION.
///
/// Le décor est instancié étalé sur plusieurs frames (`spawn_budget_per_frame`),
/// mais les ennemis apparaissent d'un coup au début d'une vague. Interroger les
/// entités déjà spawnées donnerait donc une liste incomplète et le résultat
/// dépendrait du timing. On publie les emprises **au moment du plan**, où elles
/// sont toutes connues et exactes.
///
/// C'est ce qui permet d'inverser le sens : le décor se pose dense et cohérent,
/// et ce sont les points d'apparition qui cherchent une place libre dedans.
#[derive(Resource, Debug, Clone, Default)]
pub struct DecorObstacles {
    /// (centre au sol, rayon d'emprise) de chaque prop SOLIDE **du décor**.
    ///
    /// Rempli par `sys_reconcile_decor` au moment du PLAN, avant instanciation.
    pub discs: Vec<(Vec2, f32)>,
    /// (centre au sol, rayon) des solides de l'ARÈNE : murs de pièces, modules de
    /// layout, **bâtiments autorés**, remparts.
    ///
    /// # Pourquoi ce champ existe (2026-08-12, rapporté en jeu)
    ///
    /// « Certains ennemis spawnent dans un bâtiment et restent bloqués dedans, je les
    /// vois à travers. » Cause : la recherche de place libre ne consultait que `discs`,
    /// c'est-à-dire **le décor procédural seul**. Le marqueur `SolidDecorObstacle`
    /// n'existe que dans ce fichier — vérifié, zéro occurrence ailleurs dans les 66
    /// crates. Un bâtiment autoré n'existait donc tout simplement pas pour le spawn.
    ///
    /// La source est `forgia_stage::ArenaGeometry`, la **même que le maillage de
    /// navigation** : une seule vérité pour « qu'est-ce qui est solide », au lieu de
    /// deux listes qui divergent. Un champ séparé plutôt qu'un mélange dans `discs`
    /// parce que chacun garde alors un producteur unique — `discs` au décor, `arena`
    /// à la géométrie — et qu'aucune écriture n'écrase l'autre.
    pub arena: Vec<(Vec2, f32)>,
}

impl DecorObstacles {
    /// TOUS les solides — décor **et** arène. C'est le seul itérateur que les tests
    /// de dégagement doivent utiliser : en oublier un est précisément le défaut du
    /// 2026-08-12 (ennemis nés dans les bâtiments autorés).
    pub fn solides(&self) -> impl Iterator<Item = &(Vec2, f32)> {
        self.discs.iter().chain(self.arena.iter())
    }

    /// Combien de solides sont réellement pris en compte. **Zéro n'est pas « dégagé »,
    /// c'est aveugle** — et c'est exactement l'état dans lequel le spawn a vécu.
    pub fn mesures(&self) -> usize {
        self.discs.len() + self.arena.len()
    }

    /// PUR — cette position est-elle libre pour un corps de rayon `body_radius` ?
    pub fn is_clear(&self, pos: Vec2, body_radius: f32) -> bool {
        let r = body_radius.max(0.0);
        !self.solides().any(|(c, rad)| pos.distance(*c) < rad + r)
    }

    /// Distance au bord de l'obstacle le plus proche (négatif = à l'intérieur).
    /// Sert de score pour choisir « le moins mauvais » quand tout est encombré.
    pub fn clearance(&self, pos: Vec2, body_radius: f32) -> f32 {
        self.solides()
            .map(|(c, rad)| pos.distance(*c) - rad - body_radius.max(0.0))
            .fold(f32::INFINITY, f32::min)
    }

    /// PUR — cherche un angle LIBRE sur un anneau de rayon `radius`.
    ///
    /// Part de `wanted` et balaie l'anneau par pas réguliers. Retourne le premier
    /// angle libre ; si l'anneau entier est encombré, retourne celui qui maximise
    /// le dégagement — **jamais rien, jamais l'angle voulu par défaut** : un
    /// ennemi doit toujours apparaître quelque part, mais au moins au moins pire
    /// endroit. `tries` bornes le coût (spawn ponctuel, pas un hot path).
    pub fn clear_angle_on_ring(
        &self,
        radius: f32,
        wanted: f32,
        body_radius: f32,
        tries: u32,
    ) -> f32 {
        self.clear_angle_on_ring_at(Vec2::ZERO, radius, wanted, body_radius, tries)
    }

    /// Story-686 — même recherche, mais autour d'un CENTRE quelconque.
    ///
    /// L'anneau d'apparition est centré sur le JOUEUR, pas sur l'origine du
    /// monde. Chercher la place libre autour de l'origine alors que les ennemis
    /// naissent autour du joueur testerait le mauvais endroit.
    pub fn clear_angle_on_ring_at(
        &self,
        center: Vec2,
        radius: f32,
        wanted: f32,
        body_radius: f32,
        tries: u32,
    ) -> f32 {
        let n = tries.max(1);
        let step = std::f32::consts::TAU / n as f32;
        let mut best = (wanted, f32::NEG_INFINITY);
        for i in 0..n {
            let a = wanted + step * i as f32;
            let p = center + Vec2::new(radius * a.cos(), radius * a.sin());
            if self.is_clear(p, body_radius) {
                return a;
            }
            let c = self.clearance(p, body_radius);
            if c > best.1 {
                best = (a, c);
            }
        }
        best.0
    }
}

/// Story-672 — LA ZONE OÙ AUCUN PROP NE DOIT ATTERRIR.
///
/// Symptôme d'origine (2026-07-31, rapporté en jeu) : « je spawne dans un asset
/// et parfois les mobs sont bloqués dans le décor ». Cause : les anneaux
/// d'apparition ennemis (12 / 25 / 50 m) tombent en plein dans les anneaux de
/// décor (scatter 12→72 m, périmètre 42→74 m). Les bots n'ayant pas de navmesh,
/// un ennemi né contre un pilier pousse dedans sans jamais le contourner.
///
/// Le filtre s'applique en SORTIE de planification : il couvre donc TOUS les
/// générateurs de props d'un coup, au lieu d'aller corriger chaque rayon un par
/// un — c'est la classe du défaut, pas ses symptômes.
#[derive(Debug, Clone, Default)]
pub struct SpawnKeepout {
    /// Disque autour de l'apparition du joueur : (centre au sol, rayon).
    ///
    /// **C'est la SEULE zone interdite au décor.** La 1re version réservait aussi
    /// les anneaux d'apparition ennemis : mesuré, ça interdisait **54 % du rayon
    /// utile** au solide et vidait les salles — pour un résultat qui ne marchait
    /// même pas (emprises sous-estimées). Les ennemis, eux, cherchent maintenant
    /// une place libre DANS le décor (`clear_spawn_angle`), au lieu d'exiger que
    /// le décor leur cède la moitié de l'arène.
    pub player: (Vec2, f32),
    /// 2026-08-04 — les PORTES du complexe de pièces.
    ///
    /// Le semis les ignorait complètement : `decor.rs` ne mentionnait ni pièce ni
    /// couloir, et ce champ-ci se documentait comme « la SEULE zone interdite ».
    /// Résultat rapporté en jeu : des props en travers des passages.
    ///
    /// On ne réserve QUE les portes, pas le complexe : dégager les pièces entières
    /// coûterait 5,7 % de l'arène et les rendrait nues — l'erreur que
    /// `spawn-clearance.md` documente comme « un invariant qui vide la scène ».
    pub doors: forgia_stage::rooms::RoomDoors,
}

impl SpawnKeepout {
    /// Construit les zones depuis les DEUX génomes concernés. Pas de miroir : les
    /// rayons d'anneaux sont lus directement sur la config de composition, donc
    /// changer `ring.tank` dans `roguelite_waves.toml` déplace automatiquement la
    /// zone interdite. Aucune dérive possible.
    /// `player_pos` = position RÉELLE du joueur au sol au moment de planifier. Le
    /// joueur est spawné par `forgia-player`, pas par ce crate : supposer l'origine
    /// protégeait potentiellement le mauvais endroit — c'est le « j'ai respawn en
    /// plein sur un asset » du 2026-07-31.
    pub fn around_player(decor: &RogueliteDecorConfig, player_pos: Vec2) -> Self {
        Self {
            player: (player_pos, decor.keepout_player_m.max(0.0)),
            doors: forgia_stage::rooms::RoomDoors::default(),
        }
    }

    /// Idem, plus les PORTES du complexe de pièces à laisser franchissables.
    ///
    /// Séparé de `around_player` pour que les appelants sans complexe (stages
    /// autorés, tests) gardent le comportement d'avant sans rien passer.
    pub fn around_player_and_doors(
        decor: &RogueliteDecorConfig,
        player_pos: Vec2,
        doors: forgia_stage::rooms::RoomDoors,
    ) -> Self {
        Self {
            player: (player_pos, decor.keepout_player_m.max(0.0)),
            doors,
        }
    }

    /// PUR — ce prop atterrit-il sur le point d'apparition du joueur ?
    ///
    /// On refuse TOUT autour de lui, collider ou pas : c'est là qu'il ouvre les
    /// yeux, et apparaître le nez dans un tonneau même traversable est exactement
    /// le symptôme rapporté. Le rayon du prop entre dans le test.
    pub fn blocks(&self, pos: Vec2, prop_radius: f32) -> bool {
        let r = prop_radius.max(0.0);
        pos.distance(self.player.0) < self.player.1 + r
            // Une porte bouchée coupe une pièce du reste du complexe : c'est pire
            // qu'un prop mal placé, c'est un chemin qui disparaît.
            || self.doors.blocks_a_door(pos.x, pos.y, r)
    }
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
            ..
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
            ..
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
            ..
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
            ..
        } => {
            commands.spawn((
                decor_markers("Decor_Wall"),
                SceneRoot(handle.clone()),
                Transform::from_translation(*pos).with_rotation(Quat::from_rotation_y(*yaw)),
                NeedsHullCollider {
                    fallback_target_m: WALL_HEIGHT,
                    fallback_radius_factor: 0.3,
                    precise: false, // mur = convexe (pas de creux à préserver, fallback sûr)
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
        "[decor] genome loaded — ring {:.0}-{:.0}m espacement perim={:.1}m semis={:.1}m          landmarks={} braziers={:.0}% plafond={} props",
        cfg.ring_radius_min,
        cfg.ring_radius_max,
        cfg.perimeter_spacing_m,
        cfg.scatter_spacing_m,
        cfg.landmark_count,
        cfg.brazier_ratio * 100.0,
        cfg.max_props
    );
}

/// Précharge toutes les scènes GLB une fois (un seul call-site `load`).
pub fn sys_load_decor_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    palettes: Option<Res<crate::decor_palettes::DecorPalettesConfig>>,
    metrics: Option<Res<crate::asset_metrics::AssetRegistry>>,
) {
    // Story-673 — chaque prop est chargé AVEC ses mesures. C'est ici, et seulement
    // ici, qu'on dispose à la fois du chemin et du registre : la suite du pipeline
    // ne manipule plus que des `DecorAsset` porteurs de leur emprise native.
    let mk = |path: &str| -> DecorAsset {
        let m = metrics.as_ref().and_then(|r| r.get(path));
        DecorAsset {
            scene: asset_server.load(GltfAssetLabel::Scene(0).from_asset(path.to_string())),
            native_footprint_m: m.map(|m| m.footprint_radius_m).unwrap_or(0.0),
            native_height_m: m.map(|m| m.height_m).unwrap_or(0.0),
            native_max_dim_m: m.map(|m| m.max_dim()).unwrap_or(0.0),
        }
    };
    let load = |paths: &[&str]| -> Vec<DecorAsset> {
        paths
            .iter()
            .filter(|p| !p.is_empty())
            .map(|p| mk(p))
            .collect()
    };
    let load_owned =
        |paths: &[String]| -> Vec<DecorAsset> { paths.iter().map(|p| mk(p)).collect() };
    // Story-671 — précharge TOUTES les DA déclarées au génome. `Option<Res<..>>`
    // + repli sur un chargement direct : ce système ne dépend d'aucun ordre de
    // Startup (les `insert_resource` d'un autre système ne sont pas encore
    // appliqués à ce moment-là).
    let cfg = palettes
        .map(|c| c.clone())
        .unwrap_or_else(crate::decor_palettes::DecorPalettesConfig::load_or_default_public);
    let mut by_palette = std::collections::HashMap::new();
    for (id, p) in &cfg.palettes {
        by_palette.insert(
            id.clone(),
            DecorPaletteAssets {
                landmarks: load_owned(&p.landmarks),
                big: load_owned(&p.big),
                braziers: load_owned(&p.braziers),
                scatter: load_owned(&p.scatter),
                walls: load_owned(&p.walls),
                wall_corner: load(&[p.wall_corner.as_str()]),
                rubble: load_owned(&p.rubble),
                buildings: load_owned(&p.buildings),
            },
        );
    }
    let fallback = by_palette
        .get(crate::decor_palettes::FALLBACK_PALETTE)
        .cloned()
        .unwrap_or_default();
    let n = by_palette.len();
    commands.insert_resource(DecorAssets {
        by_palette,
        fallback,
    });
    info!("[decor] {n} DA préchargées");
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
            "[decor] HOT-RELOADED — ring {:.0}-{:.0}m espacement {:.1}m targets L{:.0}/B{:.0}/Br{:.1}/S{:.1} → régénération",
            new_cfg.ring_radius_min,
            new_cfg.ring_radius_max,
            new_cfg.perimeter_spacing_m,
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
/// attache un `Collider` mesh-fidèle (TriMesh si `precise`, sinon ConvexHull) sur
/// chaque entité `Mesh3d` (rapier le scale via le `GlobalTransform` = le scale
/// appliqué par la calibration). Si aucun collider n'est productible, pose un
/// cylindre de secours. Retry tant que les meshes ne sont pas chargés. Gated Roguelite.
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
        let all_loaded = mesh_entities
            .iter()
            .all(|e| q_mesh.get(*e).ok().and_then(|m| meshes.get(&m.0)).is_some());
        if !all_loaded {
            continue;
        }
        // Forme mesh-fidèle : TriMesh (creux préservés, p.ex. entre les jambes
        // d'une statue) pour les gros props concaves, ConvexHull (plus léger/sûr)
        // pour les murs. Cf le champ `precise` de `NeedsHullCollider`.
        let shape = if needs.precise {
            ComputedColliderShape::TriMesh(default())
        } else {
            ComputedColliderShape::ConvexHull
        };
        let mut built = 0u32;
        for &me in &mesh_entities {
            if let Ok(m3d) = q_mesh.get(me) {
                if let Some(mesh) = meshes.get(&m3d.0) {
                    if let Some(col) = Collider::from_bevy_mesh(mesh, &shape) {
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

/// Recopie les solides de l'ARÈNE dans [`DecorObstacles::arena`].
///
/// # Le défaut que ce système corrige (2026-08-12)
///
/// La recherche de place libre ne voyait que le décor procédural. Les bâtiments
/// autorés, les murs de pièces et les remparts lui étaient **invisibles** — d'où des
/// ennemis nés à l'intérieur, définitivement bloqués.
///
/// `ArenaGeometry` publie tout, **au moment du plan**, ce que `spawn-clearance.md` §3
/// exige justement : interroger les entités déjà spawnées donne une liste incomplète et
/// un résultat qui dépend du timing — ce qui explique le « certains » du rapport.
///
/// Les tronçons (murs) deviennent des disques qui se chevauchent : une
/// sur-approximation assumée. Un ennemi repoussé un peu trop loin coûte un placement
/// médiocre ; un ennemi né dans un mur coûte un ennemi retiré du combat.
pub fn sys_sync_arena_solids(
    geometry: Option<Res<forgia_stage::ArenaGeometry>>,
    cfg: Res<forgia_navmesh::NavmeshBuild>,
    mut obstacles: ResMut<DecorObstacles>,
) {
    let Some(geo) = geometry else {
        if !obstacles.arena.is_empty() {
            obstacles.arena.clear();
        }
        return;
    };
    if !geo.is_changed() {
        return;
    }
    // `clear()` garde la capacité : une arène en repose quelques centaines.
    obstacles.arena.clear();

    // Même prédicat que le maillage de navigation — sinon on aurait une TROISIÈME
    // notion de « solide » dans le projet, et elles divergeraient toutes les trois.
    for d in &geo.discs {
        if forgia_navmesh::blocks_agent(d.h, cfg.step_height_m) {
            obstacles.arena.push((Vec2::new(d.x, d.z), d.r));
        }
    }
    for s in &geo.segs {
        if !forgia_navmesh::blocks_agent(s.h, cfg.step_height_m) {
            continue;
        }
        let a = Vec2::new(s.x0, s.z0);
        let b = Vec2::new(s.x1, s.z1);
        let r = s.half_thick_m.max(0.1);
        let len = a.distance(b);
        // Pas = le rayon : les disques se chevauchent, aucun trou entre eux.
        let n = (len / r).ceil().max(1.0) as u32;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            obstacles.arena.push((a.lerp(b, t), r));
        }
    }
}

/// Empêche un ennemi de RESTER apparu dans un solide : s'il chevauche une emprise,
/// on le pousse juste au bord (nudge radial minimal → ne casse pas le cover).
///
/// 2026-08-12 — lisait auparavant les entités `SolidDecorObstacle`, donc **le décor
/// procédural seul** : le filet de sécurité était aussi aveugle que le spawn qu'il
/// devait rattraper. Il lit maintenant [`DecorObstacles`], décor **et** arène.
pub fn sys_unstick_bots_from_decor(
    mut q_bots: Query<&mut Transform, With<ArenaBot>>,
    obstacles: Res<DecorObstacles>,
) {
    if obstacles.mesures() == 0 {
        return;
    }
    for mut tf in &mut q_bots {
        for (centre, rayon) in obstacles.solides() {
            let dx = tf.translation.x - centre.x;
            let dz = tf.translation.z - centre.y;
            let clear = rayon + BOT_FOOTPRINT_M;
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
    // Story-671 — la DA de la salle : `stage_id` courant → palette → props.
    stage_request: Option<Res<forgia_stage::StageLoadRequest>>,
    palettes: Option<Res<crate::decor_palettes::DecorPalettesConfig>>,
    // 2026-08-04 — le chapitre courant : c'est LUI qui porte la direction
    // artistique du Livre. `Option` parce que ce système tourne aussi là où la
    // notion de chapitre n'existe pas.
    chapter: Option<Res<crate::meta_shop::SelectedChapter>>,
    // 2026-08-04 — les portes du complexe, publiées par `forgia-stage`.
    doors: Option<Res<forgia_stage::rooms::RoomDoors>>,
    // Story-672 — position RÉELLE du joueur (spawné par forgia-player) pour
    // dégager son disque d'apparition, et carte des obstacles pour les ennemis.
    q_player: Query<&Transform, With<forgia_player::Player>>,
    mut obstacles: ResMut<DecorObstacles>,
    // Story-690 — la mesure commune de l'arène, définie par `forgia-stage`.
    // `Option` : ce système tourne aussi là où le plugin de stage est absent.
    mut geometry: Option<ResMut<forgia_stage::ArenaGeometry>>,
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

    // Story-671 — quelle DA porte cette salle ? Repli explicite : une salle sans
    // palette reconnue garde la DA historique, elle n'est jamais vide.
    let stage_id = stage_request
        .as_ref()
        .map(|r| r.stage_id.clone())
        .unwrap_or_default();
    // 2026-08-04 — c'est le CHAPITRE qui décide de l'habillage, la salle ne sert
    // plus que de repli. Un chapitre garde sa direction artistique sur ses quatre
    // arènes ; sans ça on retraversait quatre univers en dix rounds.
    let chapitre = chapter.map(|c| c.0).unwrap_or(1);
    let palette_id = palettes
        .as_ref()
        .map(|c| c.palette_id_for(chapitre, &stage_id).to_string())
        .unwrap_or_else(|| crate::decor_palettes::FALLBACK_PALETTE.to_string());

    // Planifie tout (RNG only, pas d'instanciation) → le drain spawne par budget.
    let player_pos = q_player
        .iter()
        .next()
        .map(|t| Vec2::new(t.translation.x, t.translation.z))
        .unwrap_or(Vec2::ZERO);
    // 2026-08-04 — le semis connaît enfin les portes. `Option` : un stage autoré
    // n'a pas de complexe procédural, donc pas de porte à protéger.
    let keepout = SpawnKeepout::around_player_and_doors(
        &cfg,
        player_pos,
        doors.map(|d| d.clone()).unwrap_or_default(),
    );
    let specs = plan_decor_set(&cfg, assets.for_palette(&palette_id), seed, &keepout);
    // Story-672 — publie les emprises SOLIDES avant toute instanciation : les
    // ennemis d'une vague apparaissent d'un coup, ils ne peuvent pas attendre que
    // la file de décor soit drainée.
    obstacles.discs = specs
        .iter()
        .filter(|s| s.is_solid())
        .map(|s| (s.ground_pos(), s.footprint_radius()))
        .collect();
    // Story-690 — le décor DÉPOSE sa géométrie dans la mesure commune. Il porte
    // l'essentiel du couvert de l'arène ; sans lui le capteur ne voyait que les
    // murs et concluait « aucun abri » sur des cartes qui en ont des dizaines.
    //
    // Le `Background` (hors-map) et le `Loose` (sans collider) sont exclus par
    // `is_solid()` : compter une falaise à 165 m comme un abri fausserait tout.
    if let Some(geometry) = geometry.as_mut() {
        geometry
            .discs
            .extend(specs.iter().filter(|s| s.is_solid()).map(|s| {
                let p = s.ground_pos();
                forgia_core::layout::SolidDisc {
                    x: p.x,
                    z: p.y,
                    r: s.footprint_radius(),
                    h: s.height_m(),
                }
            }));
    }
    let count = specs.len();
    queue.pending = specs;
    queue.cursor = 0;
    info!(
        "[decor] planned {count} GLB props — salle '{stage_id}' / DA '{palette_id}'          (biome={biome}), étalés à {}/frame",
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

fn pick<'a>(pool: &'a [DecorAsset], rng: &mut Xoshiro256StarStar) -> Option<&'a DecorAsset> {
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
            // Le SceneRoot enfant porte InheritedVisibility : le parent doit
            // participer à la hiérarchie de visibilité pour éviter B0004.
            Visibility::Visible,
        ))
        .id();
    // Enfant visuel (scale appliqué par calibration ; parent reste scale 1).
    commands.spawn((
        ChildOf(parent),
        Name::new("DecorVisual"),
        SceneRoot(handle.clone()),
        Transform::IDENTITY,
        NeedsDecorCalibrate {
            target_m,
            user_scale,
        },
    ));
    // Collider mesh-fidèle : ConvexHull construit depuis le mesh chargé par
    // `sys_decor_build_hull_colliders` (suit la silhouette ; fiable). Fallback
    // cylindre si le mesh ne produit pas de hull. Marqueur sur le parent.
    commands.entity(parent).insert((
        NeedsHullCollider {
            fallback_target_m: target_m,
            fallback_radius_factor: col_radius_factor,
            precise: true, // gros prop concave (statue, colonne) = TriMesh exact
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
/// Semis à BRUIT BLEU dans un anneau, avec un compte DÉRIVÉ et un plafond de
/// budget qui, quand il mord, le DIT (story-674).
///
/// Avant : un littéral (`perimeter_count = 34`) et des angles tirés au hasard —
/// donc des amas et des trous, et un compte qui ne savait rien de la taille de la
/// salle. Après : `covers_expected(aire, espacement)` donne l'attendu, Bridson
/// donne des positions à espacement minimal garanti.
///
/// Le plafond ne TRONQUE pas la liste (l'ordre d'insertion de Bridson est une
/// frontière qui progresse : prendre le préfixe remplirait un secteur et laisserait
/// le reste nu). Il **sous-échantillonne à pas régulier**, ce qui garde le semis
/// réparti sur toute l'aire.
fn plan_ring_points(
    r_min: f32,
    r_max: f32,
    spacing: f32,
    seed: u64,
    cap: usize,
    what: &str,
) -> Vec<(f32, f32)> {
    if cap == 0 {
        return Vec::new();
    }
    let pts = poisson_disk_annulus(r_min, r_max, spacing, seed);
    let derived = pts.len();
    let lo = r_min.max(0.0);
    let expected = covers_expected(disc_area(r_max) - disc_area(lo), spacing);
    if derived <= cap {
        debug!(
            "[decor] {what} : {derived} positions à {spacing:.1} m              (aire/espacement² = {expected:.0}, plafond {cap})"
        );
        return pts;
    }
    warn!(
        "[decor] {what} : {derived} positions dérivées à {spacing:.1} m          (aire/espacement² = {expected:.0}) mais plafond decor_max_props = {cap} →          {} props NON posés. Salle sous-remplie volontairement (budget de frame).",
        derived - cap
    );
    let stride = derived as f32 / cap as f32;
    (0..cap)
        .map(|i| pts[((i as f32 * stride) as usize).min(derived - 1)])
        .collect()
}

fn plan_decor_set(
    cfg: &RogueliteDecorConfig,
    assets: &DecorPaletteAssets,
    seed: u64,
    keepout: &SpawnKeepout,
) -> Vec<DecorSpec> {
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
                handle: handle.scene.clone(),
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
                handle: handle.scene.clone(),
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
                handle: handle.scene.clone(),
                name: "Decor_ForgeBrazier",
                pos,
                yaw,
                target_m: cfg.target_brazier,
                footprint_m: handle.footprint_at(cfg.target_brazier),
                height_m: handle.height_at(cfg.target_brazier),
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
                handle: handle.scene.clone(),
                name: "Decor_ForgeMonument",
                pos,
                yaw,
                target_m: cfg.forge_monument_target,
                footprint_m: handle.footprint_at(cfg.forge_monument_target),
                height_m: handle.height_at(cfg.forge_monument_target),
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
        let radius = lerp(
            cfg.building_radius_min,
            cfg.building_radius_max,
            rng01(&mut rng),
        );
        let pos = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
        let yaw = angle + std::f32::consts::PI; // face au centre
        let target = cfg.target_building * (0.85 + rng01(&mut rng) * 0.4);
        if let Some(handle) = pick(&assets.buildings, &mut rng) {
            specs.push(DecorSpec::Perimeter {
                handle: handle.scene.clone(),
                name: "Decor_Building",
                pos,
                yaw,
                target_m: target,
                footprint_m: handle.footprint_at(target),
                height_m: handle.height_at(target),
                user_scale: 1.0,
                brazier: false,
                col_radius_factor: 0.32,
            });
        }
    }

    // ── LE COUVERT DE L'AIRE DE COMBAT (story-688) ────────────────────────────
    //
    // Tous les props solides vivaient dans l'anneau 42→74 m : là où l'on se bat,
    // il n'y avait aucun abri. C'était un défaut de COMPOSITION — story-674 a
    // multiplié la densité par 4,7 sans le corriger, parce qu'ajouter des props
    // ne sert à rien s'ils ne sont pas au bon endroit.
    //
    // Le rôle se DÉRIVE de la hauteur EN JEU : ≥ 1,80 m casse la ligne de vue,
    // donc c'est du couvert (`map-design-patterns.md` §11). Un prop de la bande
    // 1,2-1,7 m masque le corps sans masquer la vue — il ne sert à rien, et il
    // est écarté ici plutôt que posé « au cas où ».
    let cover_pool: Vec<&DecorAsset> = assets
        .big
        .iter()
        .chain(assets.landmarks.iter())
        .filter(|a| a.height_at(cfg.target_big) >= SIGHT_BREAK_H_M)
        .collect();
    let mut covers_placed = 0usize;
    if !cover_pool.is_empty() && cfg.cover_radius_min_m < cfg.ring_radius_min {
        // Le couvert passe EN PREMIER sur le budget : c'est du gameplay, le
        // périmètre et le semis sont de l'habillage.
        let pts = plan_ring_points(
            cfg.cover_radius_min_m,
            cfg.ring_radius_min,
            cfg.cover_spacing_m,
            seed ^ 0xC0FE_5EED_1234_ABCD,
            cfg.max_props as usize,
            "couvert",
        );
        for (px, pz) in pts {
            let idx = (rng01(&mut rng) * cover_pool.len() as f32) as usize;
            let Some(handle) = cover_pool.get(idx.min(cover_pool.len() - 1)) else {
                break;
            };
            specs.push(DecorSpec::Perimeter {
                handle: handle.scene.clone(),
                name: "Decor_Cover",
                pos: Vec3::new(px, 0.0, pz),
                yaw: rng01(&mut rng) * TAU,
                target_m: cfg.target_big,
                footprint_m: handle.footprint_at(cfg.target_big),
                height_m: handle.height_at(cfg.target_big),
                user_scale: 0.9 + rng01(&mut rng) * 0.2,
                brazier: false,
                col_radius_factor: 0.34,
            });
            covers_placed += 1;
        }
    }
    if covers_placed == 0 {
        warn!(
            "[decor] AUCUN couvert dans l'aire de combat — pool de props ≥ {:.2} m vide              (calibrés à {:.1} m) ou rayons incohérents ({:.0} ≥ {:.0} m).              Les salles seront des stands de tir.",
            SIGHT_BREAK_H_M, cfg.target_big, cfg.cover_radius_min_m, cfg.ring_radius_min
        );
    }

    // ── Anneau périmétrique ───────────────────────────────────────────────────
    // Story-674 — c'est la couche qui porte le COUVERT : son compte se dérive de
    // l'aire de l'anneau (l'ancien littéral 34 ne savait rien de la taille de la
    // salle), ses positions viennent d'un semis à espacement minimal garanti.
    // Le périmètre passe en premier sur le budget : c'est du gameplay, pas de
    // l'habillage.
    let ring_pts = plan_ring_points(
        cfg.ring_radius_min,
        cfg.ring_radius_max,
        cfg.perimeter_spacing_m,
        seed ^ 0x9E37_79B9_7F4A_7C15,
        (cfg.max_props as usize).saturating_sub(covers_placed),
        "périmètre",
    );
    let n_ring = ring_pts.len();
    let landmark_n = cfg.landmark_count.min(n_ring as u32);
    let landmark_step = (n_ring as u32)
        .checked_div(landmark_n)
        .map_or(u32::MAX, |s| s.max(1));
    let mut landmarks_placed = 0u32;

    for (i, (px, pz)) in ring_pts.iter().enumerate() {
        let i = i as u32;
        let pos = Vec3::new(*px, 0.0, *pz);
        let yaw = rng01(&mut rng) * TAU;

        let is_landmark = landmarks_placed < landmark_n && i.is_multiple_of(landmark_step);
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
            handle: handle.scene.clone(),
            name,
            pos,
            yaw,
            target_m: target,
            footprint_m: handle.footprint_at(target),
            height_m: handle.height_at(target),
            user_scale: us,
            brazier,
            col_radius_factor: crf,
        });
    }

    // ── Petits props dispersés au sol (sans collider) ─────────────────────────
    // Story-674 — l'ancien tirage polaire uniforme produisait des amas et des
    // clairières : c'est ce qui se lisait comme « la salle est vide » alors que le
    // compte était le bon. Le bruit bleu garantit l'espacement, donc l'occupation.
    // Le semis prend ce que le périmètre a laissé du budget.
    let scatter_budget = (cfg.max_props as usize).saturating_sub(n_ring + covers_placed);
    let scatter_pts = plan_ring_points(
        cfg.scatter_radius_min,
        cfg.scatter_radius_max,
        cfg.scatter_spacing_m,
        seed ^ 0xC2B2_AE3D_27D4_EB4F,
        scatter_budget,
        "semis",
    );
    for (px, pz) in scatter_pts {
        let Some(handle) = pick(&assets.scatter, &mut rng) else {
            break;
        };
        let pos = Vec3::new(px, 0.0, pz);
        let yaw = rng01(&mut rng) * TAU;
        let us = 0.75 + rng01(&mut rng) * 0.5;

        specs.push(DecorSpec::Loose {
            handle: handle.scene.clone(),
            name: "Decor_Scatter",
            pos,
            yaw,
            user_scale: us,
            target_m: cfg.target_scatter,
            footprint_m: handle.footprint_at(cfg.target_scatter),
        });
    }

    // ── Salles en L (coin + 2 bras de mur KayKit, 1 collider cuboid par bras) ─
    for r in 0..cfg.room_count {
        let angle = (r as f32 / cfg.room_count.max(1) as f32) * TAU + (rng01(&mut rng) - 0.5) * 0.8;
        let radius = lerp(cfg.room_radius_min, cfg.room_radius_max, rng01(&mut rng));
        let pos = Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin());
        let yaw0 = rng01(&mut rng) * TAU;
        plan_wall_room(
            &mut specs,
            assets,
            &mut rng,
            pos,
            yaw0,
            cfg.room_arm_segments,
        );
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
            handle: handle.scene.clone(),
            name: "Decor_Rubble",
            pos,
            yaw,
            user_scale: us,
            target_m: cfg.target_rubble,
            footprint_m: handle.footprint_at(cfg.target_rubble),
        });
    }

    // Story-672 — INVARIANT DE SORTIE : aucun prop dans une zone d'apparition.
    // Placé ICI et pas dans chaque générateur : un seul point de vérité, et tout
    // futur générateur de props en hérite sans rien faire.
    let before = specs.len();
    specs.retain(|spec| {
        spec.is_background() || !keepout.blocks(spec.ground_pos(), spec.footprint_radius())
    });
    let rejected = before - specs.len();
    if rejected > 0 {
        debug!("[decor] {rejected}/{before} props écartés (zone d'apparition)");
    }
    specs
}

/// Planifie une "salle" en L : coin + 2 bras perpendiculaires de murs KayKit.
/// Layout OUVERT (pas d'enceinte fermée) → les bots peuvent contourner. Pousse
/// des `WallPiece` (chacune = 1 instanciation, drainable) ; consomme le RNG à
/// l'identique de l'ancien `spawn_wall_room` → layout inchangé.
fn plan_wall_room(
    specs: &mut Vec<DecorSpec>,
    assets: &DecorPaletteAssets,
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
            handle: corner.scene.clone(),
            // Les murs sont posés à l'échelle NATIVE (pas de calibration) : leur
            // emprise mesurée s'applique telle quelle. Repli sur l'ancien rayon
            // d'obstacle si l'asset n'est pas au registre.
            footprint_m: if corner.native_footprint_m > 0.0 {
                corner.native_footprint_m
            } else {
                WALL_SEG_W * 0.7
            },
            // Posé à l'échelle NATIVE : sa hauteur mesurée s'applique telle quelle.
            height_m: corner.native_height_m,
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
    assets: &DecorPaletteAssets,
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
            handle: handle.scene.clone(),
            footprint_m: if handle.native_footprint_m > 0.0 {
                handle.native_footprint_m
            } else {
                WALL_SEG_W * 0.6
            },
            height_m: handle.native_height_m,
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
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

/// Pur — testable.
pub fn severity_for_decor(total: u32) -> (&'static str, &'static str) {
    if total == 0 {
        (
            "info",
            "0 prop décor (hors arène ou decor_enabled=0). Read roguelite_decor.toml.",
        )
    } else {
        (
            "ok",
            "Décor posé. Ajuste rayons/counts/targets via roguelite_decor.toml (Shift+F12).",
        )
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
id = "decor_perimeter_spacing_m"
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
        assert_eq!(c.perimeter_spacing_m, 40.0); // clamp
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

    /// Story-671 — remplace l'ex-`prop_paths_valid` (qui vérifiait seulement que
    /// les chemins étaient des `.glb` du pack Inferno, ce qui n'a plus de sens avec
    /// 4 DA de packs différents).
    ///
    /// Ce test est bien plus fort : **chaque chemin déclaré dans le génome doit
    /// exister sur le disque**. Un chemin mort ne fait pas planter le jeu — Bevy
    /// loggue une erreur d'asset et le prop est simplement absent — donc sans ce
    /// test une DA peut se vider en silence à la faveur d'un renommage de fichier.
    #[test]
    fn every_declared_prop_path_exists_on_disk() {
        use crate::decor_palettes::{DecorPalettesConfig, GENOME_PATH};
        // `cargo test` tourne avec le CWD sur la crate, le jeu sur la racine.
        let (content, assets_root) = std::fs::read_to_string(GENOME_PATH)
            .map(|c| (c, std::path::PathBuf::from("assets")))
            .or_else(|_| {
                std::fs::read_to_string(format!("../../{GENOME_PATH}"))
                    .map(|c| (c, std::path::PathBuf::from("../../assets")))
            })
            .expect("roguelite_palettes.toml introuvable depuis la crate ET depuis la racine");

        let cfg = DecorPalettesConfig::parse_toml(&content);
        let mut missing: Vec<String> = Vec::new();
        let mut checked = 0usize;
        for path in cfg.all_paths() {
            checked += 1;
            if !assets_root.join(path).is_file() {
                missing.push(path.to_string());
            }
        }
        assert!(checked > 0, "aucun chemin mesuré = test aveugle");
        assert!(
            missing.is_empty(),
            "{} chemins déclarés introuvables sur {checked} : {missing:#?}",
            missing.len()
        );
    }

    /// Story-672 — L'INVARIANT : aucun prop dans une zone d'apparition.
    #[test]
    fn no_prop_lands_in_a_spawn_zone() {
        // Assets factices AVEC mesures : 2 m d'emprise native, 4 m de dimension
        // max — un prop plausible. Le plan ne charge rien, il ne fait que du RNG.
        let h = || {
            vec![DecorAsset {
                scene: Handle::<Scene>::default(),
                native_footprint_m: 2.0,
                native_height_m: 3.0,
                native_max_dim_m: 4.0,
            }]
        };
        let assets = DecorPaletteAssets {
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
        let ko = SpawnKeepout::around_player(&cfg, Vec2::ZERO);
        // Plusieurs graines : le semis est aléatoire, l'invariant ne l'est pas.
        for seed in 0..40u64 {
            let specs = plan_decor_set(&cfg, &assets, seed.wrapping_mul(0x9E37_79B9), &ko);
            assert!(!specs.is_empty(), "graine {seed} : le filtre a tout mangé");
            for spec in &specs {
                if spec.is_background() {
                    continue; // hors-map, sans collider
                }
                assert!(
                    !ko.blocks(spec.ground_pos(), spec.footprint_radius()),
                    "graine {seed} : un prop atterrit dans une zone d'apparition                      (pos {:?}, rayon {:.2}, solide {})",
                    spec.ground_pos(),
                    spec.footprint_radius(),
                    spec.is_solid()
                );
            }
        }
    }

    #[test]
    fn the_player_disc_is_cleared_wherever_he_actually_stands() {
        let cfg = RogueliteDecorConfig::default();
        // Le joueur est spawné par forgia-player : sa position n'est PAS forcément
        // l'origine. Supposer l'origine protégeait le mauvais endroit — c'est le
        // « j'ai respawn en plein sur un asset » du 2026-07-31.
        let p = Vec2::new(17.0, -4.0);
        let ko = SpawnKeepout::around_player(&cfg, p);
        assert!(ko.blocks(p, 0.0), "le point exact du joueur est interdit");
        assert!(ko.blocks(p + Vec2::new(cfg.keepout_player_m - 0.5, 0.0), 0.0));
        assert!(
            !ko.blocks(Vec2::ZERO, 0.0),
            "l'origine n'est plus protégée si le joueur est ailleurs"
        );
        // Et le rayon du prop compte.
        let edge = p + Vec2::new(cfg.keepout_player_m + 1.0, 0.0);
        assert!(!ko.blocks(edge, 0.0));
        assert!(
            ko.blocks(edge, 3.0),
            "un gros prop tangent doit être refusé"
        );
    }

    /// Story-672 v2 — c'est le SPAWN qui cède, plus le décor. On vérifie que la
    /// recherche d'angle trouve une place libre, et qu'elle en trouve TOUJOURS une.
    #[test]
    fn an_enemy_never_spawns_inside_a_solid_prop() {
        // Un anneau de 25 m encombré d'un gros prop à l'angle 0.
        let obstacles = DecorObstacles {
            discs: vec![(Vec2::new(25.0, 0.0), 6.0)],
            ..Default::default()
        };
        let body = 0.5;
        let a = obstacles.clear_angle_on_ring(25.0, 0.0, body, 24);
        let p = Vec2::new(25.0 * a.cos(), 25.0 * a.sin());
        assert!(
            obstacles.is_clear(p, body),
            "l'angle retenu doit être libre (retenu {a:.2} rad)"
        );
    }

    #[test]
    fn un_ennemi_ne_nait_pas_dans_un_batiment_de_l_arene() {
        // LE bug rapporte en jeu le 2026-08-12 : « certains ennemis spawnent dans un
        // batiment et restent bloques dedans, je les vois a travers ».
        //
        // Avant le correctif, ce test passait a cote : la recherche de place libre
        // n'iterait que `discs` (le decor procedural), et un batiment autore vit dans
        // `arena`. Le decor est ici VIDE — seul un batiment de 8 m barre l'anneau.
        let obstacles = DecorObstacles {
            discs: Vec::new(),
            arena: vec![(Vec2::new(25.0, 0.0), 8.0)],
        };
        assert_eq!(obstacles.mesures(), 1, "un solide est bien pris en compte");

        let body = 0.5;
        let a = obstacles.clear_angle_on_ring(25.0, 0.0, body, 24);
        let p = Vec2::new(25.0 * a.cos(), 25.0 * a.sin());
        assert!(
            obstacles.is_clear(p, body),
            "l'angle retenu tombe dans le batiment (retenu {a:.2} rad) — \
             c'est exactement l'ennemi coince dedans"
        );
    }

    #[test]
    fn zero_solide_mesure_n_est_pas_un_terrain_degage() {
        // Un spawn qui ne connait AUCUN solide se croit libre partout. C'est l'etat
        // dans lequel le jeu a vecu : `mesures()` le rend visible au lieu de le taire.
        let vide = DecorObstacles::default();
        assert_eq!(vide.mesures(), 0);
        assert!(vide.is_clear(Vec2::new(25.0, 0.0), 0.5), "rien ne bloque, faute de rien savoir");
    }

    #[test]
    fn a_fully_blocked_ring_still_yields_the_least_bad_angle() {
        // Anneau entièrement ceinturé : aucune place libre.
        let mut discs = Vec::new();
        for i in 0..24 {
            let a = std::f32::consts::TAU * i as f32 / 24.0;
            discs.push((Vec2::new(25.0 * a.cos(), 25.0 * a.sin()), 5.0));
        }
        // …sauf un trou volontairement plus dégagé.
        discs[7].1 = 0.2;
        let obstacles = DecorObstacles {
            discs,
            ..Default::default()
        };
        let a = obstacles.clear_angle_on_ring(25.0, 0.0, 0.5, 24);
        let p = Vec2::new(25.0 * a.cos(), 25.0 * a.sin());
        let best = obstacles.clearance(p, 0.5);
        // On ne peut pas exiger « libre », mais on exige « le moins mauvais ».
        for i in 0..24 {
            let t = std::f32::consts::TAU * i as f32 / 24.0;
            let q = Vec2::new(25.0 * t.cos(), 25.0 * t.sin());
            assert!(
                obstacles.clearance(q, 0.5) <= best + 1.0e-3,
                "un meilleur angle existait"
            );
        }
    }

    /// Le décor ne doit PLUS s'interdire la moitié de l'arène : c'est ce qui
    /// vidait les salles (54 % du rayon utile mesuré le 2026-07-31).
    #[test]
    fn the_decor_keeps_its_density_away_from_the_player() {
        // Assets factices AVEC mesures : 2 m d'emprise native, 4 m de dimension
        // max — un prop plausible. Le plan ne charge rien, il ne fait que du RNG.
        let h = || {
            vec![DecorAsset {
                scene: Handle::<Scene>::default(),
                native_footprint_m: 2.0,
                native_height_m: 3.0,
                native_max_dim_m: 4.0,
            }]
        };
        let assets = DecorPaletteAssets {
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
        let ko = SpawnKeepout::around_player(&cfg, Vec2::ZERO);
        let with = plan_decor_set(&cfg, &assets, 0xC0FFEE, &ko);
        let without = plan_decor_set(&cfg, &assets, 0xC0FFEE, &SpawnKeepout::default());
        let kept = with.len() as f32 / without.len().max(1) as f32;
        assert!(
            kept > 0.9,
            "le filtre ne doit écarter qu'une poignée de props près du joueur,              pas vider la salle (gardé {:.0} %)",
            kept * 100.0
        );
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
        // Assets factices AVEC mesures : 2 m d'emprise native, 4 m de dimension
        // max — un prop plausible. Le plan ne charge rien, il ne fait que du RNG.
        let h = || {
            vec![DecorAsset {
                scene: Handle::<Scene>::default(),
                native_footprint_m: 2.0,
                native_height_m: 3.0,
                native_max_dim_m: 4.0,
            }]
        };
        let assets = DecorPaletteAssets {
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
        let ko = SpawnKeepout::default();
        let a = plan_decor_set(&cfg, &assets, 0xABCD, &ko);
        let b = plan_decor_set(&cfg, &assets, 0xABCD, &ko);
        // Même seed → même nombre de props (RNG préservé).
        assert_eq!(a.len(), b.len());
        // Le décor par défaut produit beaucoup de props (sinon pas de freeze à étaler).
        assert!(a.len() > 100, "plan trop petit: {}", a.len());
        // Le budget par défaut est bien < total → drainage en plusieurs frames.
        assert!((cfg.spawn_budget_per_frame as usize) < a.len());
    }
}

#[cfg(test)]
mod layout_derivation_tests {
    use super::*;

    /// Le semis n'est plus un littéral : il DOIT suivre la taille de la salle.
    /// C'est tout l'objet de la story-674 — 34 props dans une salle de 40 m et
    /// dans une salle de 90 m, ce n'était pas le même remplissage.
    #[test]
    fn the_count_follows_the_room_size() {
        let small = plan_ring_points(20.0, 35.0, 9.0, 1, 10_000, "t");
        let big = plan_ring_points(42.0, 74.0, 9.0, 1, 10_000, "t");
        assert!(
            big.len() > small.len() * 2,
            "petite salle {} props, grande {} — la dérivation ne suit pas",
            small.len(),
            big.len()
        );
    }

    /// Espacement minimal garanti : c'est ce que le tirage polaire ne donnait pas
    /// (amas + clairières, lus en jeu comme « la salle est vide »).
    #[test]
    fn no_two_props_are_closer_than_the_spacing() {
        for seed in [1u64, 7, 42, 1337, 99_999] {
            let pts = plan_ring_points(42.0, 74.0, 9.0, seed, 10_000, "t");
            assert!(!pts.is_empty(), "graine {seed} : anneau vide");
            for (i, a) in pts.iter().enumerate() {
                for b in pts.iter().skip(i + 1) {
                    let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                    assert!(d >= 9.0 - 1e-2, "graine {seed} : deux props à {d:.2} m");
                }
            }
        }
    }

    /// Le plafond SOUS-ÉCHANTILLONNE, il ne tronque pas : l'ordre d'insertion de
    /// Bridson est une frontière qui progresse, un préfixe remplirait un secteur
    /// et laisserait le reste nu.
    #[test]
    fn the_cap_keeps_the_spread_instead_of_filling_one_sector() {
        let full = plan_ring_points(0.0, 70.0, 5.0, 3, 10_000, "t");
        let capped = plan_ring_points(0.0, 70.0, 5.0, 3, 40, "t");
        assert_eq!(capped.len(), 40);
        assert!(full.len() > 40, "le plafond ne mord pas, test inutile");
        // Les 4 quadrants doivent rester habités.
        let quad = |f: &dyn Fn(&(f32, f32)) -> bool| capped.iter().filter(|p| f(p)).count();
        for (name, f) in [
            (
                "NE",
                &(|p: &(f32, f32)| p.0 >= 0.0 && p.1 >= 0.0) as &dyn Fn(&(f32, f32)) -> bool,
            ),
            ("NO", &(|p: &(f32, f32)| p.0 < 0.0 && p.1 >= 0.0)),
            ("SE", &(|p: &(f32, f32)| p.0 >= 0.0 && p.1 < 0.0)),
            ("SO", &(|p: &(f32, f32)| p.0 < 0.0 && p.1 < 0.0)),
        ] {
            assert!(quad(f) >= 4, "quadrant {name} quasi vide : {}", quad(f));
        }
    }

    /// Un plafond à 0 ne doit pas paniquer — et une salle n'est jamais vidée par
    /// le mécanisme lui-même (la leçon de story-672).
    #[test]
    fn degenerate_budgets_are_safe() {
        assert!(plan_ring_points(42.0, 74.0, 9.0, 1, 0, "t").is_empty());
        assert_eq!(plan_ring_points(42.0, 74.0, 9.0, 1, 1, "t").len(), 1);
    }

    /// CE QUE LA LIVRAISON CHANGE, en chiffres — pour ne pas annoncer « plus
    /// dense » sans le mesurer.
    #[test]
    fn the_delivered_defaults_are_measured_not_claimed() {
        let cfg = RogueliteDecorConfig::default();
        let per = plan_ring_points(
            cfg.ring_radius_min,
            cfg.ring_radius_max,
            cfg.perimeter_spacing_m,
            0xF06,
            cfg.max_props as usize,
            "périmètre",
        );
        let sc = plan_ring_points(
            cfg.scatter_radius_min,
            cfg.scatter_radius_max,
            cfg.scatter_spacing_m,
            0xF07,
            (cfg.max_props as usize).saturating_sub(per.len()),
            "semis",
        );
        // Avant story-674 : 34 périmètre + 55 semis = 89 props posés.
        assert!(
            per.len() + sc.len() > 89 * 3,
            "périmètre {} + semis {} — la densité n'a pas décollé",
            per.len(),
            sc.len()
        );
        assert!(
            per.len() + sc.len() <= cfg.max_props as usize,
            "le plafond de budget de frame doit tenir"
        );
        println!(
            "[story-674] périmètre {} props (avant 34), semis {} (avant 55), total {} (plafond {})",
            per.len(),
            sc.len(),
            per.len() + sc.len(),
            cfg.max_props
        );
    }
}

#[cfg(test)]
mod cover_composition_tests {
    use super::*;

    /// Story-688 — **le couvert doit être là où l'on se bat.**
    ///
    /// Tous les props solides vivaient dans l'anneau 42→74 m ; l'aire de combat
    /// n'avait aucun abri. Story-674 a multiplié la densité par 4,7 sans le
    /// corriger : ajouter des props ne sert à rien s'ils ne sont pas au bon
    /// endroit. C'est de la COMPOSITION, pas de la densité.
    #[test]
    fn cover_is_placed_inside_the_combat_area_not_only_on_the_rim() {
        let cfg = RogueliteDecorConfig::default();
        assert!(
            cfg.cover_radius_min_m < cfg.ring_radius_min,
            "l'anneau de couvert doit être STRICTEMENT à l'intérieur du périmètre"
        );
        let pts = plan_ring_points(
            cfg.cover_radius_min_m,
            cfg.ring_radius_min,
            cfg.cover_spacing_m,
            7,
            cfg.max_props as usize,
            "t",
        );
        assert!(!pts.is_empty(), "aucun abri planifié dans l'aire de combat");
        for (x, z) in &pts {
            let d = (x * x + z * z).sqrt();
            assert!(
                d < cfg.ring_radius_min + 1e-3,
                "un abri à {d:.1} m est sur le périmètre, pas dans l'aire de combat"
            );
        }
        println!(
            "[story-688] {} abris entre {:.0} et {:.0} m (espacement {:.0} m)",
            pts.len(),
            cfg.cover_radius_min_m,
            cfg.ring_radius_min,
            cfg.cover_spacing_m
        );
    }

    /// L'espacement doit rester dans la bande SOURCÉE 3-10 m (Watch Dogs,
    /// Gears of War). Plus serré, c'est une forêt de piliers ; plus lâche, le
    /// repli n'existe plus.
    #[test]
    fn the_cover_spacing_stays_in_the_sourced_band() {
        let c = RogueliteDecorConfig::default();
        assert!((3.0..=10.0).contains(&c.cover_spacing_m));
        let hostile = RogueliteDecorConfig::parse_toml(
            "[[genes]]
id = \"decor_cover_spacing_m\"
default = 99.0
",
        );
        assert!(hostile.cover_spacing_m <= 10.0, "borne haute non appliquée");
    }

    /// **Le rôle se dérive de la hauteur EN JEU, pas de la native.**
    ///
    /// Le kit hexagon est en miniatures : un bâtiment fait 0,93 m nativement et
    /// le décor le recalibre à `target_big` = 7 m. Filtrer sur le natif
    /// conclurait « ces cartes n'ont aucun couvert » — c'est exactement l'erreur
    /// des 1,92 m de story-672, une taille native prise pour une taille de jeu.
    #[test]
    fn a_miniature_prop_still_counts_as_cover_once_calibrated() {
        let mini = DecorAsset {
            scene: Handle::<Scene>::default(),
            native_footprint_m: 0.5,
            native_max_dim_m: 0.93,
            native_height_m: 0.93,
        };
        assert!(
            mini.height_at(0.0) < SIGHT_BREAK_H_M,
            "en natif, la miniature ne casse pas la vue"
        );
        assert!(
            mini.height_at(7.0) >= SIGHT_BREAK_H_M,
            "calibrée à 7 m elle DOIT compter comme couvert ({:.2} m)",
            mini.height_at(7.0)
        );
    }

    /// Un prop de la bande 1,2-1,7 m masque le corps sans masquer la vue : il ne
    /// doit JAMAIS être retenu comme abri (`map-design-patterns.md` §11).
    #[test]
    fn the_useless_height_band_is_never_taken_as_cover() {
        let useless = DecorAsset {
            scene: Handle::<Scene>::default(),
            native_footprint_m: 0.5,
            native_max_dim_m: 2.0,
            native_height_m: 1.5,
        };
        // Calibré pour rester dans la bande inutile.
        assert!(useless.height_at(2.0) < SIGHT_BREAK_H_M);
    }
}
