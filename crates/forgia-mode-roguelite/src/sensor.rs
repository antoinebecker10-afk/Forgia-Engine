//! sensor.rs — Producteur `forgia2_roguelite_state.json` (1Hz, V7 M1).
//!
//! Story-470 — sensor canonique 13/13. Telemetry runtime augmentée (story-471 quick) :
//! - run_state, stage, stage_count, seed
//! - tick_count (compteur frames depuis insertion plugin)
//! - elapsed_secs (time depuis boot)
//! - time_in_state_secs (durée depuis dernière transition RunState)
//! - transitions_count (nombre total transitions RunState observées)
//! - severity warn si stuck >30min en InRun (proxy run gelée)

use crate::run::{RunSeed, RunState};
use crate::waves::RogueliteWave;
use bevy::prelude::*;
// Story-571 — Or in-run = `Gold` (alias forgia-rpg-data Souls) + Souls méta.
use crate::run::{MetaSouls, RunTimer};
use crate::shockwave::ShockwaveAbility;
use forgia_rpg_data::loot_tables::Souls as Gold;

/// Telemetry runtime — incrémentée chaque frame par `sys_update_roguelite_telemetry`.
#[derive(Resource, Default, Debug, Clone)]
pub struct RogueliteTelemetry {
    pub tick_count: u64,
    pub time_in_state_secs: f32,
    pub transitions_count: u32,
    pub last_state_label: Option<&'static str>,
}

const STUCK_RUN_THRESHOLD_SECS: f32 = 30.0 * 60.0; // 30 min sans transition = warn

/// Pur — extrait pour tests headless.
///
/// `mastery_over_cap` (story-668) : au moins une arme a un niveau de maîtrise
/// STOCKÉ au-dessus du plafond `[mastery] max_level`. Sans plafond avant story-668,
/// c'est le cas normal des saves existantes — le bonus, lui, est borné à la lecture.
/// Le signaler évite de re-diagnostiquer « le plafond ne marche pas » en voyant un
/// niveau 13 dans le save alors que le jeu applique bien +20 %.
pub fn severity_for_roguelite(
    time_in_state_secs: f32,
    state_label: &str,
    mastery_over_cap: bool,
) -> (&'static str, &'static str) {
    if state_label == "in_run" && time_in_state_secs > STUCK_RUN_THRESHOLD_SECS {
        (
            "warn",
            "InRun > 30min sans transition — run possiblement gelée (boss pas tué ?)",
        )
    } else if mastery_over_cap {
        (
            "info",
            "Save antérieure au plafond de maîtrise : niveau stocké > mastery_cap. Le bonus EST borné à la lecture, rien à corriger — relever [mastery] max_level rendrait la progression réelle.",
        )
    } else {
        ("ok", "")
    }
}

fn state_label(rs: Option<&State<RunState>>) -> (&'static str, u8) {
    match rs.map(|s| s.get().clone()) {
        Some(RunState::Lobby) => ("lobby", 0u8),
        Some(RunState::InRun { stage }) => ("in_run", stage),
        Some(RunState::Boss { stage }) => ("boss", stage),
        Some(RunState::Defeat) => ("defeat", 0),
        Some(RunState::Victory) => ("victory", 0),
        None => ("none", 0),
    }
}

/// Tourne chaque frame — incrémente ticks + détecte transitions RunState.
pub fn sys_update_roguelite_telemetry(
    time: Res<Time>,
    run_state: Option<Res<State<RunState>>>,
    mut tel: ResMut<RogueliteTelemetry>,
) {
    tel.tick_count = tel.tick_count.saturating_add(1);

    let (current_label, _) = state_label(run_state.as_deref());
    let changed = tel.last_state_label != Some(current_label);
    if changed {
        tel.last_state_label = Some(current_label);
        tel.time_in_state_secs = 0.0;
        tel.transitions_count = tel.transitions_count.saturating_add(1);
    } else {
        tel.time_in_state_secs += time.delta_secs();
    }
}

/// Écrit le sensor JSON 1Hz.
pub fn sys_write_roguelite_state(
    time: Res<Time>,
    mut accum: Local<f32>,
    run_state: Option<Res<State<RunState>>>,
    run_seed: Option<Res<RunSeed>>,
    tel: Res<RogueliteTelemetry>,
    gold: Option<Res<Gold>>,
    meta: Option<Res<MetaSouls>>,
    timer: Option<Res<RunTimer>>,
    shockwave: Option<Res<ShockwaveAbility>>,
    wave: Option<Res<RogueliteWave>>,
    // Story-591 — méta-progression persistée disque (L'Enclume des Âmes).
    meta_save: Option<Res<crate::meta_shop::MetaShopSave>>,
    // Story-668 — plafond de maîtrise (genome `[mastery]`), pour rendre l'invariant
    // observable : sans ça, il fallait ouvrir %APPDATA% à la main pour le vérifier.
    meta_cat: Option<Res<crate::meta_shop::MetaShopCatalogue>>,
    // Story-669 — type de salle + densité : la preuve, en une lecture, que le choix
    // de porte change RÉELLEMENT le combat (avant, `room_kind` n'était même pas lu).
    graph_cfg: Option<Res<forgia_stage::graph::RunGraphConfig>>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let (state_str, stage) = state_label(run_state.as_deref());
    let seed = run_seed.as_ref().map(|s| s.seed).unwrap_or(0);
    let stage_count = run_seed.as_ref().map(|s| s.stage_count).unwrap_or(0);
    // Story-668 — maîtrise d'arme : niveaux stockés + plafond du genome.
    let mastery_cap = meta_cat.as_ref().map(|c| c.mastery.max_level).unwrap_or(0);
    let weapon_levels = meta_save
        .as_ref()
        .map(|s| {
            let mut parts: Vec<String> = s
                .weapon_levels
                .iter()
                .map(|(k, v)| format!("\"{k}\":{v}"))
                .collect();
            parts.sort();
            format!("{{{}}}", parts.join(","))
        })
        .unwrap_or_else(|| "{}".to_string());
    let mastery_over_cap = match (meta_save.as_ref(), mastery_cap) {
        (Some(s), cap) if cap > 0 => s.weapon_levels.values().any(|&lvl| lvl > cap),
        _ => false,
    };
    // Story-669 — la salle courante : son TYPE (porte choisie) et sa DENSITÉ.
    let room_kind = wave
        .as_ref()
        .and_then(|w| w.room_kind)
        .map(|k| format!("{k:?}").to_ascii_lowercase())
        .unwrap_or_else(|| "none".to_string());
    // Le budget est stocké en centi-crédits ; on l'expose en CRÉDITS, l'unité des
    // gènes (`director_credits_base = 2.0`). Le nom porte l'unité : un champ dont
    // l'unité est implicite finit toujours par être lu de travers.
    let room_budget_credits = wave.as_ref().map(|w| w.room_budget).unwrap_or(0) as f32
        / f32::from(forgia_stage::graph::DIRECTOR_BUDGET_SCALE);
    let room_budget = wave.as_ref().map(|w| w.room_budget).unwrap_or(0);
    let room_density = graph_cfg
        .as_ref()
        .map(|c| crate::wave_comp::density_from_budget(room_budget, c.director_budget_for_depth(0)))
        .unwrap_or(1.0);
    let (severity, next_step) =
        severity_for_roguelite(tel.time_in_state_secs, state_str, mastery_over_cap);
    // Story-571 — Or in-run + Souls méta persistant.
    let or_current = gold.as_ref().map(|s| s.current).unwrap_or(0);
    let or_collected = gold.as_ref().map(|s| s.total_collected).unwrap_or(0);
    let souls_persistent = meta.as_ref().map(|m| m.current).unwrap_or(0);
    let souls_earned_run = meta.as_ref().map(|m| m.earned_run).unwrap_or(0);
    let run_timer_secs = timer.as_ref().map(|t| t.secs).unwrap_or(0.0);
    let shockwave_casts = shockwave.as_ref().map(|s| s.casts_total).unwrap_or(0);
    // Story-573 — cooldown PAR ARME : on expose le max restant (toutes armes).
    let shockwave_cd = shockwave
        .as_ref()
        .map(|s| s.cooldowns.values().copied().fold(0.0_f32, f32::max))
        .unwrap_or(0.0);
    let current_wave = wave.as_ref().map(|w| w.current_wave).unwrap_or(0);
    // Story-646 R2 — salle courante (0-indexed, `RogueliteWave.stage`).
    let room = wave.as_ref().map(|w| w.stage).unwrap_or(0);
    let bots_alive = wave.as_ref().map(|w| w.bots_alive).unwrap_or(0);
    let break_secs_left = wave.as_ref().map(|w| w.break_secs_left).unwrap_or(0.0);
    let in_break = wave.as_ref().map(|w| w.in_break).unwrap_or(false);
    // Fix audit 2026-07-19 — `victory_emitted` est un LATCH de fin de run posé
    // aussi sur la DÉFAITE (run.rs `obs_roguelite_player_death`). L'exporter
    // sous le nom `victory` a produit un faux diagnostic (victory:true sur une
    // run perdue). Nom honnête : `run_ended`. La vraie victoire = `victories_total`.
    let run_ended = wave.as_ref().map(|w| w.victory_emitted).unwrap_or(false);
    // Story-603 — boss vaincu = la porte du socle s'ouvre (parcours débloqué).
    let boss_defeated = wave.as_ref().map(|w| w.boss_defeated).unwrap_or(false);
    // Story-591 — état persisté disque : total d'Âmes + rangs des upgrades.
    let meta_souls_total = meta_save.as_ref().map(|s| s.souls_total).unwrap_or(0);
    // R3.3 — victoires cumulées persistées (0 tant que la victoire n'a jamais été atteinte).
    let victories_total = meta_save.as_ref().map(|s| s.victories).unwrap_or(0);
    let meta_ranks = meta_save
        .as_ref()
        .map(|s| {
            let mut parts: Vec<String> = s
                .ranks
                .iter()
                .map(|(k, v)| format!("\"{k}\":{v}"))
                .collect();
            parts.sort();
            format!("{{{}}}", parts.join(","))
        })
        .unwrap_or_else(|| "{}".to_string());

    let json = format!(
        r#"{{"id":"roguelite_state","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"run_state":"{state_str}","stage":{stage},"stage_count":{stage_count},"seed":{seed},"tick_count":{},"time_in_state_secs":{:.1},"transitions_count":{},"elapsed_secs":{:.1},"or_current":{or_current},"or_collected_run":{or_collected},"souls_persistent":{souls_persistent},"souls_earned_run":{souls_earned_run},"meta_souls_total":{meta_souls_total},"meta_ranks":{meta_ranks},"weapon_levels":{weapon_levels},"mastery_cap":{mastery_cap},"run_timer_secs":{run_timer_secs:.1},"shockwave_casts":{shockwave_casts},"shockwave_cd":{shockwave_cd:.1},"current_wave":{current_wave},"room":{room},"room_kind":"{room_kind}","room_budget_credits":{room_budget_credits:.2},"room_density":{room_density:.2},"bots_alive":{bots_alive},"in_break":{in_break},"break_secs_left":{:.1},"run_ended":{run_ended},"victories_total":{victories_total},"boss_defeated":{boss_defeated}}}"#,
        time.elapsed_secs(),
        tel.tick_count,
        tel.time_in_state_secs,
        tel.transitions_count,
        time.elapsed_secs(),
        break_secs_left,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_roguelite_state.json", json) {
        warn!("[forgia-mode-roguelite] sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_in_lobby() {
        assert_eq!(severity_for_roguelite(0.0, "lobby", false).0, "ok");
        assert_eq!(severity_for_roguelite(99999.0, "lobby", false).0, "ok");
    }

    #[test]
    fn severity_ok_in_run_under_threshold() {
        assert_eq!(severity_for_roguelite(1000.0, "in_run", false).0, "ok");
        assert_eq!(
            severity_for_roguelite(STUCK_RUN_THRESHOLD_SECS, "in_run", false).0,
            "ok"
        );
    }

    #[test]
    fn severity_warn_in_run_stuck() {
        let (sev, next) = severity_for_roguelite(STUCK_RUN_THRESHOLD_SECS + 0.1, "in_run", false);
        assert_eq!(sev, "warn");
        assert!(next.contains("30min"));
    }

    #[test]
    fn telemetry_default_zero() {
        let t = RogueliteTelemetry::default();
        assert_eq!(t.tick_count, 0);
        assert_eq!(t.time_in_state_secs, 0.0);
        assert_eq!(t.transitions_count, 0);
        assert!(t.last_state_label.is_none());
    }
}
