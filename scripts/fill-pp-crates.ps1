# Fill all forgia-pp-* crates with FullscreenMaterial pattern (Bevy 0.18.1).
# Skips toon + outline (already done). Generates lib.rs from template + ensures shader stub exists.
# Idempotent : only writes if current lib.rs contains "TODO: implement" or has the default scaffolded stub.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$cratesDir = Join-Path $root "crates"
$shadersDir = Join-Path $root "assets/shaders/post_process"

# Map crate-name -> (settings field list as "name,default" pairs, shader_basename)
# Default settings : `strength: f32` only. Specific crates can override.
$ppList = @(
    "sketch","halftone","hex-pixelate","pixelation","kuwahara","oil-painting",
    "emboss","sobel-edge","cross-hatch","ascii","matrix","crt","scanlines",
    "bloom","tonemapping","color-grading","vibrance","sepia","grayscale","invert","posterize",
    "palette-quantize","auto-exposure",
    "chromatic-aberration","glitch","distortion","swirl","kaleidoscope","color-split",
    "radial-blur","gaussian-blur","motion-blur","tilt-shift",
    "vignette","fog","distance-fog","rain","heat-haze","frosted-glass","night-vision","thermal",
    "dream","light-streaks"
)

function To-PascalCase($s) {
    ($s -split '-' | ForEach-Object { $_.Substring(0,1).ToUpper() + $_.Substring(1) }) -join ''
}

function To-Snake($s) {
    $s -replace '-','_'
}

$count = 0
foreach ($name in $ppList) {
    $crateName = "forgia-pp-$name"
    $crateDir = Join-Path $cratesDir $crateName
    if (-not (Test-Path $crateDir)) {
        Write-Host "[miss] $crateName" -ForegroundColor Yellow
        continue
    }
    $libPath = Join-Path $crateDir "src/lib.rs"
    $current = if (Test-Path $libPath) { Get-Content $libPath -Raw } else { "" }
    if ($current -notmatch "TODO: implement") {
        Write-Host "[skip] $crateName (already filled)" -ForegroundColor DarkGray
        continue
    }

    $pascal = To-PascalCase $name
    $snake = To-Snake $name
    $settingsType = "${pascal}Settings"
    $pluginType = "ForgiaPp${pascal}Plugin"
    $shaderRel = "shaders/post_process/$snake.wgsl"

    $lib = @"
//! forgia-pp-$name — Post-process effect.
//!
//! Bevy 0.18.1 — uses ``FullscreenMaterialPlugin<T>``.
//!
//! Usage :
//! ``````ignore
//! app.add_plugins($pluginType);
//! commands.spawn((Camera3d::default(), ${settingsType}::default()));
//! ``````

use bevy::core_pipeline::core_3d::graph::Node3d;
use bevy::core_pipeline::fullscreen_material::{FullscreenMaterial, FullscreenMaterialPlugin};
use bevy::prelude::*;
use bevy::render::extract_component::ExtractComponent;
use bevy::render::render_graph::{InternedRenderLabel, RenderLabel};
use bevy::render::render_resource::ShaderType;
use bevy::shader::ShaderRef;

/// Post-process settings. Attach to a ``Camera3d``.
#[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Debug)]
pub struct $settingsType {
    pub strength: f32,
    pub _pad: Vec3,
}

impl Default for $settingsType {
    fn default() -> Self {
        Self { strength: 1.0, _pad: Vec3::ZERO }
    }
}

impl FullscreenMaterial for $settingsType {
    fn fragment_shader() -> ShaderRef {
        "$shaderRel".into()
    }

    fn node_edges() -> Vec<InternedRenderLabel> {
        vec![
            Node3d::Tonemapping.intern(),
            Self::node_label().intern(),
            Node3d::EndMainPassPostProcessing.intern(),
        ]
    }
}

pub struct $pluginType;

impl Plugin for $pluginType {
    fn build(&self, app: &mut App) {
        app.add_plugins(FullscreenMaterialPlugin::<$settingsType>::default());
    }
}
"@

    Set-Content -Path $libPath -Value $lib -Encoding UTF8

    # Ensure shader stub exists with proper Settings struct
    $shaderPath = Join-Path $shadersDir "$snake.wgsl"
    $shaderCur = if (Test-Path $shaderPath) { Get-Content $shaderPath -Raw } else { "" }
    if ($shaderCur -match "TODO: implement") {
        $wgsl = @"
// $snake.wgsl — post-process shader for forgia-pp-$name.
// Bindings auto-bound by FullscreenMaterialPlugin :
//   @group(0) @binding(0) : screen_texture
//   @group(0) @binding(1) : texture_sampler
//   @group(0) @binding(2) : settings (uniform)

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct $settingsType {
    strength: f32,
};

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: $settingsType;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    // TODO: implement $name effect. Currently passthrough * strength.
    return vec4<f32>(color.rgb * settings.strength, color.a);
}
"@
        Set-Content -Path $shaderPath -Value $wgsl -Encoding UTF8
    }

    Write-Host "[fill] $crateName" -ForegroundColor Green
    $count++
}

Write-Host ""
Write-Host "Filled $count PP crates." -ForegroundColor Cyan
