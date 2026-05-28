//! Category 6 — Audio : channels, music, voicelines.

use super::{fmt_opt, fmt_opt_f64, fmt_opt_str, DebugCategory};
use crate::snapshot::SensorSnapshot;
use bevy_egui::egui;

pub struct AudioCategory;

impl DebugCategory for AudioCategory {
    fn draw(&self, ui: &mut egui::Ui, snap: &SensorSnapshot) {
        let a = &snap.audio;

        ui.label("— channels —");
        ui.label(format!(
            "channels_active: {}",
            fmt_opt(a.channels_active)
        ));
        ui.label(format!(
            "master_volume:   {}",
            fmt_opt_f64(a.master_volume, 2)
        ));

        ui.separator();
        ui.label("— music —");
        ui.label(format!(
            "track:           {}",
            fmt_opt_str(a.music_track.as_deref())
        ));
        ui.label(format!(
            "intensity:       {}",
            fmt_opt_f64(a.music_intensity, 2)
        ));

        ui.separator();
        ui.label("— voicelines —");
        ui.label(format!(
            "played_session:  {}",
            fmt_opt(a.voicelines_played_session)
        ));
        ui.label(format!(
            "last:            {}",
            fmt_opt_str(a.last_voiceline.as_deref())
        ));
    }
}
