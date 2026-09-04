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
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_egui::egui;
use bevy_rapier3d::prelude::{Collider, ComputedColliderShape, RigidBody};
use forgia_ai_arena_bot::BotTarget;
use forgia_core::prelude::*;
use forgia_damage::Health;
use forgia_player::Player;
use forgia_rpg_data::boons::{
    rng_next_index, roll_candidates_weighted, ActiveBoons, BoonId, BoonsCatalogue, CoffreRng,
    UnlockedBoonTiers,
};
use forgia_rpg_data::loot_tables::Pickup;

const ROOM_ORIGIN: Vec3 = Vec3::new(2000.0, 0.0, 2000.0);
const PORTAL_RADIUS: f32 = 2.8;

// ── Constantes parcours ───────────────────────────────────────────────────────

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
pub(crate) enum PortalKind {
    /// Arène → zone 1 (entre dans le parcours).
    Enter,
    /// Zone N → zone N+1.
    Next,
    /// Zone 3 → retour arène.
    Return,
}

#[derive(Component)]
pub(crate) struct Portal {
    pub(crate) kind: PortalKind,
    pub(crate) target: Vec3,
}
#[derive(Component)]
pub(crate) struct LootRoomMarker;
#[derive(Component)]
struct PortalSpin;
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
/// Handle préchargé du parcours. Garder l'asset en mémoire évite l'I/O au moment
/// de la victoire, sans pour autant instancier ses milliers d'entités pendant le combat.
#[derive(Resource)]
struct DemoLevelScene(Handle<Scene>);
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
            .add_systems(OnEnter(GameMode::Roguelite), sys_setup)
            // Le parcours est une scène lourde (~3 000 descendants). Une victoire
            // reste dans GameMode::Roguelite et passe seulement à RunState::Lobby :
            // DespawnOnExit(GameMode) ne la nettoie donc pas. Sans ce cleanup, le
            // lobby continue à propager ses transforms et la run suivante hérite de
            // l'ancien niveau. On ne despawn que la racine de scène, pas les portails
            // et props statiques préparés par sys_setup pour la prochaine run.
            .add_systems(OnEnter(RunState::Lobby), sys_despawn_demo_level_in_lobby)
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
                    sys_spawn_demo_level_after_boss,
                    sys_mark_demo_meshes,
                    sys_collide_demo_incremental,
                )
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            // Story-603 — porte boss-gated (systèmes dans boss_portal.rs).
            .add_plugins(crate::boss_portal::BossPortalPlugin);
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
    mut gate: ResMut<crate::boss_portal::BossGate>,
) {
    let s = LEVEL_SCALE;
    let level_t = Vec3::new(
        ROOM_ORIGIN.x - s * LVL_CENTER.x,
        ROOM_ORIGIN.y - s * LVL_MIN_Y,
        ROOM_ORIGIN.z - s * LVL_CENTER.z,
    );

    // Précharge le GLB mais ne l'instancie pas encore. `DemoLevel` contient près de
    // 3 000 descendants : le créer dès l'entrée du mode faisait travailler la
    // propagation des transforms pendant chaque vague, alors qu'il est inaccessible
    // avant la victoire du boss.
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(KIT_PATH));
    commands.insert_resource(DemoLevelScene(scene));

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
        spawn_soul(
            &mut commands,
            &mut meshes,
            &mut materials,
            base + Vec3::new(0.0, 1.4, 3.5),
            5,
        );
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
            (
                PortalKind::Next,
                pad_spawns[z + 1],
                LinearRgba::new(0.6, 3.0, 0.4, 1.0),
            )
        } else {
            (
                PortalKind::Return,
                Vec3::ZERO,
                LinearRgba::new(3.2, 1.4, 0.3, 1.0),
            )
        };
        spawn_portal(
            &mut commands,
            &mut meshes,
            &mut materials,
            pos,
            emissive,
            kind,
            target,
        );
    }

    // ── Checkpoint par zone, juste après la porte/herse (sur une vraie plateforme) : tomber
    //    ensuite renvoie ici, pas au début de la zone (plus de couronne→porte à refaire). ──
    for n in CHECKPOINT_NATIVE {
        let pos = level_t + s * n + Vec3::Y * 5.5;
        spawn_checkpoint(&mut commands, &mut meshes, &mut materials, pos);
    }

    info!("[loot-room] mode parcours 3 zones spawné (offset {ROOM_ORIGIN:?})");
}

/// Instancie le parcours seulement après le boss. La porte ne devient alors
/// franchissable qu'une fois la scène déjà demandée, ce qui retire son grand arbre
/// de transforms de toute la phase combat.
fn sys_spawn_demo_level_after_boss(
    mut commands: Commands,
    scene: Option<Res<DemoLevelScene>>,
    wave: Res<RogueliteWave>,
    q_existing: Query<(), With<DemoLevelRoot>>,
) {
    if !wave.boss_defeated || !q_existing.is_empty() {
        return;
    }
    let Some(scene) = scene else {
        return;
    };

    let s = LEVEL_SCALE;
    let level_t = Vec3::new(
        ROOM_ORIGIN.x - s * LVL_CENTER.x,
        ROOM_ORIGIN.y - s * LVL_MIN_Y,
        ROOM_ORIGIN.z - s * LVL_CENTER.z,
    );
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        DemoLevelRoot,
        DemoUnmarked,
        Name::new("DemoLevel"),
        SceneRoot(scene.0.clone()),
        Transform::from_translation(level_t).with_scale(Vec3::splat(s)),
    ));
    info!("[loot-room] parcours instancié après la victoire du boss");
}

/// Nettoie le sous-arbre lourd du parcours dès le retour au lobby intra-mode.
/// `despawn()` cascade aux descendants Bevy et le handle de scène est relâché :
/// aucune référence forte ne retient alors ce gros GLB entre deux runs. La scène
/// est préchargée à nouveau à la sortie du lobby, pendant la préparation de run.
/// L'événement arrive hors combat, après l'overlay de résultat.
fn sys_despawn_demo_level_in_lobby(
    mut commands: Commands,
    q_root: Query<Entity, With<DemoLevelRoot>>,
) {
    let mut count = 0u32;
    for root in &q_root {
        commands.entity(root).despawn();
        count += 1;
    }
    if count > 0 {
        info!("[loot-room] Lobby — parcours DemoLevel despawned ({count} root)");
    }
    commands.remove_resource::<DemoLevelScene>();
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
                            commands.entity(e).insert(RigidBody::KinematicPositionBased);
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
        if let Some(col) =
            Collider::from_bevy_mesh(mesh, &ComputedColliderShape::TriMesh(default()))
        {
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

/// Story-603 — reset l'état parcours au démarrage d'un run (sortie du Lobby).
/// Sans ça, un run abandonné DEPUIS le parcours (`in_room=true`) gèlerait le
/// combat du run suivant (`combat_running()` → false, aucune wave) et laisserait
/// le joueur rétréci (`ShrinkBuff`). La porte (`BossGate`) se ré-aligne seule via
/// `sys_reconcile_boss_gate` (boss_defeated reset → branche fermeture).
fn sys_reset_parcours_on_run_start(
    mut state: ResMut<LootRoomState>,
    mut shrink: ResMut<ShrinkBuff>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    *state = LootRoomState::default();
    shrink.active = false;
    // Le handle a été relâché au retour lobby pour rendre le gros niveau
    // évictable. On relance l'I/O asynchrone dès le départ de la run afin que le
    // parcours soit déjà prêt quand le boss est vaincu, sans charge synchrone au
    // passage de porte.
    let scene = asset_server.load(GltfAssetLabel::Scene(0).from_asset(KIT_PATH));
    commands.insert_resource(DemoLevelScene(scene));
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
            info!(
                "[loot-room] checkpoint atteint ({:.0},{:.0})",
                cp.pos.x, cp.pos.z
            );
        }
    }
}

fn sys_spin_portals(time: Res<Time>, mut q: Query<&mut Transform, With<PortalSpin>>) {
    let dt = time.delta_secs();
    for mut tf in &mut q {
        tf.rotate_local_z(dt * 0.8);
    }
}

/// Contexte du tirage de boon du diamant (story-662) — regroupé en `SystemParam`
/// pour rester ≤ 12 params sur `sys_collect_level_items` (règle scalability).
#[derive(bevy::ecs::system::SystemParam)]
struct BoonRollCtx<'w> {
    active: ResMut<'w, ActiveBoons>,
    catalogue: Option<Res<'w, BoonsCatalogue>>,
    tiers: Res<'w, UnlockedBoonTiers>,
    rng: ResMut<'w, CoffreRng>,
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
    mut boons: BoonRollCtx,
    mut popups: ResMut<KillPopupState>,
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
                // Fix audit 2026-07-19 — l'ancien tirage était un index CYCLIQUE
                // non seedé sur TOUT le catalogue : gating des paliers (story-616)
                // et gate légendaire 3-tags ignorés → un compte neuf pouvait
                // toucher un légendaire gratuit. Tirage canonique du Coffre :
                // pondéré par rareté + paliers méta + légendaires, seedé CoffreRng.
                // (Le diamant reste GRATUIT — lui donner un coût = décision de
                // balance, cf audit éco §7 reco P0-4, non tranchée.)
                if let Some(cat) = boons.catalogue.as_deref() {
                    let mut idx_fn = rng_next_index(&mut boons.rng.0);
                    let picked =
                        roll_candidates_weighted(cat, &boons.active, &boons.tiers, 1, &mut idx_fn);
                    if let Some(def) = picked
                        .first()
                        .and_then(|id| cat.entries.iter().find(|b| &b.id == id))
                        .cloned()
                    {
                        boons.active.apply(&def, cat);
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
        info!(
            "[loot-room] item ramassé : {text} (pos {:.0},{:.0})",
            ipos.x, ipos.z
        );
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
    // Story-603 close (2026-07-02) — le portail Return scelle la run : émet la
    // Victoire (jusqu'ici l'event n'était JAMAIS émis → l'écran Victory était mort).
    mut end_run: MessageWriter<crate::run::EndRunEvent>,
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
                // Story-603 close — boss vaincu + parcours bouclé = run GAGNÉE.
                // `sys_end_run` transitionne RunState::Victory (overlay + flush save
                // méta + XP). Garde boss_defeated : on n'atteint le parcours que
                // post-boss, mais un futur accès dev/debug ne doit pas gagner la run.
                if wave.boss_defeated {
                    end_run.write(crate::run::EndRunEvent {
                        result: crate::run::RunResult::Victory,
                    });
                    info!("[loot-room] → retour arène — VICTOIRE (run scellée)");
                } else {
                    info!("[loot-room] → retour arène");
                }
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
    tiers: Res<UnlockedBoonTiers>,
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
        // R3.4 (story-645) — tirage pondéré par rareté (miroir du Coffre).
        .map(|cat| roll_candidates_weighted(cat, &active, &tiers, 3, &mut rng_next_index(&mut rng.0)))
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
                    egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
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
