//! # forgia-player
//!
//! Player controller : KinematicCharacterController rapier + FpsCamera 1P + spawn/respawn.

use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::Skybox;
use bevy::image::Image;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};
use bevy_rapier3d::prelude::*;
use forgia_ai_arena_bot::BotTarget;
use forgia_combat::prelude::*;
use forgia_core::prelude::*;
use forgia_damage::{Health as DamageHealth, Mortal};
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_input::{default_input_map, prelude::*};
use leafwing_input_manager::prelude::*;
use serde::Deserialize;

pub mod dash;
pub mod skybox_genome;

pub mod prelude {
    pub use crate::dash::{DashState, DashTuning, DashUsedEvent};
    pub use crate::{
        CameraFov, CameraMode, ForgiaPlayerPlugin, FpsCamera, MouseLookTuning,
        MovementSpeedMultiplier,
        Player, PlayerMovementTuning, ViewmodelCamera,
    };
}

/// Tuning du mouvement joueur — couche definition (genome TOML hot-reloadable).
///
/// Story-594 (M2-B4, audit 2026-06-10 P1) : speed/jump/gravity étaient les seuls
/// littéraux du feel non itérables sans rebuild, dans un FPS dont la qualité EST
/// l'itération du feel. Schéma plat, défauts = miroir exact des anciennes consts
/// (zéro régression, pattern gait_genome story-579 / HitFeedbackTuning).
/// Fichier : `assets/genomes/player_movement.toml` (hot-reload via file_watcher).
#[derive(Resource, Deserialize, TypePath, Clone, Debug)]
#[serde(default)]
pub struct PlayerMovementTuning {
    /// Vitesse de déplacement horizontale (m/s), avant MovementSpeedMultiplier (ADS).
    pub speed: f32,
    /// Multiplicateur sprint (Shift tenu, hors ADS). 1.0 = sprint désactivé.
    pub sprint_multiplier: f32,
    /// Vélocité verticale au saut (m/s).
    pub jump_velocity: f32,
    /// Gravité appliquée en l'air (m/s²) — le KCC n'utilise pas la gravité Rapier.
    pub gravity: f32,
    /// Vitesse de chute max (m/s, valeur positive).
    pub max_fall_speed: f32,
    /// RPG : vitesse de rotation clavier Q/D quand RMB libre (rad/s, pattern WoW).
    pub rpg_keyboard_turn_rad_per_sec: f32,
    /// RPG : sensibilité mouse X → yaw player quand RMB tenu (rad/pixel),
    /// calibrée contre forgia-camera-orbit `yaw_sensitivity`.
    pub rpg_rmb_steer_rad_per_px: f32,
}

impl Default for PlayerMovementTuning {
    fn default() -> Self {
        // Miroir exact des littéraux pré-story-594 — NE PAS modifier sans story :
        // c'est le filet anti-régression si le TOML manque ou est invalide.
        Self {
            speed: 5.0,
            // Sprint ajouté 2026-06-11 (post-594) : pas de littéral pré-genome,
            // le défaut active le sprint même si le TOML est absent.
            sprint_multiplier: 1.5,
            jump_velocity: 6.5,
            gravity: 18.0,
            max_fall_speed: 30.0,
            rpg_keyboard_turn_rad_per_sec: 2.5,
            rpg_rmb_steer_rad_per_px: 0.005,
        }
    }
}

#[derive(Resource)]
pub struct PlayerMovementTuningHandle(pub Handle<Genome<PlayerMovementTuning>>);

fn load_player_movement_tuning(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Genome<PlayerMovementTuning>> =
        asset_server.load("genomes/player_movement.toml");
    commands.insert_resource(PlayerMovementTuningHandle(handle));
}

/// Sync genome → Resource (Default au boot, écrasée dès le chargement/hot-reload).
fn sync_player_movement_tuning(
    handle: Option<Res<PlayerMovementTuningHandle>>,
    assets: Res<Assets<Genome<PlayerMovementTuning>>>,
    mut tuning: ResMut<PlayerMovementTuning>,
) {
    let Some(g) = handle.as_deref().and_then(|h| assets.get(&h.0)) else {
        return;
    };
    *tuning = g.data.clone();
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

// ── Skybox cartoon procedural ──
// Story-554 Phase 1 : cubemap 256×256×6 généré au Startup, palette hardcodée.
// Story-555 Phase 2 : palette data-driven via `assets/genomes/biome_sky.toml`
//   + hot-reload Shift+F12 + auto-switch per-biome via StageLoadResult.
// HDR-compat (sRGB sampling auto-linear).
const SKYBOX_FACE_SIZE: u32 = 256;
pub(crate) const SKYBOX_BRIGHTNESS: f32 = 500.0;

/// Génère un cubemap procedural cartoon style depuis une [`SkyPalette`].
///
/// Order faces (wgpu convention) : +X, -X, +Y, -Y, +Z, -Z.
/// - +Y (top) : solid zenith
/// - -Y (bottom) : solid ground
/// - sides : gradient vertical zenith (haut) → horizon (bas)
pub(crate) fn generate_cartoon_skybox(palette: &skybox_genome::SkyPalette) -> Image {
    let face_size = SKYBOX_FACE_SIZE as usize;
    let total_pixels = face_size * face_size * 6;
    let mut data = vec![0u8; total_pixels * 4];

    for face in 0..6 {
        for y in 0..face_size {
            for x in 0..face_size {
                let idx = (face * face_size * face_size + y * face_size + x) * 4;
                let [r, g, b] = match face {
                    2 => palette.zenith_rgb, // +Y top
                    3 => palette.ground_rgb, // -Y bottom
                    _ => {
                        // Side face : t=0 en haut (zenith), t=1 en bas (horizon).
                        let t = y as f32 / (face_size - 1) as f32;
                        lerp_rgb(palette.zenith_rgb, palette.horizon_rgb, t)
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

/// Story-618 — caméra dédiée au rendu du viewmodel (arme + bras) avec un FOV
/// séparé (RenderLayers::layer(1)), enfant de la FpsCamera. Marquée ici (à côté
/// de FpsCamera) car forgia-viewmodel (qui la spawn) ET forgia-mode-roguelite
/// (qui l'exclut du toon) dépendent tous deux de forgia-player → zéro cycle.
/// Exclue du skybox (`attach_skybox_to_camera`) et du toon (roguelite).
#[derive(Component)]
pub struct ViewmodelCamera;

#[derive(Resource, Default)]
pub struct CameraMode {
    pub is_third_person: bool,
}

/// Story-615 — FOV hipfire de la caméra FPS, en degrés. Source de vérité partagée
/// entre `forgia-ui-lib` (slider menu ESC → écrit ici) et `forgia-viewmodel`
/// (`apply_ads_camera_fov` lit ici comme base, puis lerp vers l'ADS). Vit dans
/// `forgia-player` car c'est la dépendance commune des deux crates (zéro cycle).
#[derive(Resource, Debug, Clone, Copy)]
pub struct CameraFov {
    pub hipfire_deg: f32,
}

impl Default for CameraFov {
    fn default() -> Self {
        // 90° = défaut UserSettings.fov_deg. Plus large que l'ancien hipfire 45°
        // (≈73° H, étroit) — fenêtre FPS rapide recommandée ~90-110° horizontal.
        Self { hipfire_deg: 90.0 }
    }
}

pub struct ForgiaPlayerPlugin;

impl Plugin for ForgiaPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraMode>()
            .init_resource::<CameraFov>()
            .init_resource::<MovementSpeedMultiplier>()
            .init_resource::<MouseLookTuning>()
            .init_resource::<PlayerMovementTuning>()
            .init_asset::<Genome<PlayerMovementTuning>>()
            .register_asset_loader(GenomeLoader::<PlayerMovementTuning>::default())
            .init_resource::<dash::DashTuning>()
            .init_resource::<dash::DashTapDetector>()
            .add_message::<dash::DashUsedEvent>()
            .add_plugins(skybox_genome::SkyboxGenomePlugin)
            .add_systems(Startup, (load_skybox, load_player_movement_tuning))
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
                    sync_player_movement_tuning,
                    mouse_look,
                    weapon_recoil_apply,
                    // Dash phase 1 : input AVANT player_movement (consume Jump),
                    // motion APRÈS pour écraser horizontal KCC tout en préservant
                    // le vertical_step calculé par player_movement (gravité/jump).
                    dash::dash_input_system,
                    player_movement,
                    dash::dash_motion_system,
                    dash::dash_recharge_system,
                    player_floor_safety_net,
                )
                    .chain()
                    // Story-594 (M2-B4) : chaîne player DANS GameSet::Movement
                    // (Lock L7 — était hors set : ordre vs aim_assist/Camera non
                    // garanti par le scheduler, audit 2026-06-10 P1).
                    .in_set(GameSet::Movement)
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
        dash::DashState::new(dash::DashTuning::default().max_charges),
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

/// Startup : génère un cubemap cartoon procedural (story-554 Phase 1).
/// L'image est ajoutée à `Assets<Image>` directement (pas de chargement disk).
fn load_skybox(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // Boot avec palette default (Crypts) — sera resync par
    // skybox_genome::sync_palette_from_genome dès que le TOML est loaded.
    let palette = skybox_genome::SkyPalette::default();
    let handle: Handle<Image> = images.add(generate_cartoon_skybox(&palette));
    commands.insert_resource(SkyboxPending { handle });
    info!(
        "[forgia-player] Skybox cartoon bootstrap ({}x{}x6 RGBA8 sRGB) — default palette",
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
    // Story-618 : exclure la ViewmodelCamera — sinon elle peindrait un skybox
    // plein écran par-dessus le monde (elle ne doit rendre QUE le viewmodel).
    q_cam: Query<Entity, (With<Camera3d>, Without<Skybox>, Without<ViewmodelCamera>)>,
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
    blockers: Res<InputBlockers>,
) {
    // Story-528 follow-up — bloque rotation cam pendant Roguelite Defeat/Victory
    // (forgia-ui set block_look=true OnEnter). Sans ça, mouse_look pivote la cam
    // pendant que le user vise les boutons "Nouvelle Run" / "Retour Menu".
    if blockers.block_look {
        for _ in motion.read() {}
        return;
    }
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

fn player_movement(
    time: Res<Time>,
    tuning: Res<PlayerMovementTuning>,
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
    // Story-594 : valeurs genome (assets/genomes/player_movement.toml, hot-reload).
    // Sprint (Shift) : bloqué en ADS (speed_mul < 1.0 = visée, convention CoD).
    let sprinting = action.pressed(&PlayerAction::Sprint) && speed_mul.0 >= 1.0;
    let sprint_mul = if sprinting { tuning.sprint_multiplier } else { 1.0 };
    let speed = tuning.speed * speed_mul.0 * sprint_mul;
    let jump_velocity = tuning.jump_velocity;
    let gravity = tuning.gravity;
    let max_fall_speed = tuning.max_fall_speed;
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
            player.yaw -= dx * tuning.rpg_rmb_steer_rad_per_px;
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
        let turn_speed = tuning.rpg_keyboard_turn_rad_per_sec;
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

    /// Régression story-594 : les défauts du tuning sont le MIROIR EXACT des
    /// littéraux pré-genome — si le TOML manque/est invalide, le feel ne change pas.
    /// Toute modification ici exige une story (c'est le feel du jeu ship).
    #[test]
    fn movement_tuning_defaults_mirror_pre_genome_literals() {
        let t = PlayerMovementTuning::default();
        assert_eq!(t.speed, 5.0);
        assert_eq!(t.sprint_multiplier, 1.5); // post-594 (2026-06-11), pas de littéral pré-genome
        assert_eq!(t.jump_velocity, 6.5);
        assert_eq!(t.gravity, 18.0);
        assert_eq!(t.max_fall_speed, 30.0);
        assert_eq!(t.rpg_keyboard_turn_rad_per_sec, 2.5);
        assert_eq!(t.rpg_rmb_steer_rad_per_px, 0.005);
    }
}
