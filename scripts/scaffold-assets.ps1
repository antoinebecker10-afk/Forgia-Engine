# Forgia Rewrite — Scaffold assets folder (Renzora structure + Forgia extensions)
# Usage : pwsh -File scripts/scaffold-assets.ps1
# Idempotent.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$assetsDir = Join-Path $root "assets"

function New-Dir($path) {
    if (-not (Test-Path $path)) {
        New-Item -ItemType Directory -Path $path -Force | Out-Null
        Write-Host "[dir ] $path" -ForegroundColor Green
    } else {
        Write-Host "[skip] $path" -ForegroundColor DarkGray
    }
}

function New-Readme($path, $content) {
    if (-not (Test-Path $path)) {
        Set-Content -Path $path -Value $content -Encoding UTF8
        Write-Host "[file] $path" -ForegroundColor Cyan
    }
}

function New-Stub($path, $content) {
    if (-not (Test-Path $path)) {
        Set-Content -Path $path -Value $content -Encoding UTF8
    }
}

# ─────────────────────────────────────────────────────────────────────────────
# Top-level Renzora structure (7 dirs)
# ─────────────────────────────────────────────────────────────────────────────
$renzoraDirs = @(
    @{p="fonts";     r="Typography assets (.ttf / .otf). Use workspace lints : 1 font per role (UI/code/body)."},
    @{p="locale";    r="i18n files (.toml). fr.toml = source (French project). Mirror keys across all locales."},
    @{p="materials"; r=".material files (Renzora node-graph format). Loaded by forgia-render-* and forgia-editor-material."},
    @{p="particles"; r=".particle files (Hanabi presets). Loaded by forgia-vfx-hanabi."},
    @{p="previews";  r="UI preview images (interface, logo, splash). PNG only."},
    @{p="scripts";   r="Luau scripts (gameplay-side). Wrapped by forgia-scripting-luau."},
    @{p="shaders";   r="WGSL shaders. Subdirs : generated/, layers/, post_process/."}
)

# Forgia-specific additions (data-driven engine + game assets)
$forgiaDirs = @(
    @{p="models";    r="GLB/GLTF 3D models. Sub : characters/, weapons/, props/, environment/, vehicles/."},
    @{p="textures";  r="PNG/KTX2 textures. Sub : pbr/, ui/, decals/, sprites/."},
    @{p="audio";     r="Audio assets. Sub : sfx/, music/, voice/, ambient/."},
    @{p="genomes";   r="TOML genome configs (Forgia data-driven). Loaded by forgia-genome-core. Hot-reload Shift+F12."},
    @{p="maps";      r="Scene/level files (.scn.ron). Loaded by forgia-scene."},
    @{p="hdri";      r="HDR environment maps for forgia-render-env-map."},
    @{p="icons";     r="App icons (taskbar, launcher, Steam)."}
)

Write-Host "Forgia Rewrite — Scaffold assets" -ForegroundColor Cyan
New-Dir $assetsDir

# Build root README
$rootReadme = @"
# Forgia Rewrite — Assets

Structure inspirée de Renzora (7 dossiers media-typés) + extensions Forgia (game + data-driven).

## Renzora parity
- ``fonts/`` — Typography
- ``locale/`` — i18n TOML
- ``materials/`` — .material node-graph
- ``particles/`` — .particle Hanabi presets
- ``previews/`` — UI previews
- ``scripts/`` — Luau gameplay scripts
- ``shaders/`` — WGSL (generated, layers, post_process)

## Forgia-specific
- ``models/`` — GLB 3D models
- ``textures/`` — PNG/KTX2
- ``audio/`` — sfx/music/voice/ambient
- ``genomes/`` — TOML data-driven configs (Forgia unique)
- ``maps/`` — Scene files
- ``hdri/`` — HDR env maps
- ``icons/`` — App icons

## Loading conventions

Chaque crate qui charge un asset DOIT déclarer son path dans son ``manifest.toml`` (champ ``assets``). Lock L1 hérité de V1 : whitelist ``asset_load_whitelist.txt`` enforcée par ``xtask check-orphans``.

## Hot-reload

- Genomes : Shift+F12 (debug_toggles)
- Textures/models : ``bevy::file_watcher`` activé en debug
"@
New-Readme (Join-Path $assetsDir "README.md") $rootReadme

# ─────────────────────────────────────────────────────────────────────────────
# Create Renzora dirs + README
# ─────────────────────────────────────────────────────────────────────────────
foreach ($e in $renzoraDirs) {
    $d = Join-Path $assetsDir $e.p
    New-Dir $d
    New-Readme (Join-Path $d "README.md") "# $($e.p)`n`n$($e.r)`n"
}

foreach ($e in $forgiaDirs) {
    $d = Join-Path $assetsDir $e.p
    New-Dir $d
    New-Readme (Join-Path $d "README.md") "# $($e.p)`n`n$($e.r)`n"
}

# ─────────────────────────────────────────────────────────────────────────────
# fonts/ — placeholder list (no binary)
# ─────────────────────────────────────────────────────────────────────────────
New-Readme (Join-Path $assetsDir "fonts/_REQUIRED.md") @"
# Required fonts (copy from V1 or download)

- ``FiraCode-Regular.ttf`` — Code editor (forgia-editor-code, monospace)
- ``OpenSans-Regular.ttf`` — UI body (forgia-ui-*)
- ``Roboto-Regular.ttf`` — Game HUD (forgia-ui-hud)
- ``NotoSans-Regular.ttf`` — Fallback (Latin + extended)
- ``NotoSansJP-Regular.ttf`` — Japanese fallback (optional V2)

Source V1 : ``D:\Forgia\RUST\Forgia\Forgia\assets\`` (check existing fonts).
Source CC0 : Google Fonts.
"@

# ─────────────────────────────────────────────────────────────────────────────
# locale/ — fr.toml + en.toml stubs
# ─────────────────────────────────────────────────────────────────────────────
$localeStub = @"
# Forgia Rewrite — Locale
# Source : fr.toml (French project) — mirror keys to other locales.
# Format : flat dotted keys. Hot-reload via Shift+F11.

[menu]
play = "Jouer"
build = "Créer"
edit = "Éditer"
quit = "Quitter"

[hud]
hp = "PV"
ammo = "Munitions"
score = "Score"

[combat]
hit = "Touché"
critical = "Critique"
"@
New-Stub (Join-Path $assetsDir "locale/fr.toml") $localeStub
New-Stub (Join-Path $assetsDir "locale/en.toml") ($localeStub -replace 'Jouer','Play' -replace 'Créer','Build' -replace 'Éditer','Edit' -replace 'Quitter','Quit' -replace 'PV','HP' -replace 'Munitions','Ammo' -replace 'Touché','Hit' -replace 'Critique','Critical')

# ─────────────────────────────────────────────────────────────────────────────
# shaders/ — Renzora 3-folder structure + 30 PP stubs matching crates
# ─────────────────────────────────────────────────────────────────────────────
$shaderSubs = @("generated", "layers", "post_process", "compute")
foreach ($s in $shaderSubs) {
    New-Dir (Join-Path $assetsDir "shaders/$s")
}

# Post-process WGSL stubs (1 per forgia-pp-* crate)
$ppShaders = @(
    "toon","outline","sketch","halftone","hex_pixelate","pixelation","kuwahara","oil_painting",
    "emboss","sobel_edge","cross_hatch","ascii","matrix","crt","scanlines",
    "bloom","tonemapping","color_grading","vibrance","sepia","grayscale","invert","posterize",
    "palette_quantize","auto_exposure",
    "chromatic_aberration","glitch","distortion","swirl","kaleidoscope","color_split",
    "radial_blur","gaussian_blur","motion_blur","tilt_shift",
    "vignette","fog","distance_fog","rain","heat_haze","frosted_glass","night_vision","thermal",
    "dream","light_streaks"
)
foreach ($s in $ppShaders) {
    $path = Join-Path $assetsDir "shaders/post_process/$s.wgsl"
    if (-not (Test-Path $path)) {
        $stub = @"
// $s.wgsl — post-process shader stub
// Consumed by crate : forgia-pp-$($s.Replace('_','-'))
// Schema : fullscreen pass (vert -> frag), bind group 0 = screen texture + sampler.

#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, texture_sampler, in.uv);
    // TODO: implement $s
    return color;
}
"@
        Set-Content -Path $path -Value $stub -Encoding UTF8
    }
}

# Terrain shader stubs in generated/ + layers/
New-Stub (Join-Path $assetsDir "shaders/generated/terrain_material.wgsl") "// terrain_material.wgsl — generated by forgia-terrain (placeholder stub).`n"
foreach ($l in @("default","default_dark","grass","rocky","water","sand")) {
    New-Stub (Join-Path $assetsDir "shaders/layers/$l.wgsl") "// $l.wgsl — terrain layer stub.`n"
}

# Other shader stubs (Renzora parity)
foreach ($f in @("grass_material","grass_vertex","material_graph","material_vertex","splatmap_blend")) {
    New-Stub (Join-Path $assetsDir "shaders/$f.wgsl") "// $f.wgsl — stub.`n"
}

# ─────────────────────────────────────────────────────────────────────────────
# materials/ — Renzora .material stubs
# ─────────────────────────────────────────────────────────────────────────────
$materials = @(
    "01_radial_gradient","02_rainbow_fbm","03_polar_checker","04_hue_rotator","06_fresnel_rim",
    "07_fbm_rock","08_srgb_showcase","09_warped_clouds","10_voronoi_cells","11_plasma_pure_nodes",
    "12_curl_flow","13_spiral_gradient","14_normal_visualizer","15_fresnel_only","16_toon_step",
    "17_brick_pbr","18_metallic_gold","19_radial_vignette","20_angular_sweep","21_diamond_tile"
)
foreach ($m in $materials) {
    $path = Join-Path $assetsDir "materials/$m.material"
    if (-not (Test-Path $path)) {
        $stub = @"
# $m.material — Renzora-format material stub.
# Consumed by forgia-editor-material + forgia-render-* crates.
# TODO: port node-graph from Renzora source.

[meta]
name = "$m"
version = "0.1.0"
"@
        Set-Content -Path $path -Value $stub -Encoding UTF8
    }
}

# ─────────────────────────────────────────────────────────────────────────────
# particles/ — Renzora .particle stubs
# ─────────────────────────────────────────────────────────────────────────────
foreach ($p in @("fire","firework","firework_rocket","rain","snow","muzzle_flash","impact_metal","impact_wood","impact_concrete","explosion","blood","smoke")) {
    $path = Join-Path $assetsDir "particles/$p.particle"
    if (-not (Test-Path $path)) {
        Set-Content -Path $path -Value "# $p.particle — Hanabi preset stub.`n# Consumed by forgia-vfx-hanabi.`n" -Encoding UTF8
    }
}

# ─────────────────────────────────────────────────────────────────────────────
# scripts/ — Luau examples (Renzora has 10 .lua → mirror with .luau)
# ─────────────────────────────────────────────────────────────────────────────
foreach ($s in @("boat_rocking","car_movement","car_physics","car_with_camera","follow_camera","fps_controller","sphere_bobbing","wasd_movement","water_wave","wheel_steering","weapon_personality","npc_dialogue")) {
    $path = Join-Path $assetsDir "scripts/$s.luau"
    if (-not (Test-Path $path)) {
        Set-Content -Path $path -Value "-- $s.luau — Luau script stub. Loaded by forgia-scripting-luau.`n-- TODO: port from Renzora .lua equivalent.`n" -Encoding UTF8
    }
}

# ─────────────────────────────────────────────────────────────────────────────
# Forgia-specific subdirs
# ─────────────────────────────────────────────────────────────────────────────
$modelsSubs = @("characters","weapons","props","environment","vehicles","ui")
foreach ($s in $modelsSubs) { New-Dir (Join-Path $assetsDir "models/$s") }

$texturesSubs = @("pbr","ui","decals","sprites","hud","lut")
foreach ($s in $texturesSubs) { New-Dir (Join-Path $assetsDir "textures/$s") }

$audioSubs = @("sfx","music","voice","ambient")
foreach ($s in $audioSubs) { New-Dir (Join-Path $assetsDir "audio/$s") }

# sfx subcategories
foreach ($s in @("weapons","footsteps","ui","impacts","explosions")) { New-Dir (Join-Path $assetsDir "audio/sfx/$s") }

# Genomes — port reference list from V1
$genomeCategories = @(
    "weapons","biomes","enemies","ai","atmosphere","audio","damage_feedback",
    "debug_monitor","economy","economy_global","industry","input","map_gen",
    "particles","perfs","sky","steam","quality","ui","vfx"
)
foreach ($g in $genomeCategories) { New-Dir (Join-Path $assetsDir "genomes/$g") }
New-Readme (Join-Path $assetsDir "genomes/_REFERENCE.md") @"
# Genomes reference

Port from V1 ``D:\Forgia\RUST\Forgia\Forgia\config\genomes\`` (~100 TOML files).

Schema : flat key=value, hot-reloadable Shift+F12, validated by ``forgia-genome-validator``.

Each genome category maps to one or more crates :
- ``weapons/`` → forgia-weapon-* + forgia-combat
- ``biomes/`` → forgia-terrain + forgia-foliage + forgia-audio-biome
- ``enemies/`` → forgia-ai-* + forgia-mode-fps-arena
- ``economy/`` → forgia-xp-curves + forgia-loot-tables + forgia-genome-economy
- ``debug_monitor/`` → forgia-observability + forgia-qa-*
"@

# Maps
foreach ($m in @("arena","openworld","tutorial","menu")) { New-Dir (Join-Path $assetsDir "maps/$m") }

Write-Host ""
Write-Host "Done." -ForegroundColor Cyan
