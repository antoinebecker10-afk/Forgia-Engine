//! # forgia-rpg
//!
//! RPG OpenWorld mode plugin. Spawns world OnEnter(GameMode::Rpg), cleanup OnExit.
//!
//! W1 (2026-05-15) — Heightmap-grid via `forgia-terrain` (industry RPG pattern :
//! Skyrim / Witcher 3 / Horizon) :
//! - 1 chunk static à l'origine, échantillonné via `heightmap_at` du pipeline
//!   noise V1 (multi-octave + redistribution biome + features).
//! - `Collider::heightfield` rapier3d natif.
//! - `BiomeMap` Voronoi (10 biomes) prêt — W3 active les vertex colors variés.
//! - 2 buildings + 5 typed NPCs with InteractablePoint + dialogue trees.
//!
//! W2 : streaming N chunks autour joueur. W3 : Voronoi 10 biomes. W4 : preset_island.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_audio::prelude::AudioSampleOffset;
use forgia_core::prelude::*;
use forgia_rpg_data::dialogue::{
    DialogueChoice, DialogueEffect, DialogueId, DialogueNode, DialogueRegistry, DialogueSession,
    DialogueTree, NodeId, StartDialogue,
};
use forgia_rpg_data::shop::ShopSession;
use forgia_damage::Health;
use forgia_rpg_data::gold::Gold;
use forgia_rpg_data::inventory::{Inventory, UseItemRequest};
use forgia_rpg_data::items::ItemRegistry;
use forgia_rpg_data::quests::{
    Objective, QuestCatalogue, QuestDef, QuestId, QuestLog, QuestStatus, TrackedQuest,
};
use forgia_rpg_data::xp_curves::XpProgress;

/// Or de départ du joueur RPG. TODO(economy-genome) : déplacer vers un genome
/// économie data-driven (Phase ultérieure) plutôt qu'une const.
const STARTING_GOLD: u32 = 50;
use forgia_foliage::prelude::VegetationManager;
use forgia_foliage::{FoliageExclusionDisc, RpgSampleOffset, VegetationTree};
use forgia_genome_village::VillageGenome;
use forgia_input::PlayerAction;
use forgia_player::prelude::Player;
use forgia_streaming::{
    EvictionEvent, EvictionReason, StreamingConfig, StreamingPause, StreamingStats,
};
use forgia_terrain::{
    build_chunk_mesh, build_path_segment, spawn_chunk_entity, BiomeMap, ChunkCoord, ChunkManager,
    FlattenZones, Lod2TileManager, LodSampleOffset, LodStats, MapGenConfig, PathNetwork, RoadTier,
    TerrainConfig, TerrainSharedMaterial, VillageFlattenZone, CHUNK_X, CHUNK_Z,
};
use forgia_village_loader::{
    cleanup_village, LoadVillageGenomeRequest, LoadVillageRequest, VillageLoadResult,
};
use leafwing_input_manager::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

pub mod character;
pub mod npc_pose;
pub mod worldgen_village;
// proc_walk déplacé vers forgia-anim-locomotion (story-482 P1)

pub mod prelude {
    pub use crate::{ForgiaRpgPlugin, InteractablePoint, Npc, RpgWorldMarker};
}

/// Marker for everything spawned by the RPG world (used by cleanup).
#[derive(Component)]
pub struct RpgWorldMarker;

#[derive(Component)]
pub struct Npc {
    pub name: String,
    pub greeting: String,
}

#[derive(Component)]
pub struct InteractablePoint {
    pub label: String,
    pub radius: f32,
}

/// Lien explicite PNJ → quêtes (pour marqueurs ! / ? story-58x Phase 4).
/// Greffé à côté de `Npc` sur les donneurs de quête (ex Maître Forgeron).
#[derive(Component, Default)]
pub struct QuestGiver {
    /// Quêtes proposées par ce PNJ (marqueur `!` si pas encore démarrées).
    pub offers: Vec<QuestId>,
    /// Quêtes à rendre à ce PNJ (marqueur `?` si `Completed`).
    pub completes: Vec<QuestId>,
}

/// Ancre du village (≈ puits central) en coordonnées monde, insérée par
/// `spawn_world` OnEnter(Rpg). Sert de point fixe trouvable pour le spawn des
/// PNJ on-brand en arc (story-570) — au lieu d'un placement relatif au joueur
/// qui s'éloigne. `center.y` = hauteur terrain flattené au centre village.
#[derive(Resource, Debug, Clone, Copy)]
pub struct RpgVillageAnchor {
    pub center: Vec3,
}

// ── W1/W2 — World layout constants ──────────────────────────────────────
// `sample_offset` = ZERO depuis 2026-05-19. La fn `heightmap_at` a été
// migrée vers la convention "world centré sur (0,0), bords à ±half" (cf
// heightmap.rs:276 doc). L'ancien offset map_size/2 servait à compenser la
// convention [0, map_size] obsolète ; combiné à la nouvelle fn, il plaçait
// world (0,0) PILE au coin du noise valide → spawn sur edge_factor=0 →
// terrain flat à sea_level côté +X/+Z (visible sous bevy_water = "lac
// asymétrique"). Voir test heightmap.rs:425 edge_falloff(19,19,...) qui
// confirme la nouvelle convention.
const RPG_MAP_SIZE: f32 = 2048.0;
const RPG_SEED: u32 = 1337;
/// Public depuis 2026-05-20 : forgia-water lit `SeaLevel` Resource insérée
/// par ce plugin, plus de hardcode dupliqué cross-crate (cf bug Arena 2026-05-12).
pub const RPG_SEA_LEVEL: f32 = 4.0;
// 2026-06-05 (story-574) — relief dramatique. Lignée : audit procgen → B2 a
// confirmé que max_height=28 était une baseline PLATE délibérée du vertical
// slice (pas un bug) ; user a explicitement choisi un relief plus marqué.
// 28 → 80 : amplitude 4..80m (×3.2 vs 24m). RPG_SEA_LEVEL inchangé À DESSEIN :
// `heightmap_at` fait `h = sea + h_norm*(max-sea)` avec h_norm>=0, donc le
// terrain ne descend JAMAIS sous sea_level (plancher = sea_level) → l'eau reste
// flush quel que soit sea_level, aucune cascade swim/eau. Consommateurs
// auto-adaptés : snow band (max*0.75), biome altitude (gen_config.max_height),
// village flatten (target_y échantillonné). Const Rust = NON hot-reloadable :
// pour tuner la magnitude en live, voir option C (genome TOML).
const RPG_MAX_HEIGHT: f32 = 80.0;

/// Fallback Manhattan radius si `StreamingConfig` indisponible (boot frame 0,
/// avant que load_streaming_genome ait inséré la Resource). Cible la couverture
/// LOD1 (320m) : ceil(LOD1_MAX_M / CHUNK_X) = 10. Au-delà : LOD2 mega-tiles.
/// Story-450 Wave 2 : la vraie source est `Res<StreamingConfig>`.
const FALLBACK_RENDER_DIST: i32 = 10;
/// Fallback chunks/frame si config absente. Override par `StreamingConfig.async_pipeline`.
const FALLBACK_CHUNKS_PER_FRAME: usize = 4;
/// Estimation mémoire par chunk chargé (MB). DÉRIVÉE du mesh uniforme 33×33
/// (audit procgen 2026-06-05, corrige sensor ×25) : positions(13ko)+uvs(8.7ko)
/// +colors(17.4ko)+normals(13ko)+indices(24.6ko)+heights(4.4ko) ≈ 81ko CPU +
/// ~73ko upload GPU ≈ 0.15-0.16 MB/chunk. L'ancien 5.0 surestimait ×25 → budget
/// eviction faussement conservateur. Comme tous les chunks ont la MÊME topo,
/// cette constante EST la valeur réelle (pas un proxy). Raffinage GPU-exact = P2.
const CHUNK_MB_EST: f32 = 0.16;
/// W2 — intervalle d'export sensor JSON (secondes).
const SENSOR_INTERVAL_S: f32 = 1.0;

/// Modes où Rex est piloté en 3e personne : le RPG, et la démo perf "Cyber City"
/// (2026-06-15) qui réutilise le pipeline locomotion + caméra orbitale de Rex
/// pour se déplacer dans la ville. Élargit le gating sans dupliquer la chaîne
/// d'animation mono-perso (`.single()`) ni spawner le monde RPG (terrain/village
/// restent gatés strictement `OnEnter(Rpg)`, le lineup PNJ no-op sans anchor).
fn rex_third_person_active(state: Res<State<GameMode>>) -> bool {
    matches!(state.get(), GameMode::Rpg | GameMode::CyberCity)
}

pub struct ForgiaRpgPlugin;

impl Plugin for ForgiaRpgPlugin {
    fn build(&self, app: &mut App) {
        // Story-440 R&D 2026-05-17 night : re-enable ForgiaAutoRigPlugin avec
        // backend PinocchioV1 (voxelize → medial axis → embed template).
        // Skinning DISABLE en interne du plugin → bones gizmos visibles sans
        // déformer le mesh visuel. ProcBodyAnim/locomotion restent disable.
        // Validation visuelle bones d'abord, anim Phase 2+ après.
        app.add_plugins(forgia_auto_rig::ForgiaAutoRigPlugin);
        // Rendu final Rex (2026-06-07) : masque l'overlay debug du rig — rings +
        // lignes gizmos OFF, et Rex reste OPAQUE (rex_make_transparent_one_shot
        // se gate sur ce même flag). Le sensor forgia_rig_bones.json continue
        // d'écrire (draw_rig_gizmos ne gate que le visuel). Flip à `true` pour
        // re-déboguer le squelette à travers le mesh.
        app.insert_resource(forgia_auto_rig::AutoRigGizmosConfig {
            enabled: false,
            ..Default::default()
        });
        // Story-450 wave 2 FIX 2026-05-28 — `SeaLevel(pub f32)` Resource
        // désormais défini dans forgia-water::prelude. Insert ici sync
        // WaterSettings.height via sync_sea_level_resource. Plus de hardcode
        // cross-crate dupliqué.
        app.insert_resource(forgia_water::prelude::SeaLevel(RPG_SEA_LEVEL));
        app.init_resource::<character::TestCharacterMode>();
        // Story-450 wave 2 : residence tracking pour hystérèse unload UE5-style.
        app.init_resource::<ChunkResidence>();
        // Story-450 wave 4c : debug overlay toggle (F3).
        app.init_resource::<StreamingDebugOverlay>();
        app.init_resource::<forgia_anim_locomotion::RpgEntryCount>();
        app.add_systems(Startup, (register_sample_dialogues, register_sample_quests))
            .add_systems(
                OnEnter(GameMode::Rpg),
                (
                    forgia_anim_locomotion::increment_rpg_entry_count,
                    spawn_world,
                )
                    .chain(),
            )
            .add_systems(
                OnExit(GameMode::Rpg),
                (character::cleanup_rex_character, cleanup_world, cleanup_village).chain(),
            )
            // Démo Cyber City (2026-06-15) : Rex 3P réutilise le pipeline RPG.
            // Cleanup dédié (despawn Rex + OrbitCamera, ré-active FpsCamera) — sans
            // cleanup_world/village (aucun monde RPG n'est spawné en CyberCity).
            .add_systems(
                OnExit(GameMode::CyberCity),
                character::cleanup_rex_character,
            )
            .add_systems(
                Update,
                (
                    spawn_village_paths_when_loaded,
                    teleport_player_to_terrain,
                    terrain_reload_input_system,
                    apply_terrain_reload, // Shift+F12 → régén live (AVANT stream)
                    stream_chunks_around_player,
                    enforce_chunk_memory_budget, // wave 3a — runs AFTER stream
                    toggle_streaming_overlay,    // wave 4c — F3 toggle
                    draw_streaming_debug_overlay, // wave 4c — gizmos
                    write_chunks_sensor,
                    grant_rpg_player_components,
                    interact_system,
                    apply_item_use,
                    rpg_cloud_drift_system,
                )
                    .chain()
                    .run_if(in_state(GameMode::Rpg)),
            )
            // Rex spawn one-shot (retry-jusqu'à-success : race avec spawn_player
            // dans un autre plugin OnEnter Rpg). Story-451 Phase 2 (2026-05-18) :
            // animation par-bone activée — Pinocchio skinning + proc walk cycle.
            .add_systems(
                Update,
                (
                    character::spawn_rex_character,
                    character::calibrate_rex_y_one_shot,
                    character::rex_make_transparent_one_shot,
                    character::spawn_character_lineup,
                    character::calibrate_lineup_y_and_height,
                    // Story-482 P1 : locomotion systems déplacés vers
                    // forgia-anim-locomotion. Imports via use forgia_anim_locomotion::*.
                    forgia_anim_locomotion::apply_stance_offsets_from_template,
                    forgia_anim_locomotion::attach_locomotion_bones,
                    forgia_anim_locomotion::procedural_locomotion,
                    character::procedural_whole_body_anim,
                    // Story-482 P3 : foot IK (skip silencieux si bones shin/foot missing).
                    forgia_anim_locomotion::foot_ik_system,
                    forgia_anim_locomotion::write_foot_ik_sensor,
                    forgia_anim_locomotion::write_walk_pose_sensor,
                    forgia_anim_locomotion::write_rex_bones_live_sensor,
                    forgia_anim_locomotion::write_walk_dir_probe,
                    forgia_anim_locomotion::write_anim_full_sensor,
                )
                    .chain()
                    .in_set(GameSet::Movement)
                    // Rpg + démo Cyber City (Rex 3P réutilise ce pipeline).
                    .run_if(rex_third_person_active),
            )
            // Démo Cyber City : overlay debug rig ON (rings + transparence) pour
            // voir le squelette à travers Cyber, OFF en sortie → RPG reste opaque.
            .add_systems(OnEnter(GameMode::CyberCity), character::enable_rig_overlay)
            .add_systems(OnExit(GameMode::CyberCity), character::disable_rig_overlay)
            // Story-583 (2026-06-08) — pose idle « bras le long » des PNJ (sort la
            // T-pose). Système séparé du cœur locomotion (mono-perso .single) pour
            // ne PAS toucher l'anim de Rex. Idempotent (NpcPosed), retry jusqu'à
            // ce que l'auto-rig ait spawné les os.
            .add_systems(
                Update,
                npc_pose::sys_pose_npcs_idle
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Rpg)),
            )
            .init_resource::<character::LineupSpawned>()
            .init_resource::<forgia_anim_locomotion::WalkPoseSensorTimer>()
            .init_resource::<forgia_anim_locomotion::RexBonesLiveSensorTimer>()
            .init_resource::<forgia_anim_locomotion::FootIkConfig>()
            .init_resource::<forgia_anim_locomotion::FootIkStats>()
            .init_resource::<forgia_anim_locomotion::FootIkSensorTimer>()
            // 2026-06-04 : freeze révoqué (alignement squelette↔mesh validé en freeze).
            // Défaut OFF (Default=false) → Rex s'anime. Toggle F6 conservé pour re-figer
            // au bind si besoin de re-vérifier un alignement (mesh original non déformé).
            .init_resource::<forgia_anim_locomotion::AnimFreezeBind>()
            .add_systems(
                Update,
                forgia_anim_locomotion::toggle_anim_freeze_bind.run_if(in_state(GameMode::Rpg)),
            )
            .add_systems(
                bevy_egui::EguiPrimaryContextPass,
                (character::draw_lineup_names, draw_quest_markers)
                    .run_if(in_state(GameMode::Rpg)),
            )
            .add_systems(OnExit(GameMode::Rpg), character::cleanup_character_lineup)
            // Story-578 — village procédural via la toolbox forgia-worldgen, posé sur le terrain.
            .init_resource::<worldgen_village::RpgVillageState>()
            .add_systems(
                Update,
                (
                    worldgen_village::sys_spawn_worldgen_village,
                    worldgen_village::sys_clear_village_foliage,
                )
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Rpg)),
            )
            .add_systems(
                OnExit(GameMode::Rpg),
                worldgen_village::sys_cleanup_worldgen_village,
            );
    }
}

/// Returns the world-space offset applied when sampling the noise pipeline,
/// so that the visual origin (0,0,0) sees the interior of the procedural
/// world instead of `edge_falloff`-flattened borders.
fn sample_offset() -> Vec2 {
    // ZERO depuis migration heightmap centrée. Voir bloc de constantes
    // RPG_MAP_SIZE pour le raisonnement complet. Les 9 call-sites internes
    // restent compilants car ils additionnent juste Vec2::ZERO (no-op).
    Vec2::ZERO
}

/// Genome de forme du terrain (hot-reloadable : octaves / edge_falloff / max_height).
const TERRAIN_SHAPE_GENOME_PATH: &str = "config/genomes/terrain_shape.toml";

// Genome du village de spawn (terrain_leveling hot-reloadable + buildings).
const VILLAGE_GENOME_PATH: &str = "config/genomes/villages/starter_hamlet.toml";

fn make_terrain_config() -> TerrainConfig {
    // 2026-06-06 — la FORME du relief (octaves, edge_falloff, max_height) vient
    // désormais du genome `terrain_shape.toml` (data-driven, tunable sans rebuild).
    // Fallback = retune par défaut (rolling/traversable) si TOML absent/invalide.
    // `sea_level` reste sur RPG_SEA_LEVEL (couplé à la Resource SeaLevel/eau).
    let shape = forgia_terrain::TerrainShapeGenome::load_or_default(TERRAIN_SHAPE_GENOME_PATH);
    TerrainConfig {
        map_size: RPG_MAP_SIZE,
        seed: RPG_SEED,
        sea_level: RPG_SEA_LEVEL,
        max_height: shape.max_height,
        streaming_radius: 4,
        chunks_per_frame: 2,
        y_offset: 0.0,
        octaves: shape.octave_pairs(),
        edge_falloff_m: shape.edge_falloff_m,
        warp_strength: shape.warp_strength,
        warp_freq: shape.warp_freq,
    }
}

fn make_map_gen_config() -> MapGenConfig {
    // `preset_forgia_showcase` (default) utilise `BiomeMode::Directional` →
    // 5 zones cardinales rigides. On force `Voronoi` pour avoir 10 biomes
    // hexagonaux distribués naturellement (W3 ready).
    //
    // 2026-05-19 fix : `..MapGenConfig::default()` héritait TOUTES les
    // features du preset_forgia_showcase (6 Lakes depth 3-8, Craters depth 70,
    // Plateaus height 42, etc.) calibrées pour map_size=4096 / max_height=180
    // / sea_level=18. En RPG mode (max=28, sea=4), ces features créent des
    // cuvettes énormes (Lake depth=8 sur max=28 = 29% dip → terrain à
    // Y=-4 < sea=4 → bandes d'eau parasites mid-distance autour du spawn).
    // Fix : features RPG-adaptées light (à étoffer plus tard si besoin lacs).
    // MàJ 2026-06-05 (story-574) : RPG_MAX_HEIGHT 28→80 (relief dramatique).
    // features RESTENT vides : `heightmap_at` (le path mesh RPG, cf
    // meshing_heightmap.rs:77) ignore de toute façon gen_config.features ;
    // re-enable = pass dédié quand le besoin lacs/cratères viendra.
    MapGenConfig {
        seed: RPG_SEED,
        map_size: RPG_MAP_SIZE,
        sea_level: RPG_SEA_LEVEL,
        max_height: RPG_MAX_HEIGHT,
        island_mode: false, // pas d'île pour le vertical slice 1 chunk
        biome_mode: forgia_terrain::BiomeMode::Voronoi,
        biome_cell_size: 96.0, // cellules + petites → biomes visibles au W1 (32m chunk)
        features: vec![],      // preset features non-adaptées RPG max=28 → bandes d'eau parasites
        ..MapGenConfig::default()
    }
}

fn terrain_height_local(x: f32, z: f32, config: &TerrainConfig) -> f32 {
    let off = sample_offset();
    forgia_terrain::heightmap_at(x + off.x, z + off.y, config)
}

/// Story-447 Niveau A.5 — variante qui applique [`FlattenZones`] sur le Y du
/// terrain. Utilisée par le path ribbon mesh pour que les routes restent au
/// niveau du plateau dans le inner_radius, et descendent smoothement dans le
/// falloff. Sinon les routes traversent le plateau (rampe boueuse).
fn terrain_height_local_with_flatten(
    x: f32,
    z: f32,
    config: &TerrainConfig,
    flatten_zones: Option<&FlattenZones>,
) -> f32 {
    let raw = terrain_height_local(x, z, config);
    match flatten_zones {
        Some(z_def) => z_def.sample(x, z, raw),
        None => raw,
    }
}

/// Story-447 + 577 — construit la `FlattenZones` du village depuis le genome
/// `starter_hamlet.toml` (target_y = `heightmap_at(centre village)`, inner/falloff
/// = genome). Appelé par `spawn_world` (OnEnter) ET `apply_terrain_reload` (Shift+F12)
/// pour que `radius_m`/`falloff_m` soient hot-reloadables comme `max_height` — tuner
/// le « bowl » anti-mur du spawn en live. Vide si le genome désactive le leveling.
fn make_village_flatten_zones(terrain_cfg: &TerrainConfig) -> FlattenZones {
    let village_world_center = Vec2::new(CHUNK_X as f32 * 0.5, CHUNK_Z as f32 * 0.5);
    let off = sample_offset();
    let target_y = forgia_terrain::heightmap_at(
        village_world_center.x + off.x,
        village_world_center.y + off.y,
        terrain_cfg,
    );
    let mut flatten_zones = FlattenZones::new();
    // Story-578 — hex KayKit village : flat disc large enough for the hex grid + the player spawn
    // (world origin). Pushed UNCONDITIONALLY so the village is always flat, independent of the
    // legacy starter-hamlet genome below. Done before the chunks mesh (caller = spawn_world).
    flatten_zones.push(worldgen_village::village_flatten_zone(
        village_world_center,
        target_y,
    ));
    match VillageGenome::load_from_path(VILLAGE_GENOME_PATH) {
        Ok(genome) if genome.terrain_leveling.enabled => {
            flatten_zones.push(VillageFlattenZone {
                center: village_world_center,
                target_y,
                inner_radius: genome.terrain_leveling.radius_m,
                falloff_radius: genome.terrain_leveling.falloff_m,
            });
            info!(
                "[forgia-rpg] terrain_leveling : target_y={:.2}m, inner={}m, falloff={}m",
                target_y, genome.terrain_leveling.radius_m, genome.terrain_leveling.falloff_m
            );
        }
        Ok(_) => {
            info!("[forgia-rpg] terrain_leveling disabled (genome.terrain_leveling.enabled=false)")
        }
        Err(e) => {
            warn!("[forgia-rpg] terrain_leveling : genome load failed ({e}) — terrain restera brut")
        }
    }
    flatten_zones
}

// ── Hot-reload terrain live (Shift+F12) — story-576 increment 2 ───────────────

/// Requête de régén terrain posée par `terrain_reload_input_system` (Shift+F12),
/// consommée une fois par `apply_terrain_reload`.
#[derive(Resource)]
struct TerrainReloadRequest;

/// Shift+F12 → demande une régén live du terrain depuis `terrain_shape.toml`
/// (même touche que le hot-reload biomes/village : "recharger les genomes").
fn terrain_reload_input_system(keyboard: Res<ButtonInput<KeyCode>>, mut commands: Commands) {
    if keyboard.pressed(KeyCode::ShiftLeft) && keyboard.just_pressed(KeyCode::F12) {
        commands.insert_resource(TerrainReloadRequest);
        info!("[terrain-shape] hot-reload terrain demandé (Shift+F12)");
    }
}

/// Régén LIVE du terrain : recharge le genome → met à jour `TerrainConfig`
/// (octaves/edge/max) → despawn tous les chunks + LOD2 + clear `ChunkManager`
/// (cache inclus) → reset `last_player_chunk` (None). Le `stream_chunks_around_player`
/// qui suit dans la chaîne régénère le ring avec la nouvelle forme. Le player est
/// re-snappé sur le nouveau sol à son XZ courant (évite chute/burial transitoire).
/// Le village (FlattenZones + bâtiments) est laissé tel quel — cohérent entre eux ;
/// re-enter le RPG pour ré-aligner le village si on change beaucoup max_height.
#[allow(clippy::too_many_arguments)]
fn apply_terrain_reload(
    mut commands: Commands,
    request: Option<Res<TerrainReloadRequest>>,
    mut terrain_cfg: ResMut<TerrainConfig>,
    mut chunk_mgr: ResMut<ChunkManager>,
    mut lod2_mgr: ResMut<Lod2TileManager>,
    mut player_q: Query<&mut Transform, With<Player>>,
) {
    if request.is_none() {
        return;
    }
    commands.remove_resource::<TerrainReloadRequest>();

    // 1. Recharge le genome → update la config (consommée par heightmap_at).
    let shape = forgia_terrain::TerrainShapeGenome::load_or_default(TERRAIN_SHAPE_GENOME_PATH);
    terrain_cfg.max_height = shape.max_height;
    terrain_cfg.octaves = shape.octave_pairs();
    terrain_cfg.edge_falloff_m = shape.edge_falloff_m;
    terrain_cfg.warp_strength = shape.warp_strength;
    terrain_cfg.warp_freq = shape.warp_freq;
    // Re-publie aussi l'amplitude relief par biome → éditer un config/biomes/*.toml
    // (amplitude_mult) + Shift+F12 retune le relief par biome en live.
    forgia_terrain::publish_biome_amplitudes_from_config();
    // Story-577 : rebuild la FlattenZones du village (radius_m/falloff_m du genome
    // hot-reloadables comme max_height) → tuner le "bowl" anti-mur du spawn en live.
    // target_y re-échantillonné sur la nouvelle forme. Les chunks régénérés ci-dessous
    // + le foliage l'appliquent → cohérence mesh/arbres/village (pas d'arbre flottant).
    let flatten_zones = make_village_flatten_zones(&terrain_cfg);
    commands.insert_resource(flatten_zones.clone());

    // 2. Despawn tous les chunks trackés + LOD2, clear le ChunkManager (cache
    //    inclus → pas de restore d'anciennes hauteurs). last_player_chunk=None
    //    → stream_chunks_around_player régénère le ring avec la nouvelle forme.
    for entity in chunk_mgr.loaded_entities.values() {
        if let Ok(mut ec) = commands.get_entity(*entity) {
            ec.try_despawn();
        }
    }
    chunk_mgr.clear_all();
    lod2_mgr.despawn_all(&mut commands);

    // 3. Re-snap le player sur le nouveau sol à son XZ courant.
    if let Ok(mut tf) = player_q.single_mut() {
        let h = terrain_height_local_with_flatten(
            tf.translation.x,
            tf.translation.z,
            &terrain_cfg,
            Some(&flatten_zones),
        );
        tf.translation.y = h + 1.5;
    }

    info!(
        "[terrain-shape] terrain régénéré LIVE : max={:.0} edge={:.0} octaves={}",
        terrain_cfg.max_height,
        terrain_cfg.edge_falloff_m,
        terrain_cfg.octaves.len()
    );
}

/// W1 — Spawn 1 terrain chunk via forgia-terrain (heightmap-grid + Voronoi biomes).
/// W2 étendra à streaming N chunks autour du joueur.
fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    shared_mat: Option<Res<TerrainSharedMaterial>>,
) {
    let terrain_cfg = make_terrain_config();
    let map_cfg = make_map_gen_config();
    let biome_map = BiomeMap::generate(&terrain_cfg, Some(&map_cfg));
    // Publie la BiomeMap globale (lue par heightmap_at) → forme biome-aware
    // cohérente partout : mesh + foliage + village + spawn + décor reposent sur
    // le MÊME sol biome-aware (fix assets flottants story-576 inc.5).
    forgia_terrain::set_world_biome_map(biome_map.clone());
    // Publie aussi la table d'amplitude relief par biome (config/biomes/*.toml,
    // fallback hardcodé) → heightmap_at applique Mountain haut / Plains plat de
    // façon data-driven (story-576 inc.6).
    forgia_terrain::publish_biome_amplitudes_from_config();

    // Material partagé : fourni par ForgiaTerrainPlugin Startup, fallback lazy
    // si la session a été nettoyée OnExit (W2+).
    let terrain_mat_handle: Handle<StandardMaterial> = match shared_mat.as_ref() {
        Some(s) => s.0.clone(),
        None => {
            let diff: Handle<Image> = asset_server.load("textures-v1/terrain/grass/diff.jpg");
            let normal: Handle<Image> = asset_server.load("textures-v1/terrain/grass/normal.jpg");
            let rough: Handle<Image> = asset_server.load("textures-v1/terrain/grass/roughness.jpg");
            let h = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(diff),
                normal_map_texture: Some(normal),
                metallic_roughness_texture: Some(rough),
                perceptual_roughness: 0.90,
                reflectance: 0.05,
                ..default()
            });
            commands.insert_resource(TerrainSharedMaterial(h.clone()));
            h
        }
    };

    // ── Story-447 — village terrain leveling (calc AVANT chunk mesh) ────
    // FlattenZones depuis starter_hamlet.toml (cf make_village_flatten_zones,
    // partagé avec apply_terrain_reload pour hot-reload Shift+F12). Le
    // village-loader re-lira le genome plus tard pour spawner les buildings.
    let village_world_center = Vec2::new(CHUNK_X as f32 * 0.5, CHUNK_Z as f32 * 0.5);
    let flatten_zones = make_village_flatten_zones(&terrain_cfg);

    // ── 1 chunk static à l'origine (W1 vertical slice) ───────────────────
    let coord = ChunkCoord::new(0, 0);
    let mesh_data = build_chunk_mesh(
        coord,
        sample_offset(),
        &terrain_cfg,
        &biome_map,
        Some(&flatten_zones),
    );
    let mesh_handle = meshes.add(mesh_data.mesh.clone());
    let chunk_entity = spawn_chunk_entity(
        &mut commands,
        coord,
        mesh_data,
        mesh_handle,
        terrain_mat_handle,
    );
    // Tag cleanup OnExit + tracer dans ChunkManager pour la W2.
    commands.entity(chunk_entity).insert(RpgWorldMarker);
    let mut chunk_mgr = ChunkManager::default();
    chunk_mgr.loaded_entities.insert(coord, chunk_entity);

    // Story-570 : ancre village (≈ puits central) pour le spawn des PNJ on-brand.
    // `character::spawn_character_lineup` lit cette ancre et place les 4 PNJ en arc
    // autour (position fixe trouvable, pas relative au joueur). `well_y` = terrain
    // flattené au centre. TODO(story-445) : forgia-village-npc-spawner data-driven.
    {
        let off = sample_offset();
        let well_y = forgia_terrain::heightmap_at(
            village_world_center.x + off.x,
            village_world_center.y + off.y,
            &terrain_cfg,
        );
        commands.insert_resource(RpgVillageAnchor {
            center: Vec3::new(village_world_center.x, well_y, village_world_center.y),
        });
    }

    commands.insert_resource(terrain_cfg);
    commands.insert_resource(map_cfg);
    commands.insert_resource(biome_map);
    commands.insert_resource(chunk_mgr);
    // Story-447 — FlattenZones partagée : stream_chunks_around_player (W2) +
    // forgia-village-loader (Y des buildings) la consultent pour sampler le
    // heightmap leveled. Cleanup OnExit avec les autres ressources.
    commands.insert_resource(flatten_zones);
    // forgia-foliage + forgia-terrain LOD : aligne les samples (heightmap + biome)
    // avec notre décalage RPG (sample_offset = map_size/2).
    let off = sample_offset();
    commands.insert_resource(RpgSampleOffset { x: off.x, z: off.y });
    commands.insert_resource(LodSampleOffset { x: off.x, z: off.y });
    commands.insert_resource(AudioSampleOffset { x: off.x, z: off.y });
    // Marque qu'un teleport joueur doit firer ce cycle Rpg (consommé en Update).
    commands.insert_resource(PendingPlayerTeleport);
    // Story-450 wave 4a : active StreamingPause au boot RPG. Teleport gate
    // sur pause.active=false → garantit que le sim ring est chargé avant
    // que le joueur arrive sur place (pattern Roblox StreamingPauseMode).
    commands.insert_resource(StreamingPause {
        active: true,
        reason: "rpg_world_spawn".into(),
        waiting_chunks: 0, // mis à jour frame suivante par stream_chunks_around_player
    });

    // ── Village = système hexagonal KayKit (worldgen_village.rs) UNIQUEMENT ──
    // story-586 (2026-06-09) : le système legacy "village-loader" (genome procgen,
    // 6 bâtiments rouges + routes ribbon Bézier) est DÉBRANCHÉ du RPG — il rendait
    // des bâtiments DOUBLONS imbriqués dans le village hex au même centre (16,16),
    // colliders chevauchants. On ne pose plus `LoadVillageGenomeRequest` →
    // `process_village_genome_request` no-op → 0 bâtiment B, 0 `VillageLoadResult`,
    // 0 route ribbon. Ce que B fournissait seul est re-câblé : spawn joueur via
    // `RpgVillageAnchor` (cf `teleport_player_to_terrain`) + `FoliageExclusionDisc`
    // posé par le village hex (cf `sys_spawn_worldgen_village`). `PathNetwork` reste
    // vide (les routes hex de A sont des tuiles, pas des samples).
    commands.insert_resource(PathNetwork::default());

    // Caves removed 2026-05-17 PM — n'apportaient rien visuellement, le village
    // procgen est désormais le seul focal du spawn. La fonction `spawn_cave_entrance`
    // est conservée sous `#[allow(dead_code)]` pour réutilisation future
    // (donjons, points d'intérêt nature).

    // ── Sun (cartoon FPS-arena-style) ────────────────────────────────────
    commands.spawn((
        RpgWorldMarker,
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 22_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.35, -0.78, 0.0)),
        Name::new("RpgSun"),
    ));

    // ── Cartoon cloud clusters (orbit) — mêmes data que FPS arena ────────
    spawn_rpg_cartoon_clouds(&mut commands, &mut meshes, &mut materials);

    // ── Buildings : spawnés par `forgia-village-loader` (genome procgen). ──
    // ── NPCs : retirés story-446 (5 Capsule3d hardcodés (12,14)..(26,20))
    //          ne respectaient pas le layout procgen. Reviendront via
    //          `forgia-village-npc-spawner` (story-445) qui placera les villagers
    //          relatif aux building positions du VillageGraph.
    let _ = make_terrain_config();

    info!(
        "[forgia-rpg] World spawned : 1 chunk (32×32m, heightmap-grid) + sun + village genome request inserted (config/genomes/villages/starter_hamlet.toml)"
    );
}

/// Per-chunk residence tracking for unload hysteresis (story-450 wave 2).
/// `loaded_at_secs[coord]` = elapsed_secs au moment du spawn. Un chunk n'est
/// éligible à eviction par distance que si `now - loaded_at >= min_residence_secs`.
/// Pattern UE5 Level Streaming Volumes default 2.0s.
#[derive(Resource, Default)]
pub struct ChunkResidence {
    /// elapsed_secs au moment du spawn — gate `min_residence_secs` (UE5 hysteresis).
    pub loaded_at_secs: HashMap<ChunkCoord, f32>,
    /// Story-450 wave 3a — LRU tracking : elapsed_secs du dernier frame où le
    /// chunk était dans le view ring (≤ view_m). Pattern Unity Addressables
    /// memoryBudgetKB : LRU eviction triée par (distance_desc, last_seen_asc).
    pub last_seen_secs: HashMap<ChunkCoord, f32>,
}

/// W2 — Stream les chunks autour du joueur via `StreamingConfig` (story-450).
///
/// Industry pattern composite :
/// - **Triple radius** Minecraft + UE5 + Roblox : `view_chunks` (load ring) vs
///   `unload_chunks` (eviction ring, > view → hysteresis spatiale).
/// - **Temporal hysteresis** UE5 : chunk doit avoir résidé `min_residence_secs`
///   avant d'être éligible à eviction par distance.
/// - **Async pipeline budget** : `chunks_per_frame` configurable.
/// - **StreamingStats observability** : record load timing, eviction reasons,
///   queue depth, lod counts → sensor `forgia_chunk_stream.json`.
///
/// Fallback graceful : si `StreamingConfig` Resource absente (boot frame 0),
/// utilise `FALLBACK_RENDER_DIST` + `FALLBACK_CHUNKS_PER_FRAME`.
#[allow(clippy::too_many_arguments)]
fn stream_chunks_around_player(
    mut commands: Commands,
    time: Res<Time>,
    streaming_cfg: Option<Res<StreamingConfig>>,
    mut stats: ResMut<StreamingStats>,
    mut pause: ResMut<StreamingPause>,
    mut residence: ResMut<ChunkResidence>,
    mut chunk_mgr: ResMut<ChunkManager>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    biome_map: Option<Res<BiomeMap>>,
    shared_mat: Option<Res<TerrainSharedMaterial>>,
    flatten_zones: Option<Res<FlattenZones>>,
    lod_stats: Option<Res<LodStats>>,
    mut meshes: ResMut<Assets<Mesh>>,
    player_q: Query<&Transform, With<Player>>,
    mut pending: Local<VecDeque<ChunkCoord>>,
) {
    let (Some(terrain_cfg), Some(biome_map), Some(shared_mat)) =
        (terrain_cfg, biome_map, shared_mat)
    else {
        return;
    };
    let Ok(player_tf) = player_q.single() else {
        return;
    };

    // Resolve runtime radii + chunks_per_frame depuis StreamingConfig (genome)
    // ou fallback hardcoded si config pas encore chargée.
    let (view_chunks, unload_chunks, chunks_per_frame, min_residence_secs) =
        if let Some(cfg) = streaming_cfg.as_deref() {
            let chunk_m = CHUNK_X as f32;
            let view = (cfg.radii.view_m / chunk_m).ceil() as i32;
            let unload = (cfg.radii.unload_m / chunk_m).ceil() as i32;
            (
                view,
                unload.max(view + 1), // garantit hystérèse spatiale ≥ 1 chunk
                cfg.async_pipeline.chunks_per_frame as usize,
                cfg.hysteresis.min_residence_secs,
            )
        } else {
            (
                FALLBACK_RENDER_DIST,
                FALLBACK_RENDER_DIST + 2,
                FALLBACK_CHUNKS_PER_FRAME,
                2.0,
            )
        };

    let now = time.elapsed_secs();
    let player_chunk = ChunkCoord::from_world(player_tf.translation);
    let need_recompute_set = chunk_mgr.last_player_chunk != Some(player_chunk);

    // 1. Si traversée de frontière chunk : recalculer desired set + diff load/unload.
    if need_recompute_set {
        // Wave 5 phase 3 (story-450) : EUCLIDEAN distance au lieu de
        // MANHATTAN. Le Manhattan diamond créait des trous diagonaux visibles
        // (chunk (2,2) Manhattan=4 NOT loaded malgré Euclidean=90m < view=96m).
        // Pattern industrie : UE5 World Partition + Unity Addressables utilisent
        // cercles Euclidean. Minecraft utilise Chebyshev (square). Le Manhattan
        // n'est utilisé que pour économiser RAM en voxel hardcore (Vintage Story).
        //
        // Forgia choix Euclidean : matche les anneaux F3 overlay (cercles),
        // patch les trous diamantaires.
        let view_r_sq = (view_chunks as f32).powi(2);
        let mut desired: HashSet<ChunkCoord> = HashSet::new();
        for dx in -view_chunks..=view_chunks {
            for dz in -view_chunks..=view_chunks {
                let dist_sq = (dx * dx + dz * dz) as f32;
                if dist_sq <= view_r_sq {
                    desired.insert(ChunkCoord::new(player_chunk.x + dx, player_chunk.z + dz));
                }
            }
        }

        // Unload candidates = chunks hors unload_chunks ring (Euclidean aussi
        // pour cohérence) ET résidence expirée. Hystérèse UE5 + spatiale.
        let unload_r_sq = (unload_chunks as f32).powi(2);
        let loaded_coords: Vec<ChunkCoord> = chunk_mgr.loaded_entities.keys().copied().collect();
        for coord in loaded_coords {
            let dx = coord.x - player_chunk.x;
            let dz = coord.z - player_chunk.z;
            let dist_sq = (dx * dx + dz * dz) as f32;
            if dist_sq <= unload_r_sq {
                continue; // Dans la zone de garde, on ne touche pas.
            }
            // Au-delà du unload ring : check residence hysteresis.
            let resident_since = residence.loaded_at_secs.get(&coord).copied().unwrap_or(now);
            let residence_secs = now - resident_since;
            if residence_secs < min_residence_secs {
                stats.hysteresis_blocked_unloads =
                    stats.hysteresis_blocked_unloads.saturating_add(1);
                continue;
            }
            // Eviction autorisée.
            if let Some(entity) = chunk_mgr.loaded_entities.remove(&coord) {
                // try_despawn (Bevy 0.18) idempotent au flush — silent si
                // entity déjà queued for despawn par un autre system (cf
                // forgia-foliage::despawn_unloaded_chunks race conditions).
                if let Ok(mut ec) = commands.get_entity(entity) {
                    ec.try_despawn();
                }
                residence.loaded_at_secs.remove(&coord);
                residence.last_seen_secs.remove(&coord);
                let dist_m = dist_sq.sqrt() * CHUNK_X as f32;
                stats.record_eviction(EvictionEvent {
                    reason: EvictionReason::Distance,
                    coord: [coord.x, coord.z],
                    timestamp_secs: now,
                    distance_m: dist_m,
                });
            }
        }

        // Queue les chunks manquants — wave 4b frustum-aware priority :
        // pattern Witcher 3 (Umbra) + UE5 World Partition. Chunks **devant le
        // player** (dot(forward_xz, chunk_dir) > 0) chargés EN PREMIER, puis
        // tie-break par distance Manhattan ascendante. Player tourne la
        // caméra → ce qu'il regarde apparaît avant ce qui est derrière lui.
        pending.clear();
        let mut sorted: Vec<ChunkCoord> = desired
            .into_iter()
            .filter(|c| !chunk_mgr.loaded_entities.contains_key(c))
            .collect();
        // Forward XZ du player (Bevy convention : -Z = forward, projeté sur XZ).
        let forward = player_tf.forward();
        let fwd_xz = Vec2::new(forward.x, forward.z).normalize_or_zero();
        sorted.sort_by_key(|c| {
            let center = c.world_center();
            let dx = center.x - player_tf.translation.x;
            let dz = center.z - player_tf.translation.z;
            // Dot product XZ : > 0 = devant, ≤ 0 = derrière.
            let dot = fwd_xz.x * dx + fwd_xz.y * dz;
            let in_front: i32 = if dot > 0.0 { 0 } else { 1 };
            // Wave 5 phase 3 : tie-break Euclidean (squared, integer) au lieu
            // de Manhattan pour cohérence avec desired set + LRU touch.
            let cdx = c.x - player_chunk.x;
            let cdz = c.z - player_chunk.z;
            let dist_sq = cdx * cdx + cdz * cdz;
            (in_front, dist_sq)
        });
        for c in sorted {
            pending.push_back(c);
        }

        chunk_mgr.last_player_chunk = Some(player_chunk);
    }

    // 2. Mesh+spawn ≤ chunks_per_frame pour amortir le coût + instrument gen_ms.
    let off = sample_offset();
    let mut gen_count_this_frame = 0u32;
    for _ in 0..chunks_per_frame {
        let Some(coord) = pending.pop_front() else {
            break;
        };
        if chunk_mgr.loaded_entities.contains_key(&coord) {
            continue;
        }

        let t0 = std::time::Instant::now();
        let mesh_data = build_chunk_mesh(
            coord,
            off,
            &terrain_cfg,
            &biome_map,
            flatten_zones.as_deref(),
        );
        let gen_ms = t0.elapsed().as_secs_f32() * 1000.0;
        stats.record_gen_ms(gen_ms);
        gen_count_this_frame += 1;

        let mesh_handle = meshes.add(mesh_data.mesh.clone());
        let entity = spawn_chunk_entity(
            &mut commands,
            coord,
            mesh_data,
            mesh_handle,
            shared_mat.0.clone(),
        );
        commands.entity(entity).insert(RpgWorldMarker);
        chunk_mgr.loaded_entities.insert(coord, entity);
        residence.loaded_at_secs.insert(coord, now);
        residence.last_seen_secs.insert(coord, now);
    }

    // 2b. LRU touch — refresh last_seen pour tous les chunks dans le view ring
    // (Euclidean ≤ view_chunks). Pattern Unity Addressables refcount/LRU.
    // Wave 5 phase 3 : Euclidean cohérent avec desired set (vs Manhattan
    // qui sous-comptait les chunks diagonaux pourtant proches).
    let view_r_sq_touch = (view_chunks as f32).powi(2);
    for coord in chunk_mgr.loaded_entities.keys().copied() {
        let dx = coord.x - player_chunk.x;
        let dz = coord.z - player_chunk.z;
        let dist_sq = (dx * dx + dz * dz) as f32;
        if dist_sq <= view_r_sq_touch {
            residence.last_seen_secs.insert(coord, now);
        }
    }

    // 3. Update stats observability (lus par sensor 1Hz).
    stats.loaded_count = chunk_mgr.loaded_entities.len() as u32;
    stats.pending_load_count = pending.len() as u32;
    stats.pending_gen_count = gen_count_this_frame;

    // 3b. Wave 4a — StreamingPause auto-track (Roblox pattern).
    // Compute pending chunks dans le simulation ring (inner radius critique).
    // Pause active tant qu'il reste des chunks à charger dans la zone de
    // simulation player. Évite spawn-in-water / pop-in catastrophique.
    let sim_chunks = if let Some(cfg) = streaming_cfg.as_deref() {
        (cfg.radii.simulation_m / CHUNK_X as f32).ceil() as i32
    } else {
        view_chunks / 2 // fallback
    };
    // Wave 5 phase 3 : Euclidean check pour matcher desired set logic.
    let sim_r_sq = (sim_chunks as f32).powi(2);
    let pending_in_sim = pending
        .iter()
        .filter(|c| {
            let dx = c.x - player_chunk.x;
            let dz = c.z - player_chunk.z;
            (dx * dx + dz * dz) as f32 <= sim_r_sq
        })
        .count() as u32;

    // Auto-track pause : active si chunks pending dans le sim ring.
    if pending_in_sim > 0 {
        if !pause.active {
            pause.active = true;
            pause.reason = "loading_sim_ring".into();
        }
        pause.waiting_chunks = pending_in_sim;
    } else if pause.active {
        // Sim ring fully loaded → release pause.
        info!(
            "[forgia-rpg::streaming] StreamingPause released (reason={}, sim_chunks={})",
            pause.reason, sim_chunks
        );
        pause.active = false;
        pause.reason.clear();
        pause.waiting_chunks = 0;
    }
    if let Some(lod) = lod_stats {
        stats.lod0_count = lod.lod0_count;
        stats.lod1_count = lod.lod1_count;
        stats.lod2_count = lod.lod2_count;
    }
    // Memory estimation : CHUNK_MB_EST (~0.16 MB) dérivé du mesh uniforme 33×33.
    // Ancien 5.0 = ×25 overestimate (audit 2026-06-05). Raffinage GPU-exact = P2.
    stats.loaded_mb_est = stats.loaded_count as f32 * CHUNK_MB_EST;
}

// ─────────────────────────────────────────────────────────────────────────────
// Wave 4c — Debug streaming overlay (F3 toggle)
// ─────────────────────────────────────────────────────────────────────────────

/// Toggle visibility du debug overlay streaming. F3 cycle off → on → off.
/// Bevy `Gizmos` API : pas de ressources persistantes, draws chaque frame.
#[derive(Resource, Default)]
pub struct StreamingDebugOverlay {
    pub enabled: bool,
}

/// Toggle handler — F3 cycle visibility. Pattern UE5 `stat streaming` toggle.
fn toggle_streaming_overlay(
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<StreamingDebugOverlay>,
) {
    if keys.just_pressed(KeyCode::F3) {
        overlay.enabled = !overlay.enabled;
        info!(
            "[forgia-rpg::streaming] F3 overlay = {}",
            if overlay.enabled { "ON" } else { "OFF" }
        );
    }
}

/// Wave 4c — Draws chunk grid + LOD colors + load state via Bevy Gizmos.
///
/// Pattern UE5 World Partition Editor viewer + Roblox MicroProfiler streaming :
/// - Cube outline par chunk loaded (color per LOD : LOD0 green, LOD1 yellow, LOD2 red)
/// - Anneau view_m (cyan) + unload_m (orange) + simulation_m (magenta) au sol
/// - Pause indicator : sphère rouge au player si pause active
#[allow(clippy::too_many_arguments)]
fn draw_streaming_debug_overlay(
    overlay: Res<StreamingDebugOverlay>,
    streaming_cfg: Option<Res<StreamingConfig>>,
    pause: Res<StreamingPause>,
    chunk_mgr: Option<Res<ChunkManager>>,
    q_chunk_lod: Query<&forgia_terrain::ChunkLod>,
    player_q: Query<&Transform, With<Player>>,
    mut gizmos: Gizmos,
) {
    if !overlay.enabled {
        return;
    }
    let Some(cfg) = streaming_cfg else { return };
    let Some(chunk_mgr) = chunk_mgr else { return };
    let Ok(player_tf) = player_q.single() else {
        return;
    };
    let player_pos = player_tf.translation;
    let chunk_m = CHUNK_X as f32;
    let half = chunk_m * 0.5;
    let y_overlay = player_pos.y + 0.2; // léger offset sol

    // 1. Anneaux radii — circles XZ au sol.
    let center = Vec3::new(player_pos.x, y_overlay, player_pos.z);
    gizmos
        .circle(
            bevy::math::Isometry3d::new(center, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            cfg.radii.simulation_m,
            Color::srgb(1.0, 0.0, 1.0), // magenta
        )
        .resolution(64);
    gizmos
        .circle(
            bevy::math::Isometry3d::new(center, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            cfg.radii.view_m,
            Color::srgb(0.0, 1.0, 1.0), // cyan
        )
        .resolution(64);
    gizmos
        .circle(
            bevy::math::Isometry3d::new(center, Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            cfg.radii.unload_m,
            Color::srgb(1.0, 0.6, 0.0), // orange
        )
        .resolution(64);

    // 2. Cubes par chunk loaded — couleur per LOD.
    for (coord, entity) in &chunk_mgr.loaded_entities {
        let origin = coord.world_origin();
        let cube_center = Vec3::new(origin.x + half, y_overlay, origin.z + half);
        let lod = q_chunk_lod
            .get(*entity)
            .copied()
            .unwrap_or(forgia_terrain::ChunkLod::Lod0);
        let color = match lod {
            forgia_terrain::ChunkLod::Lod0 => Color::srgb(0.0, 1.0, 0.0), // green
            forgia_terrain::ChunkLod::Lod1 => Color::srgb(1.0, 1.0, 0.0), // yellow
            forgia_terrain::ChunkLod::Lod2 => Color::srgb(1.0, 0.3, 0.3), // red
        };
        // Carré XZ via 4 lignes (cube outline complet trop coûteux à 33 chunks).
        let corners = [
            Vec3::new(origin.x, y_overlay, origin.z),
            Vec3::new(origin.x + chunk_m, y_overlay, origin.z),
            Vec3::new(origin.x + chunk_m, y_overlay, origin.z + chunk_m),
            Vec3::new(origin.x, y_overlay, origin.z + chunk_m),
        ];
        gizmos.line(corners[0], corners[1], color);
        gizmos.line(corners[1], corners[2], color);
        gizmos.line(corners[2], corners[3], color);
        gizmos.line(corners[3], corners[0], color);
        // Point central pour repérage chunk coord (cf. recent_evictions dans sensor).
        let _ = cube_center;
    }

    // 3. Pause indicator — sphère rouge au-dessus player si pause active.
    if pause.active {
        gizmos.sphere(
            bevy::math::Isometry3d::from_translation(player_pos + Vec3::Y * 2.5),
            0.5,
            Color::srgb(1.0, 0.2, 0.2),
        );
    }

    // 4. Wave 5 phase 3 — MISSING chunks debug : pour chaque chunk du view
    // ring qui devrait être loaded mais ne l'est pas, draw un X ROUGE. Permet
    // diagnostic instant du "Manhattan diamond bug" ou tout autre trou.
    // Pattern UE5 World Partition Editor : missing cells highlighted.
    let player_chunk = forgia_terrain::ChunkCoord::from_world(player_pos);
    let view_chunks = (cfg.radii.view_m / chunk_m).ceil() as i32;
    let view_r_sq = (view_chunks as f32).powi(2);
    for dx in -view_chunks..=view_chunks {
        for dz in -view_chunks..=view_chunks {
            let dist_sq = (dx * dx + dz * dz) as f32;
            if dist_sq > view_r_sq {
                continue;
            }
            let coord = forgia_terrain::ChunkCoord::new(player_chunk.x + dx, player_chunk.z + dz);
            if chunk_mgr.loaded_entities.contains_key(&coord) {
                continue; // OK loaded
            }
            // MISSING : draw red X in chunk center
            let origin = coord.world_origin();
            let cx = origin.x + half;
            let cz = origin.z + half;
            let h = chunk_m * 0.4;
            let red = Color::srgb(1.0, 0.0, 0.0);
            gizmos.line(
                Vec3::new(cx - h, y_overlay + 1.0, cz - h),
                Vec3::new(cx + h, y_overlay + 1.0, cz + h),
                red,
            );
            gizmos.line(
                Vec3::new(cx - h, y_overlay + 1.0, cz + h),
                Vec3::new(cx + h, y_overlay + 1.0, cz - h),
                red,
            );
            // Encadré rouge complet pour visibilité
            let corners = [
                Vec3::new(origin.x, y_overlay + 0.5, origin.z),
                Vec3::new(origin.x + chunk_m, y_overlay + 0.5, origin.z),
                Vec3::new(origin.x + chunk_m, y_overlay + 0.5, origin.z + chunk_m),
                Vec3::new(origin.x, y_overlay + 0.5, origin.z + chunk_m),
            ];
            gizmos.line(corners[0], corners[1], red);
            gizmos.line(corners[1], corners[2], red);
            gizmos.line(corners[2], corners[3], red);
            gizmos.line(corners[3], corners[0], red);
        }
    }
}

/// Memory budget LRU eviction (story-450 wave 3a).
///
/// Pattern industrie : Unity Addressables `memoryBudgetKB` + GTA 5-style
/// per-pool LRU. Si `loaded_mb_est > cfg.budget.max_mb` OU
/// `loaded_count > cfg.budget.max_chunks`, évince les chunks dans cet ordre :
/// 1. Distance Manhattan **décroissante** (le plus loin évincé en premier)
/// 2. Tie-break : `last_seen_secs` **croissant** (le moins récemment vu)
///
/// Eviction respecte la hysteresis temporelle (chunks dans
/// `min_residence_secs` sont protégés — pattern UE5 LSV).
///
/// Run AFTER `stream_chunks_around_player` dans la chain Update.
#[allow(clippy::too_many_arguments)]
fn enforce_chunk_memory_budget(
    mut commands: Commands,
    time: Res<Time>,
    streaming_cfg: Option<Res<StreamingConfig>>,
    mut stats: ResMut<StreamingStats>,
    mut residence: ResMut<ChunkResidence>,
    mut chunk_mgr: ResMut<ChunkManager>,
    player_q: Query<&Transform, With<Player>>,
) {
    let Some(cfg) = streaming_cfg else { return };
    let Ok(player_tf) = player_q.single() else {
        return;
    };
    let player_chunk = ChunkCoord::from_world(player_tf.translation);
    let now = time.elapsed_secs();
    let min_residence = cfg.hysteresis.min_residence_secs;

    let loaded_count = chunk_mgr.loaded_entities.len() as u32;
    let loaded_mb_est = loaded_count as f32 * CHUNK_MB_EST;
    let over_mb = loaded_mb_est > cfg.budget.max_mb;
    let over_count = loaded_count > cfg.budget.max_chunks;
    if !over_mb && !over_count {
        return;
    }

    // Build candidates : chunks éligibles à eviction (résidence expirée),
    // triés par priorité d'eviction (le plus loin + plus ancien d'abord).
    let mut candidates: Vec<(ChunkCoord, i32, f32)> = chunk_mgr
        .loaded_entities
        .keys()
        .filter_map(|c| {
            let resident_since = residence.loaded_at_secs.get(c).copied().unwrap_or(now);
            if now - resident_since < min_residence {
                return None; // protégé par hysteresis temporelle
            }
            let last_seen = residence.last_seen_secs.get(c).copied().unwrap_or(now);
            // Wave 5 phase 3 : distance squared Euclidean (cohérent avec
            // desired/unload/touch/sim filters), evite Manhattan diamant.
            let cdx = c.x - player_chunk.x;
            let cdz = c.z - player_chunk.z;
            let dist_sq = cdx * cdx + cdz * cdz;
            Some((*c, dist_sq, last_seen))
        })
        .collect();

    // Tri industrie LRU + distance : distance desc, last_seen asc.
    candidates.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
    });

    // Évince jusqu'à retomber sous les deux caps.
    let target_count = cfg.budget.max_chunks.saturating_sub(1).min(loaded_count) as usize;
    let target_mb = cfg.budget.max_mb;
    let mut remaining_count = loaded_count as usize;
    let mut remaining_mb = loaded_mb_est;
    let mut evicted: u32 = 0;

    for (coord, dist_sq_chunks, _last_seen) in candidates {
        if remaining_count <= target_count && remaining_mb <= target_mb {
            break;
        }
        if let Some(entity) = chunk_mgr.loaded_entities.remove(&coord) {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.try_despawn();
            }
            residence.loaded_at_secs.remove(&coord);
            residence.last_seen_secs.remove(&coord);
            // dist_sq_chunks est en chunks² Euclidean. dist_m = sqrt(dist_sq) * CHUNK_X
            let dist_m = (dist_sq_chunks as f32).sqrt() * CHUNK_X as f32;
            stats.record_eviction(EvictionEvent {
                reason: EvictionReason::Budget,
                coord: [coord.x, coord.z],
                timestamp_secs: now,
                distance_m: dist_m,
            });
            remaining_count = remaining_count.saturating_sub(1);
            remaining_mb -= 5.0;
            evicted += 1;
        }
    }

    if evicted > 0 {
        info!(
            "[forgia-rpg::streaming] budget eviction : {} chunks (was {:.0}MB/{} chunks, now ~{:.0}MB/{}) over caps mb={} count={}",
            evicted,
            loaded_mb_est,
            loaded_count,
            remaining_mb,
            remaining_count,
            over_mb,
            over_count,
        );
    }
}

/// W2 — Sensor `forgia_chunks_snapshot.json` (observability-required.md).
fn write_chunks_sensor(
    time: Res<Time>,
    chunk_mgr: Option<Res<ChunkManager>>,
    biome_map: Option<Res<BiomeMap>>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    streaming_cfg: Option<Res<StreamingConfig>>,
    mut last_write: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last_write < SENSOR_INTERVAL_S {
        return;
    }
    *last_write = now;

    let (Some(chunk_mgr), Some(biome_map), Some(terrain_cfg)) = (chunk_mgr, biome_map, terrain_cfg)
    else {
        return;
    };

    // P0 observability fix (audit 2026-06-05) : render_dist DOIT refléter la
    // portée runtime réelle, pas la constante FALLBACK_RENDER_DIST. Même formule
    // que stream_chunks_around_player (ceil(view_m / CHUNK_X)), fallback si la
    // StreamingConfig n'est pas encore chargée.
    let render_dist = streaming_cfg
        .as_deref()
        .map(|c| (c.radii.view_m / CHUNK_X as f32).ceil() as i32)
        .unwrap_or(FALLBACK_RENDER_DIST);

    // Distribution biomes : compte 1 sample / chunk au centre.
    let off = sample_offset();
    let mut dist: HashMap<&'static str, u32> = HashMap::new();
    for coord in chunk_mgr.loaded_entities.keys() {
        let center = coord.world_center();
        let biome = biome_map.biome_at(center.x + off.x, center.z + off.y);
        *dist.entry(biome.as_str()).or_insert(0) += 1;
    }
    let dist_json: String = dist
        .iter()
        .map(|(k, v)| format!("\"{}\":{}", k, v))
        .collect::<Vec<_>>()
        .join(",");

    let json = format!(
        "{{\"timestamp_secs\":{:.1},\"loaded_count\":{},\"player_chunk\":[{},{}],\"render_dist\":{},\"map_size\":{},\"sea_level\":{},\"max_height\":{},\"biome_distribution\":{{{}}}}}",
        now,
        chunk_mgr.loaded_entities.len(),
        chunk_mgr.last_player_chunk.map(|c| c.x).unwrap_or(0),
        chunk_mgr.last_player_chunk.map(|c| c.z).unwrap_or(0),
        render_dist,
        terrain_cfg.map_size,
        terrain_cfg.sea_level,
        terrain_cfg.max_height,
        dist_json,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia_chunks_snapshot.json", json);
}

/// Marqueur Resource posée OnEnter(Rpg), retirée par `teleport_player_to_terrain`
/// après téléportation réussie. Garantit que la téléportation fire à CHAQUE
/// entrée Rpg, mais une seule fois (vs Local<bool> qui persistait à travers les
/// state transitions → joueur restait sous le terrain au 2e Rpg).
#[derive(Resource)]
struct PendingPlayerTeleport;

/// Téléporte le Player au centre du village hex (story-586) via `RpgVillageAnchor`.
/// `anchor.center.y` = `well_y` = le Y flatten du centre village → le joueur atterrit
/// sur le sol plat, pas sous le terrain. (Avant story-586 : lisait
/// `VillageLoadResult.spawn_position` du système B legacy, désormais débranché du RPG.)
/// Gating : attend que le sim ring soit chargé (StreamingPause), une seule fois par Rpg.
fn teleport_player_to_terrain(
    mut commands: Commands,
    pending: Option<Res<PendingPlayerTeleport>>,
    anchor: Option<Res<RpgVillageAnchor>>,
    pause: Option<Res<StreamingPause>>,
    mut q: Query<&mut Transform, With<Player>>,
) {
    if pending.is_none() {
        return;
    }
    let Some(anchor) = anchor else { return };
    // Wave 4a — block teleport tant que le sim ring n'est pas loaded.
    // Pattern Roblox StreamingPauseMode : "min-radius is a correctness contract".
    if let Some(pause) = pause {
        if pause.active {
            return; // retry frame suivante
        }
    }
    let Ok(mut tf) = q.single_mut() else { return };
    // Léger offset XZ (+3,+3) pour ne pas spawn pile sur le puits ; +2 en Y = marge
    // au-dessus du sol flatten, la gravité Rapier pose ensuite le joueur au sol.
    let spawn = Vec3::new(anchor.center.x + 3.0, anchor.center.y + 2.0, anchor.center.z + 3.0);
    tf.translation = spawn;
    commands.remove_resource::<PendingPlayerTeleport>();
    info!(
        "[forgia-rpg] Player teleported to village hex spawn ({:.1},{:.1},{:.1}) — sim ring loaded",
        spawn.x, spawn.y, spawn.z
    );
}

/// Consomme `VillageLoadResult` une fois publié par le village loader : pour
/// chaque RoadSegment, construit une polyline Bezier (warp 0.0 = droite radiale
/// par défaut) et remplace le `PathNetwork`. Le système ribbon mesh
/// `(`spawn_path_ribbons`) est invoqué une seule fois ici puis le marker
/// `VillagePathsBuilt` empêche la reconstruction.
#[derive(Resource)]
struct VillagePathsBuilt;

#[allow(clippy::too_many_arguments)]
fn spawn_village_paths_when_loaded(
    mut commands: Commands,
    village: Option<Res<VillageLoadResult>>,
    built: Option<Res<VillagePathsBuilt>>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    flatten_zones: Option<Res<FlattenZones>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    q_existing_trees: Query<(Entity, &Transform), With<VegetationTree>>,
) {
    if built.is_some() {
        return;
    }
    let Some(village) = village else { return };
    // BUG-441-04 fix : lire Res<TerrainConfig> au lieu de reconstruire via
    // `make_terrain_config()` — évite la dérive si les constantes changent.
    let Some(terrain_cfg) = terrain_cfg else {
        return;
    };
    let mut polylines = Vec::with_capacity(village.roads.len());
    for seg in &village.roads {
        let tier = RoadTier::from_tier_name(&seg.tier);
        let pl = build_path_segment(seg.start, seg.end, tier, 0.0);
        if !pl.samples.is_empty() {
            polylines.push(pl);
        }
    }
    let path_net = PathNetwork { polylines };
    spawn_path_ribbons(
        &mut commands,
        &mut meshes,
        &mut materials,
        &asset_server,
        &path_net,
        &terrain_cfg,
        flatten_zones.as_deref(),
    );

    // Foliage exclusion disc autour du village. Buffer +3m sur le bounding pour
    // une clairière nette autour des bâtiments les plus en bordure.
    let excl_radius = village.bounding_radius + 3.0;
    let excl_center = village.center;

    // Despawn rétroactif des arbres déjà spawnés à l'intérieur du disc OU sur
    // les routes (race : foliage a pu populate le chunk avant que les Resources
    // PathNetwork / FoliageExclusionDisc ne soient insérées). One-shot ici car
    // protégé par VillagePathsBuilt marker. Le sweep utilise path_net local
    // AVANT son move dans insert_resource.
    let r2 = excl_radius * excl_radius;
    let path_buffer_base = 4.0_f32;
    let mut cleared = 0u32;
    let mut cleared_paths = 0u32;
    for (e, tf) in &q_existing_trees {
        let p = Vec2::new(tf.translation.x, tf.translation.z);
        if p.distance_squared(excl_center) < r2 {
            commands.entity(e).despawn();
            cleared += 1;
            continue;
        }
        let too_close_path = path_net.samples_iter().any(|s| {
            let buf = s.tier.half_width() + path_buffer_base;
            p.distance_squared(s.pos) < buf * buf
        });
        if too_close_path {
            commands.entity(e).despawn();
            cleared_paths += 1;
        }
    }
    if cleared > 0 || cleared_paths > 0 {
        info!(
            "[forgia-rpg] cleared {} trees inside village exclusion disc r={:.1}m + {} trees on paths",
            cleared, excl_radius, cleared_paths
        );
    }

    commands.insert_resource(FoliageExclusionDisc {
        center: excl_center,
        radius: excl_radius,
    });
    commands.insert_resource(path_net);
    commands.insert_resource(VillagePathsBuilt);
}

/// Register sample DialogueTrees so the E-interact loop has content to show.
/// Each NPC's tree_id follows the convention "npc_<lowercase_name_with_underscores>".
fn register_sample_dialogues(mut registry: ResMut<DialogueRegistry>) {
    fn node(
        speaker: &str,
        line: &str,
        choices: Vec<(&str, Option<&str>, Vec<DialogueEffect>)>,
    ) -> DialogueNode {
        DialogueNode {
            speaker: speaker.to_string(),
            line: line.to_string(),
            choices: choices
                .into_iter()
                .map(|(text, next, effects)| DialogueChoice {
                    text: text.to_string(),
                    next: next.map(|s| NodeId(s.to_string())),
                    effects,
                })
                .collect(),
        }
    }

    // Story-570 : arbres on-brand, keyés sur les noms du lineup character
    // (interact_system dérive tree_id = "npc_<name_lowercase_snake>").

    // ── Maître Forgeron Célèste : mentor / quest giver ─────────────────────
    let mut maitre = DialogueTree {
        id: DialogueId("npc_maitreforgeron".into()),
        root: NodeId("greet".into()),
        nodes: HashMap::new(),
    };
    maitre.nodes.insert(
        NodeId("greet".into()),
        node(
            "Maître Forgeron Célèste",
            "Approche, apprenti. Des gobelins infestent mes mines — débarrasse-m'en.",
            vec![
                (
                    "J'accepte.",
                    Some("accept"),
                    vec![DialogueEffect::StartQuest {
                        id: "kill_goblins".into(),
                    }],
                ),
                ("Pas maintenant.", Some("refuse"), vec![]),
                (
                    "(Debug) C'est déjà fait.",
                    None,
                    vec![
                        // Self-start idempotent (or_insert ne reset pas) → évite
                        // l'AdvanceQuest orphelin si non acceptée (BUG-570-06).
                        DialogueEffect::StartQuest {
                            id: "kill_goblins".into(),
                        },
                        DialogueEffect::AdvanceQuest {
                            tag: "kill_goblin".into(),
                            delta: 5,
                        },
                    ],
                ),
                ("Au revoir.", None, vec![DialogueEffect::EndConversation]),
            ],
        ),
    );
    maitre.nodes.insert(
        NodeId("accept".into()),
        node(
            "Maître Forgeron Célèste",
            "Bien. Cinq gobelins, puis reviens me voir. Tiens, prends cette dague.",
            vec![(
                "Merci, maître.",
                None,
                vec![
                    DialogueEffect::GiveItem {
                        id: "iron_dagger".into(),
                        count: 1,
                    },
                    DialogueEffect::EndConversation,
                ],
            )],
        ),
    );
    maitre.nodes.insert(
        NodeId("refuse".into()),
        node(
            "Maître Forgeron Célèste",
            "Hmpf. Les mines ne se nettoieront pas seules, apprenti.",
            vec![("Au revoir.", None, vec![DialogueEffect::EndConversation])],
        ),
    );
    registry.trees.insert(maitre.id.clone(), maitre);

    // ── Maître Forgeron : remise de quête (tree_id "_turnin", sélectionné par
    //    interact_system quand kill_goblins est Completed) ───────────────────
    let mut maitre_turnin = DialogueTree {
        id: DialogueId("npc_maitreforgeron_turnin".into()),
        root: NodeId("greet".into()),
        nodes: HashMap::new(),
    };
    maitre_turnin.nodes.insert(
        NodeId("greet".into()),
        node(
            "Maître Forgeron Célèste",
            "Les mines sont nettoyées ?! Tu as fait du bon travail, apprenti. Merci à toi.",
            vec![(
                "Avec plaisir, maître.",
                None,
                vec![
                    DialogueEffect::TurnInQuest {
                        id: "kill_goblins".into(),
                    },
                    DialogueEffect::EndConversation,
                ],
            )],
        ),
    );
    registry.trees.insert(maitre_turnin.id.clone(), maitre_turnin);

    // ── Mira : marchande ───────────────────────────────────────────────────
    let mut mira = DialogueTree {
        id: DialogueId("npc_mira".into()),
        root: NodeId("greet".into()),
        nodes: HashMap::new(),
    };
    mira.nodes.insert(
        NodeId("greet".into()),
        node(
            "Mira",
            "Mes étals sont ouverts ! Que cherches-tu ?",
            vec![
                ("Voir tes étals.", None, vec![DialogueEffect::OpenShop]),
                ("Tes potions.", Some("potions"), vec![]),
                ("Tes armes.", Some("weapons"), vec![]),
                ("Rien, merci.", None, vec![DialogueEffect::EndConversation]),
            ],
        ),
    );
    mira.nodes.insert(
        NodeId("potions".into()),
        node(
            "Mira",
            "Une fiole de soin, cadeau de bienvenue !",
            vec![
                (
                    "Merci !",
                    None,
                    vec![
                        DialogueEffect::GiveItem {
                            id: "potion_heal".into(),
                            count: 1,
                        },
                        DialogueEffect::EndConversation,
                    ],
                ),
                ("Retour", Some("greet"), vec![]),
            ],
        ),
    );
    mira.nodes.insert(
        NodeId("weapons".into()),
        node(
            "Mira",
            "Je n'ai qu'un poignard rouillé pour le moment.",
            vec![("Tant pis.", Some("greet"), vec![])],
        ),
    );
    registry.trees.insert(mira.id.clone(), mira);

    // ── Dorin : villageois (lore) ──────────────────────────────────────────
    let mut dorin = DialogueTree {
        id: DialogueId("npc_dorin".into()),
        root: NodeId("greet".into()),
        nodes: HashMap::new(),
    };
    dorin.nodes.insert(
        NodeId("greet".into()),
        node(
            "Dorin",
            "Salut l'ami ! Le Maître Forgeron cherche de l'aide, tu devrais aller le voir.",
            vec![("À bientôt.", None, vec![DialogueEffect::EndConversation])],
        ),
    );
    registry.trees.insert(dorin.id.clone(), dorin);

    // ── Apprenti : villageois (lore) ───────────────────────────────────────
    let mut apprenti = DialogueTree {
        id: DialogueId("npc_apprenti".into()),
        root: NodeId("greet".into()),
        nodes: HashMap::new(),
    };
    apprenti.nodes.insert(
        NodeId("greet".into()),
        node(
            "L'Apprenti",
            "Oh... bonjour. Je m'entraîne encore. Un jour je forgerai comme le Maître !",
            vec![("Courage !", None, vec![DialogueEffect::EndConversation])],
        ),
    );
    registry.trees.insert(apprenti.id.clone(), apprenti);

    info!("[forgia-rpg] Registered 4 dialogue trees (MaitreForgeron, Mira, Dorin, Apprenti)");
}

/// Construit 1 mesh contenant N polylines ribbon distinctes avec **7 vertex par
/// sample** : 5 top (outer_L, inner_L, center, inner_R, outer_R) + 2 bottom
/// (outer_L_bot, outer_R_bot). Le path est un **slab épais posé sur le terrain** :
/// top surface profil Roman road (bombement +8cm + ornières), parois latérales
/// visibles, épaisseur 4-12cm hashée par sample → silhouette irrégulière style
/// pavés. Le slab coule de 2cm dans le terrain → pas de gap aux bords + le
/// terrain ne peut plus traverser la route même sur noise local. Bonus :
/// compute_normals() lisse l'arête top/wall → look "worn cobble" pas pavé neuf.
fn build_path_ribbon_mesh(
    path_net: &PathNetwork,
    terrain_cfg: &TerrainConfig,
    flatten_zones: Option<&FlattenZones>,
) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    // Couleurs cibles : dirt foncé (centre) vs herbe (bord) pour le blend edge.
    // CENTER_DIRT légèrement plus clair que inner = compaction passage répété.
    const DIRT_BASE: [f32; 3] = [0.13, 0.09, 0.05];
    const CENTER_DIRT: [f32; 3] = [0.18, 0.13, 0.08];
    const GRASS_BASE: [f32; 3] = [0.22, 0.32, 0.12];

    // Profil transversal — exception `no-hardcode.md` "structural visual".
    const BOMB_CENTER_M: f32 = 0.08; // +8cm bombement central (Roman road)
    const RUT_INNER_M: f32 = -0.02; // -2cm ornières roues côté inner
    const RUT_NOISE_M: f32 = 0.04; // amplitude bruit ornière (±2cm)
    const THICKNESS_MIN_M: f32 = 0.04; // épaisseur slab min (4cm)
    const THICKNESS_RANGE_M: f32 = 0.08; // amplitude hash → max 12cm
    const SINK_INTO_TERRAIN_M: f32 = 0.02; // 2cm sous terrain → pas de gap visible

    let total_samples: usize = path_net.polylines.iter().map(|p| p.samples.len()).sum();
    let verts_per_sample: usize = 7;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(total_samples * verts_per_sample);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(total_samples * verts_per_sample);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(total_samples * verts_per_sample);
    // 6 bandes (4 top + 2 walls) × 6 indices = 36 indices par paire de samples.
    let mut indices: Vec<u32> = Vec::with_capacity(total_samples * 36);

    let mut base_vertex: u32 = 0;
    for polyline in &path_net.polylines {
        let n = polyline.samples.len();
        if n < 2 {
            continue;
        }

        let mut v_dist = 0.0_f32;
        let mut prev_center: Option<Vec2> = None;
        for (i, s) in polyline.samples.iter().enumerate() {
            let perp = Vec2::new(-s.tangent.y, s.tangent.x);
            let hw = s.tier.half_width();
            let ef = s.tier.edge_fade();
            // 5 positions XZ : outer_l, inner_l, center, inner_r, outer_r.
            // inner_l/r placés à 60% du half_width pour laisser place au bombement.
            let p_ol = s.pos - perp * (hw + ef);
            let p_il = s.pos - perp * (hw * 0.6);
            let p_c = s.pos;
            let p_ir = s.pos + perp * (hw * 0.6);
            let p_or = s.pos + perp * (hw + ef);

            // Épaisseur slab variable par sample — hash déterministe.
            // 4-12cm donne une silhouette irrégulière "pavés posés", pas planche plate.
            let thickness_h = (i as u32).wrapping_mul(2_654_435_761) as f32 / u32::MAX as f32;
            let thickness = THICKNESS_MIN_M + thickness_h * THICKNESS_RANGE_M;

            // Y per vertex — profil bombé Roman road sur slab épais :
            //   outer_top  = terrain + thickness (base haute du pavé)
            //   inner_top  = terrain + thickness + ornière (-2cm + bruit)
            //   center_top = terrain + thickness + bombement (+8cm)
            //   outer_bot  = terrain - 2cm (sous le sol → pas de gap)
            // Story-447 A.5 — FlattenZones leveled quand path traverse village.
            let terrain_y = |p: Vec2| -> f32 {
                terrain_height_local_with_flatten(p.x, p.y, terrain_cfg, flatten_zones)
            };
            let h_inner = |p: Vec2, seed: usize| {
                let n = ((seed as u32).wrapping_mul(2_246_822_507) as f32 / u32::MAX as f32) - 0.5;
                terrain_y(p) + thickness + RUT_INNER_M + n * RUT_NOISE_M
            };
            let h_center = |p: Vec2, seed: usize| {
                let n = ((seed as u32).wrapping_mul(1_597_334_677) as f32 / u32::MAX as f32) - 0.5;
                terrain_y(p) + thickness + BOMB_CENTER_M + n * 0.02
            };
            let y_ol_top = terrain_y(p_ol) + thickness;
            let y_il_top = h_inner(p_il, i);
            let y_c_top = h_center(p_c, i);
            let y_ir_top = h_inner(p_ir, i.wrapping_add(7));
            let y_or_top = terrain_y(p_or) + thickness;
            let y_ol_bot = terrain_y(p_ol) - SINK_INTO_TERRAIN_M;
            let y_or_bot = terrain_y(p_or) - SINK_INTO_TERRAIN_M;

            let center = s.pos;
            if let Some(pc) = prev_center {
                v_dist += (center - pc).length();
            }
            prev_center = Some(center);

            // 5 verts top (indices 0..4) puis 2 verts bottom (5,6).
            positions.push([p_ol.x, y_ol_top, p_ol.y]);
            positions.push([p_il.x, y_il_top, p_il.y]);
            positions.push([p_c.x, y_c_top, p_c.y]);
            positions.push([p_ir.x, y_ir_top, p_ir.y]);
            positions.push([p_or.x, y_or_top, p_or.y]);
            positions.push([p_ol.x, y_ol_bot, p_ol.y]);
            positions.push([p_or.x, y_or_bot, p_or.y]);

            uvs.push([0.0, v_dist * 0.5]);
            uvs.push([0.30, v_dist * 0.5]);
            uvs.push([0.50, v_dist * 0.5]);
            uvs.push([0.70, v_dist * 0.5]);
            uvs.push([1.0, v_dist * 0.5]);
            // Bottom verts : u = 0 / 1 (= outer u), v décalé pour que la texture
            // continue côté wall (effet "même pavé continue sur la tranche").
            uvs.push([0.0, v_dist * 0.5 - thickness * 0.5]);
            uvs.push([1.0, v_dist * 0.5 - thickness * 0.5]);

            // Variation dirt déterministe (i hashé) — texture procédurale.
            let h = ((i as u32).wrapping_mul(2_654_435_761) as f32 / u32::MAX as f32) * 0.20;
            let dirt = [
                (DIRT_BASE[0] + h).min(0.5),
                (DIRT_BASE[1] + h * 0.7).min(0.4),
                (DIRT_BASE[2] + h * 0.5).min(0.3),
                1.0,
            ];
            let center_dirt = [
                (CENTER_DIRT[0] + h * 0.5).min(0.55),
                (CENTER_DIRT[1] + h * 0.4).min(0.45),
                (CENTER_DIRT[2] + h * 0.3).min(0.35),
                1.0,
            ];
            let outer = [
                GRASS_BASE[0] * 0.7 + dirt[0] * 0.3,
                GRASS_BASE[1] * 0.7 + dirt[1] * 0.3,
                GRASS_BASE[2] * 0.7 + dirt[2] * 0.3,
                1.0,
            ];
            // Wall verts = dirt sombre (l'ombre sous le pavé) → contraste avec top.
            let wall = [
                DIRT_BASE[0] * 0.6,
                DIRT_BASE[1] * 0.6,
                DIRT_BASE[2] * 0.6,
                1.0,
            ];
            colors.push(outer);
            colors.push(dirt);
            colors.push(center_dirt);
            colors.push(dirt);
            colors.push(outer);
            colors.push(wall);
            colors.push(wall);

            if i > 0 {
                let stride = verts_per_sample as u32;
                let base0 = base_vertex + (i as u32 - 1) * stride;
                let base1 = base_vertex + (i as u32) * stride;
                // 4 bandes top : outer_L→inner_L, inner_L→center, center→inner_R, inner_R→outer_R.
                // Winding CCW vu d'en haut (normale vers +Y).
                for band in 0..4u32 {
                    let a0 = base0 + band;
                    let b0 = a0 + 1;
                    let a1 = base1 + band;
                    let b1 = a1 + 1;
                    indices.extend_from_slice(&[a0, b0, a1, b0, b1, a1]);
                }
                // Wall gauche : outer_L_top (idx 0) ↓ outer_L_bot (idx 5).
                // Convention code : outer_L = pos - perp, mais perp pointe -X
                // pour tangent +Z → outer_L est en réalité côté +X. Normale wall
                // doit pointer +X (extérieur du path côté outer_L). Winding fix.
                let lw_a0 = base0; // outer_L_top i-1
                let lw_b0 = base0 + 5; // outer_L_bot i-1
                let lw_a1 = base1; // outer_L_top i
                let lw_b1 = base1 + 5; // outer_L_bot i
                indices.extend_from_slice(&[lw_a0, lw_a1, lw_b0, lw_a1, lw_b1, lw_b0]);
                // Wall droite : outer_R_top (idx 4) ↓ outer_R_bot (idx 6).
                // outer_R = pos + perp → côté -X. Normale wall doit pointer -X.
                // Winding inverse de wall gauche.
                let rw_a0 = base0 + 4; // outer_R_top i-1
                let rw_b0 = base0 + 6; // outer_R_bot i-1
                let rw_a1 = base1 + 4; // outer_R_top i
                let rw_b1 = base1 + 6; // outer_R_bot i
                indices.extend_from_slice(&[rw_a0, rw_b0, rw_a1, rw_b0, rw_b1, rw_a1]);
            }
        }
        base_vertex += (n * verts_per_sample) as u32;
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();
    mesh
}

/// Spawn le ribbon continu (1 entity) + material PBR dirt + NotShadowCaster.
fn spawn_path_ribbons(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    asset_server: &Res<AssetServer>,
    path_net: &PathNetwork,
    terrain_cfg: &TerrainConfig,
    flatten_zones: Option<&FlattenZones>,
) {
    if path_net.polylines.is_empty() {
        return;
    }

    let mesh = build_path_ribbon_mesh(path_net, terrain_cfg, flatten_zones);
    let mesh_handle = meshes.add(mesh);

    // V1 path PBR pack via junction textures-v1/. Diff (albedo) + normal +
    // roughness JPG → vrai dirt path crédible (vs vertex colors aplats).
    // vertex_color reste actif comme MULTIPLICATEUR (full dirt au centre,
    // blend grass tint sur les outer pour fade naturel bord/herbe).
    // Sampler en Repeat (cf. terrain_material::repeat_sampler) car les UV
    // ribbon utilisent `v = v_dist * 0.5` cumulatif → sans Repeat le sampler
    // clamp à v=1.0 et étire un seul texel sur toute la longueur du path.
    let diff: Handle<Image> = asset_server.load_with_settings(
        "textures-v1/terrain/path/diff.jpg",
        forgia_terrain::repeat_sampler(),
    );
    let normal: Handle<Image> = asset_server.load_with_settings(
        "textures-v1/terrain/path/normal.jpg",
        forgia_terrain::repeat_sampler(),
    );
    let rough: Handle<Image> = asset_server.load_with_settings(
        "textures-v1/terrain/path/roughness.jpg",
        forgia_terrain::repeat_sampler(),
    );

    // Profil Roman road + ornières + bombement central → la géométrie 5-vert
    // catch la lumière sur le sommet. Roughness baissée légèrement (0.85)
    // pour spec-highlight diffus sur cailloux humides. Reflectance 0.04 = dirt.
    let road_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(diff),
        normal_map_texture: Some(normal),
        metallic_roughness_texture: Some(rough),
        perceptual_roughness: 0.85,
        reflectance: 0.04,
        ..default()
    });
    commands.spawn((
        RpgWorldMarker,
        Mesh3d(mesh_handle),
        MeshMaterial3d(road_mat),
        Transform::IDENTITY,
        bevy::light::NotShadowCaster,
        Name::new("PathRibbon"),
    ));

    let total: usize = path_net.polylines.iter().map(|p| p.samples.len()).sum();
    info!(
        "[forgia-rpg] Path ribbon spawned : {} polylines, {} samples total",
        path_net.polylines.len(),
        total
    );
}

/// Composite cave entrance = 1 arche centrale + 2 piliers latéraux + 1 escalier
/// descendant légèrement enfoncé + 3 rubble alentour. Orientation : entrée fait
/// face au centre du chunk (joueur spawn à 16,16).
///
/// **Disabled depuis 2026-05-17 PM** : appel retiré de `spawn_world` (user feedback :
/// "les grottes/cavernes n'apportent rien"). Fonction conservée pour réutilisation
/// (donjons futurs, POI nature).
#[allow(dead_code)]
fn spawn_cave_entrance(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    pos: Vec2,
    terrain_cfg: &TerrainConfig,
) {
    let h_ground = terrain_height_local(pos.x, pos.y, terrain_cfg);
    let center = Vec3::new(pos.x, h_ground, pos.y);

    // Oriente l'entrée vers le centre du chunk (16, _, 16).
    let to_center = (Vec3::new(16.0, h_ground, 16.0) - center).normalize_or_zero();
    let yaw = to_center.x.atan2(to_center.z);
    let face = Quat::from_rotation_y(yaw);
    // Vecteurs locaux : "forward" = vers le centre joueur, "right" = perpendiculaire.
    let forward = to_center;
    let right = Vec3::new(-forward.z, 0.0, forward.x);

    let arched: Handle<Scene> = asset_server
        .load(GltfAssetLabel::Scene(0).from_asset("models/kaykit/dungeon/wall_arched.glb"));
    let pillar: Handle<Scene> =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/kaykit/dungeon/pillar.glb"));
    let stairs: Handle<Scene> =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/kaykit/dungeon/stairs.glb"));
    let rubble: Handle<Scene> =
        asset_server.load(GltfAssetLabel::Scene(0).from_asset("models/kaykit/dungeon/rubble.glb"));

    // 1) Arche centrale — entrée, légèrement enfoncée -0.3m pour effet enterré.
    commands.spawn((
        RpgWorldMarker,
        SceneRoot(arched),
        Transform {
            translation: center + Vec3::Y * -0.3,
            rotation: face,
            scale: Vec3::splat(1.2),
        },
        Name::new("CaveArch"),
    ));
    // 2) 2 piliers de chaque côté de l'arche (-right et +right, 1.4m écartement).
    for side in [-1.0f32, 1.0] {
        commands.spawn((
            RpgWorldMarker,
            SceneRoot(pillar.clone()),
            Transform {
                translation: center + right * (side * 1.4),
                rotation: face,
                scale: Vec3::splat(1.0),
            },
            Name::new(format!("CavePillar_{side}")),
        ));
    }
    // 3) Escalier descendant 1.5m devant l'arche, légèrement enfoncé.
    commands.spawn((
        RpgWorldMarker,
        SceneRoot(stairs),
        Transform {
            translation: center + forward * 1.5 + Vec3::Y * -0.5,
            rotation: face,
            scale: Vec3::splat(1.0),
        },
        Name::new("CaveStairs"),
    ));
    // 4) 3 rubble dispersés alentour (déco usée).
    let rubble_offsets = [
        right * 2.0 + forward * 0.5,
        -right * 2.2 + forward * 0.3,
        forward * -0.8 + right * -0.9,
    ];
    for (i, off) in rubble_offsets.iter().enumerate() {
        let h = terrain_height_local(pos.x + off.x, pos.y + off.z, terrain_cfg);
        commands.spawn((
            RpgWorldMarker,
            SceneRoot(rubble.clone()),
            Transform {
                translation: Vec3::new(pos.x + off.x, h, pos.y + off.z),
                rotation: Quat::from_rotation_y(i as f32 * 1.7),
                scale: Vec3::splat(0.9),
            },
            Name::new(format!("CaveRubble_{i}")),
        ));
    }
}

// ── Cartoon sky (clouds) ────────────────────────────────────────────────────
// Port direct du pattern forgia-mode-fps-arena → 18 cloud clusters orbital +
// drift system. Tagged RpgWorldMarker pour cleanup auto OnExit Rpg.
//
// Stockage polaire (angle/radius/height) pour éviter wrap brusque sur drift +X
// linéaire (le bug V1 où nuages disparaissaient à 4 km est exclu).

const RPG_CLOUD_COLOR: Color = Color::srgb(0.95, 0.96, 0.97);
const RPG_CLOUD_EMISSIVE: LinearRgba = LinearRgba::new(0.05, 0.05, 0.06, 1.0);
const RPG_CLOUD_ORBIT_SPEED: f32 = 0.025; // rad/s — orbit complet ~4 min

#[derive(Component)]
pub struct RpgCloudOrbit {
    pub angle: f32,
    pub radius: f32,
    pub height: f32,
}

fn spawn_rpg_cartoon_clouds(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let cloud_blob_mesh = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());
    let cloud_mat = materials.add(StandardMaterial {
        base_color: RPG_CLOUD_COLOR,
        emissive: RPG_CLOUD_EMISSIVE,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        reflectance: 0.0,
        ..default()
    });

    let blobs_popcorn: &[(f32, f32, f32, f32)] = &[
        (0.0, 0.0, 0.0, 5.0),
        (4.5, 0.8, 1.2, 3.5),
        (-4.0, 0.4, -1.8, 3.2),
        (1.5, 1.5, 4.5, 2.8),
        (-2.0, -0.6, -4.2, 2.6),
        (3.5, -0.4, -3.0, 2.2),
    ];
    let blobs_stratus: &[(f32, f32, f32, f32)] = &[
        (-12.0, 0.0, 0.0, 2.5),
        (-8.0, 0.5, 0.8, 3.0),
        (-4.0, 0.3, -0.5, 3.5),
        (0.0, 0.0, 0.5, 4.0),
        (4.0, 0.4, -0.3, 3.6),
        (8.0, 0.2, 0.7, 3.0),
        (12.0, 0.0, -0.4, 2.4),
        (15.0, -0.3, 0.0, 1.8),
    ];
    let blobs_puff: &[(f32, f32, f32, f32)] = &[
        (0.0, 0.0, 0.0, 3.0),
        (2.5, 0.5, 0.5, 2.2),
        (-2.0, 0.3, -0.5, 2.0),
        (0.5, -0.3, 1.8, 1.8),
    ];

    let clusters: &[(f32, f32, f32, f32, f32, u8)] = &[
        (-40.0, 55.0, -55.0, 1.0, 1.0, 0),
        (55.0, 62.0, -35.0, 1.3, 1.2, 0),
        (5.0, 68.0, 65.0, 1.6, 0.9, 0),
        (-60.0, 50.0, 38.0, 1.0, 1.1, 0),
        (45.0, 58.0, 55.0, 0.85, 1.0, 0),
        (-25.0, 60.0, -80.0, 1.2, 0.8, 1),
        (70.0, 55.0, 20.0, 1.0, 0.9, 1),
        (-80.0, 52.0, -10.0, 1.1, 0.85, 1),
        (15.0, 48.0, -25.0, 0.7, 1.0, 2),
        (-15.0, 50.0, 25.0, 0.8, 1.0, 2),
        (-100.0, 80.0, -60.0, 1.8, 0.7, 1),
        (90.0, 85.0, -90.0, 1.5, 0.8, 0),
        (-50.0, 90.0, 100.0, 2.0, 0.7, 1),
        (100.0, 75.0, 70.0, 1.4, 1.0, 0),
        (0.0, 95.0, -120.0, 1.6, 0.8, 1),
        (-110.0, 70.0, 40.0, 1.2, 0.9, 0),
        (60.0, 88.0, 110.0, 1.7, 0.75, 1),
        (30.0, 78.0, -70.0, 0.9, 1.1, 2),
    ];

    for (ci, &(cx, cy, cz, scale, hscale, preset)) in clusters.iter().enumerate() {
        let blobs: &[(f32, f32, f32, f32)] = match preset {
            1 => blobs_stratus,
            2 => blobs_puff,
            _ => blobs_popcorn,
        };
        let preset_name = match preset {
            1 => "stratus",
            2 => "puff",
            _ => "popcorn",
        };

        let angle = cz.atan2(cx);
        let radius = (cx * cx + cz * cz).sqrt();
        let parent_id = commands
            .spawn((
                RpgWorldMarker,
                RpgCloudOrbit {
                    angle,
                    radius,
                    height: cy,
                },
                Transform::from_xyz(cx, cy, cz).with_scale(Vec3::new(scale, hscale * scale, scale)),
                Visibility::default(),
                Name::new(format!("RpgCloud_{preset_name}_{ci}")),
            ))
            .id();

        for (bi, &(bx, by, bz, br)) in blobs.iter().enumerate() {
            let child = commands
                .spawn((
                    Mesh3d(cloud_blob_mesh.clone()),
                    MeshMaterial3d(cloud_mat.clone()),
                    Transform::from_xyz(bx, by, bz).with_scale(Vec3::splat(br)),
                    bevy::light::NotShadowCaster,
                    Name::new(format!("RpgBlob_{ci}_{bi}")),
                ))
                .id();
            commands.entity(parent_id).add_child(child);
        }
    }
    info!("[forgia-rpg] Cartoon sky : 18 cloud clusters spawned");
}

fn rpg_cloud_drift_system(time: Res<Time>, mut q: Query<(&mut Transform, &mut RpgCloudOrbit)>) {
    let dt = time.delta_secs();
    for (mut tf, mut orbit) in &mut q {
        orbit.angle += dt * RPG_CLOUD_ORBIT_SPEED;
        tf.translation.x = orbit.angle.cos() * orbit.radius;
        tf.translation.z = orbit.angle.sin() * orbit.radius;
        tf.translation.y = orbit.height;
    }
}

fn cleanup_world(
    mut commands: Commands,
    q: Query<Entity, With<RpgWorldMarker>>,
    trees: Query<Entity, With<VegetationTree>>,
    mut lod2_mgr: ResMut<Lod2TileManager>,
) {
    let count = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    let tree_count = trees.iter().count();
    for e in &trees {
        commands.entity(e).despawn();
    }
    // LOD2 mega-tiles : despawn explicite (entités non taggées RpgWorldMarker).
    lod2_mgr.despawn_all(&mut commands);
    // Resources terrain — TerrainSharedMaterial conservé (réutilisable session suivante).
    commands.remove_resource::<ChunkManager>();
    commands.remove_resource::<BiomeMap>();
    // Efface la BiomeMap globale (sinon heightmap_at resterait biome-aware hors RPG).
    forgia_terrain::clear_world_biome_map();
    forgia_terrain::clear_world_biome_amplitudes();
    commands.remove_resource::<MapGenConfig>();
    commands.remove_resource::<TerrainConfig>();
    commands.remove_resource::<RpgSampleOffset>();
    commands.remove_resource::<LodSampleOffset>();
    commands.remove_resource::<AudioSampleOffset>();
    commands.remove_resource::<PathNetwork>();
    commands.remove_resource::<VillagePathsBuilt>();
    commands.remove_resource::<PendingPlayerTeleport>();
    commands.remove_resource::<LoadVillageRequest>();
    commands.remove_resource::<LoadVillageGenomeRequest>();
    commands.remove_resource::<FoliageExclusionDisc>();
    commands.insert_resource(LodStats::default());
    commands.insert_resource(VegetationManager::default());
    info!(
        "[forgia-rpg] World cleaned : {} entities + {} trees + LOD2 tiles despawned",
        count, tree_count,
    );
}

/// Interaction system : when player presses E, find nearest InteractablePoint
/// within its radius. Triggers StartDialogue for NPCs, info log for buildings.
#[allow(clippy::type_complexity)]
fn interact_system(
    // BUG-review-#8 : garde via Without — pas de nouvelle interaction si un
    // dialogue ou une boutique est déjà ouvert sur le Player.
    players: Query<
        (Entity, &Transform, &ActionState<PlayerAction>, &QuestLog),
        (With<Player>, Without<DialogueSession>, Without<ShopSession>),
    >,
    interactables: Query<(
        Entity,
        &Transform,
        &InteractablePoint,
        Option<&Npc>,
        Option<&QuestGiver>,
    )>,
    mut start_dialogue: MessageWriter<StartDialogue>,
) {
    let Ok((player_e, player_tf, action, log)) = players.single() else {
        return;
    };
    if !action.just_pressed(&PlayerAction::Interact) {
        return;
    }
    let player_pos = player_tf.translation;

    // Find closest interactable in radius
    let mut best: Option<(
        Entity,
        f32,
        &InteractablePoint,
        Option<&Npc>,
        Option<&QuestGiver>,
    )> = None;
    for (e, tf, ip, npc, qg) in &interactables {
        let d = tf.translation.distance(player_pos);
        if d <= ip.radius && (best.is_none() || d < best.unwrap().1) {
            best = Some((e, d, ip, npc, qg));
        }
    }

    let Some((target, dist, ip, npc, qgiver)) = best else {
        info!("[interact] no interactable in range");
        return;
    };

    info!("[interact] '{}' at {:.1}m", ip.label, dist);

    if let Some(npc) = npc {
        let base = format!("npc_{}", npc.name.to_lowercase().replace(' ', "_"));
        // BUG-review-#3 : variante turn-in si ce PNJ attend une quête Completed.
        let has_turnin = qgiver
            .map(|g| {
                g.completes.iter().any(|qid| {
                    log.entries
                        .get(qid)
                        .is_some_and(|s| s.status == QuestStatus::Completed)
                })
            })
            .unwrap_or(false);
        let tree_id = if has_turnin {
            DialogueId(format!("{base}_turnin"))
        } else {
            DialogueId(base)
        };
        info!("[dialogue] {} : « {} »", npc.name, npc.greeting);
        start_dialogue.write(StartDialogue {
            player: player_e,
            npc: target,
            tree_id,
        });
    }
}

/// Startup : peuple le `QuestCatalogue` (story-570). L'id `kill_goblins` doit
/// matcher l'effet `StartQuest` du dialogue d'Aldric (`register_sample_dialogues`).
fn register_sample_quests(mut catalogue: ResMut<QuestCatalogue>) {
    catalogue.register(QuestDef {
        id: QuestId("kill_goblins".into()),
        title: "Nettoyer les mines".into(),
        description: "Le Forgeron Aldric a besoin d'aide : 5 gobelins infestent ses mines.".into(),
        objectives: vec![Objective {
            tag: "kill_goblin".into(),
            label: "Tuer 5 gobelins".into(),
            target: 5,
        }],
        xp_reward: 120,
        item_rewards: vec![("gold".into(), 50), ("health_potion".into(), 2)],
    });
}

/// Update (RPG, retry-jusqu'à-success) : greffe les components RPG sur le Player
/// existant. Le Player est spawné par forgia-player (OnEnter InGame, partagé
/// FPS/Roguelite/RPG) — le filtre `Without<Inventory>` rend l'op idempotente et
/// évite de polluer les autres modes (story-570).
fn grant_rpg_player_components(
    mut commands: Commands,
    players: Query<Entity, (With<Player>, Without<Inventory>)>,
) {
    for e in &players {
        commands.entity(e).insert((
            Inventory::default(),
            QuestLog::default(),
            XpProgress::default(),
            Gold(STARTING_GOLD),
            TrackedQuest::default(),
        ));
        info!(
            "[forgia-rpg] RPG components (Inventory/QuestLog/XpProgress/Gold={}/TrackedQuest) attachés au Player",
            STARTING_GOLD
        );
    }
}

// Story-58x Phase 3 : l'UI inventaire a migré vers
// `forgia-ui-lib::inventory_panel` (sacs à icônes + tooltips + or, cartoon Forge).

/// Update (RPG) : applique l'usage d'un objet (clic droit inventaire, story-58x
/// Phase 3b). Consommable (`heal_amount > 0`) → soigne le Player + retire 1 ;
/// équipement non encore implémenté (log honnête).
fn apply_item_use(
    mut events: MessageReader<UseItemRequest>,
    registry: Res<ItemRegistry>,
    mut q: Query<(&mut Inventory, &mut Health), With<Player>>,
) {
    for ev in events.read() {
        let Ok((mut inv, mut hp)) = q.get_mut(ev.entity) else {
            continue;
        };
        let Some(stack) = inv.slot(ev.slot) else {
            continue;
        };
        let id = stack.id.clone();
        let Some(def) = registry.get(&id) else {
            continue;
        };
        if def.heal_amount > 0 {
            let before = hp.current;
            hp.current = (hp.current + def.heal_amount as f32).min(hp.max);
            inv.consume_one(ev.slot);
            info!(
                "[forgia-rpg] {} utilisé → +{:.0} HP ({:.0}→{:.0})",
                def.display_name,
                hp.current - before,
                before,
                hp.current
            );
        } else {
            info!(
                "[forgia-rpg] {} : pas d'effet d'usage (équipement à venir)",
                def.display_name
            );
        }
    }
}

/// EguiPrimaryContextPass (RPG) : marqueurs flottants ! / ? au-dessus des PNJ
/// donneurs de quête (story-58x Phase 4). Projection world→viewport via la
/// caméra orbit RPG (modèle kill_popup.rs). Culling distance 60m.
fn draw_quest_markers(
    mut contexts: EguiContexts,
    cam: Query<(&Camera, &GlobalTransform), With<character::RpgOrbitCamera>>,
    givers: Query<(&Transform, &QuestGiver)>,
    player_log: Query<&QuestLog, With<Player>>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };
    let Ok((camera, cam_xf)) = cam.single() else {
        return;
    };
    let Ok(log) = player_log.single() else {
        return;
    };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_rpg_quest_markers"),
    ));
    let cam_pos = cam_xf.translation();
    for (tf, giver) in &givers {
        let pos = tf.translation;
        if cam_pos.distance(pos) > 60.0 {
            continue;
        }
        let Some((sym, color)) = marker_for(giver, log) else {
            continue;
        };
        let Ok(screen) = camera.world_to_viewport(cam_xf, pos + Vec3::Y * 2.6) else {
            continue;
        };
        // world_to_viewport renvoie un bevy::Vec2 → convertir en egui::Pos2.
        let screen = egui::pos2(screen.x, screen.y);
        painter.circle_filled(screen, 15.0, color);
        painter.circle_stroke(screen, 15.0, egui::Stroke::new(2.5, egui::Color32::BLACK));
        let font = egui::FontId::proportional(24.0);
        for (dx, dy) in [(-1.5, -1.5), (1.5, -1.5), (-1.5, 1.5), (1.5, 1.5)] {
            painter.text(
                screen + egui::vec2(dx, dy),
                egui::Align2::CENTER_CENTER,
                sym,
                font.clone(),
                egui::Color32::BLACK,
            );
        }
        painter.text(
            screen,
            egui::Align2::CENTER_CENTER,
            sym,
            font,
            egui::Color32::WHITE,
        );
    }
}

/// Détermine le marqueur d'un donneur de quête selon le QuestLog du joueur.
/// Priorité : à rendre (`?` or) > dispo (`!` or) > en cours (`?` gris).
fn marker_for(giver: &QuestGiver, log: &QuestLog) -> Option<(&'static str, egui::Color32)> {
    const GOLD: egui::Color32 = egui::Color32::from_rgb(244, 196, 48);
    const GREY: egui::Color32 = egui::Color32::from_rgb(150, 150, 150);
    for qid in &giver.completes {
        if log
            .entries
            .get(qid)
            .is_some_and(|s| s.status == QuestStatus::Completed)
        {
            return Some(("?", GOLD));
        }
    }
    for qid in &giver.offers {
        if !log.entries.contains_key(qid) {
            return Some(("!", GOLD));
        }
    }
    for qid in &giver.offers {
        if log
            .entries
            .get(qid)
            .is_some_and(|s| s.status == QuestStatus::Active)
        {
            return Some(("?", GREY));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaRpgPlugin;
    }

    #[test]
    fn terrain_height_deterministic_via_pipeline() {
        let cfg = make_terrain_config();
        let a = terrain_height_local(5.0, 10.0, &cfg);
        let b = terrain_height_local(5.0, 10.0, &cfg);
        assert_eq!(a, b);
    }

    // ── spawn_rex_character race-condition fix (story-438 / playtest 2026-05-16) ──
    // Bug : spawn_rex_character était dans OnEnter(Rpg) → tournait avant spawn Player
    // (autre plugin, même OnEnter) → warn "no Player found" + Rex jamais spawné.
    // Fix : déplacé en Update GameSet::Movement + guard idempotent.
    // Tests : prouver que (a) no-op sans Player, (b) spawn correct, (c) idempotent.

    use crate::character::{spawn_rex_character, RexCharacter};
    use bevy::asset::AssetPlugin;
    use bevy::scene::ScenePlugin;
    use forgia_player::prelude::Player;

    fn make_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(ScenePlugin)
            .add_systems(Update, spawn_rex_character);
        app
    }

    #[test]
    fn spawn_rex_noop_when_no_player() {
        let mut app = make_test_app();
        app.update();
        let world = app.world_mut();
        let mut q = world.query::<&RexCharacter>();
        assert_eq!(
            q.iter(world).count(),
            0,
            "Rex ne doit PAS spawn si Player absent (anti race-condition)"
        );
    }

    #[test]
    fn spawn_rex_when_player_present() {
        let mut app = make_test_app();
        app.world_mut().spawn(Player::default());
        app.update();
        let world = app.world_mut();
        let mut q = world.query::<&RexCharacter>();
        assert_eq!(
            q.iter(world).count(),
            1,
            "Rex doit spawn UNE fois quand Player présent"
        );
    }

    #[test]
    fn spawn_rex_idempotent_across_frames() {
        let mut app = make_test_app();
        app.world_mut().spawn(Player::default());
        // 5 frames consécutives — Rex doit rester unique (guard q_existing_rex)
        for _ in 0..5 {
            app.update();
        }
        let world = app.world_mut();
        let mut q = world.query::<&RexCharacter>();
        assert_eq!(
            q.iter(world).count(),
            1,
            "Rex doit rester unique (guard idempotent)"
        );
    }

    #[test]
    fn spawn_rex_retry_after_late_player() {
        // Reproduit la race-condition réelle : 1ère frame sans Player → no-op.
        // Player apparaît frame 2 → Rex doit spawner.
        let mut app = make_test_app();
        app.update(); // T0 : no player, no-op
        {
            let world = app.world_mut();
            let mut q = world.query::<&RexCharacter>();
            assert_eq!(q.iter(world).count(), 0);
        }
        app.world_mut().spawn(Player::default());
        app.update(); // T1 : player présent, Rex spawn
        let world = app.world_mut();
        let mut q = world.query::<&RexCharacter>();
        assert_eq!(
            q.iter(world).count(),
            1,
            "Rex doit spawn quand Player apparaît à T1 (retry-jusqu'à-success)"
        );
    }

    #[test]
    fn terrain_height_finite_inside_chunk() {
        let cfg = make_terrain_config();
        for x in (0..32).step_by(4) {
            for z in (0..32).step_by(4) {
                let h = terrain_height_local(x as f32, z as f32, &cfg);
                assert!(h.is_finite(), "h={} not finite at ({},{})", h, x, z);
                assert!(
                    h >= 0.0 && h <= cfg.max_height * 1.1,
                    "h={} out of expected bounds at ({},{})",
                    h,
                    x,
                    z
                );
            }
        }
    }
}
