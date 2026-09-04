//! Santé du writer asynchrone des capteurs, `forgia2_sensor_io.json` (1 Hz).
//!
//! Sans cette sonde, une queue pleine ou une erreur disque faisait perdre des
//! snapshots sans aucun signal : l'observabilité pouvait paraître saine alors
//! que ses fichiers ne se mettaient plus à jour.

use bevy::prelude::*;

fn severity_for(stats: forgia_core::sensor_io::SensorIoStats) -> (&'static str, &'static str) {
    if stats.disconnected > 0 {
        (
            "critical",
            "sensor writer disconnected — diagnostics are no longer persisted",
        )
    } else if stats.dropped_full > 0 || stats.write_failures > 0 || stats.pending > 128 {
        (
            "warn",
            "sensor I/O degraded — inspect queue, dropped samples and filesystem permissions",
        )
    } else {
        ("ok", "")
    }
}

pub fn sys_write_sensor_io_sensor(time: Res<Time>, mut accum: Local<f32>) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;

    let stats = forgia_core::sensor_io::stats();
    let (severity, next_step) = severity_for(stats);

    let json = format!(
        r#"{{"id":"sensor_io","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"enqueued":{},"processed":{},"pending":{},"dropped_full":{},"disconnected":{},"write_failures":{}}}"#,
        time.elapsed_secs(),
        stats.enqueued,
        stats.processed,
        stats.pending,
        stats.dropped_full,
        stats.disconnected,
        stats.write_failures,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia2_sensor_io.json", json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_writer_is_ok() {
        assert_eq!(severity_for(Default::default()).0, "ok");
    }

    #[test]
    fn disconnected_writer_is_critical() {
        let stats = forgia_core::sensor_io::SensorIoStats {
            disconnected: 1,
            ..Default::default()
        };
        assert_eq!(severity_for(stats).0, "critical");
    }

    #[test]
    fn losses_failures_and_backlog_warn() {
        for stats in [
            forgia_core::sensor_io::SensorIoStats {
                dropped_full: 1,
                ..Default::default()
            },
            forgia_core::sensor_io::SensorIoStats {
                write_failures: 1,
                ..Default::default()
            },
            forgia_core::sensor_io::SensorIoStats {
                pending: 129,
                ..Default::default()
            },
        ] {
            assert_eq!(severity_for(stats).0, "warn");
        }
    }
}
