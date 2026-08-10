//! lag_events_sensor.rs — Producteur `forgia2_lag_events.json` (1Hz write, frame tick).
//!
//! Ring buffer bounded (50 events) des frames où `dt > LAG_THRESHOLD_MS` (30ms = ~33fps).
//! Granularité fine vs `forgia2_watchdog.json` qui n'expose qu'un compteur cumulatif.
//!
//! Permet la corrélation avec :
//! - `forgia_foliage_fallback.json` (bursts foliage population)
//! - `forgia2_stage.json` (load events)
//! - `forgia_chunk_stream.json` (chunk load activity)
//!
//! Pattern : flush 1Hz système séparé du push hot-path (0-alloc en hot).
//! Capture cause hint heuristique (last_dt_ms + frame index).
//!
//! Story-526 — Diagnostic freeze RPG+Roguelite (2026-05-26). V1 forgia_lag_events.json
//! existait — re-porté ici pour V2.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::VecDeque;

const LAG_THRESHOLD_MS: f32 = 30.0;
const RING_CAPACITY: usize = 50;

#[derive(Clone)]
pub struct LagEvent {
    pub t_secs: f32,
    pub dt_ms: f32,
    pub frame_index: u64,
}

#[derive(Resource, Default)]
pub struct LagEventsRing {
    pub events: VecDeque<LagEvent>,
    pub total_recorded: u64,
    pub frame_index: u64,
    pub ignored_unfocused: u64,
}

/// Tick frame en `First` schedule — push si lag detected. 0-alloc en hot path
/// (VecDeque préalloué + bounded).
pub fn sys_track_lag_events(
    time: Res<Time>,
    primary_window: Query<&Window, With<PrimaryWindow>>,
    mut ring: ResMut<LagEventsRing>,
) {
    ring.frame_index = ring.frame_index.saturating_add(1);
    let dt_ms = time.delta_secs() * 1000.0;
    if dt_ms <= LAG_THRESHOLD_MS {
        return;
    }
    if primary_window.single().is_ok_and(|window| !window.focused) {
        ring.ignored_unfocused = ring.ignored_unfocused.saturating_add(1);
        return;
    }
    let event = LagEvent {
        t_secs: time.elapsed_secs(),
        dt_ms,
        frame_index: ring.frame_index,
    };
    if ring.events.len() >= RING_CAPACITY {
        ring.events.pop_front();
    }
    ring.events.push_back(event);
    ring.total_recorded = ring.total_recorded.saturating_add(1);
}

pub fn severity_for_lag_events(events_last_30s: usize) -> (&'static str, &'static str) {
    if events_last_30s > 30 {
        (
            "critical",
            "lag events > 30 sur 30s — stutter sustained, profiler GPU/CPU recommandé",
        )
    } else if events_last_30s > 10 {
        (
            "warn",
            "lag events > 10 sur 30s — cluster stutter, croiser avec forgia_foliage_fallback.json + forgia_chunk_stream.json timestamps",
        )
    } else {
        ("ok", "")
    }
}

pub fn sys_write_lag_events_sensor(
    time: Res<Time>,
    ring: Res<LagEventsRing>,
    mut accum: Local<f32>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let now = time.elapsed_secs();
    let events_last_30s = ring
        .events
        .iter()
        .filter(|e| now - e.t_secs <= 30.0)
        .count();

    let (severity, next_step) = severity_for_lag_events(events_last_30s);

    let mut events_json = String::with_capacity(ring.events.len() * 64);
    for (i, ev) in ring.events.iter().enumerate() {
        if i > 0 {
            events_json.push(',');
        }
        events_json.push_str(&format!(
            r#"{{"t":{:.2},"dt_ms":{:.2},"frame":{}}}"#,
            ev.t_secs, ev.dt_ms, ev.frame_index
        ));
    }

    let json = format!(
        r#"{{"id":"lag_events","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"threshold_ms":{:.1},"total_recorded":{},"ignored_unfocused":{},"buffer_len":{},"events_last_30s":{},"events":[{}]}}"#,
        now,
        LAG_THRESHOLD_MS,
        ring.total_recorded,
        ring.ignored_unfocused,
        ring.events.len(),
        events_last_30s,
        events_json,
    );

    if let Err(e) = forgia_core::sensor_io::enqueue("forgia2_lag_events.json", json) {
        warn!("[forgia-observability] lag_events sensor write failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_ok_few_events() {
        assert_eq!(severity_for_lag_events(3).0, "ok");
    }

    #[test]
    fn severity_warn_at_10() {
        let (sev, _) = severity_for_lag_events(15);
        assert_eq!(sev, "warn");
    }

    #[test]
    fn severity_critical_at_30() {
        let (sev, next) = severity_for_lag_events(50);
        assert_eq!(sev, "critical");
        assert!(next.contains("profiler"));
    }

    #[test]
    fn ring_bounded_at_50() {
        let mut ring = LagEventsRing::default();
        for i in 0..100 {
            if ring.events.len() >= RING_CAPACITY {
                ring.events.pop_front();
            }
            ring.events.push_back(LagEvent {
                t_secs: i as f32,
                dt_ms: 50.0,
                frame_index: i as u64,
            });
        }
        assert_eq!(ring.events.len(), 50);
        // FIFO : 50 plus récents (50..99) gardés
        assert_eq!(ring.events.front().unwrap().frame_index, 50);
        assert_eq!(ring.events.back().unwrap().frame_index, 99);
    }

    #[test]
    fn threshold_is_30ms() {
        assert!((LAG_THRESHOLD_MS - 30.0).abs() < 1e-6);
    }
}
