//! # forgia-fps
//!
//! Mode FPS Arena — assets KayKit Dungeon Pack.
//!
//! Pattern V1 :
//! - KayKit walls **`WALL_Y = 0.0`** (LOCK absolu : pivot mesh au sol, pas centre)
//! - `TILE_SIZE = 4.0` (KayKit dungeon convention)
//! - Forgia scaled scene pattern : parent scale=1 + child SceneRoot scale (rapier3d 0.33 quirk)

use bevy::input::mouse::MouseButtonInput;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_combat::prelude::*;
use forgia_combat::weapons::WeaponType;
use forgia_core::prelude::*;
use forgia_effects::prelude::{
    spawn_hitscan_tracer, spawn_impact_vfx, spawn_muzzle_flash, TracerResources,
    WeaponVfxEffects,
};
use forgia_player::prelude::*;

pub mod prelude {
    pub use crate::ForgiaFpsPlugin;
}

const TILE_SIZE: f32 = 4.0;
const ARENA_SIZE: i32 = 11; // 11×11 tiles = 44×44m (vrai map FPS multi-zones)

// ── Sky / cloud constants (cosmétiques cartoon dusk forge) ───────────
// Futur : à porter en `config/genomes/sky_arena.toml` quand FpsTuning V2 ready.
const SKY_DOME_RADIUS: f32 = 250.0;
const SKY_DOME_COLOR: Color = Color::srgb(1.0, 0.55, 0.35); // warm dusk orange
const SKY_DOME_EMISSIVE: LinearRgba = LinearRgba::new(0.6, 0.30, 0.18, 1.0);
const SUN_DISK_RADIUS: f32 = 8.0;
const SUN_DISK_POS: Vec3 = Vec3::new(80.0, 100.0, -40.0);
const SUN_DISK_COLOR: Color = Color::srgb(1.0, 0.95, 0.85);
const SUN_DISK_EMISSIVE: LinearRgba = LinearRgba::new(15.0, 12.0, 8.0, 1.0);
const CLOUD_COLOR: Color = Color::srgb(0.95, 0.96, 0.97);
const CLOUD_EMISSIVE: LinearRgba = LinearRgba::new(0.05, 0.05, 0.06, 1.0);
const CLOUD_DRIFT_SPEED: f32 = 0.8; // m/s vent doux vers +X
const CLOUD_WRAP_DIST: f32 = 150.0; // wrap autour ±150m horizon

#[derive(Component)]
pub struct ArenaMarker;

/// Marker pour les cubes-cibles (testables via fire_weapon_minimal).
#[derive(Component)]
pub struct TargetCube;

/// Marker pour les cluster nuages — animation drift wind via cloud_drift_system.
#[derive(Component)]
pub struct CloudDrift;

pub struct ForgiaFpsPlugin;

impl Plugin for ForgiaFpsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameMode::Fps), spawn_arena)
            .add_systems(OnExit(GameMode::Fps), cleanup_arena)
            .add_systems(
                Update,
                (fire_weapon_minimal, despawn_dead_cubes)
                    .chain()
                    .in_set(GameSet::Combat)
                    .run_if(in_state(GameMode::Fps)),
            )
            .add_systems(
                Update,
                cloud_drift_system
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Fps)),
            );
    }
}

fn spawn_arena(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // ── Asset handles (1 load chacun, clone Arc partout) ────────────────
    let floor: Handle<Scene> = asset_server.load("models/kaykit/dungeon/floor.glb#Scene0");
    let floor_dirt: Handle<Scene> = asset_server.load("models/kaykit/dungeon/floor_dirt.glb#Scene0");
    let floor_rocks: Handle<Scene> = asset_server.load("models/kaykit/dungeon/floor_rocks.glb#Scene0");
    let wall: Handle<Scene> = asset_server.load("models/kaykit/dungeon/wall.glb#Scene0");
    let wall_arched: Handle<Scene> = asset_server.load("models/kaykit/dungeon/wall_arched.glb#Scene0");
    let wall_window: Handle<Scene> = asset_server.load("models/kaykit/dungeon/wall_window.glb#Scene0");
    let wall_broken: Handle<Scene> = asset_server.load("models/kaykit/dungeon/wall_broken.glb#Scene0");
    let column: Handle<Scene> = asset_server.load("models/kaykit/dungeon/column.glb#Scene0");
    let pillar: Handle<Scene> = asset_server.load("models/kaykit/dungeon/pillar.glb#Scene0");
    let pillar_deco: Handle<Scene> = asset_server.load("models/kaykit/dungeon/pillar_deco.glb#Scene0");
    let torch: Handle<Scene> = asset_server.load("models/kaykit/dungeon/torch.glb#Scene0");
    let torch_wall: Handle<Scene> = asset_server.load("models/kaykit/dungeon/torch_wall.glb#Scene0");
    let banner_red: Handle<Scene> = asset_server.load("models/kaykit/dungeon/banner_red.glb#Scene0");
    let banner_blue: Handle<Scene> = asset_server.load("models/kaykit/dungeon/banner_blue.glb#Scene0");
    let banner_yellow: Handle<Scene> = asset_server.load("models/kaykit/dungeon/banner_yellow.glb#Scene0");
    let chest: Handle<Scene> = asset_server.load("models/kaykit/dungeon/chest.glb#Scene0");
    let chest_gold: Handle<Scene> = asset_server.load("models/kaykit/dungeon/chest_gold.glb#Scene0");
    let crates: Handle<Scene> = asset_server.load("models/kaykit/dungeon/crates.glb#Scene0");
    let rubble: Handle<Scene> = asset_server.load("models/kaykit/dungeon/rubble.glb#Scene0");
    let table: Handle<Scene> = asset_server.load("models/kaykit/dungeon/table.glb#Scene0");
    let barrel: Handle<Scene> = asset_server.load("models/kaykit/dungeon/barrel.glb#Scene0");
    let barrels_stack: Handle<Scene> = asset_server.load("models/kaykit/dungeon/barrels_stack.glb#Scene0");

    let half = ARENA_SIZE / 2;
    let arena_extent = (ARENA_SIZE as f32 * TILE_SIZE) / 2.0;

    // ── Sol : grille 11×11 = 121 tiles, mix variations selon zone ────────
    // Centre = floor classique, périphérie = dirt/rocks pour ambiance ruines
    for x in -half..=half {
        for z in -half..=half {
            let dist = (x.abs().max(z.abs())) as i32;
            let tile_handle = if dist >= 4 {
                // Zone périphérique : mix dirt/rocks
                if (x + z).rem_euclid(3) == 0 {
                    floor_rocks.clone()
                } else {
                    floor_dirt.clone()
                }
            } else {
                floor.clone()
            };
            let pos = Vec3::new(x as f32 * TILE_SIZE, 0.0, z as f32 * TILE_SIZE);
            commands.spawn((
                ArenaMarker,
                Transform::from_translation(pos),
                Visibility::default(),
                Name::new(format!("Floor_{x}_{z}")),
                children![(SceneRoot(tile_handle), Transform::default())],
            ));
        }
    }

    // ── Sol Collider 1m épais anti-tunneling ─────────────────────────────
    commands.spawn((
        ArenaMarker,
        Transform::from_xyz(0.0, -0.5, 0.0),
        RigidBody::Fixed,
        Collider::cuboid(arena_extent + 5.0, 0.5, arena_extent + 5.0),
        Name::new("ArenaGroundCollider"),
    ));

    // ── Murs périphérie : MIX wall classique + arched (passages) + broken (ruines) + window
    let edge = (half as f32 + 0.5) * TILE_SIZE;
    for i in -half..=half {
        let offset = i as f32 * TILE_SIZE;
        // Choix du wall variant selon position (pour casser la monotonie)
        let pick_wall = |idx: i32| -> Handle<Scene> {
            match idx.rem_euclid(7) {
                0 => wall_window.clone(),
                3 => wall_broken.clone(),
                _ => wall.clone(),
            }
        };
        // Passages arched aux 4 cardinales (i==0)
        let nord = if i == 0 { wall_arched.clone() } else { pick_wall(i) };
        let sud = if i == 0 { wall_arched.clone() } else { pick_wall(i + 1) };
        let est = if i == 0 { wall_arched.clone() } else { pick_wall(i + 2) };
        let ouest = if i == 0 { wall_arched.clone() } else { pick_wall(i + 3) };

        spawn_wall(&mut commands, &nord, Vec3::new(offset, 0.0, edge), 0.0);
        spawn_wall(&mut commands, &sud, Vec3::new(offset, 0.0, -edge), std::f32::consts::PI);
        spawn_wall(&mut commands, &est, Vec3::new(edge, 0.0, offset), -std::f32::consts::FRAC_PI_2);
        spawn_wall(&mut commands, &ouest, Vec3::new(-edge, 0.0, offset), std::f32::consts::FRAC_PI_2);
    }

    // ── Centre arena : 4 piliers décorés en carré (zone tactique) ───────
    let col_d = TILE_SIZE * 2.5;
    for &(x, z) in &[(col_d, col_d), (-col_d, col_d), (col_d, -col_d), (-col_d, -col_d)] {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(x, 0.0, z),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cylinder(2.0, 0.5),
            Name::new(format!("CenterPillar_{x}_{z}")),
            children![(SceneRoot(pillar_deco.clone()), Transform::default())],
        ));
    }

    // ── 4 colonnes outer ring (cover supplémentaire) ────────────────────
    let outer_d = TILE_SIZE * 4.5;
    for &(x, z) in &[(outer_d, 0.0), (-outer_d, 0.0), (0.0, outer_d), (0.0, -outer_d)] {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(x, 0.0, z),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cylinder(2.0, 0.5),
            Name::new(format!("OuterPillar_{x}_{z}")),
            children![(SceneRoot(pillar.clone()), Transform::default())],
        ));
    }

    // ── 4 colonnes décor intermédiaires (sans collider, juste visuel) ───
    for &(x, z) in &[(col_d, 0.0), (-col_d, 0.0), (0.0, col_d), (0.0, -col_d)] {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(x, 0.0, z),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cylinder(2.0, 0.4),
            Name::new(format!("MidColumn_{x}_{z}")),
            children![(SceneRoot(column.clone()), Transform::default())],
        ));
    }

    // ── Cover tactique : crates, rubble, barrels stack, table dispersés ─
    let cover_props: &[(&str, f32, f32, Handle<Scene>, f32, f32)] = &[
        ("Crates_NE", 8.0, -8.0, crates.clone(), 0.6, 0.6),
        ("Crates_SW", -8.0, 8.0, crates.clone(), 0.6, 0.6),
        ("Rubble_N", 0.0, -14.0, rubble.clone(), 1.0, 0.4),
        ("Rubble_S", 0.0, 14.0, rubble.clone(), 1.0, 0.4),
        ("Rubble_E", 14.0, 0.0, rubble.clone(), 1.0, 0.4),
        ("Rubble_W", -14.0, 0.0, rubble.clone(), 1.0, 0.4),
        ("BarrelStack_NW", -10.0, -10.0, barrels_stack.clone(), 0.7, 0.7),
        ("BarrelStack_SE", 10.0, 10.0, barrels_stack.clone(), 0.7, 0.7),
        ("Table_E", 6.0, 4.0, table.clone(), 1.5, 0.6),
        ("Table_W", -6.0, -4.0, table.clone(), 1.5, 0.6),
        ("Barrel_1", 3.0, 12.0, barrel.clone(), 0.5, 0.4),
        ("Barrel_2", -3.0, -12.0, barrel.clone(), 0.5, 0.4),
        ("Barrel_3", 12.0, -3.0, barrel.clone(), 0.5, 0.4),
        ("Barrel_4", -12.0, 3.0, barrel.clone(), 0.5, 0.4),
    ];
    for (name, x, z, scene, half_w, half_h) in cover_props {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(*x, 0.0, *z),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cuboid(*half_w, *half_h, *half_w),
            Name::new(name.to_string()),
            children![(SceneRoot(scene.clone()), Transform::default())],
        ));
    }

    // ── Chests précieux dans coins (objectifs visuels) ──────────────────
    commands.spawn((
        ArenaMarker,
        Transform::from_xyz(16.0, 0.0, 16.0),
        Visibility::default(),
        Name::new("ChestGold_NE"),
        children![(SceneRoot(chest_gold.clone()), Transform::default())],
    ));
    commands.spawn((
        ArenaMarker,
        Transform::from_xyz(-16.0, 0.0, -16.0),
        Visibility::default(),
        Name::new("Chest_SW"),
        children![(SceneRoot(chest.clone()), Transform::default())],
    ));

    // ── Banners colorés sur murs (décor faction) ────────────────────────
    let banners: &[(&str, f32, f32, Handle<Scene>, f32)] = &[
        ("Banner_N_red", -6.0, edge - 0.3, banner_red.clone(), 0.0),
        ("Banner_N_blue", 6.0, edge - 0.3, banner_blue.clone(), 0.0),
        ("Banner_S_yellow", 0.0, -edge + 0.3, banner_yellow.clone(), std::f32::consts::PI),
        ("Banner_E_red", edge - 0.3, -6.0, banner_red.clone(), -std::f32::consts::FRAC_PI_2),
        ("Banner_W_blue", -edge + 0.3, 6.0, banner_blue.clone(), std::f32::consts::FRAC_PI_2),
    ];
    for (name, x, z, scene, yaw) in banners {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(*x, 3.0, *z).with_rotation(Quat::from_rotation_y(*yaw)),
            Visibility::default(),
            Name::new(name.to_string()),
            children![(SceneRoot(scene.clone()), Transform::default())],
        ));
    }

    // ── Torches lit (avec PointLight orange) — ambiance forge ──────────
    let torch_positions: &[(&str, f32, f32, Handle<Scene>)] = &[
        ("Torch_NE", 18.0, -18.0, torch.clone()),
        ("Torch_NW", -18.0, -18.0, torch.clone()),
        ("Torch_SE", 18.0, 18.0, torch.clone()),
        ("Torch_SW", -18.0, 18.0, torch.clone()),
        ("TorchWall_N", -10.0, edge - 0.5, torch_wall.clone()),
        ("TorchWall_S", 10.0, -edge + 0.5, torch_wall.clone()),
        ("TorchWall_E", edge - 0.5, 10.0, torch_wall.clone()),
        ("TorchWall_W", -edge + 0.5, -10.0, torch_wall.clone()),
    ];
    for (name, x, z, scene) in torch_positions {
        commands.spawn((
            ArenaMarker,
            Transform::from_xyz(*x, 0.0, *z),
            Visibility::default(),
            Name::new(name.to_string()),
            children![
                (SceneRoot(scene.clone()), Transform::default()),
                (
                    PointLight {
                        intensity: 50_000.0,
                        color: Color::srgb(1.0, 0.55, 0.2),
                        radius: 0.3,
                        range: 12.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Transform::from_xyz(0.0, 1.8, 0.0),
                )
            ],
        ));
    }

    // ── Sun DirectionalLight (pattern V1 — direction via euler pour Atmosphere) ──
    // Atmosphere Hillaire utilise la rotation du DirectionalLight comme position du soleil.
    // Quat::from_euler XYZ (-0.35, -0.78, 0) = soleil mi-haut SE (V1 pattern terrain_systems.rs:282)
    commands.spawn((
        ArenaMarker,
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88), // golden warm
            illuminance: 22_000.0, // V1 default (was 8K = trop sombre pour Atmosphere)
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.35, -0.78, 0.0)),
        Name::new("ArenaSunLight"),
    ));

    // ── Target cubes (5 maintenant, dispersés dans la map plus grande) ──
    let cube_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let cube_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.15, 0.15),
        emissive: LinearRgba::new(0.8, 0.0, 0.0, 1.0),
        ..default()
    });
    for &(x, z) in &[(-4.0, -7.0), (0.0, -7.0), (4.0, -7.0), (10.0, -14.0), (-14.0, 10.0)] {
        commands.spawn((
            ArenaMarker,
            TargetCube,
            Mesh3d(cube_mesh.clone()),
            MeshMaterial3d(cube_mat.clone()),
            Transform::from_xyz(x, 1.0, z),
            RigidBody::Fixed,
            Collider::cuboid(0.5, 0.5, 0.5),
            Health::new(100.0),
            Name::new(format!("TargetCube_{x}_{z}")),
        ));
    }

    // ── Cartoon skybox dome warm dusk (sphère inverted unlit) ───────────
    let sky_mesh = meshes.add(Sphere::new(SKY_DOME_RADIUS).mesh().ico(3).unwrap());
    let sky_mat = materials.add(StandardMaterial {
        base_color: SKY_DOME_COLOR,
        emissive: SKY_DOME_EMISSIVE,
        unlit: true,
        cull_mode: Some(bevy::render::render_resource::Face::Front),
        ..default()
    });
    commands.spawn((
        ArenaMarker,
        Mesh3d(sky_mesh),
        MeshMaterial3d(sky_mat),
        Transform::from_xyz(0.0, 0.0, 0.0),
        bevy::light::NotShadowCaster,
        Name::new("CartoonSkyDome"),
    ));

    // ── Sun disk visible (sphère blanche émissive HDR) ──────────────────
    let sun_mesh = meshes.add(Sphere::new(SUN_DISK_RADIUS).mesh().ico(2).unwrap());
    let sun_mat = materials.add(StandardMaterial {
        base_color: SUN_DISK_COLOR,
        emissive: SUN_DISK_EMISSIVE,
        unlit: true,
        ..default()
    });
    commands.spawn((
        ArenaMarker,
        Mesh3d(sun_mesh),
        MeshMaterial3d(sun_mat),
        Transform::from_translation(SUN_DISK_POS),
        bevy::light::NotShadowCaster,
        Name::new("SunDisk"),
    ));

    // ── Nuages cartoon volumétriques : clusters de sphères "popcorn" ──────
    // Pattern Mario Galaxy / Studio Ghibli : chaque nuage = 6 blobs sphériques
    // de tailles variées agglomérés (cumulus stylisé). 3D vrai, pas plat.
    let cloud_blob_mesh = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());
    // Pattern V1 (sky/clouds.rs) : blanc pur quasi neutre, lit par DirectionalLight.
    let cloud_mat = materials.add(StandardMaterial {
        base_color: CLOUD_COLOR,
        emissive: CLOUD_EMISSIVE,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        reflectance: 0.0,
        ..default()
    });

    // 3 PRESETS de formes nuage (offset XYZ + radius local par blob)
    // Popcorn : cluster compact 6 blobs jitter (cumulus classique stylisé)
    let blobs_popcorn: &[(f32, f32, f32, f32)] = &[
        (0.0, 0.0, 0.0, 5.0),
        (4.5, 0.8, 1.2, 3.5),
        (-4.0, 0.4, -1.8, 3.2),
        (1.5, 1.5, 4.5, 2.8),
        (-2.0, -0.6, -4.2, 2.6),
        (3.5, -0.4, -3.0, 2.2),
    ];
    // Stratus : nuage allongé horizontal 8 blobs en ligne avec variations
    let blobs_stratus: &[(f32, f32, f32, f32)] = &[
        (-12.0, 0.0, 0.0, 2.5),
        (-8.0, 0.5, 0.8, 3.0),
        (-4.0, 0.3, -0.5, 3.5),
        (0.0, 0.0, 0.5, 4.0),
        (4.0, 0.4, -0.3, 3.6),
        (8.0, 0.2, 0.7, 3.0),
        (12.0, 0.0, -0.4, 2.4),
        (15.0, -0.3, 0.0, 1.8),
    ];
    // Puff : petit nuage compact dense 4 blobs (cumulus mini)
    let blobs_puff: &[(f32, f32, f32, f32)] = &[
        (0.0, 0.0, 0.0, 3.0),
        (2.5, 0.5, 0.5, 2.2),
        (-2.0, 0.3, -0.5, 2.0),
        (0.5, -0.3, 1.8, 1.8),
    ];

    // 18 clusters dispersés autour arène : (x, y, z, scale_xz, scale_y, blobs_preset_idx)
    // preset 0 = popcorn, 1 = stratus, 2 = puff
    let clusters: &[(f32, f32, f32, f32, f32, u8)] = &[
        // === Anneau intérieur (45-65m hauteur, plus proches) ===
        (-40.0, 55.0, -55.0, 1.0, 1.0, 0),   // popcorn NO
        (55.0, 62.0, -35.0, 1.3, 1.2, 0),    // popcorn NE big
        (5.0, 68.0, 65.0, 1.6, 0.9, 0),      // popcorn S overhead
        (-60.0, 50.0, 38.0, 1.0, 1.1, 0),    // popcorn SW
        (45.0, 58.0, 55.0, 0.85, 1.0, 0),    // popcorn SE
        (-25.0, 60.0, -80.0, 1.2, 0.8, 1),   // stratus N
        (70.0, 55.0, 20.0, 1.0, 0.9, 1),     // stratus E allongé
        (-80.0, 52.0, -10.0, 1.1, 0.85, 1),  // stratus W
        (15.0, 48.0, -25.0, 0.7, 1.0, 2),    // puff petit central
        (-15.0, 50.0, 25.0, 0.8, 1.0, 2),    // puff petit
        // === Anneau extérieur (75-95m hauteur, lointain horizon) ===
        (-100.0, 80.0, -60.0, 1.8, 0.7, 1),  // stratus far NW big
        (90.0, 85.0, -90.0, 1.5, 0.8, 0),    // popcorn far NE
        (-50.0, 90.0, 100.0, 2.0, 0.7, 1),   // stratus far S big
        (100.0, 75.0, 70.0, 1.4, 1.0, 0),    // popcorn far SE
        (0.0, 95.0, -120.0, 1.6, 0.8, 1),    // stratus far N horizon
        (-110.0, 70.0, 40.0, 1.2, 0.9, 0),   // popcorn far W
        (60.0, 88.0, 110.0, 1.7, 0.75, 1),   // stratus far SE
        (30.0, 78.0, -70.0, 0.9, 1.1, 2),    // puff mid-distance
    ];

    for (ci, &(cx, cy, cz, scale, hscale, preset)) in clusters.iter().enumerate() {
        let blobs: &[(f32, f32, f32, f32)] = match preset {
            1 => blobs_stratus,
            2 => blobs_puff,
            _ => blobs_popcorn,
        };
        let preset_name = match preset {
            1 => "stratus",
            2 => "puff",
            _ => "popcorn",
        };

        let parent_id = commands
            .spawn((
                ArenaMarker,
                CloudDrift,
                Transform::from_xyz(cx, cy, cz).with_scale(Vec3::new(scale, hscale * scale, scale)),
                Visibility::default(),
                Name::new(format!("Cloud_{preset_name}_{ci}")),
            ))
            .id();

        for (bi, &(bx, by, bz, br)) in blobs.iter().enumerate() {
            let child = commands
                .spawn((
                    Mesh3d(cloud_blob_mesh.clone()),
                    MeshMaterial3d(cloud_mat.clone()),
                    Transform::from_xyz(bx, by, bz).with_scale(Vec3::splat(br)),
                    bevy::light::NotShadowCaster,
                    Name::new(format!("Blob_{ci}_{bi}")),
                ))
                .id();
            commands.entity(parent_id).add_child(child);
        }
    }

    info!(
        "[forgia-fps] Arena spawned : {}×{}m, 121 floor + 44 walls + 12 pillars + 14 covers + 5 banners + 8 torches + 5 cubes + cartoon sky dome + sun disk + 18 cloud clusters (3 shapes : popcorn/stratus/puff)",
        (ARENA_SIZE as f32 * TILE_SIZE) as i32,
        (ARENA_SIZE as f32 * TILE_SIZE) as i32
    );
}

fn spawn_wall(
    commands: &mut Commands,
    wall_scene: &Handle<Scene>,
    pos: Vec3,
    yaw: f32,
) {
    commands.spawn((
        ArenaMarker,
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
        Visibility::default(),
        RigidBody::Fixed,
        // Collider mur : ~4m large × 3m haut × 0.3m épaisseur
        Collider::cuboid(TILE_SIZE / 2.0, 1.5, 0.15),
        Name::new("Wall"),
        children![(
            SceneRoot(wall_scene.clone()),
            Transform::default(),
        )],
    ));
}

/// Tire un raycast depuis la FpsCamera quand le joueur clique gauche.
/// Si touche un TargetCube → applique damage + flash blanc + emit CombatHitEvent.
/// Phase 2.1 minimum (sans tracer/muzzle, sans cooldown réel).
fn fire_weapon_minimal(
    // MouseButtonInput events (NOT consumed by egui, contrairement à ButtonInput Resource)
    mut mouse_evs: MessageReader<MouseButtonInput>,
    rapier: ReadRapierContext,
    q_cam: Query<&GlobalTransform, With<FpsCamera>>,
    q_player: Query<Entity, With<Player>>,
    // Fusionné en 1 query mut pour éviter B0001 (anti-trap V1 CLAUDE.md §6)
    mut q_target: Query<(&MeshMaterial3d<StandardMaterial>, &mut Health), With<TargetCube>>,
    mut commands: Commands,
    flash_cache: Res<HitFlashCache>,
    tracer_res: Option<Res<TracerResources>>,
    weapon_vfx: Option<Res<WeaponVfxEffects>>,
    cooldown: Option<Res<WeaponFireCooldown>>,
    mut virtual_time: ResMut<Time<Virtual>>,
    mut hit_events: MessageWriter<CombatHitEvent>,
    mut recoil_events: MessageWriter<WeaponRecoilImpulse>,
) {
    // Cooldown anti-spam : ModernAR ~0.1s entre tirs
    if cooldown.is_some() {
        // Drain les events pour ne pas fire au prochain frame
        for _ in mouse_evs.read() {}
        return;
    }
    let mut left_pressed = false;
    for ev in mouse_evs.read() {
        if ev.button == MouseButton::Left && ev.state == ButtonState::Pressed {
            left_pressed = true;
        }
    }
    if !left_pressed {
        return;
    }
    let Ok(cam_tf) = q_cam.single() else {
        warn!("[fire] FpsCamera not found");
        return;
    };
    let Ok(ctx) = rapier.single() else {
        warn!("[fire] RapierContext not found");
        return;
    };

    let origin = cam_tf.translation();
    let direction = cam_tf.forward().as_vec3();

    // Insert cooldown 100ms (10 shots/sec ModernAR)
    commands.insert_resource(WeaponFireCooldown {
        timer: Timer::from_seconds(0.1, TimerMode::Once),
    });

    // Camera recoil DÉSACTIVÉ — choix design Antoine 2026-05-14 : pas de visual recoil
    // (style Valorant skill-ceiling pur). Le système weapon_recoil_apply reste wired
    // dans forgia-player mais ne reçoit jamais d'event = no-op.
    // Pour réactiver : décommenter le write ci-dessous.
    // let yaw_jitter = ((origin.x * 1000.0).sin() * 0.012).clamp(-0.012, 0.012);
    // recoil_events.write(WeaponRecoilImpulse { pitch_rad: 0.045, yaw_rad: yaw_jitter });
    let _ = &mut recoil_events; // suppress unused warning

    // Muzzle flash au barrel tip (0.3m devant cam, 0.1m sous = position canon)
    let barrel_tip = origin + direction * 0.3 + Vec3::new(0.0, -0.1, 0.0);
    if let Some(vfx) = weapon_vfx.as_deref() {
        spawn_muzzle_flash(&mut commands, vfx, barrel_tip, direction, ());
    }

    // Exclure le Player du raycast (sinon hit immédiat le collider capsule du player
    // car FpsCamera est enfant de Player → origin ray DANS le collider, toi=0).
    let player_entity = q_player.single().ok();
    let predicate = |e: Entity| Some(e) != player_entity;
    let filter = QueryFilter::default().predicate(&predicate);
    let hit_result = ctx.cast_ray(origin, direction, 100.0, true, filter);
    let hit_dist = hit_result.map(|(_, t)| t).unwrap_or(50.0);

    // Tracer visuel (toujours, même sur miss — fly to max range)
    if let Some(tres) = tracer_res.as_deref() {
        spawn_hitscan_tracer(
            &mut commands,
            tres,
            origin,
            direction,
            hit_dist,
            &WeaponType::ModernAR,
            120.0, // tracer_max_length (V1 wfx_tracer_max_length)
            0.30,  // tracer_fade — 300ms (V1=80ms trop court à 60fps = 5 frames invisible)
        );
    }

    // Impact VFX au point d'impact (cube, mur, colonne — pas miss)
    if let Some((_, toi)) = hit_result {
        let impact_pos = origin + direction * toi;
        if let Some(vfx) = weapon_vfx.as_deref() {
            spawn_impact_vfx(&mut commands, vfx, impact_pos, ());
        }
    }

    if let Some((entity, toi)) = hit_result {
        if let Ok((mat, mut hp)) = q_target.get_mut(entity) {
            let damage = 25.0;
            hp.current = (hp.current - damage).max(0.0);
            let dead = hp.is_dead();
            let new_hp = hp.current;
            let mat_handle = mat.0.clone();

            // Swap material vers flash blanc + insert HitFlashTimer
            commands
                .entity(entity)
                .insert(MeshMaterial3d(flash_cache.flash_material.clone()))
                .insert(HitFlashTimer {
                    timer: Timer::from_seconds(0.15, TimerMode::Once),
                    original_emissive: LinearRgba::new(0.0, 0.0, 0.0, 1.0),
                    original_handle: Some(mat_handle),
                });

            // Hit-stop pattern Apex : 50ms de pause Time<Virtual> sur hit confirmé
            virtual_time.set_relative_speed(0.05);
            commands.insert_resource(HitStopState {
                timer: Timer::from_seconds(0.05, TimerMode::Once),
                restore_speed: 1.0,
            });

            hit_events.write(CombatHitEvent {
                target: entity,
                damage,
                is_kill: dead,
            });

            info!(
                "[fire] HIT cube {:?} toi={:.2} dmg={} hp={}/100 dead={}",
                entity, toi, damage, new_hp, dead
            );
        } else {
            info!("[fire] hit non-target entity {:?} toi={:.2}", entity, toi);
        }
    } else {
        info!("[fire] miss");
    }
}

/// Drift wind lent des nuages vers +X (pattern V1 sky/clouds.rs).
/// Wrap autour de ±150m pour cycle continu sans pop.
fn cloud_drift_system(
    time: Res<Time>,
    mut q: Query<&mut Transform, With<CloudDrift>>,
) {
    let drift = CLOUD_DRIFT_SPEED * time.delta_secs();
    for mut tf in q.iter_mut() {
        tf.translation.x += drift;
        if tf.translation.x > CLOUD_WRAP_DIST {
            tf.translation.x -= 2.0 * CLOUD_WRAP_DIST;
        }
    }
}

/// Despawn les cubes morts (HP=0). Système séparé chained après fire.
fn despawn_dead_cubes(
    mut commands: Commands,
    q: Query<(Entity, &Health), With<TargetCube>>,
) {
    for (entity, hp) in &q {
        if hp.is_dead() {
            commands.entity(entity).despawn();
            info!("[death] cube {:?} despawned (HP=0)", entity);
        }
    }
}

fn cleanup_arena(mut commands: Commands, q: Query<Entity, With<ArenaMarker>>) {
    let count = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    info!("[forgia-fps] Arena cleaned : {count} entities despawned");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaFpsPlugin;
    }

    #[test]
    fn arena_size_is_odd() {
        // ARENA_SIZE doit être impair pour avoir un centre exact (0,0)
        assert_eq!(ARENA_SIZE % 2, 1, "ARENA_SIZE must be odd for centered grid");
    }
}
