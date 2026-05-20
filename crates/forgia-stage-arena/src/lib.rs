//! forgia-stage-arena — Generic stage arena loader (data-driven, mode-agnostic).
//!
//! Story-483 V7 P0 (NEW crate). Loads a bounded arena from a `stage_id`
//! referenced in `assets/genomes/roguelite_stages.toml`. Spawns :
//! - terrain bornée (extent_m × extent_m) ;
//! - ramparts hexagonaux KayKit (réutilise pattern story-441 village) ;
//! - anchor points (PlayerSpawn / PoiSlot / BossPad / Landmark) via
//!   `forgia-anchor` ;
//! - POIs (chest / shrine / elite-pad / boss-pad) via `forgia-prefab`.
//!
//! Consommé par `forgia-mode-roguelite` (V1) et par `forgia-mode-fps-arena`
//! (M4 backlog migration).
//!
//! ## P0 scope (this commit)
//!
//! - Types : `StageLoadRequest`, `StageLoadResult`, `StageState`, `StageDef`,
//!   `RogueliteStagesGenome`, `PoiDef`, `RoguelitePoisGenome`, `RampartsShape`.
//! - Plugin shell : registers genomes + insert default `StageLoadResult` +
//!   sensor writer 1Hz.
//! - Pure helpers : `ramparts_hex_positions`, `poi_anchor_positions`,
//!   `splitmix64`, `severity_for_stage`, `next_step_for_stage`.
//! - Tests purs ~12 cases.
//! - **Pas encore de spawn runtime** — c'est P1.
//!
//! ## Sources
//!
//! - GDC 2022 Returnal "Never The Same Twice" (Ethan Watson) — handcrafted +
//!   procedural connections.
//! - RoR2 Hopoo design ([gamedeveloper.com](https://www.gamedeveloper.com/design/how-moving-from-2d-to-3d-shaped-the-design-of-i-risk-of-rain-2-i-)) — stages pre-built + objets randomisés.
//! - Forgia memory `[[reference-loader-request-result-pattern]]` (story-441).
//! - Forgia memory `[[reference-pattern-genome-driven-plugin-with-sensor]]`.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_anchor::{AnchorKind, AnchorPoint, AnchorStats};
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_prefab::{spawn_gltf_prefab, PrefabSpawn, PrefabStats};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

const STAGES_GENOME_PATH: &str = "genomes/roguelite_stages.toml";
const POIS_GENOME_PATH: &str = "genomes/roguelite_pois.toml";
const SENSOR_PATH: &str = "forgia2_stage.json";
const SENSOR_WRITE_PERIOD_SEC: f64 = 1.0;

// ─── splitmix64 (inline, déterministe, pas de dep RNG) ──────────────────────

/// SplitMix64 — Vigna 2014. Stateful, déterministe par seed. Réutilise le
/// même algo que `forgia-stage-graph` pour cohérence cross-crate.
#[inline]
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

// ─── StageState ─────────────────────────────────────────────────────────────

/// État cycle de vie d'un stage load. Transitions :
/// `Idle → Loading → Ready` (happy path) ou `Idle → Loading → Error`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageState {
    #[default]
    Idle,
    Loading,
    Ready,
    Error,
}

impl StageState {
    pub const fn as_str(self) -> &'static str {
        match self {
            StageState::Idle => "idle",
            StageState::Loading => "loading",
            StageState::Ready => "ready",
            StageState::Error => "error",
        }
    }
}

// ─── RampartsShape ──────────────────────────────────────────────────────────

/// Forme du bornage de l'arène. V1 supporte hexagonal seulement (pattern village
/// story-441). Extensions futures : `Square`, `Cliffs` (heightmap forced edge).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RampartsShape {
    #[default]
    Hexagonal,
}

// ─── StageDef (genome data) ─────────────────────────────────────────────────

/// Définition d'un stage chargée depuis `assets/genomes/roguelite_stages.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct StageDef {
    /// Biome name (matches `forgia-terrain` BiomeKind string).
    pub biome: String,
    /// Demi-côté de l'arène (m). Pour ramparts hex inscrits dans cercle rayon
    /// `arena_extent_m`, l'aire utile gameplay = πr² ≈ 3.46 × extent_m².
    pub arena_extent_m: f32,
    /// Kit KayKit à utiliser pour les ramparts (e.g. "kaykit_dungeon",
    /// "medieval_hexagon"). Lookup résolu côté loader P1.
    pub ramparts_kit: String,
    /// Forme du bornage. V1 = hexagonal exclusivement.
    #[serde(default)]
    pub ramparts_shape: RampartsShape,
    /// Nombre d'anchor slots pour POIs (chest / elite-pad / etc.) sur le
    /// cercle anchor. PlayerSpawn et BossPad sont des anchors dédiés en plus.
    pub anchor_slots: u32,
    /// Music state à activer (e.g. "combat_intense"). Optionnel.
    #[serde(default)]
    pub music_state: Option<String>,
    /// Si true, force le spawn d'un BossPad anchor + POI prefab.
    #[serde(default)]
    pub boss_pad_required: bool,
    /// Override météo (e.g. "ashfall"). Optionnel — consommé par audio/VFX.
    #[serde(default)]
    pub weather_override: Option<String>,
    /// Longueur visuelle naturelle d'1 wall.glb du kit (m). Optionnel — si
    /// omis ou ≤ 0, fallback `wall_natural_len_for_kit(ramparts_kit)` (smart
    /// default par-kit). Hot-reload Shift+F12 supporté.
    #[serde(default)]
    pub wall_natural_len_m: Option<f32>,
}

/// Smart default par kit basé sur les dimensions natives du pack KayKit.
/// Sourcé empiriquement — à corriger si AABB runtime montre autre chose.
/// Pattern Larian Definition layer : ce mapping vit en code seulement parce
/// que les dimensions sont une propriété du pack asset, pas du gameplay.
pub fn wall_natural_len_for_kit(kit: &str) -> f32 {
    match kit {
        // KayKit Dungeon Pack wall.glb mesure environ 1m sur axe principal
        // (à confirmer empiriquement — sensor `wall_natural_len_used` exposé
        // pour itération sans rebuild).
        "kaykit_dungeon" => 1.0,
        // Medieval Hexagon Castle wall_straight.gltf : 1 hex tile-side, ~4m.
        "medieval_hexagon" => 4.0,
        _ => 1.0,
    }
}

/// Genome TOML : `[stages.<id>] StageDef`. Hot-reloadable via Shift+F12.
#[derive(Debug, Default, Clone, Deserialize, TypePath)]
pub struct RogueliteStagesGenome {
    #[serde(default)]
    pub stages: HashMap<String, StageDef>,
}

// ─── PoiDef (genome data) ───────────────────────────────────────────────────

/// Définition d'un POI (Point of Interest) — chest, elite-pad, boss-pad, etc.
#[derive(Debug, Clone, Deserialize)]
pub struct PoiDef {
    /// Weight pour pondération RNG. 0 = forced/never (e.g. boss_pad weight=0
    /// est forcé uniquement quand `stage.boss_pad_required`).
    pub weight: u32,
    /// Chemin GLB relatif à `assets/` (e.g. "models/kaykit/dungeon/chest_basic.glb").
    pub prefab: String,
    /// Type d'encounter déclenché par ce POI ("none", "wave_elite", etc.).
    #[serde(default = "default_encounter")]
    pub encounter: String,
    /// Footprint approximatif (m). Utilisé pour éviter overlap POI<->POI.
    pub size_m: f32,
}

fn default_encounter() -> String {
    "none".to_string()
}

#[derive(Debug, Default, Clone, Deserialize, TypePath)]
pub struct RoguelitePoisGenome {
    #[serde(default)]
    pub pois: HashMap<String, PoiDef>,
}

// ─── StageLoadRequest / StageLoadResult Resources ───────────────────────────

/// Resource Request (caller insert) — déclenche le load d'un stage.
///
/// Pattern Resource Request → Resource Result (story-441 reference). Consommé
/// par `spawn_stage_arena_on_request` (P1) qui spawn terrain + ramparts +
/// anchors et update `StageLoadResult`.
#[derive(Resource, Debug, Clone, Default)]
pub struct StageLoadRequest {
    /// Identifiant stage (clef `[stages.<id>]` dans roguelite_stages.toml).
    pub stage_id: String,
    /// Seed RNG pour placement anchors (host-authoritative reproductible).
    pub seed: u64,
}

/// Resource Result (loader update) — état runtime du stage chargé.
#[derive(Resource, Debug, Default)]
pub struct StageLoadResult {
    pub state: StageState,
    pub stage_id: String,
    pub biome: String,
    pub extent_m: f32,
    pub anchors_placed: u32,
    pub props_spawned: u32,
    /// Timestamp en secondes (since startup) du dernier passage en Loading.
    /// Utilisé pour détecter Loading > 5s (health alert).
    pub loading_started_secs: f64,
    /// Message d'erreur si state == Error.
    pub error_message: String,
    /// `wall_natural_len_m` effectif utilisé (TOML override OU smart default).
    /// Exposé pour debug calibration ramparts via `forgia2_stage.json`.
    pub wall_natural_len_used: f32,
    /// Nombre de murs tilés par segment hex (1 segment = 1/6 du périmètre).
    /// Diagnostic gaps/overlaps avant rebuild.
    pub walls_per_segment: u32,
    /// `music_state` du stage def actuel (toggle TOML). Vide si stage def n'en
    /// définit pas. Consommé par caller (forgia-mode-roguelite::sys_apply_stage_toggles).
    pub music_state_id: String,
    /// `weather_override` du stage def actuel. Vide si non défini. Consommé par
    /// future crate `forgia-weather` (V2). Visibilité runtime via sensor.
    pub weather_override: String,}

// ─── Genome handles Resource ────────────────────────────────────────────────

/// Storage des handles genome chargés — assure que les assets ne sont pas
/// drop entre frames. Pattern arena_bots.toml hot-reload.
#[derive(Resource, Default)]
pub struct StageGenomeHandles {
    pub stages: Handle<Genome<RogueliteStagesGenome>>,
    pub pois: Handle<Genome<RoguelitePoisGenome>>,
}

// ─── Marker Component ───────────────────────────────────────────────────────

/// Marker — toute entité spawnée par stage-arena la porte. Cleanup via
/// `DespawnOnExit(GameMode::Roguelite)` côté caller, ou via query directe.
#[derive(Component, Debug)]
pub struct StageArenaMarker;

// ─── Plugin ─────────────────────────────────────────────────────────────────

/// Bevy Plugin. Add via `app.add_plugins(ForgiaStageArenaPlugin)`.
///
/// **P0 scope** : registers genomes + insère Resources + lance sensor writer.
/// **P1 scope** : ajoutera `spawn_stage_arena_on_request` system.
pub struct ForgiaStageArenaPlugin;

impl Plugin for ForgiaStageArenaPlugin {
    fn build(&self, app: &mut App) {
        // Idempotent : add dependent plugins only once.
        if !app.is_plugin_added::<forgia_anchor::ForgiaAnchorPlugin>() {
            app.add_plugins(forgia_anchor::ForgiaAnchorPlugin);
        }
        if !app.is_plugin_added::<forgia_prefab::ForgiaPrefabPlugin>() {
            app.add_plugins(forgia_prefab::ForgiaPrefabPlugin);
        }
        app.init_asset::<Genome<RogueliteStagesGenome>>()
            .register_asset_loader(GenomeLoader::<RogueliteStagesGenome>::default())
            .init_asset::<Genome<RoguelitePoisGenome>>()
            .register_asset_loader(GenomeLoader::<RoguelitePoisGenome>::default())
            .init_resource::<StageLoadResult>()
            .init_resource::<StageGenomeHandles>()
            .init_resource::<StageArenaTuning>()
            .add_systems(Startup, load_stage_genomes)
            // L7 SystemSets — stage-arena is **mode-agnostic by design** : il sera
            // consommé par forgia-mode-roguelite (V1), forgia-mode-fps-arena (M4),
            // forgia-mode-rpg-openworld (POI hubs), etc. Pas de `run_if(in_state(
            // GameMode::X))` au niveau plugin — guard naturel via `request: Option<
            // Res<StageLoadRequest>>` + `q_existing` archetype scan (~0 cost si
            // empty). Caller responsible pour insert/cleanup le StageLoadRequest.
            .add_systems(
                Update,
                spawn_stage_arena_on_request
                    .in_set(forgia_core::prelude::GameSet::Movement),
            )
            .add_systems(
                Update,
                write_stage_sensor.in_set(forgia_core::prelude::GameSet::Sensors),
            );
        info!(
            "[forgia-stage-arena] Plugin loaded — genome paths: {} + {}",
            STAGES_GENOME_PATH, POIS_GENOME_PATH
        );
    }
}

// ─── Startup : load genomes ─────────────────────────────────────────────────

fn load_stage_genomes(
    asset_server: Res<AssetServer>,
    mut handles: ResMut<StageGenomeHandles>,
) {
    handles.stages = asset_server.load(STAGES_GENOME_PATH);
    handles.pois = asset_server.load(POIS_GENOME_PATH);
    info!(
        "[forgia-stage-arena] Genome handles loading: stages={} pois={}",
        STAGES_GENOME_PATH, POIS_GENOME_PATH
    );
}

// ─── Sensor writer 1Hz ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct StageSensorJson<'a> {
    id: &'a str,
    severity: &'a str,
    timestamp_secs: f64,
    state: &'a str,
    stage_id: &'a str,
    biome: &'a str,
    extent_m: f32,
    anchors_placed: u32,
    props_spawned: u32,
    wall_natural_len_used: f32,
    walls_per_segment: u32,
    music_state_id: &'a str,
    weather_override: &'a str,
    next_step: &'a str,
}

fn write_stage_sensor(
    result: Res<StageLoadResult>,
    tuning: Res<StageArenaTuning>,
    mut last_write: Local<f64>,
) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    if now - *last_write < SENSOR_WRITE_PERIOD_SEC {
        return;
    }
    *last_write = now;

    let loading_elapsed = if result.state == StageState::Loading {
        now - result.loading_started_secs
    } else {
        0.0
    };
    let severity = severity_for_stage_with_threshold(
        result.state,
        loading_elapsed,
        tuning.loading_sustained_warn_sec,
    );
    let next_step = next_step_for_stage_with_threshold(
        result.state,
        loading_elapsed,
        tuning.loading_sustained_warn_sec,
    );

    let payload = StageSensorJson {
        id: "stage",
        severity,
        timestamp_secs: now,
        state: result.state.as_str(),
        stage_id: &result.stage_id,
        biome: &result.biome,
        extent_m: result.extent_m,
        anchors_placed: result.anchors_placed,
        props_spawned: result.props_spawned,
        wall_natural_len_used: result.wall_natural_len_used,
        walls_per_segment: result.walls_per_segment,
        music_state_id: &result.music_state_id,
        weather_override: &result.weather_override,
        next_step,
    };
    if let Ok(json) = serde_json::to_string_pretty(&payload) {
        let _ = fs::write(SENSOR_PATH, json);
    }
}

// ─── Severity / Next-step (pure, testable) ──────────────────────────────────

/// Default seuil au-delà duquel un state `Loading` est considéré dégradé.
/// Surchargeable via Resource [`StageArenaTuning`] (conforme observability-required.md
/// "Genes genome — seuils/toggles hot-reloadables").
pub const DEFAULT_LOADING_SUSTAINED_WARN_SEC: f64 = 5.0;

/// Resource tunable — seuils observabilité stage-arena. Évite hardcode const
/// (cf. qa-lead audit story-483 BUG-483-08). Future : sync depuis
/// `debug_monitor.toml::dbg_stage_arena_loading_timeout_sec` (M2 wire-up).
#[derive(Resource, Debug, Clone)]
pub struct StageArenaTuning {
    pub loading_sustained_warn_sec: f64,
}

impl Default for StageArenaTuning {
    fn default() -> Self {
        Self {
            loading_sustained_warn_sec: DEFAULT_LOADING_SUSTAINED_WARN_SEC,
        }
    }
}

/// Severity pure — paramétré par `loading_warn_sec` (TBD M2 : lu de genome).
pub fn severity_for_stage_with_threshold(
    state: StageState,
    loading_elapsed: f64,
    loading_warn_sec: f64,
) -> &'static str {
    match state {
        StageState::Idle => "info",
        StageState::Loading if loading_elapsed > loading_warn_sec => "warn",
        StageState::Loading => "info",
        StageState::Ready => "ok",
        StageState::Error => "critical",
    }
}

/// Next-step pure — paramétré par `loading_warn_sec`.
pub fn next_step_for_stage_with_threshold(
    state: StageState,
    loading_elapsed: f64,
    loading_warn_sec: f64,
) -> &'static str {
    match state {
        StageState::Idle => {
            "No stage requested. Caller (forgia-mode-roguelite) must insert StageLoadRequest at run start."
        }
        StageState::Loading if loading_elapsed > loading_warn_sec => {
            "Stage stuck Loading > threshold. Read forgia_prefab.json + forgia2_anchor.json to identify stalled asset. Verify GLB paths in roguelite_stages.toml exist."
        }
        StageState::Loading => "Stage loading — assets fetching.",
        StageState::Ready => "Stage ready. Read forgia2_anchor.json for anchor placement details.",
        StageState::Error => {
            "Stage load failed. Read forgia2_stage.json error_message field + verify assets/genomes/roguelite_stages.toml syntax + GLB paths exist."
        }
    }
}

/// Severity pure — wrapper avec threshold default. Conservé pour tests pre-P3.
pub fn severity_for_stage(state: StageState, loading_elapsed: f64) -> &'static str {
    severity_for_stage_with_threshold(state, loading_elapsed, DEFAULT_LOADING_SUSTAINED_WARN_SEC)
}

/// Next-step pure — wrapper avec threshold default.
pub fn next_step_for_stage(state: StageState, loading_elapsed: f64) -> &'static str {
    next_step_for_stage_with_threshold(state, loading_elapsed, DEFAULT_LOADING_SUSTAINED_WARN_SEC)
}

// ─── Pure helpers : placement géométrique (testable headless) ───────────────

/// Positions + rotations des N segments de ramparts pour un hexagone régulier
/// inscrit dans le cercle rayon `extent_m`. 6 segments connectés bord-à-bord.
///
/// Retourne (position_center_segment, rotation_yaw). La position est le milieu
/// du segment, prêt pour spawn_gltf_prefab. Y = 0.0 (WALL_Y LOCK, pivot at floor).
pub fn ramparts_hex_positions(extent_m: f32) -> Vec<(Vec3, Quat)> {
    if extent_m <= 0.0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(6);
    // Vertices de l'hex : 6 points sur cercle rayon extent_m, angles 30°,90°,...
    // Segments = milieu entre vertex i et vertex (i+1)%6.
    let r = extent_m;
    let mut verts = [Vec3::ZERO; 6];
    for (i, v) in verts.iter_mut().enumerate() {
        let a = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * i as f32;
        *v = Vec3::new(r * a.cos(), 0.0, r * a.sin());
    }
    for i in 0..6 {
        let a = verts[i];
        let b = verts[(i + 1) % 6];
        let mid = (a + b) * 0.5;
        // Yaw : ALIGNE l'axe local X du mur sur la direction segment (a→b).
        // Convention KayKit GLTF : X=width (long), Y=height, Z=thickness (mince).
        // Pour que world.X = dir.normalized() après Quat::from_rotation_y(yaw) :
        //   cos(yaw)= dir.x/|dir|, -sin(yaw)= dir.z/|dir|
        //   → yaw = atan2(-dir.z, dir.x)
        // (Bug fix 2026-05-20 PM : ancien `atan2(-dir.x, dir.z)` alignait
        //  l'axe Z=thickness sur le segment → murs vus par la tranche)
        let dir = b - a;
        let yaw = (-dir.z).atan2(dir.x);
        out.push((mid, Quat::from_rotation_y(yaw)));
    }
    out
}

/// Positions tilées des murs ramparts hexagonaux — `wall_natural_len` par mur,
/// placés **côte-à-côte sans stretch** (préserve fidélité visuelle KayKit).
///
/// Pour chaque segment (6 segments hex), spawne `ceil(side / wall_len)` murs
/// à leur taille naturelle, distribués uniformément. Retourne
/// `walls_per_segment * 6` entries (position centrée sur le mur + rotation).
///
/// Bug fix story-483 P1 (2026-05-20 PM) : la version précédente stretchait 1
/// mur via `scale_x = side / 4.0` → ratio 22× sur extent=90, murs apparaissaient
/// tordus comme des planches. Cf. screenshot user 2026-05-20 PM.
pub fn ramparts_hex_tiled_positions(
    extent_m: f32,
    wall_natural_len: f32,
) -> Vec<(Vec3, Quat)> {
    if extent_m <= 0.0 || wall_natural_len <= 0.0 {
        return Vec::new();
    }
    let mut verts = [Vec3::ZERO; 6];
    for (i, v) in verts.iter_mut().enumerate() {
        let a = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * i as f32;
        *v = Vec3::new(extent_m * a.cos(), 0.0, extent_m * a.sin());
    }
    let hex_side_len = extent_m; // hex inscrit cercle rayon=extent_m → côté = extent_m
    let walls_per_segment = ((hex_side_len / wall_natural_len).ceil() as u32).max(1);
    let mut out = Vec::with_capacity((walls_per_segment * 6) as usize);
    for i in 0..6 {
        let a = verts[i];
        let b = verts[(i + 1) % 6];
        let dir = b - a;
        // Cf. `ramparts_hex_positions` : aligne X local sur dir (KayKit width=X).
        let yaw = (-dir.z).atan2(dir.x);
        let rot = Quat::from_rotation_y(yaw);
        for j in 0..walls_per_segment {
            let t = (j as f32 + 0.5) / walls_per_segment as f32;
            let pos = a + dir * t;
            out.push((pos, rot));
        }
    }
    out
}

/// Positions + rotations des **midpoints** des 6 segments (1 entry par segment).
/// Utile pour 1 collider physics par segment (cf spawn system).
pub fn ramparts_hex_segment_midpoints(extent_m: f32) -> Vec<(Vec3, Quat)> {
    ramparts_hex_positions(extent_m)
}

/// Positions des POI anchor slots sur un cercle inscrit à `radius_factor *
/// extent_m`. Distribution uniforme avec jitter déterministe (splitmix64).
///
/// Garantit zéro overlap pairwise tant que `slots <= floor(2π * radius / max_size_m)`.
/// V1 utilise `radius_factor = 0.55` pour anchors bien inside ramparts.
pub fn poi_anchor_positions(extent_m: f32, slots: u32, seed: u64) -> Vec<Vec3> {
    if slots == 0 || extent_m <= 0.0 {
        return Vec::new();
    }
    let radius = extent_m * 0.55;
    let mut out = Vec::with_capacity(slots as usize);
    let step = std::f32::consts::TAU / slots as f32;
    let mut rng = seed ^ 0xFEED_BEEF_DEAD_CAFE_u64;
    for i in 0..slots {
        let base_angle = step * i as f32;
        // Jitter ±step/6 → 30° max d'écart pour préserver l'espacement.
        let r = splitmix64(&mut rng);
        let jitter_norm = (r as f64 / u64::MAX as f64) as f32 - 0.5;
        let jitter = jitter_norm * (step / 3.0);
        let angle = base_angle + jitter;
        out.push(Vec3::new(radius * angle.cos(), 0.0, radius * angle.sin()));
    }
    out
}

// ─── Spawn system (P1 runtime) ──────────────────────────────────────────────

const RAMPARTS_WALL_HEIGHT_M: f32 = 4.0;
const RAMPARTS_WALL_THICKNESS_M: f32 = 0.4;

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Mapping `stage_def.ramparts_kit` → GLB path. Centralisé pour évolution
/// future (registry crate dédié si > 5 kits).
fn ramparts_wall_glb(kit: &str) -> &'static str {
    match kit {
        "kaykit_dungeon" => "models/kaykit/dungeon/wall.glb",
        "medieval_hexagon" => "models/kaykit/medieval_hexagon/walls/wall_straight.gltf",
        _ => "models/kaykit/dungeon/wall.glb",
    }
}

/// Couleur du sol par biome. Hardcoded UI/visual only (autorisé par
/// `.claude/rules/no-hardcode.md` exception cosmetic) — pourra migrer vers
/// biome_genome.toml en M2 si raffinage demandé.
fn biome_floor_color(biome: &str) -> Color {
    match biome {
        "Volcanic" => Color::srgb(0.30, 0.20, 0.18),
        "Plains" => Color::srgb(0.40, 0.45, 0.32),
        "Desert" => Color::srgb(0.65, 0.55, 0.38),
        "Forest" => Color::srgb(0.22, 0.32, 0.20),
        "Tundra" => Color::srgb(0.75, 0.78, 0.82),
        "Jungle" => Color::srgb(0.18, 0.30, 0.18),
        "Swamp" => Color::srgb(0.28, 0.30, 0.20),
        "Mountain" => Color::srgb(0.45, 0.45, 0.48),
        "Canyon" => Color::srgb(0.55, 0.35, 0.25),
        "Savanna" => Color::srgb(0.62, 0.55, 0.32),
        _ => Color::srgb(0.32, 0.30, 0.28),
    }
}

/// Pondère et tire un POI parmi `pool` selon les `weight` field. Pure (testable).
/// Retourne `None` si pool vide ou total_weight == 0.
fn pick_poi_weighted<'a>(
    pool: &'a [(&'a String, &'a PoiDef)],
    rng_state: &mut u64,
) -> Option<&'a PoiDef> {
    if pool.is_empty() {
        return None;
    }
    let total: u32 = pool.iter().map(|(_, p)| p.weight).sum();
    if total == 0 {
        return None;
    }
    let r = (splitmix64(rng_state) % u64::from(total)) as u32;
    let mut acc = 0u32;
    for (_, poi) in pool {
        acc += poi.weight;
        if r < acc {
            return Some(*poi);
        }
    }
    pool.last().map(|(_, p)| *p)
}

/// System core P1 — consomme `StageLoadRequest`, lit les 2 genomes, et spawne
/// terrain + ramparts hex + anchors + POIs + sun. Idempotent : skip si
/// même `stage_id` déjà chargé en Ready.
///
/// Cleanup via `cleanup_stage_arena` (caller OnExit GameMode::Roguelite).
#[allow(clippy::too_many_arguments)]
fn spawn_stage_arena_on_request(
    mut commands: Commands,
    request: Option<Res<StageLoadRequest>>,
    handles: Res<StageGenomeHandles>,
    stages_assets: Res<Assets<Genome<RogueliteStagesGenome>>>,
    pois_assets: Res<Assets<Genome<RoguelitePoisGenome>>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut result: ResMut<StageLoadResult>,
    anchor_stats: Res<AnchorStats>,
    prefab_stats: Res<PrefabStats>,
    q_existing: Query<Entity, With<StageArenaMarker>>,
    mut last_processed_id: Local<String>,
) {
    let Some(req) = request else {
        return;
    };
    if req.stage_id.is_empty() {
        return;
    }
    // Idempotent : on ne ré-spawn pas le même stage si déjà Ready.
    if *last_processed_id == req.stage_id && result.state == StageState::Ready {
        return;
    }
    // P2 — Stage transition detection : nouveau stage_id différent du dernier
    // spawn → cleanup old entities AVANT spawn new. Permet le run loop multi-stage.
    if !last_processed_id.is_empty()
        && *last_processed_id != req.stage_id
        && !q_existing.is_empty()
    {
        let prev = std::mem::take(&mut *last_processed_id);
        let n = despawn_stage_entities(&mut commands, &q_existing, &anchor_stats, &mut result);
        info!(
            "[stage-arena] Stage transition '{}' → '{}' : despawned {} entities",
            prev, req.stage_id, n
        );
        // Next frame re-entre dans le system avec last_processed_id vide → spawn new.
        return;
    }

    // Wait for both genomes loaded.
    let Some(stages_genome) = stages_assets.get(&handles.stages) else {
        if result.state != StageState::Loading {
            result.state = StageState::Loading;
            result.loading_started_secs = now_secs();
            result.stage_id = req.stage_id.clone();
        }
        return;
    };
    let Some(pois_genome) = pois_assets.get(&handles.pois) else {
        if result.state != StageState::Loading {
            result.state = StageState::Loading;
            result.loading_started_secs = now_secs();
        }
        return;
    };

    let Some(stage_def) = stages_genome.data.stages.get(&req.stage_id) else {
        result.state = StageState::Error;
        result.error_message = format!(
            "Stage '{}' not found in roguelite_stages.toml",
            req.stage_id
        );
        return;
    };

    info!(
        "[stage-arena] Spawning stage '{}' biome={} extent={}m anchors={}",
        req.stage_id, stage_def.biome, stage_def.arena_extent_m, stage_def.anchor_slots
    );

    let extent = stage_def.arena_extent_m;
    let mut props_spawned: u32 = 0;

    // 1. Floor — primitive Bevy + collider Rapier, biome-colored.
    let floor_color = biome_floor_color(&stage_def.biome);
    let floor_mat = materials.add(StandardMaterial {
        base_color: floor_color,
        perceptual_roughness: 0.92,
        ..default()
    });
    commands.spawn((
        Name::new(format!("StageFloor_{}", req.stage_id)),
        StageArenaMarker,
        Mesh3d(meshes.add(Plane3d::default().mesh().size(extent * 2.0, extent * 2.0))),
        MeshMaterial3d(floor_mat),
        Transform::IDENTITY,
        RigidBody::Fixed,
        Collider::cuboid(extent, 0.1, extent),
    ));
    props_spawned += 1;

    // 2. Ramparts — 6 segments hexagonaux, walls **tilés côte-à-côte sans stretch**
    //    (fix story-483 P1 2026-05-20 PM, screenshot user → murs étirés tordus).
    //    Visual : N walls par segment à taille naturelle (preserve KayKit fidelity).
    //    Physics : 1 collider cuboid par segment (6 colliders au total, pas 6*N).
    let wall_glb = ramparts_wall_glb(&stage_def.ramparts_kit);
    // Wall natural length per kit : TOML override > smart default par-kit.
    let wall_len = match stage_def.wall_natural_len_m {
        Some(v) if v > 0.05 => v,
        _ => wall_natural_len_for_kit(&stage_def.ramparts_kit),
    };
    let hex_side_len = extent;
    // 1 collider per segment (midpoint + rotation).
    for (i, (mid, rot)) in ramparts_hex_segment_midpoints(extent).into_iter().enumerate() {
        commands.spawn((
            Name::new(format!("RampartCollider_{i}")),
            StageArenaMarker,
            Transform {
                translation: mid + Vec3::new(0.0, RAMPARTS_WALL_HEIGHT_M * 0.5, 0.0),
                rotation: rot,
                scale: Vec3::ONE,
            },
            GlobalTransform::default(),
            RigidBody::Fixed,
            Collider::cuboid(
                hex_side_len * 0.5,
                RAMPARTS_WALL_HEIGHT_M * 0.5,
                RAMPARTS_WALL_THICKNESS_M * 0.5,
            ),
        ));
    }
    // N walls tilés par segment (visual seulement, pas de collider — physics
    // déjà couverte par les 6 colliders ci-dessus).
    let tiled = ramparts_hex_tiled_positions(extent, wall_len);
    // Walls = geometry visuelle pure (pas des anchors). Le count est porté par
    // `props_spawned` + `forgia_prefab.json::total_spawned`. Évite la
    // sur-fonction sémantique de `AnchorKind::Landmark` (cf qa-lead BUG-483-02).
    for (idx, (pos, rot)) in tiled.iter().enumerate() {
        let spawn = PrefabSpawn::new(wall_glb, *pos)
            .with_rotation(*rot)
            .with_name(format!("RampartTile_{idx}"));
        let _e = spawn_gltf_prefab(
            &mut commands,
            &asset_server,
            &prefab_stats,
            spawn,
            (StageArenaMarker,),
        );
        props_spawned += 1;
    }

    // 3. PlayerSpawn anchor (just a marker entity, no visual).
    commands.spawn((
        Name::new("StagePlayerSpawn"),
        StageArenaMarker,
        AnchorPoint::new(AnchorKind::PlayerSpawn, 0),
        Transform::from_xyz(0.0, 0.0, 0.0),
        GlobalTransform::default(),
    ));
    anchor_stats.record(AnchorKind::PlayerSpawn);

    // 4. POI anchors + prefabs (pondéré).
    let poi_positions = poi_anchor_positions(extent, stage_def.anchor_slots, req.seed);
    let pois_pool: Vec<(&String, &PoiDef)> = pois_genome
        .data
        .pois
        .iter()
        .filter(|(_, p)| p.weight > 0)
        .collect();
    let mut rng_state = req.seed.wrapping_mul(0xDEAD_BEEF_C0DE_F00D);
    for (slot, pos) in poi_positions.iter().enumerate() {
        let Some(poi) = pick_poi_weighted(&pois_pool, &mut rng_state) else {
            break;
        };
        let spawn = PrefabSpawn::new(&poi.prefab, *pos)
            .with_name(format!("POI_{slot}"));
        let _e = spawn_gltf_prefab(&mut commands, &asset_server, &prefab_stats, spawn, (StageArenaMarker,));
        commands.spawn((
            Name::new(format!("StagePoiAnchor_{slot}")),
            StageArenaMarker,
            AnchorPoint::new(AnchorKind::PoiSlot, slot as u32),
            Transform::from_translation(*pos),
            GlobalTransform::default(),
        ));
        anchor_stats.record(AnchorKind::PoiSlot);
        props_spawned += 1;
    }

    // 5. BossPad if required — pick the POI tagged encounter="boss".
    if stage_def.boss_pad_required {
        if let Some(boss) = pois_genome
            .data
            .pois
            .values()
            .find(|p| p.encounter == "boss")
        {
            let boss_pos = Vec3::new(0.0, 0.0, -extent * 0.7);
            let spawn = PrefabSpawn::new(&boss.prefab, boss_pos)
                .with_scale((boss.size_m / 4.0).max(1.0))
                .with_name("BossPad");
            let _e = spawn_gltf_prefab(&mut commands, &asset_server, &prefab_stats, spawn, (StageArenaMarker,));
            commands.spawn((
                Name::new("StageBossPadAnchor"),
                StageArenaMarker,
                AnchorPoint::new(AnchorKind::BossPad, 0),
                Transform::from_translation(boss_pos),
                GlobalTransform::default(),
            ));
            anchor_stats.record(AnchorKind::BossPad);
            props_spawned += 1;
        }
    }

    // 6. Sun — directional light biome-tuned later (P2). Default cool sky.
    commands.spawn((
        Name::new("StageSun"),
        StageArenaMarker,
        DirectionalLight {
            illuminance: 12_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(50.0, 100.0, 50.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Finalize result.
    result.state = StageState::Ready;
    result.stage_id = req.stage_id.clone();
    result.biome = stage_def.biome.clone();
    result.extent_m = extent;
    result.anchors_placed = anchor_stats.total();
    result.props_spawned = props_spawned;
    result.error_message.clear();
    result.wall_natural_len_used = wall_len;
    result.walls_per_segment = ((hex_side_len / wall_len).ceil() as u32).max(1);
    result.music_state_id = stage_def.music_state.clone().unwrap_or_default();
    result.weather_override = stage_def.weather_override.clone().unwrap_or_default();
    *last_processed_id = req.stage_id.clone();
    info!(
        "[stage-arena] Stage '{}' READY: {} props, {} anchors, walls/segment={} wall_len={:.2}m",
        req.stage_id,
        props_spawned,
        anchor_stats.total(),
        result.walls_per_segment,
        wall_len,
    );
}

/// Despawn helper inline — réutilisé par le system wrapper `cleanup_stage_arena`
/// ET par `spawn_stage_arena_on_request` lors d'une transition stage_id→stage_id.
/// Retourne le nombre d'entités despawn.
pub fn despawn_stage_entities(
    commands: &mut Commands,
    query: &Query<Entity, With<StageArenaMarker>>,
    stats: &AnchorStats,
    result: &mut StageLoadResult,
) -> u32 {
    let mut n = 0u32;
    for e in query.iter() {
        commands.entity(e).despawn();
        n += 1;
    }
    forgia_anchor::reset_anchor_stats(stats);
    *result = StageLoadResult::default();
    n
}

/// Cleanup — caller calls this OnExit(GameMode::Roguelite) ou avant
/// transition stage→stage. Despawn toutes les entités StageArenaMarker + reset
/// AnchorStats + reset StageLoadResult.
pub fn cleanup_stage_arena(
    mut commands: Commands,
    q: Query<Entity, With<StageArenaMarker>>,
    stats: Res<AnchorStats>,
    mut result: ResMut<StageLoadResult>,
) {
    let n = despawn_stage_entities(&mut commands, &q, &stats, &mut result);
    if n > 0 {
        info!("[stage-arena] cleanup: despawned {n} stage entities");
    }
}

// ─── Tests purs ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_as_str_round_trip() {
        assert_eq!(StageState::Idle.as_str(), "idle");
        assert_eq!(StageState::Loading.as_str(), "loading");
        assert_eq!(StageState::Ready.as_str(), "ready");
        assert_eq!(StageState::Error.as_str(), "error");
    }

    #[test]
    fn severity_idle_is_info() {
        assert_eq!(severity_for_stage(StageState::Idle, 0.0), "info");
    }

    #[test]
    fn severity_loading_short_is_info() {
        assert_eq!(severity_for_stage(StageState::Loading, 2.0), "info");
    }

    #[test]
    fn severity_loading_sustained_is_warn() {
        assert_eq!(severity_for_stage(StageState::Loading, 6.0), "warn");
    }

    #[test]
    fn severity_ready_is_ok() {
        assert_eq!(severity_for_stage(StageState::Ready, 0.0), "ok");
    }

    #[test]
    fn severity_error_is_critical() {
        assert_eq!(severity_for_stage(StageState::Error, 0.0), "critical");
    }

    #[test]
    fn next_step_idle_points_to_request_action() {
        let s = next_step_for_stage(StageState::Idle, 0.0);
        assert!(s.contains("StageLoadRequest"));
    }

    #[test]
    fn next_step_loading_sustained_points_to_prefab_sensor() {
        let s = next_step_for_stage(StageState::Loading, 7.0);
        assert!(s.contains("forgia_prefab.json"));
        assert!(s.contains("roguelite_stages.toml"));
    }

    #[test]
    fn next_step_error_points_to_error_message() {
        let s = next_step_for_stage(StageState::Error, 0.0);
        assert!(s.contains("error_message"));
    }

    #[test]
    fn ramparts_hex_yields_6_segments() {
        let segs = ramparts_hex_positions(50.0);
        assert_eq!(segs.len(), 6);
    }

    #[test]
    fn ramparts_hex_zero_extent_yields_empty() {
        assert!(ramparts_hex_positions(0.0).is_empty());
        assert!(ramparts_hex_positions(-10.0).is_empty());
    }

    #[test]
    fn ramparts_tiled_zero_yields_empty() {
        assert!(ramparts_hex_tiled_positions(0.0, 4.0).is_empty());
        assert!(ramparts_hex_tiled_positions(50.0, 0.0).is_empty());
        assert!(ramparts_hex_tiled_positions(-10.0, 4.0).is_empty());
    }

    #[test]
    fn ramparts_tiled_count_matches_walls_per_segment() {
        // extent=8, wall_len=4 → 2 walls/segment × 6 = 12 total
        let pts = ramparts_hex_tiled_positions(8.0, 4.0);
        assert_eq!(pts.len(), 12);

        // extent=12, wall_len=4 → 3 walls/segment × 6 = 18 total
        let pts = ramparts_hex_tiled_positions(12.0, 4.0);
        assert_eq!(pts.len(), 18);

        // extent=4.1, wall_len=4 → ceil(1.025) = 2 walls/segment × 6 = 12 total
        let pts = ramparts_hex_tiled_positions(4.1, 4.0);
        assert_eq!(pts.len(), 12);
    }

    #[test]
    fn ramparts_tiled_walls_within_extent_radius() {
        // Tous les murs doivent être à distance ≤ extent du center (sur les côtés de l'hex).
        let extent = 30.0;
        let pts = ramparts_hex_tiled_positions(extent, 4.0);
        assert!(!pts.is_empty());
        for (p, _) in &pts {
            let d = (p.x * p.x + p.z * p.z).sqrt();
            // Apothème hex = extent * sqrt(3)/2 ≈ extent * 0.866 (min dist sur segments)
            // Vertex hex = extent (max dist sur segments)
            assert!(
                d <= extent * 1.001,
                "tile {p:?} d={d} exceeds extent {extent}"
            );
            assert!(p.y.abs() < 1e-6, "y not zero: {}", p.y);
        }
    }

    #[test]
    fn ramparts_yaw_aligns_x_axis_with_segment() {
        // Garde-fou contre la régression "murs vus par la tranche" :
        // l'axe X local du mur DOIT pointer le long du segment.
        // Convention KayKit GLTF : X=width (long axis du wall.glb).
        let pts = ramparts_hex_tiled_positions(60.0, 4.0);
        assert!(pts.len() >= 6);
        // Pour les 6 premiers walls (1er de chaque segment) :
        // - rotation appliquée à Vec3::X doit pointer dans la direction segment
        let extent = 60.0;
        let mut verts = [Vec3::ZERO; 6];
        for (i, v) in verts.iter_mut().enumerate() {
            let a = std::f32::consts::FRAC_PI_6 + std::f32::consts::FRAC_PI_3 * i as f32;
            *v = Vec3::new(extent * a.cos(), 0.0, extent * a.sin());
        }
        let walls_per_seg = pts.len() / 6;
        for seg in 0..6 {
            let dir = (verts[(seg + 1) % 6] - verts[seg]).normalize();
            let (_pos, rot) = pts[seg * walls_per_seg];
            let x_world = rot * Vec3::X;
            // Dot product → 1.0 si même direction.
            let dot = x_world.dot(dir);
            assert!(
                dot > 0.99,
                "segment {seg}: X local doit aligner sur segment direction, dot={dot}"
            );
        }
    }

    #[test]
    fn ramparts_yaw_consistent_between_helpers() {
        // ramparts_hex_positions (midpoints, used for colliders) doit avoir
        // la même rotation que ramparts_hex_tiled_positions (visual tiles).
        let mids = ramparts_hex_positions(50.0);
        let tiles = ramparts_hex_tiled_positions(50.0, 4.0);
        let walls_per_seg = tiles.len() / 6;
        for seg in 0..6 {
            let mid_rot = mids[seg].1;
            let tile_rot = tiles[seg * walls_per_seg].1;
            let diff = (mid_rot * tile_rot.inverse()).to_euler(EulerRot::YXZ).0;
            assert!(
                diff.abs() < 1e-3,
                "segment {seg}: midpoint rotation != tile rotation, diff={diff}"
            );
        }
    }

    #[test]
    fn ramparts_tiled_walls_distributed_per_segment() {
        // Sur extent=8 wall_len=4 → 2 walls/segment, ils doivent être à t=0.25 et t=0.75
        // le long de chaque segment (centrés sur leurs slots).
        let pts = ramparts_hex_tiled_positions(8.0, 4.0);
        // Walls 0 et 1 sont sur le segment 0. Leur distance pairwise = wall_len.
        let d01 = pts[0].0.distance(pts[1].0);
        assert!(
            (d01 - 4.0).abs() < 1e-3,
            "walls dans même segment doivent être à distance wall_len={}, mesuré {}",
            4.0,
            d01
        );
    }

    #[test]
    fn ramparts_hex_segments_form_closed_polygon() {
        let segs = ramparts_hex_positions(30.0);
        // Centres tous à même distance du center
        let r0 = (segs[0].0.x.powi(2) + segs[0].0.z.powi(2)).sqrt();
        for (p, _) in &segs {
            let r = (p.x.powi(2) + p.z.powi(2)).sqrt();
            assert!((r - r0).abs() < 1e-3, "segment off-radius: {r} vs {r0}");
        }
        // Y = 0 partout (WALL_Y LOCK)
        for (p, _) in &segs {
            assert!(p.y.abs() < 1e-6, "y not zero: {}", p.y);
        }
    }

    #[test]
    fn poi_anchors_zero_slots_yields_empty() {
        assert!(poi_anchor_positions(50.0, 0, 42).is_empty());
    }

    #[test]
    fn poi_anchors_count_matches_slots() {
        let pts = poi_anchor_positions(50.0, 6, 42);
        assert_eq!(pts.len(), 6);
    }

    #[test]
    fn poi_anchors_deterministic_per_seed() {
        let a = poi_anchor_positions(50.0, 6, 42);
        let b = poi_anchor_positions(50.0, 6, 42);
        assert_eq!(a, b, "same seed must yield same layout");
    }

    #[test]
    fn poi_anchors_differ_with_seed() {
        let a = poi_anchor_positions(50.0, 6, 42);
        let b = poi_anchor_positions(50.0, 6, 99);
        assert_ne!(a, b, "different seed should yield different layout");
    }

    #[test]
    fn poi_anchors_within_radius() {
        let pts = poi_anchor_positions(100.0, 8, 7);
        for p in &pts {
            let r = (p.x.powi(2) + p.z.powi(2)).sqrt();
            assert!(r <= 55.5, "POI {p:?} outside radius_factor*extent");
        }
    }

    #[test]
    fn splitmix64_deterministic() {
        let mut a = 42u64;
        let mut b = 42u64;
        for _ in 0..10 {
            assert_eq!(splitmix64(&mut a), splitmix64(&mut b));
        }
    }

    #[test]
    fn ramparts_shape_default_is_hexagonal() {
        assert_eq!(RampartsShape::default(), RampartsShape::Hexagonal);
    }

    #[test]
    fn stage_def_toml_parse_minimal() {
        let toml_str = r#"
            biome = "Volcanic"
            arena_extent_m = 90.0
            ramparts_kit = "kaykit_dungeon"
            anchor_slots = 6
            boss_pad_required = true
        "#;
        let def: StageDef = toml::from_str(toml_str).expect("parse StageDef");
        assert_eq!(def.biome, "Volcanic");
        assert_eq!(def.arena_extent_m, 90.0);
        assert_eq!(def.anchor_slots, 6);
        assert!(def.boss_pad_required);
        assert_eq!(def.ramparts_shape, RampartsShape::Hexagonal);
        assert!(def.music_state.is_none());
    }

    #[test]
    fn stages_genome_toml_parse_multi() {
        let toml_str = r#"
            [stages.crypts_of_anvil]
            biome = "Volcanic"
            arena_extent_m = 90.0
            ramparts_kit = "kaykit_dungeon"
            anchor_slots = 6
            boss_pad_required = true

            [stages.forge_sanctum]
            biome = "Plains"
            arena_extent_m = 80.0
            ramparts_kit = "medieval_hexagon"
            anchor_slots = 5
            boss_pad_required = false
        "#;
        let g: RogueliteStagesGenome = toml::from_str(toml_str).expect("parse genome");
        assert_eq!(g.stages.len(), 2);
        assert!(g.stages.contains_key("crypts_of_anvil"));
        assert!(g.stages.contains_key("forge_sanctum"));
    }

    #[test]
    fn poi_def_toml_parse() {
        let toml_str = r#"
            weight = 50
            prefab = "models/kaykit/dungeon/chest_basic.glb"
            size_m = 4.0
        "#;
        let p: PoiDef = toml::from_str(toml_str).expect("parse PoiDef");
        assert_eq!(p.weight, 50);
        assert_eq!(p.encounter, "none"); // default
        assert_eq!(p.size_m, 4.0);
    }
}
