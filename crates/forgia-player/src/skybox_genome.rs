//! # skybox_genome
//!
//! Story-555 (Phase 2 de story-554) — palette skybox cartoon data-driven
//! per-biome via `assets/genomes/biome_sky.toml`.
//!
//! ## Flow
//!
//! 1. `Startup` : load `Genome<SkyPaletteGenome>` handle. Resource
//!    `CurrentSkyPalette` initialisée à `SkyPalette::default()` (palette
//!    Crypts canon bible v1) pour éviter écran noir pendant le load async.
//! 2. `Update` : `AssetEvent::Added/Modified/LoadedWithDependencies` sur le
//!    genome → resync palette si biome courant connu (apply hot-reload TOML).
//! 3. `Update` : `Res<StageLoadResult>` is_changed et `.biome != ""` →
//!    lookup palette dans le genome → set `CurrentSkyPalette` si différente.
//! 4. `Update` : `CurrentSkyPalette` is_changed → re-générer le cubemap via
//!    `crate::generate_cartoon_skybox` + remplacer le handle dans
//!    `Assets<Image>` + insérer `Skybox` neuf sur toutes les `Camera3d`.
//!
//! ## Pattern
//!
//! Identique à `forgia-killfeed::tuning` (cf src/tuning.rs:107-142) :
//! `Handle<Genome<T>>` resource + MessageReader AssetEvent listener.
//!
//! ## Hot path
//!
//! Non hot — regen cubemap = 1 alloc 262KB par changement palette (256×256×6×4),
//! déclenché uniquement OnEnter stage OR Shift+F12. Pas par-frame.

use bevy::asset::AssetEvent;
use bevy::core_pipeline::Skybox;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_stage::StageLoadResult;
use serde::Deserialize;
use std::collections::HashMap;

use crate::{generate_cartoon_skybox, SKYBOX_BRIGHTNESS};

/// 3 couleurs RGB 0-255 d'une palette skybox cartoon.
///
/// Default = palette Crypts canon bible v1 (cohérent story-554 Phase 1).
#[derive(Resource, Deserialize, Clone, Copy, Debug, TypePath)]
pub struct SkyPalette {
    pub zenith_rgb: [u8; 3],
    pub horizon_rgb: [u8; 3],
    pub ground_rgb: [u8; 3],
}

impl Default for SkyPalette {
    fn default() -> Self {
        Self {
            zenith_rgb: [61, 36, 102],
            horizon_rgb: [255, 107, 53],
            ground_rgb: [42, 27, 61],
        }
    }
}

/// Resource Resource — palette skybox courante. Re-générée à chaque change.
/// L'image cubemap actuellement attachée aux Cameras est dans `SkyboxPending`
/// (cf lib.rs), elle est remplacée par `regen_skybox_image` à chaque is_changed.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct CurrentSkyPalette(pub SkyPalette);

/// Conteneur TOML genome — clés = biome_id lowercase (e.g. "volcanic",
/// "plains") matchant `StageLoadResult.biome`. Clé "default" obligatoire
/// (fallback hors Roguelite ou biome inconnu).
#[derive(Deserialize, Clone, Debug, TypePath)]
pub struct SkyPaletteGenome {
    #[serde(flatten)]
    pub palettes: HashMap<String, SkyPalette>,
}

impl SkyPaletteGenome {
    /// Renvoie la palette pour `biome_id`, fallback "default", fallback
    /// `SkyPalette::default()` si même "default" est absent.
    ///
    /// Lookup case-insensitive : `StageLoadResult.biome` peut être en
    /// CamelCase (e.g. "Volcanic" depuis l'enum BiomeType) alors que les
    /// clés TOML sont en snake_case lowercase ("volcanic").
    pub fn palette_for(&self, biome_id: &str) -> SkyPalette {
        let key = biome_id.to_ascii_lowercase();
        self.palettes
            .get(key.as_str())
            .copied()
            .or_else(|| self.palettes.get("default").copied())
            .unwrap_or_default()
    }
}

/// Handle storage pour garder l'asset vivant (sinon dropped immédiatement).
#[derive(Resource)]
pub struct SkyPaletteGenomeHandle(pub Handle<Genome<SkyPaletteGenome>>);

const GENOME_PATH: &str = "genomes/biome_sky.toml";

pub struct SkyboxGenomePlugin;

impl Plugin for SkyboxGenomePlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Genome<SkyPaletteGenome>>()
            .register_asset_loader(GenomeLoader::<SkyPaletteGenome>::default())
            .init_resource::<CurrentSkyPalette>()
            .add_systems(Startup, load_sky_palette_genome)
            .add_systems(
                Update,
                (
                    sync_palette_from_genome,
                    track_stage_biome,
                    regen_skybox_image,
                )
                    .chain(),
            );
    }
}

fn load_sky_palette_genome(mut commands: Commands, server: Res<AssetServer>) {
    let handle: Handle<Genome<SkyPaletteGenome>> = server.load(GENOME_PATH);
    commands.insert_resource(SkyPaletteGenomeHandle(handle));
    info!("[forgia-player] Sky palette genome load requested ({GENOME_PATH})");
}

/// On AssetEvent (Added/Modified/Loaded) : re-set CurrentSkyPalette depuis
/// le genome rechargé, en utilisant le biome courant si connu, sinon "default".
fn sync_palette_from_genome(
    mut events: MessageReader<AssetEvent<Genome<SkyPaletteGenome>>>,
    handle: Option<Res<SkyPaletteGenomeHandle>>,
    assets: Res<Assets<Genome<SkyPaletteGenome>>>,
    stage: Option<Res<StageLoadResult>>,
    mut current: ResMut<CurrentSkyPalette>,
) {
    let Some(handle) = handle else { return };
    let mut should = false;
    for ev in events.read() {
        match ev {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id } => {
                if *id == handle.0.id() {
                    should = true;
                }
            }
            _ => {}
        }
    }
    if !should {
        return;
    }
    let Some(g) = assets.get(&handle.0) else {
        return;
    };
    let biome_id_raw = stage
        .as_ref()
        .map(|s| s.biome.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("default");
    let biome_id = biome_id_raw.to_ascii_lowercase();
    let new_palette = g.data.palette_for(&biome_id);
    current.0 = new_palette;
    info!(
        "[forgia-player] Skybox palette loaded from genome ({} biomes, applied '{biome_id}')",
        g.data.palettes.len()
    );
}

/// Track changements du `StageLoadResult.biome` (ex: OnEnter nouveau stage
/// Roguelite). Lookup la palette + set CurrentSkyPalette si différente.
fn track_stage_biome(
    handle: Option<Res<SkyPaletteGenomeHandle>>,
    assets: Res<Assets<Genome<SkyPaletteGenome>>>,
    stage: Option<Res<StageLoadResult>>,
    mut current: ResMut<CurrentSkyPalette>,
    mut last_biome: Local<String>,
) {
    let Some(stage) = stage else { return };
    if !stage.is_changed() {
        return;
    }
    let biome_id = stage.biome.to_ascii_lowercase();
    if biome_id.is_empty() || biome_id == *last_biome {
        return;
    }
    let Some(handle) = handle else { return };
    let Some(g) = assets.get(&handle.0) else {
        return; // genome pas encore loaded, sync_palette_from_genome appliquera plus tard
    };
    let new_palette = g.data.palette_for(&biome_id);
    current.0 = new_palette;
    *last_biome = biome_id.clone();
    info!("[forgia-player] Palette switched to biome '{biome_id}'");
}

/// Si `CurrentSkyPalette` a changé : régénérer le cubemap + remplacer le
/// handle dans `Assets<Image>` + insérer un nouveau `Skybox` sur chaque
/// `Camera3d` existante.
fn regen_skybox_image(
    mut commands: Commands,
    current: Res<CurrentSkyPalette>,
    mut images: ResMut<Assets<Image>>,
    q_cam: Query<Entity, With<Camera3d>>,
) {
    if !current.is_changed() {
        return;
    }
    let new_image = generate_cartoon_skybox(&current.0);
    let new_handle: Handle<Image> = images.add(new_image);

    let mut count = 0;
    for cam in q_cam.iter() {
        commands.entity(cam).insert(Skybox {
            image: new_handle.clone(),
            brightness: SKYBOX_BRIGHTNESS,
            rotation: Quat::IDENTITY,
        });
        count += 1;
    }
    info!(
        "[forgia-player] Skybox regenerated (palette changed) — re-attached to {count} Camera3d(s)"
    );
}
