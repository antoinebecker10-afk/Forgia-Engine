//! # forgia-sensors
//!
//! Stack observability : 12 sensors max (vs 95 V1), 1 producteur unique chacun.
//!
//! Sensors planifiés Phase 5 :
//! 1. forgia2_health.json
//! 2. forgia2_perf.json (fusion 4 V1)
//! 3. forgia2_lifecycle.json
//! 4. forgia2_entities.json
//! 5. forgia2_memory.json (fusion 3 V1)
//! 6. forgia2_watchdog.json (thread OS)
//! 7. forgia2_combat.json
//! 8. forgia2_arena.json (fusion 4 V1)
//! 9. forgia2_chunks.json (inactif FPS)
//! 10. forgia2_audio.json
//! 11. forgia2_input.json
//! 12. forgia2_sensor_health.json (meta)

use bevy::prelude::*;
use forgia_core::prelude::*;

pub mod prelude {
    pub use crate::ForgiaSensorsPlugin;
}

pub struct ForgiaSensorsPlugin;

impl Plugin for ForgiaSensorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sensor_liveness_tick.in_set(GameSet::Sensors));
    }
}

fn sensor_liveness_tick() {
    // Phase 5 : check tous les sensors déclarés sont alive (timestamp < 60s).
    // Si stale > 60s → flag dans forgia2_sensor_health.json.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaSensorsPlugin;
    }
}
