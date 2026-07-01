//! Tuning genome — nameplate sizing + lifetime + colors.
//!
//! Hot-reload via genome `genomes/ui/enemy_nameplate.toml`.

use bevy::prelude::*;
use forgia_genome_core::{Genome, GenomeLoader};
use serde::Deserialize;

#[derive(Deserialize, TypePath, Clone, Debug)]
#[serde(default)]
pub struct EnemyNameplateTuning {
    /// Largeur world-space du nameplate (m).
    pub width: f32,
    /// Hauteur world-space du nameplate (m).
    pub height: f32,
    /// Offset Y au-dessus du bot parent (m).
    pub y_offset: f32,
    /// Durée visible totale (s) — fade démarre à `lifetime - fade_out_secs`.
    pub lifetime: f32,
    /// Durée du fade-out final (s).
    pub fade_out_secs: f32,
    /// Couleur RGB du fill HP plein (linear).
    pub fill_color: [f32; 3],
    /// Couleur RGB du background bar (linear).
    pub bg_color: [f32; 3],
    /// Couleur RGB du bord (linear).
    pub border_color: [f32; 3],
    /// Épaisseur bord en UV [0..1].
    pub border_thickness: f32,
    /// Couleur RGB du fill Bouclier (linear) — story-644 P1 Inc.2 (barre au-dessus de la HP).
    pub shield_color: [f32; 3],
    /// Couleur RGB du fill Armure (linear) — story-644 P1 Inc.2.
    pub armor_color: [f32; 3],
}

impl Default for EnemyNameplateTuning {
    fn default() -> Self {
        Self {
            width: 1.4,
            height: 0.18,
            y_offset: 2.4,
            lifetime: 2.5,
            fade_out_secs: 0.6,
            fill_color: [0.95, 0.15, 0.15],
            bg_color: [0.05, 0.05, 0.08],
            border_color: [0.0, 0.0, 0.0],
            border_thickness: 0.06,
            // Bouclier bleu électrique, Armure jaune (miroir enemy_nameplate.toml).
            shield_color: [0.28, 0.55, 0.95],
            armor_color: [0.92, 0.76, 0.24],
        }
    }
}

#[derive(Resource)]
pub struct EnemyNameplateTuningHandle(pub Handle<Genome<EnemyNameplateTuning>>);

/// Resource runtime — sync depuis genome chargé.
#[derive(Resource, Default, Debug, Clone)]
pub struct EnemyNameplate(pub EnemyNameplateTuning);

pub(crate) fn register_tuning(app: &mut App) {
    app.init_asset::<Genome<EnemyNameplateTuning>>()
        .register_asset_loader(GenomeLoader::<EnemyNameplateTuning>::default())
        .init_resource::<EnemyNameplate>()
        .add_systems(Startup, load_tuning)
        .add_systems(Update, sync_tuning);
}

fn load_tuning(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Genome<EnemyNameplateTuning>> =
        asset_server.load("genomes/ui/enemy_nameplate.toml");
    commands.insert_resource(EnemyNameplateTuningHandle(handle));
}

fn sync_tuning(
    handle: Option<Res<EnemyNameplateTuningHandle>>,
    assets: Res<Assets<Genome<EnemyNameplateTuning>>>,
    mut runtime: ResMut<EnemyNameplate>,
) {
    let Some(g) = handle.as_deref().and_then(|h| assets.get(&h.0)) else {
        return;
    };
    runtime.0 = g.data.clone();
}
