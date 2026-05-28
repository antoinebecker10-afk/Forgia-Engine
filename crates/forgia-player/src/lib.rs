//! # forgia-player
//!
//! Player controller : KinematicCharacterController rapier + FpsCamera 1P + spawn/respawn.

use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::core_pipeline::Skybox;
use bevy::image::Image;
use bevy::input::mouse::MouseMotion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use bevy_rapier3d::prelude::*;
use forgia_ai_arena_bot::BotTarget;
use forgia_combat::prelude::*;
use forgia_core::prelude::*;
use forgia_damage::{Health as DamageHealth, Mortal};
use forgia_input::{default_input_map, prelude::*};
use leafwing_input_manager::prelude::*;

pub mod prelude {
    pub use crate::{
        CameraMode, ForgiaPlayerPlugin, FpsCamera, MouseLookTuning, MovementSpeedMultiplier, Player,
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

// ── Skybox cartoon procedural (story-554 Phase 1) ──
// Cubemap 256×256×6 généré au Startup. Gradient zénith→horizon par face.
// Style cartoon Cult of the Lamb / Death's Door : palette Crypts of Anvil.
// HDR-compat (sRGB sampling auto-linear), wire pour re-add stories 549/550/551.
const SKYBOX_FACE_SIZE: u32 = 256;
const SKYBOX_BRIGHTNESS: f32 = 500.0;

/// Palette cartoon Crypts of Anvil (bible v1).
/// Phase 2 : déplacer dans `config/genomes/biome_sky.toml` per-biome.
const SKY_ZENITH_RGB: [u8; 3] = [61, 36, 102]; // #3D2466 deep violet
const SKY_HORIZON_RGB: [u8; 3] = [255, 107, 53]; // #FF6B35 warm orange
const SKY_GROUND_RGB: [u8; 3] = [42, 27, 61]; // #2A1B3D dark mauve

/// Génère un cubemap procedural cartoon style.
/// Order faces (wgpu convention) : +X, -X, +Y, -Y, +Z, -Z.
/// - +Y (top) : solid zenith
/// - -Y (bottom) : solid ground
/// - sides : gradient vertical zenith (haut) → horizon (bas)
fn generate_cartoon_skybox() -> Image {
    let face_size = SKYBOX_FACE_SIZE as usize;
    let total_pixels = face_size * face_size * 6;
    let mut data = vec![0u8; total_pixels * 4];

    for face in 0..6 {
        for y in 0..face_size {
            for x in 0..face_size {
                let idx = (face * face_size * face_size + y * face_size + x) * 4;
                let [r, g, b] = match face {
                    2 => SKY_ZENITH_RGB, // +Y top
                    3 => SKY_GROUND_RGB, // -Y bottom
                    _ => {
                        // Side face : t=0 en haut (zenith), t=1 en bas (horizon).
                        let t = y as f32 / (face_size - 1) as f32;
                        lerp_rgb(SKY_ZENITH_RGB, SKY_HORIZON_RGB, t)
                    }
                };
                data[idx] = r;
                data[idx + 1] = g;
                data[idx + 2] = b;
                data[idx + 3] = 255;
            }
        }
    }

    let mut img = Image::new(
        Extent3d {
            width: SKYBOX_FACE_SIZE,
            height: SKYBOX_FACE_SIZE,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    img.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    img
}

fn lerp_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round() as u8;
    [lerp(a[0], b[0]), lerp(a[1], b[1]), lerp(a[2], b[2])]
}

#[derive(Resource)]
struct SkyboxPending {
    handle: Handle<Image>,
}

/// Player marker — entité joueur principale.
///
/// Story-461 (Vague 3 Bevy 0.18 idioms) : `#[require(Transform, Visibility)]`
/// garantit que tout spawn de Player insère ces deux components avec leur
/// Default si non explicitement fournis. Anti-pattern bundles V0.13-V0.15.
#[derive(Component)]
#[require(Transform, Visibility)]
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
            // Story-517 fix : despawn UNIQUEMENT au retour au menu, PAS sur OnExit(InGame).
            // ESC pause = transition InGame→Paused → OnExit(InGame) tirait avant fix,
            // ce qui despawn le player et perdait position/HP/ammo au resume.
            // Memory ref : reference_player_lifecycle_pause_safe.md.
            .add_systems(OnEnter(AppMode::Menu), despawn_player)
            .add_systems(
                Update,
                (
                    mouse_look,
                    weapon_recoil_apply,
                    player_movement,
                    player_floor_safety_net,
                )
                    .chain()
                    .run_if(in_state(AppMode::InGame)),
            );
    }
}

fn spawn_player(mut commands: Commands, existing: Query<Entity, With<Player>>) {
    // Story-517 fix : idempotent guard. OnEnter(InGame) fire à chaque transition
    // INTO InGame, incluant Paused→InGame après resume. Si player déjà spawné
    // (pause case), skip pour ne pas créer un doublon.
    if !existing.is_empty() {
        return;
    }
    let map = default_input_map();
    // Spawn y=2 (vs y=5) : limite vélocité d'impact pour éviter tunneling.
    commands.spawn((
        Player::default(),
        Transform::from_xyz(0.0, 2.0, 0.0),
        // Visibility::default() supprimé — fourni par #[require(Visibility)] sur Player.
        RigidBody::KinematicPositionBased,
        Collider::capsule_y(0.7, 0.3),
        // Phase I : Player health + BotTarget marker pour les enemies arena.
        DamageHealth::new(100.0),
        Mortal,
        BotTarget,
        KinematicCharacterController {
            up: Vec3::Y,
            // Story-540 (2026-05-27) — offset 0.01 → 0.05 : safety net contre
            // penetration creep silencieuse vs trimesh GLB props (kcc_collisions
            // restait à 0 et velocity tombait à 0). 0.05 = 5 cm de marge soft
            // pour slide naturel ; ne dégrade pas le tunneling vu que les colliders
            // statiques sont plus larges que 5 cm.
            offset: CharacterLength::Absolute(0.05),
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
            // Story-549 v2 : Bloom (auto-require Hdr en 0.18) + TonyMcMapface.
            // Compat skybox cartoon procedural story-554 (RGBA8 sRGB → auto-linear).
            Bloom::NATURAL,
            Tonemapping::TonyMcMapface,
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

/// Startup : génère un cubemap cartoon procedural (story-554 Phase 1).
/// L'image est ajoutée à `Assets<Image>` directement (pas de chargement disk).
fn load_skybox(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let handle: Handle<Image> = images.add(generate_cartoon_skybox());
    commands.insert_resource(SkyboxPending { handle });
    info!(
        "[forgia-player] Skybox cartoon generated ({}x{}x6 RGBA8 sRGB)",
        SKYBOX_FACE_SIZE, SKYBOX_FACE_SIZE
    );
}

/// Update : attach Skybox Component sur FpsCamera/RpgOrbitCamera dès qu'elle existe.
/// Story-553 : KTX2 cubemap HDR natif — plus de phase reinterpret.
fn attach_skybox_to_camera(
    mut commands: Commands,
    pending: Option<Res<SkyboxPending>>,
    // Story 2026-05-17 : query relaxée à toute Camera3d sans Skybox (couvre
    // RpgOrbitCamera RPG en plus de FpsCamera FPS). MenuCamera2d est Camera2d
    // donc exclue naturellement.
    q_cam: Query<Entity, (With<Camera3d>, Without<Skybox>)>,
) {
    let Some(pending) = pending else { return };

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
        info!("[forgia-player] Skybox HDR attached to {attached} Camera3d(s)");
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
    let yaw_recover = (debt.yaw_rad * decay).abs().min(debt.yaw_rad.abs()) * debt.yaw_rad.signum();

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
    // Story-517 fix jitter à l'arrêt : ne PAS appliquer gravité tant que grounded
    // (sinon micro-fall -gravity*dt² chaque frame → KCC snap_to_ground corrige →
    // oscillation visible sub-pixel).
    if !grounded {
        player.vertical_velocity -= gravity * dt;
        player.vertical_velocity = player.vertical_velocity.max(-max_fall_speed);
    }

    // Quand grounded ET vertical_velocity ≤ 0, on annule la translation verticale
    // pour éviter le micro-déplacement qui retrigger le snap_to_ground.
    let vertical_step = if grounded && player.vertical_velocity <= 0.0 {
        0.0
    } else {
        player.vertical_velocity * dt
    };

    let move_vec = Vec3::new(horizontal.x * dt, vertical_step, horizontal.z * dt);

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
