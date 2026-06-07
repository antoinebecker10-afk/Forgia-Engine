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
use bevy_rapier3d::prelude::{Collider, ComputedColliderShape, RigidBody};
use forgia_ai_arena_bot::BotTarget;
use forgia_core::prelude::*;
use forgia_player::Player;
use forgia_rpg_data::boons::{ActiveBoons, BoonId, BoonsCatalogue};
use forgia_rpg_data::loot_tables::Pickup;

const ROOM_ORIGIN: Vec3 = Vec3::new(2000.0, 0.0, 2000.0);
const ARENA_PORTAL_POS: Vec3 = Vec3::new(0.0, 1.4, -34.0);
const PORTAL_RADIUS: f32 = 2.8;
const TELEPORT_COOLDOWN: f32 = 1.2;
/// Sous ce Y → chute → retour au pad de la zone courante (checkpoint).
const FALL_DEATH_Y: f32 = -0.5;
const KIT_PATH: &str = "models/environment/platformer/platformer_underworld.glb";

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
#[derive(Component)]
struct WeaponUpgradePickup {
    boon_id: BoonId,
    popup_text: &'static str,
    popup_color: egui::Color32,
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

pub struct RogueliteLootRoomPlugin;

impl Plugin for RogueliteLootRoomPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LootRoomState>()
            .add_systems(OnEnter(GameMode::Roguelite), sys_setup)
            .add_systems(
                Update,
                (
                    sys_portal_walkover,
                    sys_spin_portals,
                    sys_collect_upgrades,
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

/// Définition d'un orbe d'upgrade (couleur, boon, libellé HUD).
struct OrbDef {
    emissive: LinearRgba,
    boon: &'static str,
    text: &'static str,
    color: egui::Color32,
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

    // ── Spawn pad posé SUR une vraie plateforme de départ de chaque zone ────────
    // (au DÉBUT du parcours, sur la géométrie du niveau démo — plus dessous).
    let pad_spawns: [Vec3; 3] = std::array::from_fn(|z| {
        let n = ZONE_SPAWN_NATIVE[z];
        let pad_center = Vec3::new(
            level_t.x + s * n.x,
            level_t.y + s * n.y + 3.5, // ≈ dessus de la plateforme démo (top natif +4.86)
            level_t.z + s * n.z,
        );
        spawn_pad(&mut commands, &mut meshes, &mut materials, pad_center, Vec3::new(11.0, 1.0, 11.0));
        pad_center + Vec3::new(0.0, 2.0, 0.0)
    });

    // ── Orbes d'upgrade répartis (2 zone1, 1 zone2, 1 zone3) ────────────────────
    let orbs: [OrbDef; 4] = [
        OrbDef { emissive: LinearRgba::new(4.0, 0.4, 0.15, 1.0), boon: "metal_chaud", text: "+15% DÉGÂTS", color: egui::Color32::from_rgb(231, 76, 60) },
        OrbDef { emissive: LinearRgba::new(0.3, 1.4, 4.0, 1.0), boon: "souffle_du_maitre", text: "+20% CADENCE", color: egui::Color32::from_rgb(80, 160, 255) },
        OrbDef { emissive: LinearRgba::new(0.3, 4.0, 0.6, 1.0), boon: "eclat_ame_nourrissant", text: "SOIN AU KILL", color: egui::Color32::from_rgb(80, 220, 120) },
        OrbDef { emissive: LinearRgba::new(3.5, 2.6, 0.4, 1.0), boon: "benediction_enclume", text: "-10% DÉGÂTS REÇUS", color: egui::Color32::from_rgb(244, 196, 48) },
    ];
    let orb_zone = [0usize, 0, 1, 2]; // quelle zone pour chaque orbe
    let mut placed = [0u32; 3];
    for (i, orb) in orbs.iter().enumerate() {
        let z = orb_zone[i];
        let base = pad_spawns[z] - Vec3::Y * 1.0; // top du pad
        let off = Vec3::new(-2.0 + 2.0 * placed[z] as f32, 1.3, -2.0);
        placed[z] += 1;
        spawn_upgrade_orb(&mut commands, &mut meshes, &mut materials, base + off, orb.emissive, orb.boon, orb.text, orb.color);
    }

    // ── Loot (pièces + âmes) sur chaque pad ─────────────────────────────────────
    for spawn in pad_spawns {
        let base = spawn - Vec3::Y * 1.0;
        for off in [Vec3::new(-4.0, 1.0, 2.0), Vec3::new(4.0, 1.0, 2.0)] {
            spawn_coin(&mut commands, &mut meshes, &mut materials, base + off, 16);
        }
        spawn_soul(&mut commands, &mut meshes, &mut materials, base + Vec3::new(0.0, 1.4, 3.5), 5);
    }

    // ── Portails : arène → z1, z1 → z2, z2 → z3, z3 → arène ──────────────────────
    spawn_portal(&mut commands, &mut meshes, &mut materials, ARENA_PORTAL_POS, LinearRgba::new(0.3, 2.6, 3.2, 1.0), PortalKind::Enter, pad_spawns[0]);
    // Portails "suivant" sur les pads (devant le spawn).
    spawn_portal(&mut commands, &mut meshes, &mut materials, pad_spawns[0] + Vec3::new(0.0, -1.0 + 1.4, 5.5), LinearRgba::new(0.6, 3.0, 0.4, 1.0), PortalKind::Next, pad_spawns[1]);
    spawn_portal(&mut commands, &mut meshes, &mut materials, pad_spawns[1] + Vec3::new(0.0, -1.0 + 1.4, 5.5), LinearRgba::new(0.6, 3.0, 0.4, 1.0), PortalKind::Next, pad_spawns[2]);
    spawn_portal(&mut commands, &mut meshes, &mut materials, pad_spawns[2] + Vec3::new(0.0, -1.0 + 1.4, 5.5), LinearRgba::new(3.2, 1.4, 0.3, 1.0), PortalKind::Return, Vec3::ZERO);

    info!("[loot-room] mode parcours 3 zones spawné (offset {ROOM_ORIGIN:?})");
}

/// Plateforme d'arrivée solide (box pierre).
fn spawn_pad(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    center: Vec3,
    size: Vec3,
) {
    let mesh = meshes.add(Cuboid::new(size.x, size.y, size.z));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.15, 0.13),
        perceptual_roughness: 0.92,
        ..default()
    });
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        Name::new("SpawnPad"),
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(center),
        RigidBody::Fixed,
        Collider::cuboid(size.x * 0.5, size.y * 0.5, size.z * 0.5),
    ));
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
        let mut stack: Vec<Entity> = top.iter().collect();
        let mut count = 0u32;
        let mut spinners = 0u32;
        while let Some(e) = stack.pop() {
            if q_mesh.get(e).is_ok() {
                commands.entity(e).insert(NeedsLevelCollider);
                count += 1;
            }
            // Spinners de parcours : barres obstacle_3 / obstacle_6 (pivot centré) →
            // on tag le node (sa rotation entraîne mesh + collider enfants).
            if let Ok(name) = q_name.get(e) {
                let s = name.as_str();
                if s.starts_with("obstacle_3") || s.starts_with("obstacle_6") {
                    commands.entity(e).insert(RotatingObstacle);
                    spinners += 1;
                }
            }
            if let Ok(ch) = q_children.get(e) {
                stack.extend(ch.iter());
            }
        }
        commands.entity(root).remove::<DemoUnmarked>();
        info!("[loot-room] niveau démo : {count} meshes → collider ; {spinners} spinners");
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

#[allow(clippy::too_many_arguments)]
fn spawn_upgrade_orb(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    emissive: LinearRgba,
    boon_id: &str,
    popup_text: &'static str,
    popup_color: egui::Color32,
) {
    let mesh = meshes.add(Sphere::new(0.45));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.1, 0.12),
        emissive,
        ..default()
    });
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        PortalSpin,
        Name::new("WeaponUpgradeOrb"),
        WeaponUpgradePickup {
            boon_id: BoonId(boon_id.to_string()),
            popup_text,
            popup_color,
        },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(pos),
        children![(
            PointLight {
                color: Color::srgb(emissive.red, emissive.green, emissive.blue),
                intensity: 2_500.0,
                range: 10.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::default(),
        )],
    ));
}

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

fn sys_spin_portals(time: Res<Time>, mut q: Query<&mut Transform, With<PortalSpin>>) {
    let dt = time.delta_secs();
    for mut tf in &mut q {
        tf.rotate_local_z(dt * 0.8);
    }
}

fn sys_collect_upgrades(
    mut commands: Commands,
    time: Res<Time>,
    state: Res<LootRoomState>,
    q_player: Query<&Transform, With<Player>>,
    q_up: Query<(Entity, &Transform, &WeaponUpgradePickup)>,
    mut active: ResMut<ActiveBoons>,
    mut popups: ResMut<KillPopupState>,
    catalogue: Option<Res<BoonsCatalogue>>,
) {
    if !state.in_room {
        return;
    }
    let Ok(ptf) = q_player.single() else {
        return;
    };
    let Some(cat) = catalogue else {
        return;
    };
    for (e, tf, up) in &q_up {
        if dist_xz(ptf.translation, tf.translation) < 4.0 {
            if let Some(def) = cat.entries.iter().find(|b| b.id == up.boon_id) {
                active.apply(def, &cat);
                popups.active.push(KillPopup {
                    world_pos: tf.translation + Vec3::Y * 1.2,
                    text: up.popup_text,
                    color: up.popup_color,
                    spawned_secs: time.elapsed_secs(),
                });
                info!("[loot-room] UPGRADE ARME : {} ({})", def.name, up.popup_text);
            }
            commands.entity(e).despawn();
        }
    }
}

fn sys_portal_walkover(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<LootRoomState>,
    mut q_player: Query<(Entity, &mut Transform), With<Player>>,
    q_portals: Query<(&Transform, &Portal), Without<Player>>,
) {
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
                ptf.translation = portal.target;
                state.current_pad = portal.target;
                info!("[loot-room] → zone suivante");
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
