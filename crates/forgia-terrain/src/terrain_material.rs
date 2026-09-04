//! Minimal shared terrain material — StandardMaterial PBR + V1 grass textures.
//!
//! W1 vertical slice : 1 seul material partagé par tous les chunks, vertex
//! colors fournissent la teinte par biome. Pas de splat tri-planar (V2 ultérieur).
//!
//! Sampler en `AddressMode::Repeat` pour matcher le tiling UV >1 du mesh
//! heightmap (cf. `meshing_heightmap::TEXTURE_TILES_PER_CHUNK`). Sans Repeat,
//! les UV au-delà de 1.0 sont clamped → texture étirée + stries.

use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;

/// Shared handle inserted as a Resource by `ForgiaTerrainPlugin` on Startup.
/// Tous les chunks meshent avec ce material. Asset path = junction `textures-v1/`.
#[derive(Resource, Clone)]
pub struct TerrainSharedMaterial(pub Handle<StandardMaterial>);

/// Closure factory : settings sampler en `Repeat` sur U/V/W, base linear filter.
/// Réutilisable par tout asset terrain (chunks, paths) qui a besoin de tiling
/// au-delà de UV>1.0. Sans ça, le sampler clamp et étire la texture.
pub fn repeat_sampler() -> impl Fn(&mut ImageLoaderSettings) + Send + Sync + 'static {
    |s: &mut ImageLoaderSettings| {
        s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            address_mode_w: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
    }
}

pub fn init_terrain_material(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let diff: Handle<Image> =
        asset_server.load_with_settings("textures-v1/terrain/grass/diff.jpg", repeat_sampler());
    let normal: Handle<Image> =
        asset_server.load_with_settings("textures-v1/terrain/grass/normal.jpg", repeat_sampler());
    let rough: Handle<Image> = asset_server
        .load_with_settings("textures-v1/terrain/grass/roughness.jpg", repeat_sampler());

    let handle = materials.add(StandardMaterial {
        base_color: Color::WHITE, // multiplié par vertex color
        base_color_texture: Some(diff),
        normal_map_texture: Some(normal),
        metallic_roughness_texture: Some(rough),
        perceptual_roughness: 0.90,
        reflectance: 0.05,
        ..default()
    });

    commands.insert_resource(TerrainSharedMaterial(handle));
}
