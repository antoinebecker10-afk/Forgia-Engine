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
use forgia_core::prelude::*;
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
#[derive(Component, Default)]
pub struct RogueliteRunMarker;

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
