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
        MovementSpeedMultiplier, Player, PlayerLocomotion, PlayerMovementTuning, ViewmodelCamera,
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

/// Vitesse horizontale réelle du joueur (m/s), mesurée **en FixedUpdate** (= au
/// rythme où le mouvement se fait) pour fournir un signal PROPRE aux consommateurs
/// qui tournent en Update (ex. bob du viewmodel). Keystone 0.1a-2 slice 3 : mesurer
/// le delta de position en Update donnait un signal bruité (0 si aucun step fixe
/// cette frame, élevé sinon) → tremblement/à-coups du viewmodel. Ici c'est lisse.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct PlayerLocomotion {
    pub horizontal_speed: f32,
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
///
/// `overlay` = `(données_rgba_empilées, taille_face)` d'un cubemap image
/// (6 faces empilées verticalement, même ordre wgpu) — ex. le ciel cartoon
/// `sky_129_stacked.png`. Fondu PAR-DESSUS le gradient selon `palette.overlay_blend`
/// (0 = gradient seul, 1 = overlay seul). Le gradient de biome n'est JAMAIS
/// supprimé : à 0.6 on garde ses teintes ET on y fond les nuages (story-661bis).
pub(crate) fn generate_cartoon_skybox(
    palette: &skybox_genome::SkyPalette,
    overlay: Option<(&[u8], usize, usize)>,
) -> Image {
    let face_size = SKYBOX_FACE_SIZE as usize;
    let total_pixels = face_size * face_size * 6;
    let mut data = vec![0u8; total_pixels * 4];

    // Overlay valide seulement si son image est exactement 6 faces carrées
    // empilées et encodée RGB/RGBA. Évite de lire hors limites si un artiste
    // remplace l'asset par une image 2D ordinaire ou un format inattendu.
    let overlay = overlay.and_then(|(buf, width, height)| {
        let pixels = width.checked_mul(height)?;
        let bpp = buf.len().checked_div(pixels)?;
        (palette.overlay_blend > 0.001
            && width > 0
            && height == width.checked_mul(6)?
            && matches!(bpp, 3 | 4)
            && buf.len() == pixels.checked_mul(bpp)?)
        .then_some((buf, width, bpp))
    });
    let blend = palette.overlay_blend.clamp(0.0, 1.0);

    for face in 0..6 {
        for y in 0..face_size {
            for x in 0..face_size {
                let idx = (face * face_size * face_size + y * face_size + x) * 4;
                let base = match face {
                    2 => palette.zenith_rgb, // +Y top
                    3 => palette.ground_rgb, // -Y bottom
                    _ => {
                        // Side face : t=0 en haut (zenith), t=1 en bas (horizon).
                        let t = y as f32 / (face_size - 1) as f32;
                        lerp_rgb(palette.zenith_rgb, palette.horizon_rgb, t)
                    }
                };
                let [r, g, b] = match overlay {
                    Some((buf, ovf, bpp)) => {
                        // Échantillonne la face `face` de l'overlay (nearest) puis
                        // fond au-dessus du gradient.
                        let ox = x * ovf / face_size;
                        let oy = y * ovf / face_size;
                        let orow = face * ovf + oy;
                        let oi = (orow * ovf + ox) * bpp;
                        let ov = [buf[oi], buf[oi + 1], buf[oi + 2]];
                        lerp_rgb(base, ov, blend)
                    }
                    None => base,
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

// ─── La capsule du joueur ────────────────────────────────────────────────────
//
// 2026-08-14 — ces deux valeurs étaient un littéral dans `spawn_player`, donc
// invisibles à qui doit POSER le joueur quelque part. Le mode Expédition a placé
// le joueur à l'altitude du SOL en croyant poser ses pieds : l'origine du
// `Transform` est le CENTRE de la capsule, donc les pieds se sont retrouvés un
// mètre sous le terrain. Symptôme : `grounded: true`, 20 contacts KCC, et un
// joueur qui ne peut plus bouger — un diagnostic qui n'évoque ni le spawn ni la
// géométrie de la capsule.
//
// C'est la classe de défaut n°1 du projet : une grandeur écrite deux fois. Elles
// sont donc publiques, et `PLAYER_FOOT_OFFSET_M` porte la dérivation une seule
// fois pour tout le monde — jumelle exacte du `foot_offset_m` des bots.

/// Demi-hauteur cylindrique de la capsule (m), hors hémisphères.
pub const PLAYER_CAPSULE_HALF_HEIGHT_M: f32 = 0.7;
/// Rayon de la capsule (m).
pub const PLAYER_CAPSULE_RADIUS_M: f32 = 0.3;
/// Distance des PIEDS au centre du `Transform` (m) — donc 1,0 m ici.
///
/// Poser un joueur sur un sol d'altitude `y` demande `y + PLAYER_FOOT_OFFSET_M`,
/// jamais `y`. Confondre les deux l'enterre de la moitié de sa capsule.
pub const PLAYER_FOOT_OFFSET_M: f32 = PLAYER_CAPSULE_HALF_HEIGHT_M + PLAYER_CAPSULE_RADIUS_M;

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

/// Story-615 — FOV hipfire de la caméra FPS, en degrés **HORIZONTAUX** (convention
/// joueur/marché — CS2, Gunfire quotent l'horizontal). Source de vérité partagée
/// entre `forgia-ui-lib` (slider menu ESC → écrit ici) et `forgia-viewmodel`
/// (`apply_ads_camera_fov` lit ici, convertit en vertical Bevy selon l'aspect de
/// la projection, puis lerp vers l'ADS). Vit dans `forgia-player` car c'est la
/// dépendance commune des deux crates (zéro cycle).
///
/// ⚠️ Fix 2026-08-05 : ces degrés étaient appliqués tels quels au `fov` VERTICAL
/// de Bevy → 90 donnait 121°H réels à 16:9 (étirement des bords, nausée). La
/// conversion H→V vit dans `forgia-viewmodel::pose::horizontal_fov_to_vertical_deg`.
#[derive(Resource, Debug, Clone, Copy)]
pub struct CameraFov {
    pub hipfire_deg: f32,
}

impl Default for CameraFov {
    fn default() -> Self {
        // 90° HORIZONTAL = défaut UserSettings.fov_deg (≈58,7° vertical à 16:9).
        // Fenêtre FPS rapide recommandée ~90-110° horizontal.
        Self { hipfire_deg: 90.0 }
    }
}

pub struct ForgiaPlayerPlugin;

impl Plugin for ForgiaPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraMode>()
            .init_resource::<CameraFov>()
            .init_resource::<PlayerLocomotion>()
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
            // Caméra (mouse_look + recoil) + sync genome restent en Update (cadence
            // rendu, fluidité de visée). Lock L7 : DANS GameSet::Movement (Update).
            .add_systems(
                Update,
                (sync_player_movement_tuning, mouse_look, weapon_recoil_apply)
                    .chain()
                    .in_set(GameSet::Movement)
                    .run_if(in_state(AppMode::InGame)),
            )
            // Keystone 0.1a-2 slice 3 (story-634) — la SIM (mouvement + dash + safety)
            // passe en FixedUpdate (timestep fixe déterministe, aligné Rapier en
            // FixedUpdate). mouse_look écrit player.yaw/rotation en Update ;
            // player_movement (FixedUpdate, AVANT Update dans la frame) lit la rotation
            // de la frame précédente (~lag négligeable, pattern fixed-timestep standard).
            // `just_pressed(Jump)` lu DIRECT : leafwing 0.20 gère l'edge par step fixe.
            .add_systems(
                FixedUpdate,
                (
                    // Dash phase 1 : input AVANT player_movement (consume Jump),
                    // motion APRÈS pour écraser horizontal KCC tout en préservant
                    // le vertical_step calculé par player_movement (gravité/jump).
                    dash::dash_input_system,
                    player_movement,
                    dash::dash_motion_system,
                    dash::dash_recharge_system,
                    player_floor_safety_net,
                    // Mesure la vitesse horizontale au rythme du mouvement (signal
                    // propre pour le bob viewmodel en Update). APRÈS le move.
                    track_player_speed,
                )
                    .chain()
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
        Collider::capsule_y(PLAYER_CAPSULE_HALF_HEIGHT_M, PLAYER_CAPSULE_RADIUS_M),
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
            // Story-647 : Hdr RETIRÉ (2026-07-02 soir) après essai runtime — le
            // pipeline HDR exige que TOUTES les caméras de la fenêtre soient HDR
            // (FpsCamera + ViewmodelCamera + MenuCamera2d, sinon passes écrasées /
            // ghosting Text2d) ET recalibrer tout l'éclairage réglé sur l'écrêtage
            // LDR (6 couches chaudes → rouge saturé). Bloom différé → story de
            // calibration HDR dédiée (voir story-647 §incident). Si réactivation :
            // Hdr À LA CRÉATION uniquement (jamais post-hoc, leçon cyber_city).
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
    // Boot : pas d'overlay (image pas encore chargée + default blend 0) ; le
    // fondu s'applique au 1er regen (genome + overlay loaded).
    let handle: Handle<Image> = images.add(generate_cartoon_skybox(&palette, None));
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
    // WoW pattern : en vue 3ᵉ personne, `mouse_look` ne tourne le player QUE si
    // RMB est tenu (mouselook steer). Sans bouton tenu la souris bouge librement
    // à l'écran et le perso reste fixe. En FPS : comportement standard, curseur
    // capturé en permanence.
    //
    // La condition suit la CAMÉRA, pas un mode en particulier : `forgia-camera-
    // orbit::orbit_cursor_grab` relâche le curseur dès qu'aucun bouton n'est
    // tenu. Laisser le Hall en branche FPS ferait pivoter le personnage à chaque
    // déplacement d'un curseur pourtant libre — les deux systèmes se
    // contrediraient.
    let third_person = matches!(*game_mode.get(), GameMode::Rpg | GameMode::CastleHub);
    let rmb_held = mouse_buttons.pressed(MouseButton::Right);
    if third_person && !rmb_held {
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
    // 2026-08-04 — atouts « corps ». `Option` : hors Roguelite la Resource peut
    // ne pas exister, et le déplacement ne doit pas en dépendre.
    combat_mods: Option<Res<forgia_combat::combat_mods::PlayerCombatMods>>,
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
    let sprint_mul = if sprinting {
        tuning.sprint_multiplier
    } else {
        1.0
    };
    // 2026-08-04 — atouts « corps ». Multiplicatif avec l'ADS et le sprint : les
    // trois sont des raisons INDÉPENDANTES d'aller plus ou moins vite, et les
    // additionner les ferait s'annuler (viser en sprintant avec un atout ne doit
    // pas donner la vitesse de base). Absent hors Roguelite → ×1.0, no-op.
    let boon_mul = combat_mods.map(|m| m.move_speed_mul).unwrap_or(1.0);
    let speed = tuning.speed * speed_mul.0 * sprint_mul * boon_mul;
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

/// Mesure la vitesse horizontale réelle du joueur (m/s) en FixedUpdate, depuis le
/// delta de position par step fixe (dt constant → signal propre, sans le jitter
/// 0/élevé qu'aurait une mesure en Update). Lue par le bob du viewmodel. Keystone
/// 0.1a-2 slice 3.
fn track_player_speed(
    time: Res<Time>,
    q: Query<&Transform, With<Player>>,
    mut last: Local<Option<Vec3>>,
    mut loco: ResMut<PlayerLocomotion>,
) {
    let Ok(tf) = q.single() else {
        *last = None;
        loco.horizontal_speed = 0.0;
        return;
    };
    let p = tf.translation;
    let dt = time.delta_secs().max(1e-5);
    loco.horizontal_speed = last
        .map(|lp| {
            let d = p - lp;
            Vec2::new(d.x, d.z).length() / dt
        })
        .unwrap_or(0.0);
    *last = Some(p);
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

    #[test]
    fn cartoon_skybox_is_a_srgb_cubemap_with_expected_poles() {
        let palette = skybox_genome::SkyPalette {
            zenith_rgb: [1, 2, 3],
            horizon_rgb: [10, 20, 30],
            ground_rgb: [4, 5, 6],
            overlay_blend: 0.0,
        };
        let image = generate_cartoon_skybox(&palette, None);

        assert_eq!(image.width(), SKYBOX_FACE_SIZE);
        assert_eq!(image.height(), SKYBOX_FACE_SIZE);
        assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 6);
        assert_eq!(
            image.texture_descriptor.format,
            TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(
            image
                .texture_view_descriptor
                .as_ref()
                .and_then(|d| d.dimension),
            Some(TextureViewDimension::Cube)
        );

        let data = image
            .data
            .as_deref()
            .expect("generated skybox keeps CPU data");
        let face_bytes = (SKYBOX_FACE_SIZE * SKYBOX_FACE_SIZE * 4) as usize;
        assert_eq!(&data[0..4], &[1, 2, 3, 255]);
        assert_eq!(&data[face_bytes * 3..face_bytes * 3 + 4], &[4, 5, 6, 255]);
    }

    #[test]
    fn malformed_overlay_is_ignored_without_corrupting_the_gradient() {
        let palette = skybox_genome::SkyPalette {
            overlay_blend: 1.0,
            ..default()
        };
        // Une image carrée n'est pas un cubemap verticalement empilé.
        let malformed = vec![255; 4 * 4 * 4];
        let image = generate_cartoon_skybox(&palette, Some((&malformed, 4, 4)));
        let data = image
            .data
            .as_deref()
            .expect("generated skybox keeps CPU data");
        assert_eq!(&data[0..3], &palette.zenith_rgb);
    }
}
