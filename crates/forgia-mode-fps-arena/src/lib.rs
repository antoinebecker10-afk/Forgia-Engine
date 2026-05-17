//! # forgia-mode-fps-arena
//!
//! Spawn / cleanup arena KayKit Dungeon Pack + clouds orbit.
//!
//! Extrait de `forgia-fps` 2026-05-16 (règle `fine-grained-crates.md`).
//!
//! Pattern V1 :
//! - KayKit walls **`WALL_Y = 0.0`** (LOCK absolu : pivot mesh au sol, pas centre)
//! - `TILE_SIZE = 4.0` (KayKit dungeon convention)
//! - Forgia scaled scene pattern : parent scale=1 + child SceneRoot scale (rapier3d 0.33 quirk)

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_combat::prelude::*;
use forgia_core::prelude::*;

pub mod prelude {
    pub use crate::{ArenaMarker, CloudOrbit, ForgiaModeFpsArenaPlugin, HitZone, TargetCube};
}

/// Zone d'impact sur un training bot. Inseré sur chaque collider enfant
/// (Head/Body) pour permettre damage multiplier en headshot (style Overwatch).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitZone {
    Head,
    Body,
}

pub const TILE_SIZE: f32 = 4.0;
pub const ARENA_SIZE: i32 = 11; // 11×11 tiles = 44×44m

// ── Cloud constants (skybox cubemap V1 wired par forgia-player) ──────
const CLOUD_COLOR: Color = Color::srgb(0.95, 0.96, 0.97);
const CLOUD_EMISSIVE: LinearRgba = LinearRgba::new(0.05, 0.05, 0.06, 1.0);
const CLOUD_ORBIT_SPEED: f32 = 0.025; // rad/s — orbit complet ~4 min

#[derive(Component)]
pub struct ArenaMarker;

/// Marker pour les cubes-cibles (testables via fire_weapon_minimal).
#[derive(Component)]
pub struct TargetCube;

/// Cluster nuage en orbit circulaire autour du centre arène (Y axis).
/// Stocke angle/radius/height pour calcul polaire continu (pas de wrap brusque).
#[derive(Component)]
pub struct CloudOrbit {
    pub angle: f32,
    pub radius: f32,
    pub height: f32,
}

pub struct ForgiaModeFpsArenaPlugin;

impl Plugin for ForgiaModeFpsArenaPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameMode::Fps), spawn_arena)
            .add_systems(OnExit(GameMode::Fps), cleanup_arena)
            .add_systems(
                Update,
                cloud_drift_system
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Fps)),
            );
    }
}

fn spawn_arena(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let floor: Handle<Scene> = asset_server.load("models/kaykit/dungeon/floor.glb#Scene0");
    let floor_dirt: Handle<Scene> = asset_server.load("models/kaykit/dungeon/floor_dirt.glb#Scene0");
    let floor_rocks: Handle<Scene> = asset_server.load("models/kaykit/dungeon/floor_rocks.glb#Scene0");
    let wall: Handle<Scene> = asset_server.load("models/kaykit/dungeon/wall.glb#Scene0");
    let wall_arched: Handle<Scene> = asset_server.load("models/kaykit/dungeon/wall_arched.glb#Scene0");
    let wall_window: Handle<Scene> = asset_server.load("models/kaykit/dungeon/wall_window.glb#Scene0");
    let wall_broken: Handle<Scene> = asset_server.load("models/kaykit/dungeon/wall_broken.glb#Scene0");
    let column: Handle<Scene> = asset_server.load("models/kaykit/dungeon/column.glb#Scene0");
    let pillar: Handle<Scene> = asset_server.load("models/kaykit/dungeon/pillar.glb#Scene0");
    let pillar_deco: Handle<Scene> = asset_server.load("models/kaykit/dungeon/pillar_deco.glb#Scene0");
    let torch: Handle<Scene> = asset_server.load("models/kaykit/dungeon/torch.glb#Scene0");
    let torch_wall: Handle<Scene> = asset_server.load("models/kaykit/dungeon/torch_wall.glb#Scene0");
    let banner_red: Handle<Scene> = asset_server.load("models/kaykit/dungeon/banner_red.glb#Scene0");
    let banner_blue: Handle<Scene> = asset_server.load("models/kaykit/dungeon/banner_blue.glb#Scene0");
    let banner_yellow: Handle<Scene> = asset_server.load("models/kaykit/dungeon/banner_yellow.glb#Scene0");
    let chest: Handle<Scene> = asset_server.load("models/kaykit/dungeon/chest.glb#Scene0");
    let chest_gold: Handle<Scene> = asset_server.load("models/kaykit/dungeon/chest_gold.glb#Scene0");
    let crates: Handle<Scene> = asset_server.load("models/kaykit/dungeon/crates.glb#Scene0");
    let rubble: Handle<Scene> = asset_server.load("models/kaykit/dungeon/rubble.glb#Scene0");
    let table: Handle<Scene> = asset_server.load("models/kaykit/dungeon/table.glb#Scene0");
    let barrel: Handle<Scene> = asset_server.load("models/kaykit/dungeon/barrel.glb#Scene0");
    let barrels_stack: Handle<Scene> = asset_server.load("models/kaykit/dungeon/barrels_stack.glb#Scene0");

    let half = ARENA_SIZE / 2;
    let arena_extent = (ARENA_SIZE as f32 * TILE_SIZE) / 2.0;

    for x in -half..=half {
        for z in -half..=half {
            let dist = x.abs().max(z.abs());
            let tile_handle = if dist >= 4 {
                if (x + z).rem_euclid(3) == 0 {
                    floor_rocks.clone()
                } else {
                    floor_dirt.clone()
                }
            } else {
                floor.clone()
            };
            let pos = Vec3::new(x as f32 * TILE_SIZE, 0.0, z as f32 * TILE_SIZE);
            commands.spawn((
                ArenaMarker,
                Transform::from_translation(pos),
                Visibility::default(),
                Name::new(format!("Floor_{x}_{z}")),
                children![(SceneRoot(tile_handle), Transform::default())],
            ));
        }
    }

    commands.spawn((
        ArenaMarker,
        Transform::from_xyz(0.0, -0.5, 0.0),
        RigidBody::Fixed,
        Collider::cuboid(arena_extent + 5.0, 0.5, arena_extent + 5.0),
        Name::new("ArenaGroundCollider"),
    ));

    let edge = (half as f32 + 0.5) * TILE_SIZE;
    for i in -half..=half {
        let offset = i as f32 * TILE_SIZE;
        let pick_wall = |idx: i32| -> Handle<Scene> {
            match idx.rem_euclid(7) {
                0 => wall_window.clone(),
                3 => wall_broken.clone(),
                _ => wall.clone(),
            }
        };
        let nord = if i == 0 { wall_arched.clone() } else { pick_wall(i) };
        let sud = if i == 0 { wall_arched.clone() } else { pick_wall(i + 1) };
        let est = if i == 0 { wall_arched.clone() } else { pick_wall(i + 2) };
        let ouest = if i == 0 { wall_arched.clone() } else { pick_wall(i + 3) };

        spawn_wall(&mut commands, &nord, Vec3::new(offset, 0.0, edge), 0.0);
        spawn_wall(&mut commands, &sud, Vec3::new(offset, 0.0, -edge), std::f32::consts::PI);
        spawn_wall(&mut commands, &est, Vec3::new(edge, 0.0, offset), -std::f32::consts::FRAC_PI_2);
        spawn_wall(&mut commands, &ouest, Vec3::new(-edge, 0.0, offset), std::f32::consts::FRAC_PI_2);
    }

    let col_d = TILE_SIZE * 2.5;
    for &(x, z) in &[(col_d, col_d), (-col_d, col_d), (col_d, -col_d), (-col_d, -col_d)] {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(x, 0.0, z),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cylinder(2.0, 0.5),
            Name::new(format!("CenterPillar_{x}_{z}")),
            children![(SceneRoot(pillar_deco.clone()), Transform::default())],
        ));
    }

    let outer_d = TILE_SIZE * 4.5;
    for &(x, z) in &[(outer_d, 0.0), (-outer_d, 0.0), (0.0, outer_d), (0.0, -outer_d)] {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(x, 0.0, z),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cylinder(2.0, 0.5),
            Name::new(format!("OuterPillar_{x}_{z}")),
            children![(SceneRoot(pillar.clone()), Transform::default())],
        ));
    }

    for &(x, z) in &[(col_d, 0.0), (-col_d, 0.0), (0.0, col_d), (0.0, -col_d)] {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(x, 0.0, z),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cylinder(2.0, 0.4),
            Name::new(format!("MidColumn_{x}_{z}")),
            children![(SceneRoot(column.clone()), Transform::default())],
        ));
    }

    let cover_props: &[(&str, f32, f32, Handle<Scene>, f32, f32)] = &[
        ("Crates_NE", 8.0, -8.0, crates.clone(), 0.6, 0.6),
        ("Crates_SW", -8.0, 8.0, crates, 0.6, 0.6),
        ("Rubble_N", 0.0, -14.0, rubble.clone(), 1.0, 0.4),
        ("Rubble_S", 0.0, 14.0, rubble.clone(), 1.0, 0.4),
        ("Rubble_E", 14.0, 0.0, rubble.clone(), 1.0, 0.4),
        ("Rubble_W", -14.0, 0.0, rubble, 1.0, 0.4),
        ("BarrelStack_NW", -10.0, -10.0, barrels_stack.clone(), 0.7, 0.7),
        ("BarrelStack_SE", 10.0, 10.0, barrels_stack, 0.7, 0.7),
        ("Table_E", 6.0, 4.0, table.clone(), 1.5, 0.6),
        ("Table_W", -6.0, -4.0, table, 1.5, 0.6),
        ("Barrel_1", 3.0, 12.0, barrel.clone(), 0.5, 0.4),
        ("Barrel_2", -3.0, -12.0, barrel.clone(), 0.5, 0.4),
        ("Barrel_3", 12.0, -3.0, barrel.clone(), 0.5, 0.4),
        ("Barrel_4", -12.0, 3.0, barrel, 0.5, 0.4),
    ];
    for (name, x, z, scene, half_w, half_h) in cover_props {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(*x, 0.0, *z),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cuboid(*half_w, *half_h, *half_w),
            Name::new(name.to_string()),
            children![(SceneRoot(scene.clone()), Transform::default())],
        ));
    }

    commands.spawn((
        ArenaMarker,
        Transform::from_xyz(16.0, 0.0, 16.0),
        Visibility::default(),
        Name::new("ChestGold_NE"),
        children![(SceneRoot(chest_gold), Transform::default())],
    ));
    commands.spawn((
        ArenaMarker,
        Transform::from_xyz(-16.0, 0.0, -16.0),
        Visibility::default(),
        Name::new("Chest_SW"),
        children![(SceneRoot(chest), Transform::default())],
    ));

    let banners: &[(&str, f32, f32, Handle<Scene>, f32)] = &[
        ("Banner_N_red", -6.0, edge - 0.3, banner_red.clone(), 0.0),
        ("Banner_N_blue", 6.0, edge - 0.3, banner_blue.clone(), 0.0),
        ("Banner_S_yellow", 0.0, -edge + 0.3, banner_yellow, std::f32::consts::PI),
        ("Banner_E_red", edge - 0.3, -6.0, banner_red, -std::f32::consts::FRAC_PI_2),
        ("Banner_W_blue", -edge + 0.3, 6.0, banner_blue, std::f32::consts::FRAC_PI_2),
    ];
    for (name, x, z, scene, yaw) in banners {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(*x, 3.0, *z).with_rotation(Quat::from_rotation_y(*yaw)),
            Visibility::default(),
            Name::new(name.to_string()),
            children![(SceneRoot(scene.clone()), Transform::default())],
        ));
    }

    let torch_positions: &[(&str, f32, f32, Handle<Scene>)] = &[
        ("Torch_NE", 18.0, -18.0, torch.clone()),
        ("Torch_NW", -18.0, -18.0, torch.clone()),
        ("Torch_SE", 18.0, 18.0, torch.clone()),
        ("Torch_SW", -18.0, 18.0, torch),
        ("TorchWall_N", -10.0, edge - 0.5, torch_wall.clone()),
        ("TorchWall_S", 10.0, -edge + 0.5, torch_wall.clone()),
        ("TorchWall_E", edge - 0.5, 10.0, torch_wall.clone()),
        ("TorchWall_W", -edge + 0.5, -10.0, torch_wall),
    ];
    for (name, x, z, scene) in torch_positions {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(*x, 0.0, *z),
            Visibility::default(),
            Name::new(name.to_string()),
            children![
                (SceneRoot(scene.clone()), Transform::default()),
                (
                    PointLight {
                        intensity: 50_000.0,
                        color: Color::srgb(1.0, 0.55, 0.2),
                        radius: 0.3,
                        range: 12.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 1.8, 0.0),
                )
            ],
        ));
    }

    commands.spawn((
        ArenaMarker,
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 22_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.35, -0.78, 0.0)),
        Name::new("ArenaSunLight"),
    ));

    // Training bots style Overwatch : tall humanoid silhouette avec head + body
    // distinct → headshot mechanic possible. Hauteur ~1.8m, Y=0 pieds au sol.
    //
    // Hierarchy :
    //   parent (TargetCube + Health 100hp, no mesh/collider)
    //   ├─ Body  (cuboid 0.7×1.3×0.4, Y=0.65, HitZone::Body)
    //   └─ Head  (sphere 0.22, Y=1.55, HitZone::Head)
    //
    // Raycast hit retourne l'entity enfant → on lit HitZone pour multiplier dmg,
    // puis on remonte via ChildOf au parent pour appliquer Health.
    let body_mesh = meshes.add(Cuboid::new(0.7, 1.3, 0.4));
    let head_mesh = meshes.add(Sphere::new(0.22).mesh().ico(3).unwrap());
    let body_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.15, 0.15),
        emissive: LinearRgba::new(0.6, 0.0, 0.0, 1.0),
        ..default()
    });
    // Head : tone légèrement différent pour lisibilité immédiate (Overwatch training bot style).
    let head_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.35, 0.20),
        emissive: LinearRgba::new(1.2, 0.3, 0.0, 1.0),
        ..default()
    });

    for &(x, z) in &[(-4.0, -7.0), (0.0, -7.0), (4.0, -7.0), (10.0, -14.0), (-14.0, 10.0)] {
        let parent = commands
            .spawn((
                ArenaMarker,
                TargetCube,
                Transform::from_xyz(x, 0.0, z),
                Visibility::default(),
                Health::new(100.0),
                Name::new(format!("TrainingBot_{x}_{z}")),
            ))
            .id();

        // Body — cuboid central. Collider half-extents = mesh / 2.
        commands.entity(parent).with_children(|p| {
            p.spawn((
                HitZone::Body,
                Mesh3d(body_mesh.clone()),
                MeshMaterial3d(body_mat.clone()),
                Transform::from_xyz(0.0, 0.65, 0.0),
                RigidBody::Fixed,
                Collider::cuboid(0.35, 0.65, 0.20),
                Name::new("Body"),
            ));
            p.spawn((
                HitZone::Head,
                Mesh3d(head_mesh.clone()),
                MeshMaterial3d(head_mat.clone()),
                Transform::from_xyz(0.0, 1.55, 0.0),
                RigidBody::Fixed,
                Collider::ball(0.22),
                Name::new("Head"),
            ));
        });
    }

    // ── Cartoon cloud clusters (orbit) ─────────────────────────────────
    let cloud_blob_mesh = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());
    let cloud_mat = materials.add(StandardMaterial {
        base_color: CLOUD_COLOR,
        emissive: CLOUD_EMISSIVE,
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
                ArenaMarker,
                CloudOrbit { angle, radius, height: cy },
                Transform::from_xyz(cx, cy, cz).with_scale(Vec3::new(scale, hscale * scale, scale)),
                Visibility::default(),
                Name::new(format!("Cloud_{preset_name}_{ci}")),
            ))
            .id();

        for (bi, &(bx, by, bz, br)) in blobs.iter().enumerate() {
            let child = commands
                .spawn((
                    Mesh3d(cloud_blob_mesh.clone()),
                    MeshMaterial3d(cloud_mat.clone()),
                    Transform::from_xyz(bx, by, bz).with_scale(Vec3::splat(br)),
                    bevy::light::NotShadowCaster,
                    Name::new(format!("Blob_{ci}_{bi}")),
                ))
                .id();
            commands.entity(parent_id).add_child(child);
        }
    }

    info!(
        "[forgia-mode-fps-arena] Arena spawned : {}×{}m, KayKit modular + 5 cubes + 18 cloud clusters",
        (ARENA_SIZE as f32 * TILE_SIZE) as i32,
        (ARENA_SIZE as f32 * TILE_SIZE) as i32
    );
}

fn spawn_wall(commands: &mut Commands, wall_scene: &Handle<Scene>, pos: Vec3, yaw: f32) {
    commands.spawn((
        ArenaMarker,
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
        Visibility::default(),
        RigidBody::Fixed,
        Collider::cuboid(TILE_SIZE / 2.0, 1.5, 0.15),
        Name::new("Wall"),
        children![(SceneRoot(wall_scene.clone()), Transform::default())],
    ));
}

/// Orbit nuages autour du centre Y axis : rotation continue, jamais wrap brusque.
fn cloud_drift_system(time: Res<Time>, mut q: Query<(&mut Transform, &mut CloudOrbit)>) {
    let dt = time.delta_secs();
    for (mut tf, mut orbit) in &mut q {
        orbit.angle += CLOUD_ORBIT_SPEED * dt;
        if orbit.angle > std::f32::consts::TAU {
            orbit.angle -= std::f32::consts::TAU;
        }
        tf.translation.x = orbit.angle.cos() * orbit.radius;
        tf.translation.z = orbit.angle.sin() * orbit.radius;
        tf.translation.y = orbit.height;
    }
}

fn cleanup_arena(mut commands: Commands, q: Query<Entity, With<ArenaMarker>>) {
    let count = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    info!("[forgia-mode-fps-arena] Arena cleaned : {count} entities despawned");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaModeFpsArenaPlugin;
    }

    #[test]
    fn arena_size_is_odd() {
        assert_eq!(ARENA_SIZE % 2, 1, "ARENA_SIZE must be odd for centered grid");
    }
}
