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
    // Story-672 — zones interdites au décor (voir `SpawnKeepout`).
    pub keepout_player_m: f32,
    pub keepout_spawn_margin_m: f32,
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
            keepout_player_m: 8.0,
            keepout_spawn_margin_m: 3.5,
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
#[derive(Default, Clone)]
pub struct DecorPaletteAssets {
    pub landmarks: Vec<Handle<Scene>>,
    pub big: Vec<Handle<Scene>>,
    pub braziers: Vec<Handle<Scene>>,
    pub scatter: Vec<Handle<Scene>>,
    pub walls: Vec<Handle<Scene>>,
    pub wall_corner: Vec<Handle<Scene>>,
    pub rubble: Vec<Handle<Scene>>,
    pub buildings: Vec<Handle<Scene>>,
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
            // `col_radius_factor` RÉTRÉCIT le collider pour le feel de tir — ce
            // n'est PAS l'emprise au sol. L'utiliser ici sous-estimait un bâtiment
            // de 12 m à 1,92 m de rayon (3× trop petit) : il passait le test et le
            // mob naissait dedans. Mesuré sur le retour du 2026-07-31.
            DecorSpec::Perimeter {
                target_m,
                user_scale,
                ..
            } => 0.5 * target_m * user_scale.max(0.01),
            DecorSpec::Loose {
                target_m,
                user_scale,
                ..
            } => 0.5 * target_m * user_scale.max(0.01),
            DecorSpec::WallPiece {
                obstacle_radius, ..
            } => *obstacle_radius,
        }
    }

    /// Le fond n'est jamais filtré : il est hors de l'enceinte par construction.
    fn is_background(&self) -> bool {
        matches!(self, DecorSpec::Background { .. })
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
    /// (centre au sol, rayon d'emprise) de chaque prop SOLIDE.
    pub discs: Vec<(Vec2, f32)>,
}

impl DecorObstacles {
    /// PUR — cette position est-elle libre pour un corps de rayon `body_radius` ?
    pub fn is_clear(&self, pos: Vec2, body_radius: f32) -> bool {
        let r = body_radius.max(0.0);
        !self
            .discs
            .iter()
            .any(|(c, rad)| pos.distance(*c) < rad + r)
    }

    /// Distance au bord de l'obstacle le plus proche (négatif = à l'intérieur).
    /// Sert de score pour choisir « le moins mauvais » quand tout est encombré.
    pub fn clearance(&self, pos: Vec2, body_radius: f32) -> f32 {
        self.discs
            .iter()
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
        let n = tries.max(1);
        let step = std::f32::consts::TAU / n as f32;
        let mut best = (wanted, f32::NEG_INFINITY);
        for i in 0..n {
            let a = wanted + step * i as f32;
            let p = Vec2::new(radius * a.cos(), radius * a.sin());
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
        }
    }

    /// PUR — ce prop atterrit-il sur le point d'apparition du joueur ?
    ///
    /// On refuse TOUT autour de lui, collider ou pas : c'est là qu'il ouvre les
    /// yeux, et apparaître le nez dans un tonneau même traversable est exactement
    /// le symptôme rapporté. Le rayon du prop entre dans le test.
    pub fn blocks(&self, pos: Vec2, prop_radius: f32) -> bool {
        pos.distance(self.player.0) < self.player.1 + prop_radius.max(0.0)
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
pub fn sys_load_decor_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    palettes: Option<Res<crate::decor_palettes::DecorPalettesConfig>>,
) {
    let load = |paths: &[&str]| -> Vec<Handle<Scene>> {
        paths
            .iter()
            .filter(|p| !p.is_empty())
            .map(|p| asset_server.load(GltfAssetLabel::Scene(0).from_asset(p.to_string())))
            .collect()
    };
    let load_owned = |paths: &[String]| -> Vec<Handle<Scene>> {
        paths
            .iter()
            .map(|p| asset_server.load(GltfAssetLabel::Scene(0).from_asset(p.clone())))
            .collect()
    };
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
    // Story-671 — la DA de la salle : `stage_id` courant → palette → props.
    stage_request: Option<Res<forgia_stage::StageLoadRequest>>,
    palettes: Option<Res<crate::decor_palettes::DecorPalettesConfig>>,
    // Story-672 — position RÉELLE du joueur (spawné par forgia-player) pour
    // dégager son disque d'apparition, et carte des obstacles pour les ennemis.
    q_player: Query<&Transform, With<forgia_player::Player>>,
    mut obstacles: ResMut<DecorObstacles>,
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
    let palette_id = palettes
        .as_ref()
        .map(|c| c.palette_id_for_stage(&stage_id).to_string())
        .unwrap_or_else(|| crate::decor_palettes::FALLBACK_PALETTE.to_string());

    // Planifie tout (RNG only, pas d'instanciation) → le drain spawne par budget.
    let player_pos = q_player
        .iter()
        .next()
        .map(|t| Vec2::new(t.translation.x, t.translation.z))
        .unwrap_or(Vec2::ZERO);
    let keepout = SpawnKeepout::around_player(&cfg, player_pos);
    let specs = plan_decor_set(&cfg, assets.for_palette(&palette_id), seed, &keepout);
    // Story-672 — publie les emprises SOLIDES avant toute instanciation : les
    // ennemis d'une vague apparaissent d'un coup, ils ne peuvent pas attendre que
    // la file de décor soit drainée.
    obstacles.discs = specs
        .iter()
        .filter(|s| s.is_solid())
        .map(|s| (s.ground_pos(), s.footprint_radius()))
        .collect();
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
    let landmark_step = cfg
        .perimeter_count
        .checked_div(landmark_n)
        .map_or(u32::MAX, |s| s.max(1));
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
            handle: handle.clone(),
            name: "Decor_Rubble",
            pos,
            yaw,
            user_scale: us,
            target_m: cfg.target_rubble,
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
        let h = || vec![Handle::<Scene>::default()];
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
        assert!(ko.blocks(edge, 3.0), "un gros prop tangent doit être refusé");
    }

    /// Story-672 v2 — c'est le SPAWN qui cède, plus le décor. On vérifie que la
    /// recherche d'angle trouve une place libre, et qu'elle en trouve TOUJOURS une.
    #[test]
    fn an_enemy_never_spawns_inside_a_solid_prop() {
        // Un anneau de 25 m encombré d'un gros prop à l'angle 0.
        let obstacles = DecorObstacles {
            discs: vec![(Vec2::new(25.0, 0.0), 6.0)],
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
    fn a_fully_blocked_ring_still_yields_the_least_bad_angle() {
        // Anneau entièrement ceinturé : aucune place libre.
        let mut discs = Vec::new();
        for i in 0..24 {
            let a = std::f32::consts::TAU * i as f32 / 24.0;
            discs.push((Vec2::new(25.0 * a.cos(), 25.0 * a.sin()), 5.0));
        }
        // …sauf un trou volontairement plus dégagé.
        discs[7].1 = 0.2;
        let obstacles = DecorObstacles { discs };
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
        let h = || vec![Handle::<Scene>::default()];
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
        let h = || vec![Handle::<Scene>::default()];
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
