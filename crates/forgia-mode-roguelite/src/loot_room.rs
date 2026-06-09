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

use crate::kill_popup::{KillPopup, KillPopupState};
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use bevy::scene::SceneRoot;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_egui::egui;
use bevy_rapier3d::prelude::{Collider, ComputedColliderShape};
use forgia_ai_arena_bot::BotTarget;
use forgia_core::prelude::*;
use forgia_damage::Health;
use forgia_player::Player;
use forgia_rpg_data::boons::{
    rng_next_index, roll_candidates, ActiveBoons, BoonId, BoonsCatalogue, CoffreRng,
};
use forgia_rpg_data::loot_tables::Pickup;

const ROOM_ORIGIN: Vec3 = Vec3::new(2000.0, 0.0, 2000.0);
const ARENA_PORTAL_POS: Vec3 = Vec3::new(0.0, 1.4, -34.0);
const PORTAL_RADIUS: f32 = 2.8;
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
/// Spinner de parcours : pièce qui tourne sur son axe Y (obstacle_3/_6 du démo).
#[derive(Component)]
struct RotatingObstacle;

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

/// Phase du choix de boon au portail de fin de zone (agency Hadès, story-585).
#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum RewardPhase {
    #[default]
    Closed,
    NeedRoll,
    Choosing,
}

/// Choix 1-parmi-3 GRATUIT au portail `Next` : 3 boons tirés, le joueur en prend 1 (touche 1/2/3),
/// puis est téléporté vers `target`. Distinct du Coffre du Forgeron (shop, fin de wave).
#[derive(Resource, Default)]
pub(crate) struct ZoneReward {
    phase: RewardPhase,
    pub(crate) candidates: Vec<BoonId>,
    target: Vec3,
}

impl ZoneReward {
    /// True quand les 3 cartes sont affichées (le HUD lit ça).
    pub(crate) fn choosing(&self) -> bool {
        self.phase == RewardPhase::Choosing
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
                    sys_rotate_obstacles,
                )
                    .in_set(GameSet::Movement)
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

    // ── Portails : arène → z1 (entrée), puis CHAQUE zone a son portail AU BOUT du
    //    chemin (vraie traversée). Les sorties z1/z2 ouvrent le choix de boon ;
    //    z3 ramène à l'arène. Plus d'orbes auto : le seul upgrade = le choix au portail.
    spawn_portal(&mut commands, &mut meshes, &mut materials, ARENA_PORTAL_POS, LinearRgba::new(0.3, 2.6, 3.2, 1.0), PortalKind::Enter, pad_spawns[0]);
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

fn sys_mark_demo_meshes(
    mut commands: Commands,
    q_root: Query<Entity, (With<DemoLevelRoot>, With<DemoUnmarked>)>,
    q_children: Query<&Children>,
    q_mesh: Query<(), With<Mesh3d>>,
    q_name: Query<&Name>,
) {
    for root in &q_root {
        let Ok(top) = q_children.get(root) else {
            continue;
        };
        let mut stack: Vec<(Entity, Option<LevelItemKind>)> =
            top.iter().map(|e| (e, None)).collect();
        let mut count = 0u32;
        let mut spinners = 0u32;
        let mut items = 0u32;
        while let Some((e, mut item)) = stack.pop() {
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
                // Spinners de parcours : barres obstacle_3 / obstacle_6 (pivot centré).
                if let Ok(name) = q_name.get(e) {
                    let s = name.as_str();
                    if s.starts_with("obstacle_3") || s.starts_with("obstacle_6") {
                        commands.entity(e).insert(RotatingObstacle);
                        spinners += 1;
                    }
                }
            }
            if let Ok(ch) = q_children.get(e) {
                for c in ch.iter() {
                    stack.push((c, item));
                }
            }
        }
        commands.entity(root).remove::<DemoUnmarked>();
        info!("[loot-room] niveau démo : {count} meshes collider ; {spinners} spinners ; {items} items ramassables");
    }
}

/// Fait tourner les spinners de parcours sur leur axe Y (collider suit via propagation).
fn sys_rotate_obstacles(time: Res<Time>, mut q: Query<&mut Transform, With<RotatingObstacle>>) {
    let dt = time.delta_secs();
    for mut tf in &mut q {
        tf.rotate_local_y(dt * 1.0);
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
            PortalKind::Enter => !state.in_room,
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

/// Tire 3 candidats de boon quand un portail `Next` a ouvert le choix. Pool vide → TP direct.
fn sys_roll_zone_reward(
    mut reward: ResMut<ZoneReward>,
    catalogue: Option<Res<BoonsCatalogue>>,
    active: Res<ActiveBoons>,
    mut rng: ResMut<CoffreRng>,
    mut state: ResMut<LootRoomState>,
    mut q_player: Query<&mut Transform, With<Player>>,
) {
    if reward.phase != RewardPhase::NeedRoll {
        return;
    }
    let candidates = catalogue
        .as_deref()
        .map(|cat| roll_candidates(cat, &active, 3, &mut rng_next_index(&mut rng.0)))
        .unwrap_or_default();
    if candidates.is_empty() {
        // Aucun boon éligible → pas de choix, on TP directement.
        if let Ok(mut ptf) = q_player.single_mut() {
            ptf.translation = reward.target;
            state.current_pad = reward.target;
        }
        reward.phase = RewardPhase::Closed;
        state.cooldown = TELEPORT_COOLDOWN;
        return;
    }
    info!("[loot-room] choix de boon : {} candidats", candidates.len());
    reward.candidates = candidates;
    reward.phase = RewardPhase::Choosing;
}

/// Touche 1/2/3 → applique le boon choisi (gratuit) + TP vers la zone suivante + ferme le choix.
#[allow(clippy::too_many_arguments)]
fn sys_zone_reward_pick(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut reward: ResMut<ZoneReward>,
    mut state: ResMut<LootRoomState>,
    mut q_player: Query<&mut Transform, With<Player>>,
    mut active: ResMut<ActiveBoons>,
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
    let Some(id) = reward.candidates.get(pick).cloned() else {
        return; // touche hors candidats
    };
    let Ok(mut ptf) = q_player.single_mut() else {
        return;
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
    ptf.translation = reward.target;
    state.current_pad = reward.target;
    reward.phase = RewardPhase::Closed;
    reward.candidates.clear();
    state.cooldown = TELEPORT_COOLDOWN;
}
