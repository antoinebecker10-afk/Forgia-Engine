//! Sobel edge outline post-process (cel-shading companion).
//!
//! Genes :
//! - `thickness` : 0.5..3.0 (texel sampling radius)
//! - `threshold` : 0.05..0.5 (gradient cutoff)
//! - `strength`  : 0.0..1.0
//! - `edge_color` : RGBA

use bevy::core_pipeline::core_3d::graph::Node3d;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_graph::{InternedRenderLabel, RenderLabel};
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Debug)]
pub struct OutlineSettings {
    pub thickness: f32,
    pub threshold: f32,
    pub strength: f32,
    pub _pad: f32,
    pub edge_color: Vec4,
}

impl Default for OutlineSettings {
    fn default() -> Self {
        Self {
            thickness: 1.0,
            threshold: 0.15,
            strength: 1.0,
            _pad: 0.0,
            edge_color: Vec4::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

impl FullscreenMaterial for OutlineSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/outline.wgsl".into()
    }

    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

pub struct ForgiaPpOutlinePlugin;

impl Plugin for ForgiaPpOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<OutlineSettings>::default());
    }
}
