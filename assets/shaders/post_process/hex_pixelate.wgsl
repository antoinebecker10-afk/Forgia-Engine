// hex_pixelate.wgsl — post-process shader for forgia-pp-hex-pixelate.
// Bindings auto-bound by FullscreenMaterialPlugin :
//   @group(0) @binding(0) : screen_texture
//   @group(0) @binding(1) : texture_sampler
//   @group(0) @binding(2) : settings (uniform)

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct HexPixelateSettings {
    strength: f32,
};

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: HexPixelateSettings;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    // TODO: implement hex-pixelate effect. Currently passthrough * strength.
    return vec4<f32>(color.rgb * settings.strength, color.a);
}
