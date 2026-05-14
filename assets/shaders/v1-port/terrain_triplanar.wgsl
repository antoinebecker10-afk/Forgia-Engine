#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_types,
    pbr_functions,
    pbr_fragment::pbr_input_from_vertex_output,
}

// ─── Terrain Triplanar Uniforms ───
struct TerrainParams {
    triplanar_scale: f32,
    triplanar_sharpness: f32,
    slope_threshold: f32,
    slope_blend: f32,
    snow_altitude: f32,
    snow_blend: f32,
    texture_influence: f32,
    debug_mode: u32,
    // Genome-driven terrain visual genes
    dirt_ratio: f32,      // 0-0.7: how much dirt shows through grass
    patch_scale: f32,     // 0.02-0.20: noise frequency for patches (lower = bigger)
    plains_mix: f32,      // 0-1: secondary grass variant ratio
    color_warmth: f32,    // 0-1: cool blue → warm golden tint
    // Material layer overlays (genome-driven)
    moss_coverage: f32,   // 0-1: procedural green overlay on gentle slopes
    moss_height_max: f32, // max height in metres for moss (normalised /128)
    moss_slope_max: f32,  // max slope in degrees for moss (normalised /90)
    wetness: f32,         // 0-1: surface wetness (darkens + smooths)
    snow_coverage: f32,   // 0-1: amplifies altitude-based snow
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> terrain_params: TerrainParams;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var grass_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var grass_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var rock_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var rock_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var sand_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var sand_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(7) var grass_normal_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(8) var grass_normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(9) var forest_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(10) var forest_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(11) var snow_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(12) var snow_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(13) var volcanic_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(14) var volcanic_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(15) var path_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(16) var path_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(17) var rock_normal_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(18) var rock_normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(19) var sand_normal_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(20) var sand_normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(21) var forest_normal_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(22) var forest_normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(23) var path_normal_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(24) var path_normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(25) var dirt_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(26) var dirt_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(27) var dirt_normal_tex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(28) var dirt_normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(29) var plains_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(30) var plains_sampler: sampler;

// Roughness is now per-pixel, packed into diffuse texture alpha channel.
// biome_color.a accumulates weighted roughness through all blending stages.

// ─── Utility Functions ───

fn smoothstep_manual(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0 + 0.0001), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn hash2d(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn hash2d_vec2(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453),
        fract(sin(dot(p, vec2<f32>(269.5, 183.3))) * 43758.5453),
    );
}

// Smooth value noise (2 octaves for natural variation)
fn value_noise_2oct(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash2d(i);
    let b = hash2d(i + vec2<f32>(1.0, 0.0));
    let c = hash2d(i + vec2<f32>(0.0, 1.0));
    let d = hash2d(i + vec2<f32>(1.0, 1.0));
    let n1 = mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
    // Second octave at 2x frequency, 0.5x amplitude
    let p2 = p * 2.0 + 17.3;
    let i2 = floor(p2);
    let f2 = fract(p2);
    let u2 = f2 * f2 * (3.0 - 2.0 * f2);
    let a2 = hash2d(i2);
    let b2 = hash2d(i2 + vec2<f32>(1.0, 0.0));
    let c2 = hash2d(i2 + vec2<f32>(0.0, 1.0));
    let d2 = hash2d(i2 + vec2<f32>(1.0, 1.0));
    let n2 = mix(mix(a2, b2, u2.x), mix(c2, d2, u2.x), u2.y);
    return n1 * 0.67 + n2 * 0.33;
}

// ─── Height-Blend (Mikkelsen / JCGT 2022) ───
// Blend 2 colours using luminance as pseudo-height.
// Highest "bump" wins → realistic transitions (rock vs grass, snow vs rock).
// Replaces flat mix() at critical material boundaries.
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

// ─── Anti-Tiling: Rotation Blending (Quilez-inspired) ───
// Sample texture at 2 rotations, blend with smooth noise to kill repetition.

fn rotate_uv(uv: vec2<f32>, angle: f32) -> vec2<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return vec2<f32>(uv.x * c - uv.y * s, uv.x * s + uv.y * c);
}

fn sample_antitile(
    tex: texture_2d<f32>,
    samp: sampler,
    uv: vec2<f32>,
    world_xz: vec2<f32>,
) -> vec4<f32> {
    // Primary sample
    let col_a = textureSample(tex, samp, uv);
    // Rotated sample (offset + 2.4 radians ≈ 137° golden angle)
    let uv_b = rotate_uv(uv + 7.3, 2.399);
    let col_b = textureSample(tex, samp, uv_b);
    // Blend factor from smooth noise — varies spatially, no visible grid
    let blend = value_noise_2oct(world_xz * 0.04);
    return mix(col_a, col_b, blend);
}

// ─── Triplanar Sampling with Anti-Tiling ───

fn triplanar_weights(normal: vec3<f32>, sharpness: f32) -> vec3<f32> {
    var w = pow(abs(normal), vec3<f32>(sharpness));
    let sum = w.x + w.y + w.z + 0.0001;
    return w / sum;
}

fn sample_triplanar(
    tex: texture_2d<f32>,
    samp: sampler,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    scale: f32,
    sharpness: f32,
) -> vec4<f32> {
    let weights = triplanar_weights(normal, sharpness);
    let col_x = sample_antitile(tex, samp, world_pos.yz * scale, world_pos.yz);
    let col_y = sample_antitile(tex, samp, world_pos.xz * scale, world_pos.xz);
    let col_z = sample_antitile(tex, samp, world_pos.xy * scale, world_pos.xy);
    return col_x * weights.x + col_y * weights.y + col_z * weights.z;
}

fn sample_triplanar_normal(
    tex: texture_2d<f32>,
    samp: sampler,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    scale: f32,
    sharpness: f32,
) -> vec3<f32> {
    let weights = triplanar_weights(world_normal, sharpness);
    let n_x = textureSample(tex, samp, world_pos.yz * scale).rgb * 2.0 - 1.0;
    let n_y = textureSample(tex, samp, world_pos.xz * scale).rgb * 2.0 - 1.0;
    let n_z = textureSample(tex, samp, world_pos.xy * scale).rgb * 2.0 - 1.0;
    let w_x = vec3<f32>(0.0, n_x.y, -n_x.x);
    let w_y = vec3<f32>(n_y.x, 0.0, n_y.y);
    let w_z = vec3<f32>(n_z.x, n_z.y, 0.0);
    return normalize(world_normal + w_x * weights.x + w_y * weights.y + w_z * weights.z);
}

// ─── Biplanar Mapping (Inigo Quilez) ───
// Drops the smallest of the 3 axes — keeps only major + median.
// 2 texture fetches per material instead of 3 → -33% bandwidth vs triplanar.
// Quasi-identical visual quality. Reference: https://iquilezles.org/articles/biplanar/
//
// Combined with sample_antitile (2 rotations per fetch) the saving is:
//   triplanar = 3 axes × 2 rotations = 6 fetches per material
//   biplanar  = 2 axes × 2 rotations = 4 fetches per material
// On ~10 materials per fragment → ~20 fewer fetches per pixel.

fn sample_biplanar(
    tex: texture_2d<f32>,
    samp: sampler,
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    scale: f32,
    sharpness: f32,
) -> vec4<f32> {
    let n = abs(normal);

    // Major axis (largest abs component) — ma.x = axis index, ma.yz = following axes
    var ma: vec3<i32>;
    if n.x > n.y && n.x > n.z      { ma = vec3<i32>(0, 1, 2); }
    else if n.y > n.z              { ma = vec3<i32>(1, 2, 0); }
    else                            { ma = vec3<i32>(2, 0, 1); }

    // Minor axis (smallest abs component)
    var mi: vec3<i32>;
    if n.x < n.y && n.x < n.z      { mi = vec3<i32>(0, 1, 2); }
    else if n.y < n.z              { mi = vec3<i32>(1, 2, 0); }
    else                            { mi = vec3<i32>(2, 0, 1); }

    // Median axis = 3 - major - minor
    let me_axis = 3 - ma.x - mi.x;
    var me: vec3<i32>;
    if me_axis == 0      { me = vec3<i32>(0, 1, 2); }
    else if me_axis == 1 { me = vec3<i32>(1, 2, 0); }
    else                  { me = vec3<i32>(2, 0, 1); }

    // Project world_pos onto the 2 dominant planes
    let uv_ma = vec2<f32>(world_pos[ma.y], world_pos[ma.z]);
    let uv_me = vec2<f32>(world_pos[me.y], world_pos[me.z]);

    // Sample (with anti-tiling rotation blend)
    let col_ma = sample_antitile(tex, samp, uv_ma * scale, uv_ma);
    let col_me = sample_antitile(tex, samp, uv_me * scale, uv_me);

    // Quilez weight blend — local support + sharpness shaping
    var w = vec2<f32>(n[ma.x], n[me.x]);
    w = clamp((w - vec2<f32>(0.5773)) / vec2<f32>(1.0 - 0.5773), vec2<f32>(0.0), vec2<f32>(1.0));
    w = pow(w, vec2<f32>(sharpness / 8.0));
    return (col_ma * w.x + col_me * w.y) / (w.x + w.y + 0.0001);
}

fn sample_biplanar_normal(
    tex: texture_2d<f32>,
    samp: sampler,
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    scale: f32,
    sharpness: f32,
) -> vec3<f32> {
    let n = abs(world_normal);

    var ma: vec3<i32>;
    if n.x > n.y && n.x > n.z      { ma = vec3<i32>(0, 1, 2); }
    else if n.y > n.z              { ma = vec3<i32>(1, 2, 0); }
    else                            { ma = vec3<i32>(2, 0, 1); }

    var mi: vec3<i32>;
    if n.x < n.y && n.x < n.z      { mi = vec3<i32>(0, 1, 2); }
    else if n.y < n.z              { mi = vec3<i32>(1, 2, 0); }
    else                            { mi = vec3<i32>(2, 0, 1); }

    let me_axis = 3 - ma.x - mi.x;
    var me: vec3<i32>;
    if me_axis == 0      { me = vec3<i32>(0, 1, 2); }
    else if me_axis == 1 { me = vec3<i32>(1, 2, 0); }
    else                  { me = vec3<i32>(2, 0, 1); }

    let uv_ma = vec2<f32>(world_pos[ma.y], world_pos[ma.z]) * scale;
    let uv_me = vec2<f32>(world_pos[me.y], world_pos[me.z]) * scale;
    let n_ma = textureSample(tex, samp, uv_ma).rgb * 2.0 - 1.0;
    let n_me = textureSample(tex, samp, uv_me).rgb * 2.0 - 1.0;

    // Build axis-perpendicular perturbations (X-plane → swap-y; Y-plane → swap-z; Z-plane → swap-y)
    var pert_ma: vec3<f32>;
    if ma.x == 0      { pert_ma = vec3<f32>(0.0, n_ma.y, -n_ma.x); }
    else if ma.x == 1 { pert_ma = vec3<f32>(n_ma.x, 0.0, n_ma.y); }
    else               { pert_ma = vec3<f32>(n_ma.x, n_ma.y, 0.0); }

    var pert_me: vec3<f32>;
    if me.x == 0      { pert_me = vec3<f32>(0.0, n_me.y, -n_me.x); }
    else if me.x == 1 { pert_me = vec3<f32>(n_me.x, 0.0, n_me.y); }
    else               { pert_me = vec3<f32>(n_me.x, n_me.y, 0.0); }

    var w = vec2<f32>(n[ma.x], n[me.x]);
    w = clamp((w - vec2<f32>(0.5773)) / vec2<f32>(1.0 - 0.5773), vec2<f32>(0.0), vec2<f32>(1.0));
    w = pow(w, vec2<f32>(sharpness / 8.0));
    let total = w.x + w.y + 0.0001;
    return normalize(world_normal + (pert_ma * w.x + pert_me * w.y) / total);
}

// ─── Biome Detection from Vertex Color ───
struct BiomeWeights {
    grass: f32,
    forest: f32,
    sand: f32,
    snow: f32,
    volcanic: f32,
}

// Sub-biome identity from vertex color — 10 biomes compressed into 5 shader buckets.
// Returns a multiplicative RGB tint to differentiate siblings (Jungle vs Forest vs Swamp,
// Canyon vs Desert vs Savanna). Ref biome colors from forgia-terrain/src/biomes.rs.
fn sub_biome_tint(vertex_color: vec4<f32>, world_xz: vec2<f32>) -> vec3<f32> {
    let r = vertex_color.r;
    let g = vertex_color.g;
    let b = vertex_color.b;
    let lum = r * 0.299 + g * 0.587 + b * 0.114;
    let green_dom = g - max(r, b);
    let warmth = r - b;

    // Low-freq noise so transitions aren't abrupt
    let wobble = value_noise_2oct(world_xz * 0.003) * 0.1 + 0.95;

    var tint = vec3<f32>(1.0);

    // Jungle: dense dark green, lum<0.3, very green-dominant
    let is_jungle = smoothstep_manual(0.06, 0.14, green_dom)
                  * (1.0 - smoothstep_manual(0.25, 0.35, lum))
                  * smoothstep_manual(0.15, 0.10, b);
    tint = mix(tint, vec3<f32>(0.85, 1.22, 0.72), is_jungle * 0.85);

    // Swamp: murky olive, balanced green+red+low blue
    let is_swamp = smoothstep_manual(0.03, 0.10, green_dom)
                 * smoothstep_manual(0.25, 0.40, lum)
                 * (1.0 - smoothstep_manual(0.40, 0.50, lum))
                 * smoothstep_manual(0.25, 0.18, b);
    tint = mix(tint, vec3<f32>(0.92, 0.98, 0.72), is_swamp * 0.80);

    // Canyon: red-orange rock, high warmth + r>0.6
    let is_canyon = smoothstep_manual(0.35, 0.55, warmth)
                  * smoothstep_manual(0.60, 0.75, r)
                  * (1.0 - smoothstep_manual(0.35, 0.50, g));
    tint = mix(tint, vec3<f32>(1.20, 0.82, 0.65), is_canyon * 0.85);

    // Savanna: dry yellow grass, warm + balanced r/g
    let is_savanna = smoothstep_manual(0.25, 0.45, warmth)
                   * smoothstep_manual(0.65, 0.80, r)
                   * smoothstep_manual(0.55, 0.70, g)
                   * (1.0 - smoothstep_manual(0.45, 0.55, b));
    tint = mix(tint, vec3<f32>(1.12, 1.06, 0.75), is_savanna * 0.75);

    // Meadow grass fresh variant (Plains with high green saturation)
    let is_meadow = smoothstep_manual(0.15, 0.25, green_dom)
                  * smoothstep_manual(0.35, 0.50, lum)
                  * (1.0 - smoothstep_manual(0.55, 0.70, lum));
    tint = mix(tint, vec3<f32>(0.95, 1.08, 0.88), is_meadow * 0.40);

    return tint * wobble;
}

fn compute_biome_weights(vertex_color: vec4<f32>, world_xz: vec2<f32>) -> BiomeWeights {
    let r = vertex_color.r;
    let g = vertex_color.g;
    let b = vertex_color.b;
    var w: BiomeWeights;
    let lum = r * 0.299 + g * 0.587 + b * 0.114;

    // Noise perturbation at biome transitions for organic boundaries
    let noise = value_noise_2oct(world_xz * 0.025) * 0.14 - 0.07;

    w.snow = smoothstep_manual(0.65 + noise, 0.80 + noise, lum) * smoothstep_manual(0.35 + noise, 0.55 + noise, b);
    w.volcanic = smoothstep_manual(0.25 + noise, 0.12 + noise, lum);
    let warmth = r - b;
    w.sand = smoothstep_manual(0.15 + noise, 0.35 + noise, warmth) * smoothstep_manual(0.35 + noise, 0.55 + noise, r)
           * (1.0 - w.snow) * (1.0 - w.volcanic);
    let green_dom = g - max(r, b);
    w.forest = smoothstep_manual(0.0 + noise, 0.12 + noise, green_dom) * smoothstep_manual(0.45 + noise, 0.25 + noise, lum)
             * (1.0 - w.snow) * (1.0 - w.volcanic);
    w.grass = max(0.0, 1.0 - w.snow - w.volcanic - w.sand - w.forest);
    let total = w.grass + w.forest + w.sand + w.snow + w.volcanic + 0.0001;
    w.grass /= total;
    w.forest /= total;
    w.sand /= total;
    w.snow /= total;
    w.volcanic /= total;
    return w;
}

// ─── Fragment Shader ───

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    let world_pos = in.world_position.xyz;
    var world_normal = normalize(in.world_normal);
    if !is_front { world_normal = -world_normal; }
    let vertex_color = in.color;

    // ─── Debug Modes (unlit, bypass PBR) ───
    var dbg: FragmentOutput;
    if terrain_params.debug_mode == 1u {
        dbg.color = vec4<f32>(world_normal * 0.5 + 0.5, 1.0);
        return dbg;
    }
    if terrain_params.debug_mode == 2u {
        let tw = triplanar_weights(world_normal, terrain_params.triplanar_sharpness);
        dbg.color = vec4<f32>(tw.x, tw.y, tw.z, 1.0);
        return dbg;
    }
    if terrain_params.debug_mode == 3u {
        let uv = fract(world_pos.xz * terrain_params.triplanar_scale);
        dbg.color = vec4<f32>(uv.x, uv.y, 0.5, 1.0);
        return dbg;
    }
    if terrain_params.debug_mode == 4u {
        let s = world_normal.y;
        dbg.color = vec4<f32>(1.0 - s, s, 0.0, 1.0);
        return dbg;
    }
    if terrain_params.debug_mode == 5u {
        let bw = compute_biome_weights(vertex_color, world_pos.xz);
        dbg.color = vec4<f32>(bw.sand + bw.volcanic, bw.forest + bw.grass * 0.5, bw.snow, 1.0);
        return dbg;
    }

    // ─── Fast path: vertex colors only (still PBR-lit) ───
    if terrain_params.texture_influence < 0.01 {
        var pbr_vc = pbr_input_from_vertex_output(in, is_front, true);
        pbr_vc.material.base_color = vertex_color;
        pbr_vc.material.perceptual_roughness = 0.8;
        pbr_vc.material.metallic = 0.0;
        var out_vc: FragmentOutput;
        out_vc.color = pbr_functions::apply_pbr_lighting(pbr_vc);
        out_vc.color = pbr_functions::main_pass_post_lighting_processing(pbr_vc, out_vc.color);
        return out_vc;
    }

    // ─── Biome weights ───
    let biome = compute_biome_weights(vertex_color, world_pos.xz);

    // ─── Triplanar Sampling ───
    let scale = terrain_params.triplanar_scale;
    let sharp = terrain_params.triplanar_sharpness;
    let threshold = 0.05;

    let rock_col = sample_biplanar(rock_texture, rock_sampler, world_pos, world_normal, scale * 1.3, sharp);

    var biome_color = vec4<f32>(0.0);
    var biome_n = world_normal;

    if biome.grass > threshold {
        // ── Genome-driven triple blend: grass + plains + dirt ──
        // Three grass layers at different scales for variety
        let grass_col = sample_biplanar(grass_texture, grass_sampler, world_pos, world_normal, scale, sharp);
        let plains_col = sample_biplanar(plains_texture, plains_sampler, world_pos, world_normal, scale * 0.85, sharp);
        let dirt_col = sample_biplanar(dirt_texture, dirt_sampler, world_pos, world_normal, scale * 1.1, sharp);

        // 1) Blend grass variants — genome tv_plains_mix controls ratio
        //    Noise-driven: areas of primary grass vs plains variant for natural variety
        let grass_var_noise = value_noise_2oct(world_pos.xz * 0.05 + 100.0);
        let plains_factor = smoothstep_manual(0.5 - terrain_params.plains_mix * 0.4,
                                               0.5 + terrain_params.plains_mix * 0.1,
                                               grass_var_noise);
        let grass_blend = mix(grass_col, plains_col, 1.0 - plains_factor);

        // 2) Dirt patches — genome tv_dirt_ratio + tv_patch_scale
        //    Multi-frequency noise for organic, irregular patches
        let ps = terrain_params.patch_scale;
        let patch_lo = value_noise_2oct(world_pos.xz * ps);            // broad patches
        let patch_hi = value_noise_2oct(world_pos.xz * ps * 3.5);     // fine detail edges
        let patch_noise = patch_lo * 0.65 + patch_hi * 0.35;
        // dirt_ratio controls the threshold: more ratio = more dirt visible
        let dirt_thresh = 0.5 - terrain_params.dirt_ratio;
        let grass_factor = smoothstep_manual(dirt_thresh, dirt_thresh + 0.20, patch_noise);
        let col = mix(dirt_col, grass_blend, grass_factor);

        // 3) Color warmth — genome tv_color_warmth shifts toward golden/dry
        let warmth = terrain_params.color_warmth;
        let warm_tint = vec4<f32>(
            col.r * (1.0 + warmth * 0.12),
            col.g * (1.0 + warmth * 0.04),
            col.b * (1.0 - warmth * 0.15),
            col.a
        );
        biome_color += warm_tint * biome.grass;

        // Normals: blend all three layers
        let grass_n_val = sample_biplanar_normal(grass_normal_tex, grass_normal_sampler, world_pos, world_normal, scale, sharp);
        let dirt_n_val = sample_biplanar_normal(dirt_normal_tex, dirt_normal_sampler, world_pos, world_normal, scale * 1.1, sharp);
        let mixed_n = mix(dirt_n_val, grass_n_val, grass_factor);
        biome_n = mix(biome_n, mixed_n, biome.grass);
    }
    if biome.forest > threshold {
        // Forest floor + dirt + dead leaves — genome dirt_ratio scaled up for forests
        let forest_col = sample_biplanar(forest_texture, forest_sampler, world_pos, world_normal, scale * 1.1, sharp);
        let dirt_forest = sample_biplanar(dirt_texture, dirt_sampler, world_pos, world_normal, scale * 0.9, sharp);
        let f_patch = value_noise_2oct(world_pos.xz * terrain_params.patch_scale * 1.2 + 50.0);
        let f_dirt_ratio = terrain_params.dirt_ratio * 1.3; // forests have more exposed soil
        let f_thresh = 0.5 - f_dirt_ratio;
        let f_dirt = smoothstep_manual(f_thresh, f_thresh + 0.18, f_patch);
        let col = mix(dirt_forest, forest_col, f_dirt);
        biome_color += col * biome.forest;
        let n = sample_biplanar_normal(forest_normal_tex, forest_normal_sampler, world_pos, world_normal, scale * 1.1, sharp);
        biome_n = mix(biome_n, n, biome.forest);
    }
    if biome.sand > threshold {
        let col = sample_biplanar(sand_texture, sand_sampler, world_pos, world_normal, scale * 0.9, sharp);
        biome_color += col * biome.sand;
        let n = sample_biplanar_normal(sand_normal_tex, sand_normal_sampler, world_pos, world_normal, scale * 0.9, sharp);
        biome_n = mix(biome_n, n, biome.sand);
    }
    if biome.snow > threshold {
        let col = sample_biplanar(snow_texture, snow_sampler, world_pos, world_normal, scale * 0.7, sharp);
        biome_color += col * biome.snow;
    }
    if biome.volcanic > threshold {
        let col = sample_biplanar(volcanic_texture, volcanic_sampler, world_pos, world_normal, scale * 1.2, sharp);
        biome_color += col * biome.volcanic;
    }

    // ─── Sub-biome tint (Jungle/Swamp/Canyon/Savanna/Meadow) ───
    // Applied before rocks so cliffs stay neutral grey across biomes.
    let sub_tint = sub_biome_tint(vertex_color, world_pos.xz);
    biome_color = vec4<f32>(biome_color.rgb * sub_tint, biome_color.a);

    // ─── Slope-based rock overlay ───
    let slope = world_normal.y;
    let noise_val = hash2d(world_pos.xz * 0.1) * 0.15 - 0.075;
    let slope_rock = 1.0 - smoothstep_manual(
        terrain_params.slope_threshold - terrain_params.slope_blend + noise_val,
        terrain_params.slope_threshold + terrain_params.slope_blend + noise_val,
        slope
    );
    // Height-blend (Mikkelsen) instead of linear mix → rocks "win" on outcrops, no muddy transitions
    biome_color = height_blend2(biome_color, rock_col, slope_rock, 0.10);
    let rock_n = sample_biplanar_normal(rock_normal_tex, rock_normal_sampler, world_pos, world_normal, scale * 1.3, sharp);
    biome_n = mix(biome_n, rock_n, slope_rock);

    // ─── Moss overlay (procedural, genome-driven) ───
    // Grows on gentle slopes at low altitude — forest/jungle/swamp biomes primarily
    if terrain_params.moss_coverage > 0.01 {
        let altitude_raw = clamp(world_pos.y / 128.0, 0.0, 1.0);
        let moss_height_norm = terrain_params.moss_height_max / 128.0;
        let moss_slope_norm = terrain_params.moss_slope_max / 90.0;
        // Slope mask: moss on gentle slopes only (world_normal.y = 1.0 = flat)
        let slope_angle_norm = 1.0 - slope; // 0 = flat, 1 = vertical
        let moss_slope_mask = smoothstep_manual(moss_slope_norm, moss_slope_norm - 0.17, slope_angle_norm);
        // Height mask: moss below max height
        let moss_height_mask = smoothstep_manual(moss_height_norm, moss_height_norm - 0.15, altitude_raw);
        // Noise variation: organic patchy coverage
        let moss_noise = value_noise_2oct(world_pos.xz * 0.12 + 200.0);
        let moss_detail = value_noise_2oct(world_pos.xz * 0.5 + 300.0);
        let moss_pattern = moss_noise * 0.7 + moss_detail * 0.3;
        // Biome affinity: forest/jungle/swamp love moss, desert/volcanic hate it
        let moss_biome = biome.forest * 1.0 + biome.grass * 0.5 + biome.snow * 0.1;
        // Final mask
        let moss_mask = moss_slope_mask * moss_height_mask * moss_pattern * moss_biome
                       * terrain_params.moss_coverage * (1.0 - slope_rock);
        if moss_mask > 0.02 {
            // Moss tint: multiply base color toward dark green
            let moss_tint = biome_color.rgb * vec3<f32>(0.55, 0.85, 0.35);
            biome_color = vec4<f32>(mix(biome_color.rgb, moss_tint, moss_mask), biome_color.a);
            // Moss is slightly smoother than rock/dirt
            biome_color = vec4<f32>(biome_color.rgb, mix(biome_color.a, 0.7, moss_mask * 0.4));
        }
    }

    // ─── Altitude-based snow (genome-amplified) ───
    let altitude = clamp(world_pos.y / 128.0, 0.0, 1.0);
    let snow_noise = hash2d(world_pos.xz * 0.05) * 0.06;
    // snow_coverage amplifies: lower the altitude threshold when coverage > 0
    let snow_alt_adj = terrain_params.snow_altitude - terrain_params.snow_coverage * 0.3;
    let snow_factor = smoothstep_manual(
        snow_alt_adj - terrain_params.snow_blend + snow_noise,
        snow_alt_adj + terrain_params.snow_blend + snow_noise,
        altitude
    ) * (1.0 - slope_rock * 0.7);
    // Also boost snow on flat tops when coverage is high
    let snow_flat_bonus = terrain_params.snow_coverage * slope * 0.15;
    let snow_total = clamp(snow_factor + snow_flat_bonus, 0.0, 1.0);
    if snow_total > 0.05 {
        let snow_col = sample_biplanar(snow_texture, snow_sampler, world_pos, world_normal, scale * 0.7, sharp);
        // Height-blend → snow accumulates in concavities, rock peaks stay exposed
        biome_color = height_blend2(biome_color, snow_col, snow_total, 0.08);
    }

    // ─── Path overlay (biome-aware) ───
    // Alpha channel encodes path influence: 1.0=terrain, 0.0=full path
    // Reuses existing biome textures for per-biome road appearance — zero new textures
    let path_influence = 1.0 - vertex_color.a;
    if path_influence > 0.05 {
        let biome_w = compute_biome_weights(vertex_color, world_pos.xz);
        let path_scale = scale * 1.2;

        // Forest/Jungle: dirt trail with subtle forest floor
        let dirt_path = sample_biplanar(dirt_texture, dirt_sampler, world_pos, world_normal, path_scale, sharp);
        let forest_floor = sample_biplanar(forest_texture, forest_sampler, world_pos, world_normal, path_scale * 0.9, sharp);
        let forest_path = mix(dirt_path, forest_floor, 0.35);

        // Desert/Canyon/Savanna: darkened sand — packed sandy trail
        let sand_path_raw = sample_biplanar(sand_texture, sand_sampler, world_pos, world_normal, path_scale * 0.9, sharp);
        let sand_path = vec4<f32>(sand_path_raw.rgb * 0.78, sand_path_raw.a);

        // Mountain/Tundra: stone path
        let stone_path = sample_biplanar(rock_texture, rock_sampler, world_pos, world_normal, path_scale, sharp);

        // Plains/Grass: cobblestone (village) blended with dirt on edges
        let cobble = sample_biplanar(path_texture, path_sampler, world_pos, world_normal, path_scale * 0.7, sharp);
        let grass_edge = sample_biplanar(grass_texture, grass_sampler, world_pos, world_normal, path_scale, sharp);
        let plains_path = mix(mix(dirt_path, cobble, 0.75), grass_edge, 0.12);

        // Volcanic: dark basalt
        let volcanic_path_raw = sample_biplanar(volcanic_texture, volcanic_sampler, world_pos, world_normal, path_scale, sharp);
        let volcanic_path = mix(volcanic_path_raw, stone_path, 0.4);

        // Blend per biome weight
        var path_col = plains_path * biome_w.grass
                     + forest_path * biome_w.forest
                     + sand_path * biome_w.sand
                     + stone_path * biome_w.snow
                     + volcanic_path * biome_w.volcanic;

        // Micro-variation: subtle noise to break uniformity on paths
        let path_noise = value_noise_2oct(world_pos.xz * 0.4 + 42.0) * 0.12 + 0.94;
        path_col = vec4<f32>(path_col.rgb * path_noise, path_col.a);

        // Height-blend → path edges show grass tufts, packed earth stays clean in middle
        biome_color = height_blend2(biome_color, path_col, path_influence, 0.12);

        // Normal: dirt base + cobblestone on plains + rock for mountain/volcanic paths
        let dirt_path_n = sample_biplanar_normal(dirt_normal_tex, dirt_normal_sampler, world_pos, world_normal, path_scale, sharp);
        let cobble_n = sample_biplanar_normal(path_normal_tex, path_normal_sampler, world_pos, world_normal, path_scale * 0.7, sharp);
        let rock_path_n = sample_biplanar_normal(rock_normal_tex, rock_normal_sampler, world_pos, world_normal, path_scale, sharp);
        let grass_path_n = mix(dirt_path_n, cobble_n, 0.75);
        let path_n = grass_path_n * biome_w.grass
                   + dirt_path_n * biome_w.forest
                   + dirt_path_n * biome_w.sand
                   + rock_path_n * (biome_w.snow + biome_w.volcanic);
        biome_n = mix(biome_n, path_n, path_influence);
    }

    // ─── Houdini-style multi-scale micro-detail ───
    // 3 noise octaves at different frequencies for organic, non-repetitive look
    let micro_coarse = value_noise_2oct(world_pos.xz * 0.15) * 0.12 + 0.94; // ~7m patches
    let micro_fine = value_noise_2oct(world_pos.xz * 0.6 + 33.0) * 0.08 + 0.96; // ~1.5m detail
    let micro_ultra = hash2d(world_pos.xz * 2.5) * 0.04 + 0.98; // per-pixel grain
    let micro = micro_coarse * micro_fine * micro_ultra;
    biome_color = vec4<f32>(biome_color.rgb * micro, biome_color.a);

    // ─── Houdini: moisture darkening in concavities ───
    // Low areas (crevices, depressions) accumulate moisture → darker
    let cavity_noise = value_noise_2oct(world_pos.xz * 0.4 + 77.0);
    let moisture = (1.0 - world_normal.y) * cavity_noise * 0.15;
    biome_color = vec4<f32>(biome_color.rgb * (1.0 - moisture), biome_color.a);

    // ─── Wetness overlay (genome-driven) ───
    // Rain/proximity to water: darkens albedo + reduces roughness for specular sheen
    if terrain_params.wetness > 0.01 {
        let wet = terrain_params.wetness;
        // Wetness varies spatially — puddles in concavities, dry on ridges
        let wet_noise = value_noise_2oct(world_pos.xz * 0.3 + 150.0);
        let wet_mask = wet * smoothstep_manual(0.3, 0.6, wet_noise + wet * 0.3);
        // Darken albedo (wet surfaces absorb more light)
        biome_color = vec4<f32>(biome_color.rgb * (1.0 - wet_mask * 0.2), biome_color.a);
        // Reduce roughness (wet = more specular/shiny)
        biome_color = vec4<f32>(biome_color.rgb, biome_color.a * mix(1.0, 0.3, wet_mask));
    }

    // ─── Merge with vertex colors ───
    let influence = terrain_params.texture_influence;
    let final_albedo = mix(vertex_color, biome_color, influence);

    // ─── Normal perturbation ───
    let shading_n = normalize(mix(world_normal, biome_n, 0.65));

    // ─── Per-pixel roughness from texture alpha ───
    // biome_color.a carries the weighted roughness through all blend stages
    // Houdini-style: noise-driven roughness variation for material depth
    let roughness_noise = value_noise_2oct(world_pos.xz * 0.8 + 55.0) * 0.10 - 0.05;
    // Min 0.45 prevents mirror-like terrain reflections (white flash on camera turn)
    var roughness = clamp(biome_color.a + roughness_noise, 0.45, 0.99);
    // Paths are smoother (packed earth/stone)
    if path_influence > 0.3 {
        roughness = mix(roughness, 0.65, path_influence * 0.5);
    }

    // ─── Houdini-style Terrain AO (cavity + height + slope + micro) ───
    let cavity = smoothstep_manual(0.0, 0.5, slope) * 0.30 + 0.70;
    let height_ao = smoothstep_manual(0.0, 0.15, altitude) * 0.25 + 0.75;
    // Micro-AO from noise (simulates small-scale surface concavities)
    let micro_ao = value_noise_2oct(world_pos.xz * 1.2 + 88.0) * 0.08 + 0.92;
    let ao = cavity * height_ao * micro_ao;

    // ─── PBR Lighting (Bevy pipeline — real shadows, lights, fog, tonemapping) ───
    var pbr_input = pbr_input_from_vertex_output(in, is_front, true);
    pbr_input.material.base_color = vec4<f32>(final_albedo.rgb, 1.0);
    pbr_input.material.perceptual_roughness = roughness;
    pbr_input.material.metallic = 0.0;
    pbr_input.material.reflectance = vec3<f32>(0.02); // Low: terrain is not reflective (was 0.5)
    pbr_input.N = shading_n;
    pbr_input.world_normal = world_normal;
    pbr_input.diffuse_occlusion = vec3<f32>(ao);
    pbr_input.specular_occlusion = ao;

    var out: FragmentOutput;
    out.color = pbr_functions::apply_pbr_lighting(pbr_input);
    out.color = pbr_functions::main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
