//! # forgia-mode-fps-arena
//!
//! Spawn / cleanup arena KayKit Dungeon Pack + clouds orbit.
//!
//! Extrait de `forgia-fps` 2026-05-16 (règle `fine-grained-crates.md`).
//!
//! Pattern V1 :
//! - KayKit walls **`WALL_Y = 0.0`** (LOCK absolu : pivot mesh au sol, pas centre)
//! - `TILE_SIZE = 4.0` (KayKit dungeon convention)
//! - Forgia scaled scene pattern : parent scale=1 + child SceneRoot scale (rapier3d 0.33 quirk)

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_ai_arena_bot::{ArenaBot, BotShootConfig, ForgiaAiArenaBotPlugin};
use forgia_combat::prelude::*;
use forgia_core::prelude::*;
use forgia_genome_core::{Genome, GenomeLoader};
use serde::Deserialize;

pub mod prelude {
    pub use crate::{
        ArenaBotsGenome, ArenaBotsGenomeHandle, ArenaMarker, CloudOrbit,
        ForgiaModeFpsArenaPlugin, HitZone, TargetCube,
    };
}

/// Zone d'impact sur un training bot. Inseré sur chaque collider enfant
/// (Head/Body) pour permettre damage multiplier en headshot (style Overwatch).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitZone {
    Head,
    Body,
}

// ─── Arena Bots Genome (Phase H+ — data-driven training bots) ────────

#[derive(Deserialize, TypePath, Clone)]
pub struct ArenaBotsGenome {
    pub hp: f32,
    pub body_y: f32,
    pub head_y: f32,
    pub body: BotPart,
    pub head: BotPart,
    #[serde(default)]
    pub show_collider_debug: bool,
    pub ai: BotAi,
    pub spawn_positions: Vec<BotSpawn>,
}

#[derive(Deserialize, TypePath, Clone)]
pub struct BotPart {
    /// Pour le body : [width, height, depth]. Pour le head : seul `radius` est lu.
    #[serde(default)]
    pub width: f32,
    #[serde(default)]
    pub height: f32,
    #[serde(default)]
    pub depth: f32,
    #[serde(default)]
    pub radius: f32,
    pub color: [f32; 3],
    pub emissive: [f32; 3],
}

#[derive(Deserialize, TypePath, Clone)]
pub struct BotAi {
    pub shot_range: f32,
    pub shot_cooldown_secs: f32,
    pub shot_damage: f32,
    pub shot_warmup_secs: f32,
    pub detect_range: f32,
    pub shot_jitter_deg: f32,
    pub tracer_emissive: [f32; 3],
}

#[derive(Deserialize, TypePath, Clone)]
pub struct BotSpawn {
    pub character_glb: String,
    #[serde(default = "default_character_scale")]
    pub character_scale: f32,
    /// Yaw offset local du SceneRoot — corrige le forward axis du mesh (Meshy varie).
    /// 0 = mesh forward déjà aligné -Z (Bevy convention), 180 = flip si mesh face +Z.
    #[serde(default)]
    pub character_yaw_deg: f32,
    /// Lift Y local du mesh pour compenser le pivot au centre (vs pieds-au-sol).
    /// Default 0.9 = lift half ~1.8m character. Tweakable per-character (Meshy variable).
    #[serde(default = "default_character_y_offset")]
    pub character_y_offset: f32,
    pub x: f32,
    pub z: f32,
}

fn default_character_scale() -> f32 {
    1.0
}

fn default_character_y_offset() -> f32 {
    0.9
}

#[derive(Resource)]
pub struct ArenaBotsGenomeHandle(pub Handle<Genome<ArenaBotsGenome>>);

/// Fallback hardcoded utilisé si genome pas encore chargé au spawn.
fn default_arena_bots() -> ArenaBotsGenome {
    ArenaBotsGenome {
        hp: 100.0,
        body_y: 0.65,
        head_y: 1.55,
        body: BotPart {
            width: 0.7,
            height: 1.3,
            depth: 0.4,
            radius: 0.0,
            color: [0.85, 0.15, 0.15],
            emissive: [0.6, 0.0, 0.0],
        },
        head: BotPart {
            width: 0.0,
            height: 0.0,
            depth: 0.0,
            radius: 0.22,
            color: [1.0, 0.35, 0.20],
            emissive: [1.2, 0.3, 0.0],
        },
        show_collider_debug: true, // fallback = montre cubes (lisible si TOML pas loaded)
        ai: BotAi {
            shot_range: 35.0,
            shot_cooldown_secs: 1.5,
            shot_damage: 12.0,
            shot_warmup_secs: 0.8,
            detect_range: 50.0,
            shot_jitter_deg: 4.0,
            tracer_emissive: [4.0, 1.5, 0.5],
        },
        spawn_positions: vec![
            BotSpawn {
                character_glb: String::new(),
                character_scale: 1.0,
                character_yaw_deg: 0.0,
                character_y_offset: 0.9,
                x: -4.0,
                z: -7.0,
            },
            BotSpawn {
                character_glb: String::new(),
                character_scale: 1.0,
                character_yaw_deg: 0.0,
                character_y_offset: 0.9,
                x: 0.0,
                z: -7.0,
            },
            BotSpawn {
                character_glb: String::new(),
                character_scale: 1.0,
                character_yaw_deg: 0.0,
                character_y_offset: 0.9,
                x: 4.0,
                z: -7.0,
            },
            BotSpawn {
                character_glb: String::new(),
                character_scale: 1.0,
                character_yaw_deg: 0.0,
                character_y_offset: 0.9,
                x: 10.0,
                z: -14.0,
            },
            BotSpawn {
                character_glb: String::new(),
                character_scale: 1.0,
                character_yaw_deg: 0.0,
                character_y_offset: 0.9,
                x: -14.0,
                z: 10.0,
            },
        ],
    }
}

pub const TILE_SIZE: f32 = 4.0;
pub const ARENA_SIZE: i32 = 19; // 19×19 tiles = 76×76m (story-441 agrandi 2026-05-17 night, x~3 surface vs 11)

// ── Cloud constants (skybox cubemap V1 wired par forgia-player) ──────
const CLOUD_COLOR: Color = Color::srgb(0.95, 0.96, 0.97);
const CLOUD_EMISSIVE: LinearRgba = LinearRgba::new(0.05, 0.05, 0.06, 1.0);
const CLOUD_ORBIT_SPEED: f32 = 0.025; // rad/s — orbit complet ~4 min

#[derive(Component)]
pub struct ArenaMarker;

/// Marker pour les cubes-cibles (testables via fire_weapon_minimal).
#[derive(Component)]
pub struct TargetCube;

/// Cluster nuage en orbit circulaire autour du centre arène (Y axis).
/// Stocke angle/radius/height pour calcul polaire continu (pas de wrap brusque).
#[derive(Component)]
pub struct CloudOrbit {
    pub angle: f32,
    pub radius: f32,
    pub height: f32,
}

pub struct ForgiaModeFpsArenaPlugin;

impl Plugin for ForgiaModeFpsArenaPlugin {
    fn build(&self, app: &mut App) {
        // ForgiaAiArenaBotPlugin idempotent — owne shooting AI + respawn logic.
        if !app.is_plugin_added::<ForgiaAiArenaBotPlugin>() {
            app.add_plugins(ForgiaAiArenaBotPlugin);
        }
        app.init_asset::<Genome<ArenaBotsGenome>>()
            .register_asset_loader(GenomeLoader::<ArenaBotsGenome>::default())
            .add_systems(Startup, load_arena_bots_genome)
            .add_systems(OnEnter(GameMode::Fps), spawn_arena)
            .add_systems(OnExit(GameMode::Fps), cleanup_arena)
            .add_systems(
                Update,
                cloud_drift_system
                    .in_set(GameSet::Effects)
                    .run_if(in_state(GameMode::Fps)),
            );
    }
}

fn load_arena_bots_genome(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Genome<ArenaBotsGenome>> = asset_server.load("genomes/arena_bots.toml");
    commands.insert_resource(ArenaBotsGenomeHandle(handle));
    info!("[forgia-mode-fps-arena] arena_bots genome loading : genomes/arena_bots.toml");
}

fn spawn_arena(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    bots_handle: Option<Res<ArenaBotsGenomeHandle>>,
    bots_assets: Res<Assets<Genome<ArenaBotsGenome>>>,
) {
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

    for x in -half..=half {
        for z in -half..=half {
            let dist = x.abs().max(z.abs());
            let tile_handle = if dist >= 4 {
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

    commands.spawn((
        ArenaMarker,
        Transform::from_xyz(0.0, -0.5, 0.0),
        RigidBody::Fixed,
        Collider::cuboid(arena_extent + 5.0, 0.5, arena_extent + 5.0),
        Name::new("ArenaGroundCollider"),
    ));

    let edge = (half as f32 + 0.5) * TILE_SIZE;
    for i in -half..=half {
        let offset = i as f32 * TILE_SIZE;
        let pick_wall = |idx: i32| -> Handle<Scene> {
            match idx.rem_euclid(7) {
                0 => wall_window.clone(),
                3 => wall_broken.clone(),
                _ => wall.clone(),
            }
        };
        let nord = if i == 0 { wall_arched.clone() } else { pick_wall(i) };
        let sud = if i == 0 { wall_arched.clone() } else { pick_wall(i + 1) };
        let est = if i == 0 { wall_arched.clone() } else { pick_wall(i + 2) };
        let ouest = if i == 0 { wall_arched.clone() } else { pick_wall(i + 3) };

        spawn_wall(&mut commands, &nord, Vec3::new(offset, 0.0, edge), 0.0);
        spawn_wall(&mut commands, &sud, Vec3::new(offset, 0.0, -edge), std::f32::consts::PI);
        spawn_wall(&mut commands, &est, Vec3::new(edge, 0.0, offset), -std::f32::consts::FRAC_PI_2);
        spawn_wall(&mut commands, &ouest, Vec3::new(-edge, 0.0, offset), std::f32::consts::FRAC_PI_2);
    }

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

    let cover_props: &[(&str, f32, f32, Handle<Scene>, f32, f32)] = &[
        ("Crates_NE", 8.0, -8.0, crates.clone(), 0.6, 0.6),
        ("Crates_SW", -8.0, 8.0, crates, 0.6, 0.6),
        ("Rubble_N", 0.0, -14.0, rubble.clone(), 1.0, 0.4),
        ("Rubble_S", 0.0, 14.0, rubble.clone(), 1.0, 0.4),
        ("Rubble_E", 14.0, 0.0, rubble.clone(), 1.0, 0.4),
        ("Rubble_W", -14.0, 0.0, rubble, 1.0, 0.4),
        ("BarrelStack_NW", -10.0, -10.0, barrels_stack.clone(), 0.7, 0.7),
        ("BarrelStack_SE", 10.0, 10.0, barrels_stack, 0.7, 0.7),
        ("Table_E", 6.0, 4.0, table.clone(), 1.5, 0.6),
        ("Table_W", -6.0, -4.0, table, 1.5, 0.6),
        ("Barrel_1", 3.0, 12.0, barrel.clone(), 0.5, 0.4),
        ("Barrel_2", -3.0, -12.0, barrel.clone(), 0.5, 0.4),
        ("Barrel_3", 12.0, -3.0, barrel.clone(), 0.5, 0.4),
        ("Barrel_4", -12.0, 3.0, barrel, 0.5, 0.4),
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

    commands.spawn((
        ArenaMarker,
        Transform::from_xyz(16.0, 0.0, 16.0),
        Visibility::default(),
        Name::new("ChestGold_NE"),
        children![(SceneRoot(chest_gold), Transform::default())],
    ));
    commands.spawn((
        ArenaMarker,
        Transform::from_xyz(-16.0, 0.0, -16.0),
        Visibility::default(),
        Name::new("Chest_SW"),
        children![(SceneRoot(chest), Transform::default())],
    ));

    let banners: &[(&str, f32, f32, Handle<Scene>, f32)] = &[
        ("Banner_N_red", -6.0, edge - 0.3, banner_red.clone(), 0.0),
        ("Banner_N_blue", 6.0, edge - 0.3, banner_blue.clone(), 0.0),
        ("Banner_S_yellow", 0.0, -edge + 0.3, banner_yellow, std::f32::consts::PI),
        ("Banner_E_red", edge - 0.3, -6.0, banner_red, -std::f32::consts::FRAC_PI_2),
        ("Banner_W_blue", -edge + 0.3, 6.0, banner_blue, std::f32::consts::FRAC_PI_2),
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

    let torch_positions: &[(&str, f32, f32, Handle<Scene>)] = &[
        ("Torch_NE", 18.0, -18.0, torch.clone()),
        ("Torch_NW", -18.0, -18.0, torch.clone()),
        ("Torch_SE", 18.0, 18.0, torch.clone()),
        ("Torch_SW", -18.0, 18.0, torch),
        ("TorchWall_N", -10.0, edge - 0.5, torch_wall.clone()),
        ("TorchWall_S", 10.0, -edge + 0.5, torch_wall.clone()),
        ("TorchWall_E", edge - 0.5, 10.0, torch_wall.clone()),
        ("TorchWall_W", -edge + 0.5, -10.0, torch_wall),
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

    commands.spawn((
        ArenaMarker,
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 22_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.35, -0.78, 0.0)),
        Name::new("ArenaSunLight"),
    ));

    // Training bots Overwatch-style — TOUT data-driven depuis arena_bots.toml.
    // Fallback hardcoded (default_arena_bots) si genome pas encore loaded.
    //
    // Hierarchy :
    //   parent (TargetCube + Health, no mesh/collider)
    //   ├─ Body (cuboid + collider, Y=body_y, HitZone::Body)
    //   └─ Head (sphere + collider, Y=head_y, HitZone::Head)
    //
    // Raycast hit retourne l'entity enfant → HitZone pour multiplier dmg, ChildOf
    // → parent pour Health.
    let bots_owned;
    let bots_data: &ArenaBotsGenome = match bots_handle
        .as_deref()
        .and_then(|h| bots_assets.get(&h.0))
    {
        Some(g) => &g.data,
        None => {
            warn!("[forgia-mode-fps-arena] arena_bots genome pas chargé — fallback hardcoded defaults");
            bots_owned = default_arena_bots();
            &bots_owned
        }
    };

    // Visuels debug colliders (cubes opaques) — uniquement si TOML `show_collider_debug = true`.
    let debug_meshes = if bots_data.show_collider_debug {
        let body_mesh = meshes.add(Cuboid::new(
            bots_data.body.width,
            bots_data.body.height,
            bots_data.body.depth,
        ));
        let head_mesh = meshes.add(Sphere::new(bots_data.head.radius).mesh().ico(3).unwrap());
        let body_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(
                bots_data.body.color[0],
                bots_data.body.color[1],
                bots_data.body.color[2],
            ),
            emissive: LinearRgba::new(
                bots_data.body.emissive[0],
                bots_data.body.emissive[1],
                bots_data.body.emissive[2],
                1.0,
            ),
            ..default()
        });
        let head_mat = materials.add(StandardMaterial {
            base_color: Color::srgb(
                bots_data.head.color[0],
                bots_data.head.color[1],
                bots_data.head.color[2],
            ),
            emissive: LinearRgba::new(
                bots_data.head.emissive[0],
                bots_data.head.emissive[1],
                bots_data.head.emissive[2],
                1.0,
            ),
            ..default()
        });
        Some((body_mesh, head_mesh, body_mat, head_mat))
    } else {
        None
    };

    let body_half = (
        bots_data.body.width * 0.5,
        bots_data.body.height * 0.5,
        bots_data.body.depth * 0.5,
    );
    let ai = &bots_data.ai;

    for spawn in &bots_data.spawn_positions {
        let (x, z) = (spawn.x, spawn.z);
        let bot_name = spawn
            .character_glb
            .rsplit('/')
            .next()
            .and_then(|s| s.split('.').next())
            .unwrap_or("Enemy")
            .to_string();

        // Parent : Health + ArenaBot AI + position.
        // Pas de mesh sur le parent ; les enfants ont mesh visuel + colliders.
        let parent = commands
            .spawn((
                ArenaMarker,
                TargetCube,
                Transform::from_xyz(x, 0.0, z),
                Visibility::default(),
                Health::new(bots_data.hp),
                ArenaBot {
                    state: forgia_ai_arena_bot::BotState::Idle,
                    speed: 0.0, // V1 : bots statiques (rotate to face player only)
                    detect_range: ai.detect_range,
                    attack_range: ai.shot_range,
                    attack_cooldown: ai.shot_cooldown_secs,
                    attack_left: ai.shot_warmup_secs, // warmup anti spawn-kill
                },
                BotShootConfig {
                    damage: ai.shot_damage,
                    range: ai.shot_range,
                    jitter_rad: ai.shot_jitter_deg.to_radians(),
                    tracer_emissive: LinearRgba::new(
                        ai.tracer_emissive[0],
                        ai.tracer_emissive[1],
                        ai.tracer_emissive[2],
                        1.0,
                    ),
                    shoulder_y: bots_data.head_y - 0.15, // shoulder un peu sous la tête
                    target_torso_y: 1.0,
                },
                Name::new(format!("Enemy_{bot_name}_{x}_{z}")),
            ))
            .id();

        let body_y = bots_data.body_y;
        let head_y = bots_data.head_y;
        let head_radius = bots_data.head.radius;
        let character_path = spawn.character_glb.clone();
        let character_scale = spawn.character_scale;
        let character_yaw = spawn.character_yaw_deg.to_radians();
        let character_y = spawn.character_y_offset;
        let debug = debug_meshes.clone();

        commands.entity(parent).with_children(|p| {
            // Mesh character (visible). Pas de collider ici — les colliders sont
            // séparés en head/body pour la hitzone Overwatch.
            // Y offset compense le pivot Meshy au centre du mesh (vs pieds au sol).
            if !character_path.is_empty() {
                p.spawn((
                    SceneRoot(asset_server.load(&character_path)),
                    Transform::from_xyz(0.0, character_y, 0.0)
                        .with_rotation(Quat::from_rotation_y(character_yaw))
                        .with_scale(Vec3::splat(character_scale)),
                    Name::new("CharacterMesh"),
                ));
            }

            // Body hit zone — collider INVISIBLE (sauf debug). HitZone Component.
            let mut body_entity = p.spawn((
                HitZone::Body,
                Transform::from_xyz(0.0, body_y, 0.0),
                RigidBody::Fixed,
                Collider::cuboid(body_half.0, body_half.1, body_half.2),
                Name::new("Body"),
            ));
            if let Some((ref bm, _, ref bmat, _)) = debug {
                body_entity.insert((Mesh3d(bm.clone()), MeshMaterial3d(bmat.clone())));
            }

            // Head hit zone — collider INVISIBLE (sauf debug).
            let mut head_entity = p.spawn((
                HitZone::Head,
                Transform::from_xyz(0.0, head_y, 0.0),
                RigidBody::Fixed,
                Collider::ball(head_radius),
                Name::new("Head"),
            ));
            if let Some((_, ref hm, _, ref hmat)) = debug {
                head_entity.insert((Mesh3d(hm.clone()), MeshMaterial3d(hmat.clone())));
            }
        });
    }

    // ── Cartoon cloud clusters (orbit) ─────────────────────────────────
    let cloud_blob_mesh = meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap());
    let cloud_mat = materials.add(StandardMaterial {
        base_color: CLOUD_COLOR,
        emissive: CLOUD_EMISSIVE,
        perceptual_roughness: 1.0,
        metallic: 0.0,
        reflectance: 0.0,
        ..default()
    });

    let blobs_popcorn: &[(f32, f32, f32, f32)] = &[
        (0.0, 0.0, 0.0, 5.0),
        (4.5, 0.8, 1.2, 3.5),
        (-4.0, 0.4, -1.8, 3.2),
        (1.5, 1.5, 4.5, 2.8),
        (-2.0, -0.6, -4.2, 2.6),
        (3.5, -0.4, -3.0, 2.2),
    ];
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
    let blobs_puff: &[(f32, f32, f32, f32)] = &[
        (0.0, 0.0, 0.0, 3.0),
        (2.5, 0.5, 0.5, 2.2),
        (-2.0, 0.3, -0.5, 2.0),
        (0.5, -0.3, 1.8, 1.8),
    ];

    let clusters: &[(f32, f32, f32, f32, f32, u8)] = &[
        (-40.0, 55.0, -55.0, 1.0, 1.0, 0),
        (55.0, 62.0, -35.0, 1.3, 1.2, 0),
        (5.0, 68.0, 65.0, 1.6, 0.9, 0),
        (-60.0, 50.0, 38.0, 1.0, 1.1, 0),
        (45.0, 58.0, 55.0, 0.85, 1.0, 0),
        (-25.0, 60.0, -80.0, 1.2, 0.8, 1),
        (70.0, 55.0, 20.0, 1.0, 0.9, 1),
        (-80.0, 52.0, -10.0, 1.1, 0.85, 1),
        (15.0, 48.0, -25.0, 0.7, 1.0, 2),
        (-15.0, 50.0, 25.0, 0.8, 1.0, 2),
        (-100.0, 80.0, -60.0, 1.8, 0.7, 1),
        (90.0, 85.0, -90.0, 1.5, 0.8, 0),
        (-50.0, 90.0, 100.0, 2.0, 0.7, 1),
        (100.0, 75.0, 70.0, 1.4, 1.0, 0),
        (0.0, 95.0, -120.0, 1.6, 0.8, 1),
        (-110.0, 70.0, 40.0, 1.2, 0.9, 0),
        (60.0, 88.0, 110.0, 1.7, 0.75, 1),
        (30.0, 78.0, -70.0, 0.9, 1.1, 2),
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

        let angle = cz.atan2(cx);
        let radius = (cx * cx + cz * cz).sqrt();
        let parent_id = commands
            .spawn((
                ArenaMarker,
                CloudOrbit { angle, radius, height: cy },
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
        "[forgia-mode-fps-arena] Arena spawned : {}×{}m, KayKit modular + {} training bots (head+body) + 18 cloud clusters",
        (ARENA_SIZE as f32 * TILE_SIZE) as i32,
        (ARENA_SIZE as f32 * TILE_SIZE) as i32,
        bots_data.spawn_positions.len(),
    );
}

fn spawn_wall(commands: &mut Commands, wall_scene: &Handle<Scene>, pos: Vec3, yaw: f32) {
    commands.spawn((
        ArenaMarker,
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(yaw)),
        Visibility::default(),
        RigidBody::Fixed,
        Collider::cuboid(TILE_SIZE / 2.0, 1.5, 0.15),
        Name::new("Wall"),
        children![(SceneRoot(wall_scene.clone()), Transform::default())],
    ));
}

/// Orbit nuages autour du centre Y axis : rotation continue, jamais wrap brusque.
fn cloud_drift_system(time: Res<Time>, mut q: Query<(&mut Transform, &mut CloudOrbit)>) {
    let dt = time.delta_secs();
    for (mut tf, mut orbit) in &mut q {
        orbit.angle += CLOUD_ORBIT_SPEED * dt;
        if orbit.angle > std::f32::consts::TAU {
            orbit.angle -= std::f32::consts::TAU;
        }
        tf.translation.x = orbit.angle.cos() * orbit.radius;
        tf.translation.z = orbit.angle.sin() * orbit.radius;
        tf.translation.y = orbit.height;
    }
}

fn cleanup_arena(mut commands: Commands, q: Query<Entity, With<ArenaMarker>>) {
    let count = q.iter().count();
    for e in &q {
        commands.entity(e).despawn();
    }
    info!("[forgia-mode-fps-arena] Arena cleaned : {count} entities despawned");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaModeFpsArenaPlugin;
    }

    #[test]
    fn arena_size_is_odd() {
        assert_eq!(ARENA_SIZE % 2, 1, "ARENA_SIZE must be odd for centered grid");
    }
}
