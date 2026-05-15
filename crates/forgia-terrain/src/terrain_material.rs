//! Minimal shared terrain material — StandardMaterial PBR + V1 grass textures.
//!
//! W1 vertical slice : 1 seul material partagé par tous les chunks, vertex
//! colors fournissent la teinte par biome. Pas de splat tri-planar (V2 ultérieur).

use bevy::prelude::*;

/// Shared handle inserted as a Resource by `ForgiaTerrainPlugin` on Startup.
/// Tous les chunks meshent avec ce material. Asset path = junction `textures-v1/`.
#[derive(Resource, Clone)]
pub struct TerrainSharedMaterial(pub Handle<StandardMaterial>);

pub fn init_terrain_material(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let diff: Handle<Image> = asset_server.load("textures-v1/terrain/grass/diff.jpg");
    let normal: Handle<Image> = asset_server.load("textures-v1/terrain/grass/normal.jpg");
    let rough: Handle<Image> = asset_server.load("textures-v1/terrain/grass/roughness.jpg");

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
