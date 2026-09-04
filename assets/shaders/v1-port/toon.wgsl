// Toon / cel-shading material — Story 350 Phase 1 J1 (Overwatch-like).
//
// Day 1 = standalone forward-lit shader (hardcoded directional sun) for cube
// debug test. Day 4 will integrate Bevy's PBR lighting / shadows.
//
// Algorithm:
//   - Lambert N·L
//   - Quantize to `cel_bands` discrete steps (Overwatch-like = 3)
//   - Rim light = pow(1 - dot(N, V), rim_power) * rim_intensity
//   - Output = base_color * (band_value + rim)

#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_clip, mesh_normal_local_to_world}
#import bevy_pbr::mesh_view_bindings::view

struct ToonParams {
    base_color: vec4<f32>,    // RGBA tint
    rim_color:  vec4<f32>,    // RGB rim tint, A = rim intensity
    sun_dir:    vec4<f32>,    // XYZ normalized, W unused
    cel_bands:  f32,          // 3.0 for Overwatch
    rim_power:  f32,          // exponent (2.0..6.0)
    _pad0:      f32,
    _pad1:      f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: ToonParams;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) uv:       vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal:   vec3<f32>,
};

@vertex
fn vertex(in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = get_world_from_local(in.instance_index);
    out.clip_position    = mesh_position_local_to_clip(world_from_local, vec4<f32>(in.position, 1.0));
    out.world_position   = (world_from_local * vec4<f32>(in.position, 1.0)).xyz;
    out.world_normal     = normalize(mesh_normal_local_to_world(in.normal, in.instance_index));
    return out;
}

// Quantize a continuous lambert value [0..1] into N discrete bands.
// Overwatch-style 3 bands = shadow / mid / lit.
fn quantize(x: f32, bands: f32) -> f32 {
    let b = max(bands, 1.0);
    return floor(x * b) / b + (0.5 / b);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let l = normalize(params.sun_dir.xyz);

    // Camera position from view uniform (Bevy 0.18 convention).
    let view_pos = view.world_position;
    let v = normalize(view_pos - in.world_position);

    // Lambert + cel quantization. Min band = 0.3 to avoid pitch-black shadows
    // (Overwatch ambient floor).
    let n_dot_l = max(dot(n, l), 0.0);
    let cel     = mix(0.3, 1.0, quantize(n_dot_l, params.cel_bands));

    // Rim light — silhouette glow.
    let rim_raw = pow(1.0 - max(dot(n, v), 0.0), max(params.rim_power, 0.5));
    let rim     = rim_raw * params.rim_color.a;

    let lit = params.base_color.rgb * cel + params.rim_color.rgb * rim;
    return vec4<f32>(lit, params.base_color.a);
}
