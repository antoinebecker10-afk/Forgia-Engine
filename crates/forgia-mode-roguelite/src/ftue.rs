//! ftue.rs — First-Time User Experience (story-597 Phase B : « mort = centre de
//! gravité »).
//!
//! Persiste l'état d'onboarding **séparément** de `meta_shop_save.toml` (ne pas
//! coupler shop et FTUE — story-597). Pour l'incrément 1 : `first_death_recap_seen`
//! (le récap pédagogique de la 1re mort ne s'affiche qu'**une fois à vie**) +
//! `first_death_run_secs` (funnel). Étendu plus tard avec `seen_hints` (Phase A).
//!
//! Pattern config Forgia (`fs` + `serde` + `toml` + `config_dir`), miroir EXACT de
//! `meta_shop.rs` (story-591). Sensor `forgia2_ftue.json` (observability-required).

use bevy::prelude::*;
use forgia_core::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SAVE_FILE: &str = "ftue_save.toml";
const SAVE_VERSION: u32 = 1;
const SENSOR_PATH: &str = "forgia2_ftue.json";

/// État FTUE persisté disque (séparé du shop). Flags one-shot à vie.
#[derive(Resource, Serialize, Deserialize, Clone, Debug)]
pub struct FtueSave {
    pub version: u32,
    /// Le récap pédagogique de la 1re mort a-t-il déjà été montré ? (one-shot à vie)
    pub first_death_recap_seen: bool,
    /// Secondes de jeu à la 1re mort (funnel). 0 = pas encore mort.
    pub first_death_run_secs: f32,
    /// Hints one-shot déjà vus (Phase A, à venir).
    #[serde(default)]
    pub seen_hints: Vec<String>,
}

impl Default for FtueSave {
    fn default() -> Self {
        Self {
            version: SAVE_VERSION,
            first_death_recap_seen: false,
            first_death_run_secs: 0.0,
            seen_hints: Vec::new(),
        }
    }
}

impl FtueSave {
    fn save_path() -> PathBuf {
        crate::persist::save_dir().join(SAVE_FILE)
    }

    pub fn load_or_default() -> Self {
        crate::persist::load_toml_migrating(SAVE_FILE)
    }

    pub fn save(&self) {
        crate::persist::save_toml_atomic(&Self::save_path(), self, "ftue");
    }

    /// PUR (pas d'IO, testable) : pose les flags 1re mort. Retourne `true` si c'était
    /// la 1re fois (→ le caller persiste). Idempotent.
    fn note_first_death(&mut self, run_secs: f32) -> bool {
        if self.first_death_recap_seen {
            return false;
        }
        self.first_death_recap_seen = true;
        if self.first_death_run_secs <= 0.0 {
            self.first_death_run_secs = run_secs;
        }
        true
    }

    /// Marque le récap 1re mort comme vu + capture le temps de run (funnel) + save
    /// disque. Idempotent. Appelé depuis les boutons de l'écran Defeat (L'Enclume ET
    /// Menu) → couvre les 2 chemins de sortie quel que soit le comportement OnExit
    /// des SubStates Bevy (fix qa BUG-597-B-01).
    pub fn mark_first_death(&mut self, run_secs: f32) {
        if self.note_first_death(run_secs) {
            self.save();
            info!(
                "[ftue] première mort enregistrée (récap vu, run {:.0}s)",
                self.first_death_run_secs
            );
        }
    }
}

/// Sensor funnel `forgia2_ftue.json` 1Hz (observability-required).
pub fn sys_write_ftue_sensor(time: Res<Time>, mut accum: Local<f32>, ftue: Res<FtueSave>) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    let json = format!(
        r#"{{"id":"ftue","severity":"ok","next_step":"","timestamp_secs":{:.1},"first_death_recap_seen":{},"first_death_run_secs":{:.1},"hints_seen":{}}}"#,
        time.elapsed_secs(),
        ftue.first_death_recap_seen,
        ftue.first_death_run_secs,
        ftue.seen_hints.len(),
    );
    let _ = std::fs::write(SENSOR_PATH, json);
}

pub struct FtuePlugin;

impl Plugin for FtuePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FtueSave::load_or_default());
        // Le marquage « 1re mort vue » se fait au clic d'un bouton de l'écran Defeat
        // (`hud::draw_defeat_overlay` → `FtueSave::mark_first_death`) pour couvrir
        // les 2 chemins (L'Enclume + Menu) — cf fix qa BUG-597-B-01.
        app.add_systems(Update, sys_write_ftue_sensor.in_set(GameSet::Sensors));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_unseen() {
        let s = FtueSave::default();
        assert!(!s.first_death_recap_seen);
        assert_eq!(s.first_death_run_secs, 0.0);
        assert_eq!(s.version, SAVE_VERSION);
        assert!(s.seen_hints.is_empty());
    }

    #[test]
    fn note_first_death_is_one_shot() {
        let mut s = FtueSave::default();
        assert!(s.note_first_death(42.0)); // 1re fois → true (à persister)
        assert!(s.first_death_recap_seen);
        assert_eq!(s.first_death_run_secs, 42.0);
        // 2e appel = no-op : false + ne ré-écrase pas le temps.
        assert!(!s.note_first_death(999.0));
        assert_eq!(s.first_death_run_secs, 42.0);
    }

    #[test]
    fn toml_roundtrip() {
        let mut s = FtueSave::default();
        s.first_death_recap_seen = true;
        s.first_death_run_secs = 123.4;
        s.seen_hints.push("first_pickup".into());
        let ser = toml::to_string_pretty(&s).unwrap();
        let de: FtueSave = toml::from_str(&ser).unwrap();
        assert!(de.first_death_recap_seen);
        assert_eq!(de.first_death_run_secs, 123.4);
        assert_eq!(de.seen_hints, vec!["first_pickup".to_string()]);
    }
}
