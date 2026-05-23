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
use forgia_core::prelude::*;
// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// use forgia_audio_voicelines::{BarkEvent, BarkKind};
use forgia_combat::prelude::{EquippedWeapons, WeaponType};
use forgia_damage::DeathEvent;
// TODO(story-471..479): PickupAnimState + PickupKind supprimés de forgia_loot_tables
use forgia_loot_tables::{Pickup, PickupCollector};
use forgia_player::Player;
use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;

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
        let mixed =
            self.stage_seed(stage) ^ u64::from(encounter_idx).wrapping_mul(0xBF58476D1CE4E5B9);
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

/// Mapping V1 depth → stage_id. Pattern simple alternance + boss force volcanique.
///
/// Story-483 P2 (2026-05-20). V7 ne shippe que 2 stages TOML (`crypts_of_anvil`
/// + `forge_sanctum`). Mapping :
/// - Boss state → toujours `crypts_of_anvil` (le seul stage avec `boss_pad_required`)
/// - Pair depths (0, 2, ...) → `crypts_of_anvil` (Volcanic, boss-ready)
/// - Impair depths (1, 3, ...) → `forge_sanctum` (Plains, lighter biome)
///
/// V2 : étendre `roguelite_stages.toml` + remplacer ce mapping par lookup
/// `RunGraph::stages[depth].variants[chosen].stage_id_pool` (data-driven).
pub fn stage_id_for_depth(depth: u8, is_boss: bool) -> &'static str {
    if is_boss {
        return "crypts_of_anvil";
    }
    if depth.is_multiple_of(2) {
        "crypts_of_anvil"
    } else {
        "forge_sanctum"
    }
}

/// Story-483 V7 P2 (2026-05-20) — dispatcher stage_id sur transition RunState.
///
/// Watches `Res<State<RunState>>` et insère un `StageLoadRequest` dès qu'on
/// entre dans `InRun{stage}` ou `Boss{stage}`. Lobby = depth 0 (pre-run
/// visual). Defeat/Victory ne déclenchent rien (laisse stage visible jusqu'à
/// OnExit GameMode cleanup).
///
/// Idempotent : `Local<Option<(u8, bool)>>` track last_depth → no-op si
/// inchangé. Le spawn system côté stage-arena détecte stage_id change et
/// cleanup les entités du stage précédent avant spawn du nouveau.
pub fn sys_stage_dispatch(
    mut commands: Commands,
    run_state: Option<Res<State<RunState>>>,
    run_seed: Option<Res<RunSeed>>,
    mut last_depth: Local<Option<(u8, bool)>>,
) {
    let Some(state) = run_state.as_deref().map(|s| s.get()) else {
        return;
    };
    let key = match state {
        RunState::Lobby => Some((0u8, false)),
        RunState::InRun { stage } => Some((*stage, false)),
        RunState::Boss { stage } => Some((*stage, true)),
        // Defeat/Victory : keep last stage visible (no-op).
        _ => return,
    };
    if key == *last_depth {
        return;
    }
    let Some((depth, is_boss)) = key else {
        return;
    };
    let stage_id = stage_id_for_depth(depth, is_boss);
    const FALLBACK_SEED: u64 = 0xC0FF_EE51_C0BA_1700;
    let seed = run_seed
        .as_ref()
        .map(|s| s.stage_seed(depth))
        .unwrap_or(FALLBACK_SEED);
    commands.insert_resource(forgia_stage_arena::StageLoadRequest {
        stage_id: stage_id.to_string(),
        seed,
    });
    *last_depth = key;
    info!(
        "[roguelite] Stage dispatch → '{stage_id}' (depth={depth}, boss={is_boss}, seed={seed:#x})"
    );
}

// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// parse_music_state désactivé : MusicState n'existe plus dans forgia_audio_music_state (scaffold vide).
// Stub retourne toujours None pour éviter les callsites de casser.
pub fn parse_music_state(_s: &str) -> Option<()> {
    None
}

// TODO(story-471..479): API removed, refactor abandonné — re-implémenter
// sys_apply_stage_toggles désactivé : RequestMusicState n'existe plus dans
// forgia_audio_music_state (scaffold vide). Stub no-op.
pub fn sys_apply_stage_toggles(
    stage_result: Res<forgia_stage_arena::StageLoadResult>,
    mut last_applied_id: Local<String>,
) {
    if stage_result.state != forgia_stage_arena::StageState::Ready {
        return;
    }
    if stage_result.stage_id == *last_applied_id || stage_result.stage_id.is_empty() {
        return;
    }
    // Music toggle disabled — MusicState/RequestMusicState supprimés.
    // Weather override : log seulement (pas de consumer V1, future `forgia-weather`).
    if !stage_result.weather_override.is_empty() {
        info!(
            "[roguelite] Stage '{}' weather_override='{}' (consumer V2)",
            stage_result.stage_id, stage_result.weather_override
        );
    }
    *last_applied_id = stage_result.stage_id.clone();
}

/// Story-483 V7 P2 (2026-05-20) — `OnEnter(GameMode::Roguelite)` minimal.
///
/// Stations health/ammo (cross-stage gameplay, pas map-bound). Le stage
/// arena lui-même est piloté par `sys_stage_dispatch` (Update, watches
/// RunState transitions).
pub fn sys_spawn_roguelite_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    crate::stations::spawn_stations(&mut commands, &mut meshes, &mut materials);
}

/// V7 M3 step 4 (2026-05-20) — Observer DeathEvent ciblant le Player → Defeat.
///
/// Pipeline : bot tire (BotShootConfig) → `apply_damage` mute Health → trigger
/// DeathEvent quand HP=0 → cet observer match `target == Player entity` et émet
/// `EndRunEvent(Defeat)`. `sys_end_run` transitionne `RunState::Defeat`, ce qui
/// déclenche `draw_defeat_overlay` (gated). Pattern Hadès "die → return to lobby".
///
/// Idempotent : la victory_emitted latch dans RogueliteWave bloque double-emit
/// si trigger arrive après une Victory déjà émise.
pub fn obs_roguelite_player_death(
    event: On<DeathEvent>,
    q_player: Query<Entity, With<Player>>,
    run_state: Option<Res<State<RunState>>>,
    mut end_run: MessageWriter<EndRunEvent>,
    mut wave: ResMut<crate::waves::RogueliteWave>,
) {
    // Filter : death cible le Player (pas un bot).
    let Ok(player_entity) = q_player.single() else {
        return;
    };
    if event.target != player_entity {
        return;
    }
    // Gate sur run active (Lobby/Defeat/Victory : ignore).
    let active = matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    );
    if !active {
        return;
    }
    // Anti double-emit (si Victory déjà fired = run finie, n'override pas).
    if wave.victory_emitted {
        return;
    }
    wave.victory_emitted = true; // bloque transitions further en orchestrator
    end_run.write(EndRunEvent {
        result: RunResult::Defeat,
    });
    info!("[roguelite] Player died — DEFEAT");
}

/// Observer Bevy 0.18 — sur DeathEvent d'un ennemi Roguelite, spawn un Pickup
/// glowing à sa position. Value selon archetype (Tank > Sniper > Runner).
///
/// Pattern miroir : `forgia-ai-arena-bot::on_bot_death` (story-466).
#[allow(clippy::too_many_arguments)]
pub fn obs_roguelite_enemy_death(
    event: On<DeathEvent>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    enemies_q: Query<(&Transform, &crate::EnemyArchetype)>,
    time: Res<Time>,
    q_player_hp: Query<&forgia_damage::Health, With<forgia_player::Player>>,
    equipped: Option<Res<EquippedWeapons>>,
) {
    let target = event.target;
    let Ok((xf, archetype)) = enemies_q.get(target) else {
        return; // pas un ennemi Roguelite (probablement bot Arena → ignore)
    };

    // TODO(story-471..479): API removed, refactor abandonné — re-implémenter
    // BarkEvent/BarkKind supprimés de forgia_audio_voicelines (scaffold vide).
    // Le bloc bark est désactivé ci-dessous.
    let _ = &equipped; // suppress unused warning

    // V7 M3 step 3 — Heart drop scaled par player HP (low HP = chance plus haute).
    // Boss garantit toujours un Heart big. Sources : Hadès "Centaur Hearts" + RoR2.
    let player_hp_frac = q_player_hp
        .iter()
        .next()
        .map(|h| if h.max > 0.0 { h.current / h.max } else { 1.0 })
        .unwrap_or(1.0);

    // Seed pseudo-deterministic from time + entity bits (cheap xorshift).
    let entity_seed: u64 = target.to_bits();
    let mut seed =
        (time.elapsed_secs_f64() * 1000.0) as u64 ^ entity_seed.wrapping_mul(2_654_435_761);
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    let roll = (seed % 100) as u32; // 0..99

    // TODO(story-471..479): PickupKind supprimé de forgia_loot_tables — kind remplacé par bool is_heart
    let (is_heart, value, color, emissive) = match *archetype {
        crate::EnemyArchetype::Boss => (
            true,
            40_u32, // big heart : ~33% of max HP (player max 100).
            (1.0_f32, 0.30_f32, 0.30_f32),
            LinearRgba::new(1.8, 0.20, 0.20, 1.0),
        ),
        _ if player_hp_frac < 0.40 && roll < 35 => (
            // Low HP + 35% chance → heart drop (Hadès Centaur pattern).
            true,
            20,
            (1.0, 0.30, 0.30),
            LinearRgba::new(1.6, 0.20, 0.20, 1.0),
        ),
        crate::EnemyArchetype::Tank => (
            false,
            5,
            (0.55, 0.80, 1.0),
            LinearRgba::new(0.45, 0.85, 1.6, 1.0),
        ),
        crate::EnemyArchetype::Sniper => (
            false,
            3,
            (0.55, 0.80, 1.0),
            LinearRgba::new(0.45, 0.85, 1.6, 1.0),
        ),
        crate::EnemyArchetype::Runner => (
            false,
            2,
            (0.55, 0.80, 1.0),
            LinearRgba::new(0.45, 0.85, 1.6, 1.0),
        ),
    };

    let base_y = 0.85;
    let pos = xf.translation.with_y(base_y);
    // TODO(story-471..479): PickupKind supprimé — is_heart bool remplace kind == PickupKind::Heart
    let core_radius = if is_heart { 0.35 } else { 0.28 };
    let halo_radius = if is_heart { 0.70 } else { 0.55 };

    let core_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(color.0, color.1, color.2, 1.0),
        emissive,
        metallic: 0.0,
        perceptual_roughness: 0.4,
        ..default()
    });
    let halo_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(color.0 * 0.6, color.1 * 0.6, color.2 * 0.6, 0.30),
        emissive: LinearRgba::new(
            emissive.red * 0.5,
            emissive.green * 0.5,
            emissive.blue * 0.5,
            1.0,
        ),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        unlit: true,
        ..default()
    });
    let core_mesh = meshes.add(Sphere::new(core_radius));
    let halo_mesh = meshes.add(Sphere::new(halo_radius));

    // TODO(story-471..479): PickupKind + PickupAnimState supprimés de forgia_loot_tables
    let label = if is_heart {
        format!("RoguelitePickup_heart{value}")
    } else {
        format!("RoguelitePickup_{value}souls")
    };
    let parent_id = commands
        .spawn((
            Name::new(label),
            RogueliteRunMarker,
            DespawnOnExit(GameMode::Roguelite),
            Pickup {
                value,
                lifetime_secs: 30.0,
                collect_radius: 2.5,
            },
            // PickupAnimState supprimé — disabled
            Mesh3d(core_mesh),
            MeshMaterial3d(core_mat),
            Transform::from_translation(pos),
        ))
        .id();
    commands.spawn((
        ChildOf(parent_id),
        Name::new("PickupHalo"),
        Mesh3d(halo_mesh),
        MeshMaterial3d(halo_mat),
        Transform::default(),
    ));
}

/// Tag le Player avec PickupCollector pour que `forgia_loot_tables::sys_collect_pickups`
/// puisse trigger sur sa position.
///
/// V7 M2.5 (2026-05-20) — DOIT tourner en `Update + run_if(in_state(Roguelite))`,
/// PAS en `OnEnter`. Player est spawn par `forgia-player::OnEnter(AppMode::InGame)`
/// (autre plugin) et l'ordre OnEnter cross-plugin n'est PAS garanti par Bevy 0.18.
/// Pattern récurrent : voir memory `feedback_cross_plugin_onenter_race_pattern.md`.
/// Guard idempotent via `Without<PickupCollector>` → no-op après 1er tag.
pub fn sys_tag_player_as_collector(
    mut commands: Commands,
    q_player: Query<Entity, (With<Player>, Without<PickupCollector>)>,
) {
    for e in &q_player {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.insert(PickupCollector);
            info!("[roguelite] Player {e:?} tagged PickupCollector");
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sys_start_run(
    mut events: MessageReader<StartRunEvent>,
    mut next: ResMut<NextState<RunState>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    stage_graph_config: Res<forgia_stage_graph::RunGraphConfig>,
    mut wave: ResMut<crate::waves::RogueliteWave>,
) {
    for ev in events.read() {
        let seed = ev.seed.unwrap_or_else(default_seed_from_clock);
        let graph = forgia_stage_graph::generate_run_graph(&stage_graph_config, seed);
        let total_stages = graph.total_stages;
        let boss_depth = graph.boss_depth();

        // Reset RogueliteWave (run repeatable depuis Lobby).
        *wave = crate::waves::RogueliteWave::default();

        // V7 M3 step 4 — Reset Player HP au max au start (sinon HP=0 sticky après Defeat).
        commands.queue(|world: &mut World| {
            let mut q = world.query_filtered::<&mut forgia_damage::Health, With<Player>>();
            if let Ok(mut hp) = q.single_mut(world) {
                hp.current = hp.max;
            }
        });

        commands.insert_resource(RunSeed {
            seed,
            stage_count: total_stages,
        });
        commands.insert_resource(graph.clone());

        // TODO(story-471..479): API removed, refactor abandonné — re-implémenter
        // current_stage_node / composition_for_stage / spawn_stage_enemies supprimés de crate::waves.
        // wave.current_stage_depth / wave.current_stage_kind supprimés de RogueliteWave.
        // Fallback : spawn wave 1 directement via spawn_wave_enemies.
        {
            let _ = &graph; // graph utilisé pour total_stages + boss_depth ci-dessus
            let spawned = crate::waves::spawn_wave_enemies(
                &mut commands,
                &mut meshes,
                &mut materials,
                1, // wave 1
            );
            info!("[roguelite] sys_start_run fallback — wave 1 spawned {spawned} enemies");
            next.set(RunState::InRun { stage: 0 });
        }
        info!(
            "[roguelite] Run started — seed={seed} total_stages={total_stages} boss_depth={boss_depth}"
        );
    }
}

pub fn sys_end_run(mut events: MessageReader<EndRunEvent>, mut next: ResMut<NextState<RunState>>) {
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

/// Story-481 — Map WeaponType courante → speaker_id du genome dialogue.
/// Clés alignées sur `assets/genomes/roguelite/roguelite_dialogue.toml`
/// (pools indexés `speaker = "pepin" | "bourrasque" | "lenoir" | "boucherie"`).
/// Armes hors Arena V1 ou sans persona → "any" (fallback pool générique).
pub fn weapon_to_speaker(w: WeaponType) -> &'static str {
    match w {
        WeaponType::ModernAR => "pepin",
        WeaponType::AssaultRifle => "bourrasque",
        WeaponType::Shotgun => "lenoir",
        WeaponType::RocketLauncher => "boucherie",
        _ => "any",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runstate_default_is_lobby() {
        assert_eq!(RunState::default(), RunState::Lobby);
    }

    #[test]
    fn stage_id_for_depth_alternates() {
        assert_eq!(stage_id_for_depth(0, false), "crypts_of_anvil");
        assert_eq!(stage_id_for_depth(1, false), "forge_sanctum");
        assert_eq!(stage_id_for_depth(2, false), "crypts_of_anvil");
        assert_eq!(stage_id_for_depth(3, false), "forge_sanctum");
    }

    #[test]
    fn stage_id_for_depth_boss_forces_crypts() {
        // Boss state utilise toujours crypts_of_anvil (boss_pad_required=true).
        assert_eq!(stage_id_for_depth(4, true), "crypts_of_anvil");
        assert_eq!(stage_id_for_depth(5, true), "crypts_of_anvil");
        assert_eq!(stage_id_for_depth(0, true), "crypts_of_anvil");
    }

    // TODO(story-471..479) : tests parse_music_state_combat_variants +
    // parse_music_state_all_known_states supprimés car forgia_audio_music_state
    // ::MusicState n'existe plus (scaffold vide post-refactor). parse_music_state
    // est stub Option<()> retournant None (cf src/run.rs:165-169). À ré-instaurer
    // quand l'enum MusicState sera re-implémentée.

    #[test]
    fn parse_music_state_unknown_returns_none() {
        assert!(parse_music_state("").is_none());
        assert!(parse_music_state("nope").is_none());
        assert!(parse_music_state("xyz_combat").is_none()); // pas en prefix
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

    #[test]
    fn weapon_to_speaker_arena_v1_mapped() {
        assert_eq!(weapon_to_speaker(WeaponType::ModernAR), "pepin");
        assert_eq!(weapon_to_speaker(WeaponType::AssaultRifle), "bourrasque");
        assert_eq!(weapon_to_speaker(WeaponType::Shotgun), "lenoir");
        assert_eq!(weapon_to_speaker(WeaponType::RocketLauncher), "boucherie");
    }

    #[test]
    fn weapon_to_speaker_unknown_falls_back_to_any() {
        assert_eq!(weapon_to_speaker(WeaponType::AK47), "any");
        assert_eq!(weapon_to_speaker(WeaponType::PlasmaRifle), "any");
        assert_eq!(weapon_to_speaker(WeaponType::Chainsaw), "any");
    }
}
