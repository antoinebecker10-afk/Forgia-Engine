//! Portail → NIVEAU PARCOURS « Underworld » habillé du kit ithappy (2026-06-06).
//! Un portail dans l'arène téléporte le joueur vers un niveau de plateforme
//! complet en 6 sections (plaza d'entrée → sauts → checkpoint → plateformes
//! mobiles → précision → pont étroit → sommet trésor), à un offset lointain.
//! Tomber = retour immédiat à l'arène (mort). Récompenses : pièces (Or), âmes,
//! et au sommet 4 ORBES D'UPGRADE D'ARME (boons via `ActiveBoons`) + un coffre,
//! pour repartir plus fort tuer le boss. En entrant on retire `BotTarget`.
//!
//! Visuel : meshes ithappy référencés par INDEX depuis le GLB entier
//! (`gltf.meshes[i]`), build DIFFÉRÉ (attend le chargement async). Colliders
//! ConvexHull sur les pièces marchables. La déco est POSÉE AU SOL via l'offset
//! de pivot natif (`Y0_*`) — sinon gate/passage/pillar (pivot sous la base)
//! s'enfoncent dans le sol. Placement symétrique. Zéro touche forgia-stage.

use crate::kill_popup::{KillPopup, KillPopupState};
use bevy::gltf::{Gltf, GltfMesh};
use bevy::prelude::*;
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
const PORTAL_RADIUS: f32 = 2.6;
const TELEPORT_COOLDOWN: f32 = 1.2;
const FALL_DEATH_Y: f32 = -0.5;
const KIT_PATH: &str = "models/environment/platformer/platformer_underworld.glb";

// Index de mesh ithappy (cf inventaire GLB).
const MI_PLATFORM: usize = 2;
const MI_CIRCLE: usize = 9;
const MI_ROAD: usize = 45;
const MI_PILLAR: usize = 0;
const MI_COLUMN: usize = 76;
const MI_FIRE: usize = 10;
const MI_PASSAGE: usize = 6;
const MI_GATE: usize = 32;
const MI_STATUE_KNIGHT: usize = 75;
const MI_STATUE_WARLOCK: usize = 63;
const MI_CHEST: usize = 53;
const MI_CHEST_GOLD: usize = 51;
const MI_COIN: usize = 58;
const MI_DIAMOND: usize = 56;
const MI_CROWN: usize = 57;
const MI_HEART: usize = 55;

// Offset de pivot natif (min Y) : négatif = pivot sous la base → la déco doit
// être remontée de `-y0*scale` pour que sa base touche le sol.
const Y0_GATE: f32 = -16.9;
const Y0_PASSAGE: f32 = -7.7;
const Y0_PILLAR: f32 = -16.4;
const Y0_FIRE: f32 = -1.6;
const Y0_BASE: f32 = 0.0; // column / knight / warlock / chest (pivot base)
const Y0_CHEST_GOLD: f32 = 0.4;

const PLAT_TOP: f32 = 4.86;
const PLAT_HALF: f32 = 5.0;
const PLAT_SCALE: f32 = 0.35;

#[derive(Component)]
struct LootRoomMarker;
#[derive(Component)]
struct ArenaPortal;
#[derive(Component)]
struct ReturnPortal;
#[derive(Component)]
struct PortalSpin;
#[derive(Component)]
struct MovingPlatform {
    origin: Vec3,
    axis: Vec3,
    amplitude: f32,
    freq: f32,
    phase: f32,
    half: Vec2,
    top_off: f32,
}
#[derive(Component)]
struct WeaponUpgradePickup {
    boon_id: BoonId,
    popup_text: &'static str,
    popup_color: egui::Color32,
}

#[derive(Clone)]
struct Piece {
    mesh: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub struct LootRoomState {
    pub in_room: bool,
    pub return_pos: Vec3,
    cooldown: f32,
}

#[derive(Resource)]
struct KitHandle(Handle<Gltf>);

#[derive(Resource, Default)]
struct ParkourBuilt(bool);

pub struct RogueliteLootRoomPlugin;

impl Plugin for RogueliteLootRoomPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LootRoomState>()
            .init_resource::<ParkourBuilt>()
            .add_systems(OnEnter(GameMode::Roguelite), sys_load_kit_and_portal)
            .add_systems(
                Update,
                (
                    sys_build_parkour,
                    sys_portal_walkover,
                    sys_spin_portals,
                    sys_move_platforms,
                    sys_collect_upgrades,
                )
                    .in_set(GameSet::Movement)
                    .run_if(in_state(GameMode::Roguelite)),
            );
    }
}

fn dist_xz(a: Vec3, b: Vec3) -> f32 {
    Vec2::new(a.x - b.x, a.z - b.z).length()
}

fn sys_load_kit_and_portal(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut built: ResMut<ParkourBuilt>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    built.0 = false;
    let kit: Handle<Gltf> = asset_server.load(KIT_PATH);
    commands.insert_resource(KitHandle(kit));

    let arena = spawn_portal_visual(
        &mut commands,
        &mut meshes,
        &mut materials,
        ARENA_PORTAL_POS,
        LinearRgba::new(0.3, 2.6, 3.2, 1.0),
    );
    commands.entity(arena).insert(ArenaPortal);
    info!("[loot-room] kit ithappy en chargement, portail d'arène spawné");
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn sys_build_parkour(
    mut commands: Commands,
    kit: Option<Res<KitHandle>>,
    gltfs: Res<Assets<Gltf>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut built: ResMut<ParkourBuilt>,
) {
    if built.0 {
        return;
    }
    let Some(kit) = kit else {
        return;
    };
    let Some(gltf) = gltfs.get(&kit.0) else {
        return;
    };
    let resolve = |mi: usize| -> Option<Piece> {
        let gm = gltf_meshes.get(gltf.meshes.get(mi)?)?;
        let prim = gm.primitives.first()?;
        Some(Piece {
            mesh: prim.mesh.clone(),
            mat: prim.material.clone()?,
        })
    };
    let (Some(platform), Some(circle), Some(road)) =
        (resolve(MI_PLATFORM), resolve(MI_CIRCLE), resolve(MI_ROAD))
    else {
        return;
    };
    if meshes.get(&platform.mesh).is_none()
        || meshes.get(&circle.mesh).is_none()
        || meshes.get(&road.mesh).is_none()
    {
        return;
    }
    let pillar = resolve(MI_PILLAR);
    let column = resolve(MI_COLUMN);
    let fire = resolve(MI_FIRE);
    let passage = resolve(MI_PASSAGE);
    let gate = resolve(MI_GATE);
    let knight = resolve(MI_STATUE_KNIGHT);
    let warlock = resolve(MI_STATUE_WARLOCK);
    let chest = resolve(MI_CHEST);
    let chest_gold = resolve(MI_CHEST_GOLD);
    let coin = resolve(MI_COIN);
    let diamond = resolve(MI_DIAMOND);
    let crown = resolve(MI_CROWN);
    let heart = resolve(MI_HEART);

    let o = ROOM_ORIGIN;
    let pscale = PLAT_SCALE;

    // ── S0 — Plaza d'entrée (large disque pour que la déco repose dessus) ──
    let plaza_scale = 1.6;
    walk(&mut commands, &meshes, &circle, o, plaza_scale, 0.68, 4.56, None);
    let pg = o + Vec3::Y * (0.68 * plaza_scale); // sol de la plaza
    deco(&mut commands, &gate, pg + Vec3::new(0.0, 0.0, 5.5), Y0_GATE, 0.26, 0.0);
    deco(&mut commands, &pillar, pg + Vec3::new(-5.0, 0.0, 5.0), Y0_PILLAR, 0.09, 0.0);
    deco(&mut commands, &pillar, pg + Vec3::new(5.0, 0.0, 5.0), Y0_PILLAR, 0.09, 0.0);
    deco(&mut commands, &warlock, pg + Vec3::new(-4.5, 0.0, 2.5), Y0_BASE, 0.4, 0.6);
    deco(&mut commands, &warlock, pg + Vec3::new(4.5, 0.0, 2.5), Y0_BASE, 0.4, -0.6);
    deco(&mut commands, &fire, pg + Vec3::new(-3.0, 0.0, -2.5), Y0_FIRE, 0.4, 0.0);
    deco(&mut commands, &fire, pg + Vec3::new(3.0, 0.0, -2.5), Y0_FIRE, 0.4, 0.0);

    // ── S1 — Sauts faciles (plateformes propres, pas de déco dessus) ──────
    for top_rel in [
        Vec3::new(-2.0, 1.4, -7.0),
        Vec3::new(2.0, 2.0, -12.0),
        Vec3::new(-2.0, 2.6, -17.0),
    ] {
        let top = o + top_rel;
        walk(&mut commands, &meshes, &platform, base_of(top, pscale, PLAT_TOP), pscale, PLAT_TOP, PLAT_HALF, None);
    }

    // ── S2 — Checkpoint + arche centrée (passe dessous) ───────────────────
    let l1_top = o + Vec3::new(0.0, 3.2, -23.0);
    walk(&mut commands, &meshes, &circle, base_of(l1_top, 0.8, 0.68), 0.8, 0.68, 4.56, None);
    deco(&mut commands, &passage, l1_top, Y0_PASSAGE, 0.24, 0.0);
    deco(&mut commands, &fire, l1_top + Vec3::new(-2.3, 0.0, 0.0), Y0_FIRE, 0.35, 0.0);
    deco(&mut commands, &fire, l1_top + Vec3::new(2.3, 0.0, 0.0), Y0_FIRE, 0.35, 0.0);

    // ── S3 — Plateformes mobiles ──────────────────────────────────────────
    let m1 = o + Vec3::new(0.0, 3.8, -29.0);
    let m2 = o + Vec3::new(0.0, 4.4, -35.0);
    walk(&mut commands, &meshes, &platform, base_of(m1, pscale, PLAT_TOP), pscale, PLAT_TOP, PLAT_HALF, Some((2.5, 0.8, 0.0)));
    walk(&mut commands, &meshes, &platform, base_of(m2, pscale, PLAT_TOP), pscale, PLAT_TOP, PLAT_HALF, Some((2.5, 0.8, std::f32::consts::PI)));

    // ── S4 — Sauts de précision (petites plateformes propres) ─────────────
    let ps = 0.22;
    for top_rel in [Vec3::new(-2.2, 5.0, -40.0), Vec3::new(2.2, 5.6, -44.5)] {
        let top = o + top_rel;
        walk(&mut commands, &meshes, &platform, base_of(top, ps, PLAT_TOP), ps, PLAT_TOP, PLAT_HALF, None);
    }

    // ── S5 — Pont étroit + arche d'entrée ─────────────────────────────────
    let bridge_top = o + Vec3::new(0.0, 6.0, -51.0);
    walk(&mut commands, &meshes, &road, base_of(bridge_top, 0.34, 0.7), 0.34, 0.7, 2.15, None);
    deco(&mut commands, &passage, bridge_top + Vec3::new(0.0, 0.0, 4.5), Y0_PASSAGE, 0.22, 0.0);

    // ── S6 — Sommet trésor (déco symétrique, posée au sol) ────────────────
    let top_pos = o + Vec3::new(0.0, 6.4, -58.0);
    let tscale = 0.75;
    walk(&mut commands, &meshes, &platform, base_of(top_pos, tscale, PLAT_TOP), tscale, PLAT_TOP, PLAT_HALF, None);
    deco(&mut commands, &knight, top_pos + Vec3::new(-3.2, 0.0, -2.0), Y0_BASE, 0.4, 0.5);
    deco(&mut commands, &knight, top_pos + Vec3::new(3.2, 0.0, -2.0), Y0_BASE, 0.4, -0.5);
    deco(&mut commands, &column, top_pos + Vec3::new(-2.6, 0.0, -2.6), Y0_BASE, 0.13, 0.0);
    deco(&mut commands, &column, top_pos + Vec3::new(2.6, 0.0, -2.6), Y0_BASE, 0.13, 0.0);
    deco(&mut commands, &fire, top_pos + Vec3::new(-2.8, 0.0, 1.5), Y0_FIRE, 0.45, 0.0);
    deco(&mut commands, &fire, top_pos + Vec3::new(2.8, 0.0, 1.5), Y0_FIRE, 0.45, 0.0);
    deco(&mut commands, &chest, top_pos + Vec3::new(0.0, 0.0, -2.6), Y0_BASE, 2.2, 0.0);
    deco(&mut commands, &chest_gold, top_pos + Vec3::new(0.0, 0.0, -2.6), Y0_CHEST_GOLD, 2.2, 0.0);
    // Gemmes tournoyantes au-dessus du coffre (flottantes par design).
    spin_opt(&mut commands, &crown, top_pos + Vec3::new(0.0, 1.8, -2.6), 1.6);
    spin_opt(&mut commands, &diamond, top_pos + Vec3::new(-1.4, 1.4, -2.6), 1.6);
    spin_opt(&mut commands, &diamond, top_pos + Vec3::new(1.4, 1.4, -2.6), 1.6);
    spin_opt(&mut commands, &heart, top_pos + Vec3::new(0.0, 1.4, 2.5), 1.6);

    // ── Loot fonctionnel : pièces d'Or (ithappy coin) + âmes ──────────────
    if let Some(c) = &coin {
        for top_rel in [
            Vec3::new(-2.0, 2.0, -7.0),
            Vec3::new(2.0, 2.6, -12.0),
            Vec3::new(-2.0, 3.2, -17.0),
            Vec3::new(0.0, 3.9, -23.0),
            Vec3::new(-2.2, 5.7, -40.0),
            Vec3::new(2.2, 6.3, -44.5),
        ] {
            spawn_coin(&mut commands, c, o + top_rel, 14);
        }
        for (dx, dz) in [(-2.0, -1.0), (2.0, -1.0), (-2.0, 1.0), (2.0, 1.0)] {
            spawn_coin(&mut commands, c, top_pos + Vec3::new(dx, 1.0, dz), 26);
        }
    }
    spawn_soul(&mut commands, &mut meshes, &mut materials, top_pos + Vec3::new(-1.0, 1.5, 0.5), 5);
    spawn_soul(&mut commands, &mut meshes, &mut materials, top_pos + Vec3::new(1.0, 1.5, 0.5), 5);
    spawn_soul(&mut commands, &mut meshes, &mut materials, l1_top + Vec3::new(0.0, 1.2, 0.0), 5);

    // ── 4 ORBES D'UPGRADE D'ARME (récompense premium du sommet) ───────────
    let orbs = [
        (Vec3::new(-2.4, 1.3, 1.2), LinearRgba::new(4.0, 0.4, 0.15, 1.0), "metal_chaud", "+15% DÉGÂTS", egui::Color32::from_rgb(231, 76, 60)),
        (Vec3::new(-0.8, 1.3, 1.8), LinearRgba::new(0.3, 1.4, 4.0, 1.0), "souffle_du_maitre", "+20% CADENCE", egui::Color32::from_rgb(80, 160, 255)),
        (Vec3::new(0.8, 1.3, 1.8), LinearRgba::new(0.3, 4.0, 0.6, 1.0), "eclat_ame_nourrissant", "SOIN AU KILL", egui::Color32::from_rgb(80, 220, 120)),
        (Vec3::new(2.4, 1.3, 1.2), LinearRgba::new(3.5, 2.6, 0.4, 1.0), "benediction_enclume", "-10% DÉGÂTS REÇUS", egui::Color32::from_rgb(244, 196, 48)),
    ];
    for (off, col, id, txt, pcol) in orbs {
        spawn_upgrade_orb(&mut commands, &mut meshes, &mut materials, top_pos + off, col, id, txt, pcol);
    }

    // Portail retour (orange), sur la plaza.
    let ret = spawn_portal_visual(
        &mut commands,
        &mut meshes,
        &mut materials,
        pg + Vec3::new(0.0, 1.4, 3.5),
        LinearRgba::new(3.2, 1.4, 0.3, 1.0),
    );
    commands.entity(ret).insert(ReturnPortal);

    info!("[loot-room] niveau ithappy complet bâti (offset {o:?})");
    built.0 = true;
}

/// Base (pivot) d'une pièce marchable pour que son dessus soit à `top`.
fn base_of(top: Vec3, scale: f32, native_top: f32) -> Vec3 {
    top - Vec3::Y * (native_top * scale)
}

/// Pièce marchable = mesh + ConvexHull. Statique (Fixed) ou mobile (Kinematic).
#[allow(clippy::too_many_arguments)]
fn walk(
    commands: &mut Commands,
    meshes: &Assets<Mesh>,
    piece: &Piece,
    base_pos: Vec3,
    scale: f32,
    native_top: f32,
    native_half: f32,
    moving: Option<(f32, f32, f32)>,
) {
    let Some(collider) = meshes
        .get(&piece.mesh)
        .and_then(|m| Collider::from_bevy_mesh(m, &ComputedColliderShape::ConvexHull))
    else {
        warn!("[loot-room] ConvexHull KO");
        return;
    };
    let mut e = commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        Name::new("KitPlatform"),
        Mesh3d(piece.mesh.clone()),
        MeshMaterial3d(piece.mat.clone()),
        Transform::from_translation(base_pos).with_scale(Vec3::splat(scale)),
        collider,
    ));
    match moving {
        Some((amp, freq, phase)) => {
            e.insert((
                RigidBody::KinematicPositionBased,
                MovingPlatform {
                    origin: base_pos,
                    axis: Vec3::X,
                    amplitude: amp,
                    freq,
                    phase,
                    half: Vec2::splat(native_half * scale),
                    top_off: native_top * scale,
                },
            ));
        }
        None => {
            e.insert(RigidBody::Fixed);
        }
    }
}

/// Déco cosmétique POSÉE AU SOL. `ground` = point au sol visé ; `y0` = min Y natif
/// du mesh (négatif si pivot sous la base) → la base repose exactement sur `ground`.
fn deco(commands: &mut Commands, piece: &Option<Piece>, ground: Vec3, y0: f32, scale: f32, yaw: f32) {
    let Some(p) = piece else {
        return;
    };
    let pos = Vec3::new(ground.x, ground.y - y0 * scale, ground.z);
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        Name::new("KitDecor"),
        Mesh3d(p.mesh.clone()),
        MeshMaterial3d(p.mat.clone()),
        Transform::from_translation(pos)
            .with_rotation(Quat::from_rotation_y(yaw))
            .with_scale(Vec3::splat(scale)),
    ));
}

/// Gemme décorative tournoyante (emissive, pas de collider, pas de pickup).
fn spin_opt(commands: &mut Commands, piece: &Option<Piece>, pos: Vec3, scale: f32) {
    let Some(p) = piece else {
        return;
    };
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        PortalSpin,
        Name::new("KitGem"),
        Mesh3d(p.mesh.clone()),
        MeshMaterial3d(p.mat.clone()),
        Transform::from_translation(pos).with_scale(Vec3::splat(scale)),
    ));
}

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

fn spawn_coin(commands: &mut Commands, coin: &Piece, pos: Vec3, value: u32) {
    commands.spawn((
        LootRoomMarker,
        DespawnOnExit(GameMode::Roguelite),
        Name::new("LootRoom_Coin"),
        Pickup {
            value,
            lifetime_secs: 1.0e9,
            collect_radius: 2.5,
        },
        Mesh3d(coin.mesh.clone()),
        MeshMaterial3d(coin.mat.clone()),
        Transform::from_translation(pos).with_scale(Vec3::splat(2.0)),
        crate::CoinSpin,
    ));
}

fn spawn_portal_visual(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    pos: Vec3,
    emissive: LinearRgba,
) -> Entity {
    let ring = meshes.add(Torus {
        minor_radius: 0.28,
        major_radius: 2.0,
    });
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.15, 0.2),
        emissive,
        ..default()
    });
    commands
        .spawn((
            LootRoomMarker,
            DespawnOnExit(GameMode::Roguelite),
            PortalSpin,
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
        ))
        .id()
}

fn sys_spin_portals(time: Res<Time>, mut q: Query<&mut Transform, With<PortalSpin>>) {
    let dt = time.delta_secs();
    for mut tf in &mut q {
        tf.rotate_local_z(dt * 0.8);
    }
}

fn sys_move_platforms(
    time: Res<Time>,
    state: Res<LootRoomState>,
    mut q_plat: Query<(&mut Transform, &MovingPlatform)>,
    mut q_player: Query<&mut Transform, (With<Player>, Without<MovingPlatform>)>,
) {
    let t = time.elapsed_secs();
    let mut player = q_player.single_mut().ok();
    for (mut tf, plat) in &mut q_plat {
        let offset = plat.amplitude * (t * plat.freq + plat.phase).sin();
        let new_pos = plat.origin + plat.axis * offset;
        let delta = new_pos - tf.translation;
        tf.translation = new_pos;
        if state.in_room {
            if let Some(ptf) = player.as_deref_mut() {
                let top = new_pos.y + plat.top_off;
                let on_xz = (ptf.translation.x - new_pos.x).abs() <= plat.half.x + 0.5
                    && (ptf.translation.z - new_pos.z).abs() <= plat.half.y + 0.5;
                let on_y = ptf.translation.y > top + 0.1 && ptf.translation.y < top + 2.2;
                if on_xz && on_y {
                    ptf.translation += delta;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
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
        if dist_xz(ptf.translation, tf.translation) < 2.5 {
            if let Some(def) = cat.entries.iter().find(|b| b.id == up.boon_id) {
                active.apply(def, &cat);
                // Feedback HUD : popup world-space (réutilise le système kill-popup).
                popups.active.push(KillPopup {
                    world_pos: tf.translation + Vec3::Y * 1.2,
                    text: up.popup_text,
                    color: up.popup_color,
                    spawned_secs: time.elapsed_secs(),
                });
                info!("[loot-room] UPGRADE ARME collectée : {} ({})", def.name, up.popup_text);
            }
            commands.entity(e).despawn();
        }
    }
}

/// run_if : le combat (orchestrateur de vagues) tourne SEULEMENT hors du parcours.
/// En entrant dans la salle de loot on retire `BotTarget` (bots idle) ET on gèle
/// la progression de vague (le break ne s'écoule pas pendant l'absence).
pub fn combat_running(state: Res<LootRoomState>) -> bool {
    !state.in_room
}

#[allow(clippy::too_many_arguments)]
fn sys_portal_walkover(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<LootRoomState>,
    mut q_player: Query<(Entity, &mut Transform), With<Player>>,
    q_arena: Query<&Transform, (With<ArenaPortal>, Without<Player>)>,
    q_return: Query<&Transform, (With<ReturnPortal>, Without<Player>)>,
) {
    if state.cooldown > 0.0 {
        state.cooldown -= time.delta_secs();
        return;
    }
    let Ok((player, mut ptf)) = q_player.single_mut() else {
        return;
    };
    if !state.in_room {
        if let Ok(portal) = q_arena.single() {
            if dist_xz(ptf.translation, portal.translation) < PORTAL_RADIUS {
                enter_room(&mut commands, &mut state, player, &mut ptf);
            }
        }
    } else {
        if ptf.translation.y < ROOM_ORIGIN.y + FALL_DEATH_Y {
            exit_room(&mut commands, &mut state, player, &mut ptf);
            info!("[loot-room] chute → retour arène");
            return;
        }
        if let Ok(portal) = q_return.single() {
            if dist_xz(ptf.translation, portal.translation) < PORTAL_RADIUS {
                exit_room(&mut commands, &mut state, player, &mut ptf);
            }
        }
    }
}

fn enter_room(commands: &mut Commands, state: &mut LootRoomState, player: Entity, ptf: &mut Transform) {
    state.return_pos = ptf.translation;
    ptf.translation = ROOM_ORIGIN + Vec3::new(0.0, 2.5, 0.0);
    state.in_room = true;
    state.cooldown = TELEPORT_COOLDOWN;
    commands.entity(player).remove::<BotTarget>();
    info!("[loot-room] → niveau parcours");
}

fn exit_room(commands: &mut Commands, state: &mut LootRoomState, player: Entity, ptf: &mut Transform) {
    ptf.translation = state.return_pos + Vec3::new(0.0, 0.0, 5.0);
    state.in_room = false;
    state.cooldown = TELEPORT_COOLDOWN;
    commands.entity(player).insert(BotTarget);
}
