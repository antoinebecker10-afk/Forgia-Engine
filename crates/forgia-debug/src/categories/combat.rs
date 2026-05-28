//! Category 2 — Combat : damage, hitscan, screen flash, bot AI.
//!
//! Bug canonique : story-545 (player invincible) — `damage_events_received` doit
//! croître ; si 0 sustained → bot raycast self-hit.

use super::{fmt_opt, fmt_opt_str, DebugCategory};
use crate::snapshot::SensorSnapshot;
use bevy_egui::egui;

pub struct CombatCategory;

impl DebugCategory for CombatCategory {
    fn draw(&self, ui: &mut egui::Ui, snap: &SensorSnapshot) {
        let c = &snap.combat;

        ui.label("— damage received —");
        ui.label(format!(
            "events_received:    {}",
            fmt_opt(c.damage_events_received)
        ));
        ui.label(format!(
            "damage_flashes:     {}",
            fmt_opt(c.damage_flashes_session)
        ));
        ui.label(format!(
            "low_hp_active:      {}",
            c.low_hp_active.map_or("n/a".into(), |b| b.to_string())
        ));

        ui.separator();
        ui.label("— player shots —");
        ui.label(format!(
            "weapon:             {}",
            fmt_opt_str(c.current_weapon.as_deref())
        ));
        ui.label(format!("total_shots:        {}", fmt_opt(c.total_shots)));
        ui.label(format!(
            "hits_with_damage:   {}",
            fmt_opt(c.hits_with_damage)
        ));
        ui.label(format!(
            "blocked_by_world:   {}",
            fmt_opt(c.hits_blocked_by_world)
        ));
        ui.label(format!("missed_no_hit:      {}", fmt_opt(c.missed_no_hit)));

        ui.separator();
        ui.label("— kills —");
        ui.label(format!(
            "total_kills:        {}",
            fmt_opt(c.total_kills_session)
        ));
        ui.label(format!("streak:             {}", fmt_opt(c.streak_current)));
        ui.label(format!(
            "kill_flashes:       {}",
            fmt_opt(c.kill_flashes_session)
        ));

        ui.separator();
        ui.label("— bot AI —");
        ui.label(format!("bots_alive:         {}", fmt_opt(c.bots_alive)));
        ui.label(format!("bots_with_los:      {}", fmt_opt(c.bots_with_los)));
        ui.label(format!(
            "alerts_triggered:   {}",
            fmt_opt(c.alerts_triggered_session)
        ));
        ui.label(format!(
            "los_checks:         {}",
            fmt_opt(c.los_checks_session)
        ));
    }
}
