// Toon post-process — Story 350 Cartoon Island MVP V1.
//
// Fullscreen pass applied between EndMainPass and Tonemapping (linear HDR).
// Effects (en cascade) :
//   1. Quantize luminance vers `cel_bands` (cel-shading look)
//   2. Rim fresnel additif via gradient de profondeur (silhouette glow)
//   3. Saturation boost (palette WoW saturee)
//
// Volontairement leger : pas de prepass normal requis, derive des normales
// approximees via depth gradient en screen-space (4-tap). Coût ~5 textures
// fetch + math, < 0.3 ms 1080p sur GPU mid-range.

// Toon post-process V1 minimal — sans dépendance depth prepass.
// Rim light (qui requiert depth) reporté en V1.5 — d'abord on garantit que
// le pass tourne en validant cel-bands + saturation à l'écran.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct ToonPostSettings {
    cel_bands: f32,
    rim_intensity: f32,    // unused V1, gardé pour stabilité layout uniform
    rim_power: f32,        // idem
    saturation_boost: f32,
}

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
@group(0) @binding(2) var<uniform> settings: ToonPostSettings;

// Quantize la luminance vers `bands` paliers, EN PRÉSERVANT la magnitude HDR
// d'origine. Pass tourne entre EndMainPass et Tonemapping (linear HDR).
//
// Historique :
//   - V1 (2026-04-25 15:30) : avec `inverse Reinhard stepped / (1 - stepped)`
//     → blow-out sunset (everything → 5.0 HDR luma → tonemapper saturate blanc).
//   - V1.1 (2026-04-26 00:50) : retire inverse Reinhard. On scale en ratio
//     direct `stepped_norm / luma_norm` qui préserve l'intensité originale
//     mais quantizée en `bands` paliers de saturation perceptuelle (Reinhard).
//
// `+ 0.4` shift garantit floor minimum > 0 (pas de pure black sur faces
// ombrées) sans gonfler les paliers hauts.
fn quantize_luma(rgb: vec3<f32>, bands: f32) -> vec3<f32> {
    let luma = max(dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722)), 0.0001);
    // Reinhard normalize HDR luma → [0,1] (perceptuel)
    let luma_norm = luma / (1.0 + luma);
    // Quantize avec floor minimum non-nul (pas de noir pur)
    let stepped_norm = clamp((floor(luma_norm * bands) + 0.4) / bands, 0.05, 0.95);
    // Scale ratio direct — préserve la magnitude HDR sans inverse Reinhard
    // (qui amplifiait dangereusement vers les hauts paliers).
    let scale = stepped_norm / max(luma_norm, 0.001);
    return rgb * scale;
}

// Boost saturation autour de la luma. boost=1.0 = identique, 1.3 = +30 %.
fn boost_saturation(rgb: vec3<f32>, boost: f32) -> vec3<f32> {
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    let gray = vec3<f32>(luma);
    return mix(gray, rgb, boost);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let src = textureSample(screen_texture, screen_sampler, in.uv);

    let bands = max(settings.cel_bands, 1.0);
    var color = quantize_luma(src.rgb, bands);
    color = boost_saturation(color, max(settings.saturation_boost, 1.0));

    return vec4<f32>(color, src.a);
}
