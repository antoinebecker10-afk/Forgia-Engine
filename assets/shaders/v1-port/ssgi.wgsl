#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct SsgiSettings {
    intensity: f32,
    radius_px: f32,
    thickness: f32,
    max_samples: f32,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var depth_texture: texture_depth_2d;
@group(0) @binding(3) var normal_texture: texture_2d<f32>;
@group(0) @binding(4) var<uniform> settings: SsgiSettings;

const PI: f32 = 3.14159265;
const SLICE_COUNT: u32 = 4u;

// Per-pixel noise to break banding
fn hash21(p: vec2<f32>) -> f32 {
    let n = dot(p, vec2(127.1, 311.7));
    return fract(sin(n) * 43758.5453);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let pixel = vec2<i32>(in.position.xy);
    let dims = vec2<f32>(textureDimensions(screen_texture));

    let center_color = textureSample(screen_texture, screen_sampler, uv);
    let center_depth = textureLoad(depth_texture, pixel, 0);

    // Skip sky (reversed Z: 0.0 = far plane / sky)
    if center_depth <= 0.0 {
        return center_color;
    }

    let center_normal = normalize(textureLoad(normal_texture, pixel, 0).xyz);
    let noise = hash21(vec2<f32>(pixel));
    let steps_per_dir = u32(settings.max_samples);

    var indirect = vec3<f32>(0.0);

    // Horizon-based SSGI: for each slice direction, march outward
    // tracking the maximum horizon angle. When a new (higher) horizon
    // is found, the occluder surface contributes its screen color.
    for (var slice = 0u; slice < SLICE_COUNT; slice++) {
        let angle = (f32(slice) + noise) * PI / f32(SLICE_COUNT);
        let dir = vec2<f32>(cos(angle), sin(angle));

        // March in +/- directions
        for (var sign_idx = 0u; sign_idx < 2u; sign_idx++) {
            let s = select(-1.0, 1.0, sign_idx == 0u);
            var max_horizon: f32 = 0.0;

            for (var step = 1u; step <= steps_per_dir; step++) {
                let t = f32(step) / f32(steps_per_dir);
                let r = t * settings.radius_px;
                let sample_px = vec2<f32>(pixel) + dir * s * r;
                let sp = vec2<i32>(sample_px);

                // Bounds check
                if any(sp < vec2<i32>(0)) || any(sp >= vec2<i32>(dims)) {
                    break;
                }

                let sample_depth = textureLoad(depth_texture, sp, 0);
                if sample_depth <= 0.0 { continue; } // skip sky

                // Reversed Z: larger depth = closer to camera = "above" center pixel
                let horizon = sample_depth - center_depth;

                // New horizon found — this sample contributes indirect light
                if horizon > max_horizon && horizon < settings.thickness {
                    let delta = horizon - max_horizon;
                    max_horizon = horizon;

                    let sample_uv = (sample_px + 0.5) / dims;
                    let sample_color = textureSample(screen_texture, screen_sampler, sample_uv).rgb;

                    // Normal-aware: favor light from surfaces facing us
                    let sample_normal = textureLoad(normal_texture, sp, 0).xyz;
                    let facing = saturate(-dot(center_normal, sample_normal) * 0.5 + 0.5);

                    // Weight: horizon delta × distance falloff × facing
                    let dist_falloff = 1.0 - t;
                    let w = delta * dist_falloff * facing;

                    indirect += sample_color * w;
                }
            }
        }
    }

    // Normalize by number of half-slices
    indirect /= f32(SLICE_COUNT * 2u);

    return vec4<f32>(center_color.rgb + indirect * settings.intensity, center_color.a);
}
