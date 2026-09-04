// forgia-enemy-nameplate — Custom WGSL shader pour HP bar (story-457).
//
// Status V1 (2026-05-19) : NON UTILISÉ runtime. Le crate utilise actuellement
// 2 quads StandardMaterial unlit (background + fill scale.x = hp_fraction) qui
// shippe sans boilerplate Material custom. Ce shader est livré comme upgrade
// future : single quad + uniform `hp_fraction` + bord arrondi GPU + animation
// flash sur kill.
//
// Pour l'activer : implémenter `Material` trait avec `AsBindGroup` exposant
// les uniforms ci-dessous, remplacer le 2-quads spawn par 1 quad de cette
// matière, brancher uniform update sur Changed<Health>.

#import bevy_pbr::forward_io::VertexOutput

struct NameplateUniforms {
    hp_fraction: f32,      // [0..1]
    alpha: f32,            // fade global
    fill_rgb: vec3<f32>,
    bg_rgb: vec3<f32>,
    border_rgb: vec3<f32>,
    border_thickness: f32, // en UV [0..1]
};

@group(2) @binding(0) var<uniform> u: NameplateUniforms;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    // Bord : si dans la zone bord, return border color.
    let b = u.border_thickness;
    let in_border = uv.x < b || uv.x > 1.0 - b || uv.y < b || uv.y > 1.0 - b;
    if (in_border) {
        return vec4<f32>(u.border_rgb, u.alpha);
    }

    // Inner : fill si uv.x < hp_fraction, sinon bg.
    let inner_uv_x = (uv.x - b) / (1.0 - 2.0 * b);
    let is_fill = inner_uv_x < u.hp_fraction;
    let rgb = select(u.bg_rgb, u.fill_rgb, is_fill);
    return vec4<f32>(rgb, u.alpha);
}
