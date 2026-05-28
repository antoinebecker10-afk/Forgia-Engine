//! Category 4 — Terrain : chunks, LOD, foliage, streaming.
//!
//! Bug canonique : story-502 (G2 chunks vegetation_total=0 sustained).

use super::{fmt_opt, DebugCategory};
use crate::snapshot::SensorSnapshot;
use bevy_egui::egui;

pub struct TerrainCategory;

impl DebugCategory for TerrainCategory {
    fn draw(&self, ui: &mut egui::Ui, snap: &SensorSnapshot) {
        let t = &snap.terrain;

        ui.label("— chunks —");
        ui.label(format!("total_chunks:        {}", fmt_opt(t.total_chunks)));
        ui.label(format!("loading:             {}", fmt_opt(t.chunks_loading)));
        ui.label(format!("loaded:              {}", fmt_opt(t.chunks_loaded)));

        ui.separator();
        ui.label("— LOD —");
        ui.label(format!("lod0:                {}", fmt_opt(t.lod0_count)));
        ui.label(format!("lod1:                {}", fmt_opt(t.lod1_count)));

        ui.separator();
        ui.label("— vegetation / foliage —");
        ui.label(format!(
            "vegetation_total:    {}",
            fmt_opt(t.vegetation_total)
        ));
        ui.label(format!(
            "foliage fb 30s:      {}",
            fmt_opt(t.foliage_fallback_events_30s)
        ));
        ui.label(format!(
            "foliage fb total:    {}",
            fmt_opt(t.foliage_fallback_total)
        ));
    }
}
