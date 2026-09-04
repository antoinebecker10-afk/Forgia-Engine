#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_types::{PbrInput, pbr_input_new},
    pbr_functions,
    mesh_view_bindings::view,
}

// ─── Terrain Array Uniforms ───
struct TerrainParams {
    triplanar_scale: f32,
    triplanar_sharpness: f32,
    slope_threshold: f32,
    slope_blend: f32,
    snow_altitude: f32,
    snow_blend: f32,
    texture_influence: f32,
    debug_mode: u32,
    dirt_ratio: f32,
    patch_scale: f32,
    plains_mix: f32,
    color_warmth: f32,
    moss_coverage: f32,
    moss_height_max: f32,
    moss_slope_max: f32,
    wetness: f32,
    snow_coverage: f32,
    // story-312 Vague 1 — Color Script
    macro_color_strength: f32,
    macro_variation_scale: f32,
    biome_primary: vec3<f32>,
    biome_secondary: vec3<f32>,
    biome_accent: vec3<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> params: TerrainParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var diffuse_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var diffuse_sampler: sampler;

// ─── Utility ───

fn hash2d(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2d(i);
    let b = hash2d(i + vec2<f32>(1.0, 0.0));
    let c = hash2d(i + vec2<f32>(0.0, 1.0));
    let d = hash2d(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn smoothstep_manual(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0 + 0.0001), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// ─── Anti-tile rotation sample (story-324) ──────────────────────────────────
// Sample texture at 2 rotations + blend by spatial noise.
// Eliminates the visible grid repetition that ruins outdoor terrain at distance.
// Reference : Quilez tech, used in Horizon Zero Dawn, Death Stranding.
fn rotate_uv(uv: vec2<f32>, angle: f32) -> vec2<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec2<f32>(uv.x * c - uv.y * s, uv.x * s + uv.y * c);
}

fn sample_antitile(
    layer: i32,
    uv: vec2<f32>,
    world_xz: vec2<f32>,
) -> vec4<f32> {
    let col_a = textureSample(diffuse_array, diffuse_sampler, uv, layer);
    // Rotated sample : offset + 137° (golden angle = max anti-correlation)
    let uv_b = rotate_uv(uv + vec2<f32>(7.3, 13.1), 2.399);
    let col_b = textureSample(diffuse_array, diffuse_sampler, uv_b, layer);
    // Blend factor varies spatially via low-freq noise — no visible grid
    let blend = value_noise(world_xz * 0.04);
    return mix(col_a, col_b, blend);
}

// ─── Multi-octave detail noise (story-324) ──────────────────────────────────
// Procedural micro-detail to break the flat triplanar look at close range.
// 3 octaves with different scales : ~7m / ~1.5m / ~0.4m patches.
// Pure procedural -- no extra texture binding.
fn detail_noise_3oct(p: vec2<f32>) -> f32 {
    let n1 = value_noise(p * 0.15);          // ~7m wavelength patches
    let n2 = value_noise(p * 0.65 + 31.7);  // ~1.5m wavelength patches
    let n3 = hash2d(p * 2.5 + 88.0);         // ~0.4m grain
    return n1 * 0.55 + n2 * 0.30 + n3 * 0.15;
}

// ─── Triplanar sampling from texture array ───
// Kept as fallback / for compatibility, but biplanar is now the default.

fn triplanar_sample(
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    layer: i32,
    scale: f32,
    sharpness: f32,
) -> vec4<f32> {
    let uv_x = world_pos.yz * scale;
    let uv_y = world_pos.xz * scale;
    let uv_z = world_pos.xy * scale;

    let blend = pow(abs(world_normal), vec3<f32>(sharpness));
    let blend_sum = blend.x + blend.y + blend.z + 0.0001;
    let w = blend / blend_sum;

    let tex_x = textureSample(diffuse_array, diffuse_sampler, uv_x, layer);
    let tex_y = textureSample(diffuse_array, diffuse_sampler, uv_y, layer);
    let tex_z = textureSample(diffuse_array, diffuse_sampler, uv_z, layer);

    return tex_x * w.x + tex_y * w.y + tex_z * w.z;
}

// ─── Biplanar sampling (Inigo Quilez) — story-312 v2 ────────────────────────
// Drop the smallest of 3 axes — sample only 2 planes instead of 3.
// 33% bandwidth reduction vs triplanar with quasi-identical visual quality.
// Reference: https://iquilezles.org/articles/biplanar/
fn biplanar_sample(
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    layer: i32,
    scale: f32,
    sharpness: f32,
) -> vec4<f32> {
    let n = abs(world_normal);

    // Major axis (largest) and median axis (middle) — drop the smallest
    var ma_axis: i32;
    var me_axis: i32;
    if n.x >= n.y && n.x >= n.z {
        ma_axis = 0;
        if n.y >= n.z { me_axis = 1; } else { me_axis = 2; }
    } else if n.y >= n.z {
        ma_axis = 1;
        if n.x >= n.z { me_axis = 0; } else { me_axis = 2; }
    } else {
        ma_axis = 2;
        if n.x >= n.y { me_axis = 0; } else { me_axis = 1; }
    }

    // Build UVs for the 2 dominant planes
    var uv_ma: vec2<f32>;
    if ma_axis == 0 {
        uv_ma = world_pos.yz * scale;
    } else if ma_axis == 1 {
        uv_ma = world_pos.xz * scale;
    } else {
        uv_ma = world_pos.xy * scale;
    }

    var uv_me: vec2<f32>;
    if me_axis == 0 {
        uv_me = world_pos.yz * scale;
    } else if me_axis == 1 {
        uv_me = world_pos.xz * scale;
    } else {
        uv_me = world_pos.xy * scale;
    }

    // Story-324 : anti-tile rotation samples (2 rotations blended by spatial noise)
    let col_ma = sample_antitile(layer, uv_ma, world_pos.xz);
    let col_me = sample_antitile(layer, uv_me, world_pos.xz);

    // Quilez weight blend — local support + sharpness shaping
    var w = vec2<f32>(n[ma_axis], n[me_axis]);
    w = clamp((w - vec2<f32>(0.5773)) / vec2<f32>(1.0 - 0.5773), vec2<f32>(0.0), vec2<f32>(1.0));
    w = pow(w, vec2<f32>(sharpness / 8.0));
    return (col_ma * w.x + col_me * w.y) / (w.x + w.y + 0.0001);
}

// ─── Height-blend (Mikkelsen / JCGT 2022) — story-312 v2 ────────────────────
// Blend 2 colours using luminance as pseudo-height.
// Highest "bump" wins → realistic transitions (rock vs grass, snow vs rock).
// Replaces flat mix() at material boundaries.
fn height_blend2(a: vec4<f32>, b: vec4<f32>, t: f32, bias: f32) -> vec4<f32> {
    let lum_a = dot(a.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let lum_b = dot(b.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let ha = lum_a + (1.0 - t);
    let hb = lum_b + t;
    let m = max(ha, hb) - bias;
    let wa = max(ha - m, 0.0);
    let wb = max(hb - m, 0.0);
    return (a * wa + b * wb) / (wa + wb + 0.0001);
}

// ─── Fragment ───

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    // ── Debug modes (unlit, bypass PBR) — Shift+F8 ──
    // Mode 99 = solid red smoke test (confirms shader runs at all)
    if params.debug_mode == 99u {
        var dbg99: FragmentOutput;
        dbg99.color = vec4<f32>(1.0, 0.0, 0.0, 1.0);
        return dbg99;
    }
    // Mode 1 = world normals visualization
    if params.debug_mode == 1u {
        let wn = normalize(in.world_normal);
        var dbg1: FragmentOutput;
        dbg1.color = vec4<f32>(wn * 0.5 + 0.5, 1.0);
        return dbg1;
    }
    // Mode 2 = triplanar weights (R=X, G=Y, B=Z)
    if params.debug_mode == 2u {
        let wn = normalize(in.world_normal);
        let blend = pow(abs(wn), vec3<f32>(params.triplanar_sharpness));
        let bsum = blend.x + blend.y + blend.z + 0.0001;
        let w = blend / bsum;
        var dbg2: FragmentOutput;
        dbg2.color = vec4<f32>(w.x, w.y, w.z, 1.0);
        return dbg2;
    }
    // Mode 3 = UV fract pattern (world XZ * scale)
    if params.debug_mode == 3u {
        let uv = fract(in.world_position.xz * params.triplanar_scale);
        var dbg3: FragmentOutput;
        dbg3.color = vec4<f32>(uv.x, uv.y, 0.5, 1.0);
        return dbg3;
    }
    // Mode 4 = slope (uv_b.y) — green = flat, red = steep
    if params.debug_mode == 4u {
        let s = in.uv_b.y;
        var dbg4: FragmentOutput;
        dbg4.color = vec4<f32>(1.0 - s, s, 0.0, 1.0);
        return dbg4;
    }
    // Mode 5 = biome index (uv_b.x) — gradient by biome
    if params.debug_mode == 5u {
        let t = in.uv_b.x;
        var dbg5: FragmentOutput;
        dbg5.color = vec4<f32>(t, fract(t * 10.0), 1.0 - t, 1.0);
        return dbg5;
    }

    // ── PBR input setup (manual mapping from VertexOutput, official Bevy pattern) ──
    var pbr_input: PbrInput = pbr_input_new();
    pbr_input.frag_coord = in.position;
    pbr_input.world_position = in.world_position;
    pbr_input.world_normal = normalize(in.world_normal);
    pbr_input.is_orthographic = view.clip_from_view[3].w == 1.0;
    pbr_input.N = pbr_input.world_normal;
    pbr_input.V = pbr_functions::calculate_view(in.world_position, pbr_input.is_orthographic);

    // Flip normal for back faces (matches Triplanar shader behavior)
    if !is_front {
        pbr_input.world_normal = -pbr_input.world_normal;
        pbr_input.N = -pbr_input.N;
    }

    // Bail early if texture not yet loaded (diffuse_array handle is placeholder)
    if params.texture_influence < 0.01 {
        // Use vertex colors only (StandardMaterial fallback behavior)
        pbr_input.material.base_color = in.color;
        pbr_input.material.perceptual_roughness = 0.85;
        pbr_input.material.metallic = 0.05;
        var fout: FragmentOutput;
        fout.color = pbr_functions::apply_pbr_lighting(pbr_input);
        return fout;
    }

    let world_pos = in.world_position.xyz;
    let world_normal = pbr_input.world_normal;

    // UV1 (uv_b): x = biome layer index (normalized 0.0-0.9), y = slope factor
    let biome_idx = i32(round(in.uv_b.x * 10.0));
    let slope_factor = in.uv_b.y; // 1.0 = flat, 0.0 = vertical

    // Story-324 fix : reverted to triplanar_sample because biplanar was suspected
    // of producing the lavender/white veil at certain camera angles. The biplanar
    // weight blend `pow(w, sharpness/8)` can amplify a single axis when 2 axes
    // have similar magnitude (e.g. corners of cubes) → over-bright sample.
    // Triplanar uses all 3 axes with stable weights → safer.
    let scale = params.triplanar_scale;
    let sharpness = params.triplanar_sharpness;
    let biome_color = triplanar_sample(world_pos, world_normal, biome_idx, scale, sharpness);

    // Rock layer for steep slopes (layer 3 = Mountain/cliff)
    let rock_color = triplanar_sample(world_pos, world_normal, 3, scale * 0.8, sharpness);

    // Slope-based HEIGHT-BLEND (story-312 v2 — Mikkelsen) :
    // Instead of flat mix(), use luminance as pseudo-height so rocks "win" on outcrops
    // and grass keeps concavities — eliminates muddy transitions.
    let slope_blend = smoothstep_manual(params.slope_threshold, params.slope_threshold + params.slope_blend, slope_factor);
    // Blend factor : 0 = full rock, 1 = full biome (slope)
    var terrain_color = height_blend2(rock_color, biome_color, slope_blend, 0.10);

    // Vertex color tint (preserves the procedural color variation from CPU side)
    let vertex_rgb = in.color.rgb;

    // Blend: texture × vertex color tint
    let textured = terrain_color.rgb * vertex_rgb * 2.0; // 2x for overlay-like effect
    var final_rgb = mix(vertex_rgb, textured, params.texture_influence);

    // ─── story-312 Vague 1 — Color Script application ───
    // Macro variation noise (km-scale, low-freq) — breaks the visible tiling
    let macro_uv = world_pos.xz * params.macro_variation_scale;
    let macro_noise = value_noise(macro_uv);
    // Mix biome primary <-> secondary based on macro noise (zones de teinte)
    let biome_tint = mix(params.biome_primary, params.biome_secondary, macro_noise);
    // Apply biome tint as a soft multiply (texture stays dominant)
    let tinted = final_rgb * (biome_tint * 2.0);
    final_rgb = mix(final_rgb, tinted, params.macro_color_strength);
    // Accent boost on bright pixels (rim/highlight effect — hot spots de couleur)
    // Story-324 fix : tone down (was 1.5x → 1.15x) to prevent white blowout on
    // bright pixels when combined with PBR specular at grazing angles.
    let luminance = dot(final_rgb, vec3<f32>(0.299, 0.587, 0.114));
    let accent_factor = smoothstep_manual(0.55, 0.85, luminance) * params.macro_color_strength * 0.3;
    final_rgb = mix(final_rgb, final_rgb * params.biome_accent * 1.15, accent_factor);
    // Hard clamp to prevent any single channel exceeding 1.0 before PBR lighting
    final_rgb = clamp(final_rgb, vec3<f32>(0.0), vec3<f32>(1.0));

    // ─── Story-324 — Multi-octave detail noise micro-variation ──────────────
    // Modulates the final color with 3 octaves of procedural noise to break
    // the flat triplanar look and add per-pixel grain. Slope-aware : flat
    // surfaces get more detail (visible to player), steep surfaces get less.
    let detail = detail_noise_3oct(world_pos.xz);
    // Map detail [0..1] -> [0.85..1.15] luminance modulation
    let detail_mod = 0.85 + detail * 0.30;
    // Slope mask : flat = full effect, vertical = half effect (avoid striping on cliffs)
    let detail_strength = mix(0.5, 1.0, slope_factor);
    final_rgb = final_rgb * mix(1.0, detail_mod, detail_strength);

    // Per-pixel concavity darkening (Houdini-style cavity AO from noise)
    let cavity = value_noise(world_pos.xz * 0.4 + vec2<f32>(77.0, 23.0));
    let cavity_dark = 1.0 - (1.0 - slope_factor) * cavity * 0.12;
    final_rgb = final_rgb * cavity_dark;

    // Roughness from texture alpha (if packed) or fallback
    // Story-324 : add detail-based roughness variation for material depth
    // Story-324 fix : raise floor to 0.55 (was 0.05) — terrain is matte/rough
    // by nature (rocks, grass, dirt). Allowing roughness < 0.5 enables SSR
    // (threshold 0.5) which causes the "ground turns white when looking from
    // certain angles" bug — SSR reflects bright sky onto terrain.
    let base_roughness = mix(0.85, terrain_color.a, params.texture_influence);
    let roughness = clamp(base_roughness + (detail - 0.5) * 0.10, 0.55, 0.99);

    // Wetness: darken + reduce roughness
    // Story-324 fix : wet floor 0.55 (was 0.1) to keep SSR off
    let wet_darken = 1.0 - params.wetness * 0.3;
    let wet_roughness = mix(roughness, 0.55, params.wetness);

    pbr_input.material.base_color = vec4<f32>(final_rgb * wet_darken, 1.0);
    pbr_input.material.perceptual_roughness = wet_roughness;
    pbr_input.material.metallic = 0.0;  // Story-324 : terrain is non-metallic (was 0.05)

    var fout: FragmentOutput;
    fout.color = pbr_functions::apply_pbr_lighting(pbr_input);
    return fout;
}
