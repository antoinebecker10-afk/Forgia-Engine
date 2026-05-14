// toon.wgsl — Cel-shading band quantization post-process.
// Consumed by crate : forgia-pp-toon
//
// Pipeline : sample screen color -> quantize luminance to N bands -> output.
// Settings uniform (group 0 binding 2) :
//   bands     : f32 (number of bands, typical 3.0..8.0)
//   strength  : f32 (0=passthrough, 1=full toon)
//   edge_dark : f32 (darken factor for shadow band, 0..1)

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ToonSettings {
    bands: f32,
    strength: f32,
    edge_dark: f32,
    _pad: f32,
}
@group(0) @binding(2) var<uniform> settings: ToonSettings;

// Rec.709 luminance
fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    let lum = luminance(color.rgb);

    // Quantize luminance to N bands
    let bands = max(settings.bands, 2.0);
    let quantized = floor(lum * bands) / bands;

    // Rescale rgb to preserve hue
    let safe_lum = max(lum, 0.001);
    var toon = color.rgb * (quantized / safe_lum);

    // Apply edge_dark to lowest band
    if (quantized < (1.0 / bands)) {
        toon = toon * (1.0 - settings.edge_dark);
    }

    let out = mix(color.rgb, toon, settings.strength);
    return vec4<f32>(out, color.a);
}
