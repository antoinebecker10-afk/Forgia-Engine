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
use forgia_dialogue::{DialogueId, StartDialogue};
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

// ── Terrain config (Phase 0 hardcoded — V2 via genome) ───────────────────────
const TERRAIN_SIZE: f32 = 80.0;
const TERRAIN_GRID: usize = 64;        // 64×64 quads
const TERRAIN_AMPLITUDE: f32 = 3.5;    // ±3.5m hills
const TERRAIN_FREQ: f64 = 0.04;
const TERRAIN_SEED: u32 = 1337;

pub struct ForgiaRpgPlugin;

impl Plugin for ForgiaRpgPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameMode::Rpg), spawn_world)
            .add_systems(OnExit(GameMode::Rpg), cleanup_world)
            .add_systems(
                Update,
                interact_system.run_if(in_state(GameMode::Rpg)),
            );
    }
}

/// Heightmap noise sampler.
fn terrain_height(noise: &Perlin, world_x: f32, world_z: f32) -> f32 {
    let n = noise.get([world_x as f64 * TERRAIN_FREQ, world_z as f64 * TERRAIN_FREQ]);
    let n2 = noise.get([world_x as f64 * TERRAIN_FREQ * 2.7, world_z as f64 * TERRAIN_FREQ * 2.7]);
    (n + n2 * 0.35) as f32 * TERRAIN_AMPLITUDE
}

/// Build the heightmap mesh.
fn build_terrain_mesh(noise: &Perlin) -> (Mesh, Vec<f32>) {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};

    let grid = TERRAIN_GRID;
    let step = TERRAIN_SIZE / grid as f32;
    let half = TERRAIN_SIZE * 0.5;

    let mut positions = Vec::with_capacity((grid + 1) * (grid + 1));
    let mut uvs = Vec::with_capacity((grid + 1) * (grid + 1));
    let mut heights = Vec::with_capacity((grid + 1) * (grid + 1));

    for z in 0..=grid {
        for x in 0..=grid {
            let wx = x as f32 * step - half;
            let wz = z as f32 * step - half;
            let h = terrain_height(noise, wx, wz);
            positions.push([wx, h, wz]);
            uvs.push([x as f32 / grid as f32, z as f32 / grid as f32]);
            heights.push(h);
        }
    }

    // Indices (2 triangles per quad)
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
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();

    (mesh, heights)
}

fn spawn_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // ── Procedural heightmap ground ──────────────────────────────────────
    let noise = Perlin::new(TERRAIN_SEED);
    let (mesh, heights) = build_terrain_mesh(&noise);
    let mesh_handle = meshes.add(mesh);
    let ground_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.55, 0.22),
        perceptual_roughness: 0.95,
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
