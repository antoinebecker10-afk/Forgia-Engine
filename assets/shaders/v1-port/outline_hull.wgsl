// Outline via inverted hull — Story 350 Phase 1 J2.
//
// Technique AAA standard (Overwatch / Genshin / Borderlands) :
//   - Render le mesh une seconde fois extrudé le long des normales
//   - Cull = Front (on ne voit que l'arrière, donc juste la silhouette)
//   - Fragment = noir uni → contour net autour du mesh principal
//
// Avantage vs post-process sobel : zéro render graph custom, fonctionne
// per-mesh, contrôle de l'épaisseur par mesh, robuste aux creases.

#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_clip, mesh_normal_local_to_world}

struct OutlineParams {
    color: vec4<f32>,        // RGB du contour, A inutile
    thickness: f32,          // unités monde (≈ 0.03 = 3 cm)
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: OutlineParams;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal:   vec3<f32>,
    @location(2) uv:       vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vertex(in: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = get_world_from_local(in.instance_index);
    let world_normal     = normalize(mesh_normal_local_to_world(in.normal, in.instance_index));
    // Extrude la position le long de la normale en world space.
    let extruded_local   = in.position + in.normal * params.thickness;
    out.clip_position    = mesh_position_local_to_clip(world_from_local, vec4<f32>(extruded_local, 1.0));
    return out;
}

@fragment
fn fragment(_in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(params.color.rgb, 1.0);
}
