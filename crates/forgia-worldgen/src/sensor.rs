//! Observability — `forgia2_worldgen.json` (rule `observability-required.md`).
//!
//! Written once per second (accumulator), workspace root, flat JSON readable in one Read.

use crate::registry::AssetRegistry;
use crate::spawn::{SpawnQueue, WorldgenStats};
use crate::streaming::CityStreaming;
use crate::CityLayout;
use bevy::prelude::*;

/// Pure severity logic (testable headless): a loaded registry is the precondition for
/// everything else, so an empty registry is the one real alarm.
pub fn severity_for_worldgen(registry_modules: usize) -> (&'static str, &'static str) {
    if registry_modules == 0 {
        (
            "warn",
            "registry empty - assets/registry/asset_meta.ron missing or invalid",
        )
    } else {
        ("ok", "")
    }
}

/// Write `forgia2_worldgen.json` at ~1 Hz.
pub fn sys_write_worldgen_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    registry: Option<Res<AssetRegistry>>,
    stats: Res<WorldgenStats>,
    queue: Res<SpawnQueue>,
    city: Res<CityLayout>,
    streaming: Res<CityStreaming>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let registry_modules = registry.map(|r| r.len()).unwrap_or(0);
    let (severity, next_step) = severity_for_worldgen(registry_modules);
    let roads = if city.present {
        city.roads.segments.len()
    } else {
        0
    };
    let parcels = if city.present { city.parcels.len() } else { 0 };

    let json = format!(
        r#"{{"id":"worldgen","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"registry_modules":{registry_modules},"spawned_modules":{},"pending":{},"last_row":{},"city_roads":{roads},"city_parcels":{parcels},"stream_chunks":{}}}"#,
        time.elapsed_secs(),
        stats.spawned,
        queue.pending.len(),
        stats.last_row,
        streaming.active_chunks(),
    );

    let _ = forgia_core::sensor_io::enqueue("forgia2_worldgen.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_registry_warns() {
        assert_eq!(severity_for_worldgen(0).0, "warn");
        assert_eq!(severity_for_worldgen(107).0, "ok");
    }
}
