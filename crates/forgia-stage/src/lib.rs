//! forgia-stage-arena — Generic stage arena loader (data-driven, mode-agnostic).
//!
//! Story-483 V7 P0 (NEW crate). Loads a bounded arena from a `stage_id`
//! referenced in `assets/genomes/roguelite_stages.toml`. Spawns :
//! - terrain bornée (extent_m × extent_m) ;
//! - ramparts hexagonaux KayKit (réutilise pattern story-441 village) ;
//! - anchor points (PlayerSpawn / PoiSlot / BossPad / Landmark) via
//!   `forgia-anchor` ;
//! - POIs (chest / shrine / elite-pad / boss-pad) via `forgia-prefab`.
//!
//! Consommé par `forgia-mode-roguelite` (V1) et par `forgia-mode-fps-arena`
//! (M4 backlog migration).
//!
//! ## P0 scope (this commit)
//!
//! - Types : `StageLoadRequest`, `StageLoadResult`, `StageState`, `StageDef`,
//!   `RogueliteStagesGenome`, `PoiDef`, `RoguelitePoisGenome`, `RampartsShape`.
//! - Plugin shell : registers genomes + insert default `StageLoadResult` +
//!   sensor writer 1Hz.
//! - Pure helpers : `ramparts_hex_positions`, `poi_anchor_positions`,
//!   `splitmix64`, `severity_for_stage`, `next_step_for_stage`.
//! - Tests purs ~12 cases.
//! - **Pas encore de spawn runtime** — c'est P1.
//!
//! ## Sources
//!
//! - GDC 2022 Returnal "Never The Same Twice" (Ethan Watson) — handcrafted +
//!   procedural connections.
//! - RoR2 Hopoo design ([gamedeveloper.com](https://www.gamedeveloper.com/design/how-moving-from-2d-to-3d-shaped-the-design-of-i-risk-of-rain-2-i-)) — stages pre-built + objets randomisés.
//! - Forgia memory `[[reference-loader-request-result-pattern]]` (story-441).
//! - Forgia memory `[[reference-pattern-genome-driven-plugin-with-sensor]]`.

pub mod authored;
pub mod floor_merge;
pub mod graph;
pub mod layout;
pub mod rooms;
pub mod layout_sensor;

/// Sol de REPLI de l'arène — celui d'avant story-676, quand c'était le seul.
///
/// Depuis story-676 le sol vient de l'ambiance du round
/// (`StageLoadRequest.floor`) ; cette table reste le repli quand la requête n'en
/// porte pas, et la garantie qu'une arène a toujours un plancher.
/// Sans `#SceneN` : chaque consommateur ajoute le label de scène voulu.
pub const MERGED_FLOOR_GLB: [&str; 3] = [
    "models/kaykit/dungeon/floor.glb",
    "models/kaykit/dungeon/floor_dirt.glb",
    "models/kaykit/dungeon/floor_rocks.glb",
];

/// La palette de sol d'une arène, passée par l'appelant.
///
/// `forgia-stage` reste générique : il ne connaît ni les univers du roguelite ni
/// leur génome. Il reçoit trois chemins et un pas de trame, et pave.
#[derive(Debug, Clone, PartialEq)]
pub struct FloorTiles {
    /// Côté RÉEL de la tuile (m) — l'arène pave à ce pas.
    pub tile_size_m: f32,
    /// Exactement 3 : le mélangeur produit un `kind` 0/1/2.
    pub tiles: [String; 3],
}

impl Default for FloorTiles {
    fn default() -> Self {
        Self {
            // La const historique reste la source du pas de repli : elle porte
            // l'invariant « même sol que le mode FPS » (cf sa doc).
            tile_size_m: FLOOR_TILE_SIZE,
            tiles: MERGED_FLOOR_GLB.map(str::to_owned),
        }
    }
}

/// Chemins de sol SUPPLÉMENTAIRES à précharger au Lobby.
///
/// Le warmup de pipelines tourne dans `forgia-stage`, qui ne sait pas quels
/// univers la run traversera. L'appelant (mode roguelite) remplit cette
/// ressource au Startup avec toutes les tuiles de toutes ses ambiances — sinon
/// le matériau du 2ᵉ univers compilerait au 1er affichage, en plein combat
/// (c'est le défaut que story-664 avait corrigé pour le sol unique).
#[derive(Resource, Debug, Default, Clone)]
pub struct ExtraFloorPreloads(pub Vec<String>);

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_anchor::{AnchorKind, AnchorPoint, AnchorStats};
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_level_presets::{LevelModulesGenome, LevelModulesHandles, ModulePaletteEntry};
use forgia_prefab::{spawn_gltf_prefab, PrefabSpawn, PrefabStats};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

const STAGES_GENOME_PATH: &str = "genomes/roguelite_stages.toml";
const POIS_GENOME_PATH: &str = "genomes/roguelite_pois.toml";
/// Story-625 - coquille authored data-driven (modele Returnal hybride).
const ARENA_LAYOUTS_GENOME_PATH: &str = "genomes/arena_layouts.toml";
const SENSOR_PATH: &str = "forgia2_stage.json";
const SENSOR_WRITE_PERIOD_SEC: f64 = 1.0;

// ─── splitmix64 (inline, déterministe, pas de dep RNG) ──────────────────────

/// SplitMix64 — Vigna 2014. Stateful, déterministe par seed. Réutilise le
/// même algo que `forgia-stage-graph` pour cohérence cross-crate.
#[inline]
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

// ─── StageState ─────────────────────────────────────────────────────────────

/// État cycle de vie d'un stage load. Transitions :
/// `Idle → Loading → Ready` (happy path) ou `Idle → Loading → Error`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageState {
    #[default]
    Idle,
    Loading,
    Ready,
    Error,
}

impl StageState {
    pub const fn as_str(self) -> &'static str {
        match self {
            StageState::Idle => "idle",
            StageState::Loading => "loading",
            StageState::Ready => "ready",
            StageState::Error => "error",
        }
    }
}

// ─── RampartsShape ──────────────────────────────────────────────────────────

/// Forme du bornage de l'arène. V1 supporte hexagonal seulement (pattern village
/// story-441). Extensions futures : `Square`, `Cliffs` (heightmap forced edge).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RampartsShape {
    #[default]
    Hexagonal,
}

// ─── StageDef (genome data) ─────────────────────────────────────────────────

/// Définition d'un stage chargée depuis `assets/genomes/roguelite_stages.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct StageDef {
    /// Biome name (matches `forgia-terrain` BiomeKind string).
    pub biome: String,
    /// Demi-côté de l'arène (m). Pour ramparts hex inscrits dans cercle rayon
    /// `arena_extent_m`, l'aire utile gameplay = πr² ≈ 3.46 × extent_m².
    pub arena_extent_m: f32,
    /// Kit KayKit à utiliser pour les ramparts (e.g. "kaykit_dungeon",
    /// "medieval_hexagon"). Lookup résolu côté loader P1.
    pub ramparts_kit: String,
    /// Forme du bornage. V1 = hexagonal exclusivement.
    #[serde(default)]
    pub ramparts_shape: RampartsShape,
    /// Nombre d'anchor slots pour POIs (chest / elite-pad / etc.) sur le
    /// cercle anchor. PlayerSpawn et BossPad sont des anchors dédiés en plus.
    pub anchor_slots: u32,
    /// Music state à activer (e.g. "combat_intense"). Optionnel.
    #[serde(default)]
    pub music_state: Option<String>,
    /// Si true, force le spawn d'un BossPad anchor + POI prefab.
    #[serde(default)]
    pub boss_pad_required: bool,
    /// Override météo (e.g. "ashfall"). Optionnel — consommé par audio/VFX.
    #[serde(default)]
    pub weather_override: Option<String>,
    /// Longueur visuelle naturelle d'1 wall.glb du kit (m). Optionnel — si
    /// omis ou ≤ 0, fallback `wall_natural_len_for_kit(ramparts_kit)` (smart
    /// default par-kit). Hot-reload Shift+F12 supporté.
    #[serde(default)]
    pub wall_natural_len_m: Option<f32>,
    /// Palette de modules à placer intra-arena (story-485 phase 3).
    /// Chaque entry référence un `ModuleDef` de `LevelModulesGenome` par id + count.
    /// Vide = stage sans cover designed (rétro-compat story-483 baseline).
    #[serde(default)]
    pub module_palette: Vec<ModulePaletteEntry>,
}

/// Smart default par kit basé sur les dimensions natives du pack KayKit.
/// Sourcé empiriquement — à corriger si AABB runtime montre autre chose.
/// Pattern Larian Definition layer : ce mapping vit en code seulement parce
/// que les dimensions sont une propriété du pack asset, pas du gameplay.
pub fn wall_natural_len_for_kit(kit: &str) -> f32 {
    match kit {
        // MESURÉ (story-673, `assets/genomes/asset_registry.toml`) :
        // dungeon/wall.glb = 4,00 × 1,00 × 4,00 m. L'ancien 1,0 « à confirmer
        // empiriquement » était faux d'un facteur 4 ; le TOML de crypts_of_anvil
        // l'avait déjà contourné à la main en passant à 4,0 après un diagnostic
        // de crash. La mesure confirme le contournement et corrige la source.
        "kaykit_dungeon" | "kaykit_dungeon_remastered" => 4.0,
        // MESURÉ : medieval_hexagon/wall_straight.gltf = 2,00 × 0,80 × 1,10 m.
        // L'ancien 4,0 espaçait les murs du DOUBLE de leur largeur → un trou sur
        // deux dans l'enceinte.
        //
        // ⚠️ DÉFAUT CONNU, non corrigible ici : ce mur ne fait que **1,10 m de
        // haut** alors que le collider d'enceinte fait `RAMPARTS_WALL_HEIGHT_M`
        // = 4 m. Les stages qui utilisent ce kit ont donc un mur INVISIBLE de
        // 2,90 m au-dessus d'une barrière visible. Baisser le collider n'est pas
        // une option : 1,10 m est SOUS la hauteur de saut du joueur (1,174 m),
        // l'arène deviendrait franchissable. Il faut un autre kit de remparts
        // pour ces stages — décision de DA, pas de code.
        "medieval_hexagon" => 2.0,
        _ => 1.0,
    }
}

/// Genome TOML : `[stages.<id>] StageDef`. Hot-reloadable via Shift+F12.
#[derive(Debug, Default, Clone, Deserialize, TypePath)]
pub struct RogueliteStagesGenome {
    #[serde(default)]
    pub stages: HashMap<String, StageDef>,
}

// ─── PoiDef (genome data) ───────────────────────────────────────────────────

/// Définition d'un POI (Point of Interest) — chest, elite-pad, boss-pad, etc.
#[derive(Debug, Clone, Deserialize)]
pub struct PoiDef {
    /// Weight pour pondération RNG. 0 = forced/never (e.g. boss_pad weight=0
    /// est forcé uniquement quand `stage.boss_pad_required`).
    pub weight: u32,
    /// Chemin GLB relatif à `assets/` (e.g. "models/kaykit/dungeon/chest_basic.glb").
    pub prefab: String,
    /// Type d'encounter déclenché par ce POI ("none", "wave_elite", etc.).
    #[serde(default = "default_encounter")]
    pub encounter: String,
    /// Footprint approximatif (m). Utilisé pour éviter overlap POI<->POI.
    pub size_m: f32,
}

fn default_encounter() -> String {
    "none".to_string()
}

#[derive(Debug, Default, Clone, Deserialize, TypePath)]
pub struct RoguelitePoisGenome {
    #[serde(default)]
    pub pois: HashMap<String, PoiDef>,
}

// ─── StageLoadRequest / StageLoadResult Resources ───────────────────────────

/// Resource Request (caller insert) — déclenche le load d'un stage.
///
/// Pattern Resource Request → Resource Result (story-441 reference). Consommé
/// par `spawn_stage_arena_on_request` (P1) qui spawn terrain + ramparts +
/// anchors et update `StageLoadResult`.
#[derive(Resource, Debug, Clone, Default)]
pub struct StageLoadRequest {
    /// Identifiant stage (clef `[stages.<id>]` dans roguelite_stages.toml).
    pub stage_id: String,
    /// Seed RNG pour placement anchors (host-authoritative reproductible).
    pub seed: u64,
    /// Story-676 — palette de sol de l'univers du round. `None` = sol de repli
    /// (celui d'avant, `MERGED_FLOOR_GLB`) : une arène a TOUJOURS un plancher.
    pub floor: Option<FloorTiles>,
    /// Story-676 — clé de ciel de l'univers (`biome_sky.toml`). `None` = le ciel
    /// suit le biome du stage, comme avant.
    ///
    /// Champ SÉPARÉ du biome, exprès : le biome reste ce que le stage déclare,
    /// et le capteur continue de dire la vérité. Le ciel d'une arène n'est pas
    /// un fait de terrain, c'est un choix d'univers.
    pub sky: Option<String>,
}

/// Resource Result (loader update) — état runtime du stage chargé.
#[derive(Resource, Debug, Default)]
pub struct StageLoadResult {
    pub state: StageState,
    pub stage_id: String,
    pub biome: String,
    /// Story-676 — clé de palette de ciel effective. Vide = suivre `biome`.
    pub sky_key: String,
    pub extent_m: f32,
    pub anchors_placed: u32,
    pub props_spawned: u32,
    /// Nombre de PointLights d'ambiance cartoon (glow chaud par POI + BossPad,
    /// biome-tuned). Cosmétique, audit §6 gap#3 emissive/glow. Visible via
    /// `forgia2_stage.json::ambiance_lights`.
    pub ambiance_lights: u32,
    /// Timestamp en secondes (since startup) du dernier passage en Loading.
    /// Utilisé pour détecter Loading > 5s (health alert).
    pub loading_started_secs: f64,
    /// Message d'erreur si state == Error.
    pub error_message: String,
    /// `wall_natural_len_m` effectif utilisé (TOML override OU smart default).
    /// Exposé pour debug calibration ramparts via `forgia2_stage.json`.
    pub wall_natural_len_used: f32,
    /// Nombre de murs tilés par segment hex (1 segment = 1/6 du périmètre).
    /// Diagnostic gaps/overlaps avant rebuild.
    pub walls_per_segment: u32,
    /// Story-663 — meshes fusionnés du sol (cellule × matériau). 0 après
    /// stabilisation = fallback tuile-par-tuile actif (voir log warn).
    pub floor_merged_cells: u32,
    /// Story-663 Inc.2 — meshes fusionnés des murs de ramparts.
    pub walls_merged_cells: u32,
    /// Story-663 — nombre de lots de fusion en attente (sol/murs). Soutenu
    /// > MERGE_TIMEOUT_SECS = sondes bloquées (GLB KO ?) → voir log warn.
    pub static_merge_pending: u32,
    /// `music_state` du stage def actuel (toggle TOML). Vide si stage def n'en
    /// définit pas. Consommé par caller (forgia-mode-roguelite::sys_apply_stage_toggles).
    pub music_state_id: String,
    /// `weather_override` du stage def actuel. Vide si non défini. Consommé par
    /// future crate `forgia-weather` (V2). Visibilité runtime via sensor.
    pub weather_override: String,
}

// ─── Genome handles Resource ────────────────────────────────────────────────

/// Storage des handles genome chargés — assure que les assets ne sont pas
/// drop entre frames. Pattern arena_bots.toml hot-reload.
#[derive(Resource, Default)]
pub struct StageGenomeHandles {
    pub stages: Handle<Genome<RogueliteStagesGenome>>,
    pub pois: Handle<Genome<RoguelitePoisGenome>>,
    /// Story-625 - layouts authored par stage (arena_layouts.toml).
    pub arena_layouts: Handle<Genome<authored::ArenaLayoutsGenome>>,
}

/// Handles de toutes les scènes référencées par les genomes de stage. Ils sont
/// résolus dès le boot afin que l'I/O GLTF et les pipelines soient amortis au
/// Lobby, plutôt que déclenchés au passage d'un portail.
#[derive(Resource, Default)]
pub struct StageScenePreloads {
    pub scenes: Vec<Handle<Scene>>,
    pub ready: bool,
}

// Note story-485 phase 3 : `LevelModulesGenome` n'est pas stocké ici. Le
// `ForgiaLevelPresetsPlugin` (ajouté via `is_plugin_added` guard) gère son
// propre `LevelModulesHandles` Resource. Phase 5 read directement via
// `Res<LevelModulesHandles>` + `Res<Assets<Genome<LevelModulesGenome>>>`.

// ─── LayoutResult Resource (story-485 phase 5) ──────────────────────────────

/// État runtime du dernier placement de modules. Mis à jour par
/// `spawn_stage_arena_on_request` quand un stage devient Ready.
/// Consommé par `layout_sensor::write_layout_sensor` pour
/// `forgia2_stage_layout.json`.
#[derive(Resource, Default, Debug, Clone)]
pub struct LayoutResult {
    pub stage_id: String,
    pub placements: Vec<layout::ModulePlacement>,
    pub longest_sightline_m: f32,
    pub min_cover_spacing_m: f32,
    /// Combien d'instances de la palette n'ont pas pu être placées (dart-throw
    /// exhausted ou id inconnu). Si élevé → palette trop dense ou TOML cassé.
    pub instances_skipped: u32,
    pub module_palette_used: Vec<String>,
    /// Story-625 - provenance : "authored" si des pieces authored posees, sinon "procedural".
    pub layout_source: String,
    /// Story-625 - nombre de pieces authored posees (0 = layout procedural pur).
    pub authored_pieces: u32,
    /// Story-625 - nombre de sections authored distinctes (bible).
    pub authored_sections: u32,
}

// ─── LayoutParams SystemParam bundle (évite limite Bevy 16 params) ──────────

/// Bundle des ressources liées à la layout palette. Évite le system param
/// overflow dans `spawn_stage_arena_on_request` (limite Bevy 0.18 = 16).
#[derive(SystemParam)]
pub struct LayoutParams<'w> {
    pub handles: Res<'w, LevelModulesHandles>,
    pub assets: Res<'w, Assets<Genome<LevelModulesGenome>>>,
    pub result: ResMut<'w, LayoutResult>,
    /// Story-625 - assets des layouts authored (lookup par stage_id via le
    /// handle `StageGenomeHandles::arena_layouts`).
    pub arena_assets: Res<'w, Assets<Genome<authored::ArenaLayoutsGenome>>>,
}

// ─── Marker Component ───────────────────────────────────────────────────────

/// Marker — toute entité spawnée par stage-arena la porte. Cleanup via
/// `DespawnOnExit(GameMode::Roguelite)` côté caller, ou via query directe.
#[derive(Component, Debug)]
pub struct StageArenaMarker;

// ─── Plugin ─────────────────────────────────────────────────────────────────

/// Bevy Plugin. Add via `app.add_plugins(ForgiaStageArenaPlugin)`.
///
/// **P0 scope** : registers genomes + insère Resources + lance sensor writer.
/// **P1 scope** : ajoutera `spawn_stage_arena_on_request` system.
pub struct ForgiaStageArenaPlugin;

impl Plugin for ForgiaStageArenaPlugin {
    fn build(&self, app: &mut App) {
        // Idempotent : add dependent plugins only once.
        if !app.is_plugin_added::<forgia_anchor::ForgiaAnchorPlugin>() {
            app.add_plugins(forgia_anchor::ForgiaAnchorPlugin);
        }
        if !app.is_plugin_added::<forgia_prefab::ForgiaPrefabPlugin>() {
            app.add_plugins(forgia_prefab::ForgiaPrefabPlugin);
        }
        // Story-485 phase 3 — level modules palette (cover/lane data layer).
        if !app.is_plugin_added::<forgia_level_presets::ForgiaLevelPresetsPlugin>() {
            app.add_plugins(forgia_level_presets::ForgiaLevelPresetsPlugin);
        }
        app.init_asset::<Genome<RogueliteStagesGenome>>()
            .register_asset_loader(GenomeLoader::<RogueliteStagesGenome>::default())
            .init_asset::<Genome<RoguelitePoisGenome>>()
            .register_asset_loader(GenomeLoader::<RoguelitePoisGenome>::default())
            // Story-625 - coquille authored data-driven.
            .init_asset::<Genome<authored::ArenaLayoutsGenome>>()
            .register_asset_loader(GenomeLoader::<authored::ArenaLayoutsGenome>::default())
            .init_resource::<StageLoadResult>()
            .init_resource::<StageGenomeHandles>()
            .init_resource::<StageScenePreloads>()
            .init_resource::<StageArenaTuning>()
            .init_resource::<LayoutResult>()
            // Story-663 — cache des fusions statiques (re-entrée = zéro rebuild).
            .init_resource::<ExtraFloorPreloads>()
            .init_resource::<floor_merge::MergedStaticCache>()
            .add_systems(Startup, load_stage_genomes)
            .add_systems(
                Update,
                sys_preload_stage_scenes.in_set(forgia_core::prelude::GameSet::Movement),
            )
            // L7 SystemSets — stage-arena is **mode-agnostic by design** : il sera
            // consommé par forgia-mode-roguelite (V1), forgia-mode-fps-arena (M4),
            // forgia-rpg (POI hubs), etc. Pas de `run_if(in_state(
            // GameMode::X))` au niveau plugin — guard naturel via `request: Option<
            // Res<StageLoadRequest>>` + `q_existing` archetype scan (~0 cost si
            // empty). Caller responsible pour insert/cleanup le StageLoadRequest.
            .add_systems(
                Update,
                spawn_stage_arena_on_request
                    .in_set(forgia_core::prelude::GameSet::Movement),
            )
            // Story-625 - collider differe des pieces authored walkable/blocker
            // (GLB async -> retry jusqu'a mesh pret). Apres le spawn.
            .add_systems(
                Update,
                authored::sys_collide_authored_pieces
                    .in_set(forgia_core::prelude::GameSet::Movement)
                    .after(spawn_stage_arena_on_request),
            )
            // Story-663 - fusion statique par cellule (retry jusqu'a sondes pretes)
            // + drain etale du spawn (fix burst 412 ms au 1er rendu).
            .add_systems(
                Update,
                (
                    floor_merge::sys_build_merged_static,
                    floor_merge::sys_drain_merge_spawn_queue,
                )
                    .chain()
                    .in_set(forgia_core::prelude::GameSet::Movement)
                    .after(spawn_stage_arena_on_request),
            )
            .add_systems(
                Update,
                write_stage_sensor.in_set(forgia_core::prelude::GameSet::Sensors),
            )
            .add_systems(
                Update,
                layout_sensor::write_layout_sensor
                    .in_set(forgia_core::prelude::GameSet::Sensors)
                    // BUG-485-02 fix : garantit que le sensor lit le LayoutResult
                    // de la frame actuelle, pas la précédente. GameSet::Movement
                    // précède Sensors dans la L7 chain, mais .after explicite
                    // protège contre re-ordering futur.
                    .after(spawn_stage_arena_on_request),
            );
        info!(
            "[forgia-stage-arena] Plugin loaded — genome paths: {} + {}",
            STAGES_GENOME_PATH, POIS_GENOME_PATH
        );
    }
}

// ─── Startup : load genomes ─────────────────────────────────────────────────

fn load_stage_genomes(asset_server: Res<AssetServer>, mut handles: ResMut<StageGenomeHandles>) {
    handles.stages = asset_server.load(STAGES_GENOME_PATH);
    handles.pois = asset_server.load(POIS_GENOME_PATH);
    handles.arena_layouts = asset_server.load(ARENA_LAYOUTS_GENOME_PATH);
    info!(
        "[forgia-stage-arena] Genome handles loading: stages={} pois={} arena_layouts={}",
        STAGES_GENOME_PATH, POIS_GENOME_PATH, ARENA_LAYOUTS_GENOME_PATH
    );
}

/// Une fois les deux genomes disponibles, charge de façon asynchrone toutes les
/// scènes possibles de stage. Les handles restent vivants dans la ressource et
/// sont aussi consommés par le warmup de pipeline Roguelite ; aucun chemin
/// d'asset n'est dupliqué dans le code du mode.
fn sys_preload_stage_scenes(
    handles: Res<StageGenomeHandles>,
    stages_assets: Res<Assets<Genome<RogueliteStagesGenome>>>,
    layout_assets: Res<Assets<Genome<authored::ArenaLayoutsGenome>>>,
    asset_server: Res<AssetServer>,
    mut preloads: ResMut<StageScenePreloads>,
    extra_floors: Res<ExtraFloorPreloads>,
) {
    if preloads.ready {
        return;
    }
    let Some(stages) = stages_assets.get(&handles.stages) else {
        return;
    };
    let Some(layouts) = layout_assets.get(&handles.arena_layouts) else {
        return;
    };
    let mut paths = BTreeSet::new();
    paths.extend(MERGED_FLOOR_GLB.map(str::to_owned));
    // Tous les sols des univers déclarés par l'appelant : le matériau doit
    // compiler au Lobby, pas au 1er affichage du 2ᵉ univers (story-676).
    paths.extend(extra_floors.0.iter().cloned());
    for stage in stages.data.stages.values() {
        paths.insert(ramparts_wall_glb(&stage.ramparts_kit).to_owned());
    }
    for layout in layouts.data.layouts.values() {
        paths.extend(layout.pieces.iter().map(|piece| piece.prefab.clone()));
    }
    preloads.scenes = paths
        .into_iter()
        .map(|path| asset_server.load(format!("{path}#Scene0")))
        .collect();
    preloads.ready = true;
    info!(
        "[stage-arena] preloaded {} unique stage scene classes from genomes",
        preloads.scenes.len()
    );
}

// ─── Sensor writer 1Hz ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct StageSensorJson<'a> {
    id: &'a str,
    severity: &'a str,
    timestamp_secs: f64,
    state: &'a str,
    stage_id: &'a str,
    biome: &'a str,
    extent_m: f32,
    anchors_placed: u32,
    props_spawned: u32,
    ambiance_lights: u32,
    wall_natural_len_used: f32,
    walls_per_segment: u32,
    floor_merged_cells: u32,
    walls_merged_cells: u32,
    static_merge_pending: u32,
    music_state_id: &'a str,
    weather_override: &'a str,
    next_step: &'a str,
}

fn write_stage_sensor(
    result: Res<StageLoadResult>,
    tuning: Res<StageArenaTuning>,
    mut last_write: Local<f64>,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    if now - *last_write < SENSOR_WRITE_PERIOD_SEC {
        return;
    }
    *last_write = now;

    let loading_elapsed = if result.state == StageState::Loading {
        now - result.loading_started_secs
    } else {
        0.0
    };
    let severity = severity_for_stage_with_threshold(
        result.state,
        loading_elapsed,
        tuning.loading_sustained_warn_sec,
    );
    let next_step = next_step_for_stage_with_threshold(
        result.state,
        loading_elapsed,
        tuning.loading_sustained_warn_sec,
    );

    let payload = StageSensorJson {
        id: "stage",
        severity,
        timestamp_secs: now,
        state: result.state.as_str(),
        stage_id: &result.stage_id,
        biome: &result.biome,
        extent_m: result.extent_m,
        anchors_placed: result.anchors_placed,
        props_spawned: result.props_spawned,
        ambiance_lights: result.ambiance_lights,
        wall_natural_len_used: result.wall_natural_len_used,
        walls_per_segment: result.walls_per_segment,
        floor_merged_cells: result.floor_merged_cells,
        walls_merged_cells: result.walls_merged_cells,
        static_merge_pending: result.static_merge_pending,
        music_state_id: &result.music_state_id,
        weather_override: &result.weather_override,
        next_step,
    };
    if let Ok(json) = serde_json::to_string_pretty(&payload) {
        let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
    }
}

// ─── Severity / Next-step (pure, testable) ──────────────────────────────────

/// Default seuil au-delà duquel un state `Loading` est considéré dégradé.
/// Surchargeable via Resource [`StageArenaTuning`] (conforme observability-required.md
/// "Genes genome — seuils/toggles hot-reloadables").
pub const DEFAULT_LOADING_SUSTAINED_WARN_SEC: f64 = 5.0;

/// Resource tunable — seuils observabilité stage-arena. Évite hardcode const
/// (cf. qa-lead audit story-483 BUG-483-08). Future : sync depuis
/// `debug_monitor.toml::dbg_stage_arena_loading_timeout_sec` (M2 wire-up).
#[derive(Resource, Debug, Clone)]
pub struct StageArenaTuning {
    pub loading_sustained_warn_sec: f64,
}

impl Default for StageArenaTuning {
    fn default() -> Self {
        Self {
            loading_sustained_warn_sec: DEFAULT_LOADING_SUSTAINED_WARN_SEC,
        }
    }
}

/// Severity pure — paramétré par `loading_warn_sec` (TBD M2 : lu de genome).
pub fn severity_for_stage_with_threshold(
    state: StageState,
    loading_elapsed: f64,
    loading_warn_sec: f64,
) -> &'static str {
    match state {
        StageState::Idle => "info",
        StageState::Loading if loading_elapsed > loading_warn_sec => "warn",
        StageState::Loading => "info",
        StageState::Ready => "ok",
        StageState::Error => "critical",
    }
}

/// Next-step pure — paramétré par `loading_warn_sec`.
pub fn next_step_for_stage_with_threshold(
    state: StageState,
    loading_elapsed: f64,
    loading_warn_sec: f64,
) -> &'static str {
    match state {
        StageState::Idle => {
            "No stage requested. Caller (forgia-mode-roguelite) must insert StageLoadRequest at run start."
        }
        StageState::Loading if loading_elapsed > loading_warn_sec => {
            "Stage stuck Loading > threshold. Read forgia_prefab.json + forgia2_anchor.json to identify stalled asset. Verify GLB paths in roguelite_stages.toml exist."
        }
        StageState::Loading => "Stage loading — assets fetching.",
        StageState::Ready => "Stage ready. Read forgia2_anchor.json for anchor placement details.",
        StageState::Error => {
            "Stage load failed. Read forgia2_stage.json error_message field + verify assets/genomes/roguelite_stages.toml syntax + GLB paths exist."
        }
    }
}

/// Severity pure — wrapper avec threshold default. Conservé pour tests pre-P3.
pub fn severity_for_stage(state: StageState, loading_elapsed: f64) -> &'static str {
    severity_for_stage_with_threshold(state, loading_elapsed, DEFAULT_LOADING_SUSTAINED_WARN_SEC)
}

/// Next-step pure — wrapper avec threshold default.
pub fn next_step_for_stage(state: StageState, loading_elapsed: f64) -> &'static str {
    next_step_for_stage_with_threshold(state, loading_elapsed, DEFAULT_LOADING_SUSTAINED_WARN_SEC)
}

// ─── Pure helpers : placement géométrique (testable headless) ───────────────

/// Positions + rotations des N segments de ramparts pour un hexagone régulier
/// inscrit dans le cercle rayon `extent_m`. 6 segments connectés bord-à-bord.
///
/// Retourne (position_center_segment, rotation_yaw). La position est le milieu
/// du segment, prêt pour spawn_gltf_prefab. Y = 0.0 (WALL_Y LOCK, pivot at floor).
pub fn ramparts_hex_positions(extent_m: f32) -> Vec<(Vec3, Quat)> {
    if extent_m <= 0.0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(6);
    // Vertices de l'hex : 6 points sur cercle rayon extent_m, angles 30°,90°,...
    // Segments = milieu entre vertex i et vertex (i+1)%6.
    let r = extent_m;
    let mut verts = [Vec3::ZERO; 6];
    for (i, v) in verts.iter_mut().enumerate() {
        let a = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * i as f32;
        *v = Vec3::new(r * a.cos(), 0.0, r * a.sin());
    }
    for i in 0..6 {
        let a = verts[i];
        let b = verts[(i + 1) % 6];
        let mid = (a + b) * 0.5;
        // Yaw : ALIGNE l'axe local X du mur sur la direction segment (a→b).
        // Convention KayKit GLTF : X=width (long), Y=height, Z=thickness (mince).
        // Pour que world.X = dir.normalized() après Quat::from_rotation_y(yaw) :
        //   cos(yaw)= dir.x/|dir|, -sin(yaw)= dir.z/|dir|
        //   → yaw = atan2(-dir.z, dir.x)
        // (Bug fix 2026-05-20 PM : ancien `atan2(-dir.x, dir.z)` alignait
        //  l'axe Z=thickness sur le segment → murs vus par la tranche)
        let dir = b - a;
        let yaw = (-dir.z).atan2(dir.x);
        out.push((mid, Quat::from_rotation_y(yaw)));
    }
    out
}

/// Positions tilées des murs ramparts hexagonaux — `wall_natural_len` par mur,
/// placés **côte-à-côte sans stretch** (préserve fidélité visuelle KayKit).
///
/// Pour chaque segment (6 segments hex), spawne `ceil(side / wall_len)` murs
/// à leur taille naturelle, distribués uniformément. Retourne
/// `walls_per_segment * 6` entries (position centrée sur le mur + rotation).
///
/// Bug fix story-483 P1 (2026-05-20 PM) : la version précédente stretchait 1
/// mur via `scale_x = side / 4.0` → ratio 22× sur extent=90, murs apparaissaient
/// tordus comme des planches. Cf. screenshot user 2026-05-20 PM.
pub fn ramparts_hex_tiled_positions(extent_m: f32, wall_natural_len: f32) -> Vec<(Vec3, Quat)> {
    if extent_m <= 0.0 || wall_natural_len <= 0.0 {
        return Vec::new();
    }
    let mut verts = [Vec3::ZERO; 6];
    for (i, v) in verts.iter_mut().enumerate() {
        let a = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * i as f32;
        *v = Vec3::new(extent_m * a.cos(), 0.0, extent_m * a.sin());
    }
    let hex_side_len = extent_m; // hex inscrit cercle rayon=extent_m → côté = extent_m
    let walls_per_segment = ((hex_side_len / wall_natural_len).ceil() as u32).max(1);
    let mut out = Vec::with_capacity((walls_per_segment * 6) as usize);
    for i in 0..6 {
        let a = verts[i];
        let b = verts[(i + 1) % 6];
        let dir = b - a;
        // Cf. `ramparts_hex_positions` : aligne X local sur dir (KayKit width=X).
        let yaw = (-dir.z).atan2(dir.x);
        let rot = Quat::from_rotation_y(yaw);
        for j in 0..walls_per_segment {
            let t = (j as f32 + 0.5) / walls_per_segment as f32;
            let pos = a + dir * t;
            out.push((pos, rot));
        }
    }
    out
}

/// Positions + rotations des **midpoints** des 6 segments (1 entry par segment).
/// Utile pour 1 collider physics par segment (cf spawn system).
pub fn ramparts_hex_segment_midpoints(extent_m: f32) -> Vec<(Vec3, Quat)> {
    ramparts_hex_positions(extent_m)
}

/// Positions des POI anchor slots sur un cercle inscrit à `radius_factor *
/// extent_m`. Distribution uniforme avec jitter déterministe (splitmix64).
///
/// Garantit zéro overlap pairwise tant que `slots <= floor(2π * radius / max_size_m)`.
/// V1 utilise `radius_factor = 0.55` pour anchors bien inside ramparts.
pub fn poi_anchor_positions(extent_m: f32, slots: u32, seed: u64) -> Vec<Vec3> {
    if slots == 0 || extent_m <= 0.0 {
        return Vec::new();
    }
    let radius = extent_m * 0.55;
    let mut out = Vec::with_capacity(slots as usize);
    let step = std::f32::consts::TAU / slots as f32;
    let mut rng = seed ^ 0xFEED_BEEF_DEAD_CAFE_u64;
    for i in 0..slots {
        let base_angle = step * i as f32;
        // Jitter ±step/6 → 30° max d'écart pour préserver l'espacement.
        let r = splitmix64(&mut rng);
        let jitter_norm = (r as f64 / u64::MAX as f64) as f32 - 0.5;
        let jitter = jitter_norm * (step / 3.0);
        let angle = base_angle + jitter;
        out.push(Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin()));
    }
    out
}

// ─── Spawn system (P1 runtime) ──────────────────────────────────────────────

const RAMPARTS_WALL_HEIGHT_M: f32 = 4.0;
const RAMPARTS_WALL_THICKNESS_M: f32 = 0.4;

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Mapping `stage_def.ramparts_kit` → GLB path. Centralisé pour évolution
/// future (registry crate dédié si > 5 kits).
fn ramparts_wall_glb(kit: &str) -> &'static str {
    match kit {
        "kaykit_dungeon" => "models/kaykit/dungeon/wall.glb",
        "medieval_hexagon" => "models/kaykit/medieval_hexagon/walls/wall_straight.gltf",
        _ => "models/kaykit/dungeon/wall.glb",
    }
}

/// Côté (m) d'une tuile de sol GLB KayKit dungeon. MÊME valeur que le mode FPS
/// (`forgia-mode-fps-arena::TILE_SIZE = 4.0`) pour un sol identique.
const FLOOR_TILE_SIZE: f32 = 4.0;

/// Lighting cartoon family-friendly per biome. Bible v1 direction artistique :
/// jamais pitch black, fill light cartoon généreux, sky teinté biome.
///
/// Pattern key+fill : Sun (illuminance haut, shadows) + Fill (illuminance
/// modéré ~30% du sun, no shadows, couleur ambient inverse).
///
/// Returns (sun_color, sun_illuminance, fill_color, fill_illuminance, sky_color).
fn biome_lighting_params(biome: &str) -> (Color, f32, Color, f32, Color) {
    match biome {
        // Crypts of Anvil bible: warm amber sun + fill orange forge + sky rouge sombre
        "Volcanic" => (
            Color::srgb(1.0, 0.75, 0.50),  // sun amber forge
            8_000.0,                       // dimmer = moodier mais lisible
            Color::srgb(0.95, 0.55, 0.35), // fill orangé braise (réchauffe les ombres)
            3_000.0,                       // ~37% du sun, fill généreux
            Color::srgb(0.12, 0.06, 0.05), // sky rouge braise sombre
        ),
        // Forge Sanctum bible: lumière douce sanctuaire ouvert
        "Plains" => (
            Color::srgb(1.0, 0.98, 0.92),
            12_000.0,
            Color::srgb(0.70, 0.78, 0.90), // fill bleu ciel doux
            3_500.0,
            Color::srgb(0.55, 0.70, 0.88),
        ),
        "Desert" => (
            Color::srgb(1.0, 0.92, 0.75),
            14_000.0,
            Color::srgb(0.92, 0.82, 0.60),
            4_000.0,
            Color::srgb(0.85, 0.78, 0.62),
        ),
        "Forest" | "Jungle" => (
            Color::srgb(0.95, 1.0, 0.88),
            10_000.0,
            Color::srgb(0.55, 0.78, 0.55),
            2_800.0,
            Color::srgb(0.40, 0.55, 0.45),
        ),
        "Tundra" => (
            Color::srgb(0.92, 0.96, 1.0),
            11_000.0,
            Color::srgb(0.80, 0.86, 0.95),
            3_200.0,
            Color::srgb(0.78, 0.85, 0.92),
        ),
        _ => (
            Color::srgb(1.0, 0.98, 0.95),
            12_000.0,
            Color::srgb(0.70, 0.72, 0.78),
            3_000.0,
            Color::srgb(0.50, 0.55, 0.65),
        ),
    }
}

/// Glow d'ambiance cartoon par biome posé sur chaque POI + BossPad
/// (Cult of the Lamb : emissive/glow chaud pour l'atmosphère, audit §6 gap#3).
/// Crée des pools de lumière chaude façon "torche/forge" sans toucher au
/// key+fill `biome_lighting_params` (qui reste la source de vérité globale).
///
/// ⚠️ Intensité FAIBLE volontaire (≤ 8k lumens) : un PointLight plein-intensité
/// (100k+ "réaliste") casse l'Atmosphere/bloom — leçon dette tech 2026-05-18,
/// cf `forgia-effects/weapon_vfx` muzzle light. Shadowless = cheap, spawn-time
/// (pas hot path).
///
/// Returns (color, intensity_lumens, range_m).
fn biome_ambiance_glow(biome: &str) -> (Color, f32, f32) {
    match biome {
        "Volcanic" => (Color::srgb(1.0, 0.45, 0.18), 7_000.0, 12.0), // braise forge
        "Plains" => (Color::srgb(1.0, 0.95, 0.82), 4_500.0, 11.0),   // sanctuaire doux
        "Desert" => (Color::srgb(1.0, 0.86, 0.58), 5_000.0, 11.0),
        "Forest" | "Jungle" => (Color::srgb(0.78, 1.0, 0.68), 4_000.0, 10.0),
        "Tundra" => (Color::srgb(0.72, 0.88, 1.0), 4_500.0, 11.0), // glow froid
        _ => (Color::srgb(1.0, 0.92, 0.80), 4_500.0, 11.0),
    }
}

/// Pondère et tire un POI parmi `pool` selon les `weight` field. Pure (testable).
/// Retourne `None` si pool vide ou total_weight == 0.
fn pick_poi_weighted<'a>(
    pool: &'a [(&'a String, &'a PoiDef)],
    rng_state: &mut u64,
) -> Option<&'a PoiDef> {
    if pool.is_empty() {
        return None;
    }
    let total: u32 = pool.iter().map(|(_, p)| p.weight).sum();
    if total == 0 {
        return None;
    }
    let r = (splitmix64(rng_state) % u64::from(total)) as u32;
    let mut acc = 0u32;
    for (_, poi) in pool {
        acc += poi.weight;
        if r < acc {
            return Some(*poi);
        }
    }
    pool.last().map(|(_, p)| *p)
}

/// System core P1 — consomme `StageLoadRequest`, lit les 2 genomes, et spawne
/// terrain + ramparts hex + anchors + POIs + sun. Idempotent : skip si
/// même `stage_id` déjà chargé en Ready.
///
/// Cleanup via `cleanup_stage_arena` (caller OnExit GameMode::Roguelite).
#[allow(clippy::too_many_arguments)]
fn spawn_stage_arena_on_request(
    mut commands: Commands,
    request: Option<Res<StageLoadRequest>>,
    handles: Res<StageGenomeHandles>,
    stages_assets: Res<Assets<Genome<RogueliteStagesGenome>>>,
    pois_assets: Res<Assets<Genome<RoguelitePoisGenome>>>,
    asset_server: Res<AssetServer>,
    mut result: ResMut<StageLoadResult>,
    anchor_stats: Res<AnchorStats>,
    prefab_stats: Res<PrefabStats>,
    q_existing: Query<Entity, With<StageArenaMarker>>,
    mut last_processed_id: Local<String>,
    mut layout_params: LayoutParams,
) {
    let Some(req) = request else {
        return;
    };
    if req.stage_id.is_empty() {
        return;
    }
    // Idempotent : on ne ré-spawn pas le même stage si déjà Ready.
    if *last_processed_id == req.stage_id && result.state == StageState::Ready {
        return;
    }
    // P2 — Stage transition detection : nouveau stage_id différent du dernier
    // spawn → cleanup old entities AVANT spawn new. Permet le run loop multi-stage.
    if !last_processed_id.is_empty() && *last_processed_id != req.stage_id && !q_existing.is_empty()
    {
        let prev = std::mem::take(&mut *last_processed_id);
        let n = despawn_stage_entities(&mut commands, &q_existing, &anchor_stats, &mut result);
        info!(
            "[stage-arena] Stage transition '{}' → '{}' : despawned {} entities",
            prev, req.stage_id, n
        );
        // Next frame re-entre dans le system avec last_processed_id vide → spawn new.
        return;
    }

    // Wait for both genomes loaded.
    let Some(stages_genome) = stages_assets.get(&handles.stages) else {
        if result.state != StageState::Loading {
            result.state = StageState::Loading;
            result.loading_started_secs = now_secs();
            result.stage_id = req.stage_id.clone();
        }
        return;
    };
    let Some(pois_genome) = pois_assets.get(&handles.pois) else {
        if result.state != StageState::Loading {
            result.state = StageState::Loading;
            result.loading_started_secs = now_secs();
        }
        return;
    };
    // Story-625 (BUG-625-01) — attendre AUSSI le genome des layouts authored.
    // Sinon race : si arena_layouts.toml charge apres stages/pois, le stage
    // passe Ready en procedural pur et la garde idempotente bloque la re-entree
    // -> la coquille authored ne s'applique jamais. Meme pattern d'attente.
    if layout_params
        .arena_assets
        .get(&handles.arena_layouts)
        .is_none()
    {
        if result.state != StageState::Loading {
            result.state = StageState::Loading;
            result.loading_started_secs = now_secs();
        }
        return;
    }

    let Some(stage_def) = stages_genome.data.stages.get(&req.stage_id) else {
        result.state = StageState::Error;
        result.error_message = format!(
            "Stage '{}' not found in roguelite_stages.toml",
            req.stage_id
        );
        return;
    };

    info!(
        "[stage-arena] Spawning stage '{}' biome={} extent={}m anchors={}",
        req.stage_id, stage_def.biome, stage_def.arena_extent_m, stage_def.anchor_slots
    );

    let extent = stage_def.arena_extent_m;
    let mut props_spawned: u32 = 0;

    // Glow d'ambiance cartoon biome-tuné (un PointLight chaud shadowless par POI
    // + BossPad). Calculé une fois, réutilisé dans les boucles ci-dessous.
    let (glow_color, glow_intensity, glow_range) = biome_ambiance_glow(&stage_def.biome);
    let mut ambiance_lights: u32 = 0;

    // 1. Floor — story-663 : le sol tuilé (~1 600 entités + ~3 200 nœuds de
    //    scène) est FUSIONNÉ par cellule × matériau (perf : la scène statique
    //    borne le CPU, cible 60 fps GTX 1060 — cf `floor_merge`). Ici on ne
    //    spawne que le PLAN (`PendingFloorMerge`, mêmes maths que l'ex-boucle :
    //    culling circulaire, mix dirt/rocks, yaw déterministe) + 3 sondes
    //    cachées ; `sys_build_merged_floor` construit les meshes dès que les
    //    GLB sont instanciés (cache par extent, fallback tuile-par-tuile si
    //    GLB KO). Mêmes assets KayKit que le mode FPS (le layout diffère :
    //    culling circulaire + yaw ici, grille carrée fixe là-bas — QA-663 #2).
    //    1 collider global inchangé.
    // Source unique des paths (consommée aussi par le warmup de pipelines au
    // Lobby, story-664 — les matériaux du sol doivent compiler avant le combat).
    // Story-676 — le sol vient de l'univers du round. Repli explicite sur la
    // table historique : jamais d'arène sans plancher.
    let floor = req.floor.clone().unwrap_or_default();
    let scenes: [Handle<Scene>; 3] = floor
        .tiles
        .clone()
        .map(|p| asset_server.load(format!("{p}#Scene0")));
    let tiles = floor_merge::plan_floor_tiles(extent, floor.tile_size_m);
    let floor_tiles_planned = tiles.len();
    let floor_instances: Vec<(u8, Transform)> = tiles
        .into_iter()
        .map(|t| {
            (
                t.kind,
                Transform::from_translation(t.pos).with_rotation(Quat::from_rotation_y(t.yaw)),
            )
        })
        .collect();
    floor_merge::spawn_static_merge(
        &mut commands,
        "floor",
        floor_instances,
        scenes.to_vec(),
        extent,
        &req.stage_id,
    );
    result.floor_merged_cells = 0;
    // Collider sol global (le sol rendu — fusionné ou tuiles — n'en a pas).
    commands.spawn((
        Name::new(format!("StageFloorCollider_{}", req.stage_id)),
        StageArenaMarker,
        Transform::from_xyz(0.0, -0.5, 0.0),
        RigidBody::Fixed,
        Collider::cuboid(extent + 5.0, 0.5, extent + 5.0),
    ));
    props_spawned += 1;
    info!(
        "[stage-arena] Floor: {floor_tiles_planned} tuiles planifiées → merge par cellule (~{}m)",
        extent
    );

    // 2. Ramparts — 6 segments hexagonaux, walls **tilés côte-à-côte sans stretch**
    //    (fix story-483 P1 2026-05-20 PM, screenshot user → murs étirés tordus).
    //    Visual : N walls par segment à taille naturelle (preserve KayKit fidelity).
    //    Physics : 1 collider cuboid par segment (6 colliders au total, pas 6*N).
    let wall_glb = ramparts_wall_glb(&stage_def.ramparts_kit);
    // Wall natural length per kit : TOML override > smart default par-kit.
    let wall_len = match stage_def.wall_natural_len_m {
        Some(v) if v > 0.05 => v,
        _ => wall_natural_len_for_kit(&stage_def.ramparts_kit),
    };
    let hex_side_len = extent;
    // 1 collider per segment (midpoint + rotation).
    for (i, (mid, rot)) in ramparts_hex_segment_midpoints(extent)
        .into_iter()
        .enumerate()
    {
        commands.spawn((
            Name::new(format!("RampartCollider_{i}")),
            StageArenaMarker,
            Transform {
                translation: mid + Vec3::new(0.0, RAMPARTS_WALL_HEIGHT_M * 0.5, 0.0),
                rotation: rot,
                scale: Vec3::ONE,
            },
            GlobalTransform::default(),
            RigidBody::Fixed,
            Collider::cuboid(
                hex_side_len * 0.5,
                RAMPARTS_WALL_HEIGHT_M * 0.5,
                RAMPARTS_WALL_THICKNESS_M * 0.5,
            ),
        ));
    }
    // N walls tilés par segment (visual seulement, pas de collider — physics
    // déjà couverte par les 6 colliders ci-dessus).
    // Story-663 Inc.2 : ~130 prefabs mur → FUSIONNÉS par cellule × matériau
    // (cf `floor_merge::spawn_static_merge`). Même GLB, mêmes poses — le count
    // reste porté par `props_spawned` ; `forgia_prefab.json::total_spawned` ne
    // compte plus les murs (fusionnés hors prefab pipeline, assumé).
    let tiled = ramparts_hex_tiled_positions(extent, wall_len);
    let wall_scene: Handle<Scene> = asset_server.load(format!("{wall_glb}#Scene0"));
    let wall_instances: Vec<(u8, Transform)> = tiled
        .iter()
        .map(|(pos, rot)| (0u8, Transform::from_translation(*pos).with_rotation(*rot)))
        .collect();
    props_spawned += wall_instances.len() as u32;
    floor_merge::spawn_static_merge(
        &mut commands,
        "walls",
        wall_instances,
        vec![wall_scene],
        extent,
        &req.stage_id,
    );
    result.walls_merged_cells = 0;

    // 2.5 Coquille AUTHORED (story-625) - composition data-driven depuis
    //     arena_layouts.toml. Si presente, c'est la STRUCTURE de l'arene ; le
    //     scatter procedural (5.5) est coupe si suppress_procedural_modules.
    //     Pieces = GLB atomiques Inferno/KayKit existants, instancies par prefab.
    let authored_layout = layout_params
        .arena_assets
        .get(&handles.arena_layouts)
        .and_then(|g| g.data.layouts.get(&req.stage_id))
        .cloned();
    let suppress_procedural = authored_layout
        .as_ref()
        .map(|l| l.suppress_procedural_modules)
        .unwrap_or(false);
    let mut authored_pieces_spawned: u32 = 0;
    let mut authored_sections: std::collections::HashSet<String> = std::collections::HashSet::new();
    if let Some(layout) = authored_layout.as_ref() {
        for (i, piece) in layout.pieces.iter().enumerate() {
            let pos = Vec3::new(piece.pos[0], piece.pos[1], piece.pos[2]);
            let anchor = authored::role_to_anchor(&piece.role);
            // melee_pit : nom `Module_melee_pit_authored` pour que `boss_portal`
            // solidifie le dais + y pose la porte (synergie story-603).
            let name = if anchor == Some(AnchorKind::MeleePit) {
                "Module_melee_pit_authored".to_string()
            } else {
                let sec = if piece.section.is_empty() {
                    "x"
                } else {
                    piece.section.as_str()
                };
                format!("AuthoredPiece_{sec}_{i}")
            };
            let spawn = PrefabSpawn::new(&piece.prefab, pos)
                .with_yaw_deg(piece.rot_deg)
                .with_scale(piece.scale)
                .with_name(name);
            let e = spawn_gltf_prefab(
                &mut commands,
                &asset_server,
                &prefab_stats,
                spawn,
                (StageArenaMarker,),
            );
            // Anchor gameplay depuis la data (pose au point de la piece).
            if let Some(kind) = anchor {
                commands.entity(e).insert(AnchorPoint::new(kind, i as u32));
                anchor_stats.record(kind);
            }
            // Collider differe pour pieces walkable/blocker (perchoir, etc.).
            if piece.walkable || piece.blocker {
                commands.entity(e).insert(authored::AuthoredNeedsCollider);
            }
            if !piece.section.is_empty() {
                authored_sections.insert(piece.section.clone());
            }
            authored_pieces_spawned += 1;
            props_spawned += 1;
        }
        info!(
            "[stage-arena] Authored shell '{}' : {} pieces, {} sections, suppress_procedural={}",
            req.stage_id,
            authored_pieces_spawned,
            authored_sections.len(),
            suppress_procedural
        );
    }

    // 2.4 PIÈCES ET COULOIRS (story-683) — le complexe central.
    //
    // On ne se bat plus dans un disque nu : un bloc de pièces reliées par des
    // portes occupe le centre, l'anneau extérieur reste ouvert (forme Gunfire).
    // TOUT RESTE À PLAT — les bots n'ont ni navmesh ni gravité, mais leur
    // `collide_and_slide` sait déjà longer un mur et enfiler un couloir.
    //
    // Le mur utilisé est celui du kit DUNGEON quel que soit le stage : c'est le
    // seul mesuré à 4,00 m de haut. `medieval_hexagon/wall_straight` ne fait que
    // 1,10 m — le joueur saute 1,174 m et passerait par-dessus les cloisons.
    //
    // ⚠️ JAMAIS sur une coquille AUTORÉE. `forge_sanctum` pose son marché à
    // x = 18 m, exactement là où tomberait le mur est du complexe — et son
    // beffroi, son puits et sa taverne composent une place pensée à la main.
    // Générer par-dessus, ce serait bulldozer un travail de design pour le
    // remplacer par une grille. Le complexe est fait pour les stages SANS
    // agencement autoré ; `suppress_procedural_modules` dit déjà exactement ça.
    // `suppress_procedural` a déjà été calculé plus haut depuis le même layout —
    // le relire évite un second emprunt ET une seconde source de vérité.
    let authored_suppresses = suppress_procedural;
    let rooms_cfg = rooms::RoomsConfig::load_or_default();
    let room_plan = if authored_suppresses {
        rooms::RoomPlan::default()
    } else {
        rooms::plan_rooms(extent, req.seed, &rooms_cfg)
    };
    if !room_plan.walls.is_empty() {
        let room_wall_glb = ramparts_wall_glb("kaykit_dungeon");
        let room_scene: Handle<Scene> = asset_server.load(format!("{room_wall_glb}#Scene0"));
        let mut room_instances: Vec<(u8, Transform)> = Vec::new();
        for seg in &room_plan.walls {
            // Visuel : modules à taille naturelle, fusionnés.
            for (pos, yaw) in seg.module_poses() {
                room_instances.push((
                    0u8,
                    Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
                ));
            }
            // Physique : UN cuboïde par tronçon (pas un par module) — les bots
            // sondent par raycast, 30 boîtes coûtent bien moins que 120.
            let half = seg.collider_half_extents(
                RAMPARTS_WALL_HEIGHT_M,
                RAMPARTS_WALL_THICKNESS_M,
            );
            commands.spawn((
                Name::new("RoomWallCollider"),
                StageArenaMarker,
                Transform::from_xyz(
                    seg.center_x,
                    RAMPARTS_WALL_HEIGHT_M * 0.5,
                    seg.center_z,
                ),
                GlobalTransform::default(),
                RigidBody::Fixed,
                Collider::cuboid(half.x, half.y, half.z),
            ));
        }
        props_spawned += room_instances.len() as u32;
        floor_merge::spawn_static_merge(
            &mut commands,
            "rooms",
            room_instances,
            vec![room_scene],
            extent,
            &req.stage_id,
        );
        info!(
            "[stage-arena] complexe : {} pièces de {:.0} m, portes {:.0} m, {} tronçons",
            room_plan.room_centers.len(),
            room_plan.cell_m,
            room_plan.corridor_w_m,
            room_plan.walls.len()
        );
    }

    // 3. PlayerSpawn anchor (just a marker entity, no visual).
    //
    // Story-682 — la position vient de l'AGENCEMENT AUTORÉ quand il en déclare
    // une. Elle était en dur à `(0,0,0)` pour tous les stages, pendant que
    // `forge_sanctum` posait délibérément un puits solide à `[0,0,0]` : le
    // joueur apparaissait DEDANS, bloqué. Une carte qui met une pièce maîtresse
    // en son centre doit pouvoir dire où l'on démarre.
    let spawn_pos = authored_layout
        .map(|l| l.spawn_pos())
        .unwrap_or(Vec3::ZERO);
    commands.spawn((
        Name::new("StagePlayerSpawn"),
        StageArenaMarker,
        AnchorPoint::new(AnchorKind::PlayerSpawn, 0),
        Transform::from_translation(spawn_pos),
        GlobalTransform::default(),
    ));
    anchor_stats.record(AnchorKind::PlayerSpawn);

    // 4. POI anchors + prefabs (pondéré).
    let poi_positions = poi_anchor_positions(extent, stage_def.anchor_slots, req.seed);
    let pois_pool: Vec<(&String, &PoiDef)> = pois_genome
        .data
        .pois
        .iter()
        .filter(|(_, p)| p.weight > 0)
        .collect();
    let mut rng_state = req.seed.wrapping_mul(0xDEAD_BEEF_C0DE_F00D);
    for (slot, pos) in poi_positions.iter().enumerate() {
        let Some(poi) = pick_poi_weighted(&pois_pool, &mut rng_state) else {
            break;
        };
        let spawn = PrefabSpawn::new(&poi.prefab, *pos).with_name(format!("POI_{slot}"));
        let _e = spawn_gltf_prefab(
            &mut commands,
            &asset_server,
            &prefab_stats,
            spawn,
            (StageArenaMarker,),
        );
        commands.spawn((
            Name::new(format!("StagePoiAnchor_{slot}")),
            StageArenaMarker,
            AnchorPoint::new(AnchorKind::PoiSlot, slot as u32),
            Transform::from_translation(*pos),
            GlobalTransform::default(),
        ));
        anchor_stats.record(AnchorKind::PoiSlot);
        props_spawned += 1;

        // Glow d'ambiance cartoon : pool de lumière chaude au-dessus du POI.
        commands.spawn((
            Name::new(format!("StagePoiGlow_{slot}")),
            StageArenaMarker,
            PointLight {
                color: glow_color,
                intensity: glow_intensity,
                range: glow_range,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(*pos + Vec3::Y * 1.8),
        ));
        ambiance_lights += 1;
    }

    // 5. BossPad if required — pick the POI tagged encounter="boss".
    if stage_def.boss_pad_required {
        if let Some(boss) = pois_genome
            .data
            .pois
            .values()
            .find(|p| p.encounter == "boss")
        {
            let boss_pos = Vec3::new(0.0, 0.0, -extent * 0.7);
            let spawn = PrefabSpawn::new(&boss.prefab, boss_pos)
                .with_scale((boss.size_m / 4.0).max(1.0))
                .with_name("BossPad");
            let _e = spawn_gltf_prefab(
                &mut commands,
                &asset_server,
                &prefab_stats,
                spawn,
                (StageArenaMarker,),
            );
            commands.spawn((
                Name::new("StageBossPadAnchor"),
                StageArenaMarker,
                AnchorPoint::new(AnchorKind::BossPad, 0),
                Transform::from_translation(boss_pos),
                GlobalTransform::default(),
            ));
            anchor_stats.record(AnchorKind::BossPad);
            props_spawned += 1;

            // Glow focal BossPad : plus intense (×1.4) + plus haut pour signaler
            // le point d'arène majeur (lisibilité, audit §5 telegraph boss).
            commands.spawn((
                Name::new("StageBossPadGlow"),
                StageArenaMarker,
                PointLight {
                    color: glow_color,
                    // Clamp à 8k : garde-fou Atmosphere (un PointLight plein-intensité
                    // casse le bloom — dette 2026-05-18). Robuste si un biome futur
                    // pousse la base trop haut.
                    intensity: (glow_intensity * 1.4).min(8_000.0),
                    range: glow_range * 1.3,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_translation(boss_pos + Vec3::Y * 2.5),
            ));
            ambiance_lights += 1;
        }
    }

    // 5.5 — Module placements (story-485 phase 5).
    // Place les modules de la palette du stage (cover/lanes/perch/pit) via
    // `layout::place_modules` (pur, déterministe via req.seed). Spawn prefab GLB
    // + AnchorPoint companion + record stats.
    let player_pos = Vec3::ZERO;
    let boss_pos = if stage_def.boss_pad_required {
        Some(Vec3::new(0.0, 0.0, -extent * 0.7))
    } else {
        None
    };
    let modules_genome_opt = layout_params.assets.get(&layout_params.handles.modules);
    let mut layout_placements: Vec<layout::ModulePlacement> = Vec::new();
    let mut instances_skipped: u32 = 0;
    let mut palette_used: Vec<String> = Vec::new();
    if suppress_procedural {
        info!(
            "[stage-arena] Authored layout '{}' present -> scatter procedural de modules coupe",
            req.stage_id
        );
    } else if let Some(modules_genome) = modules_genome_opt {
        let expected_total: u32 = stage_def.module_palette.iter().map(|e| e.count).sum();
        layout_placements = layout::place_modules(
            extent,
            &modules_genome.data.modules,
            &stage_def.module_palette,
            player_pos,
            boss_pos,
            req.seed,
        );
        instances_skipped = expected_total.saturating_sub(layout_placements.len() as u32);
        palette_used = stage_def
            .module_palette
            .iter()
            .filter(|e| e.count > 0)
            .map(|e| e.id.clone())
            .collect();

        for (idx, placement) in layout_placements.iter().enumerate() {
            let def = match modules_genome.data.modules.get(&placement.module_id) {
                Some(d) => d,
                None => continue,
            };
            let prop = match def.prop_palette.get(placement.prop_index) {
                Some(p) => p,
                None => continue,
            };
            let spawn = PrefabSpawn::new(&prop.prefab, placement.position)
                .with_name(format!("Module_{}_{idx}", placement.module_id));
            let _e = spawn_gltf_prefab(
                &mut commands,
                &asset_server,
                &prefab_stats,
                spawn,
                (StageArenaMarker,),
            );
            commands.spawn((
                Name::new(format!("StageModuleAnchor_{}_{idx}", placement.module_id)),
                StageArenaMarker,
                AnchorPoint::new(placement.anchor_kind, idx as u32),
                Transform::from_translation(placement.position),
                GlobalTransform::default(),
            ));
            anchor_stats.record(placement.anchor_kind);
            props_spawned += 1;
        }
        info!(
            "[stage-arena] Modules placed: {}/{} (skipped={}) palette={:?}",
            layout_placements.len(),
            expected_total,
            instances_skipped,
            palette_used
        );
    } else if !stage_def.module_palette.is_empty() {
        warn!(
            "[stage-arena] Stage '{}' has module_palette but LevelModulesGenome not yet loaded — skipping module placement this frame",
            req.stage_id
        );
    }

    // Compute layout metrics and store in LayoutResult Resource (consommé par
    // layout_sensor::write_layout_sensor).
    let longest_sightline_m =
        layout::longest_unbroken_sightline_m(player_pos, boss_pos, &layout_placements);
    let min_cover_spacing_m = layout::min_cover_cluster_spacing_m(&layout_placements);
    layout_params.result.stage_id = req.stage_id.clone();
    layout_params.result.placements = layout_placements;
    layout_params.result.longest_sightline_m = longest_sightline_m;
    layout_params.result.min_cover_spacing_m = min_cover_spacing_m;
    layout_params.result.instances_skipped = instances_skipped;
    layout_params.result.module_palette_used = palette_used;
    layout_params.result.authored_pieces = authored_pieces_spawned;
    layout_params.result.authored_sections = authored_sections.len() as u32;
    layout_params.result.layout_source = if authored_pieces_spawned > 0 {
        "authored".to_string()
    } else {
        "procedural".to_string()
    };

    // 6. Lighting cartoon biome-tuned — bible v1 direction artistique
    //    family-friendly. Pattern key+fill : Sun (key, shadows) + Fill
    //    (opposé, no shadows, couleur ambient) = jamais pitch black sans
    //    Resource AmbientLight (deprecated Resource→Component Bevy 0.18).
    //    + ClearColor sky teinté biome.
    let (sun_color, sun_illuminance, fill_color, fill_illuminance, sky_color) =
        biome_lighting_params(&stage_def.biome);

    commands.spawn((
        Name::new("StageSun"),
        StageArenaMarker,
        DirectionalLight {
            color: sun_color,
            illuminance: sun_illuminance,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(50.0, 100.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        Name::new("StageFill"),
        StageArenaMarker,
        DirectionalLight {
            color: fill_color,
            illuminance: fill_illuminance,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-50.0, 60.0, -50.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(ClearColor(sky_color));

    // Finalize result.
    result.state = StageState::Ready;
    result.stage_id = req.stage_id.clone();
    result.biome = stage_def.biome.clone();
    result.sky_key = req.sky.clone().unwrap_or_default();
    result.extent_m = extent;
    result.anchors_placed = anchor_stats.total();
    result.props_spawned = props_spawned;
    result.ambiance_lights = ambiance_lights;
    result.error_message.clear();
    result.wall_natural_len_used = wall_len;
    result.walls_per_segment = ((hex_side_len / wall_len).ceil() as u32).max(1);
    result.music_state_id = stage_def.music_state.clone().unwrap_or_default();
    result.weather_override = stage_def.weather_override.clone().unwrap_or_default();
    *last_processed_id = req.stage_id.clone();
    info!(
        "[stage-arena] Stage '{}' READY: {} props, {} anchors, walls/segment={} wall_len={:.2}m",
        req.stage_id,
        props_spawned,
        anchor_stats.total(),
        result.walls_per_segment,
        wall_len,
    );
}

/// Despawn helper inline — réutilisé par le system wrapper `cleanup_stage_arena`
/// ET par `spawn_stage_arena_on_request` lors d'une transition stage_id→stage_id.
/// Retourne le nombre d'entités despawn.
pub fn despawn_stage_entities(
    commands: &mut Commands,
    query: &Query<Entity, With<StageArenaMarker>>,
    stats: &AnchorStats,
    result: &mut StageLoadResult,
) -> u32 {
    let mut n = 0u32;
    for e in query.iter() {
        commands.entity(e).despawn();
        n += 1;
    }
    forgia_anchor::reset_anchor_stats(stats);
    *result = StageLoadResult::default();
    n
}

/// Cleanup — caller calls this OnExit(GameMode::Roguelite) ou avant
/// transition stage→stage. Despawn toutes les entités StageArenaMarker + reset
/// AnchorStats + reset StageLoadResult.
pub fn cleanup_stage_arena(
    mut commands: Commands,
    q: Query<Entity, With<StageArenaMarker>>,
    stats: Res<AnchorStats>,
    mut result: ResMut<StageLoadResult>,
) {
    let n = despawn_stage_entities(&mut commands, &q, &stats, &mut result);
    // Story-600 (re-appliqué 2026-06-17) : retirer NOTRE propre requête de spawn —
    // sinon le système ungated `spawn_stage_arena_on_request` la revoit à la frame
    // suivante (state reset ≠ Ready) et RE-spawne le stage Roguelite dans le mode
    // suivant (arène crypts_of_anvil dans CyberCity, confirmé runtime 2026-06-17).
    commands.remove_resource::<StageLoadRequest>();
    if n > 0 {
        info!("[stage-arena] cleanup: despawned {n} stage entities + StageLoadRequest retirée");
    }
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Garde-fou Atmosphere : le glow POI (base) ne dépasse jamais 8k lumens
    /// (un PointLight plein-intensité casse l'Atmosphere/bloom — dette 2026-05-18).
    /// Le BossPad applique ×1.4 PUIS clamp `.min(8000)` au spawn — vérifié ici aussi.
    #[test]
    fn ambiance_glow_intensity_bounded_for_atmosphere() {
        for biome in [
            "Volcanic", "Plains", "Desert", "Forest", "Jungle", "Tundra", "Unknown",
        ] {
            let (_color, intensity, range) = biome_ambiance_glow(biome);
            // POI glow utilise la base directement.
            assert!(
                intensity <= 8_000.0,
                "{biome}: glow POI {intensity} dépasse la borne Atmosphere 8k"
            );
            // BossPad glow = base ×1.4 clampé à 8k.
            let boss = (intensity * 1.4).min(8_000.0);
            assert!(boss <= 8_000.0, "{biome}: glow boss {boss} dépasse 8k");
            assert!(intensity > 0.0, "{biome}: glow nul");
            assert!(
                (5.0..=20.0).contains(&range),
                "{biome}: range {range} hors plage"
            );
        }
    }

    #[test]
    fn state_as_str_round_trip() {
        assert_eq!(StageState::Idle.as_str(), "idle");
        assert_eq!(StageState::Loading.as_str(), "loading");
        assert_eq!(StageState::Ready.as_str(), "ready");
        assert_eq!(StageState::Error.as_str(), "error");
    }

    #[test]
    fn severity_idle_is_info() {
        assert_eq!(severity_for_stage(StageState::Idle, 0.0), "info");
    }

    #[test]
    fn severity_loading_short_is_info() {
        assert_eq!(severity_for_stage(StageState::Loading, 2.0), "info");
    }

    #[test]
    fn severity_loading_sustained_is_warn() {
        assert_eq!(severity_for_stage(StageState::Loading, 6.0), "warn");
    }

    #[test]
    fn severity_ready_is_ok() {
        assert_eq!(severity_for_stage(StageState::Ready, 0.0), "ok");
    }

    #[test]
    fn severity_error_is_critical() {
        assert_eq!(severity_for_stage(StageState::Error, 0.0), "critical");
    }

    #[test]
    fn next_step_idle_points_to_request_action() {
        let s = next_step_for_stage(StageState::Idle, 0.0);
        assert!(s.contains("StageLoadRequest"));
    }

    #[test]
    fn next_step_loading_sustained_points_to_prefab_sensor() {
        let s = next_step_for_stage(StageState::Loading, 7.0);
        assert!(s.contains("forgia_prefab.json"));
        assert!(s.contains("roguelite_stages.toml"));
    }

    #[test]
    fn next_step_error_points_to_error_message() {
        let s = next_step_for_stage(StageState::Error, 0.0);
        assert!(s.contains("error_message"));
    }

    #[test]
    fn ramparts_hex_yields_6_segments() {
        let segs = ramparts_hex_positions(50.0);
        assert_eq!(segs.len(), 6);
    }

    #[test]
    fn ramparts_hex_zero_extent_yields_empty() {
        assert!(ramparts_hex_positions(0.0).is_empty());
        assert!(ramparts_hex_positions(-10.0).is_empty());
    }

    #[test]
    fn ramparts_tiled_zero_yields_empty() {
        assert!(ramparts_hex_tiled_positions(0.0, 4.0).is_empty());
        assert!(ramparts_hex_tiled_positions(50.0, 0.0).is_empty());
        assert!(ramparts_hex_tiled_positions(-10.0, 4.0).is_empty());
    }

    #[test]
    fn ramparts_tiled_count_matches_walls_per_segment() {
        // extent=8, wall_len=4 → 2 walls/segment × 6 = 12 total
        let pts = ramparts_hex_tiled_positions(8.0, 4.0);
        assert_eq!(pts.len(), 12);

        // extent=12, wall_len=4 → 3 walls/segment × 6 = 18 total
        let pts = ramparts_hex_tiled_positions(12.0, 4.0);
        assert_eq!(pts.len(), 18);

        // extent=4.1, wall_len=4 → ceil(1.025) = 2 walls/segment × 6 = 12 total
        let pts = ramparts_hex_tiled_positions(4.1, 4.0);
        assert_eq!(pts.len(), 12);
    }

    #[test]
    fn ramparts_tiled_walls_within_extent_radius() {
        // Tous les murs doivent être à distance ≤ extent du center (sur les côtés de l'hex).
        let extent = 30.0;
        let pts = ramparts_hex_tiled_positions(extent, 4.0);
        assert!(!pts.is_empty());
        for (p, _) in &pts {
            let d = (p.x * p.x + p.z * p.z).sqrt();
            // Apothème hex = extent * sqrt(3)/2 ≈ extent * 0.866 (min dist sur segments)
            // Vertex hex = extent (max dist sur segments)
            assert!(
                d <= extent * 1.001,
                "tile {p:?} d={d} exceeds extent {extent}"
            );
            assert!(p.y.abs() < 1e-6, "y not zero: {}", p.y);
        }
    }

    #[test]
    fn ramparts_yaw_aligns_x_axis_with_segment() {
        // Garde-fou contre la régression "murs vus par la tranche" :
        // l'axe X local du mur DOIT pointer le long du segment.
        // Convention KayKit GLTF : X=width (long axis du wall.glb).
        let pts = ramparts_hex_tiled_positions(60.0, 4.0);
        assert!(pts.len() >= 6);
        // Pour les 6 premiers walls (1er de chaque segment) :
        // - rotation appliquée à Vec3::X doit pointer dans la direction segment
        let extent = 60.0;
        let mut verts = [Vec3::ZERO; 6];
        for (i, v) in verts.iter_mut().enumerate() {
            let a = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * i as f32;
            *v = Vec3::new(extent * a.cos(), 0.0, extent * a.sin());
        }
        let walls_per_seg = pts.len() / 6;
        for seg in 0..6 {
            let dir = (verts[(seg + 1) % 6] - verts[seg]).normalize();
            let (_pos, rot) = pts[seg * walls_per_seg];
            let x_world = rot * Vec3::X;
            // Dot product → 1.0 si même direction.
            let dot = x_world.dot(dir);
            assert!(
                dot > 0.99,
                "segment {seg}: X local doit aligner sur segment direction, dot={dot}"
            );
        }
    }

    #[test]
    fn ramparts_yaw_consistent_between_helpers() {
        // ramparts_hex_positions (midpoints, used for colliders) doit avoir
        // la même rotation que ramparts_hex_tiled_positions (visual tiles).
        let mids = ramparts_hex_positions(50.0);
        let tiles = ramparts_hex_tiled_positions(50.0, 4.0);
        let walls_per_seg = tiles.len() / 6;
        for seg in 0..6 {
            let mid_rot = mids[seg].1;
            let tile_rot = tiles[seg * walls_per_seg].1;
            let diff = (mid_rot * tile_rot.inverse()).to_euler(EulerRot::YXZ).0;
            assert!(
                diff.abs() < 1e-3,
                "segment {seg}: midpoint rotation != tile rotation, diff={diff}"
            );
        }
    }

    #[test]
    fn ramparts_tiled_walls_distributed_per_segment() {
        // Sur extent=8 wall_len=4 → 2 walls/segment, ils doivent être à t=0.25 et t=0.75
        // le long de chaque segment (centrés sur leurs slots).
        let pts = ramparts_hex_tiled_positions(8.0, 4.0);
        // Walls 0 et 1 sont sur le segment 0. Leur distance pairwise = wall_len.
        let d01 = pts[0].0.distance(pts[1].0);
        assert!(
            (d01 - 4.0).abs() < 1e-3,
            "walls dans même segment doivent être à distance wall_len={}, mesuré {}",
            4.0,
            d01
        );
    }

    #[test]
    fn ramparts_hex_segments_form_closed_polygon() {
        let segs = ramparts_hex_positions(30.0);
        // Centres tous à même distance du center
        let r0 = (segs[0].0.x.powi(2) + segs[0].0.z.powi(2)).sqrt();
        for (p, _) in &segs {
            let r = (p.x.powi(2) + p.z.powi(2)).sqrt();
            assert!((r - r0).abs() < 1e-3, "segment off-radius: {r} vs {r0}");
        }
        // Y = 0 partout (WALL_Y LOCK)
        for (p, _) in &segs {
            assert!(p.y.abs() < 1e-6, "y not zero: {}", p.y);
        }
    }

    #[test]
    fn poi_anchors_zero_slots_yields_empty() {
        assert!(poi_anchor_positions(50.0, 0, 42).is_empty());
    }

    #[test]
    fn poi_anchors_count_matches_slots() {
        let pts = poi_anchor_positions(50.0, 6, 42);
        assert_eq!(pts.len(), 6);
    }

    #[test]
    fn poi_anchors_deterministic_per_seed() {
        let a = poi_anchor_positions(50.0, 6, 42);
        let b = poi_anchor_positions(50.0, 6, 42);
        assert_eq!(a, b, "same seed must yield same layout");
    }

    #[test]
    fn poi_anchors_differ_with_seed() {
        let a = poi_anchor_positions(50.0, 6, 42);
        let b = poi_anchor_positions(50.0, 6, 99);
        assert_ne!(a, b, "different seed should yield different layout");
    }

    #[test]
    fn poi_anchors_within_radius() {
        let pts = poi_anchor_positions(100.0, 8, 7);
        for p in &pts {
            let r = (p.x.powi(2) + p.z.powi(2)).sqrt();
            assert!(r <= 55.5, "POI {p:?} outside radius_factor*extent");
        }
    }

    #[test]
    fn splitmix64_deterministic() {
        let mut a = 42u64;
        let mut b = 42u64;
        for _ in 0..10 {
            assert_eq!(splitmix64(&mut a), splitmix64(&mut b));
        }
    }

    #[test]
    fn ramparts_shape_default_is_hexagonal() {
        assert_eq!(RampartsShape::default(), RampartsShape::Hexagonal);
    }

    #[test]
    fn stage_def_toml_parse_minimal() {
        let toml_str = r#"
            biome = "Volcanic"
            arena_extent_m = 90.0
            ramparts_kit = "kaykit_dungeon"
            anchor_slots = 6
            boss_pad_required = true
        "#;
        let def: StageDef = toml::from_str(toml_str).expect("parse StageDef");
        assert_eq!(def.biome, "Volcanic");
        assert_eq!(def.arena_extent_m, 90.0);
        assert_eq!(def.anchor_slots, 6);
        assert!(def.boss_pad_required);
        assert_eq!(def.ramparts_shape, RampartsShape::Hexagonal);
        assert!(def.music_state.is_none());
    }

    #[test]
    fn stage_def_without_palette_defaults_empty() {
        // Story-485 phase 3 — rétro-compat : stage sans module_palette = Vec vide
        let toml_str = r#"
            biome = "Volcanic"
            arena_extent_m = 90.0
            ramparts_kit = "kaykit_dungeon"
            anchor_slots = 6
            boss_pad_required = false
        "#;
        let def: StageDef = toml::from_str(toml_str).expect("parse");
        assert!(
            def.module_palette.is_empty(),
            "module_palette must default to empty for backward compat"
        );
    }

    #[test]
    fn stage_def_with_module_palette_parses() {
        // Story-485 phase 3 — palette annotation parses correctly
        let toml_str = r#"
            biome = "Volcanic"
            arena_extent_m = 90.0
            ramparts_kit = "kaykit_dungeon"
            anchor_slots = 6
            boss_pad_required = true
            module_palette = [
                { id = "cover_low_cluster", count = 3 },
                { id = "sniper_perch",      count = 1 },
            ]
        "#;
        let def: StageDef = toml::from_str(toml_str).expect("parse with palette");
        assert_eq!(def.module_palette.len(), 2);
        assert_eq!(def.module_palette[0].id, "cover_low_cluster");
        assert_eq!(def.module_palette[0].count, 3);
        assert_eq!(def.module_palette[1].id, "sniper_perch");
        assert_eq!(def.module_palette[1].count, 1);
    }

    #[test]
    fn stages_genome_toml_parse_multi() {
        let toml_str = r#"
            [stages.crypts_of_anvil]
            biome = "Volcanic"
            arena_extent_m = 90.0
            ramparts_kit = "kaykit_dungeon"
            anchor_slots = 6
            boss_pad_required = true

            [stages.forge_sanctum]
            biome = "Plains"
            arena_extent_m = 80.0
            ramparts_kit = "medieval_hexagon"
            anchor_slots = 5
            boss_pad_required = false
        "#;
        let g: RogueliteStagesGenome = toml::from_str(toml_str).expect("parse genome");
        assert_eq!(g.stages.len(), 2);
        assert!(g.stages.contains_key("crypts_of_anvil"));
        assert!(g.stages.contains_key("forge_sanctum"));
    }

    #[test]
    fn poi_def_toml_parse() {
        let toml_str = r#"
            weight = 50
            prefab = "models/kaykit/dungeon/chest_basic.glb"
            size_m = 4.0
        "#;
        let p: PoiDef = toml::from_str(toml_str).expect("parse PoiDef");
        assert_eq!(p.weight, 50);
        assert_eq!(p.encounter, "none"); // default
        assert_eq!(p.size_m, 4.0);
    }
}
