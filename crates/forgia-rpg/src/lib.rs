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
use bevy_rapier3d::prelude::*;
use forgia_core::prelude::*;
use forgia_dialogue::{
    DialogueChoice, DialogueEffect, DialogueId, DialogueNode, DialogueRegistry, DialogueTree,
    NodeId, StartDialogue,
};
use forgia_input::PlayerAction;
use forgia_player::prelude::Player;
use forgia_foliage::prelude::VegetationManager;
use forgia_foliage::{RpgSampleOffset, VegetationTree};
use forgia_terrain::{
    build_chunk_mesh, spawn_chunk_entity, BiomeMap, ChunkCoord, ChunkManager, Lod2TileManager,
    LodSampleOffset, LodStats, MapGenConfig, TerrainConfig, TerrainSharedMaterial,
};
use leafwing_input_manager::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

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

// ── W1/W2 — World layout constants ──────────────────────────────────────
// `sample_offset = map_size/2` : on échantillonne au centre du monde virtuel
// pour éviter le `edge_falloff` qui aplatit les bords [0, map_size].
// Player visuel reste autour de l'origine.
const RPG_MAP_SIZE: f32 = 2048.0;
const RPG_SEED: u32 = 1337;
const RPG_SEA_LEVEL: f32 = 4.0;
const RPG_MAX_HEIGHT: f32 = 28.0;

/// Rayon Manhattan de streaming chunks. Cible la couverture LOD1 (320m) :
/// ceil(LOD1_MAX_M / CHUNK_X) = 10. Au-delà : LOD2 mega-tiles, pas de chunk.
const RENDER_DIST: i32 = 10;
/// Chunks max meshés par frame (anti-freeze démarrage). 4 pour atteindre la
/// pleine ring LOD1 en ~1s (221 chunks Manhattan disk).
const CHUNKS_PER_FRAME: usize = 4;
/// W2 — intervalle d'export sensor JSON (secondes).
const SENSOR_INTERVAL_S: f32 = 1.0;

pub struct ForgiaRpgPlugin;

impl Plugin for ForgiaRpgPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, register_sample_dialogues)
            .add_systems(OnEnter(GameMode::Rpg), spawn_world)
            .add_systems(OnExit(GameMode::Rpg), cleanup_world)
            .add_systems(
                Update,
                (
                    teleport_player_to_terrain,
                    stream_chunks_around_player,
                    write_chunks_sensor,
                    interact_system,
                )
                    .chain()
                    .run_if(in_state(GameMode::Rpg)),
            );
    }
}

/// Returns the world-space offset applied when sampling the noise pipeline,
/// so that the visual origin (0,0,0) sees the interior of the procedural
/// world instead of `edge_falloff`-flattened borders.
fn sample_offset() -> Vec2 {
    Vec2::splat(RPG_MAP_SIZE * 0.5)
}

fn make_terrain_config() -> TerrainConfig {
    TerrainConfig {
        map_size: RPG_MAP_SIZE,
        seed: RPG_SEED,
        sea_level: RPG_SEA_LEVEL,
        max_height: RPG_MAX_HEIGHT,
        streaming_radius: 4,
        chunks_per_frame: 2,
        y_offset: 0.0,
    }
}

fn make_map_gen_config() -> MapGenConfig {
    // `preset_forgia_showcase` (default) utilise `BiomeMode::Directional` →
    // 5 zones cardinales rigides. On force `Voronoi` pour avoir 10 biomes
    // hexagonaux distribués naturellement (W3 ready).
    MapGenConfig {
        seed: RPG_SEED,
        map_size: RPG_MAP_SIZE,
        sea_level: RPG_SEA_LEVEL,
        max_height: RPG_MAX_HEIGHT,
        island_mode: false, // pas d'île pour le vertical slice 1 chunk
        biome_mode: forgia_terrain::BiomeMode::Voronoi,
        biome_cell_size: 96.0, // cellules + petites → biomes visibles au W1 (32m chunk)
        ..MapGenConfig::default()
    }
}

fn terrain_height_local(x: f32, z: f32, config: &TerrainConfig) -> f32 {
    let off = sample_offset();
    forgia_terrain::heightmap_at(x + off.x, z + off.y, config)
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

    // ── 1 chunk static à l'origine (W1 vertical slice) ───────────────────
    let coord = ChunkCoord::new(0, 0);
    let mesh_data = build_chunk_mesh(coord, sample_offset(), &terrain_cfg, &biome_map);
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

    commands.insert_resource(terrain_cfg);
    commands.insert_resource(map_cfg);
    commands.insert_resource(biome_map);
    commands.insert_resource(chunk_mgr);
    // forgia-foliage + forgia-terrain LOD : aligne les samples (heightmap + biome)
    // avec notre décalage RPG (sample_offset = map_size/2).
    let off = sample_offset();
    commands.insert_resource(RpgSampleOffset { x: off.x, z: off.y });
    commands.insert_resource(LodSampleOffset { x: off.x, z: off.y });
    // Marque qu'un teleport joueur doit firer ce cycle Rpg (consommé en Update).
    commands.insert_resource(PendingPlayerTeleport);

    // ── Sun ──────────────────────────────────────────────────────────────
    commands.spawn((
        RpgWorldMarker,
        DirectionalLight {
            illuminance: 12_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(20.0, 30.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("RpgSun"),
    ));

    // ── Buildings + NPCs posés via sampler local sur le terrain procédural ──
    let tcfg = make_terrain_config();
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.40, 0.25),
        perceptual_roughness: 0.85,
        ..default()
    });
    let building_mesh = meshes.add(Cuboid::new(6.0, 4.0, 6.0));
    for (i, (x, z)) in [(8.0_f32, 6.0_f32), (20.0, 10.0)].iter().enumerate() {
        let y_ground = terrain_height_local(*x, *z, &tcfg);
        commands.spawn((
            RpgWorldMarker,
            Mesh3d(building_mesh.clone()),
            MeshMaterial3d(wood_mat.clone()),
            Transform::from_xyz(*x, y_ground + 2.0, *z),
            RigidBody::Fixed,
            Collider::cuboid(3.0, 2.0, 3.0),
            InteractablePoint {
                label: format!("Maison #{}", i + 1),
                radius: 4.0,
            },
            Name::new(format!("Building{}", i + 1)),
        ));
    }

    let npc_mesh = meshes.add(Capsule3d::new(0.4, 1.2));
    let npc_data = [
        ("Forgeron Aldric",  "Bienvenue voyageur. J'ai besoin d'aide aux mines.", 12.0_f32, 14.0_f32, Color::srgb(0.8, 0.3, 0.2)),
        ("Marchande Lyra",   "Mes étals sont ouverts. Voulez-vous troquer ?",     16.0,    18.0,    Color::srgb(0.3, 0.5, 0.8)),
        ("Garde Brennus",    "Halte ! Identifiez-vous, étranger.",                22.0,    16.0,    Color::srgb(0.4, 0.4, 0.4)),
        ("Sage Eldwyn",      "Les anciens parlent de prophéties...",              10.0,    22.0,    Color::srgb(0.6, 0.4, 0.7)),
        ("Aubergiste Mira",  "Un lit chaud et une bière fraîche, voyageur ?",     26.0,    20.0,    Color::srgb(0.7, 0.6, 0.3)),
    ];
    for (name, greeting, x, z, color) in npc_data {
        let y_ground = terrain_height_local(x, z, &tcfg);
        let mat = materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.8,
            ..default()
        });
        commands.spawn((
            RpgWorldMarker,
            Npc { name: name.to_string(), greeting: greeting.to_string() },
            InteractablePoint {
                label: format!("Parler à {}", name),
                radius: 2.5,
            },
            Mesh3d(npc_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_xyz(x, y_ground + 1.0, z),
            RigidBody::Fixed,
            Collider::capsule_y(0.6, 0.4),
            Name::new(name.to_string()),
        ));
    }

    info!(
        "[forgia-rpg] W1 World spawned : 1 chunk (32×32m, heightmap-grid) + sun + 2 buildings + 5 NPCs"
    );
}

/// W2 — Stream les chunks dans un rayon Manhattan `RENDER_DIST` autour du joueur.
/// - Si le joueur n'a pas changé de chunk : skip total (early return).
/// - Spawn `CHUNKS_PER_FRAME` max par frame depuis la queue de pending (anti-freeze).
/// - Despawn les chunks hors rayon (entité + retire de `loaded_entities`).
fn stream_chunks_around_player(
    mut commands: Commands,
    mut chunk_mgr: ResMut<ChunkManager>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    biome_map: Option<Res<BiomeMap>>,
    shared_mat: Option<Res<TerrainSharedMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    player_q: Query<&Transform, With<Player>>,
    mut pending: Local<VecDeque<ChunkCoord>>,
) {
    let (Some(terrain_cfg), Some(biome_map), Some(shared_mat)) =
        (terrain_cfg, biome_map, shared_mat) else { return };
    let Ok(player_tf) = player_q.single() else { return };

    let player_chunk = ChunkCoord::from_world(player_tf.translation);
    let need_recompute_set = chunk_mgr.last_player_chunk != Some(player_chunk);

    // 1. Si traversée de frontière chunk : recalculer desired set + diff load/unload.
    if need_recompute_set {
        let mut desired: HashSet<ChunkCoord> = HashSet::new();
        for dx in -RENDER_DIST..=RENDER_DIST {
            for dz in -RENDER_DIST..=RENDER_DIST {
                if dx.abs() + dz.abs() <= RENDER_DIST {
                    desired.insert(ChunkCoord::new(player_chunk.x + dx, player_chunk.z + dz));
                }
            }
        }

        // Unload chunks hors rayon
        let to_remove: Vec<ChunkCoord> = chunk_mgr
            .loaded_entities
            .keys()
            .filter(|c| !desired.contains(c))
            .copied()
            .collect();
        for coord in to_remove {
            if let Some(entity) = chunk_mgr.loaded_entities.remove(&coord) {
                commands.entity(entity).despawn();
            }
        }

        // Queue les chunks manquants (proche d'abord pour visu prioritaire)
        pending.clear();
        let mut sorted: Vec<ChunkCoord> = desired
            .into_iter()
            .filter(|c| !chunk_mgr.loaded_entities.contains_key(c))
            .collect();
        sorted.sort_by_key(|c| c.distance(&player_chunk));
        for c in sorted {
            pending.push_back(c);
        }

        chunk_mgr.last_player_chunk = Some(player_chunk);
    }

    // 2. Mesh+spawn ≤ CHUNKS_PER_FRAME pour amortir le coût.
    let off = sample_offset();
    for _ in 0..CHUNKS_PER_FRAME {
        let Some(coord) = pending.pop_front() else { break };
        if chunk_mgr.loaded_entities.contains_key(&coord) { continue; }
        let mesh_data = build_chunk_mesh(coord, off, &terrain_cfg, &biome_map);
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
    }
}

/// W2 — Sensor `forgia_chunks_snapshot.json` (observability-required.md).
fn write_chunks_sensor(
    time: Res<Time>,
    chunk_mgr: Option<Res<ChunkManager>>,
    biome_map: Option<Res<BiomeMap>>,
    terrain_cfg: Option<Res<TerrainConfig>>,
    mut last_write: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last_write < SENSOR_INTERVAL_S { return; }
    *last_write = now;

    let (Some(chunk_mgr), Some(biome_map), Some(terrain_cfg)) =
        (chunk_mgr, biome_map, terrain_cfg) else { return };

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
        RENDER_DIST,
        terrain_cfg.map_size,
        terrain_cfg.sea_level,
        terrain_cfg.max_height,
        dist_json,
    );
    let _ = std::fs::write("forgia_chunks_snapshot.json", json);
}

/// Marqueur Resource posée OnEnter(Rpg), consommée par `teleport_player_to_terrain`
/// dès qu'il a vraiment téléporté le joueur (présence Player + TerrainConfig).
/// Garantit que la téléportation fire à CHAQUE entrée Rpg (vs Local<bool> qui
/// persistait à travers les state transitions → joueur restait à (0,2,0) en
/// dessous du terrain h=15+, fall through).
#[derive(Resource)]
struct PendingPlayerTeleport;

fn teleport_player_to_terrain(
    mut commands: Commands,
    pending: Option<Res<PendingPlayerTeleport>>,
    cfg: Option<Res<TerrainConfig>>,
    mut q: Query<&mut Transform, With<Player>>,
) {
    if pending.is_none() { return; }
    let Some(cfg) = cfg else { return };
    let Ok(mut tf) = q.single_mut() else { return };
    // Pose le joueur à world (16, h+2, 16) — milieu du chunk (0,0).
    let target_x = 16.0_f32;
    let target_z = 16.0_f32;
    let h = terrain_height_local(target_x, target_z, &cfg);
    tf.translation = Vec3::new(target_x, h + 2.0, target_z);
    commands.remove_resource::<PendingPlayerTeleport>();
    info!("[forgia-rpg] Player teleported to terrain surface (h={:.2})", h);
}

/// Register sample DialogueTrees so the E-interact loop has content to show.
/// Each NPC's tree_id follows the convention "npc_<lowercase_name_with_underscores>".
fn register_sample_dialogues(mut registry: ResMut<DialogueRegistry>) {
    fn node(speaker: &str, line: &str, choices: Vec<(&str, Option<&str>, Vec<DialogueEffect>)>) -> DialogueNode {
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

    // ── Forgeron Aldric : quête mines ─────────────────────────────────────
    let mut aldric = DialogueTree {
        id: DialogueId("npc_forgeron_aldric".into()),
        root: NodeId("greet".into()),
        nodes: HashMap::new(),
    };
    aldric.nodes.insert(NodeId("greet".into()), node(
        "Forgeron Aldric",
        "Bienvenue voyageur. Mes mines sont infestées de gobelins, j'ai besoin d'aide.",
        vec![
            ("J'accepte la quête.", Some("accept"), vec![DialogueEffect::StartQuest { id: "kill_goblins".into() }]),
            ("Pas maintenant.", Some("refuse"), vec![]),
            ("Au revoir.", None, vec![DialogueEffect::EndConversation]),
        ],
    ));
    aldric.nodes.insert(NodeId("accept".into()), node(
        "Forgeron Aldric",
        "Excellent. Tuez 5 gobelins et revenez me voir. Voici une dague pour commencer.",
        vec![
            ("Merci.", None, vec![
                DialogueEffect::GiveItem { id: "iron_dagger".into(), count: 1 },
                DialogueEffect::EndConversation,
            ]),
        ],
    ));
    aldric.nodes.insert(NodeId("refuse".into()), node(
        "Forgeron Aldric",
        "Comme vous voudrez. Mais les mines ne se nettoieront pas toutes seules...",
        vec![("Au revoir.", None, vec![DialogueEffect::EndConversation])],
    ));
    registry.trees.insert(aldric.id.clone(), aldric);

    // ── Marchande Lyra : commerce ─────────────────────────────────────────
    let mut lyra = DialogueTree {
        id: DialogueId("npc_marchande_lyra".into()),
        root: NodeId("greet".into()),
        nodes: HashMap::new(),
    };
    lyra.nodes.insert(NodeId("greet".into()), node(
        "Marchande Lyra",
        "Mes étals sont ouverts ! Que cherchez-vous ?",
        vec![
            ("Vos potions.", Some("potions"), vec![]),
            ("Vos armes.", Some("weapons"), vec![]),
            ("Rien, merci.", None, vec![DialogueEffect::EndConversation]),
        ],
    ));
    lyra.nodes.insert(NodeId("potions".into()), node(
        "Marchande Lyra",
        "Une fiole de soin pour 10 pièces ?",
        vec![
            ("J'achète.", None, vec![
                DialogueEffect::GiveItem { id: "potion_heal".into(), count: 1 },
                DialogueEffect::EndConversation,
            ]),
            ("Retour", Some("greet"), vec![]),
        ],
    ));
    lyra.nodes.insert(NodeId("weapons".into()), node(
        "Marchande Lyra",
        "Je n'ai qu'un poignard rouillé pour le moment.",
        vec![
            ("Tant pis.", Some("greet"), vec![]),
        ],
    ));
    registry.trees.insert(lyra.id.clone(), lyra);

    info!("[forgia-rpg] Registered 2 sample dialogue trees (Aldric + Lyra)");
}

fn cleanup_world(
    mut commands: Commands,
    q: Query<Entity, With<RpgWorldMarker>>,
    trees: Query<Entity, With<VegetationTree>>,
    mut lod2_mgr: ResMut<Lod2TileManager>,
) {
    let count = q.iter().count();
    for e in &q { commands.entity(e).despawn(); }
    let tree_count = trees.iter().count();
    for e in &trees { commands.entity(e).despawn(); }
    // LOD2 mega-tiles : despawn explicite (entités non taggées RpgWorldMarker).
    lod2_mgr.despawn_all(&mut commands);
    // Resources terrain — TerrainSharedMaterial conservé (réutilisable session suivante).
    commands.remove_resource::<ChunkManager>();
    commands.remove_resource::<BiomeMap>();
    commands.remove_resource::<MapGenConfig>();
    commands.remove_resource::<TerrainConfig>();
    commands.remove_resource::<RpgSampleOffset>();
    commands.remove_resource::<LodSampleOffset>();
    commands.insert_resource(LodStats::default());
    commands.insert_resource(VegetationManager::default());
    info!(
        "[forgia-rpg] World cleaned : {} entities + {} trees + LOD2 tiles despawned",
        count, tree_count,
    );
}

/// Interaction system : when player presses E, find nearest InteractablePoint
/// within its radius. Triggers StartDialogue for NPCs, info log for buildings.
fn interact_system(
    players: Query<(Entity, &Transform, &ActionState<PlayerAction>), With<Player>>,
    interactables: Query<(Entity, &Transform, &InteractablePoint, Option<&Npc>)>,
    mut start_dialogue: MessageWriter<StartDialogue>,
) {
    let Ok((player_e, player_tf, action)) = players.single() else { return };
    if !action.just_pressed(&PlayerAction::Interact) {
        return;
    }
    let player_pos = player_tf.translation;

    // Find closest interactable in radius
    let mut best: Option<(Entity, f32, &InteractablePoint, Option<&Npc>)> = None;
    for (e, tf, ip, npc) in &interactables {
        let d = tf.translation.distance(player_pos);
        if d <= ip.radius && (best.is_none() || d < best.unwrap().1) {
            best = Some((e, d, ip, npc));
        }
    }

    let Some((target, dist, ip, npc)) = best else {
        info!("[interact] no interactable in range");
        return;
    };

    info!("[interact] '{}' at {:.1}m", ip.label, dist);

    if let Some(npc) = npc {
        info!("[dialogue] {} : « {} »", npc.name, npc.greeting);
        start_dialogue.write(StartDialogue {
            player: player_e,
            npc: target,
            tree_id: DialogueId(format!("npc_{}", npc.name.to_lowercase().replace(' ', "_"))),
        });
    }
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

    #[test]
    fn terrain_height_finite_inside_chunk() {
        let cfg = make_terrain_config();
        for x in (0..32).step_by(4) {
            for z in (0..32).step_by(4) {
                let h = terrain_height_local(x as f32, z as f32, &cfg);
                assert!(h.is_finite(), "h={} not finite at ({},{})", h, x, z);
                assert!(h >= 0.0 && h <= RPG_MAX_HEIGHT * 1.1,
                    "h={} out of expected bounds at ({},{})", h, x, z);
            }
        }
    }
}
