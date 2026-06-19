//! Portail → MODE PARCOURS 3 niveaux (kit ithappy « Platformer Underworld »,
//! 2026-06-06). Un portail dans l'arène téléporte le joueur vers le niveau démo
//! COMPLET du kit (Scene 0, ~1200 pièces) chargé tel quel comme environnement
//! marchable (colliders ConvexHull générés INCRÉMENTALEMENT, ~20/frame → pas de
//! freeze), posé à un offset lointain. Le GLB contient 3 niveaux côte à côte ; on
//! pose un PAD d'arrivée à la base de chaque zone, et des portails qui chaînent
//! zone 1 → 2 → 3 → arène. Les 4 ORBES D'UPGRADE D'ARME sont répartis sur les 3
//! zones (faut toutes les traverser pour tout récupérer). Chute = retour au pad de
//! la zone courante (checkpoint). En entrant on retire `BotTarget` + la wave est
//! gelée (cf lib.rs `combat_running`). Zéro touche forgia-stage (contendu).

use crate::elements::{Element, ElementUnlocks};
use crate::kill_popup::{KillPopup, KillPopupState};
use crate::parcours_obstacles::{
    classify, phase_from_pos, AnimatedObstacle, ObstacleAnim, ObstacleStats, RotatingObstacle,
    SlidingObstacle, SwingingHammer,
};
use crate::run::RunState;
use crate::waves::RogueliteWave;
use bevy::camera::primitives::Aabb;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_egui::egui;
use bevy_rapier3d::prelude::{
    Collider, ComputedColliderShape, QueryFilter, ReadRapierContext, RigidBody,
};
use forgia_ai_arena_bot::BotTarget;
use forgia_anchor::{AnchorKind, AnchorPoint};
use forgia_core::prelude::*;
use forgia_damage::Health;
use forgia_player::Player;
use forgia_rpg_data::boons::{
    rng_next_index, roll_candidates, ActiveBoons, BoonId, BoonsCatalogue, CoffreRng,
};
use forgia_rpg_data::loot_tables::Pickup;

const ROOM_ORIGIN: Vec3 = Vec3::new(2000.0, 0.0, 2000.0);
const PORTAL_RADIUS: f32 = 2.8;

// ── Story-603 : porte du socle (boss-gated) → parcours ────────────────────────
// La porte est posée sur le DAIS EXISTANT au centre de l'arène : le module
// `melee_pit` (forgia-stage, prefab CirclePlatformSmall, ancre AnchorKind::MeleePit).
// Ce dais est placé par le seed (bouge chaque run) et n'a PAS de collider →
// `sys_reconcile_boss_gate` lit son ancre, lui ajoute un collider et y pose la porte.
/// Repli central si aucune ancre MeleePit (stage sans melee_pit).
const DAIS_FALLBACK_POS: Vec3 = Vec3::new(0.0, 0.0, -16.0);
/// Préfixe du nom de l'entité du dais (module melee_pit posé par forgia-stage).
const DAIS_MODULE_NAME_PREFIX: &str = "Module_melee_pit";
/// Hauteur cible du portail (calibrée via AABB, GLB de taille native inconnue).
const PORTAL_TARGET_HEIGHT: f32 = 7.0;
/// Décalage Y du portail au-dessus du sommet MESURÉ du dais (tunable selon pivot GLB).
const PORTAL_Y_OFFSET: f32 = 0.0;
/// Décalage de yaw du portail (la porte regarde le spawn). Axe avant du GLB =
/// +Z (la porte tournait le dos) → `PI` (180°). `±FRAC_PI_2` si jamais de profil.
const PORTAL_YAW_OFFSET: f32 = std::f32::consts::PI;
const PORTAL_CLOSED_GLB: &str = "models/environment/portal/portal_closed.glb";
const PORTAL_OPEN_GLB: &str = "models/environment/portal/portal_open.glb";
/// Positions (locales au centre-base du portail = `portal_pos`) des 4 yeux des 2
/// crânes flanquant l'arche. Tunable : si les flammes ne sont pas pile dans les
/// orbites, ajuster X (écart crânes), Y (hauteur), Z (avancée face joueur, +Z).
const SKULL_EYE_OFFSETS: [Vec3; 4] = [
    Vec3::new(-2.30, 0.50, 1.30), // crâne gauche — œil externe
    Vec3::new(-1.95, 0.50, 1.30), // crâne gauche — œil interne
    Vec3::new(1.95, 0.50, 1.30),  // crâne droit — œil interne
    Vec3::new(2.30, 0.50, 1.30),  // crâne droit — œil externe
];
const TELEPORT_COOLDOWN: f32 = 1.2;
/// Sous ce Y → chute → retour au pad de la zone courante (checkpoint).
const FALL_DEATH_Y: f32 = -0.5;
const KIT_PATH: &str = "models/environment/platformer/platformer_underworld.glb";

// ── Items ramassables du niveau (couronne / cœur / diamant / pièce / étoile) ─────
/// Rayon de ramassage walk-over des items du GLB.
const ITEM_COLLECT_RADIUS: f32 = 3.0;
/// Couronne → rétrécissement (permanent dans le parcours, pour passer la porte mi-ouverte).
const SHRINK_SCALE: f32 = 0.4;
const SHRINK_LERP: f32 = 6.0;
/// Demi-hauteur de la capsule joueur (`capsule_y(0.7, 0.3)` → 0.7 + 0.3). Sert à recaler le Y du
/// joueur quand il change de taille (garder les pieds au sol).
const PLAYER_CAPSULE_HALF: f32 = 1.0;
/// Cœur → +PV max permanent (run).
const HEART_MAX_HP: f32 = 20.0;
/// Valeurs monnaie (pièce = Or-like, étoile = Âmes-like ; ici cumulé en `MetaSouls`).
const COIN_VALUE: u32 = 10;
const STAR_VALUE: u32 = 25;

// Échelle + AABB natif du niveau démo (cf inspection GLB) pour le poser : base à
// y=0. Assez grand pour que les plateformes (~10u) soient à la taille du joueur.
const LEVEL_SCALE: f32 = 0.8;
const LVL_CENTER: Vec3 = Vec3::new(-416.0, 40.6, -295.0);
const LVL_MIN_Y: f32 = -36.8;
/// Position native (base) d'un spawn pad par zone. [0] = SPOT VALIDÉ par l'user
/// (entrée depuis l'arène, le beau chemin pavé) ; [1]/[2] = 2 autres zones du GLB.
const ZONE_SPAWN_NATIVE: [Vec3; 3] = [
    Vec3::new(-421.8, 2.2, -260.4), // bon spot (world ≈ 1995,37,2028)
    Vec3::new(-800.0, 10.0, -288.0),
    Vec3::new(0.0, 5.0, -291.0),
];
/// Fin du chemin de CHAQUE zone (dernière plateforme nord, GLB) — bien au-delà de la porte
/// mi-ouverte. Le portail de sortie de chaque zone se pose ici = vraie traversée des 3 zones.
const ZONE_END_NATIVE: [Vec3; 3] = [
    Vec3::new(-400.0, 0.0, 10.0), // zone 1
    Vec3::new(-800.0, 0.0, 10.0), // zone 2
    Vec3::new(0.0, 0.0, 10.0),    // zone 3
];
/// Checkpoint par zone, posé sur une vraie plateforme y=0 (GLB) **juste après la porte/herse** —
/// pour ne pas refaire couronne→rétrécir→porte si on tombe ensuite.
const CHECKPOINT_NATIVE: [Vec3; 3] = [
    Vec3::new(-400.0, 0.0, -170.0), // zone 1 — juste après la porte (door_001 à z=-180)
    Vec3::new(-800.0, 0.0, -10.0),  // zone 2
    Vec3::new(0.0, 0.0, -141.0),    // zone 3 — juste après la herse (gate_001 à z=-151)
];

#[derive(Clone, Copy, PartialEq)]
enum PortalKind {
    /// Arène → zone 1 (entre dans le parcours).
    Enter,
    /// Zone N → zone N+1.
    Next,
    /// Zone 3 → retour arène.
    Return,
}

#[derive(Component)]
struct Portal {
    kind: PortalKind,
    target: Vec3,
}
#[derive(Component)]
struct LootRoomMarker;
#[derive(Component)]
struct PortalSpin;
/// Story-603 — entité racine de la porte FERMÉE (GLB + collider bloquant).
#[derive(Component)]
struct ClosedPortal;
/// Story-603 — entité racine de la porte OUVERTE (GLB + `Portal{Enter}` walk-over).
#[derive(Component)]
struct OpenPortal;
/// Story-603 — posé sur un SceneRoot de portail : `sys_calibrate_portal` mesure
/// l'AABB une fois la scène chargée et applique `scale = target_h / max_dim`
/// (GLB de taille native inconnue), puis passe le relais à `NeedsPortalGround`.
#[derive(Component)]
struct NeedsPortalCalibrate {
    target_h: f32,
    /// Y monde où la BASE du portail doit reposer (= sommet du socle).
    base_world_y: f32,
}
/// Story-603 — après calibration (scale propagé), `sys_ground_portal` décale le
/// SceneRoot en Y pour que la base réelle de la géométrie repose sur `base_world_y`
/// (corrige « le portail est dans le socle » — pivot GLB non au pied).
#[derive(Component)]
struct NeedsPortalGround {
    base_world_y: f32,
}
/// Story-603 — flamme dans un œil de crâne du portail. Scintille (flicker) via
/// `sys_flicker_portal_flames` (intensité PointLight modulée + phase par œil).
#[derive(Component)]
struct PortalFlame {
    base_intensity: f32,
    phase: f32,
}
/// Balise de checkpoint le long d'une zone : la toucher met `current_pad` (respawn sur chute) sur
/// `pos`. On n'avance que vers l'avant du chemin (z croissant) → pas de régression.
#[derive(Component)]
struct Checkpoint {
    pos: Vec3,
}
/// Racine de la Scene démo + flag « pas encore marqué pour collider ».
#[derive(Component)]
struct DemoLevelRoot;
#[derive(Component)]
struct DemoUnmarked;
/// Mesh du niveau démo en attente de son collider ConvexHull (généré par lots).
#[derive(Component)]
struct NeedsLevelCollider;
// Obstacles animés (marteau/balayeur/coulissant) : composants + systèmes dans
// `parcours_obstacles.rs` (story-590). Ici on ne fait que les TAGUER au marquage.

#[derive(Resource, Default)]
pub struct LootRoomState {
    pub in_room: bool,
    pub return_pos: Vec3,
    /// Pad de la zone courante (checkpoint de respawn sur chute).
    current_pad: Vec3,
    cooldown: f32,
}

/// Story-603 — état de la porte posée sur le dais melee_pit. `opened` suit
/// `RogueliteWave.boss_defeated` via `sys_reconcile_boss_gate` (fermée→ouverte au
/// boss-clear, ré-fermée au nouveau run). `placed_at` = centre du dais courant
/// (re-pose si le seed déplace le dais). `portal_pos` = sommet du dais (pose porte) ;
/// `parcours_entry` = pad zone 1.
#[derive(Resource, Default)]
pub struct BossGate {
    opened: bool,
    placed_at: Option<Vec3>,
    /// True quand les colliders TriMesh ont été ajoutés au mesh du dais (étape avant
    /// le raycast de surface — il faut 1 frame que rapier les enregistre).
    dais_ready: bool,
    /// Surface marchable au CENTRE du dais (raycast) — `None` tant que pas mesurée.
    /// La porte attend cette valeur → posée PILE sur le sol (pas l'AABB max).
    dais_top_y: Option<f32>,
    portal_pos: Vec3,
    parcours_entry: Vec3,
}

/// Kind d'item ramassable repéré dans le GLB par nom de node.
#[derive(Clone, Copy, PartialEq, Eq)]
enum LevelItemKind {
    Crown,
    Heart,
    Diamond,
    Coin,
    Star,
}

/// Item ramassable du niveau (mesh GLB SANS collider → traversable + collecté à effet).
#[derive(Component)]
struct LevelPickup {
    kind: LevelItemKind,
}

/// État de rétrécissement (couronne). `active` → joueur petit (permanent dans le parcours, lerp
/// scale). Reset en quittant le parcours.
#[derive(Resource, Default)]
pub struct ShrinkBuff {
    active: bool,
}

/// Phase du choix au portail de fin de zone (agency Hadès, story-585).
#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum RewardPhase {
    #[default]
    Closed,
    NeedRoll,
    Choosing,
}

/// Nature du choix présenté au portail (story-589) : armer un ÉLÉMENT tant qu'il
/// en reste à débloquer, sinon retomber sur un boon stat.
#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub(crate) enum ChoiceKind {
    #[default]
    Boon,
    Element,
}

/// Choix 1-parmi-3 GRATUIT au portail `Next` (touche 1/2/3) puis TP vers `target`.
/// Story-589 : si des éléments restent verrouillés → on offre des **éléments**
/// (`kind=Element`, `element_candidates`) ; sinon des **boons** (`candidates`).
/// Distinct du Coffre du Forgeron (shop, fin de wave).
#[derive(Resource, Default)]
pub(crate) struct ZoneReward {
    phase: RewardPhase,
    pub(crate) kind: ChoiceKind,
    pub(crate) candidates: Vec<BoonId>,
    pub(crate) element_candidates: Vec<Element>,
    target: Vec3,
}

impl ZoneReward {
    /// True quand les cartes sont affichées (le HUD lit ça).
    pub(crate) fn choosing(&self) -> bool {
        self.phase == RewardPhase::Choosing
    }
    /// True si le choix courant arme un élément (vs un boon).
    pub(crate) fn is_element_choice(&self) -> bool {
        self.kind == ChoiceKind::Element
    }
}

/// Nom de node GLB → kind d'item ramassable (sinon `None` = décor normal avec collider).
fn level_item_kind(name: &str) -> Option<LevelItemKind> {
    if name.starts_with("crown") {
        Some(LevelItemKind::Crown)
    } else if name.starts_with("heart") {
        Some(LevelItemKind::Heart)
    } else if name.starts_with("diamond") {
        Some(LevelItemKind::Diamond)
    } else if name.starts_with("coin") {
        Some(LevelItemKind::Coin)
    } else if name.starts_with("star") {
        Some(LevelItemKind::Star)
    } else {
        None
    }
}

pub struct RogueliteLootRoomPlugin;

impl Plugin for RogueliteLootRoomPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LootRoomState>()
            .init_resource::<ShrinkBuff>()
            .init_resource::<ZoneReward>()
            // Story-603 — porte du socle (boss-gated).
            .init_resource::<BossGate>()
            .add_systems(OnEnter(GameMode::Roguelite), sys_setup)
            // Story-603 — reset parcours au démarrage d'un run (anti soft-lock
            // si abandon depuis le parcours). Lobby → InRun = sortie du Lobby.
            .add_systems(OnExit(RunState::Lobby), sys_reset_parcours_on_run_start)
            .add_systems(
                Update,
                (
                    sys_portal_walkover,
                    sys_roll_zone_reward,
                    sys_zone_reward_pick,
                    sys_checkpoint_touch,
                    sys_spin_portals,
                    sys_collect_level_items,
                    sys_player_shrink,
                    sys_mark_demo_meshes,
                    sys_collide_demo_incremental,
                    // Story-603 — swap porte fermée↔ouverte + calibration GLB + pose sur socle.
                    sys_reconcile_boss_gate,
                    sys_calibrate_portal,
                    sys_ground_portal,
                )
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            // Story-603 — scintillement des flammes des yeux des crânes.
            .add_systems(
                Update,
                sys_flicker_portal_flames
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Roguelite)),
            );
    }
}

fn dist_xz(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

/// run_if : le combat (orchestrateur de vagues) tourne SEULEMENT hors du parcours.
pub fn combat_running(state: Res<LootRoomState>) -> bool {
    !state.in_room
}

fn sys_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gate: ResMut<BossGate>,
) {
    let s = LEVEL_SCALE;
    let level_t = Vec3::new(
        ROOM_ORIGIN.x - s * LVL_CENTER.x,
        ROOM_ORIGIN.y - s * LVL_MIN_Y,
        ROOM_ORIGIN.z - s * LVL_CENTER.z,
    );

    // ── Niveau démo COMPLET (Scene 0) — colliders incrémentaux ──────────────────
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(KIT_PATH));
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        DemoLevelRoot,
        DemoUnmarked,
        Name::new("DemoLevel"),
        SceneRoot(scene),
        Transform::from_translation(level_t).with_scale(Vec3::splat(s)),
    ));

    // ── Points de spawn/téléport au DÉBUT de chaque zone (sur la géométrie du
    //    niveau démo). 2026-06-08 : plus de pad cuboïde — le sol du GLB suffit.
    //    Les pads carrés étaient redondants avec le sol du parcours et celui de
    //    la zone 2 bloquait le joueur. Le joueur retombe ~2u sur la plateforme
    //    démo (top natif +4.86). pad_center sert encore à positionner
    //    portails/orbes/loot/checkpoints.
    let pad_spawns: [Vec3; 3] = std::array::from_fn(|z| {
        let n = ZONE_SPAWN_NATIVE[z];
        let pad_center = Vec3::new(
            level_t.x + s * n.x,
            level_t.y + s * n.y + 3.5, // ≈ dessus de la plateforme démo (top natif +4.86)
            level_t.z + s * n.z,
        );
        pad_center + Vec3::new(0.0, 2.0, 0.0)
    });

    // ── Loot (pièces + âmes) au spawn de chaque zone ────────────────────────────
    for spawn in pad_spawns {
        let base = spawn - Vec3::Y * 1.0;
        for off in [Vec3::new(-4.0, 1.0, 2.0), Vec3::new(4.0, 1.0, 2.0)] {
            spawn_coin(&mut commands, &mut meshes, &mut materials, base + off, 16);
        }
        spawn_soul(&mut commands, &mut meshes, &mut materials, base + Vec3::new(0.0, 1.4, 3.5), 5);
    }

    // ── Story-603 — la porte (boss-gated) est posée sur le dais melee_pit central
    //    par `sys_reconcile_boss_gate` (qui lit l'ancre AnchorKind::MeleePit, suit le
    //    seed). L'ancien anneau Enter (z=-34, toujours ouvert) est SUPPRIMÉ : le
    //    parcours est accessible UNIQUEMENT via cette porte, après la mort du boss.
    //    Ici on ne mémorise que l'entrée du parcours (pad zone 1).
    gate.parcours_entry = pad_spawns[0];

    // ── Portails INTRA-parcours : chaque zone a son portail AU BOUT du chemin
    //    (vraie traversée). Les sorties z1/z2 ouvrent le choix de boon ; z3 ramène
    //    à l'arène. Plus d'orbes auto : le seul upgrade = le choix au portail.
    for z in 0..3 {
        let pos = level_t + s * ZONE_END_NATIVE[z] + Vec3::Y * 2.0;
        let (kind, target, emissive) = if z < 2 {
            (PortalKind::Next, pad_spawns[z + 1], LinearRgba::new(0.6, 3.0, 0.4, 1.0))
        } else {
            (PortalKind::Return, Vec3::ZERO, LinearRgba::new(3.2, 1.4, 0.3, 1.0))
        };
        spawn_portal(&mut commands, &mut meshes, &mut materials, pos, emissive, kind, target);
    }

    // ── Checkpoint par zone, juste après la porte/herse (sur une vraie plateforme) : tomber
    //    ensuite renvoie ici, pas au début de la zone (plus de couronne→porte à refaire). ──
    for n in CHECKPOINT_NATIVE {
        let pos = level_t + s * n + Vec3::Y * 5.5;
        spawn_checkpoint(&mut commands, &mut meshes, &mut materials, pos);
    }

    info!("[loot-room] mode parcours 3 zones spawné (offset {ROOM_ORIGIN:?})");
}

// ── Colliders incrémentaux du niveau démo ───────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn sys_mark_demo_meshes(
    mut commands: Commands,
    q_root: Query<Entity, (With<DemoLevelRoot>, With<DemoUnmarked>)>,
    q_children: Query<&Children>,
    q_mesh: Query<(), With<Mesh3d>>,
    q_name: Query<&Name>,
    q_tf: Query<&Transform>,
    mut stats: ResMut<ObstacleStats>,
) {
    for root in &q_root {
        let Ok(top) = q_children.get(root) else {
            continue;
        };
        // Le 3e champ = `animated` : propagé au sous-arbre d'un obstacle pour
        // marquer aussi les entités-mesh ENFANT (qui portent le collider).
        let mut stack: Vec<(Entity, Option<LevelItemKind>, bool)> =
            top.iter().map(|e| (e, None, false)).collect();
        let mut count = 0u32;
        let (mut hammers, mut spinners, mut sliders) = (0u32, 0u32, 0u32);
        let mut items = 0u32;
        while let Some((e, mut item, mut animated)) = stack.pop() {
            // Item ramassable repéré par nom (couronne/cœur/…) → tag une fois sur le
            // node nommé ; son sous-arbre n'aura PAS de collider (traversable + collecté).
            if item.is_none() {
                if let Ok(name) = q_name.get(e) {
                    if let Some(k) = level_item_kind(name.as_str()) {
                        item = Some(k);
                        commands.entity(e).insert(LevelPickup { kind: k });
                        items += 1;
                    }
                }
            }
            if item.is_none() {
                if q_mesh.get(e).is_ok() {
                    commands.entity(e).insert(NeedsLevelCollider);
                    count += 1;
                }
                // Obstacle animé (story-590) : seulement le node-RACINE (pas déjà dans
                // un sous-arbre animé). On en fait un corps KinematicPositionBased → son
                // collider (sur ce node OU un enfant-mesh) bouge VRAIMENT dans Rapier
                // (le KCC bloque + intersect_shape détecte le chevauchement). Un collider
                // statique dont seul le parent bouge ne se resynchronise pas → le joueur
                // traversait l'obstacle (fix 2026-06-09, sensor push.events restait 0).
                if !animated {
                    if let Ok(name) = q_name.get(e) {
                        if let Some(anim) = classify(name.as_str()) {
                            let base = q_tf.get(e).copied().unwrap_or_default();
                            animated = true;
                            commands
                                .entity(e)
                                .insert(RigidBody::KinematicPositionBased);
                            match anim {
                                ObstacleAnim::Hammer => {
                                    commands.entity(e).insert(SwingingHammer {
                                        base: base.rotation,
                                        phase: phase_from_pos(base.translation),
                                    });
                                    hammers += 1;
                                }
                                ObstacleAnim::Spinner => {
                                    commands.entity(e).insert(RotatingObstacle);
                                    spinners += 1;
                                }
                                ObstacleAnim::Slider => {
                                    commands.entity(e).insert(SlidingObstacle {
                                        base: base.translation,
                                        axis: Vec3::X,
                                        phase: phase_from_pos(base.translation),
                                    });
                                    sliders += 1;
                                }
                            }
                        }
                    }
                }
            }
            // Marqueur de push sur TOUT le sous-arbre animé : le collider est souvent
            // sur une entité-mesh ENFANT créée par le loader glTF, pas le node nommé.
            if animated {
                commands.entity(e).insert(AnimatedObstacle);
            }
            if let Ok(ch) = q_children.get(e) {
                for c in ch.iter() {
                    stack.push((c, item, animated));
                }
            }
        }
        stats.hammers += hammers;
        stats.spinners += spinners;
        stats.sliders += sliders;
        commands.entity(root).remove::<DemoUnmarked>();
        info!(
            "[loot-room] niveau démo : {count} meshes collider ; {hammers} marteaux ; {spinners} balayeurs ; {sliders} blocs coulissants ; {items} items"
        );
    }
}

fn sys_collide_demo_incremental(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    q: Query<(Entity, &Mesh3d), With<NeedsLevelCollider>>,
) {
    let mut budget = 20u32;
    for (e, m3d) in &q {
        if budget == 0 {
            break;
        }
        let Some(mesh) = meshes.get(&m3d.0) else {
            continue;
        };
        budget -= 1;
        // TriMesh (pas ConvexHull) : suit la vraie forme → les arches gardent leur
        // ouverture (traversable), au lieu d'être bouchées par l'enveloppe convexe.
        if let Some(col) = Collider::from_bevy_mesh(mesh, &ComputedColliderShape::TriMesh(default())) {
            commands.entity(e).try_insert(col);
        }
        commands.entity(e).remove::<NeedsLevelCollider>();
    }
}

// ── Spawns visuels ──────────────────────────────────────────────────────────────

fn spawn_soul(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    value: u32,
) {
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.85, 0.95),
        emissive: LinearRgba::new(0.3, 2.6, 3.2, 1.0),
        perceptual_roughness: 0.3,
        ..default()
    });
    let mesh = meshes.add(Sphere::new(0.3));
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        Name::new("LootRoom_Soul"),
        crate::run::SoulWisp {
            value,
            collect_radius: 2.5,
            base_y: pos.y,
            phase: 0.0,
        },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(pos),
    ));
}

fn spawn_coin(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    value: u32,
) {
    let mesh = meshes.add(Sphere::new(0.35));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.8, 0.12),
        emissive: LinearRgba::new(2.5, 1.6, 0.2, 1.0),
        metallic: 0.4,
        ..default()
    });
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        Name::new("LootRoom_Coin"),
        Pickup {
            value,
            lifetime_secs: 1.0e9,
            collect_radius: 2.5,
        },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(pos),
        crate::CoinSpin,
    ));
}

#[allow(clippy::too_many_arguments)]
fn spawn_portal(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    emissive: LinearRgba,
    kind: PortalKind,
    target: Vec3,
) {
    let ring = meshes.add(Torus {
        minor_radius: 0.28,
        major_radius: 2.0,
    });
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.15, 0.2),
        emissive,
        ..default()
    });
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        PortalSpin,
        Portal { kind, target },
        Name::new("Portal"),
        Mesh3d(ring),
        MeshMaterial3d(mat),
        Transform::from_translation(pos)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        children![(
            PointLight {
                color: Color::srgb(emissive.red, emissive.green, emissive.blue),
                intensity: 3_000.0,
                range: 16.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::default(),
        )],
    ));
}

/// Balise de checkpoint : faisceau vert lumineux sur le chemin.
fn spawn_checkpoint(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
) {
    let mesh = meshes.add(Cylinder::new(0.3, 9.0));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.3, 0.15),
        emissive: LinearRgba::new(0.2, 3.0, 0.8, 1.0),
        ..default()
    });
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        Name::new("Checkpoint"),
        Checkpoint { pos },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(pos),
        children![(
            PointLight {
                color: Color::srgb(0.3, 1.0, 0.4),
                intensity: 2_000.0,
                range: 12.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::default(),
        )],
    ));
}

// ── Story-603 : socle + porte boss-gated ─────────────────────────────────────

/// Entité racine du dais melee_pit (nom `Module_melee_pit*`, posé par forgia-stage).
fn find_dais_root(q_named: &Query<(Entity, &Name)>) -> Option<Entity> {
    q_named
        .iter()
        .find(|(_, n)| n.as_str().starts_with(DAIS_MODULE_NAME_PREFIX))
        .map(|(e, _)| e)
}

/// Walk récursif : collecte les entités `Mesh3d` sous `root`.
fn collect_mesh_descendants(
    root: Entity,
    q_children: &Query<&Children>,
    q_mesh: &Query<&Mesh3d>,
    out: &mut Vec<Entity>,
) {
    if q_mesh.contains(root) {
        out.push(root);
    }
    if let Ok(children) = q_children.get(root) {
        for c in children.iter() {
            collect_mesh_descendants(c, q_children, q_mesh, out);
        }
    }
}

/// Ajoute un collider TriMesh à chaque mesh du dais (→ solide + marchable, épouse
/// le visuel exact, visible F10). Retourne `true` quand tous les meshes sont chargés
/// et collidérés (retry sinon). Idempotent (skip si déjà collideré).
fn solidify_dais(
    commands: &mut Commands,
    root: Entity,
    q_children: &Query<&Children>,
    q_mesh: &Query<&Mesh3d>,
    q_has_col: &Query<(), With<Collider>>,
    meshes: &Assets<Mesh>,
) -> bool {
    let mut mesh_ents = Vec::new();
    collect_mesh_descendants(root, q_children, q_mesh, &mut mesh_ents);
    if mesh_ents.is_empty() {
        return false; // scène pas encore peuplée
    }
    let all_loaded = mesh_ents
        .iter()
        .all(|e| q_mesh.get(*e).ok().and_then(|m| meshes.get(&m.0)).is_some());
    if !all_loaded {
        return false;
    }
    for &e in &mesh_ents {
        if q_has_col.get(e).is_ok() {
            continue; // déjà collideré
        }
        if let Ok(m3d) = q_mesh.get(e) {
            if let Some(mesh) = meshes.get(&m3d.0) {
                if let Some(col) =
                    Collider::from_bevy_mesh(mesh, &ComputedColliderShape::TriMesh(default()))
                {
                    commands.entity(e).try_insert(col);
                }
            }
        }
    }
    true
}

/// Surface marchable au CENTRE du dais via raycast vertical (sur les colliders
/// qu'on vient d'ajouter). Donne le VRAI sol où poser la porte — contrairement à
/// l'AABB max qui captait une déco haute (totem/braséro) → top=3.67 m alors que le
/// sol est ~1 m. `None` si rien touché (colliders pas encore enregistrés → retry).
fn raycast_dais_surface(center: Vec3, rapier: &ReadRapierContext) -> Option<f32> {
    let ctx = rapier.single().ok()?;
    let origin = Vec3::new(center.x, center.y + 60.0, center.z);
    ctx.cast_ray(origin, Vec3::NEG_Y, 120.0, true, QueryFilter::default())
        .map(|(_, toi)| origin.y - toi)
}

/// Porte FERMÉE : GLB visuel (calibré AABB + posé sur le socle) + collider cuboïde
/// BLOQUANT (le joueur ne peut pas la franchir) + flammes dans les yeux des crânes.
/// PAS de composant `Portal` → ignorée par le walk-over.
fn spawn_closed_portal(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    yaw: f32,
) {
    let glb = asset_server.load(GltfAssetLabel::Scene(0).from_asset(PORTAL_CLOSED_GLB));
    let parent = commands
        .spawn((
            LootRoomMarker,
            DespawnOnExit(GameMode::Roguelite),
            ClosedPortal,
            Name::new("ClosedPortal"),
            RigidBody::Fixed,
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
        ))
        .id();
    // Collider bloquant dimensionné depuis la cible (fiable, indépendant du
    // chargement async du GLB — cf decor.rs : collider primitif au spawn).
    commands.spawn((
        ChildOf(parent),
        Name::new("ClosedPortalCollider"),
        Transform::from_xyz(0.0, PORTAL_TARGET_HEIGHT * 0.5, 0.0),
        Collider::cuboid(
            PORTAL_TARGET_HEIGHT * 0.30,
            PORTAL_TARGET_HEIGHT * 0.5,
            0.6,
        ),
    ));
    // Visuel GLB (scale calibré + posé sur le socle par sys_calibrate/ground_portal).
    commands.spawn((
        ChildOf(parent),
        Name::new("ClosedPortalVisual"),
        SceneRoot(glb),
        Transform::IDENTITY,
        NeedsPortalCalibrate {
            target_h: PORTAL_TARGET_HEIGHT,
            base_world_y: pos.y,
        },
    ));
    spawn_eye_flames(commands, meshes, materials, parent);
}

/// Porte OUVERTE : parent (`Portal{Enter}` walk-over → parcours + glow teal +
/// flammes des yeux) + enfant visuel GLB (calibré + posé sur le socle). PAS de
/// collider bloquant (on entre dedans). Spawné au boss-clear.
#[allow(clippy::too_many_arguments)]
fn spawn_open_portal(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    target: Vec3,
    yaw: f32,
) {
    let glb = asset_server.load(GltfAssetLabel::Scene(0).from_asset(PORTAL_OPEN_GLB));
    let parent = commands
        .spawn((
            LootRoomMarker,
            DespawnOnExit(GameMode::Roguelite),
            OpenPortal,
            Portal {
                kind: PortalKind::Enter,
                target,
            },
            Name::new("OpenPortal"),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            children![(
                PointLight {
                    color: Color::srgb(0.3, 1.0, 1.2),
                    intensity: 4_000.0,
                    range: 18.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, PORTAL_TARGET_HEIGHT * 0.5, 0.0),
            )],
        ))
        .id();
    commands.spawn((
        ChildOf(parent),
        Name::new("OpenPortalVisual"),
        SceneRoot(glb),
        Transform::IDENTITY,
        NeedsPortalCalibrate {
            target_h: PORTAL_TARGET_HEIGHT,
            base_world_y: pos.y,
        },
    ));
    spawn_eye_flames(commands, meshes, materials, parent);
}

/// Spawn les 4 flammes (1 par œil de crâne) en enfants du `parent` du portail,
/// aux offsets `SKULL_EYE_OFFSETS`. Chaque flamme = sphère émissive orange +
/// PointLight scintillant (`sys_flicker_portal_flames`). Mesh+matériau partagés.
fn spawn_eye_flames(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    parent: Entity,
) {
    let mesh = meshes.add(Sphere::new(0.12));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.5, 0.12),
        emissive: LinearRgba::new(7.0, 2.2, 0.3, 1.0),
        unlit: true,
        ..default()
    });
    for (i, off) in SKULL_EYE_OFFSETS.iter().enumerate() {
        commands.spawn((
            ChildOf(parent),
            Name::new("PortalEyeFlame"),
            PortalFlame {
                base_intensity: 2_200.0,
                phase: i as f32 * 1.7,
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(*off),
            PointLight {
                color: Color::srgb(1.0, 0.45, 0.12),
                intensity: 2_200.0,
                range: 4.5,
                shadows_enabled: false,
                ..default()
            },
        ));
    }
}

/// Story-603 — ouvre/ferme la porte du socle en suivant `RogueliteWave.boss_defeated`.
/// Boss vaincu → fermée→ouverte (parcours débloqué). Nouveau run (reset
/// `boss_defeated`) → ouverte→fermée. Cheap : agit seulement sur transition.
#[allow(clippy::too_many_arguments)]
fn sys_reconcile_boss_gate(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    wave: Res<RogueliteWave>,
    mut gate: ResMut<BossGate>,
    q_anchor: Query<(&AnchorPoint, &Transform)>,
    q_named: Query<(Entity, &Name)>,
    q_children: Query<&Children>,
    q_mesh3d: Query<&Mesh3d>,
    q_has_col: Query<(), With<Collider>>,
    q_closed: Query<Entity, With<ClosedPortal>>,
    q_open: Query<Entity, With<OpenPortal>>,
    rapier: ReadRapierContext,
) {
    // 1. Centre du dais = ancre MeleePit (suit le seed) ; sinon garde la place
    //    actuelle ; sinon repli central (1re frame, avant chargement du stage).
    let anchor_pos = q_anchor
        .iter()
        .find(|(a, _)| a.kind == AnchorKind::MeleePit)
        .map(|(_, t)| t.translation);
    let dais_center = anchor_pos.or(gate.placed_at).unwrap_or(DAIS_FALLBACK_POS);
    // Yaw : la porte regarde le spawn (origine), +PORTAL_YAW_OFFSET (PI) car l'avant
    // natif du GLB est +Z (sans ça la porte tournait le dos au joueur).
    let yaw = dais_center.x.atan2(dais_center.z) + PORTAL_YAW_OFFSET;

    // 2. (Re)pose si le dais a bougé (nouveau run/seed) ou jamais posé : on efface
    //    nos portes et on ré-arme la mesure. Les colliders TriMesh ajoutés au mesh du
    //    dais partent avec lui (cleanup forgia-stage au reload de stage).
    let moved = gate
        .placed_at
        .map(|p| p.distance(dais_center) > 0.5)
        .unwrap_or(true);
    if moved {
        for e in q_closed.iter().chain(&q_open) {
            commands.entity(e).despawn();
        }
        gate.placed_at = Some(dais_center);
        gate.dais_ready = false;
        gate.dais_top_y = None;
        gate.opened = false;
        return;
    }

    // 3a. Solidifie le dais : collider TriMesh sur SON mesh (épouse le visuel,
    //     marchable, F10). Attend 1 frame ensuite que rapier l'enregistre.
    if !gate.dais_ready {
        let Some(root) = find_dais_root(&q_named) else {
            return; // module melee_pit pas encore spawné
        };
        if !solidify_dais(&mut commands, root, &q_children, &q_mesh3d, &q_has_col, &meshes) {
            return; // meshes du dais pas tous chargés → retry
        }
        gate.dais_ready = true;
        return;
    }

    // 3b. Surface marchable au CENTRE via raycast (sur les colliders ajoutés) →
    //     porte posée PILE sur le sol. Fini l'AABB max qui captait une déco haute
    //     (totem/braséro) → top=3.67 m alors que le sol est ~1 m.
    if gate.dais_top_y.is_none() {
        let Some(top) = raycast_dais_surface(dais_center, &rapier) else {
            return; // colliders pas encore enregistrés par rapier → retry
        };
        // Garde : sol du stage à y≈0. Un hit ≤ 0.5 = le collider du dais n'est pas
        // encore enregistré (rayon traversé jusqu'au sol) → on retente (évite de
        // poser la porte sous terre).
        if top <= 0.5 {
            return;
        }
        let portal_pos = Vec3::new(dais_center.x, top + PORTAL_Y_OFFSET, dais_center.z);
        spawn_closed_portal(&mut commands, &asset_server, &mut meshes, &mut materials, portal_pos, yaw);
        gate.dais_top_y = Some(top);
        gate.portal_pos = portal_pos;
        info!(
            "[boss-gate] surface dais (raycast) y={top:.2}m → porte posée @ ({:.1},{:.1},{:.1}), yaw={:.0}°",
            portal_pos.x, portal_pos.y, portal_pos.z, yaw.to_degrees()
        );
        return;
    }

    // 4. Swap fermée↔ouverte selon boss_defeated.
    if wave.boss_defeated && !gate.opened {
        for e in &q_closed {
            commands.entity(e).despawn();
        }
        spawn_open_portal(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut materials,
            gate.portal_pos,
            gate.parcours_entry,
            yaw,
        );
        gate.opened = true;
        info!("[boss-gate] BOSS VAINCU → porte du dais OUVERTE (parcours débloqué)");
    } else if !wave.boss_defeated && gate.opened {
        for e in &q_open {
            commands.entity(e).despawn();
        }
        spawn_closed_portal(&mut commands, &asset_server, &mut meshes, &mut materials, gate.portal_pos, yaw);
        gate.opened = false;
        info!("[boss-gate] nouveau run → porte du dais REFERMÉE");
    }
}

/// Story-603 — reset l'état parcours au démarrage d'un run (sortie du Lobby).
/// Sans ça, un run abandonné DEPUIS le parcours (`in_room=true`) gèlerait le
/// combat du run suivant (`combat_running()` → false, aucune wave) et laisserait
/// le joueur rétréci (`ShrinkBuff`). La porte (`BossGate`) se ré-aligne seule via
/// `sys_reconcile_boss_gate` (boss_defeated reset → branche fermeture).
fn sys_reset_parcours_on_run_start(
    mut state: ResMut<LootRoomState>,
    mut shrink: ResMut<ShrinkBuff>,
) {
    *state = LootRoomState::default();
    shrink.active = false;
}

/// Scale un SceneRoot de portail à `target_h` une fois l'AABB chargée (GLB de
/// taille native inconnue). Mirroir de `decor::sys_calibrate_decor`.
fn sys_calibrate_portal(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsPortalCalibrate)>,
    q_aabb: Query<&Aabb>,
    q_children: Query<&Children>,
    mut q_tf: Query<&mut Transform>,
) {
    for (e, needs) in &q_needs {
        let Some(max_dim) = portal_aabb_max_dim(e, &q_aabb, &q_children) else {
            continue; // scène pas encore chargée → retry next frame
        };
        if max_dim <= 0.0 || !max_dim.is_finite() {
            commands.entity(e).remove::<NeedsPortalCalibrate>();
            continue;
        }
        let scale = needs.target_h / max_dim;
        if let Ok(mut tf) = q_tf.get_mut(e) {
            tf.scale = Vec3::splat(scale);
        }
        // Passe le relais au grounding : une fois le scale propagé (frame
        // suivante), `sys_ground_portal` posera la base réelle sur le socle.
        commands
            .entity(e)
            .remove::<NeedsPortalCalibrate>()
            .insert(NeedsPortalGround {
                base_world_y: needs.base_world_y,
            });
    }
}

/// Story-603 — pose la BASE réelle du portail sur le socle. Après le scale (frame
/// précédente, GlobalTransform propagé), mesure le min Y monde de la géométrie et
/// décale le SceneRoot pour que ce min repose sur `base_world_y` (corrige pivot GLB
/// non centré au pied → « le portail est dans le socle »).
fn sys_ground_portal(
    mut commands: Commands,
    q_needs: Query<(Entity, &NeedsPortalGround)>,
    q_children: Query<&Children>,
    q_gt_aabb: Query<(&GlobalTransform, &Aabb)>,
    mut q_tf: Query<&mut Transform>,
) {
    for (root, ground) in &q_needs {
        let mut min_y = f32::MAX;
        let mut found = false;
        collect_world_min_y(root, &q_children, &q_gt_aabb, &mut min_y, &mut found);
        if !found {
            continue; // GlobalTransform/Aabb pas encore propagés → retry
        }
        let delta = ground.base_world_y - min_y;
        if let Ok(mut tf) = q_tf.get_mut(root) {
            tf.translation.y += delta;
        }
        commands.entity(root).remove::<NeedsPortalGround>();
    }
}

/// Walk récursif : min Y monde sur tous les `Aabb` du sous-arbre (8 coins
/// transformés par leur `GlobalTransform`, robuste aux rotations).
fn collect_world_min_y(
    e: Entity,
    q_children: &Query<&Children>,
    q_gt_aabb: &Query<(&GlobalTransform, &Aabb)>,
    acc: &mut f32,
    found: &mut bool,
) {
    if let Ok((gt, aabb)) = q_gt_aabb.get(e) {
        let c = Vec3::from(aabb.center);
        let he = Vec3::from(aabb.half_extents);
        for sx in [-1.0_f32, 1.0] {
            for sy in [-1.0_f32, 1.0] {
                for sz in [-1.0_f32, 1.0] {
                    let corner = c + Vec3::new(sx * he.x, sy * he.y, sz * he.z);
                    *acc = acc.min(gt.transform_point(corner).y);
                }
            }
        }
        *found = true;
    }
    if let Ok(children) = q_children.get(e) {
        for child in children.iter() {
            collect_world_min_y(child, q_children, q_gt_aabb, acc, found);
        }
    }
}

/// Scintillement des flammes des yeux (intensité PointLight modulée, phase/œil).
fn sys_flicker_portal_flames(time: Res<Time>, mut q: Query<(&PortalFlame, &mut PointLight)>) {
    let t = time.elapsed_secs();
    for (flame, mut light) in &mut q {
        let k = 0.65 + 0.35 * (t * 11.0 + flame.phase).sin();
        light.intensity = flame.base_intensity * k;
    }
}

/// Walk récursif des Children pour le 1er `Aabb` ; `max(half_extents)*2`.
fn portal_aabb_max_dim(
    root: Entity,
    q_aabb: &Query<&Aabb>,
    q_children: &Query<&Children>,
) -> Option<f32> {
    if let Ok(a) = q_aabb.get(root) {
        return Some(a.half_extents.max_element() * 2.0);
    }
    let children = q_children.get(root).ok()?;
    let mut max = 0.0_f32;
    let mut found = false;
    for child in children.iter() {
        if let Some(d) = portal_aabb_max_dim(child, q_aabb, q_children) {
            max = max.max(d);
            found = true;
        }
    }
    if found {
        Some(max)
    } else {
        None
    }
}

/// Toucher une balise (et seulement en avançant : z croissant) met à jour le respawn sur chute.
fn sys_checkpoint_touch(
    time: Res<Time>,
    mut state: ResMut<LootRoomState>,
    q_player: Query<&Transform, With<Player>>,
    q_cp: Query<&Checkpoint>,
    mut popups: ResMut<KillPopupState>,
) {
    if !state.in_room {
        return;
    }
    let Ok(ptf) = q_player.single() else {
        return;
    };
    for cp in &q_cp {
        if dist_xz(ptf.translation, cp.pos) < 3.5 && cp.pos.z > state.current_pad.z {
            state.current_pad = cp.pos;
            popups.active.push(KillPopup {
                world_pos: cp.pos + Vec3::Y * 1.0,
                text: "CHECKPOINT",
                color: egui::Color32::from_rgb(120, 255, 160),
                spawned_secs: time.elapsed_secs(),
            });
            info!("[loot-room] checkpoint atteint ({:.0},{:.0})", cp.pos.x, cp.pos.z);
        }
    }
}

fn sys_spin_portals(time: Res<Time>, mut q: Query<&mut Transform, With<PortalSpin>>) {
    let dt = time.delta_secs();
    for mut tf in &mut q {
        tf.rotate_local_z(dt * 0.8);
    }
}

/// Ramassage walk-over des items du niveau (couronne/cœur/diamant/pièce/étoile) → avantages in-game.
/// Utilise `GlobalTransform` car les items sont des nodes enfants du GLB (Transform local ≠ monde).
#[allow(clippy::too_many_arguments)]
fn sys_collect_level_items(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<LootRoomState>,
    q_player: Query<&Transform, With<Player>>,
    q_items: Query<(Entity, &GlobalTransform, &LevelPickup)>,
    mut shrink: ResMut<ShrinkBuff>,
    mut meta: ResMut<crate::run::MetaSouls>,
    mut q_health: Query<&mut Health, With<Player>>,
    mut active: ResMut<ActiveBoons>,
    catalogue: Option<Res<BoonsCatalogue>>,
    mut popups: ResMut<KillPopupState>,
    mut diamond_idx: Local<usize>,
) {
    if !state.in_room {
        return;
    }
    let Ok(ptf) = q_player.single() else {
        return;
    };
    for (e, gt, item) in &q_items {
        let ipos = gt.translation();
        if dist_xz(ptf.translation, ipos) >= ITEM_COLLECT_RADIUS {
            continue;
        }
        let (text, color): (&'static str, egui::Color32) = match item.kind {
            LevelItemKind::Crown => {
                shrink.active = true;
                ("COURONNE — PETIT !", egui::Color32::from_rgb(255, 215, 0))
            }
            LevelItemKind::Heart => {
                if let Ok(mut h) = q_health.single_mut() {
                    h.max += HEART_MAX_HP;
                    h.current = (h.current + HEART_MAX_HP).min(h.max);
                }
                ("+PV MAX", egui::Color32::from_rgb(231, 76, 60))
            }
            LevelItemKind::Diamond => {
                if let Some(cat) = catalogue.as_deref() {
                    if !cat.entries.is_empty() {
                        let def = cat.entries[*diamond_idx % cat.entries.len()].clone();
                        *diamond_idx += 1;
                        active.apply(&def, cat);
                    }
                }
                ("BOON !", egui::Color32::from_rgb(120, 200, 255))
            }
            LevelItemKind::Coin => {
                meta.current = meta.current.saturating_add(COIN_VALUE);
                meta.earned_run = meta.earned_run.saturating_add(COIN_VALUE);
                ("+OR", egui::Color32::from_rgb(255, 200, 40))
            }
            LevelItemKind::Star => {
                meta.current = meta.current.saturating_add(STAR_VALUE);
                meta.earned_run = meta.earned_run.saturating_add(STAR_VALUE);
                ("+AMES", egui::Color32::from_rgb(180, 120, 255))
            }
        };
        popups.active.push(KillPopup {
            world_pos: ipos + Vec3::Y * 1.0,
            text,
            color,
            spawned_secs: time.elapsed_secs(),
        });
        info!("[loot-room] item ramassé : {text} (pos {:.0},{:.0})", ipos.x, ipos.z);
        commands.entity(e).despawn();
    }
}

/// Couronne → rétrécissement : lerp le scale du Player vers petit tant que `ShrinkBuff` est actif
/// (le collider capsule ET la caméra 1P — enfant du Player — héritent du scale → on rapetisse et on
/// passe sous la porte mi-ouverte), puis retour à 1.0. Hors parcours → toujours 1.0.
fn sys_player_shrink(
    time: Res<Time>,
    mut shrink: ResMut<ShrinkBuff>,
    state: Res<LootRoomState>,
    mut q_player: Query<&mut Transform, With<Player>>,
) {
    // Hors parcours → reset (on re-grandit ; on ne re-rapetisse qu'en reprenant une couronne).
    if !state.in_room {
        shrink.active = false;
    }
    let Ok(mut tf) = q_player.single_mut() else {
        return;
    };
    let target = if state.in_room && shrink.active {
        SHRINK_SCALE
    } else {
        1.0
    };
    let cur = tf.scale.x;
    let t = 1.0 - (-time.delta_secs() * SHRINK_LERP).exp();
    let next = cur + (target - cur) * t;
    if (next - cur).abs() < 1.0e-5 {
        return;
    }
    tf.scale = Vec3::splat(next);
    // Garde les pieds au sol quand la capsule change de taille : en re-grandissant, le bas de la
    // capsule s'enfoncerait dans le sol (joueur bloqué). On remonte le centre du delta de demi-hauteur.
    tf.translation.y += (next - cur) * PLAYER_CAPSULE_HALF;
}

#[allow(clippy::too_many_arguments)]
fn sys_portal_walkover(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<LootRoomState>,
    mut reward: ResMut<ZoneReward>,
    mut q_player: Query<(Entity, &mut Transform), With<Player>>,
    q_portals: Query<(&Transform, &Portal), Without<Player>>,
    wave: Res<RogueliteWave>,
) {
    // Pendant un choix de boon (cartes affichées), on gèle les portails + la chute-respawn.
    if reward.phase != RewardPhase::Closed {
        return;
    }
    if state.cooldown > 0.0 {
        state.cooldown -= time.delta_secs();
        return;
    }
    let Ok((player, mut ptf)) = q_player.single_mut() else {
        return;
    };

    // Chute = retour au pad de la zone courante (checkpoint).
    if state.in_room && ptf.translation.y < ROOM_ORIGIN.y + FALL_DEATH_Y {
        ptf.translation = state.current_pad;
        state.cooldown = TELEPORT_COOLDOWN;
        info!("[loot-room] chute → respawn pad zone courante");
        return;
    }

    for (portal_tf, portal) in &q_portals {
        let relevant = match portal.kind {
            // Story-603 — Enter (arène → parcours) : hors parcours ET boss vaincu
            // (garde explicite ; la porte ouverte n'existe déjà qu'après le boss).
            PortalKind::Enter => !state.in_room && wave.boss_defeated,
            _ => state.in_room,
        };
        if !relevant {
            continue;
        }
        if dist_xz(ptf.translation, portal_tf.translation) >= PORTAL_RADIUS {
            continue;
        }
        match portal.kind {
            PortalKind::Enter => {
                state.return_pos = ptf.translation;
                ptf.translation = portal.target;
                state.current_pad = portal.target;
                state.in_room = true;
                commands.entity(player).remove::<BotTarget>();
                info!("[loot-room] → parcours zone 1");
            }
            PortalKind::Next => {
                // Story-585 : au lieu de TP direct, ouvre le choix de boon (le TP se fait au pick).
                reward.phase = RewardPhase::NeedRoll;
                reward.target = portal.target;
                info!("[loot-room] portail zone suivante → choix de boon");
            }
            PortalKind::Return => {
                ptf.translation = state.return_pos + Vec3::new(0.0, 0.0, 5.0);
                state.in_room = false;
                commands.entity(player).insert(BotTarget);
                info!("[loot-room] → retour arène");
            }
        }
        state.cooldown = TELEPORT_COOLDOWN;
        return;
    }
}

/// Prépare le choix quand un portail `Next` l'a ouvert. Story-589 : s'il reste
/// des éléments verrouillés → on offre les ÉLÉMENTS (jusqu'à 3) ; sinon on tire
/// 3 boons. Aucune option éligible → TP direct.
#[allow(clippy::too_many_arguments)]
fn sys_roll_zone_reward(
    mut reward: ResMut<ZoneReward>,
    catalogue: Option<Res<BoonsCatalogue>>,
    active: Res<ActiveBoons>,
    unlocks: Res<ElementUnlocks>,
    mut rng: ResMut<CoffreRng>,
    mut state: ResMut<LootRoomState>,
    mut q_player: Query<&mut Transform, With<Player>>,
) {
    if reward.phase != RewardPhase::NeedRoll {
        return;
    }

    // 1) Tant qu'un élément reste verrouillé → on l'offre (max 3 cartes).
    let locked: Vec<Element> = unlocks.locked().into_iter().take(3).collect();
    if !locked.is_empty() {
        info!("[loot-room] choix d'élément : {} à armer", locked.len());
        reward.kind = ChoiceKind::Element;
        reward.element_candidates = locked;
        reward.candidates.clear();
        reward.phase = RewardPhase::Choosing;
        return;
    }

    // 2) Tous éléments armés → choix de boon stat (comportement story-585).
    let candidates = catalogue
        .as_deref()
        .map(|cat| roll_candidates(cat, &active, 3, &mut rng_next_index(&mut rng.0)))
        .unwrap_or_default();
    if candidates.is_empty() {
        // Aucune option → pas de choix, on TP directement.
        if let Ok(mut ptf) = q_player.single_mut() {
            ptf.translation = reward.target;
            state.current_pad = reward.target;
        }
        reward.phase = RewardPhase::Closed;
        state.cooldown = TELEPORT_COOLDOWN;
        return;
    }
    info!("[loot-room] choix de boon : {} candidats", candidates.len());
    reward.kind = ChoiceKind::Boon;
    reward.candidates = candidates;
    reward.element_candidates.clear();
    reward.phase = RewardPhase::Choosing;
}

/// Touche 1/2/3 → applique le choix (gratuit) + TP vers la zone suivante + ferme.
/// Story-589 : branche selon `reward.kind` (armer un élément, ou appliquer un boon).
#[allow(clippy::too_many_arguments)]
fn sys_zone_reward_pick(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut reward: ResMut<ZoneReward>,
    mut state: ResMut<LootRoomState>,
    mut q_player: Query<&mut Transform, With<Player>>,
    mut active: ResMut<ActiveBoons>,
    mut unlocks: ResMut<ElementUnlocks>,
    catalogue: Option<Res<BoonsCatalogue>>,
    mut popups: ResMut<KillPopupState>,
) {
    if reward.phase != RewardPhase::Choosing {
        return;
    }
    let pick = if keys.just_pressed(KeyCode::Digit1) || keys.just_pressed(KeyCode::Numpad1) {
        0
    } else if keys.just_pressed(KeyCode::Digit2) || keys.just_pressed(KeyCode::Numpad2) {
        1
    } else if keys.just_pressed(KeyCode::Digit3) || keys.just_pressed(KeyCode::Numpad3) {
        2
    } else {
        return;
    };
    let Ok(mut ptf) = q_player.single_mut() else {
        return;
    };

    match reward.kind {
        ChoiceKind::Element => {
            let Some(&element) = reward.element_candidates.get(pick) else {
                return; // touche hors candidats
            };
            unlocks.unlock(element);
            popups.active.push(KillPopup {
                world_pos: ptf.translation + Vec3::Y * 1.5,
                text: element.armed_popup(),
                color: {
                    let [r, g, b] = element.rgb(&Default::default());
                    egui::Color32::from_rgb(
                        (r * 255.0) as u8,
                        (g * 255.0) as u8,
                        (b * 255.0) as u8,
                    )
                },
                spawned_secs: time.elapsed_secs(),
            });
            info!("[loot-room] élément armé : {}", element.fr_name());
        }
        ChoiceKind::Boon => {
            let Some(id) = reward.candidates.get(pick).cloned() else {
                return; // touche hors candidats
            };
            if let Some(cat) = catalogue.as_deref() {
                if let Some(def) = cat.entries.iter().find(|b| b.id == id) {
                    active.apply(def, cat);
                    popups.active.push(KillPopup {
                        world_pos: ptf.translation + Vec3::Y * 1.5,
                        text: "BOON CHOISI !",
                        color: egui::Color32::from_rgb(120, 220, 255),
                        spawned_secs: time.elapsed_secs(),
                    });
                    info!("[loot-room] boon choisi : {}", def.name);
                }
            }
        }
    }

    ptf.translation = reward.target;
    state.current_pad = reward.target;
    reward.phase = RewardPhase::Closed;
    reward.kind = ChoiceKind::Boon;
    reward.candidates.clear();
    reward.element_candidates.clear();
    state.cooldown = TELEPORT_COOLDOWN;
}
