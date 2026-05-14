// outline.wgsl — Sobel edge outline post-process (cel-shading companion).
// Consumed by crate : forgia-pp-outline
//
// Pipeline : 3x3 sobel on luminance -> threshold -> darken edges.
// Settings uniform (group 0 binding 2) :
//   thickness  : f32 (texel sampling radius, 0.5..3.0)
//   threshold  : f32 (gradient mag cutoff, 0.05..0.5)
//   edge_color : vec3<f32> (typical black 0,0,0)
//   strength   : f32 (0=off, 1=full)

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct OutlineSettings {
    thickness: f32,
    threshold: f32,
    strength: f32,
    _pad: f32,
    edge_color: vec4<f32>,
}
@group(0) @binding(2) var<uniform> settings: OutlineSettings;

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn sample_lum(uv: vec2<f32>) -> f32 {
    return luminance(textureSample(screen_texture, texture_sampler, uv).rgb);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    let tex_size = vec2<f32>(textureDimensions(screen_texture));
    let texel = settings.thickness / tex_size;

    // 3x3 sobel
    let tl = sample_lum(in.uv + vec2<f32>(-texel.x, -texel.y));
    let tc = sample_lum(in.uv + vec2<f32>( 0.0,     -texel.y));
    let tr = sample_lum(in.uv + vec2<f32>( texel.x, -texel.y));
    let ml = sample_lum(in.uv + vec2<f32>(-texel.x,  0.0));
    let mr = sample_lum(in.uv + vec2<f32>( texel.x,  0.0));
    let bl = sample_lum(in.uv + vec2<f32>(-texel.x,  texel.y));
    let bc = sample_lum(in.uv + vec2<f32>( 0.0,      texel.y));
    let br = sample_lum(in.uv + vec2<f32>( texel.x,  texel.y));

    let gx = (tr + 2.0 * mr + br) - (tl + 2.0 * ml + bl);
    let gy = (bl + 2.0 * bc + br) - (tl + 2.0 * tc + tr);
    let mag = sqrt(gx * gx + gy * gy);

    let edge = smoothstep(settings.threshold, settings.threshold * 2.0, mag);
    let out = mix(color.rgb, settings.edge_color.rgb, edge * settings.strength);
    return vec4<f32>(out, color.a);
}
