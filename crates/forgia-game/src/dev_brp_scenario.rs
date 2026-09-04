//! Commandes BRP bornées pour les scénarios agentiques du vrai runtime.
//!
//! Ce module n'est compilé qu'avec `dev-brp`. Il ne permet pas d'exécuter du
//! code arbitraire ni de muter directement l'ECS : le client demande une action
//! joueur connue pendant un nombre de frames borné, puis relit une photographie
//! métier du monde.
//!
//! # L'entrée passe par le moteur, jamais par la ressource
//!
//! 🚨 Écrire dans `ButtonInput<KeyCode>` depuis `First` produit un `just_pressed`
//! que `keyboard_input_system` EFFACE en `PreUpdate` (il appelle `clear()` avant
//! de lire ses messages). Un tel appui reste invisible pour les **62 lecteurs**
//! `just_pressed(KeyCode::…)` du workspace (recharge, console, éditeur) : ils
//! rendraient « aucun effet » sur une feature saine. Le harnais écrit donc des
//! messages `KeyboardInput`, comme le fait winit — le moteur en dérive lui-même
//! `pressed` ET `just_pressed`, et leafwing ses propres fronts.
//!
//! # Un seul écrivain
//!
//! Les pilotes (`drive_action`, `drive_expedition_path`) ne touchent aucune
//! entrée : ils DÉCLARENT ce qu'ils tiennent dans [`EntreesHarnais`], et
//! [`emettre_entrees`] traduit la déclaration en messages. Conséquences : une
//! touche relâchée est celle qui a été appuyée (une seule liste), un scénario
//! interrompu ne laisse rien de collé, et une perte de focus fenêtre — qui
//! provoque un `release_all()` côté moteur — se répare à la frame suivante.
//!
//! # Debug à la main
//!
//! `forgia.scenario.key` (n'importe quelle touche), `forgia.scenario.look`
//! (vrai chemin regard), `forgia.scenario.release_all` et
//! `forgia.scenario.snapshot` suffisent à piloter le jeu depuis un terminal :
//! `python tools/ai/brp.py --help`.

use bevy::input::{
    ButtonState,
    keyboard::{Key, KeyCode, KeyboardInput, NativeKey},
    mouse::{MouseButton, MouseButtonInput, MouseMotion},
};
use bevy::prelude::*;
use bevy::remote::{BrpError, BrpResult};
use bevy::window::PrimaryWindow;
use forgia_ai_arena_bot::ArenaBot;
use forgia_combat::Health;
use forgia_core::prelude::GameMode;
use forgia_fps::HitscanSensorState;
use forgia_input::MouseSensitivityMultiplier;
use forgia_killfeed::KillfeedSensor;
use forgia_mode_expedition::{ActiveExpedition, manifest::blender_to_bevy};
use forgia_mode_roguelite::{
    RogueliteWave, RunState,
    avatar::{AvatarClipDiag, AvatarLocomotion},
};
use forgia_player::{FpsCamera, MouseLookTuning, Player, PlayerLocomotion, Posture};
use forgia_rpg_data::loot_tables::{Pickup, Souls};
use serde::Deserialize;
use serde_json::{Value, json};

pub const ACT_METHOD: &str = "forgia.scenario.act";
pub const AIM_AT_METHOD: &str = "forgia.scenario.aim_at";
pub const STOP_METHOD: &str = "forgia.scenario.stop";
pub const FOLLOW_FIRST_CAMP_METHOD: &str = "forgia.scenario.follow_first_camp";
pub const FOLLOW_CAMP_METHOD: &str = "forgia.scenario.follow_camp";
pub const SNAPSHOT_METHOD: &str = "forgia.scenario.snapshot";
pub const KEY_METHOD: &str = "forgia.scenario.key";
pub const LOOK_METHOD: &str = "forgia.scenario.look";
pub const RELEASE_ALL_METHOD: &str = "forgia.scenario.release_all";
const MAX_ACTION_FRAMES: u16 = 600;
const MAX_PATH_FOLLOW_FRAMES: u32 = 5_000;
/// Une touche libre ne se tient pas indéfiniment sans que personne ne regarde :
/// au-delà, le harnais relâche seul plutôt que de laisser une entrée collée.
const MAX_KEY_FRAMES: u16 = 3_600;
/// Un mouvement de souris humain, borné par frame (pixels). Sans borne, une
/// correction de cap de 180° partirait en une frame et ferait un `snap`.
const MAX_REGARD_PX_PAR_FRAME: f32 = 40.0;
/// Le harnais avance quand il regarde à peu près dans la bonne direction ; en
/// dessous il tourne sur place, comme un joueur qui se réoriente d'abord.
const CAP_TOLERANCE_MARCHE_RAD: f32 = 0.60;
/// En deçà, le cap est considéré tenu : inutile de bouger la souris.
const CAP_TOLERANCE_RAD: f32 = 0.02;
/// Si le cap ne se corrige pas malgré la souris, le regard est bloqué quelque
/// part (curseur libre, `InputBlockers`, sensibilité nulle) : le dire, plutôt
/// que d'épuiser le budget de frames en silence.
const FRAMES_HORS_CAP_MAX: u32 = 240;
/// Marche tenue sans gagner un centimètre vers le waypoint : quelque chose
/// bloque. À 60 fps et 6,5 m/s, une demi-seconde immobile n'a aucune cause
/// légitime sur un chemin autoré.
const FRAMES_SANS_AVANCEE_MAX: u32 = 30;
/// Gain minimal compté comme une avancée (m). Sous ce seuil, c'est du bruit de
/// pas fixe, pas du déplacement.
const AVANCEE_MIN_M: f32 = 0.02;
/// On garde les premiers blocages, pas tous : un rapport n'a pas à porter mille
/// fois le même mur pour dire qu'il existe.
const MAX_BLOQUAGES_RETENUS: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScenarioAction {
    MoveForward,
    SprintForward,
    MoveBackward,
    StrafeLeft,
    StrafeRight,
    Jump,
    Crouch,
    Slide,
    SprintJump,
    DiagonalForwardRight,
    CrouchForward,
    CrouchBackward,
    TurnRight,
    SprintTurnRight,
    Fire,
}

impl ScenarioAction {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "move_forward" => Some(Self::MoveForward),
            "sprint_forward" => Some(Self::SprintForward),
            "move_backward" => Some(Self::MoveBackward),
            "strafe_left" => Some(Self::StrafeLeft),
            "strafe_right" => Some(Self::StrafeRight),
            "jump" => Some(Self::Jump),
            "crouch" => Some(Self::Crouch),
            "slide" => Some(Self::Slide),
            "sprint_jump" => Some(Self::SprintJump),
            "diagonal_forward_right" => Some(Self::DiagonalForwardRight),
            "crouch_forward" => Some(Self::CrouchForward),
            "crouch_backward" => Some(Self::CrouchBackward),
            "turn_right" => Some(Self::TurnRight),
            "sprint_turn_right" => Some(Self::SprintTurnRight),
            "fire" => Some(Self::Fire),
            _ => None,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::MoveForward => "move_forward",
            Self::SprintForward => "sprint_forward",
            Self::MoveBackward => "move_backward",
            Self::StrafeLeft => "strafe_left",
            Self::StrafeRight => "strafe_right",
            Self::Jump => "jump",
            Self::Crouch => "crouch",
            Self::Slide => "slide",
            Self::SprintJump => "sprint_jump",
            Self::DiagonalForwardRight => "diagonal_forward_right",
            Self::CrouchForward => "crouch_forward",
            Self::CrouchBackward => "crouch_backward",
            Self::TurnRight => "turn_right",
            Self::SprintTurnRight => "sprint_turn_right",
            Self::Fire => "fire",
        }
    }

    /// Ce que l'action TIENT tant qu'elle dure. **Une seule liste** : le
    /// relâchement se dérive de la même source que l'appui. Deux listes écrites
    /// à la main finissent par diverger, et la touche oubliée d'un côté reste
    /// collée sur toutes les actions suivantes (`controle-de-la-sortie` §7).
    /// Le `match` est exhaustif sans joker : une action neuve ne compile pas
    /// tant qu'elle n'a pas déclaré ses entrées.
    const fn entrees(self) -> Entrees {
        const fn touches(touches: &'static [KeyCode]) -> Entrees {
            Entrees {
                touches,
                souris: None,
                regard_par_frame: Vec2::ZERO,
            }
        }
        match self {
            Self::MoveForward => touches(&[KeyCode::KeyW]),
            Self::SprintForward => touches(&[KeyCode::KeyW, KeyCode::ShiftLeft]),
            Self::MoveBackward => touches(&[KeyCode::KeyS]),
            Self::StrafeLeft => touches(&[KeyCode::KeyA]),
            Self::StrafeRight => touches(&[KeyCode::KeyD]),
            Self::Jump => touches(&[KeyCode::Space]),
            Self::Crouch => touches(&[KeyCode::ControlLeft]),
            Self::Slide => touches(&[KeyCode::KeyW, KeyCode::ShiftLeft, KeyCode::ControlLeft]),
            Self::SprintJump => touches(&[KeyCode::KeyW, KeyCode::ShiftLeft, KeyCode::Space]),
            Self::DiagonalForwardRight => touches(&[KeyCode::KeyW, KeyCode::KeyD]),
            Self::CrouchForward => touches(&[KeyCode::KeyW, KeyCode::ControlLeft]),
            Self::CrouchBackward => touches(&[KeyCode::KeyS, KeyCode::ControlLeft]),
            Self::TurnRight => Entrees {
                touches: &[],
                souris: None,
                regard_par_frame: Vec2::new(5.0, 0.0),
            },
            Self::SprintTurnRight => Entrees {
                touches: &[KeyCode::KeyW, KeyCode::ShiftLeft],
                souris: None,
                regard_par_frame: Vec2::new(5.0, 0.0),
            },
            Self::Fire => Entrees {
                touches: &[],
                souris: Some(MouseButton::Left),
                regard_par_frame: Vec2::ZERO,
            },
        }
    }
}

/// Ce qu'une action tient enfoncé pendant sa durée.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Entrees {
    touches: &'static [KeyCode],
    souris: Option<MouseButton>,
    /// Déplacement souris demandé à CHAQUE frame de l'action, en pixels.
    regard_par_frame: Vec2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActiveAction {
    action: ScenarioAction,
    frames_left: u16,
    started: bool,
}

#[derive(Resource, Default)]
pub struct ScenarioDriver {
    active: Option<ActiveAction>,
    completed_actions: u32,
}

#[derive(Resource, Default)]
pub struct ExpeditionPathFollower {
    waypoints: Vec<Vec3>,
    next: usize,
    camp_center: Vec3,
    camp_radius: f32,
    frames_left: u32,
    completed: bool,
    failure: Option<String>,
    /// Écart entre le cap tenu et le cap voulu, publié pour que « il n'avance
    /// pas » se distingue de « il n'est pas tourné dans le bon sens ».
    erreur_cap_rad: f32,
    /// Frames consécutives passées hors tolérance : au-delà, le regard ne
    /// répond pas et le suivi échoue en NOMMANT la cause.
    frames_hors_cap: u32,
    /// Le campement visé, tel qu'il est écrit au manifeste.
    camp_id: String,
    distance_precedente_m: f32,
    avancee_bloquee_frames: u32,
    /// Là où le joueur a poussé sans avancer, cap correct et touche tenue.
    ///
    /// C'est la trace d'un obstacle qui n'est pas là où le décor le montre : le
    /// KCC finit par glisser autour, donc l'arrivée au camp ne prouve RIEN sur
    /// le chemin parcouru. Sans ce relevé, un trajet « vert » masque un mur
    /// invisible.
    bloquages: Vec<[f32; 3]>,
}

/// LE seul écrivain d'entrées du harnais.
///
/// Les pilotes déclarent ; [`emettre_entrees`] traduit. La déclaration est
/// remise à zéro à chaque frame : ce qui n'est plus déclaré est relâché, donc un
/// scénario interrompu (`release_all`, plantage du client, action annulée) ne
/// laisse aucune touche collée.
#[derive(Resource, Default)]
pub struct EntreesHarnais {
    /// Déclaré pour la frame en cours, consommé par l'émetteur.
    demande: Demande,
    /// Ce que l'émetteur tient depuis la frame précédente.
    tenu: Demande,
    /// Touches demandées par `forgia.scenario.key`, indépendantes des scénarios.
    libres: Vec<ToucheLibre>,
    /// Reste d'un mouvement de regard étalé par `forgia.scenario.look`.
    regard_restant: Vec2,
    regard_frames_restantes: u16,
    /// Pourquoi rien n'est parti, s'il n'est rien parti. Republié au snapshot :
    /// un harnais muet doit se voir, pas se deviner.
    panne: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct Demande {
    touches: Vec<KeyCode>,
    souris: Option<MouseButton>,
    regard: Vec2,
}

#[derive(Debug, Clone, Copy)]
struct ToucheLibre {
    code: KeyCode,
    /// `None` = tenue jusqu'à `key {state:"release"}` ou `release_all`.
    frames_restantes: Option<u16>,
}

impl EntreesHarnais {
    fn tenir(&mut self, touches: &[KeyCode]) {
        for touche in touches {
            if !self.demande.touches.contains(touche) {
                self.demande.touches.push(*touche);
            }
        }
    }

    fn cliquer(&mut self, bouton: MouseButton) {
        self.demande.souris = Some(bouton);
    }

    fn regarder(&mut self, delta: Vec2) {
        self.demande.regard += delta;
    }

    /// Tout relâcher : la déclaration se vide, l'émetteur constate la
    /// disparition à la frame suivante et écrit les `Released` correspondants.
    fn tout_relacher(&mut self) {
        self.libres.clear();
        self.regard_restant = Vec2::ZERO;
        self.regard_frames_restantes = 0;
        self.demande = Demande::default();
    }

    fn touches_tenues(&self) -> &[KeyCode] {
        &self.tenu.touches
    }
}

impl ExpeditionPathFollower {
    fn active(&self) -> bool {
        !self.waypoints.is_empty() && !self.completed && self.failure.is_none()
    }
}

#[derive(Deserialize)]
struct ActParams {
    action: String,
    frames: u16,
}

#[derive(Deserialize)]
struct AimAtParams {
    entity: u64,
}

/// Oriente le joueur et sa caméra vers une cible existante, sans toucher à sa vie.
pub fn aim_at(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let params: AimAtParams = serde_json::from_value(params.unwrap_or(Value::Null))
        .map_err(|error| invalid_params(format!("params aim_at invalides: {error}")))?;
    let target_entity = Entity::from_bits(params.entity);
    let target = world
        .query_filtered::<(&Transform, &Health), With<ArenaBot>>()
        .get(world, target_entity)
        .map(|(transform, health)| (transform.translation, health.current))
        .map_err(|_| invalid_params("la cible n'est pas un ennemi vivant observable"))?;
    if target.1 <= 0.0 {
        return Err(invalid_params("la cible est deja morte"));
    }

    let camera_position = world
        .query_filtered::<&GlobalTransform, With<FpsCamera>>()
        .iter(world)
        .next()
        .map(GlobalTransform::translation)
        .ok_or_else(|| invalid_params("camera FPS absente"))?;
    let direction = (target.0 + Vec3::Y * 0.9 - camera_position).normalize_or_zero();
    if direction == Vec3::ZERO {
        return Err(invalid_params("cible confondue avec la camera"));
    }
    let yaw = (-direction.x).atan2(-direction.z);
    let pitch = direction.y.clamp(-1.0, 1.0).asin();

    let mut player_query = world.query_filtered::<(&mut Transform, &mut Player), With<Player>>();
    let Some((mut transform, mut player)) = player_query.iter_mut(world).next() else {
        return Err(invalid_params("joueur absent"));
    };
    transform.rotation = Quat::from_rotation_y(yaw);
    player.yaw = yaw;
    player.pitch = pitch;
    drop(player_query);

    let mut camera_query = world.query_filtered::<&mut Transform, With<FpsCamera>>();
    let Some(mut camera) = camera_query.iter_mut(world).next() else {
        return Err(invalid_params("camera FPS absente"));
    };
    camera.rotation = Quat::from_rotation_x(pitch);

    Ok(json!({"aimed": true, "entity": params.entity, "yaw": yaw, "pitch": pitch}))
}

pub fn act(In(params): In<Option<Value>>, mut driver: ResMut<ScenarioDriver>) -> BrpResult {
    let params: ActParams = serde_json::from_value(params.unwrap_or(Value::Null))
        .map_err(|error| invalid_params(format!("params act invalides: {error}")))?;
    let action = ScenarioAction::parse(&params.action).ok_or_else(|| {
        invalid_params(format!(
            "action inconnue '{}'; voir le vocabulaire fermé ScenarioAction",
            params.action
        ))
    })?;
    if !(1..=MAX_ACTION_FRAMES).contains(&params.frames) {
        return Err(invalid_params(format!(
            "frames doit etre compris entre 1 et {MAX_ACTION_FRAMES}"
        )));
    }
    if driver.active.is_some() {
        return Err(invalid_params(
            "une action est deja active; attendre driver.idle=true",
        ));
    }
    driver.active = Some(ActiveAction {
        action,
        frames_left: params.frames,
        started: false,
    });
    Ok(json!({"accepted": true, "action": action.as_str(), "frames": params.frames}))
}

pub fn stop(In(_params): In<Option<Value>>, mut driver: ResMut<ScenarioDriver>) -> BrpResult {
    let Some(active) = driver.active.as_mut() else {
        return Ok(json!({"stopping": false, "reason": "idle"}));
    };
    active.frames_left = 1;
    Ok(json!({"stopping": true, "action": active.action.as_str()}))
}

#[derive(Deserialize)]
struct KeyParams {
    /// Nom physique Bevy : `KeyR`, `Space`, `ShiftLeft`, `F3`, `Digit1`…
    key: KeyCode,
    /// `tap` (défaut, quelques frames), `press` (tenue jusqu'à relâchement),
    /// `release`.
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    frames: Option<u16>,
}

/// Appuie n'importe quelle touche physique — pas seulement les quinze actions
/// du vocabulaire fermé.
///
/// C'est CE qui rend le harnais utilisable en debug : recharge, console,
/// raccourcis d'éditeur, capteurs à la demande. L'appui part en message
/// `KeyboardInput`, donc les lecteurs `just_pressed(KeyCode::…)` le voient —
/// contrairement à une écriture directe dans `ButtonInput` depuis `First`.
pub fn key(In(params): In<Option<Value>>, mut entrees: ResMut<EntreesHarnais>) -> BrpResult {
    let params: KeyParams = serde_json::from_value(params.unwrap_or(Value::Null))
        .map_err(|error| invalid_params(format!("params key invalides: {error}")))?;
    let etat = params.state.as_deref().unwrap_or("tap");
    entrees.libres.retain(|libre| libre.code != params.key);
    match etat {
        "release" => Ok(json!({"key": format!("{:?}", params.key), "state": "release"})),
        "press" => {
            entrees.libres.push(ToucheLibre {
                code: params.key,
                frames_restantes: None,
            });
            Ok(json!({"key": format!("{:?}", params.key), "state": "press", "frames": null}))
        }
        "tap" => {
            let frames = params.frames.unwrap_or(6);
            if !(1..=MAX_KEY_FRAMES).contains(&frames) {
                return Err(invalid_params(format!(
                    "frames doit etre compris entre 1 et {MAX_KEY_FRAMES}"
                )));
            }
            entrees.libres.push(ToucheLibre {
                code: params.key,
                frames_restantes: Some(frames),
            });
            Ok(json!({"key": format!("{:?}", params.key), "state": "tap", "frames": frames}))
        }
        autre => Err(invalid_params(format!(
            "state inconnu '{autre}'; attendu tap|press|release"
        ))),
    }
}

#[derive(Deserialize)]
struct LookParams {
    #[serde(default)]
    yaw_deg: f32,
    #[serde(default)]
    pitch_deg: f32,
    #[serde(default)]
    frames: Option<u16>,
}

/// Tourne le regard PAR LA SOURIS, sur `frames` frames.
///
/// La conversion degrés → pixels passe par la sensibilité réellement en vigueur
/// (`MouseLookTuning` × `MouseSensitivityMultiplier`, que l'ADS écrase) : c'est
/// donc la vraie chaîne `mouse_look` qui tourne le joueur et sa caméra, pas une
/// écriture de `Transform` qui court-circuiterait ce qu'on cherche à observer.
pub fn look(
    In(params): In<Option<Value>>,
    tuning: Res<MouseLookTuning>,
    multiplicateur: Res<MouseSensitivityMultiplier>,
    mut entrees: ResMut<EntreesHarnais>,
) -> BrpResult {
    let params: LookParams = serde_json::from_value(params.unwrap_or(Value::Null))
        .map_err(|error| invalid_params(format!("params look invalides: {error}")))?;
    let frames = params.frames.unwrap_or(6).clamp(1, MAX_ACTION_FRAMES);
    let sensibilite = sensibilite_effective(&tuning, &multiplicateur);
    let pixels = pixels_pour_rotation(
        Vec2::new(
            params.yaw_deg.to_radians(),
            params.pitch_deg.to_radians(),
        ),
        sensibilite,
    )
    .ok_or_else(|| invalid_params("sensibilite souris nulle: le regard ne peut pas etre joue"))?;
    entrees.regard_restant = pixels;
    entrees.regard_frames_restantes = frames;
    Ok(json!({
        "yaw_deg": params.yaw_deg,
        "pitch_deg": params.pitch_deg,
        "frames": frames,
        "pixels": [pixels.x, pixels.y],
        "sensibilite_rad_par_px": sensibilite,
    }))
}

/// Relâche tout : action, suivi de chemin, touches libres, regard en cours.
///
/// Le filet du debug manuel — sans lui, une touche tenue par un scénario
/// interrompu piloterait le jeu jusqu'à la fermeture, et le défaut suivant
/// serait diagnostiqué sur un joueur qui court tout seul.
pub fn release_all(
    In(_params): In<Option<Value>>,
    mut entrees: ResMut<EntreesHarnais>,
    mut driver: ResMut<ScenarioDriver>,
    mut follower: ResMut<ExpeditionPathFollower>,
) -> BrpResult {
    let relachees: Vec<String> = entrees
        .touches_tenues()
        .iter()
        .map(|code| format!("{code:?}"))
        .collect();
    let action = driver.active.map(|a| a.action.as_str());
    entrees.tout_relacher();
    driver.active = None;
    follower.waypoints.clear();
    follower.frames_left = 0;
    Ok(json!({
        "released_keys": relachees,
        "cancelled_action": action,
        "path_follow_cancelled": true,
    }))
}

fn sensibilite_effective(
    tuning: &MouseLookTuning,
    multiplicateur: &MouseSensitivityMultiplier,
) -> f32 {
    tuning.base_sensitivity * multiplicateur.factor
}

/// `mouse_look` fait `yaw -= dx * s` et `pitch -= dy * s` : la conversion
/// inverse porte donc le signe négatif. Rendre `None` sur sensibilité nulle
/// plutôt que diviser par zéro — un `inf` de pixels ferait tourner la tête d'un
/// tour complet sans que personne ne comprenne pourquoi.
fn pixels_pour_rotation(rotation_rad: Vec2, sensibilite: f32) -> Option<Vec2> {
    if sensibilite.abs() <= f32::EPSILON {
        return None;
    }
    Some(-rotation_rad / sensibilite)
}

#[derive(Deserialize)]
struct FollowCampParams {
    /// `camp_1`, `camp_2`, `camp_3` — l'id tel qu'il est écrit au manifeste.
    camp: String,
}

/// Suit la polyline autorée jusqu'au campement demandé.
///
/// La commande reste fermée : elle n'accepte pas un point arbitraire, seulement
/// un campement que le manifeste déclare. Aller plus loin que le premier camp
/// n'est pas un confort — le chemin traverse les barricades des camps suivants,
/// et c'est précisément là que le décor et sa collision se contredisent.
pub fn follow_camp(
    In(params): In<Option<Value>>,
    active: Res<ActiveExpedition>,
    follower: ResMut<ExpeditionPathFollower>,
    driver: Res<ScenarioDriver>,
) -> BrpResult {
    let params: FollowCampParams = serde_json::from_value(params.unwrap_or(Value::Null))
        .map_err(|error| invalid_params(format!("params follow_camp invalides: {error}")))?;
    armer_suivi(&params.camp, &active, follower, &driver)
}

/// L'ancien nom, gardé pour les scénarios déjà écrits : `camp_1`.
pub fn follow_first_camp(
    In(_params): In<Option<Value>>,
    active: Res<ActiveExpedition>,
    follower: ResMut<ExpeditionPathFollower>,
    driver: Res<ScenarioDriver>,
) -> BrpResult {
    armer_suivi("camp_1", &active, follower, &driver)
}

fn armer_suivi(
    camp_id: &str,
    active: &ActiveExpedition,
    mut follower: ResMut<ExpeditionPathFollower>,
    driver: &ScenarioDriver,
) -> BrpResult {
    if driver.active.is_some() || follower.active() {
        return Err(invalid_params(
            "une action ou un suivi de chemin est deja actif",
        ));
    }
    let camp = active
        .gameplay
        .campements
        .iter()
        .find(|camp| camp.id == camp_id)
        .ok_or_else(|| {
            let connus: Vec<&str> = active
                .gameplay
                .campements
                .iter()
                .map(|c| c.id.as_str())
                .collect();
            invalid_params(format!(
                "campement '{camp_id}' absent du manifeste actif; connus: {connus:?}"
            ))
        })?;
    let camp_center = blender_to_bevy(camp.centre_xyz);
    let path: Vec<Vec3> = active.gameplay.chemin_bevy().collect();
    let end = path
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.distance_squared(camp_center)
                .total_cmp(&b.distance_squared(camp_center))
        })
        .map(|(index, _)| index)
        .ok_or_else(|| invalid_params("chemin expedition vide"))?;
    follower.waypoints = path.into_iter().take(end + 1).collect();
    follower.next = 1.min(follower.waypoints.len());
    follower.camp_center = camp_center;
    follower.camp_radius = camp.rayon_m;
    follower.camp_id = camp_id.to_string();
    follower.frames_left = MAX_PATH_FOLLOW_FRAMES;
    follower.completed = false;
    follower.failure = None;
    follower.erreur_cap_rad = 0.0;
    follower.frames_hors_cap = 0;
    follower.avancee_bloquee_frames = 0;
    follower.distance_precedente_m = f32::MAX;
    follower.bloquages.clear();
    Ok(json!({
        "accepted": true,
        "camp": camp_id,
        "waypoints": follower.waypoints.len(),
        "maximum_frames": MAX_PATH_FOLLOW_FRAMES,
    }))
}

pub fn snapshot(
    In(_params): In<Option<Value>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    driver: Res<ScenarioDriver>,
    player: Query<(Entity, &Transform, &Player)>,
    enemies: Query<(Entity, &Transform, &Health, Option<&Name>), With<ArenaBot>>,
    hitscan: Option<Res<HitscanSensorState>>,
    killfeed: Option<Res<KillfeedSensor>>,
    wave: Option<Res<RogueliteWave>>,
    souls: Option<Res<Souls>>,
    pickups: Query<(), With<Pickup>>,
    avatar_diag: Option<Res<AvatarClipDiag>>,
    avatar_locomotion: Option<Res<AvatarLocomotion>>,
    player_locomotion: Option<Res<PlayerLocomotion>>,
    posture: Option<Res<Posture>>,
    follower: Res<ExpeditionPathFollower>,
    entrees: Res<EntreesHarnais>,
) -> BrpResult {
    // Le cap et le tangage AVEC la position : sans eux, « il ne va pas où je lui
    // demande » ne se distingue pas de « il regarde ailleurs », et c'est le
    // regard que la souris pilote.
    let player = player.iter().next().map(|(entity, transform, joueur)| {
        let p = transform.translation;
        json!({
            "entity": entity.to_bits(),
            "position": [p.x, p.y, p.z],
            "yaw_deg": joueur.yaw.to_degrees(),
            "pitch_deg": joueur.pitch.to_degrees(),
        })
    });
    let active = driver
        .active
        .map(|a| json!({"action": a.action.as_str(), "frames_left": a.frames_left}));
    let player_position = player.as_ref().and_then(|value| {
        value["position"].as_array().map(|p| {
            Vec3::new(
                p[0].as_f64().unwrap_or_default() as f32,
                p[1].as_f64().unwrap_or_default() as f32,
                p[2].as_f64().unwrap_or_default() as f32,
            )
        })
    });
    let targets: Vec<Value> = enemies
        .iter()
        .map(|(entity, transform, health, name)| {
            let p = transform.translation;
            json!({
                "entity": entity.to_bits(),
                "name": name.map(Name::as_str),
                "position": [p.x, p.y, p.z],
                "distance_m": player_position.map(|origin| origin.distance(p)),
                "health": {"current": health.current, "max": health.max},
            })
        })
        .collect();
    let avatar_animation: Vec<Value> = avatar_diag
        .as_ref()
        .map(|diag| {
            diag.par_corps
                .iter()
                .map(|fiche| {
                    json!({
                        "model": fiche.corps,
                        "bound": fiche.lie,
                        "requested_state": fiche.etat_demande,
                        "playing_clip": fiche.clip_joue,
                        "restarts": fiche.relances,
                        "available_clips": fiche.presents.len(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({
        "game_mode": format!("{:?}", game_mode.get()),
        "run_state": run_state.as_ref().map(|s| format!("{:?}", s.get())),
        "driver": {
            "idle": driver.active.is_none(),
            "active": active,
            "completed_actions": driver.completed_actions,
        },
        "path_follower": {
            "active": follower.active(),
            "completed": follower.completed,
            "failure": follower.failure,
            "waypoint": follower.next,
            "waypoints": follower.waypoints.len(),
            "frames_left": follower.frames_left,
            "distance_to_camp_m": player_position.map(|p| distance_horizontale(p, follower.camp_center)),
            "heading_error_deg": follower.erreur_cap_rad.to_degrees(),
            "frames_off_heading": follower.frames_hors_cap,
            "camp": follower.camp_id,
            "frames_without_progress": follower.avancee_bloquee_frames,
            "blocked_at": follower.bloquages,
        },
        // Le harnais se photographie lui-même : « il ne se passe rien » doit
        // pouvoir se trancher entre « rien n'a été demandé », « la touche part
        // mais le jeu l'ignore » et « le harnais est en panne ».
        "inputs": {
            "held_keys": entrees
                .touches_tenues()
                .iter()
                .map(|code| format!("{code:?}"))
                .collect::<Vec<_>>(),
            "held_mouse": entrees.tenu.souris.map(|bouton| format!("{bouton:?}")),
            "last_look_px": [entrees.tenu.regard.x, entrees.tenu.regard.y],
            "sticky_keys": entrees
                .libres
                .iter()
                .map(|libre| json!({
                    "key": format!("{:?}", libre.code),
                    "frames_left": libre.frames_restantes,
                }))
                .collect::<Vec<_>>(),
            "look_frames_left": entrees.regard_frames_restantes,
            "failure": entrees.panne,
        },
        "player": player,
        "enemies": targets.len(),
        "targets": targets,
        "total_shots": hitscan.as_ref().map_or(0, |s| s.total_shots),
        "hits_with_damage": hitscan.as_ref().map_or(0, |s| s.hits_with_damage),
        "kills": killfeed.as_ref().map_or(0, |s| s.total_kills_session),
        "loot": {
            "pickups": pickups.iter().count(),
            "souls_current": souls.as_ref().map_or(0, |s| s.current),
            "souls_total_collected": souls.as_ref().map_or(0, |s| s.total_collected),
        },
        "wave": wave.as_ref().map(|w| json!({
            "stage": w.stage,
            "current_wave": w.current_wave,
            "bots_alive": w.bots_alive,
            "in_break": w.in_break,
        })),
        "avatar_animation": avatar_animation,
        "locomotion": avatar_locomotion.as_ref().map(|l| json!({
            "speed_mps": l.speed,
            "forward_mps": l.avant,
            "lateral_mps": l.lateral,
            "grounded": l.au_sol,
            "vertical_mps": l.vitesse_verticale,
            "crouched": l.accroupi,
            "sliding": l.glisse,
            "aiming": l.vise,
            "firing": l.tire,
        })),
        "player_horizontal_speed_mps": player_locomotion.as_ref().map(|l| l.horizontal_speed),
        "posture": posture.as_ref().map(|p| json!({"crouched": p.accroupi, "sliding": p.glisse})),
    }))
}

/// Suit le chemin autoré **avec les entrées d'un joueur** : W pour avancer, la
/// souris pour tourner.
///
/// 🚨 La version précédente écrivait `Transform.rotation` + `Player.yaw` au
/// passage de chaque waypoint. Deux conséquences : un corps
/// `KinematicPositionBased` dont on écrit la pose est marqué téléporté (ce qui
/// avait déjà coûté la translation du KCC quand l'écriture était par frame), et
/// surtout le trajet ne traversait JAMAIS `mouse_look` — un vert ne disait donc
/// rien de la chaîne regard/caméra, celle-là même dont dépend le tir. Ici le
/// harnais ne touche plus la pose : il corrige le cap comme un joueur.
pub fn drive_expedition_path(
    mut follower: ResMut<ExpeditionPathFollower>,
    mut entrees: ResMut<EntreesHarnais>,
    player: Query<(&Transform, &Player)>,
    tuning: Res<MouseLookTuning>,
    multiplicateur: Res<MouseSensitivityMultiplier>,
) {
    if !follower.active() {
        return;
    }
    let Ok((transform, player)) = player.single() else {
        follower.failure = Some("joueur absent".into());
        return;
    };
    let position = transform.translation;
    while follower.next < follower.waypoints.len()
        && distance_horizontale(position, follower.waypoints[follower.next]) < 1.25
    {
        follower.next += 1;
    }
    if distance_horizontale(position, follower.camp_center) <= follower.camp_radius
        || follower.next >= follower.waypoints.len()
    {
        follower.completed = true;
        return;
    }
    let direction = (follower.waypoints[follower.next] - position)
        .with_y(0.0)
        .normalize_or_zero();
    if direction == Vec3::ZERO {
        follower.failure = Some("waypoint confondu avec le joueur".into());
        return;
    }

    let cap_voulu = (-direction.x).atan2(-direction.z);
    let erreur = angle_signe(cap_voulu - player.yaw);
    follower.erreur_cap_rad = erreur;
    if erreur.abs() > CAP_TOLERANCE_RAD {
        let sensibilite = sensibilite_effective(&tuning, &multiplicateur);
        let Some(pixels) = pixels_pour_rotation(Vec2::new(erreur, 0.0), sensibilite) else {
            follower.failure =
                Some("sensibilite souris nulle: le cap ne peut pas se corriger".into());
            return;
        };
        entrees.regarder(Vec2::new(
            pixels
                .x
                .clamp(-MAX_REGARD_PX_PAR_FRAME, MAX_REGARD_PX_PAR_FRAME),
            0.0,
        ));
        follower.frames_hors_cap += 1;
        if follower.frames_hors_cap >= FRAMES_HORS_CAP_MAX {
            follower.failure = Some(format!(
                "le regard ne repond pas: {:.1} deg d'ecart tenus {FRAMES_HORS_CAP_MAX} frames \
                 (mouse_look bloque, curseur libre ou sensibilite nulle ?)",
                erreur.to_degrees()
            ));
            return;
        }
    } else {
        follower.frames_hors_cap = 0;
    }

    // Un joueur ne court pas de travers pendant qu'il se réoriente : tant que le
    // cap est franchement faux, on tourne sur place — sinon la marche mange le
    // chemin en diagonale et sort de la polyline autorée.
    if erreur.abs() < CAP_TOLERANCE_MARCHE_RAD {
        entrees.tenir(&[KeyCode::KeyW]);
        // Il POUSSE, cap correct. S'il ne gagne rien sur le waypoint, quelque
        // chose bloque — et le KCC finira par glisser autour, si bien que
        // l'arrivée au camp ne dira rien de ce qui s'est passé en route. On
        // relève donc l'endroit, pas seulement le fait.
        let distance = distance_horizontale(position, follower.waypoints[follower.next]);
        if distance < follower.distance_precedente_m - AVANCEE_MIN_M {
            follower.avancee_bloquee_frames = 0;
        } else {
            follower.avancee_bloquee_frames += 1;
            if follower.avancee_bloquee_frames == FRAMES_SANS_AVANCEE_MAX
                && follower.bloquages.len() < MAX_BLOQUAGES_RETENUS
            {
                follower
                    .bloquages
                    .push([position.x, position.y, position.z]);
            }
        }
        follower.distance_precedente_m = distance;
    } else {
        // Il tourne sur place : ne pas compter ça comme un blocage.
        follower.distance_precedente_m = f32::MAX;
        follower.avancee_bloquee_frames = 0;
    }
    follower.frames_left = follower.frames_left.saturating_sub(1);
    if follower.frames_left == 0 {
        follower.failure = Some(format!(
            "budget de frames epuise avant {}",
            follower.camp_id
        ));
    }
}

fn distance_horizontale(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// Ramène un écart d'angle dans `[-π, π]`. Sans ça, un cap qui passe par ±π
/// demande un demi-tour complet là où quelques degrés suffisent.
fn angle_signe(angle: f32) -> f32 {
    let deux_pi = std::f32::consts::TAU;
    let mut a = angle % deux_pi;
    if a > std::f32::consts::PI {
        a -= deux_pi;
    } else if a < -std::f32::consts::PI {
        a += deux_pi;
    }
    a
}

/// Décide ce que l'action tient — sans écrire la moindre entrée.
pub fn drive_action(mut driver: ResMut<ScenarioDriver>, mut entrees: ResMut<EntreesHarnais>) {
    let Some(mut active) = driver.active else {
        return;
    };
    let demande = active.action.entrees();
    entrees.tenir(demande.touches);
    if let Some(bouton) = demande.souris {
        entrees.cliquer(bouton);
    }
    if demande.regard_par_frame != Vec2::ZERO {
        entrees.regarder(demande.regard_par_frame);
    }
    active.started = true;
    active.frames_left -= 1;
    if active.frames_left == 0 {
        // Rien à relâcher ici : ne plus déclarer SUFFIT. L'émetteur constate la
        // disparition à la frame suivante et écrit les `Released` — c'est ce qui
        // rend impossible la touche collée par une action interrompue.
        driver.active = None;
        driver.completed_actions = driver.completed_actions.saturating_add(1);
    } else {
        driver.active = Some(active);
    }
}

/// Traduit la déclaration des pilotes en messages moteur. **Seul écrivain.**
///
/// L'appui est ré-émis à chaque frame : c'est ce que fait un clavier qui répète,
/// et c'est ce qui répare tout seul le `release_all()` que le moteur déclenche
/// à la perte de focus de la fenêtre. Sans cette répétition, un scénario lancé
/// pendant que le focus est ailleurs marcherait deux frames puis s'arrêterait
/// sans un mot.
pub fn emettre_entrees(
    mut entrees: ResMut<EntreesHarnais>,
    mut clavier: MessageWriter<KeyboardInput>,
    mut souris: MessageWriter<MouseButtonInput>,
    mut mouvement: MessageWriter<MouseMotion>,
    fenetre: Query<Entity, With<PrimaryWindow>>,
) {
    let Some(fenetre) = fenetre.iter().next() else {
        entrees.panne = Some("fenetre principale absente: aucune entree ne peut partir".into());
        return;
    };
    entrees.panne = None;

    // Touches libres (`forgia.scenario.key`) : elles survivent aux scénarios,
    // donc elles se re-déclarent elles-mêmes tant que leur budget dure.
    let mut expirees: Vec<KeyCode> = Vec::new();
    for libre in &mut entrees.libres {
        if let Some(restantes) = libre.frames_restantes.as_mut() {
            *restantes = restantes.saturating_sub(1);
            if *restantes == 0 {
                expirees.push(libre.code);
            }
        }
    }
    let libres: Vec<KeyCode> = entrees.libres.iter().map(|libre| libre.code).collect();
    entrees.tenir(&libres);
    entrees.libres.retain(|libre| !expirees.contains(&libre.code));

    // Regard étalé par `forgia.scenario.look` sur ses frames restantes.
    if entrees.regard_frames_restantes > 0 {
        let pas = entrees.regard_restant / f32::from(entrees.regard_frames_restantes);
        entrees.regard_restant -= pas;
        entrees.regard_frames_restantes -= 1;
        entrees.regarder(pas);
    }

    let demande = std::mem::take(&mut entrees.demande);
    for touche in &demande.touches {
        let repetition = entrees.tenu.touches.contains(touche);
        clavier.write(message_clavier(
            *touche,
            ButtonState::Pressed,
            repetition,
            fenetre,
        ));
    }
    for touche in &entrees.tenu.touches {
        if !demande.touches.contains(touche) {
            clavier.write(message_clavier(
                *touche,
                ButtonState::Released,
                false,
                fenetre,
            ));
        }
    }
    match (demande.souris, entrees.tenu.souris) {
        (Some(bouton), Some(tenu)) if bouton == tenu => {}
        (Some(bouton), tenu) => {
            if let Some(tenu) = tenu {
                souris.write(MouseButtonInput {
                    button: tenu,
                    state: ButtonState::Released,
                    window: fenetre,
                });
            }
            souris.write(MouseButtonInput {
                button: bouton,
                state: ButtonState::Pressed,
                window: fenetre,
            });
        }
        (None, Some(tenu)) => {
            souris.write(MouseButtonInput {
                button: tenu,
                state: ButtonState::Released,
                window: fenetre,
            });
        }
        (None, None) => {}
    }
    if demande.regard != Vec2::ZERO {
        mouvement.write(MouseMotion {
            delta: demande.regard,
        });
    }
    entrees.tenu = demande;
}

/// `keyboard_input_system` ne lit que `key_code`, `logical_key` et `state`, mais
/// le message porte les six champs : un message tronqué mentirait au jour où un
/// lecteur de texte (console de debug) se branchera dessus.
fn message_clavier(
    code: KeyCode,
    state: ButtonState,
    repetition: bool,
    fenetre: Entity,
) -> KeyboardInput {
    KeyboardInput {
        key_code: code,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state,
        text: None,
        repeat: repetition,
        window: fenetre,
    }
}

fn invalid_params(message: impl Into<String>) -> BrpError {
    BrpError {
        code: -32602,
        message: message.into(),
        data: None,
    }
}

pub fn install(app: &mut App) {
    app.init_resource::<ScenarioDriver>()
        .init_resource::<ExpeditionPathFollower>()
        .init_resource::<EntreesHarnais>()
        // `First`, et surtout PAS `.in_set(GameSet::Input)` : la chaîne GameSet
        // n'est configurée que pour `Update` et `FixedUpdate`
        // (`forgia-core/src/lib.rs`), donc l'étiquette promettait ici un ordre
        // qui n'existait pas. L'ordre qui compte est celui-ci : déclarer, puis
        // émettre — avant que `keyboard_input_system` (PreUpdate) ne lise les
        // messages de la frame.
        .add_systems(
            First,
            (drive_expedition_path, drive_action, emettre_entrees).chain(),
        );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Recensement des actions. Le garde-fou n'est PAS cette liste (une liste
    /// écrite à la main ne casse jamais) mais les deux `match` exhaustifs sans
    /// joker de `as_str` et `entrees` : une action neuve ne compile pas tant
    /// qu'elle n'a pas déclaré son nom et ses entrées.
    const TOUTES_LES_ACTIONS: [ScenarioAction; 15] = [
        ScenarioAction::MoveForward,
        ScenarioAction::SprintForward,
        ScenarioAction::MoveBackward,
        ScenarioAction::StrafeLeft,
        ScenarioAction::StrafeRight,
        ScenarioAction::Jump,
        ScenarioAction::Crouch,
        ScenarioAction::Slide,
        ScenarioAction::SprintJump,
        ScenarioAction::DiagonalForwardRight,
        ScenarioAction::CrouchForward,
        ScenarioAction::CrouchBackward,
        ScenarioAction::TurnRight,
        ScenarioAction::SprintTurnRight,
        ScenarioAction::Fire,
    ];

    #[derive(Resource, Default)]
    struct Captures {
        clavier: Vec<(KeyCode, ButtonState)>,
        souris: Vec<(MouseButton, ButtonState)>,
        regard: Vec<Vec2>,
    }

    fn capturer(
        mut captures: ResMut<Captures>,
        mut clavier: MessageReader<KeyboardInput>,
        mut souris: MessageReader<MouseButtonInput>,
        mut mouvement: MessageReader<MouseMotion>,
    ) {
        for message in clavier.read() {
            captures.clavier.push((message.key_code, message.state));
        }
        for message in souris.read() {
            captures.souris.push((message.button, message.state));
        }
        for message in mouvement.read() {
            captures.regard.push(message.delta);
        }
    }

    /// Banc minimal : les pilotes déclarent, l'émetteur écrit, la capture relit
    /// ce qui est RÉELLEMENT parti sur le bus — pas ce que le harnais croit.
    fn banc() -> App {
        let mut app = App::new();
        app.add_message::<KeyboardInput>()
            .add_message::<MouseButtonInput>()
            .add_message::<MouseMotion>()
            .init_resource::<ScenarioDriver>()
            .init_resource::<ExpeditionPathFollower>()
            .init_resource::<EntreesHarnais>()
            .init_resource::<Captures>()
            .add_systems(
                Update,
                ((drive_action, emettre_entrees).chain(), capturer).chain(),
            );
        app.world_mut().spawn(PrimaryWindow);
        app
    }

    fn captures(app: &App) -> &Captures {
        app.world().resource::<Captures>()
    }

    fn lancer(action: ScenarioAction, frames: u16) -> App {
        let mut app = banc();
        app.world_mut().resource_mut::<ScenarioDriver>().active = Some(ActiveAction {
            action,
            frames_left: frames,
            started: false,
        });
        app
    }

    #[test]
    fn action_parser_is_closed() {
        assert_eq!(
            ScenarioAction::parse("move_forward"),
            Some(ScenarioAction::MoveForward)
        );
        assert_eq!(ScenarioAction::parse("fire"), Some(ScenarioAction::Fire));
        assert_eq!(ScenarioAction::parse("slide"), Some(ScenarioAction::Slide));
        assert_eq!(ScenarioAction::parse("execute_code"), None);
    }

    #[test]
    fn chaque_action_tient_quelque_chose_et_porte_un_nom_unique() {
        let mut noms: Vec<&'static str> = Vec::new();
        for action in TOUTES_LES_ACTIONS {
            let entrees = action.entrees();
            assert!(
                !entrees.touches.is_empty()
                    || entrees.souris.is_some()
                    || entrees.regard_par_frame != Vec2::ZERO,
                "{} ne déclare aucune entrée : elle serait un no-op silencieux",
                action.as_str()
            );
            assert_eq!(
                ScenarioAction::parse(action.as_str()),
                Some(action),
                "aller-retour cassé pour {}",
                action.as_str()
            );
            assert!(
                !noms.contains(&action.as_str()),
                "nom en double: {}",
                action.as_str()
            );
            noms.push(action.as_str());
        }
    }

    #[test]
    fn l_appui_part_en_message_clavier_pas_dans_la_ressource() {
        // 🚨 C'est LE défaut que ce module corrige : un `ButtonInput::press`
        // depuis `First` voit son `just_pressed` effacé par
        // `keyboard_input_system` en `PreUpdate`, donc les 62 lecteurs
        // `just_pressed(KeyCode::…)` du workspace ne verraient jamais rien.
        let mut app = lancer(ScenarioAction::MoveForward, 1);
        app.update();
        assert_eq!(
            captures(&app).clavier,
            vec![(KeyCode::KeyW, ButtonState::Pressed)]
        );
    }

    #[test]
    fn le_relachement_derive_de_la_meme_liste_que_l_appui() {
        let mut app = lancer(ScenarioAction::Slide, 1);
        app.update();
        app.update();
        let clavier = &captures(&app).clavier;
        for touche in ScenarioAction::Slide.entrees().touches {
            assert!(
                clavier.contains(&(*touche, ButtonState::Pressed)),
                "{touche:?} jamais appuyée"
            );
            assert!(
                clavier.contains(&(*touche, ButtonState::Released)),
                "{touche:?} appuyée mais jamais relâchée"
            );
        }
    }

    #[test]
    fn une_action_interrompue_ne_laisse_aucune_touche_collee() {
        let mut app = lancer(ScenarioAction::SprintForward, 600);
        app.update();
        // Interruption brutale : plus personne ne déclare.
        app.world_mut().resource_mut::<ScenarioDriver>().active = None;
        app.world_mut()
            .resource_mut::<EntreesHarnais>()
            .tout_relacher();
        app.update();
        let clavier = &captures(&app).clavier;
        assert!(clavier.contains(&(KeyCode::KeyW, ButtonState::Released)));
        assert!(clavier.contains(&(KeyCode::ShiftLeft, ButtonState::Released)));
        assert!(
            app.world()
                .resource::<EntreesHarnais>()
                .touches_tenues()
                .is_empty()
        );
    }

    #[test]
    fn une_touche_libre_expire_seule_apres_son_budget() {
        let mut app = banc();
        app.world_mut()
            .resource_mut::<EntreesHarnais>()
            .libres
            .push(ToucheLibre {
                code: KeyCode::KeyR,
                frames_restantes: Some(1),
            });
        app.update();
        app.update();
        let clavier = &captures(&app).clavier;
        assert_eq!(clavier[0], (KeyCode::KeyR, ButtonState::Pressed));
        assert!(clavier.contains(&(KeyCode::KeyR, ButtonState::Released)));
    }

    #[test]
    fn le_tir_tient_le_bouton_puis_le_relache() {
        let mut app = lancer(ScenarioAction::Fire, 1);
        app.update();
        app.update();
        assert_eq!(
            captures(&app).souris,
            vec![
                (MouseButton::Left, ButtonState::Pressed),
                (MouseButton::Left, ButtonState::Released),
            ]
        );
    }

    #[test]
    fn le_regard_part_en_mouvement_de_souris() {
        let mut app = lancer(ScenarioAction::TurnRight, 1);
        app.update();
        assert_eq!(captures(&app).regard, vec![Vec2::new(5.0, 0.0)]);
    }

    #[test]
    fn sans_fenetre_le_harnais_publie_sa_panne_au_lieu_de_se_taire() {
        let mut app = App::new();
        app.add_message::<KeyboardInput>()
            .add_message::<MouseButtonInput>()
            .add_message::<MouseMotion>()
            .init_resource::<EntreesHarnais>()
            .add_systems(Update, emettre_entrees);
        app.update();
        assert!(
            app.world()
                .resource::<EntreesHarnais>()
                .panne
                .as_deref()
                .is_some_and(|panne| panne.contains("fenetre"))
        );
    }

    #[test]
    fn la_conversion_regard_suit_le_signe_de_mouse_look() {
        // `mouse_look` fait `yaw -= dx * s` : tourner de +0,1 rad demande donc
        // un delta NÉGATIF. Se tromper de signe ferait fuir le cap au lieu de le
        // rattraper — et le suivi de chemin tournerait en rond.
        let pixels =
            pixels_pour_rotation(Vec2::new(0.1, 0.0), 0.002).expect("sensibilite non nulle");
        assert!(pixels.x < 0.0);
        assert!((pixels.x + 50.0).abs() < 1e-3);
        assert_eq!(pixels_pour_rotation(Vec2::new(0.1, 0.0), 0.0), None);
    }

    #[test]
    fn un_ecart_de_cap_prend_le_chemin_le_plus_court() {
        let presque_un_tour = std::f32::consts::TAU - 0.05;
        assert!((angle_signe(presque_un_tour) + 0.05).abs() < 1e-4);
        assert!((angle_signe(-presque_un_tour) - 0.05).abs() < 1e-4);
    }
}
