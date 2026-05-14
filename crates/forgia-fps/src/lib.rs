//! # forgia-fps
//!
//! Mode FPS Arena — assets KayKit Dungeon Pack.
//!
//! Pattern V1 :
//! - KayKit walls **`WALL_Y = 0.0`** (LOCK absolu : pivot mesh au sol, pas centre)
//! - `TILE_SIZE = 4.0` (KayKit dungeon convention)
//! - Forgia scaled scene pattern : parent scale=1 + child SceneRoot scale (rapier3d 0.33 quirk)

use bevy::input::mouse::MouseButtonInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_combat::prelude::*;
use forgia_core::prelude::*;
use forgia_player::prelude::*;

pub mod prelude {
    pub use crate::ForgiaFpsPlugin;
}

const TILE_SIZE: f32 = 4.0;
const ARENA_SIZE: i32 = 5; // 5×5 tiles = 20×20m

#[derive(Component)]
pub struct ArenaMarker;

/// Marker pour les cubes-cibles (testables via fire_weapon_minimal).
#[derive(Component)]
pub struct TargetCube;

pub struct ForgiaFpsPlugin;

impl Plugin for ForgiaFpsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameMode::Fps), spawn_arena)
            .add_systems(OnExit(GameMode::Fps), cleanup_arena)
            .add_systems(
                Update,
                fire_weapon_minimal
                    .in_set(GameSet::Combat)
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
    let wall: Handle<Scene> = asset_server.load("models/kaykit/dungeon/wall.glb#Scene0");
    let column: Handle<Scene> = asset_server.load("models/kaykit/dungeon/column.glb#Scene0");
    let barrel: Handle<Scene> = asset_server.load("models/kaykit/dungeon/barrel.glb#Scene0");

    let half = ARENA_SIZE / 2;

    // ── Sol : grille de tiles ────────────────────────────────────────────
    for x in -half..=half {
        for z in -half..=half {
            let pos = Vec3::new(x as f32 * TILE_SIZE, 0.0, z as f32 * TILE_SIZE);
            commands.spawn((
                ArenaMarker,
                Transform::from_translation(pos),
                Visibility::default(),
                Name::new(format!("FloorTile_{x}_{z}")),
                children![(
                    SceneRoot(floor.clone()),
                    Transform::default(),
                )],
            ));
        }
    }

    // ── Sol invisible Collider (1 seul, épais pour anti-tunneling) ──────
    // Épaisseur 1m (vs 0.05m initial) : protection contre tunneling à haute vitesse.
    // Spawn player à y=2 maintenant (vs y=5) pour limiter vélocité d'impact.
    let arena_extent = (ARENA_SIZE as f32 * TILE_SIZE) / 2.0;
    commands.spawn((
        ArenaMarker,
        Transform::from_xyz(0.0, -0.5, 0.0),
        RigidBody::Fixed,
        Collider::cuboid(arena_extent + 5.0, 0.5, arena_extent + 5.0),
        Name::new("ArenaGroundCollider"),
    ));

    // ── Murs périphérie ──────────────────────────────────────────────────
    let edge = (half as f32 + 0.5) * TILE_SIZE;
    for i in -half..=half {
        let offset = i as f32 * TILE_SIZE;
        // Mur Nord (z = +edge), face vers -Z
        spawn_wall(&mut commands, &wall, Vec3::new(offset, 0.0, edge), 0.0);
        // Mur Sud (z = -edge), face vers +Z
        spawn_wall(&mut commands, &wall, Vec3::new(offset, 0.0, -edge), std::f32::consts::PI);
        // Mur Est (x = +edge), face vers -X
        spawn_wall(&mut commands, &wall, Vec3::new(edge, 0.0, offset), -std::f32::consts::FRAC_PI_2);
        // Mur Ouest (x = -edge), face vers +X
        spawn_wall(&mut commands, &wall, Vec3::new(-edge, 0.0, offset), std::f32::consts::FRAC_PI_2);
    }

    // ── 4 Colonnes intérieures (repères) ─────────────────────────────────
    let col_d = TILE_SIZE * 1.5;
    for &(x, z) in &[(col_d, col_d), (-col_d, col_d), (col_d, -col_d), (-col_d, -col_d)] {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(x, 0.0, z),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cylinder(2.0, 0.5),
            Name::new(format!("Column_{x}_{z}")),
            children![(
                SceneRoot(column.clone()),
                Transform::default(),
            )],
        ));
    }

    // ── 3 Barrels — repères mouvement ────────────────────────────────────
    for &(x, z) in &[(2.0, 0.0), (-2.0, 5.0), (5.0, -5.0)] {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(x, 0.0, z),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cylinder(0.5, 0.4),
            Name::new(format!("Barrel_{x}_{z}")),
            children![(
                SceneRoot(barrel.clone()),
                Transform::default(),
            )],
        ));
    }

    // ── Lumière ──────────────────────────────────────────────────────────
    commands.spawn((
        ArenaMarker,
        DirectionalLight {
            illuminance: 12_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(15.0, 30.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("ArenaSunLight"),
    ));

    // ── 3 Cubes target rouges (testables au tir) ────────────────────────
    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let cube_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.15, 0.15),
        emissive: LinearRgba::new(0.5, 0.0, 0.0, 1.0),
        ..default()
    });
    // Position cubes : ligne de 3 devant le spawn (face -Z), DANS l'arène (z > -10)
    for &(x, z) in &[(-4.0, -7.0), (0.0, -7.0), (4.0, -7.0)] {
        commands.spawn((
            ArenaMarker,
            TargetCube,
            Mesh3d(cube_mesh.clone()),
            MeshMaterial3d(cube_mat.clone()),
            Transform::from_xyz(x, 1.0, z),
            RigidBody::Fixed,
            Collider::cuboid(0.5, 0.5, 0.5),
            Health::new(100.0),
            Name::new(format!("TargetCube_{x}_{z}")),
        ));
    }

    info!("[forgia-fps] Arena spawned : 25 floor tiles + 20 walls + 4 columns + 3 barrels + 3 target cubes");
}

fn spawn_wall(
    commands: &mut Commands,
    wall_scene: &Handle<Scene>,
    pos: Vec3,
    yaw: f32,
) {
    commands.spawn((
        ArenaMarker,
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
        Visibility::default(),
        RigidBody::Fixed,
        // Collider mur : ~4m large × 3m haut × 0.3m épaisseur
        Collider::cuboid(TILE_SIZE / 2.0, 1.5, 0.15),
        Name::new("Wall"),
        children![(
            SceneRoot(wall_scene.clone()),
            Transform::default(),
        )],
    ));
}

/// Tire un raycast depuis la FpsCamera quand le joueur clique gauche.
/// Si touche un TargetCube → applique damage + flash blanc + emit CombatHitEvent.
/// Phase 2.1 minimum (sans tracer/muzzle, sans cooldown réel).
fn fire_weapon_minimal(
    // MouseButtonInput events (NOT consumed by egui, contrairement à ButtonInput Resource)
    mut mouse_evs: MessageReader<MouseButtonInput>,
    rapier: ReadRapierContext,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_player: Query<Entity, With<Player>>,
    // Fusionné en 1 query mut pour éviter B0001 (anti-trap V1 CLAUDE.md §6)
    mut q_target: Query<(&MeshMaterial3d<StandardMaterial>, &mut Health), With<TargetCube>>,
    mut commands: Commands,
    flash_cache: Res<HitFlashCache>,
    mut hit_events: MessageWriter<CombatHitEvent>,
) {
    let mut left_pressed = false;
    for ev in mouse_evs.read() {
        if ev.button == MouseButton::Left && ev.state == ButtonState::Pressed {
            left_pressed = true;
        }
    }
    if !left_pressed {
        return;
    }
    let Ok(cam_tf) = q_cam.single() else {
        warn!("[fire] FpsCamera not found");
        return;
    };
    let Ok(ctx) = rapier.single() else {
        warn!("[fire] RapierContext not found");
        return;
    };

    let origin = cam_tf.translation();
    let direction = cam_tf.forward().as_vec3();

    // Exclure le Player du raycast (sinon hit immédiat le collider capsule du player
    // car FpsCamera est enfant de Player → origin ray DANS le collider, toi=0).
    let player_entity = q_player.single().ok();
    let predicate = |e: Entity| Some(e) != player_entity;
    let filter = QueryFilter::default().predicate(&predicate);
    if let Some((entity, toi)) = ctx.cast_ray(origin, direction, 100.0, true, filter) {
        // Cherche dans target query mut (1 seule query)
        if let Ok((mat, mut hp)) = q_target.get_mut(entity) {
            let damage = 25.0;
            hp.current = (hp.current - damage).max(0.0);
            let dead = hp.is_dead();
            let new_hp = hp.current;
            let mat_handle = mat.0.clone();

            // Swap material vers flash blanc + insert HitFlashTimer
            commands
                .entity(entity)
                .insert(MeshMaterial3d(flash_cache.flash_material.clone()))
                .insert(HitFlashTimer {
                    timer: Timer::from_seconds(0.15, TimerMode::Once),
                    original_emissive: LinearRgba::new(0.0, 0.0, 0.0, 1.0),
                    original_handle: Some(mat_handle),
                });

            hit_events.write(CombatHitEvent {
                target: entity,
                damage,
                is_kill: dead,
            });

            info!(
                "[fire] HIT cube {:?} toi={:.2} dmg={} hp={}/100 dead={}",
                entity, toi, damage, new_hp, dead
            );
        } else {
            info!("[fire] hit non-target entity {:?} toi={:.2}", entity, toi);
        }
    } else {
        info!("[fire] miss");
    }
}

fn cleanup_arena(mut commands: Commands, q: Query<Entity, With<ArenaMarker>>) {
    let count = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    info!("[forgia-fps] Arena cleaned : {count} entities despawned");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaFpsPlugin;
    }

    #[test]
    fn arena_size_is_odd() {
        // ARENA_SIZE doit être impair pour avoir un centre exact (0,0)
        assert_eq!(ARENA_SIZE % 2, 1, "ARENA_SIZE must be odd for centered grid");
    }
}
