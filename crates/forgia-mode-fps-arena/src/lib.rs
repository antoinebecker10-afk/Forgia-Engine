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
use forgia_ai_arena_bot::ForgiaAiArenaBotPlugin;
use forgia_core::prelude::*;
use forgia_genome_core::{Genome, GenomeLoader};
use serde::Deserialize;

mod wave;

pub mod prelude {
    pub use crate::wave::{
        ArenaWavesGenome, ArenaWavesHandle, ArenaWavesPlugin, WaveCompletedEvent, WavePhase,
        WaveStartedEvent, WaveState,
    };
    pub use crate::{
        ArenaBotsGenome, ArenaBotsGenomeHandle, ArenaMarker, CloudOrbit, ForgiaModeFpsArenaPlugin,
        TargetCube,
    };
}

pub use wave::{
    ArenaWavesGenome, ArenaWavesHandle, ArenaWavesPlugin, WaveCompletedEvent, WavePhase,
    WaveStartedEvent, WaveState,
};

// Story-453 v2 (2026-05-18) — mesh-exact hitbox via `AsyncSceneCollider` ConvexHull.
// Pas de Component custom : rapier walk le SceneRoot child et génère 1 collider
// ConvexHull par Mesh3d, attaché au RigidBody du parent bot.
// Le ray hit le collider généré → ChildOf walk dans damage path → parent Health.

/// Sync HP des bots **existants** quand le genome `arena_bots.toml` est hot-reload.
/// Pattern : track last_hp dans Local, applique nouveau hp.max + preserve ratio current/max.
fn sync_existing_bot_hp(
    bots_handle: Option<Res<ArenaBotsGenomeHandle>>,
    bots_assets: Res<Assets<Genome<ArenaBotsGenome>>>,
    mut q: Query<&mut forgia_combat::Health, With<TargetCube>>,
    mut last_hp: Local<f32>,
) {
    let Some(handle) = bots_handle else { return };
    let Some(genome) = bots_assets.get(&handle.0) else {
        return;
    };
    let new_hp = genome.data.hp;
    if (new_hp - *last_hp).abs() < 0.01 {
        return;
    }
    let was_zero = *last_hp == 0.0;
    *last_hp = new_hp;
    if was_zero {
        return;
    }
    for mut hp in &mut q {
        let ratio = if hp.max > 0.01 {
            (hp.current / hp.max).clamp(0.0, 1.0)
        } else {
            1.0
        };
        hp.max = new_hp;
        hp.current = new_hp * ratio;
    }
    info!("[arena-bots] hot-reload HP : new max = {new_hp}");
}

// ─── Arena Bots Genome (Phase H+ — data-driven training bots) ────────

/// Story-453 baseline (2026-05-18) — schema simplifié :
/// hp + capsule dims + show_collider_debug + ai + spawn_positions.
/// Plus de Head/Body split, plus de body_y/head_y, plus de BotPart.
#[derive(Deserialize, TypePath, Clone)]
pub struct ArenaBotsGenome {
    pub hp: f32,
    /// Demi-hauteur cylindrique de la capsule (sans les hemispheres).
    /// Total height = 2 * (body_half_h + body_radius).
    pub body_half_h: f32,
    /// Rayon capsule (X et Z).
    pub body_radius: f32,
    /// Couleur RGB du mesh debug viz (rouge typique).
    #[serde(default = "default_debug_color")]
    pub debug_color: [f32; 3],
    #[serde(default)]
    pub show_collider_debug: bool,
    pub ai: BotAi,
    pub spawn_positions: Vec<BotSpawn>,
}

fn default_debug_color() -> [f32; 3] {
    [1.0, 0.0, 0.0]
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
    // Story-456 Phase 1 — Activation runtime mouvement
    /// Vitesse de déplacement m/s en Chase (state machine ArenaBot.speed).
    /// 0.0 = bot statique (V2 baseline reset). 3.0-4.5 = tactical AAA standard.
    #[serde(default = "default_bot_speed")]
    pub speed: f32,
    /// Distance d'arrêt avant le player (m). Bot s'arrête pour tirer au lieu de coller.
    /// Doit être > collider player radius (~0.5m). AAA standard 4-8m.
    #[serde(default = "default_bot_stop_distance")]
    pub stop_distance: f32,
    // Story-456 Phase 2-4 — Tactical AI tuning (genome-driven, hot-reload)
    #[serde(default = "default_los_check_hz")]
    pub los_check_hz: f32,
    #[serde(default = "default_los_grace_secs")]
    pub los_grace_secs: f32,
    #[serde(default = "default_strafe_amplitude_m")]
    pub strafe_amplitude_m: f32,
    #[serde(default = "default_strafe_freq_hz")]
    pub strafe_freq_hz: f32,
    #[serde(default = "default_strafe_noise_weight")]
    pub strafe_noise_weight: f32,
    #[serde(default = "default_local_avoid_dist_m")]
    pub local_avoid_dist_m: f32,
    #[serde(default = "default_gunshot_alert_radius_m")]
    pub gunshot_alert_radius_m: f32,
    #[serde(default = "default_gunshot_alert_los_grace_secs")]
    pub gunshot_alert_los_grace_secs: f32,
    #[serde(default = "default_alert_duration_secs")]
    pub alert_duration_secs: f32,
    /// Story-464 — durée pendant laquelle un bot reste autorisé à Chase après
    /// avoir perdu LOS sur le player. Sans ce gate, bots Chase à travers les murs.
    #[serde(default = "default_los_lost_grace_secs")]
    pub los_lost_grace_secs: f32,
}

fn default_bot_speed() -> f32 {
    3.5
}
fn default_bot_stop_distance() -> f32 {
    6.0
}
fn default_los_check_hz() -> f32 {
    8.0
}
fn default_los_grace_secs() -> f32 {
    0.35
}
fn default_strafe_amplitude_m() -> f32 {
    1.8
}
fn default_strafe_freq_hz() -> f32 {
    0.9
}
fn default_strafe_noise_weight() -> f32 {
    0.35
}
fn default_local_avoid_dist_m() -> f32 {
    2.5
}
fn default_gunshot_alert_radius_m() -> f32 {
    25.0
}
fn default_gunshot_alert_los_grace_secs() -> f32 {
    0.6
}
fn default_alert_duration_secs() -> f32 {
    4.0
}
fn default_los_lost_grace_secs() -> f32 {
    2.0
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
    /// Story-457 — Y world position de la sphere sensor head proxy par rapport
    /// au parent bot. Override par-bot si character_y_offset/scale custom.
    #[serde(default = "default_head_y_offset")]
    pub head_y_offset: f32,
    /// Story-457 — Rayon sphere head proxy (m).
    #[serde(default = "default_head_radius")]
    pub head_radius: f32,
}

fn default_character_scale() -> f32 {
    1.0
}

fn default_character_y_offset() -> f32 {
    0.9
}

fn default_head_y_offset() -> f32 {
    1.75
}

fn default_head_radius() -> f32 {
    0.22
}

#[derive(Resource)]
pub struct ArenaBotsGenomeHandle(pub Handle<Genome<ArenaBotsGenome>>);

/// Fallback hardcoded utilisé si genome pas encore chargé au spawn.
fn default_arena_bots() -> ArenaBotsGenome {
    ArenaBotsGenome {
        hp: 30.0,
        body_half_h: 0.65,
        body_radius: 0.40,
        debug_color: [1.0, 0.0, 0.0],
        show_collider_debug: true,
        ai: BotAi {
            shot_range: 35.0,
            shot_cooldown_secs: 1.5,
            shot_damage: 12.0,
            shot_warmup_secs: 0.35,
            detect_range: 50.0,
            shot_jitter_deg: 4.0,
            tracer_emissive: [4.0, 1.5, 0.5],
            speed: 3.5,
            stop_distance: 6.0,
            los_check_hz: 8.0,
            los_grace_secs: 0.35,
            strafe_amplitude_m: 1.8,
            strafe_freq_hz: 0.9,
            strafe_noise_weight: 0.35,
            local_avoid_dist_m: 2.5,
            gunshot_alert_radius_m: 25.0,
            gunshot_alert_los_grace_secs: 0.6,
            alert_duration_secs: 4.0,
            los_lost_grace_secs: 2.0,
        },
        spawn_positions: vec![
            BotSpawn {
                character_glb: String::new(),
                character_scale: 1.0,
                character_yaw_deg: 0.0,
                character_y_offset: 0.9,
                x: -4.0,
                z: -7.0,
                head_y_offset: 1.75,
                head_radius: 0.22,
            },
            BotSpawn {
                character_glb: String::new(),
                character_scale: 1.0,
                character_yaw_deg: 0.0,
                character_y_offset: 0.9,
                x: 0.0,
                z: -7.0,
                head_y_offset: 1.75,
                head_radius: 0.22,
            },
            BotSpawn {
                character_glb: String::new(),
                character_scale: 1.0,
                character_yaw_deg: 0.0,
                character_y_offset: 0.9,
                x: 4.0,
                z: -7.0,
                head_y_offset: 1.75,
                head_radius: 0.22,
            },
            BotSpawn {
                character_glb: String::new(),
                character_scale: 1.0,
                character_yaw_deg: 0.0,
                character_y_offset: 0.9,
                x: 10.0,
                z: -14.0,
                head_y_offset: 1.75,
                head_radius: 0.22,
            },
            BotSpawn {
                character_glb: String::new(),
                character_scale: 1.0,
                character_yaw_deg: 0.0,
                character_y_offset: 0.9,
                x: -14.0,
                z: 10.0,
                head_y_offset: 1.75,
                head_radius: 0.22,
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
///
/// Story-461 (Vague 3) : `#[require(Transform, Visibility)]` garantit que tout
/// spawn de TargetCube insère Transform + Visibility avec Default si non fournis.
#[derive(Component)]
#[require(Transform, Visibility)]
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
        // Waves system — gère spawn de bots par vague (était hardcodé spawn_arena).
        if !app.is_plugin_added::<wave::ArenaWavesPlugin>() {
            app.add_plugins(wave::ArenaWavesPlugin);
        }
        // Story-453 v2 (2026-05-18) — debug render rapier désactivé après validation.
        // Pour ré-activer (wireframes verts sur tous les colliders) :
        //     app.add_plugins(bevy_rapier3d::render::RapierDebugRenderPlugin::default());
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
            )
            .add_systems(
                Update,
                sync_existing_bot_hp.run_if(in_state(GameMode::Fps)),
            )
            // Story-456 — sync TacticalTuning Resource depuis ArenaBotsGenome.[ai] block.
            // Hot-reload : modifier `genomes/arena_bots.toml` → ré-applique tuning runtime.
            .add_systems(Update, sync_tactical_tuning_from_genome);
    }
}

/// Story-456 — pousse les fields tactical du genome [ai] block dans la Resource
/// `TacticalTuning` consommée par `forgia-ai-arena-bot::tactical`. Run sur
/// AssetEvent::Added/Modified (hot-reload).
fn sync_tactical_tuning_from_genome(
    mut events: MessageReader<AssetEvent<Genome<ArenaBotsGenome>>>,
    handle: Option<Res<ArenaBotsGenomeHandle>>,
    assets: Res<Assets<Genome<ArenaBotsGenome>>>,
    mut tuning: ResMut<forgia_ai_arena_bot::TacticalTuning>,
) {
    let Some(handle) = handle else { return };
    let mut should = false;
    for ev in events.read() {
        match ev {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id } => {
                if *id == handle.0.id() {
                    should = true;
                }
            }
            _ => {}
        }
    }
    if !should {
        return;
    }
    let Some(g) = assets.get(&handle.0) else {
        return;
    };
    let ai = &g.data.ai;
    tuning.los_check_hz = ai.los_check_hz;
    tuning.los_grace_secs = ai.los_grace_secs;
    tuning.strafe_amplitude_m = ai.strafe_amplitude_m;
    tuning.strafe_freq_hz = ai.strafe_freq_hz;
    tuning.strafe_noise_weight = ai.strafe_noise_weight;
    tuning.local_avoid_dist_m = ai.local_avoid_dist_m;
    tuning.gunshot_alert_radius_m = ai.gunshot_alert_radius_m;
    tuning.gunshot_alert_los_grace_secs = ai.gunshot_alert_los_grace_secs;
    tuning.alert_duration_secs = ai.alert_duration_secs;
    tuning.los_lost_grace_secs = ai.los_lost_grace_secs;
    info!(
        "[arena-tactical] tuning synced (los_hz {:.1}, strafe_amp {:.2}m, alert_radius {:.1}m)",
        tuning.los_check_hz, tuning.strafe_amplitude_m, tuning.gunshot_alert_radius_m,
    );
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
) {
    let floor: Handle<Scene> = asset_server.load("models/kaykit/dungeon/floor.glb#Scene0");
    let floor_dirt: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/floor_dirt.glb#Scene0");
    let floor_rocks: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/floor_rocks.glb#Scene0");
    let wall: Handle<Scene> = asset_server.load("models/kaykit/dungeon/wall.glb#Scene0");
    let wall_arched: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/wall_arched.glb#Scene0");
    let wall_window: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/wall_window.glb#Scene0");
    let wall_broken: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/wall_broken.glb#Scene0");
    let column: Handle<Scene> = asset_server.load("models/kaykit/dungeon/column.glb#Scene0");
    let pillar: Handle<Scene> = asset_server.load("models/kaykit/dungeon/pillar.glb#Scene0");
    let pillar_deco: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/pillar_deco.glb#Scene0");
    let torch: Handle<Scene> = asset_server.load("models/kaykit/dungeon/torch.glb#Scene0");
    let torch_wall: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/torch_wall.glb#Scene0");
    let banner_red: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/banner_red.glb#Scene0");
    let banner_blue: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/banner_blue.glb#Scene0");
    let banner_yellow: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/banner_yellow.glb#Scene0");
    let chest: Handle<Scene> = asset_server.load("models/kaykit/dungeon/chest.glb#Scene0");
    let chest_gold: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/chest_gold.glb#Scene0");
    let crates: Handle<Scene> = asset_server.load("models/kaykit/dungeon/crates.glb#Scene0");
    let rubble: Handle<Scene> = asset_server.load("models/kaykit/dungeon/rubble.glb#Scene0");
    let table: Handle<Scene> = asset_server.load("models/kaykit/dungeon/table.glb#Scene0");
    let barrel: Handle<Scene> = asset_server.load("models/kaykit/dungeon/barrel.glb#Scene0");
    let barrels_stack: Handle<Scene> =
        asset_server.load("models/kaykit/dungeon/barrels_stack.glb#Scene0");

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

    // LOCK : ne pas supprimer — les floor tiles (SceneRoot pur) n'ont PAS de collider individuel.
    // Ce cuboid global est l'unique sol physique de l'arena. Garder Cuboid primitif (vs AsyncSceneCollider)
    // car aucun mesh sol n'a besoin de précision — un grand plan plat suffit.
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
        let nord = if i == 0 {
            wall_arched.clone()
        } else {
            pick_wall(i)
        };
        let sud = if i == 0 {
            wall_arched.clone()
        } else {
            pick_wall(i + 1)
        };
        let est = if i == 0 {
            wall_arched.clone()
        } else {
            pick_wall(i + 2)
        };
        let ouest = if i == 0 {
            wall_arched.clone()
        } else {
            pick_wall(i + 3)
        };

        spawn_wall(&mut commands, &nord, Vec3::new(offset, 0.0, edge), 0.0);
        spawn_wall(
            &mut commands,
            &sud,
            Vec3::new(offset, 0.0, -edge),
            std::f32::consts::PI,
        );
        spawn_wall(
            &mut commands,
            &est,
            Vec3::new(edge, 0.0, offset),
            -std::f32::consts::FRAC_PI_2,
        );
        spawn_wall(
            &mut commands,
            &ouest,
            Vec3::new(-edge, 0.0, offset),
            std::f32::consts::FRAC_PI_2,
        );
    }

    // Story-453 — Cylinder primitives simples (vs TriMesh). Bottom = Y=0.
    let pillar_half_h = 2.0;
    let pillar_radius = 0.5;
    let col_d = TILE_SIZE * 2.5;
    for &(x, z) in &[
        (col_d, col_d),
        (-col_d, col_d),
        (col_d, -col_d),
        (-col_d, -col_d),
    ] {
        commands
            .spawn((
                ArenaMarker,
                Transform::from_xyz(x, pillar_half_h, z),
                Visibility::default(),
                RigidBody::Fixed,
                Collider::cylinder(pillar_half_h, pillar_radius),
                Name::new(format!("CenterPillar_{x}_{z}")),
            ))
            .with_children(|p| {
                p.spawn((
                    SceneRoot(pillar_deco.clone()),
                    Transform::from_xyz(0.0, -pillar_half_h, 0.0),
                ));
            });
    }

    let outer_d = TILE_SIZE * 4.5;
    for &(x, z) in &[
        (outer_d, 0.0),
        (-outer_d, 0.0),
        (0.0, outer_d),
        (0.0, -outer_d),
    ] {
        commands
            .spawn((
                ArenaMarker,
                Transform::from_xyz(x, pillar_half_h, z),
                Visibility::default(),
                RigidBody::Fixed,
                Collider::cylinder(pillar_half_h, pillar_radius),
                Name::new(format!("OuterPillar_{x}_{z}")),
            ))
            .with_children(|p| {
                p.spawn((
                    SceneRoot(pillar.clone()),
                    Transform::from_xyz(0.0, -pillar_half_h, 0.0),
                ));
            });
    }

    let col_radius = 0.4;
    for &(x, z) in &[(col_d, 0.0), (-col_d, 0.0), (0.0, col_d), (0.0, -col_d)] {
        commands
            .spawn((
                ArenaMarker,
                Transform::from_xyz(x, pillar_half_h, z),
                Visibility::default(),
                RigidBody::Fixed,
                Collider::cylinder(pillar_half_h, col_radius),
                Name::new(format!("MidColumn_{x}_{z}")),
            ))
            .with_children(|p| {
                p.spawn((
                    SceneRoot(column.clone()),
                    Transform::from_xyz(0.0, -pillar_half_h, 0.0),
                ));
            });
    }

    // Story-453 — cover props : Cuboid primitives, half_h adapté par type.
    // Format : (name, x, z, scene, half_x, half_y, half_z)
    let cover_props: &[(&str, f32, f32, &Handle<Scene>, f32, f32, f32)] = &[
        ("Crates_NE", 8.0, -8.0, &crates, 0.75, 0.75, 0.75),
        ("Crates_SW", -8.0, 8.0, &crates, 0.75, 0.75, 0.75),
        ("Rubble_N", 0.0, -14.0, &rubble, 1.0, 0.3, 1.0),
        ("Rubble_S", 0.0, 14.0, &rubble, 1.0, 0.3, 1.0),
        ("Rubble_E", 14.0, 0.0, &rubble, 1.0, 0.3, 1.0),
        ("Rubble_W", -14.0, 0.0, &rubble, 1.0, 0.3, 1.0),
        (
            "BarrelStack_NW",
            -10.0,
            -10.0,
            &barrels_stack,
            0.7,
            1.0,
            0.7,
        ),
        ("BarrelStack_SE", 10.0, 10.0, &barrels_stack, 0.7, 1.0, 0.7),
        ("Table_E", 6.0, 4.0, &table, 0.8, 0.5, 0.4),
        ("Table_W", -6.0, -4.0, &table, 0.8, 0.5, 0.4),
        ("Barrel_1", 3.0, 12.0, &barrel, 0.4, 0.5, 0.4),
        ("Barrel_2", -3.0, -12.0, &barrel, 0.4, 0.5, 0.4),
        ("Barrel_3", 12.0, -3.0, &barrel, 0.4, 0.5, 0.4),
        ("Barrel_4", -12.0, 3.0, &barrel, 0.4, 0.5, 0.4),
    ];
    for (name, x, z, scene, half_x, half_y, half_z) in cover_props {
        commands
            .spawn((
                ArenaMarker,
                Transform::from_xyz(*x, *half_y, *z),
                Visibility::default(),
                RigidBody::Fixed,
                Collider::cuboid(*half_x, *half_y, *half_z),
                Name::new(name.to_string()),
            ))
            .with_children(|p| {
                p.spawn((
                    SceneRoot((*scene).clone()),
                    Transform::from_xyz(0.0, -half_y, 0.0),
                ));
            });
    }

    let chest_half = (0.6, 0.4, 0.4);
    commands
        .spawn((
            ArenaMarker,
            Transform::from_xyz(16.0, chest_half.1, 16.0),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cuboid(chest_half.0, chest_half.1, chest_half.2),
            Name::new("ChestGold_NE"),
        ))
        .with_children(|p| {
            p.spawn((
                SceneRoot(chest_gold),
                Transform::from_xyz(0.0, -chest_half.1, 0.0),
            ));
        });
    commands
        .spawn((
            ArenaMarker,
            Transform::from_xyz(-16.0, chest_half.1, -16.0),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cuboid(chest_half.0, chest_half.1, chest_half.2),
            Name::new("Chest_SW"),
        ))
        .with_children(|p| {
            p.spawn((
                SceneRoot(chest),
                Transform::from_xyz(0.0, -chest_half.1, 0.0),
            ));
        });

    let banners: &[(&str, f32, f32, Handle<Scene>, f32)] = &[
        ("Banner_N_red", -6.0, edge - 0.3, banner_red.clone(), 0.0),
        ("Banner_N_blue", 6.0, edge - 0.3, banner_blue.clone(), 0.0),
        (
            "Banner_S_yellow",
            0.0,
            -edge + 0.3,
            banner_yellow,
            std::f32::consts::PI,
        ),
        (
            "Banner_E_red",
            edge - 0.3,
            -6.0,
            banner_red,
            -std::f32::consts::FRAC_PI_2,
        ),
        (
            "Banner_W_blue",
            -edge + 0.3,
            6.0,
            banner_blue,
            std::f32::consts::FRAC_PI_2,
        ),
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

    // Bots = spawned par `wave::wave_orchestrator` (système wave-driven, voir wave.rs).
    // Cette fonction ne fait QUE le décor statique (sol, walls, lights, clouds).

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
                CloudOrbit {
                    angle,
                    radius,
                    height: cy,
                },
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
        "[forgia-mode-fps-arena] Arena spawned : {}×{}m, KayKit modular + 18 cloud clusters (bots = wave system)",
        (ARENA_SIZE as f32 * TILE_SIZE) as i32,
        (ARENA_SIZE as f32 * TILE_SIZE) as i32,
    );
}

/// Story-453 floor sink fix (2026-05-18) — revert TriMesh à Cuboid simple.
/// TriMesh KayKit avait des vertices sous Y=0 → player kinematic sink.
/// Cuboid posé avec center_y = 1.5 → bottom Y=0, top Y=3.0 (couvre mesh visuel).
/// Mesh visuel reste rendu correctement à pos.y (généralement 0).
fn spawn_wall(commands: &mut Commands, wall_scene: &Handle<Scene>, pos: Vec3, yaw: f32) {
    let wall_half_h = 1.5;
    let collider_pos = Vec3::new(pos.x, pos.y + wall_half_h, pos.z);
    commands
        .spawn((
            ArenaMarker,
            Transform::from_translation(collider_pos).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            RigidBody::Fixed,
            Collider::cuboid(TILE_SIZE / 2.0, wall_half_h, 0.15),
            Name::new(format!("Wall_{:.0}_{:.0}", pos.x, pos.z)),
        ))
        .with_children(|p| {
            // Mesh visuel : enfant, offset Y négatif pour compenser la position parent.
            p.spawn((
                SceneRoot(wall_scene.clone()),
                Transform::from_xyz(0.0, -wall_half_h, 0.0),
            ));
        });
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
        assert_eq!(
            ARENA_SIZE % 2,
            1,
            "ARENA_SIZE must be odd for centered grid"
        );
    }
}
