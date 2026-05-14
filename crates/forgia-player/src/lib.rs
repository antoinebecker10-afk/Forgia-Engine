//! # forgia-player
//!
//! Player controller : KinematicCharacterController rapier + FpsCamera 1P + spawn/respawn.

use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_core::prelude::*;
use forgia_input::{default_input_map, prelude::*};
use leafwing_input_manager::prelude::*;

pub mod prelude {
    pub use crate::{CameraMode, FpsCamera, ForgiaPlayerPlugin, Player};
}

/// Player marker — entité joueur principale.
#[derive(Component)]
pub struct Player {
    pub yaw: f32,
    pub pitch: f32,
    pub vertical_velocity: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            vertical_velocity: 0.0,
        }
    }
}

/// FpsCamera marker — caméra 1P enfant du Player.
#[derive(Component)]
pub struct FpsCamera;

#[derive(Resource, Default)]
pub struct CameraMode {
    pub is_third_person: bool,
}

pub struct ForgiaPlayerPlugin;

impl Plugin for ForgiaPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraMode>()
            .add_systems(OnEnter(AppMode::InGame), spawn_player)
            .add_systems(OnExit(AppMode::InGame), despawn_player)
            .add_systems(
                Update,
                (mouse_look, player_movement)
                    .chain()
                    .run_if(in_state(AppMode::InGame)),
            );
    }
}

fn spawn_player(mut commands: Commands) {
    let map = default_input_map();
    // Spawn y=2 (vs y=5) : limite vélocité d'impact pour éviter tunneling.
    commands.spawn((
        Player::default(),
        Transform::from_xyz(0.0, 2.0, 0.0),
        Visibility::default(),
        RigidBody::KinematicPositionBased,
        Collider::capsule_y(0.7, 0.3),
        KinematicCharacterController {
            up: Vec3::Y,
            offset: CharacterLength::Absolute(0.01),
            slide: true,
            autostep: Some(CharacterAutostep {
                max_height: CharacterLength::Absolute(0.3),
                min_width: CharacterLength::Absolute(0.05),
                include_dynamic_bodies: false,
            }),
            max_slope_climb_angle: 50f32.to_radians(),
            min_slope_slide_angle: 30f32.to_radians(),
            apply_impulse_to_dynamic_bodies: true,
            snap_to_ground: Some(CharacterLength::Absolute(0.5)),
            ..default()
        },
        ActionState::<PlayerAction>::default(),
        map,
        Name::new("Player"),
        children![(
            FpsCamera,
            Camera3d::default(),
            Transform::from_xyz(0.0, 0.7, 0.0),
        )],
    ));
    info!("[forgia-player] Player spawned at (0, 2, 0)");
}

fn despawn_player(mut commands: Commands, q: Query<Entity, With<Player>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
    info!("[forgia-player] Player despawned");
}

fn mouse_look(
    mut motion: MessageReader<MouseMotion>,
    mut q_player: Query<(&mut Transform, &mut Player), Without<FpsCamera>>,
    mut q_cam: Query<&mut Transform, With<FpsCamera>>,
) {
    let Ok((mut player_tf, mut player)) = q_player.single_mut() else {
        return;
    };
    let sensitivity = 0.002;
    let mut delta = Vec2::ZERO;
    for ev in motion.read() {
        delta += ev.delta;
    }
    if delta != Vec2::ZERO {
        player.yaw -= delta.x * sensitivity;
        player.pitch = (player.pitch - delta.y * sensitivity).clamp(-1.5, 1.5);
        player_tf.rotation = Quat::from_rotation_y(player.yaw);
        if let Ok(mut cam_tf) = q_cam.single_mut() {
            cam_tf.rotation = Quat::from_rotation_x(player.pitch);
        }
    }
}

fn player_movement(
    time: Res<Time>,
    mut q: Query<(
        &mut KinematicCharacterController,
        Option<&KinematicCharacterControllerOutput>,
        &mut Player,
        &ActionState<PlayerAction>,
        &Transform,
    )>,
) {
    let Ok((mut kcc, output, mut player, action, tf)) = q.single_mut() else {
        return;
    };
    let speed = 5.0;
    let jump_velocity = 6.5;
    let gravity = 18.0;
    let max_fall_speed = 30.0;
    let dt = time.delta_secs();

    // ── Movement horizontal (relatif au yaw du player) ────────────────
    let mut wishdir = Vec3::ZERO;
    if action.pressed(&PlayerAction::MoveForward) {
        wishdir += tf.forward().as_vec3();
    }
    if action.pressed(&PlayerAction::MoveBackward) {
        wishdir -= tf.forward().as_vec3();
    }
    if action.pressed(&PlayerAction::MoveLeft) {
        wishdir -= tf.right().as_vec3();
    }
    if action.pressed(&PlayerAction::MoveRight) {
        wishdir += tf.right().as_vec3();
    }
    wishdir.y = 0.0;
    let horizontal = wishdir.normalize_or_zero() * speed;

    // ── Vertical : gravité + jump + reset à grounded ──────────────────
    let grounded = output.map(|o| o.grounded).unwrap_or(false);
    if grounded && player.vertical_velocity < 0.0 {
        player.vertical_velocity = 0.0;
    }
    if grounded && action.just_pressed(&PlayerAction::Jump) {
        player.vertical_velocity = jump_velocity;
    }
    player.vertical_velocity -= gravity * dt;
    player.vertical_velocity = player.vertical_velocity.max(-max_fall_speed);

    let move_vec = Vec3::new(
        horizontal.x * dt,
        player.vertical_velocity * dt,
        horizontal.z * dt,
    );

    kcc.translation = Some(move_vec);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_mode_default_first_person() {
        let cm = CameraMode::default();
        assert!(!cm.is_third_person, "Default = first person (FPS)");
    }

    #[test]
    fn player_default_zero() {
        let p = Player::default();
        assert_eq!(p.yaw, 0.0);
        assert_eq!(p.pitch, 0.0);
    }
}
