//! forgia-pp-hex-pixelate — Post-process effect.
//!
//! Bevy 0.18.1 — uses `FullscreenMaterialPlugin<T>`.
//!
//! Usage :
//! ```ignore
//! app.add_plugins(ForgiaPpHexPixelatePlugin);
//! commands.spawn((Camera3d::default(), HexPixelateSettings::default()));
//! ```

use bevy::core_pipeline::core_3d::graph::Node3d;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_graph::{InternedRenderLabel, RenderLabel};
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

/// Post-process settings. Attach to a `Camera3d`.
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Debug)]
pub struct HexPixelateSettings {
    pub strength: f32,
    pub _pad: Vec3,
}

impl Default for HexPixelateSettings {
    fn default() -> Self {
        Self { strength: 1.0, _pad: Vec3::ZERO }
    }
}

impl FullscreenMaterial for HexPixelateSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/hex_pixelate.wgsl".into()
    }

    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

pub struct ForgiaPpHexPixelatePlugin;

impl Plugin for ForgiaPpHexPixelatePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<HexPixelateSettings>::default());
    }
}
