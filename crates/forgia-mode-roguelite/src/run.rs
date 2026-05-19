//! run.rs — RunState SubStates + Events + RunSeed (V7 M1).
//!
//! Story-470. Pattern Bevy 0.18 :
//! - `SubStates` avec `#[source(GameMode = GameMode::Roguelite)]` (idiom 0.18)
//! - `Message` derive (renommé Event en 0.18 PR #19596) — story-468 §0.1 BufferedEvent
//! - `RunSeed` xoshiro déterministe (audit §0.2 — host-authoritative, pas float det rapier)
//!
//! Cleanup OnExit : `RogueliteRunMarker` Component stub — la logique de despawn est
//! gérée par un terminal parallèle (V7 dédié cleanup orchestration). Ce fichier
//! n'inclut PAS le system `sys_cleanup_run_markers` pour éviter conflit merge.

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_rapier3d::prelude::{Collider, RigidBody};
use forgia_core::prelude::*;
use forgia_damage::DeathEvent;
use forgia_loot_tables::{Pickup, PickupCollector};
use forgia_player::Player;
use rand_xoshiro::Xoshiro256StarStar;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};

/// SubState de `GameMode::Roguelite` — flow de la run en cours.
#[derive(SubStates, Default, Debug, Clone, PartialEq, Eq, Hash)]
#[source(GameMode = GameMode::Roguelite)]
pub enum RunState {
    /// Lobby pré-run : choix seed, options, ready check.
    #[default]
    Lobby,
    /// Run en cours sur le stage donné (0-indexed).
    InRun { stage: u8 },
    /// Combat boss sur le stage donné.
    Boss { stage: u8 },
    /// Défaite — retour menu après écran result.
    Defeat,
    /// Victoire — retour menu après écran result + récompenses.
    Victory,
}

/// Démarre une run. `seed = None` → seed dérivé timestamp (random).
#[derive(Message, Debug, Clone)]
pub struct StartRunEvent {
    pub seed: Option<u64>,
}

/// Termine la run avec résultat. Trigger transition RunState.
#[derive(Message, Debug, Clone)]
pub struct EndRunEvent {
    pub result: RunResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunResult {
    Victory,
    Defeat,
    /// Abandon volontaire (return-to-lobby manuel).
    Abort,
}

/// Seed déterministe + état stage. Inséré au démarrage par `sys_start_run`.
#[derive(Resource, Debug, Clone)]
pub struct RunSeed {
    pub seed: u64,
    pub stage_count: u8,
}

impl RunSeed {
    /// Dérivation déterministe par stage : `seed XOR (stage * golden_ratio_u64)`.
    pub fn stage_seed(&self, stage: u8) -> u64 {
        let mixed = self.seed ^ u64::from(stage).wrapping_mul(0x9E3779B97F4A7C15);
        let mut rng = Xoshiro256StarStar::seed_from_u64(mixed);
        rng.next_u64()
    }

    /// Dérivation déterministe par encounter dans un stage.
    pub fn encounter_seed(&self, stage: u8, encounter_idx: u32) -> u64 {
        let mixed = self.stage_seed(stage)
            ^ u64::from(encounter_idx).wrapping_mul(0xBF58476D1CE4E5B9);
        let mut rng = Xoshiro256StarStar::seed_from_u64(mixed);
        rng.next_u64()
    }
}

/// Marker stub — placé sur toutes les entités à cleaner OnExit GameMode::Roguelite.
/// La logique de cleanup (`sys_cleanup_run_markers`) est gérée par un terminal
/// parallèle dédié — ne PAS dupliquer ici.
///
/// En attendant, les entités spawn par `sys_spawn_roguelite_scene` utilisent
/// `DespawnOnExit(GameMode::Roguelite)` Bevy 0.18 natif pour safety cleanup.
#[derive(Component, Default)]
pub struct RogueliteRunMarker;

/// Story-470 M1.5c — vraie zone de départ Roguelite :
/// - Floor 300×300m (collider rapier + mesh visuel matching)
/// - Murs périmètre 4 walls (300m × 6m haut) anti-fall-off
/// - 5 plateformes raised (combat verticality, RoR2-style)
/// - 25 cover obstacles xoshiro-seeded déterministe (placement = run identity)
/// - 3 landmarks couleurs distincts (orientation visuelle joueur)
/// - Sun directional + ambient bleu-gris (atmosphère arena)
///
/// Placement obstacles : `Xoshiro256StarStar::seed_from_u64(SCENE_SEED_CONST)` —
/// déterministe, reproductible. M2 remplacera par procgen biome-based.
///
/// Despawn auto OnExit(GameMode::Roguelite) via Bevy 0.18 `DespawnOnExit`.
pub fn sys_spawn_roguelite_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    const FLOOR_HALF: f32 = 150.0; // floor 300×300m
    const WALL_HEIGHT: f32 = 6.0;
    const WALL_THICKNESS: f32 = 1.0;
    const SCENE_SEED: u64 = 0xC0FF_EE51_C0BA_1700;

    // ─────── Floor 300×300m ────────────────────────────────────────────────
    let floor_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.28, 0.26, 0.24),
        perceptual_roughness: 0.92,
        ..default()
    });
    commands.spawn((
        Name::new("RogueliteFloor"),
        RogueliteRunMarker,
        DespawnOnExit(GameMode::Roguelite),
        Mesh3d(meshes.add(Plane3d::default().mesh().size(FLOOR_HALF * 2.0, FLOOR_HALF * 2.0))),
        MeshMaterial3d(floor_mat),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RigidBody::Fixed,
        Collider::cuboid(FLOOR_HALF, 0.1, FLOOR_HALF),
    ));

    // ─────── 4 murs périmètre (anti-fall-off) ─────────────────────────────
    let wall_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.32, 0.30),
        perceptual_roughness: 0.85,
        ..default()
    });
    let wall_mesh = meshes.add(Cuboid::new(
        FLOOR_HALF * 2.0,
        WALL_HEIGHT,
        WALL_THICKNESS,
    ));
    for (idx, (offset_z, rot_y)) in [
        (FLOOR_HALF, 0.0),                // mur Sud (+Z)
        (-FLOOR_HALF, std::f32::consts::PI), // mur Nord (-Z)
    ]
    .iter()
    .enumerate()
    {
        commands.spawn((
            Name::new(format!("RogueliteWall_NS_{idx}")),
            RogueliteRunMarker,
            DespawnOnExit(GameMode::Roguelite),
            Mesh3d(wall_mesh.clone()),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_xyz(0.0, WALL_HEIGHT / 2.0, *offset_z)
                .with_rotation(Quat::from_rotation_y(*rot_y)),
            RigidBody::Fixed,
            Collider::cuboid(FLOOR_HALF, WALL_HEIGHT / 2.0, WALL_THICKNESS / 2.0),
        ));
    }
    let wall_mesh_ew = meshes.add(Cuboid::new(
        WALL_THICKNESS,
        WALL_HEIGHT,
        FLOOR_HALF * 2.0,
    ));
    for (idx, offset_x) in [FLOOR_HALF, -FLOOR_HALF].iter().enumerate() {
        commands.spawn((
            Name::new(format!("RogueliteWall_EW_{idx}")),
            RogueliteRunMarker,
            DespawnOnExit(GameMode::Roguelite),
            Mesh3d(wall_mesh_ew.clone()),
            MeshMaterial3d(wall_mat.clone()),
            Transform::from_xyz(*offset_x, WALL_HEIGHT / 2.0, 0.0),
            RigidBody::Fixed,
            Collider::cuboid(WALL_THICKNESS / 2.0, WALL_HEIGHT / 2.0, FLOOR_HALF),
        ));
    }

    // ─────── 5 plateformes raised (verticality) ────────────────────────────
    let platform_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.40, 0.36),
        perceptual_roughness: 0.80,
        ..default()
    });
    let platforms: &[(Vec3, Vec3)] = &[
        (Vec3::new(40.0, 1.5, 30.0), Vec3::new(12.0, 3.0, 12.0)),
        (Vec3::new(-50.0, 2.5, 20.0), Vec3::new(14.0, 5.0, 14.0)),
        (Vec3::new(60.0, 2.0, -50.0), Vec3::new(10.0, 4.0, 10.0)),
        (Vec3::new(-30.0, 3.5, -60.0), Vec3::new(16.0, 7.0, 16.0)),
        (Vec3::new(0.0, 1.0, 80.0), Vec3::new(20.0, 2.0, 8.0)),
    ];
    for (idx, (pos, size)) in platforms.iter().enumerate() {
        let half = *size / 2.0;
        commands.spawn((
            Name::new(format!("RoguelitePlatform_{idx}")),
            RogueliteRunMarker,
            DespawnOnExit(GameMode::Roguelite),
            Mesh3d(meshes.add(Cuboid::from_size(*size))),
            MeshMaterial3d(platform_mat.clone()),
            Transform::from_translation(*pos),
            RigidBody::Fixed,
            Collider::cuboid(half.x, half.y, half.z),
        ));
    }

    // ─────── 25 cover obstacles xoshiro-seeded ─────────────────────────────
    let cover_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.50, 0.46, 0.40),
        perceptual_roughness: 0.88,
        ..default()
    });
    let mut rng = Xoshiro256StarStar::seed_from_u64(SCENE_SEED);
    let spawn_clearance: f32 = 12.0; // pas de cover dans cercle 12m autour origin (player spawn)
    let mut spawned = 0u32;
    let mut attempts = 0u32;
    while spawned < 25 && attempts < 200 {
        attempts += 1;
        // x, z dans [-FLOOR_HALF+10, FLOOR_HALF-10]
        let x = (rng.next_u64() as f64 / u64::MAX as f64 * 2.0 - 1.0) as f32
            * (FLOOR_HALF - 10.0);
        let z = (rng.next_u64() as f64 / u64::MAX as f64 * 2.0 - 1.0) as f32
            * (FLOOR_HALF - 10.0);
        if x * x + z * z < spawn_clearance * spawn_clearance {
            continue; // skip si trop proche du spawn
        }
        // Taille 1.5m-4m (cube)
        let s = 1.5 + (rng.next_u64() as f64 / u64::MAX as f64) as f32 * 2.5;
        let half_s = s / 2.0;
        commands.spawn((
            Name::new(format!("RogueliteCover_{spawned}")),
            RogueliteRunMarker,
            DespawnOnExit(GameMode::Roguelite),
            Mesh3d(meshes.add(Cuboid::from_size(Vec3::splat(s)))),
            MeshMaterial3d(cover_mat.clone()),
            Transform::from_xyz(x, half_s, z),
            RigidBody::Fixed,
            Collider::cuboid(half_s, half_s, half_s),
        ));
        spawned += 1;
    }

    // ─────── 3 landmarks colorés (orientation visuelle) ────────────────────
    let landmarks: &[(Vec3, Color)] = &[
        (Vec3::new(100.0, 4.0, 0.0), Color::srgb(0.95, 0.30, 0.30)), // rouge Est
        (Vec3::new(0.0, 4.0, 100.0), Color::srgb(0.30, 0.80, 0.95)), // bleu Sud
        (Vec3::new(-100.0, 4.0, 0.0), Color::srgb(0.95, 0.85, 0.20)), // jaune Ouest
    ];
    for (idx, (pos, color)) in landmarks.iter().enumerate() {
        let emissive_lin = color.to_srgba();
        commands.spawn((
            Name::new(format!("RogueliteLandmark_{idx}")),
            RogueliteRunMarker,
            DespawnOnExit(GameMode::Roguelite),
            Mesh3d(meshes.add(Cuboid::new(3.0, 8.0, 3.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: *color,
                emissive: LinearRgba::new(
                    emissive_lin.red * 0.6,
                    emissive_lin.green * 0.6,
                    emissive_lin.blue * 0.6,
                    1.0,
                ),
                ..default()
            })),
            Transform::from_translation(*pos),
            RigidBody::Fixed,
            Collider::cuboid(1.5, 4.0, 1.5),
        ));
    }

    // ─────── Sun (directional light) ───────────────────────────────────────
    commands.spawn((
        Name::new("RogueliteSun"),
        RogueliteRunMarker,
        DespawnOnExit(GameMode::Roguelite),
        DirectionalLight {
            illuminance: 12_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(50.0, 100.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // ─────── Wave 1 enemies (M2 step 4 — orchestré par waves.rs) ─────────
    let enemy_total =
        crate::waves::spawn_wave_enemies(&mut commands, &mut meshes, &mut materials, 1);

    info!(
        "[roguelite] Scene spawned : floor 300m + 4 walls + 5 platforms + {spawned} cover + 3 landmarks + {enemy_total} enemies wave 1 (seed={SCENE_SEED:#x})"
    );
}

/// Observer Bevy 0.18 — sur DeathEvent d'un ennemi Roguelite, spawn un Pickup
/// glowing à sa position. Value selon archetype (Tank > Sniper > Runner).
///
/// Pattern miroir : `forgia-ai-arena-bot::on_bot_death` (story-466).
pub fn obs_roguelite_enemy_death(
    event: On<DeathEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    enemies_q: Query<(&Transform, &crate::EnemyArchetype)>,
) {
    let target = event.target;
    let Ok((xf, archetype)) = enemies_q.get(target) else {
        return; // pas un ennemi Roguelite (probablement bot Arena → ignore)
    };

    let value = match *archetype {
        crate::EnemyArchetype::Tank => 5,
        crate::EnemyArchetype::Runner => 2,
        crate::EnemyArchetype::Sniper => 3,
    };

    let pos = xf.translation.with_y(0.6);
    commands.spawn((
        Name::new(format!("RoguelitePickup_{value}souls")),
        RogueliteRunMarker,
        DespawnOnExit(GameMode::Roguelite),
        Pickup {
            value,
            lifetime_secs: 30.0,
            collect_radius: 2.5,
        },
        Mesh3d(meshes.add(Sphere::new(0.35))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.30),
            emissive: LinearRgba::new(0.80, 0.55, 0.10, 1.0),
            metallic: 0.7,
            perceptual_roughness: 0.25,
            ..default()
        })),
        Transform::from_translation(pos),
    ));
}

/// OnEnter(GameMode::Roguelite) — tag le Player avec PickupCollector pour que
/// `forgia_loot_tables::sys_collect_pickups` puisse trigger sur sa position.
/// Player est spawn par forgia-player::OnEnter(AppMode::InGame) (cross-mode).
pub fn sys_tag_player_as_collector(
    mut commands: Commands,
    q_player: Query<Entity, (With<Player>, Without<PickupCollector>)>,
) {
    for e in &q_player {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.insert(PickupCollector);
        }
    }
}

pub fn sys_start_run(
    mut events: MessageReader<StartRunEvent>,
    mut next: ResMut<NextState<RunState>>,
    mut commands: Commands,
) {
    for ev in events.read() {
        let seed = ev.seed.unwrap_or_else(default_seed_from_clock);
        commands.insert_resource(RunSeed {
            seed,
            stage_count: 0,
        });
        next.set(RunState::InRun { stage: 0 });
        info!("[roguelite] Run started — seed={seed}");
    }
}

pub fn sys_end_run(
    mut events: MessageReader<EndRunEvent>,
    mut next: ResMut<NextState<RunState>>,
) {
    for ev in events.read() {
        let state = match ev.result {
            RunResult::Victory => RunState::Victory,
            RunResult::Defeat => RunState::Defeat,
            RunResult::Abort => RunState::Lobby,
        };
        next.set(state);
        info!("[roguelite] Run ended — {:?}", ev.result);
    }
}

/// Seed depuis l'horloge système (nanos) — fallback si user ne fournit pas.
/// Pas crypto-secure, suffisant pour seed déterministe roguelite.
fn default_seed_from_clock() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_nanos() & u128::from(u64::MAX)) as u64)
        .unwrap_or(0xC0FF_EEDE_ADBE_EF00)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runstate_default_is_lobby() {
        assert_eq!(RunState::default(), RunState::Lobby);
    }

    #[test]
    fn run_seed_stage_seed_deterministic() {
        let a = RunSeed {
            seed: 42,
            stage_count: 0,
        };
        let b = RunSeed {
            seed: 42,
            stage_count: 5,
        }; // stage_count NOT used in derive
        assert_eq!(a.stage_seed(3), b.stage_seed(3));
        assert_eq!(a.stage_seed(0), b.stage_seed(0));
    }

    #[test]
    fn run_seed_different_seeds_diverge() {
        let a = RunSeed {
            seed: 42,
            stage_count: 0,
        };
        let b = RunSeed {
            seed: 43,
            stage_count: 0,
        };
        assert_ne!(a.stage_seed(0), b.stage_seed(0));
    }

    #[test]
    fn encounter_seed_deterministic() {
        let s = RunSeed {
            seed: 0xCAFE,
            stage_count: 0,
        };
        assert_eq!(s.encounter_seed(2, 7), s.encounter_seed(2, 7));
        assert_ne!(s.encounter_seed(2, 7), s.encounter_seed(2, 8));
    }

    #[test]
    fn run_result_variants_distinct() {
        assert_ne!(RunResult::Victory, RunResult::Defeat);
        assert_ne!(RunResult::Defeat, RunResult::Abort);
    }
}
