//! forgia-pp-toon — Cel-shading band quantization post-process.
//!
//! Bevy 0.18.1 — uses `FullscreenMaterialPlugin<T>` (high-level PP API).
//!
//! Usage :
//! ```ignore
//! app.add_plugins(ForgiaPpToonPlugin);
//! commands.spawn((Camera3d::default(), ToonSettings::default()));
//! ```

use bevy::core_pipeline::core_3d::graph::Node3d;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_graph::{InternedRenderLabel, RenderLabel};
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

/// Toon post-process settings. Attach to a `Camera3d` entity.
///
/// Genes (hot-reload candidate, see manifest.toml) :
/// - `bands` : 2.0..16.0 (typical 4.0)
/// - `strength` : 0.0..1.0
/// - `edge_dark` : 0.0..1.0
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Debug)]
pub struct ToonSettings {
    pub bands: f32,
    pub strength: f32,
    pub edge_dark: f32,
    pub _pad: f32,
}

impl Default for ToonSettings {
    fn default() -> Self {
        Self {
            bands: 4.0,
            strength: 1.0,
            edge_dark: 0.15,
            _pad: 0.0,
        }
    }
}

impl FullscreenMaterial for ToonSettings {
    fn fragment_shader() -> ShaderRef {
        "shaders/post_process/toon.wgsl".into()
    }

    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

pub struct ForgiaPpToonPlugin;

impl Plugin for ForgiaPpToonPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<ToonSettings>::default());
    }
}
