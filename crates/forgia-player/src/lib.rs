//! # forgia-player
//!
//! Player controller : KinematicCharacterController rapier + FpsCamera 1P + spawn/respawn.

use bevy::core_pipeline::Skybox;
use bevy::image::{ImageLoaderSettings, ImageSampler};
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::render::render_resource::{TextureViewDescriptor, TextureViewDimension};
use bevy_rapier3d::prelude::*;
use forgia_ai_arena_bot::BotTarget;
use forgia_combat::prelude::*;
use forgia_core::prelude::*;
use forgia_damage::{Health as DamageHealth, Mortal};
use forgia_input::{default_input_map, prelude::*};
use leafwing_input_manager::prelude::*;

pub mod prelude {
    pub use crate::{
        CameraMode, FpsCamera, ForgiaPlayerPlugin, MouseLookTuning, MovementSpeedMultiplier, Player,
    };
}

/// Multiplicateur global sur la vitesse de déplacement player.
/// 1.0 = normal (hipfire), 0.65 = ADS (style CoD). Written par forgia-fps::ads.
#[derive(Resource)]
pub struct MovementSpeedMultiplier(pub f32);

impl Default for MovementSpeedMultiplier {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Resource Tuning hot-reload pour mouse_look + weapon_recoil_apply.
/// Push depuis fps_tuning.toml par forgia-fps. Defaults sains.
#[derive(Resource, Debug, Clone, Copy)]
pub struct MouseLookTuning {
    /// Sensibilité base avant le `MouseSensitivityMultiplier` (rad/pixel).
    pub base_sensitivity: f32,
    /// Recoil decay exponentiel /sec (recovery auto cam).
    pub recoil_decay_per_sec: f32,
}

impl Default for MouseLookTuning {
    fn default() -> Self {
        Self {
            base_sensitivity: 0.002,
            recoil_decay_per_sec: 8.0,
        }
    }
}

// ── Skybox cubemap (pattern V1 stacked PNG → reinterpret cube → attach Camera) ──
const SKYBOX_PATH: &str = "hdri/sky_129_stacked.png";
const SKYBOX_BRIGHTNESS: f32 = 1000.0; // V1 sky_skybox_brightness_day default

#[derive(Resource)]
struct SkyboxPending {
    handle: Handle<Image>,
    reinterpreted: bool,
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
            .init_resource::<MovementSpeedMultiplier>()
            .init_resource::<MouseLookTuning>()
            .add_systems(Startup, load_skybox)
            .add_systems(Update, attach_skybox_to_camera)
            .add_systems(OnEnter(AppMode::InGame), spawn_player)
            .add_systems(OnExit(AppMode::InGame), despawn_player)
            .add_systems(
                Update,
                (mouse_look, weapon_recoil_apply, player_movement, player_floor_safety_net)
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
        // Phase I : Player health + BotTarget marker pour les enemies arena.
        DamageHealth::new(100.0),
        Mortal,
        BotTarget,
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
            // Story-450 wave 5 phase 2c : étendre far plane à 2000m pour
            // couvrir LOD2_MAX_M=1500m + marge. Bevy default = 1000m
            // → LOD2 tiles 1000-1500m étaient clippées (gap horizon visible).
            Projection::from(PerspectiveProjection {
                far: 2000.0,
                near: 0.05,
                ..Default::default()
            }),
            Transform::from_xyz(0.0, 0.7, 0.0),
        )],
    ));
    info!("[forgia-player] Player spawned at (0, 2, 0)");
}

/// Startup : load skybox PNG stacked (sera reinterpreted en cube par attach_skybox_to_camera).
/// Settings linear filter pour transitions cube faces lisses (V1 default).
fn load_skybox(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Image> = asset_server.load_with_settings(
        SKYBOX_PATH,
        |s: &mut ImageLoaderSettings| {
            s.sampler = ImageSampler::linear();
        },
    );
    commands.insert_resource(SkyboxPending {
        handle,
        reinterpreted: false,
    });
}

/// Update : (1) reinterpret stacked 2D → cubemap array (6 faces) une fois loaded.
///          (2) attach Skybox Component sur FpsCamera dès qu'elle existe.
fn attach_skybox_to_camera(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    pending: Option<ResMut<SkyboxPending>>,
    // Story 2026-05-17 : query relaxée à toute Camera3d sans Skybox (couvre
    // RpgOrbitCamera RPG en plus de FpsCamera FPS). MenuCamera2d est Camera2d
    // donc exclue naturellement.
    q_cam: Query<Entity, (With<Camera3d>, Without<Skybox>)>,
) {
    let Some(mut pending) = pending else { return };

    // Phase 1 : reinterpret stacked → cube une fois
    if !pending.reinterpreted {
        let Some(image) = images.get_mut(&pending.handle) else { return };
        if let Err(e) = image.reinterpret_stacked_2d_as_array(6) {
            warn!("[forgia-player] Skybox reinterpret failed: {e}");
            commands.remove_resource::<SkyboxPending>();
            return;
        }
        image.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        });
        pending.reinterpreted = true;
        info!("[forgia-player] Skybox reinterpreted as cubemap (6 faces)");
    }

    // Phase 2 : attach sur FpsCamera (Player peut spawn plus tard)
    let mut attached = 0;
    for cam_entity in q_cam.iter() {
        commands.entity(cam_entity).insert(Skybox {
            image: pending.handle.clone(),
            brightness: SKYBOX_BRIGHTNESS,
            rotation: Quat::IDENTITY,
        });
        attached += 1;
    }
    if attached > 0 {
        info!("[forgia-player] Skybox attached to {attached} Camera3d(s)");
        // Resource conservée : nouvelles cameras (FPS reload / RPG OrbitCamera) re-attachées.
    }
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
    sens_mul: Res<MouseSensitivityMultiplier>,
    tuning: Res<MouseLookTuning>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    game_mode: Res<State<GameMode>>,
) {
    let Ok((mut player_tf, mut player)) = q_player.single_mut() else {
        return;
    };
    // WoW pattern (RPG) : mouse_look ne tourne le player QUE si RMB est tenu
    // (mouselook steer). Sans bouton tenu, la souris bouge librement à l'écran,
    // le perso reste fixe — comme dans WoW. En FPS mode : comportement standard
    // toujours actif (cursor locked + cam orientée par mouse motion).
    let is_rpg = *game_mode.get() == GameMode::Rpg;
    let rmb_held = mouse_buttons.pressed(MouseButton::Right);
    if is_rpg && !rmb_held {
        // Drain le buffer MouseMotion pour éviter qu'un mouvement accumulé pendant
        // la phase mouse-libre ne snape le perso au moment où l'user re-press RMB.
        for _ in motion.read() {}
        return;
    }
    // Sensibilité base × multiplier global (ADS l'écrase à <1.0 via forgia-fps).
    let sensitivity = tuning.base_sensitivity * sens_mul.factor;
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

/// Camera recoil system — pattern Apex/COD :
/// - Lit `WeaponRecoilImpulse` events (émis par fire_weapon)
/// - Push pitch ↑ + yaw jitter sur Player.pitch/yaw
/// - Decay exponentielle de `WeaponRecoilDebt` → auto-recenter caméra si pas d'input
fn weapon_recoil_apply(
    time: Res<Time>,
    mut impulses: MessageReader<WeaponRecoilImpulse>,
    mut debt: Option<ResMut<WeaponRecoilDebt>>,
    mut q_player: Query<(&mut Transform, &mut Player), Without<FpsCamera>>,
    mut q_cam: Query<&mut Transform, With<FpsCamera>>,
    mut commands: Commands,
    tuning: Res<MouseLookTuning>,
) {
    let Ok((mut player_tf, mut player)) = q_player.single_mut() else {
        return;
    };

    // Init Resource on first run if missing
    if debt.is_none() {
        commands.insert_resource(WeaponRecoilDebt::default());
    }
    let Some(ref mut debt) = debt else {
        return;
    };

    // Apply impulses (push pitch UP + yaw jitter, accumulate dans debt aussi)
    for ev in impulses.read() {
        player.pitch = (player.pitch + ev.pitch_rad).clamp(-1.5, 1.5);
        player.yaw -= ev.yaw_rad;
        debt.pitch_rad += ev.pitch_rad;
        debt.yaw_rad += ev.yaw_rad;
    }

    // Decay exponentielle depuis Tuning (default 8/s ≈ 125ms recovery).
    let decay = tuning.recoil_decay_per_sec * time.delta_secs();
    let pitch_recover = (debt.pitch_rad * decay).min(debt.pitch_rad);
    let yaw_recover = (debt.yaw_rad * decay)
        .abs()
        .min(debt.yaw_rad.abs())
        * debt.yaw_rad.signum();

    player.pitch = (player.pitch - pitch_recover).clamp(-1.5, 1.5);
    player.yaw += yaw_recover;
    debt.pitch_rad -= pitch_recover;
    debt.yaw_rad -= yaw_recover;

    // Apply to transforms
    player_tf.rotation = Quat::from_rotation_y(player.yaw);
    if let Ok(mut cam_tf) = q_cam.single_mut() {
        cam_tf.rotation = Quat::from_rotation_x(player.pitch);
    }
}

/// Keyboard turn speed when in RPG mode + RMB not held (WoW Q/D pattern).
/// Rad per second — calibrated for snappy but not jarring turn. Future :
/// migrate to genome tuning (story-447).
const RPG_KEYBOARD_TURN_RAD_PER_SEC: f32 = 2.5;
/// Mouse X → player yaw sensitivity when RMB held in RPG (mouselook steer).
/// Rad per pixel. Calibrated against forgia-camera-orbit `yaw_sensitivity`.
const RPG_RMB_STEER_RAD_PER_PX: f32 = 0.005;

fn player_movement(
    time: Res<Time>,
    speed_mul: Res<MovementSpeedMultiplier>,
    game_mode: Res<State<GameMode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut q: Query<(
        &mut KinematicCharacterController,
        Option<&KinematicCharacterControllerOutput>,
        &mut Player,
        &ActionState<PlayerAction>,
        &mut Transform,
    )>,
) {
    let Ok((mut kcc, output, mut player, action, mut tf)) = q.single_mut() else {
        return;
    };
    let speed = 5.0 * speed_mul.0;
    let jump_velocity = 6.5;
    let gravity = 18.0;
    let max_fall_speed = 30.0;
    let dt = time.delta_secs();

    // ── Mode-aware input interpretation (WoW pattern for RPG) ─────────
    // RPG mode :
    //   - RMB held → mouse X steers player yaw (mouselook). Q/D = strafe (override turn).
    //   - RMB free → Q/D rotate player yaw in place (turn).
    //   - LMB+RMB both held → auto-walk forward implicit.
    // FPS mode : Q/D always strafe, mouse Y handled by mouse_look elsewhere.
    let is_rpg = *game_mode.get() == GameMode::Rpg;
    let cam_steer_held = mouse_buttons.pressed(MouseButton::Right);
    let cam_look_held = mouse_buttons.pressed(MouseButton::Left);
    let auto_walk = is_rpg && cam_steer_held && cam_look_held;

    // RPG mouselook : consume mouse X to steer player yaw.
    if is_rpg && cam_steer_held {
        let mut dx = 0.0_f32;
        for ev in mouse_motion.read() {
            dx += ev.delta.x;
        }
        if dx.abs() > f32::EPSILON {
            player.yaw -= dx * RPG_RMB_STEER_RAD_PER_PX;
            tf.rotation = Quat::from_rotation_y(player.yaw);
        }
    } else {
        // Drain mouse events even when not using them, so other systems
        // (FPS mouse_look) don't see double events from accumulated buffer.
        mouse_motion.clear();
    }

    // RPG turn keys (Q/D AZERTY = KeyA/KeyD) when no mouselook override.
    let strafe_override = is_rpg && cam_steer_held;
    if is_rpg && !strafe_override {
        let turn_speed = RPG_KEYBOARD_TURN_RAD_PER_SEC;
        if action.pressed(&PlayerAction::MoveLeft) {
            player.yaw += turn_speed * dt;
            tf.rotation = Quat::from_rotation_y(player.yaw);
        }
        if action.pressed(&PlayerAction::MoveRight) {
            player.yaw -= turn_speed * dt;
            tf.rotation = Quat::from_rotation_y(player.yaw);
        }
    }

    // ── Movement horizontal (relatif au yaw du player) ────────────────
    let mut wishdir = Vec3::ZERO;
    if action.pressed(&PlayerAction::MoveForward) || auto_walk {
        wishdir += tf.forward().as_vec3();
    }
    if action.pressed(&PlayerAction::MoveBackward) {
        wishdir -= tf.forward().as_vec3();
    }
    // Q/D strafe : seulement en FPS mode OU en RPG quand RMB est tenu.
    let strafe_active = !is_rpg || strafe_override;
    if strafe_active {
        if action.pressed(&PlayerAction::MoveLeft) {
            wishdir -= tf.right().as_vec3();
        }
        if action.pressed(&PlayerAction::MoveRight) {
            wishdir += tf.right().as_vec3();
        }
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

/// Story-453 floor safety net (2026-05-18) — si le player KinematicCharacterController
/// rate son snap_to_ground et tombe sous Y=-1.0 (largement sous le sol Y=0), on
/// teleporte à Y=2.0 sur le même XZ pour récupérer. Cosmétique-debug — vise à
/// révéler le bug sous-jacent via logs sans crasher la session.
fn player_floor_safety_net(mut q: Query<&mut Transform, With<Player>>) {
    let Ok(mut tf) = q.single_mut() else { return };
    if tf.translation.y < -1.0 {
        warn!(
            "[player-safety-net] Player sous le sol (Y={:.2}) — teleport Y=2.0",
            tf.translation.y
        );
        tf.translation.y = 2.0;
    }
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
