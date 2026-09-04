# Forgia Rewrite — Scaffold crates (Renzora parity + Forgia unique features)
# Usage : pwsh -File scripts/scaffold-crates.ps1
# Idempotent : skip si le dossier existe deja.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$cratesDir = Join-Path $root "crates"

# ─────────────────────────────────────────────────────────────────────────────
# Catalogue : (name, category, intent, deps_extra)
# category drive le set de deps minimal
# ─────────────────────────────────────────────────────────────────────────────
$catalogue = @(
    # ══════════════ A. FOUNDATIONS (12) ══════════════ (skipping forgia-core, exists)
    @{n="forgia-app-state";    c="foundation"; i="States/SubStates wrapper + transitions safe"; d=@()},
    @{n="forgia-system-sets";  c="foundation"; i="GameSet enum + ordering chain (Input->Physics->...)"; d=@()},
    @{n="forgia-time";         c="foundation"; i="Time<Real>/Time<Virtual> helpers anti-trap UI"; d=@()},
    @{n="forgia-rng";          c="foundation"; i="Seeded deterministic RNG resource (bevy_rand wrapper)"; d=@("bevy_rand = `"0.14`"")},
    @{n="forgia-events-bus";   c="foundation"; i="Typed event channels inter-crates"; d=@()},
    @{n="forgia-signals";      c="foundation"; i="Signal contracts registry inter-crates"; d=@()},
    @{n="forgia-genome-core";  c="foundation"; i="TOML loader + hot-reload core (Forgia unique)"; d=@()},
    @{n="forgia-genome-registry"; c="foundation"; i="GenomeRegistry trait + catalogue accessors"; d=@()},
    @{n="forgia-manifest";     c="foundation"; i="Capability discovery for AI agents (parse manifest.toml)"; d=@()},
    @{n="forgia-editor-kit-macros"; c="macros"; i="Inspectable derive + registry macros"; d=@()},

    # ══════════════ B. POST-PROCESS (45) ══════════════
    # Stylised (15)
    @{n="forgia-pp-toon";       c="postprocess"; i="Toon/cel-shading band quantization"; d=@()},
    @{n="forgia-pp-outline";    c="postprocess"; i="Edge outline (sobel-derived) for cel-shading"; d=@()},
    @{n="forgia-pp-sketch";     c="postprocess"; i="Pencil sketch hatch lines"; d=@()},
    @{n="forgia-pp-halftone";   c="postprocess"; i="Halftone dot pattern"; d=@()},
    @{n="forgia-pp-hex-pixelate"; c="postprocess"; i="Hex pixelation"; d=@()},
    @{n="forgia-pp-pixelation"; c="postprocess"; i="Square pixelation"; d=@()},
    @{n="forgia-pp-kuwahara";   c="postprocess"; i="Kuwahara painterly filter"; d=@()},
    @{n="forgia-pp-oil-painting"; c="postprocess"; i="Oil painting stylise"; d=@()},
    @{n="forgia-pp-emboss";     c="postprocess"; i="Emboss filter"; d=@()},
    @{n="forgia-pp-sobel-edge"; c="postprocess"; i="Sobel edge detection"; d=@()},
    @{n="forgia-pp-cross-hatch"; c="postprocess"; i="Cross-hatch shading"; d=@()},
    @{n="forgia-pp-ascii";      c="postprocess"; i="ASCII art post-process"; d=@()},
    @{n="forgia-pp-matrix";     c="postprocess"; i="Matrix code rain"; d=@()},
    @{n="forgia-pp-crt";        c="postprocess"; i="CRT TV scanline + curvature"; d=@()},
    @{n="forgia-pp-scanlines";  c="postprocess"; i="Horizontal scanlines"; d=@()},
    # Color/expo (10)
    @{n="forgia-pp-bloom";      c="postprocess"; i="Tunable bloom (extends Bevy default)"; d=@()},
    @{n="forgia-pp-tonemapping"; c="postprocess"; i="Tonemapping presets (ACES, Reinhard, etc.)"; d=@()},
    @{n="forgia-pp-color-grading"; c="postprocess"; i="LUT-based color grading per game mode"; d=@()},
    @{n="forgia-pp-vibrance";   c="postprocess"; i="Vibrance/saturation boost"; d=@()},
    @{n="forgia-pp-sepia";      c="postprocess"; i="Sepia tone"; d=@()},
    @{n="forgia-pp-grayscale";  c="postprocess"; i="Grayscale (luma-weighted)"; d=@()},
    @{n="forgia-pp-invert";     c="postprocess"; i="Color invert"; d=@()},
    @{n="forgia-pp-posterize";  c="postprocess"; i="Posterize (quantize levels)"; d=@()},
    @{n="forgia-pp-palette-quantize"; c="postprocess"; i="Palette quantization (LUT lookup)"; d=@()},
    @{n="forgia-pp-auto-exposure"; c="postprocess"; i="Auto-exposure (luminance adaptation)"; d=@()},
    # Distortion/glitch (10)
    @{n="forgia-pp-chromatic-aberration"; c="postprocess"; i="Chromatic aberration RGB shift"; d=@()},
    @{n="forgia-pp-glitch";     c="postprocess"; i="Digital glitch artifacts"; d=@()},
    @{n="forgia-pp-distortion"; c="postprocess"; i="Generic UV distortion"; d=@()},
    @{n="forgia-pp-swirl";      c="postprocess"; i="Swirl distortion"; d=@()},
    @{n="forgia-pp-kaleidoscope"; c="postprocess"; i="Kaleidoscope mirror"; d=@()},
    @{n="forgia-pp-color-split"; c="postprocess"; i="Channel split RGB"; d=@()},
    @{n="forgia-pp-radial-blur"; c="postprocess"; i="Radial blur (zoom or rotation)"; d=@()},
    @{n="forgia-pp-gaussian-blur"; c="postprocess"; i="Gaussian blur"; d=@()},
    @{n="forgia-pp-motion-blur"; c="postprocess"; i="Camera motion blur"; d=@()},
    @{n="forgia-pp-tilt-shift"; c="postprocess"; i="Tilt-shift miniature effect"; d=@()},
    # Ambiance (10)
    @{n="forgia-pp-vignette";   c="postprocess"; i="Edge darkening vignette"; d=@()},
    @{n="forgia-pp-fog";        c="postprocess"; i="Screen-space fog overlay"; d=@()},
    @{n="forgia-pp-distance-fog"; c="postprocess"; i="Depth-based fog"; d=@()},
    @{n="forgia-pp-rain";       c="postprocess"; i="Screen rain droplets"; d=@()},
    @{n="forgia-pp-heat-haze";  c="postprocess"; i="Heat haze distortion"; d=@()},
    @{n="forgia-pp-frosted-glass"; c="postprocess"; i="Frosted glass blur"; d=@()},
    @{n="forgia-pp-night-vision"; c="postprocess"; i="Night vision green tint + noise"; d=@()},
    @{n="forgia-pp-thermal";    c="postprocess"; i="Thermal vision palette"; d=@()},
    @{n="forgia-pp-dream";      c="postprocess"; i="Dream-like soft glow"; d=@()},
    @{n="forgia-pp-light-streaks"; c="postprocess"; i="Light streak (anamorphic)"; d=@()},

    # ══════════════ C. RENDERING AVANCE (12) ══════════════
    @{n="forgia-render-dof";    c="render"; i="Depth of field circular bokeh"; d=@()},
    @{n="forgia-render-ssao";   c="render"; i="Screen-space ambient occlusion"; d=@()},
    @{n="forgia-render-ssr";    c="render"; i="Screen-space reflections"; d=@()},
    @{n="forgia-render-pcss";   c="render"; i="Percentage-closer soft shadows"; d=@()},
    @{n="forgia-render-oit";    c="render"; i="Order-independent transparency"; d=@()},
    @{n="forgia-render-rt";     c="render"; i="Raytracing (optional, hardware)"; d=@()},
    @{n="forgia-render-forward-decal"; c="render"; i="Forward decal rendering"; d=@()},
    @{n="forgia-render-env-map"; c="render"; i="Environment cubemap reflection"; d=@()},
    @{n="forgia-render-god-rays"; c="render"; i="Volumetric god rays"; d=@()},
    @{n="forgia-render-skybox"; c="render"; i="Skybox cubemap with day/night cycle"; d=@()},
    @{n="forgia-render-atmosphere"; c="render"; i="Atmospheric scattering"; d=@()},
    @{n="forgia-render-clouds"; c="render"; i="Volumetric clouds"; d=@()},

    # ══════════════ D. WORLD & TERRAIN (8 — terrain exists) ══════════════
    @{n="forgia-foliage";       c="world"; i="Vegetation placement (grass, trees) genome-driven"; d=@()},
    @{n="forgia-water";         c="world"; i="bevy_water wrapper + WaterTiles management"; d=@("bevy_water = `"0.18`"")},
    @{n="forgia-underwater";    c="world"; i="Player swim + underwater PP gate (sea_level)"; d=@()},
    @{n="forgia-grid";          c="world"; i="Debug + builder grid overlay"; d=@()},
    @{n="forgia-spline";        c="world"; i="Splines/paths/rails for AI/cinematics"; d=@()},
    @{n="forgia-shape-library"; c="world"; i="Primitive shapes library (cube/sphere/capsule)"; d=@()},
    @{n="forgia-level-presets"; c="world"; i="Level templates (Arena/RPG/Race/Survival)"; d=@()},
    @{n="forgia-prefab";        c="world"; i="Blueprint/prefab system for AI assembly"; d=@()},
    @{n="forgia-scene";         c="world"; i="Scene loader + map_switch + DespawnOnExit"; d=@()},

    # ══════════════ E. GAMEPLAY ATOMIC (25) ══════════════
    @{n="forgia-weapon-hitscan"; c="combat"; i="Instant raycast weapon with falloff"; d=@()},
    @{n="forgia-weapon-projectile"; c="combat"; i="Projectile weapon (rockets, grenades)"; d=@()},
    @{n="forgia-weapon-melee";  c="combat"; i="Melee weapon swing arc + capsule cast"; d=@()},
    @{n="forgia-weapon-charged"; c="combat"; i="Charged shot (hold to charge)"; d=@()},
    @{n="forgia-weapon-beam";   c="combat"; i="Continuous beam weapon"; d=@()},
    @{n="forgia-damage";        c="combat"; i="Health component + DamageEvent system"; d=@()},
    @{n="forgia-armor";         c="combat"; i="Armor mitigation layer"; d=@()},
    @{n="forgia-status-effects"; c="combat"; i="Burn/slow/stun/poison status with stacking"; d=@()},
    @{n="forgia-viewmodel";     c="combat"; i="FPS arms viewmodel (1P weapon render)"; d=@()},
    @{n="forgia-crosshair";     c="combat"; i="Dynamic crosshair (spread, hit confirm)"; d=@()},
    @{n="forgia-hitmarker";     c="combat"; i="Hit confirmation visual + audio"; d=@()},
    @{n="forgia-player-fps";    c="player"; i="FPS 1P controller (kinematic char ctrl)"; d=@()},
    @{n="forgia-player-tp";     c="player"; i="3P RPG controller (orbit camera)"; d=@()},
    @{n="forgia-player-platformer"; c="player"; i="Platformer controller (coyote, jump buffer)"; d=@()},
    @{n="forgia-player-vehicle-control"; c="player"; i="Vehicle control wrapper"; d=@()},
    @{n="forgia-camera-orbit";  c="player"; i="Orbit camera (3P RPG, with collision)"; d=@()},
    @{n="forgia-camera-fps";    c="player"; i="FPS camera (1P, head-bob)"; d=@()},
    @{n="forgia-inventory";     c="gameplay"; i="Inventory grid 80 slots (LOCK-INV-1)"; d=@()},
    @{n="forgia-equipment";     c="gameplay"; i="Equip slots (weapon, armor, accessory)"; d=@()},
    @{n="forgia-crafting";      c="gameplay"; i="Recipe + craft system genome-driven"; d=@()},
    @{n="forgia-loot-tables";   c="gameplay"; i="Drop tables genome-driven (rarity)"; d=@()},
    @{n="forgia-xp-curves";     c="gameplay"; i="XP/level curves genome-driven (Forgia unique)"; d=@()},
    @{n="forgia-skill-tree";    c="gameplay"; i="Skill tree (nodes, prereqs, allocation)"; d=@()},
    @{n="forgia-quests";        c="gameplay"; i="Quest system (objectives, rewards)"; d=@()},
    @{n="forgia-dialogue";      c="gameplay"; i="Dialogue trees (living_weapons V1 voiced)"; d=@()},

    # ══════════════ F. IA & BEHAVIOR (10) ══════════════
    @{n="forgia-ai-arena-bot";  c="ai"; i="Arena FPS bot (V1 respawn + nav)"; d=@()},
    @{n="forgia-ai-utility";    c="ai"; i="Utility AI (curves + scorers)"; d=@()},
    @{n="forgia-ai-bt";         c="ai"; i="Behavior trees"; d=@()},
    @{n="forgia-ai-goap";       c="ai"; i="GOAP (Goal-Oriented Action Planning)"; d=@()},
    @{n="forgia-ai-state-machine"; c="ai"; i="FSM helper for AI"; d=@()},
    @{n="forgia-ai-perception"; c="ai"; i="Vision/hearing cones + memory"; d=@()},
    @{n="forgia-ai-blackboard"; c="ai"; i="Shared blackboard for AI knowledge"; d=@()},
    @{n="forgia-ai-navmesh";    c="ai"; i="Navmesh wrapper (bevy_landmass)"; d=@("bevy_landmass = `"0.13`"")},
    @{n="forgia-ai-flocking";   c="ai"; i="Boids flocking behavior"; d=@()},
    @{n="forgia-ai-formation";  c="ai"; i="Squad formation movement"; d=@()},

    # ══════════════ G. EFFETS & JUICE (12) ══════════════
    @{n="forgia-vfx-hanabi";    c="effects"; i="Hanabi VFX wrapper + pre-spawn dummy anti-freeze"; d=@()},
    @{n="forgia-vfx-decals";    c="effects"; i="Bullet impact decals"; d=@()},
    @{n="forgia-juice-camera-shake"; c="effects"; i="Camera shake (trauma-based)"; d=@()},
    @{n="forgia-juice-hit-stop"; c="effects"; i="Hit-stop time pause genome-driven"; d=@()},
    @{n="forgia-juice-fov-punch"; c="effects"; i="FOV punch on shoot/hit"; d=@()},
    @{n="forgia-juice-recoil";  c="effects"; i="Apex-style camera recoil event"; d=@()},
    @{n="forgia-juice-screen-flash"; c="effects"; i="Screen flash on dmg/heal"; d=@()},
    @{n="forgia-damage-numbers"; c="effects"; i="Floating damage numbers (3D world-space)"; d=@()},
    @{n="forgia-trails";        c="effects"; i="Motion trails (sword, projectile)"; d=@()},
    @{n="forgia-ribbons";       c="effects"; i="Ribbon trails (smoke, fire)"; d=@()},
    @{n="forgia-vfx-tracers";   c="effects"; i="Bullet tracers cached mesh (Forgia gunfeel V5)"; d=@()},
    @{n="forgia-vfx-impact-library"; c="effects"; i="Impact preset library per surface type"; d=@()},

    # ══════════════ H. AUDIO (8) ══════════════
    @{n="forgia-audio-core";    c="audio"; i="bevy_kira_audio wrapper + channels"; d=@()},
    @{n="forgia-audio-footsteps"; c="audio"; i="Footsteps per surface material"; d=@()},
    @{n="forgia-audio-biome";   c="audio"; i="Biome ambient audio (Forgia)"; d=@()},
    @{n="forgia-audio-mixer";   c="audio"; i="Channel mixer + per-channel volume"; d=@()},
    @{n="forgia-audio-ducking"; c="audio"; i="Audio ducking (music behind voice)"; d=@()},
    @{n="forgia-audio-music-state"; c="audio"; i="Adaptive music state machine"; d=@()},
    @{n="forgia-audio-occlusion"; c="audio"; i="3D audio occlusion (walls muffle)"; d=@()},
    @{n="forgia-audio-voicelines"; c="audio"; i="Voicelines registry (living_weapons V1)"; d=@()},

    # ══════════════ I. UI/UX (12) ══════════════
    @{n="forgia-ui-hud";        c="ui"; i="HP/ammo/crosshair/score HUD"; d=@()},
    @{n="forgia-ui-menu";       c="ui"; i="Start menu + pause + settings"; d=@()},
    @{n="forgia-ui-inventory";  c="ui"; i="Inventory panel UI (80 slots pagination)"; d=@()},
    @{n="forgia-ui-dialogue";   c="ui"; i="Dialogue UI (choices, portrait)"; d=@()},
    @{n="forgia-ui-notifications"; c="ui"; i="Toast notifications queue"; d=@()},
    @{n="forgia-ui-tooltip";    c="ui"; i="Tooltip on hover"; d=@()},
    @{n="forgia-ui-minimap";    c="ui"; i="Minimap with cached recalc (L8 lock)"; d=@()},
    @{n="forgia-ui-objectives"; c="ui"; i="Quest objectives panel"; d=@()},
    @{n="forgia-ui-loadscreen"; c="ui"; i="Loading screen with progress"; d=@()},
    @{n="forgia-ui-credits";    c="ui"; i="Credits roll"; d=@()},
    @{n="forgia-ui-settings-panel"; c="ui"; i="Settings panel (graphics, audio, controls)"; d=@()},
    @{n="forgia-ui-gauges";     c="ui"; i="Reusable gauges/bars widgets"; d=@()},

    # ══════════════ J. INPUT & CONTROLS (5) ══════════════
    @{n="forgia-input-keybind"; c="input"; i="Keybind registry + AZERTY default"; d=@()},
    @{n="forgia-input-gamepad"; c="input"; i="Gamepad mapping + rumble"; d=@()},
    @{n="forgia-input-rebind-ui"; c="input"; i="Rebind UI panel"; d=@()},
    @{n="forgia-input-azerty-qwerty"; c="input"; i="AZERTY/QWERTY layout swap"; d=@()},
    @{n="forgia-input-recording"; c="input"; i="Input recording for replay/QA"; d=@()},

    # ══════════════ K. PHYSICS & ANIMATION (6) ══════════════
    @{n="forgia-physics";       c="physics"; i="Rapier wrapper + collision groups G1-G5"; d=@("bevy_rapier3d = `"0.33`"")},
    @{n="forgia-ragdoll";       c="physics"; i="Ragdoll on death"; d=@("bevy_rapier3d = `"0.33`"")},
    @{n="forgia-cloth";         c="physics"; i="Cloth simulation (capes, banners)"; d=@()},
    @{n="forgia-animation-mixamo"; c="animation"; i="Mixamo rig + clip registry (player_anim_*)"; d=@()},
    @{n="forgia-animation-blend"; c="animation"; i="Animation blend state machine"; d=@()},
    @{n="forgia-ik";            c="animation"; i="IK (foot placement, look-at)"; d=@()},

    # ══════════════ L. NETWORK & MULTI (8) ══════════════
    @{n="forgia-net-lightyear"; c="network"; i="lightyear wrapper + replication channels"; d=@("lightyear = `"0.26.4`"")},
    @{n="forgia-net-rollback";  c="network"; i="Rollback netcode helper"; d=@()},
    @{n="forgia-net-lobby";     c="network"; i="Lobby system (create/join/list)"; d=@()},
    @{n="forgia-net-matchmaking"; c="network"; i="Matchmaking queue"; d=@()},
    @{n="forgia-coedit-crdt";   c="network"; i="Automerge JSON-CRDT co-edit (Forgia unique)"; d=@()},
    @{n="forgia-net-chat";      c="network"; i="In-game chat (replicated)"; d=@()},
    @{n="forgia-net-voice";     c="network"; i="Voice chat (opus)"; d=@()},
    @{n="forgia-net-replication-genome"; c="network"; i="Genome-aware replication policies"; d=@()},

    # ══════════════ M. EDITOR & TOOLS (15) ══════════════
    @{n="forgia-editor-core";   c="editor"; i="Editor app shell (Play/Build/Edit modes)"; d=@()},
    @{n="forgia-editor-hierarchy"; c="editor"; i="Scene hierarchy tree panel"; d=@("bevy_egui = `"0.39.1`"")},
    @{n="forgia-editor-inspector"; c="editor"; i="Reflect-based component inspector"; d=@("bevy_egui = `"0.39.1`"")},
    @{n="forgia-editor-asset-browser"; c="editor"; i="Asset browser thumbnails"; d=@("bevy_egui = `"0.39.1`"")},
    @{n="forgia-editor-command-palette"; c="editor"; i="Cmd+P fuzzy palette"; d=@("bevy_egui = `"0.39.1`"")},
    @{n="forgia-editor-gizmo";  c="editor"; i="Translate/rotate/scale gizmos"; d=@()},
    @{n="forgia-editor-undo-redo"; c="editor"; i="Undo/redo command pattern"; d=@()},
    @{n="forgia-editor-theme";  c="editor"; i="egui theme presets"; d=@("bevy_egui = `"0.39.1`"")},
    @{n="forgia-editor-viewport"; c="editor"; i="Editor viewport camera + grid"; d=@()},
    @{n="forgia-editor-blueprint"; c="editor"; i="Visual scripting graph editor"; d=@("bevy_egui = `"0.39.1`"")},
    @{n="forgia-editor-material"; c="editor"; i="Material node graph editor"; d=@("bevy_egui = `"0.39.1`"")},
    @{n="forgia-editor-particle"; c="editor"; i="Hanabi particle editor"; d=@("bevy_egui = `"0.39.1`"")},
    @{n="forgia-editor-animation"; c="editor"; i="Animation timeline editor"; d=@("bevy_egui = `"0.39.1`"")},
    @{n="forgia-editor-terrain"; c="editor"; i="Terrain brush sculpt + paint"; d=@()},
    @{n="forgia-editor-foliage"; c="editor"; i="Foliage placement brush"; d=@()},

    # ══════════════ N. SCRIPTING (5) ══════════════
    @{n="forgia-scripting-luau"; c="scripting"; i="bevy_mod_scripting Luau wrapper"; d=@("bevy_mod_scripting = `"0.19`"")},
    @{n="forgia-script-api-bindings"; c="scripting"; i="ECS bindings exposed to Luau"; d=@()},
    @{n="forgia-script-variables"; c="scripting"; i="Script-mutable variables registry"; d=@()},
    @{n="forgia-script-hot-reload"; c="scripting"; i="Hot-reload Luau on file change"; d=@()},
    @{n="forgia-script-sandbox"; c="scripting"; i="Sandboxed script exec (user-generated)"; d=@()},

    # ══════════════ O. ASSETS PIPELINE (8) ══════════════
    @{n="forgia-assets-bundle"; c="assets"; i="Asset bundle pack (zstd)"; d=@("zstd = `"0.13`"")},
    @{n="forgia-hyworld-import"; c="assets"; i="HyWorld .hyworld importer"; d=@()},
    @{n="forgia-meshy-import";  c="assets"; i="Meshy AI GLB importer"; d=@()},
    @{n="forgia-import-glb-validated"; c="assets"; i="GLB import with PBR validation"; d=@()},
    @{n="forgia-import-usd";    c="assets"; i="USD import (V2)"; d=@()},
    @{n="forgia-asset-lod-generator"; c="assets"; i="LOD auto-generation"; d=@()},
    @{n="forgia-asset-atlas-packer"; c="assets"; i="Texture atlas packer"; d=@()},
    @{n="forgia-asset-cdn";     c="assets"; i="CDN-fetched asset registry (V2 marketplace)"; d=@()},

    # ══════════════ P. GENOME & DATA-DRIVEN (Forgia unique, 6) ══════════════
    @{n="forgia-genome-catalogue"; c="genome"; i="Catalogue of all genomes (weapon, biome, enemy)"; d=@()},
    @{n="forgia-genome-sync";   c="genome"; i="Genome -> FpsTuning sync macro"; d=@()},
    @{n="forgia-genome-validator"; c="genome"; i="Schema validation + missing-gene detection"; d=@()},
    @{n="forgia-genome-economy"; c="genome"; i="XP/loot/price curves genome (economy-designer)"; d=@()},
    @{n="forgia-genome-balance"; c="genome"; i="Balance audit + drift detection"; d=@()},
    @{n="forgia-genome-ai-personality"; c="genome"; i="AI personality traits (aggression, fear)"; d=@()},

    # ══════════════ Q. QA / OBSERVABILITY (Forgia moat, 9) ══════════════
    @{n="forgia-qa-core";       c="qa"; i="QA pipeline core"; d=@()},
    @{n="forgia-qa-telemetry";  c="qa"; i="Telemetry collection (24 sensors JSON)"; d=@()},
    @{n="forgia-qa-invariants"; c="qa"; i="Runtime invariant checks (CHECK 1-95)"; d=@()},
    @{n="forgia-qa-replay";     c="qa"; i="Input replay for regression testing"; d=@()},
    @{n="forgia-qa-harness";    c="qa"; i="Headless test harness"; d=@()},
    @{n="forgia-qa-autopilot";  c="qa"; i="Autopilot AI bot for QA runs"; d=@()},
    @{n="forgia-qa-kg";         c="qa"; i="Knowledge graph for bug patterns"; d=@()},
    @{n="forgia-qa-vlm";        c="qa"; i="VLM (LLaVA) screenshot oracle"; d=@()},
    @{n="forgia-observability"; c="qa"; i="Health monitor + lag detector + watchdog"; d=@()},

    # ══════════════ R. MONETISATION & META (6) ══════════════
    @{n="forgia-steam";         c="meta"; i="bevy-steamworks wrapper (story-424)"; d=@("bevy-steamworks = `"0.16`"")},
    @{n="forgia-founders-pass"; c="meta"; i="Founders pass entitlement check"; d=@()},
    @{n="forgia-marketplace-client"; c="meta"; i="Marketplace client (V2 igc-server)"; d=@()},
    @{n="forgia-analytics";     c="meta"; i="Analytics events (funnel)"; d=@()},
    @{n="forgia-anticheat";     c="meta"; i="Anti-cheat hooks (V2)"; d=@()},
    @{n="forgia-telemetry-funnel"; c="meta"; i="Conversion funnel telemetry"; d=@()},

    # ══════════════ S. GAME MODES (8) ══════════════
    @{n="forgia-mode-fps-arena"; c="mode"; i="FPS arena mode plugin (V1)"; d=@()},
    @{n="forgia-mode-rpg-openworld"; c="mode"; i="RPG openworld mode plugin (V2)"; d=@()},
    @{n="forgia-mode-race";     c="mode"; i="Racing mode plugin"; d=@()},
    @{n="forgia-mode-survival"; c="mode"; i="Survival mode plugin"; d=@()},
    @{n="forgia-mode-platformer"; c="mode"; i="Platformer mode plugin"; d=@()},
    @{n="forgia-mode-rts";      c="mode"; i="RTS mode plugin"; d=@()},
    @{n="forgia-mode-puzzle";   c="mode"; i="Puzzle mode plugin"; d=@()},
    @{n="forgia-mode-roguelite"; c="mode"; i="Roguelite mode plugin (procedural)"; d=@()},

    # ══════════════ T. RENZORA PORTS — VENDORED EXTERNAL CRATES (6) ══════════════
    @{n="forgia-mod-outline";   c="port"; i="Port bevy_mod_outline (renzora vendored)"; d=@()},
    @{n="forgia-gauge";         c="port"; i="Port bevy_gauge (renzora vendored)"; d=@()},
    @{n="forgia-silk";          c="port"; i="Port bevy_silk cloth (renzora vendored)"; d=@()},
    @{n="forgia-oxr";           c="port"; i="Port bevy_oxr VR (renzora vendored)"; d=@()},
    @{n="forgia-navigator";     c="port"; i="Port vleue_navigator (renzora vendored)"; d=@()},
    @{n="forgia-websocket";     c="port"; i="Port websocket_plugin (renzora vendored)"; d=@()}
)

# ─────────────────────────────────────────────────────────────────────────────
# Deps templates per category
# ─────────────────────────────────────────────────────────────────────────────
function Get-DepsForCategory($cat) {
    switch ($cat) {
        "foundation"   { return @("forgia-core = { workspace = true }") }
        "macros"       { return @() }
        "postprocess"  { return @("forgia-core = { workspace = true }") }
        "render"       { return @("forgia-core = { workspace = true }") }
        "world"        { return @("forgia-core = { workspace = true }") }
        "combat"       { return @("forgia-core = { workspace = true }") }
        "player"       { return @("forgia-core = { workspace = true }") }
        "gameplay"     { return @("forgia-core = { workspace = true }", "serde = { workspace = true }") }
        "ai"           { return @("forgia-core = { workspace = true }") }
        "effects"      { return @("forgia-core = { workspace = true }", "bevy_hanabi = { workspace = true }") }
        "audio"        { return @("forgia-core = { workspace = true }", "bevy_kira_audio = { workspace = true }") }
        "ui"           { return @("forgia-core = { workspace = true }", "bevy_egui = { workspace = true }") }
        "input"        { return @("forgia-core = { workspace = true }", "leafwing-input-manager = { workspace = true }") }
        "physics"      { return @("forgia-core = { workspace = true }") }
        "animation"    { return @("forgia-core = { workspace = true }") }
        "network"      { return @("forgia-core = { workspace = true }", "serde = { workspace = true }") }
        "editor"       { return @("forgia-core = { workspace = true }") }
        "scripting"    { return @("forgia-core = { workspace = true }") }
        "assets"       { return @("forgia-core = { workspace = true }") }
        "genome"       { return @("forgia-core = { workspace = true }", "serde = { workspace = true }", "toml = { workspace = true }") }
        "qa"           { return @("forgia-core = { workspace = true }", "serde = { workspace = true }", "serde_json = { workspace = true }") }
        "meta"         { return @("forgia-core = { workspace = true }") }
        "mode"         { return @("forgia-core = { workspace = true }") }
        "port"         { return @() }
        default        { return @("forgia-core = { workspace = true }") }
    }
}

# ─────────────────────────────────────────────────────────────────────────────
# Scaffold one crate
# ─────────────────────────────────────────────────────────────────────────────
function New-Crate($entry) {
    $name = $entry.n
    $cat  = $entry.c
    $intent = $entry.i
    $extra = $entry.d

    $crateDir = Join-Path $cratesDir $name
    if (Test-Path $crateDir) {
        Write-Host "[skip] $name (exists)" -ForegroundColor DarkGray
        return
    }

    New-Item -ItemType Directory -Path $crateDir -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $crateDir "src") -Force | Out-Null

    # Plugin Rust struct name : forgia-pp-toon -> ForgiaPpToonPlugin
    $pluginName = ($name -split '-' | ForEach-Object { $_.Substring(0,1).ToUpper() + $_.Substring(1) }) -join ''
    $pluginName += "Plugin"

    $deps = Get-DepsForCategory $cat
    $allDeps = @("bevy = { workspace = true }") + $deps + $extra
    $depsBlock = ($allDeps | ForEach-Object { $_ }) -join "`n"

    # Cargo.toml
    $cargoToml = @"
[package]
name = "$name"
version = "0.1.0"
edition = "2021"
description = "$intent"

[lints]
workspace = true

[dependencies]
$depsBlock
"@
    Set-Content -Path (Join-Path $crateDir "Cargo.toml") -Value $cargoToml -Encoding UTF8

    # src/lib.rs
    $libRs = @"
//! $name
//!
//! $intent
//!
//! Category : $cat

use bevy::prelude::*;

/// Plugin Bevy. Add to App via `app.add_plugins($pluginName)`.
pub struct $pluginName;

impl Plugin for $pluginName {
    fn build(&self, _app: &mut App) {
        // TODO: implement
    }
}
"@
    Set-Content -Path (Join-Path $crateDir "src/lib.rs") -Value $libRs -Encoding UTF8

    # manifest.toml (AI-callable contract)
    $manifestToml = @"
# Forgia capability manifest — consumed by AI assembly agent.
# Schema v1.

name = "$name"
category = "$cat"
intent = "$intent"
version = "0.1.0"
status = "stub"   # stub | wip | ready | stable

# Genes exposed via TOML hot-reload (filled when impl).
genes = []

# ECS events/signals emitted (filled when impl).
emits = []

# ECS events/signals consumed.
consumes = []

# Sensor JSON file (forgia_<feature>.json) — required if runtime feature.
sensor = ""

# Workspace deps (filled by scaffold).
depends = []
"@
    Set-Content -Path (Join-Path $crateDir "manifest.toml") -Value $manifestToml -Encoding UTF8

    # README.md
    $readme = @"
# $name

> $intent

**Category** : ``$cat`` · **Status** : ``stub``

## Usage (AI assembly)

``````rust
use $($name -replace '-','_')::$pluginName;
app.add_plugins($pluginName);
``````

## Manifest

See [``manifest.toml``](./manifest.toml) for AI-callable contract (genes, emits, consumes, sensor).
"@
    Set-Content -Path (Join-Path $crateDir "README.md") -Value $readme -Encoding UTF8

    Write-Host "[new ] $name" -ForegroundColor Green
}

# ─────────────────────────────────────────────────────────────────────────────
# Main
# ─────────────────────────────────────────────────────────────────────────────
Write-Host "Forgia Rewrite — Scaffold $($catalogue.Count) crates" -ForegroundColor Cyan
Write-Host "Target : $cratesDir" -ForegroundColor Cyan
Write-Host ""

$created = 0
$skipped = 0
foreach ($entry in $catalogue) {
    $before = Test-Path (Join-Path $cratesDir $entry.n)
    New-Crate $entry
    if ($before) { $skipped++ } else { $created++ }
}

Write-Host ""
Write-Host "Done. Created : $created. Skipped : $skipped." -ForegroundColor Cyan
Write-Host ""
Write-Host "Next : run 'pwsh -File scripts/update-workspace-members.ps1' to update Cargo.toml [workspace] members." -ForegroundColor Yellow
