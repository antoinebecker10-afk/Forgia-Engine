// toon.wgsl — Cel-shading band quantization post-process.
// Consumed by crate : forgia-pp-toon
//
// Pipeline : sample screen color -> quantize luminance -> Sobel outline -> output.
// Toon and outline intentionally share ONE fullscreen pass. Two independent
// FullscreenMaterial passes crash wgpu/SurfaceTexture on Bevy 0.18.
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
    outline_thickness: f32,
    outline_threshold: f32,
    outline_strength: f32,
    _pad0: f32,
    _pad1: f32,
    edge_color: vec4<f32>,
}
@group(0) @binding(2) var<uniform> settings: ToonSettings;

// Rec.709 luminance
fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn sample_lum(uv: vec2<f32>) -> f32 {
    return luminance(textureSample(screen_texture, texture_sampler, uv).rgb);
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

    let toon_color = mix(color.rgb, toon, settings.strength);

    let tex_size = vec2<f32>(textureDimensions(screen_texture));
    let texel = settings.outline_thickness / tex_size;
    let tl = sample_lum(in.uv + vec2<f32>(-texel.x, -texel.y));
    let tc = sample_lum(in.uv + vec2<f32>(0.0, -texel.y));
    let tr = sample_lum(in.uv + vec2<f32>(texel.x, -texel.y));
    let ml = sample_lum(in.uv + vec2<f32>(-texel.x, 0.0));
    let mr = sample_lum(in.uv + vec2<f32>(texel.x, 0.0));
    let bl = sample_lum(in.uv + vec2<f32>(-texel.x, texel.y));
    let bc = sample_lum(in.uv + vec2<f32>(0.0, texel.y));
    let br = sample_lum(in.uv + vec2<f32>(texel.x, texel.y));
    let gx = (tr + 2.0 * mr + br) - (tl + 2.0 * ml + bl);
    let gy = (bl + 2.0 * bc + br) - (tl + 2.0 * tc + tr);
    let magnitude = sqrt(gx * gx + gy * gy);
    let edge = smoothstep(
        settings.outline_threshold,
        settings.outline_threshold * 2.0,
        magnitude,
    );
    let outlined = mix(
        toon_color,
        settings.edge_color.rgb,
        edge * settings.outline_strength,
    );
    return vec4<f32>(outlined, color.a);
}
