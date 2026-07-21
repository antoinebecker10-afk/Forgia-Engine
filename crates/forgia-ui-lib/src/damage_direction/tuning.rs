//! Tuning DDI genome-driven. `assets/genomes/damage_dir_tuning.toml`.

use bevy::prelude::*;
use bevy::reflect::TypePath;
use forgia_genome_core::Genome;
use serde::Deserialize;

#[derive(Resource, Deserialize, Clone, Debug, TypePath)]
pub struct DamageDirTuning {
    /// Rayon outer des arcs depuis le crosshair (px).
    pub radius_px: f32,
    /// Épaisseur trait arc (px).
    pub thickness: f32,
    /// Segments tessellation (résolution).
    pub segments: usize,
    /// Angle de span d'un arc (degrés). 60° AAA standard CoD.
    pub arc_span_deg: f32,
    /// Cap arcs simultanés. 4 = standard, anti-clutter.
    pub max_arcs: usize,
    /// Durée totale d'un arc (secs). Fade ease-out (1-t)².
    pub duration_secs: f32,
    /// Alpha min lerp (intensity faible damage).
    pub intensity_min: f32,
    /// Alpha max lerp (intensity damage = max_hp).
    pub intensity_max: f32,
    /// Période d'écriture sensor JSON.
    pub sensor_period_secs: f32,
}

impl Default for DamageDirTuning {
    fn default() -> Self {
        Self {
            radius_px: 90.0,
            thickness: 7.0,
            segments: 16,
            arc_span_deg: 60.0,
            max_arcs: 4,
            duration_secs: 1.2,
            intensity_min: 0.5,
            intensity_max: 1.0,
            sensor_period_secs: 1.0,
        }
    }
}

#[derive(Resource)]
pub struct DamageDirTuningHandle(pub Handle<Genome<DamageDirTuning>>);

pub fn load_damage_dir_tuning(mut commands: Commands, server: Res<AssetServer>) {
    let handle: Handle<Genome<DamageDirTuning>> = server.load("genomes/damage_dir_tuning.toml");
    commands.insert_resource(DamageDirTuningHandle(handle));
}

pub fn sync_damage_dir_tuning(
    mut events: MessageReader<AssetEvent<Genome<DamageDirTuning>>>,
    handle: Option<Res<DamageDirTuningHandle>>,
    assets: Res<Assets<Genome<DamageDirTuning>>>,
    mut tuning: ResMut<DamageDirTuning>,
) {
    let Some(handle) = handle else { return };
    let mut should = false;
    for ev in events.read() {
        match ev {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id }
                if *id == handle.0.id() =>
            {
                should = true;
            }
            _ => {}
        }
    }
    if !should {
        return;
    }
    if let Some(g) = assets.get(&handle.0) {
        *tuning = g.data.clone();
        info!(
            "[damage-dir] tuning synced (radius {}px, max {})",
            tuning.radius_px, tuning.max_arcs
        );
    }
}
