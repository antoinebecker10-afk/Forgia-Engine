//! forgia-postprocess — 45 post-process effects (Bevy FullscreenMaterial).
//!
//! Story-513 (2026-05-23) : fusion de 45 anciennes crates `forgia-pp-*` en
//! une seule pour reduire la fragmentation workspace (Bevy core philosophy :
//! ~80 crates / 500k LOC, pas 250 crates / 50k LOC).
//!
//! Chaque effet est opt-in via son `*Plugin` individuel. Le mega-plugin
//! `ForgiaPostProcessPlugin` est intentionnellement vide — il sert de hook
//! pour wire-up futur sans casser la convention "rien n'est ajoute sans
//! demande explicite".
//!
//! # Usage
//! ```ignore
//! use forgia_postprocess::bloom::{BloomSettings, ForgiaPpBloomPlugin};
//! app.add_plugins(ForgiaPpBloomPlugin);
//! commands.spawn((Camera3d::default(), BloomSettings::default()));
//! ```

/// Macro to declare a simple post-process effect (single `strength: f32` setting).
///
/// Expands to a `pub mod $mod_name` containing :
/// - `pub struct $settings { strength: f32, _pad: Vec3 }`
/// - `impl Default for $settings { strength = 1.0 }`
/// - `impl FullscreenMaterial for $settings` with `$shader_path`
/// - `pub struct $plugin; impl Plugin for $plugin` adding `FullscreenMaterialPlugin::<$settings>`
///
/// Used by 43 effects sharing identical boilerplate. Outliers with richer
/// settings (outline, toon) are hand-written in their own files.
#[macro_export]
macro_rules! define_simple_pp_effect {
    ($mod_name:ident, $settings:ident, $plugin:ident, $shader_path:literal) => {
        pub mod $mod_name {
            use ::bevy::core_pipeline::core_3d::graph::Node3d;
            use ::bevy::core_pipeline::fullscreen_material::{
                FullscreenMaterial, FullscreenMaterialPlugin,
            };
            use ::bevy::prelude::*;
            use ::bevy::render::extract_component::ExtractComponent;
            use ::bevy::render::render_graph::{InternedRenderLabel, RenderLabel};
            use ::bevy::render::render_resource::ShaderType;
            use ::bevy::shader::ShaderRef;

            /// Post-process settings. Attach to a `Camera3d`.
            #[derive(Component, ExtractComponent, Clone, Copy, ShaderType, Debug)]
            pub struct $settings {
                pub strength: f32,
                pub _pad: Vec3,
            }

            impl Default for $settings {
                fn default() -> Self {
                    Self {
                        strength: 1.0,
                        _pad: Vec3::ZERO,
                    }
                }
            }

            impl FullscreenMaterial for $settings {
                fn fragment_shader() -> ShaderRef {
                    $shader_path.into()
                }

                fn node_edges() -> Vec<InternedRenderLabel> {
                    vec![
                        Node3d::Tonemapping.intern(),
                        Self::node_label().intern(),
                        Node3d::EndMainPassPostProcessing.intern(),
                    ]
                }
            }

            pub struct $plugin;

            impl Plugin for $plugin {
                fn build(&self, app: &mut App) {
                    app.add_plugins(FullscreenMaterialPlugin::<$settings>::default());
                }
            }
        }
    };
}

// 43 simple effects (identical strength: f32 template)
define_simple_pp_effect!(
    ascii,
    AsciiSettings,
    ForgiaPpAsciiPlugin,
    "shaders/post_process/ascii.wgsl"
);
define_simple_pp_effect!(
    auto_exposure,
    AutoExposureSettings,
    ForgiaPpAutoExposurePlugin,
    "shaders/post_process/auto_exposure.wgsl"
);
define_simple_pp_effect!(
    bloom,
    BloomSettings,
    ForgiaPpBloomPlugin,
    "shaders/post_process/bloom.wgsl"
);
define_simple_pp_effect!(
    chromatic_aberration,
    ChromaticAberrationSettings,
    ForgiaPpChromaticAberrationPlugin,
    "shaders/post_process/chromatic_aberration.wgsl"
);
define_simple_pp_effect!(
    color_grading,
    ColorGradingSettings,
    ForgiaPpColorGradingPlugin,
    "shaders/post_process/color_grading.wgsl"
);
define_simple_pp_effect!(
    color_split,
    ColorSplitSettings,
    ForgiaPpColorSplitPlugin,
    "shaders/post_process/color_split.wgsl"
);
define_simple_pp_effect!(
    cross_hatch,
    CrossHatchSettings,
    ForgiaPpCrossHatchPlugin,
    "shaders/post_process/cross_hatch.wgsl"
);
define_simple_pp_effect!(
    crt,
    CrtSettings,
    ForgiaPpCrtPlugin,
    "shaders/post_process/crt.wgsl"
);
define_simple_pp_effect!(
    distance_fog,
    DistanceFogSettings,
    ForgiaPpDistanceFogPlugin,
    "shaders/post_process/distance_fog.wgsl"
);
define_simple_pp_effect!(
    distortion,
    DistortionSettings,
    ForgiaPpDistortionPlugin,
    "shaders/post_process/distortion.wgsl"
);
define_simple_pp_effect!(
    dream,
    DreamSettings,
    ForgiaPpDreamPlugin,
    "shaders/post_process/dream.wgsl"
);
define_simple_pp_effect!(
    emboss,
    EmbossSettings,
    ForgiaPpEmbossPlugin,
    "shaders/post_process/emboss.wgsl"
);
define_simple_pp_effect!(
    fog,
    FogSettings,
    ForgiaPpFogPlugin,
    "shaders/post_process/fog.wgsl"
);
define_simple_pp_effect!(
    frosted_glass,
    FrostedGlassSettings,
    ForgiaPpFrostedGlassPlugin,
    "shaders/post_process/frosted_glass.wgsl"
);
define_simple_pp_effect!(
    gaussian_blur,
    GaussianBlurSettings,
    ForgiaPpGaussianBlurPlugin,
    "shaders/post_process/gaussian_blur.wgsl"
);
define_simple_pp_effect!(
    glitch,
    GlitchSettings,
    ForgiaPpGlitchPlugin,
    "shaders/post_process/glitch.wgsl"
);
define_simple_pp_effect!(
    grayscale,
    GrayscaleSettings,
    ForgiaPpGrayscalePlugin,
    "shaders/post_process/grayscale.wgsl"
);
define_simple_pp_effect!(
    halftone,
    HalftoneSettings,
    ForgiaPpHalftonePlugin,
    "shaders/post_process/halftone.wgsl"
);
define_simple_pp_effect!(
    heat_haze,
    HeatHazeSettings,
    ForgiaPpHeatHazePlugin,
    "shaders/post_process/heat_haze.wgsl"
);
define_simple_pp_effect!(
    hex_pixelate,
    HexPixelateSettings,
    ForgiaPpHexPixelatePlugin,
    "shaders/post_process/hex_pixelate.wgsl"
);
define_simple_pp_effect!(
    invert,
    InvertSettings,
    ForgiaPpInvertPlugin,
    "shaders/post_process/invert.wgsl"
);
define_simple_pp_effect!(
    kaleidoscope,
    KaleidoscopeSettings,
    ForgiaPpKaleidoscopePlugin,
    "shaders/post_process/kaleidoscope.wgsl"
);
define_simple_pp_effect!(
    kuwahara,
    KuwaharaSettings,
    ForgiaPpKuwaharaPlugin,
    "shaders/post_process/kuwahara.wgsl"
);
define_simple_pp_effect!(
    light_streaks,
    LightStreaksSettings,
    ForgiaPpLightStreaksPlugin,
    "shaders/post_process/light_streaks.wgsl"
);
define_simple_pp_effect!(
    matrix,
    MatrixSettings,
    ForgiaPpMatrixPlugin,
    "shaders/post_process/matrix.wgsl"
);
define_simple_pp_effect!(
    motion_blur,
    MotionBlurSettings,
    ForgiaPpMotionBlurPlugin,
    "shaders/post_process/motion_blur.wgsl"
);
define_simple_pp_effect!(
    night_vision,
    NightVisionSettings,
    ForgiaPpNightVisionPlugin,
    "shaders/post_process/night_vision.wgsl"
);
define_simple_pp_effect!(
    oil_painting,
    OilPaintingSettings,
    ForgiaPpOilPaintingPlugin,
    "shaders/post_process/oil_painting.wgsl"
);
define_simple_pp_effect!(
    palette_quantize,
    PaletteQuantizeSettings,
    ForgiaPpPaletteQuantizePlugin,
    "shaders/post_process/palette_quantize.wgsl"
);
define_simple_pp_effect!(
    pixelation,
    PixelationSettings,
    ForgiaPpPixelationPlugin,
    "shaders/post_process/pixelation.wgsl"
);
define_simple_pp_effect!(
    posterize,
    PosterizeSettings,
    ForgiaPpPosterizePlugin,
    "shaders/post_process/posterize.wgsl"
);
define_simple_pp_effect!(
    radial_blur,
    RadialBlurSettings,
    ForgiaPpRadialBlurPlugin,
    "shaders/post_process/radial_blur.wgsl"
);
define_simple_pp_effect!(
    rain,
    RainSettings,
    ForgiaPpRainPlugin,
    "shaders/post_process/rain.wgsl"
);
define_simple_pp_effect!(
    scanlines,
    ScanlinesSettings,
    ForgiaPpScanlinesPlugin,
    "shaders/post_process/scanlines.wgsl"
);
define_simple_pp_effect!(
    sepia,
    SepiaSettings,
    ForgiaPpSepiaPlugin,
    "shaders/post_process/sepia.wgsl"
);
define_simple_pp_effect!(
    sketch,
    SketchSettings,
    ForgiaPpSketchPlugin,
    "shaders/post_process/sketch.wgsl"
);
define_simple_pp_effect!(
    sobel_edge,
    SobelEdgeSettings,
    ForgiaPpSobelEdgePlugin,
    "shaders/post_process/sobel_edge.wgsl"
);
define_simple_pp_effect!(
    swirl,
    SwirlSettings,
    ForgiaPpSwirlPlugin,
    "shaders/post_process/swirl.wgsl"
);
define_simple_pp_effect!(
    thermal,
    ThermalSettings,
    ForgiaPpThermalPlugin,
    "shaders/post_process/thermal.wgsl"
);
define_simple_pp_effect!(
    tilt_shift,
    TiltShiftSettings,
    ForgiaPpTiltShiftPlugin,
    "shaders/post_process/tilt_shift.wgsl"
);
define_simple_pp_effect!(
    tonemapping,
    TonemappingSettings,
    ForgiaPpTonemappingPlugin,
    "shaders/post_process/tonemapping.wgsl"
);
define_simple_pp_effect!(
    vibrance,
    VibranceSettings,
    ForgiaPpVibrancePlugin,
    "shaders/post_process/vibrance.wgsl"
);
define_simple_pp_effect!(
    vignette,
    VignetteSettings,
    ForgiaPpVignettePlugin,
    "shaders/post_process/vignette.wgsl"
);

// 2 outliers with richer settings — hand-written
pub mod outline;
pub mod toon;

/// Mega-plugin. Intentionally empty — each effect is opt-in via its own
/// `*Plugin`. Use this as a future hook for default presets.
pub struct ForgiaPostProcessPlugin;

impl bevy::app::Plugin for ForgiaPostProcessPlugin {
    fn build(&self, _app: &mut bevy::app::App) {
        // No-op by design. Cf module docs.
    }
}
