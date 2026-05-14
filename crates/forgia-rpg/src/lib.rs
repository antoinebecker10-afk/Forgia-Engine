//! # forgia-rpg
//!
//! RPG OpenWorld mode plugin. Spawns world OnEnter(GameMode::Rpg), cleanup OnExit.
//!
//! Phase 0 V1 :
//! - **Procedural heightmap ground** (noise-based, 80×80m, 64×64 grid) with
//!   per-vertex elevation and proper Collider::heightfield for physics.
//! - 2 buildings + 5 typed NPCs with InteractablePoint.
//! - **Interaction system** : player presses E within radius → triggers
//!   building label log OR `StartDialogue` to forgia-dialogue.
//!
//! Phase M2 : forgia-terrain streaming (chunks, biomes, full V1 port).

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_core::prelude::*;
use forgia_dialogue::{
    DialogueChoice, DialogueEffect, DialogueId, DialogueNode, DialogueRegistry, DialogueTree,
    NodeId, StartDialogue,
};
use std::collections::HashMap;
use forgia_input::PlayerAction;
use forgia_player::prelude::Player;
use leafwing_input_manager::prelude::*;
use noise::{NoiseFn, Perlin};

pub mod prelude {
    pub use crate::{
        ForgiaRpgPlugin, InteractablePoint, Npc, RpgWorldMarker,
    };
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

// ── Terrain config (Phase 0 hardcoded — replaced by forgia-terrain port) ────
// INTERIM : while forgia-terrain port is in flight (background agent), this
// in-place heightmap delivers immediate visual : multi-octave noise, per-vertex
// biome colors (sand/grass/rock/snow), real V1 grass PBR textures.
const TERRAIN_SIZE: f32 = 80.0;
const TERRAIN_GRID: usize = 96;        // 96×96 quads (denser for smoother slopes)
const TERRAIN_AMPLITUDE: f32 = 5.5;    // ±5.5m hills (more drama)
const TERRAIN_FREQ: f64 = 0.035;
const TERRAIN_SEED: u32 = 1337;
const TERRAIN_OCTAVES: usize = 4;
const UV_TILES: f32 = 16.0;            // grass texture tiles 16×16 across map

// Biome height bands for vertex coloring
const Y_SAND_TOP: f32 = 0.4;
const Y_GRASS_TOP: f32 = 2.5;
const Y_ROCK_TOP: f32 = 4.5;

pub struct ForgiaRpgPlugin;

impl Plugin for ForgiaRpgPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, register_sample_dialogues)
            .add_systems(OnEnter(GameMode::Rpg), spawn_world)
            .add_systems(OnExit(GameMode::Rpg), cleanup_world)
            .add_systems(
                Update,
                interact_system.run_if(in_state(GameMode::Rpg)),
            );
    }
}

/// Multi-octave noise sampler. 4 octaves with detail layer + ridge mix.
fn terrain_height(noise: &Perlin, world_x: f32, world_z: f32) -> f32 {
    let mut amp = 1.0_f32;
    let mut freq = TERRAIN_FREQ;
    let mut sum = 0.0_f32;
    let mut max = 0.0_f32;
    for _ in 0..TERRAIN_OCTAVES {
        let n = noise.get([world_x as f64 * freq, world_z as f64 * freq]) as f32;
        sum += n * amp;
        max += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    // Ridge: 1 - |noise| for sharper hill crests, mixed 35%
    let ridge_n = noise.get([world_x as f64 * TERRAIN_FREQ * 1.7, world_z as f64 * TERRAIN_FREQ * 1.7]) as f32;
    let ridge = 1.0 - ridge_n.abs();
    let normalized = sum / max;
    let mixed = normalized * 0.7 + ridge * 0.3;
    mixed * TERRAIN_AMPLITUDE
}

/// Per-vertex biome color based on altitude + slope.
fn biome_color(h: f32, slope: f32) -> [f32; 4] {
    // slope = 1 - normal.y (0=flat, 1=cliff). Steep terrain leans rock regardless of height.
    let rock_blend = (slope * 2.0).clamp(0.0, 1.0);

    let base: [f32; 3] = if h < Y_SAND_TOP {
        [0.78, 0.70, 0.50]                 // sand
    } else if h < Y_GRASS_TOP {
        // grass with subtle gradient
        let t = (h - Y_SAND_TOP) / (Y_GRASS_TOP - Y_SAND_TOP);
        let r = 0.30 + t * 0.05;
        let g = 0.50 + t * 0.08;
        let b = 0.20 + t * 0.05;
        [r, g, b]
    } else if h < Y_ROCK_TOP {
        // rocky transition
        let t = (h - Y_GRASS_TOP) / (Y_ROCK_TOP - Y_GRASS_TOP);
        let r = 0.35 + t * 0.20;
        let g = 0.32 + t * 0.18;
        let b = 0.25 + t * 0.15;
        [r, g, b]
    } else {
        // snow / high alpine
        let t = ((h - Y_ROCK_TOP) / 2.0).clamp(0.0, 1.0);
        let v = 0.85 + t * 0.10;
        [v, v, v]
    };

    // mix rock at slope>0.4 (steep cliffs)
    let rock: [f32; 3] = [0.45, 0.42, 0.38];
    let mix = |a: f32, b: f32| a * (1.0 - rock_blend) + b * rock_blend;
    [mix(base[0], rock[0]), mix(base[1], rock[1]), mix(base[2], rock[2]), 1.0]
}

/// Build heightmap mesh with multi-octave noise, biome vertex colors, tiled UVs.
fn build_terrain_mesh(noise: &Perlin) -> (Mesh, Vec<f32>) {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    let grid = TERRAIN_GRID;
    let step = TERRAIN_SIZE / grid as f32;
    let half = TERRAIN_SIZE * 0.5;
    let n_verts = (grid + 1) * (grid + 1);

    let mut positions = Vec::with_capacity(n_verts);
    let mut uvs = Vec::with_capacity(n_verts);
    let mut heights = Vec::with_capacity(n_verts);

    // Pass 1: positions + heights
    for z in 0..=grid {
        for x in 0..=grid {
            let wx = x as f32 * step - half;
            let wz = z as f32 * step - half;
            let h = terrain_height(noise, wx, wz);
            positions.push([wx, h, wz]);
            // Tile UV multiple times across surface for texture repetition
            uvs.push([
                (x as f32 / grid as f32) * UV_TILES,
                (z as f32 / grid as f32) * UV_TILES,
            ]);
            heights.push(h);
        }
    }

    // Indices
    let mut indices = Vec::with_capacity(grid * grid * 6);
    let stride = grid + 1;
    for z in 0..grid {
        for x in 0..grid {
            let i = (z * stride + x) as u32;
            let i_right = i + 1;
            let i_down = i + stride as u32;
            let i_diag = i_down + 1;
            indices.push(i);
            indices.push(i_down);
            indices.push(i_right);
            indices.push(i_right);
            indices.push(i_down);
            indices.push(i_diag);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();

    // Pass 2: per-vertex colors using computed normals for slope
    let normals = mesh
        .attribute(Mesh::ATTRIBUTE_NORMAL)
        .and_then(|a| a.as_float3())
        .map(|n| n.to_vec())
        .unwrap_or_default();
    let mut colors = Vec::with_capacity(n_verts);
    for (i, pos) in positions.iter().enumerate() {
        let slope = if i < normals.len() {
            1.0 - normals[i][1].clamp(0.0, 1.0)
        } else {
            0.0
        };
        colors.push(biome_color(pos[1], slope));
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

    (mesh, heights)
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // ── Procedural heightmap ground ──────────────────────────────────────
    let noise = Perlin::new(TERRAIN_SEED);
    let (mesh, heights) = build_terrain_mesh(&noise);
    let mesh_handle = meshes.add(mesh);

    // Real V1 grass PBR textures (junction `textures-v1/` -> V1 assets).
    let grass_diff: Handle<Image> = asset_server.load("textures-v1/terrain/grass/diff.jpg");
    let grass_normal: Handle<Image> = asset_server.load("textures-v1/terrain/grass/normal.jpg");
    let grass_rough: Handle<Image> = asset_server.load("textures-v1/terrain/grass/roughness.jpg");

    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE, // multiplied by vertex color + texture
        base_color_texture: Some(grass_diff),
        normal_map_texture: Some(grass_normal),
        metallic_roughness_texture: Some(grass_rough),
        perceptual_roughness: 0.90,
        reflectance: 0.05,
        ..default()
    });

    let nrows = TERRAIN_GRID + 1;
    let ncols = TERRAIN_GRID + 1;

    commands.spawn((
        RpgWorldMarker,
        Mesh3d(mesh_handle),
        MeshMaterial3d(ground_mat),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RigidBody::Fixed,
        Collider::heightfield(
            heights.clone(),
            nrows,
            ncols,
            Vec3::new(TERRAIN_SIZE, 1.0, TERRAIN_SIZE),
        ),
        Name::new("RpgTerrain"),
    ));

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

    // ── Buildings (2 cuboids posés sur terrain) ──────────────────────────
    let wood_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.40, 0.25),
        perceptual_roughness: 0.85,
        ..default()
    });
    let building_mesh = meshes.add(Cuboid::new(6.0, 4.0, 6.0));
    for (i, (x, z)) in [(-10.0_f32, -8.0_f32), (12.0, -5.0)].iter().enumerate() {
        let y_ground = terrain_height(&noise, *x, *z);
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

    // ── NPCs posés sur terrain ───────────────────────────────────────────
    let npc_mesh = meshes.add(Capsule3d::new(0.4, 1.2));
    let npc_data = [
        ("Forgeron Aldric",  "Bienvenue voyageur. J'ai besoin d'aide aux mines.", -5.0_f32,  0.0_f32, Color::srgb(0.8, 0.3, 0.2)),
        ("Marchande Lyra",   "Mes étals sont ouverts. Voulez-vous troquer ?",     0.0,   5.0, Color::srgb(0.3, 0.5, 0.8)),
        ("Garde Brennus",    "Halte ! Identifiez-vous, étranger.",                5.0,   0.0, Color::srgb(0.4, 0.4, 0.4)),
        ("Sage Eldwyn",      "Les anciens parlent de prophéties...",             -3.0,  -5.0, Color::srgb(0.6, 0.4, 0.7)),
        ("Aubergiste Mira",  "Un lit chaud et une bière fraîche, voyageur ?",     8.0,   3.0, Color::srgb(0.7, 0.6, 0.3)),
    ];
    for (name, greeting, x, z, color) in npc_data {
        let y_ground = terrain_height(&noise, x, z);
        let mat = materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.8,
            ..default()
        });
        commands.spawn((
            RpgWorldMarker,
            Npc {
                name: name.to_string(),
                greeting: greeting.to_string(),
            },
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
        "[forgia-rpg] World spawned : procedural heightmap {0}x{0}m grid {1}x{1} + sun + 2 buildings + 5 NPCs",
        TERRAIN_SIZE as u32, TERRAIN_GRID
    );
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

fn cleanup_world(mut commands: Commands, q: Query<Entity, With<RpgWorldMarker>>) {
    let count = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    info!("[forgia-rpg] World cleaned : {} entities despawned", count);
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
        if d <= ip.radius {
            if best.is_none() || d < best.unwrap().1 {
                best = Some((e, d, ip, npc));
            }
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
    fn terrain_height_deterministic() {
        let n1 = Perlin::new(TERRAIN_SEED);
        let n2 = Perlin::new(TERRAIN_SEED);
        assert_eq!(terrain_height(&n1, 5.0, 10.0), terrain_height(&n2, 5.0, 10.0));
    }

    #[test]
    fn terrain_height_bounded() {
        let noise = Perlin::new(TERRAIN_SEED);
        for x in (-40..40).step_by(5) {
            for z in (-40..40).step_by(5) {
                let h = terrain_height(&noise, x as f32, z as f32);
                assert!(h.abs() < TERRAIN_AMPLITUDE * 2.0, "h={} out of bounds", h);
            }
        }
    }
}
