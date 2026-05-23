//! state.rs — Resources ECS du RPG Health Monitor.
//!
//! RpgHealthState : résultat agrégé des checks cross-frame.
//! SensorSnapshots : derniers JSON lus depuis disque (1Hz).
//! LastWriteTimestamps : horodatage de la dernière écriture de chaque sensor.

use bevy::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::time::SystemTime;

// ─────────────────────────── Severity ───────────────────────────

#[derive(Default, Clone, Copy, PartialEq, Eq, Serialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    #[default]
    Ok,
    Warn,
    Critical,
}

impl Severity {
    /// Retourne la sévérité la plus haute entre self et other.
    pub fn max(self, other: Self) -> Self {
        match (self, other) {
            (Self::Critical, _) | (_, Self::Critical) => Self::Critical,
            (Self::Warn, _) | (_, Self::Warn) => Self::Warn,
            _ => Self::Ok,
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::Warn => write!(f, "warn"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

// ─────────────────────────── CheckResult ───────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct CheckResult {
    pub severity: Severity,
    pub value: f32,
    pub threshold: f32,
    pub message: String,
    pub next_step: String,
}

impl CheckResult {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Ok,
            value: 0.0,
            threshold: 0.0,
            message: message.into(),
            next_step: String::new(),
        }
    }

    pub fn warn(
        value: f32,
        threshold: f32,
        message: impl Into<String>,
        next_step: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Warn,
            value,
            threshold,
            message: message.into(),
            next_step: next_step.into(),
        }
    }

    pub fn critical(
        value: f32,
        threshold: f32,
        message: impl Into<String>,
        next_step: impl Into<String>,
    ) -> Self {
        Self {
            severity: Severity::Critical,
            value,
            threshold,
            message: message.into(),
            next_step: next_step.into(),
        }
    }

    pub fn skipped(reason: impl Into<String>) -> Self {
        Self {
            severity: Severity::Ok,
            value: 0.0,
            threshold: 0.0,
            message: reason.into(),
            next_step: String::new(),
        }
    }
}

impl Default for CheckResult {
    fn default() -> Self {
        Self::ok("not yet evaluated")
    }
}

// ─────────────────────────── RpgHealthState ───────────────────────────

#[derive(Resource, Default)]
pub struct RpgHealthState {
    pub last_severity: Severity,
    pub last_message: String,
    pub last_next_step: String,
    pub checks: HashMap<&'static str, CheckResult>,
    pub tick_count: u64,
    pub last_write_secs: f32,
}

// ─────────────────────────── SensorSnapshots ───────────────────────────

#[derive(Resource, Default)]
pub struct SensorSnapshots {
    pub terrain_lod: Option<serde_json::Value>,
    pub chunks_snapshot: Option<serde_json::Value>,
    pub combat: Option<serde_json::Value>,
    pub health: Option<serde_json::Value>,
    pub anim_layer: Option<serde_json::Value>,
}

// ─────────────────────────── LastWriteTimestamps ───────────────────────────

#[derive(Resource, Default)]
pub struct LastWriteTimestamps {
    pub map: HashMap<String, SystemTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_max_critical_wins() {
        assert_eq!(Severity::Ok.max(Severity::Critical), Severity::Critical);
        assert_eq!(Severity::Critical.max(Severity::Ok), Severity::Critical);
        assert_eq!(Severity::Warn.max(Severity::Critical), Severity::Critical);
    }

    #[test]
    fn severity_max_warn_beats_ok() {
        assert_eq!(Severity::Ok.max(Severity::Warn), Severity::Warn);
        assert_eq!(Severity::Warn.max(Severity::Ok), Severity::Warn);
    }

    #[test]
    fn severity_ok_stays_ok() {
        assert_eq!(Severity::Ok.max(Severity::Ok), Severity::Ok);
    }

    #[test]
    fn check_result_ok_has_ok_severity() {
        let r = CheckResult::ok("all good");
        assert!(matches!(r.severity, Severity::Ok));
        assert_eq!(r.message, "all good");
    }

    #[test]
    fn check_result_warn_has_warn_severity() {
        let r = CheckResult::warn(0.05, 0.10, "too dark", "increase biome brightness");
        assert!(matches!(r.severity, Severity::Warn));
    }

    #[test]
    fn check_result_critical_has_critical_severity() {
        let r = CheckResult::critical(42.0, 0.0, "lod2 desync", "check lod system");
        assert!(matches!(r.severity, Severity::Critical));
    }

    #[test]
    fn check_result_skipped_is_ok() {
        let r = CheckResult::skipped("CHK-3 deferred to story-453");
        assert!(matches!(r.severity, Severity::Ok));
        assert!(r.message.contains("story-453"));
    }
}
