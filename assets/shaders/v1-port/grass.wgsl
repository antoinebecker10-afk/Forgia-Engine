#import bevy_pbr::mesh_functions::{get_world_from_local, mesh_position_local_to_clip}

struct GrassParams {
    time: f32,
    wind_strength: f32,
    wind_dir_x: f32,
    wind_dir_z: f32,
    player_x: f32,
    player_y: f32,
    player_z: f32,
    crush_radius: f32,
    crush_blend: f32,
    // story-408 vague 3A+B — sun_dir suit DayNightCycle (cycle jour/nuit
    // coherent), wind_freq/amp consomment genes grass_default vague 1A.
    sun_dir_x: f32,
    sun_dir_y: f32,
    sun_dir_z: f32,
    wind_freq: f32,
    wind_amp: f32,
    // story-410 vague 3C — lighting + crush coefs data-driven.
    ambient: f32,
    diffuse: f32,
    ao_floor: f32,
    self_shadow_floor: f32,
    crush_flatten: f32,
    crush_push: f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> params: GrassParams;

// story-364 — blade alpha texture (silhouette + tip taper). Sample dans
// le fragment pour discard ce qui est hors silhouette → vraie forme de
// brin au lieu d'un quad rectangle. Bevy 0.18 dérive bindings 1+2 depuis
// `#[texture(1)]` + `#[sampler(2)]` du Rust struct.
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var blade_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var blade_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(5) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) height_factor: f32,
    @location(4) uv: vec2<f32>,
};

// ─── Noise (Houdini-style spatial variation) ───

fn hash_grass(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash_grass(i);
    let b = hash_grass(i + vec2<f32>(1.0, 0.0));
    let c = hash_grass(i + vec2<f32>(0.0, 1.0));
    let d = hash_grass(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// ─── Vertex Shader ───

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = get_world_from_local(vertex.instance_index);

    var pos = vertex.position;
    let height_factor = vertex.uv.y;

    // World position for spatial effects
    let world_pos = (world_from_local * vec4<f32>(pos, 1.0)).xyz;

    // ── Wind displacement (Houdini multi-frequency: trunk + branch + leaf) ──
    let phase = world_pos.x * 0.31 + world_pos.z * 0.73;
    let t = params.time;

    // Gust noise: low-frequency spatial wave that modulates wind intensity
    let gust = value_noise(world_pos.xz * 0.015 + vec2<f32>(t * 0.3, t * 0.2));
    let gust_mult = 0.6 + gust * 0.8; // 0.6 to 1.4 — creates visible wind waves

    // Trunk sway (slow, large motion). story-408 V3B : freq vient de
    // params.wind_freq (gene grass_wind_sway_freq).
    let f_primary = max(params.wind_freq, 0.1); // safe min if uniform not yet set
    let trunk_sway = sin(t * f_primary * 6.2832 + phase) * 0.6;
    // Branch sway (medium frequency, perpendicular component). Garde ratio
    // 2.27× du primary pour preserver le feel multi-octave Houdini.
    let branch_sway = sin(t * (f_primary * 2.27) * 6.2832 + phase * 1.7) * 0.25;
    // Leaf flutter (fast, chaotic). Ratio 5.13× du primary.
    let leaf_flutter = sin(t * (f_primary * 5.13) * 6.2832 + phase * 3.1) * 0.1;
    // Combined: height-dependent — base barely moves, tip has full motion.
    // V3B : amp global vient de params.wind_amp (gene grass_wind_sway_amp).
    let height_curve = height_factor * height_factor; // quadratic for natural feel
    let amp = max(params.wind_amp, 0.0);
    let total_sway = (trunk_sway + branch_sway + leaf_flutter) * gust_mult
                   * params.wind_strength * amp;

    pos.x += total_sway * height_curve * params.wind_dir_x;
    pos.z += total_sway * height_curve * params.wind_dir_z;
    // Subtle cross-wind motion (perpendicular to wind direction)
    let cross_sway = (branch_sway * 0.4 + leaf_flutter * 0.6) * gust_mult
                   * params.wind_strength * (amp * 0.42) * height_curve;
    pos.x += cross_sway * (-params.wind_dir_z);
    pos.z += cross_sway * params.wind_dir_x;

    // ── Player crush ──
    let player_pos = vec3<f32>(params.player_x, params.player_y, params.player_z);
    let to_player_xz = vec2<f32>(world_pos.x - player_pos.x, world_pos.z - player_pos.z);
    let dist_to_player = length(to_player_xz);

    if dist_to_player < params.crush_radius && params.crush_radius > 0.0 {
        let crush = (1.0 - smoothstep(0.0, params.crush_radius, dist_to_player)) * params.crush_blend;
        // V3C : flatten + push coefs depuis params (genes grass_crush_flatten/push).
        pos.y -= crush * params.crush_flatten * height_factor;
        if dist_to_player > 0.01 {
            let push_dir = normalize(to_player_xz);
            pos.x += push_dir.x * crush * params.crush_push * height_factor;
            pos.z += push_dir.y * crush * params.crush_push * height_factor;
        }
    }

    out.clip_position = mesh_position_local_to_clip(world_from_local, vec4<f32>(pos, 1.0));
    out.world_position = (world_from_local * vec4<f32>(pos, 1.0)).xyz;
    out.color = vertex.color;
    out.normal = normalize((world_from_local * vec4<f32>(vertex.normal, 0.0)).xyz);
    out.height_factor = height_factor;
    out.uv = vertex.uv;

    return out;
}

// ─── Fragment Shader (Houdini-style PBR grass) ───

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // story-364 — alpha mask discard pour vraie silhouette de brin.
    // AlphaMode::Mask(0.4) côté Rust → tout pixel sous 0.4 disparaît.
    let blade_sample = textureSample(blade_texture, blade_sampler, in.uv);
    if blade_sample.a < 0.4 {
        discard;
    }

    // story-408 V3A — sun_dir suit le cycle jour/nuit (TerrainSunLight Transform
    // cote Rust). Fallback `(0, 1, 0)` straight-up si update_uniforms n'a pas
    // encore tourne (1ere frame). Avant V3A, c'etait hardcode `(0.4, 0.8, 0.3)`.
    var sun_dir = vec3<f32>(params.sun_dir_x, params.sun_dir_y, params.sun_dir_z);
    let sun_len = length(sun_dir);
    if sun_len < 0.01 {
        sun_dir = vec3<f32>(0.0, 1.0, 0.0);
    } else {
        sun_dir = sun_dir / sun_len;
    }
    let h = in.height_factor;

    // ── Base/Tip color gradient (Houdini: ramp attribute) ──
    // Base: darker, warmer (soil proximity). Tip: lighter, yellowish (sun exposure)
    let base_color = in.color.rgb * 0.55; // dark base — ground shadow
    let tip_color = in.color.rgb * 1.15 + vec3<f32>(0.04, 0.06, -0.02); // bright tip + warm shift
    let blade_color = mix(base_color, tip_color, h * h); // quadratic = natural gradient

    // ── Per-blade spatial hue variation (Houdini: attribute randomize) ──
    let hue_noise = value_noise(in.world_position.xz * 0.3 + 42.0);
    let hue_shift = vec3<f32>(
        (hue_noise - 0.5) * 0.06,          // slight red/green shift
        (hue_noise - 0.5) * 0.03,          // subtle green variation
        (hash_grass(in.world_position.xz * 0.7) - 0.5) * 0.05  // blue channel
    );
    let varied_color = clamp(blade_color + hue_shift, vec3<f32>(0.0), vec3<f32>(1.0));

    // ── Lighting ──
    let n_dot_l = max(dot(in.normal, sun_dir), 0.0);

    // Subsurface scattering / translucency (Houdini: SSS shader)
    // Light passing through thin grass blades — warm orange glow when backlit
    let view_dot_sun = max(dot(-in.normal, sun_dir), 0.0);
    let sss_power = pow(view_dot_sun, 3.0); // sharp falloff
    let sss = sss_power * 0.35 * h; // stronger at tip (thinner)
    let sss_color = vec3<f32>(0.8, 0.9, 0.3) * sss; // warm yellow-green glow

    // Ground ambient occlusion (Houdini: point cloud AO)
    // V3C : floor depuis params.ao_floor (gene grass_ao_floor).
    let ao = mix(params.ao_floor, 1.0, smoothstep(0.0, 0.5, h));

    // Self-shadow: blades cast soft shadow on each other.
    // V3C : floor depuis params.self_shadow_floor (gene grass_self_shadow_floor).
    let self_shadow_noise = value_noise(in.world_position.xz * 1.5);
    let self_shadow = mix(params.self_shadow_floor, 1.0, self_shadow_noise);

    // Specular highlight (wet grass / dew effect)
    let view_dir = normalize(vec3<f32>(0.0, 1.0, 0.0)); // approximate — camera is above
    let half_vec = normalize(sun_dir + view_dir);
    let spec = pow(max(dot(in.normal, half_vec), 0.0), 32.0) * 0.15 * h;

    // Final lighting combination.
    // V3C : ambient + diffuse coef depuis params (genes grass_ambient + grass_diffuse).
    let ambient = params.ambient;
    let diffuse = n_dot_l * params.diffuse;
    let lighting = (ambient + diffuse) * ao * self_shadow;

    // story-364 — multiply by texture RGB (white-by-default, room for color
    // variation atlas plus tard). Alpha déjà consommé par le discard.
    var final_color = (varied_color * blade_sample.rgb) * lighting + sss_color + vec3<f32>(spec);

    // ── Distance fade (Houdini: viewport fog) ──
    // Subtle atmospheric tint at distance — prevents hard culling pop-in
    let dist = length(in.world_position - vec3<f32>(params.player_x, params.player_y, params.player_z));
    let fog_factor = smoothstep(40.0, 70.0, dist) * 0.3;
    let fog_color = vec3<f32>(0.45, 0.55, 0.40); // green-tinted fog matching grass
    final_color = mix(final_color, fog_color, fog_factor);

    return vec4<f32>(final_color, 1.0);
}
