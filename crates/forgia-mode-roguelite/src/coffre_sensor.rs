//! coffre_sensor.rs — Story-558 Phase 6 (2026-05-29).
//!
//! Sensor `forgia2_coffre.json` 1Hz. Trace l'état du Coffre du Forgeron
//! (open/closed, candidates, last pick, souls spent, AFK detection).
//!
//! Schema :
//! - `coffre_open: bool` — session active
//! - `offered_boons: [{id, rarity, cost}]` — candidates actuels
//! - `last_picked_id: Option<String>` — dernier boon acheté
//! - `last_picked_secs: f32` — timestamp (elapsed_secs) du pick
//! - `souls_spent_total: u32` — cumul Souls dépensés sur boons (run)
//! - `open_duration_secs: f32` — temps écoulé depuis l'ouverture (0 si fermé)
//!
//! Severity `warn` si Coffre ouvert > 20s (AFK / indécis).

use bevy::prelude::*;
use forgia_rpg_data::boons::{BoonRarity, BoonsCatalogue, CoffrePickedEvent, CoffreSession};

const SENSOR_PATH: &str = "forgia2_coffre.json";
const AFK_WARN_THRESHOLD_SECS: f32 = 20.0;

/// État cumul + dernière interaction. Reset partiel OnEnter Roguelite (run fresh).
#[derive(Resource, Default, Debug, Clone)]
pub struct CoffreSensorState {
    pub last_picked_id: Option<String>,
    pub last_picked_secs: f32,
    pub souls_spent_total: u32,
    /// Timestamp d'ouverture courante (None si fermé). Set au front de `is_open`.
    pub opened_at_secs: Option<f32>,
}

/// Reset partiel quand on (re)entre Roguelite : on garde `souls_spent_total`
/// pour debug session-cross, mais on clear le pick courant.
pub fn sys_reset_coffre_sensor_on_run_start(mut state: ResMut<CoffreSensorState>) {
    state.last_picked_id = None;
    state.last_picked_secs = 0.0;
    state.opened_at_secs = None;
}

/// System CoffrePickedEvent — capture pick + souls cost. Run dans GameSet::Effects
/// après sys_handle_coffre_pick (Souls déjà décrémentés côté boons crate).
/// CoffrePickedEvent est un Message (Bevy 0.18), donc MessageReader (pas Observer).
pub fn sys_track_coffre_picks(
    mut events: MessageReader<CoffrePickedEvent>,
    catalogue: Res<BoonsCatalogue>,
    time: Res<Time>,
    mut state: ResMut<CoffreSensorState>,
) {
    for pick in events.read() {
        let id = pick.boon_id.clone();
        let cost = catalogue
            .find(&id)
            .map(|d| d.effective_souls_cost())
            .unwrap_or(0);
        state.last_picked_id = Some(id.0.clone());
        state.last_picked_secs = time.elapsed_secs();
        state.souls_spent_total = state.souls_spent_total.saturating_add(cost);
    }
}

/// Track opening transitions (None → Some(now)) et write JSON 1Hz.
/// Local<bool> `was_open` détecte le front montant.
pub fn sys_write_coffre_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    mut was_open: Local<bool>,
    session: Option<Res<CoffreSession>>,
    catalogue: Option<Res<BoonsCatalogue>>,
    mut state: ResMut<CoffreSensorState>,
) {
    // Détection transition open
    let is_open = session.as_ref().is_some_and(|s| s.is_open);
    if is_open && !*was_open {
        state.opened_at_secs = Some(time.elapsed_secs());
    } else if !is_open && *was_open {
        state.opened_at_secs = None;
    }
    *was_open = is_open;

    // Throttle 1Hz
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let open_duration = state
        .opened_at_secs
        .map(|t| time.elapsed_secs() - t)
        .unwrap_or(0.0);

    let (severity, next_step) = severity_for_coffre(is_open, open_duration);

    let offered_json = if is_open {
        session
            .as_ref()
            .map(|s| {
                let cat_ref = catalogue.as_deref();
                s.candidates
                    .iter()
                    .map(|id| {
                        let def = cat_ref.and_then(|c| c.find(id));
                        let rarity = def.map(|d| rarity_str(d.rarity)).unwrap_or("?");
                        let cost = def.map(|d| d.effective_souls_cost()).unwrap_or(0);
                        format!(
                            r#"{{"id":"{}","rarity":"{rarity}","cost":{cost}}}"#,
                            id.0.replace('"', "'")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    let last_picked = state
        .last_picked_id
        .as_ref()
        .map(|s| format!(r#""{}""#, s.replace('"', "'")))
        .unwrap_or_else(|| "null".to_string());

    let json = format!(
        r#"{{"id":"coffre","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"coffre_open":{is_open},"offered_boons":[{offered_json}],"last_picked_id":{last_picked},"last_picked_secs":{:.1},"souls_spent_total":{},"open_duration_secs":{:.1}}}"#,
        time.elapsed_secs(),
        state.last_picked_secs,
        state.souls_spent_total,
        open_duration,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[forgia-mode-roguelite] coffre sensor write failed: {e}");
    }
}

/// Pur — testable. Severity `warn` si Coffre ouvert > 20s sans pick (AFK proxy).
pub fn severity_for_coffre(is_open: bool, open_duration_secs: f32) -> (&'static str, &'static str) {
    if is_open && open_duration_secs > AFK_WARN_THRESHOLD_SECS {
        (
            "warn",
            "Coffre ouvert > 20s — joueur AFK ou indécis (close auto candidate)",
        )
    } else {
        ("ok", "")
    }
}

fn rarity_str(r: BoonRarity) -> &'static str {
    match r {
        BoonRarity::Common => "common",
        BoonRarity::Uncommon => "uncommon",
        BoonRarity::Rare => "rare",
        BoonRarity::Legendary => "legendary",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_when_closed() {
        assert_eq!(severity_for_coffre(false, 999.0).0, "ok");
        assert_eq!(severity_for_coffre(false, 0.0).0, "ok");
    }

    #[test]
    fn severity_ok_when_open_under_threshold() {
        assert_eq!(severity_for_coffre(true, 0.0).0, "ok");
        assert_eq!(severity_for_coffre(true, AFK_WARN_THRESHOLD_SECS).0, "ok");
    }

    #[test]
    fn severity_warn_when_afk() {
        let (sev, next) = severity_for_coffre(true, AFK_WARN_THRESHOLD_SECS + 0.1);
        assert_eq!(sev, "warn");
        assert!(next.contains("20s"));
    }

    #[test]
    fn rarity_str_lowercase_matches_toml() {
        assert_eq!(rarity_str(BoonRarity::Common), "common");
        assert_eq!(rarity_str(BoonRarity::Uncommon), "uncommon");
        assert_eq!(rarity_str(BoonRarity::Rare), "rare");
        assert_eq!(rarity_str(BoonRarity::Legendary), "legendary");
    }

    #[test]
    fn coffre_state_default_empty() {
        let s = CoffreSensorState::default();
        assert!(s.last_picked_id.is_none());
        assert_eq!(s.souls_spent_total, 0);
        assert!(s.opened_at_secs.is_none());
    }
}
