//! # forgia-streaming
//!
//! Chunk streaming foundation — industry-grade pattern composite Minecraft +
//! UE5 World Partition + Roblox StreamingEnabled + Unity Addressables.
//!
//! ## Vue d'ensemble (story-450 wave 1)
//!
//! Cette crate fournit la **couche config + observabilité** pour le système de
//! streaming chunks de Forgia. Les enforcers (ChunkManager budget eviction,
//! StreamingPause player gate) viendront wave 2-4 mais consommeront ces
//! Resources déjà disponibles.
//!
//! Pattern industrie validé sources :
//! - **Dual radii** (Minecraft `view_radius` ≠ `simulation_radius`) →
//!   `StreamingRadii { simulation_m, view_m, unload_m }`
//! - **Unload hysteresis** (UE5 Level Streaming default 2.0s) →
//!   `UnloadHysteresis { min_residence_secs }`
//! - **Memory budget LRU** (Unity Addressables `memoryBudgetKB`) →
//!   `MemoryBudget { max_mb, max_chunks }`
//! - **StreamingPause** (Roblox `StreamingPauseMode`) →
//!   `StreamingPause { active, reason }`
//! - **Async metrics** (Bevy `AsyncComputeTaskPool` pattern) →
//!   `StreamingStats { gen_ms_p50, gen_ms_p99, async_queue_depth }`

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::Path;
use thiserror::Error;

pub mod prelude {
    pub use crate::{
        EvictionEvent, EvictionReason, FoliageCoverageReport, ForgiaStreamingPlugin,
        GenMsHistogram, MemoryBudget, StreamingConfig, StreamingPause, StreamingRadii,
        StreamingStats, UnloadHysteresis,
    };
}

// ── Genome schema ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamingGenome {
    pub schema_version: u32,
    pub radii: RadiiGenome,
    pub hysteresis: HysteresisGenome,
    pub budget: BudgetGenome,
    pub async_pipeline: AsyncPipelineGenome,
    pub debug: DebugGenome,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RadiiGenome {
    pub simulation_m: f32,
    pub view_m: f32,
    pub unload_m: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HysteresisGenome {
    pub min_residence_secs: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetGenome {
    pub max_mb: f32,
    pub max_chunks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AsyncPipelineGenome {
    pub max_queue_depth: u32,
    pub chunks_per_frame: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DebugGenome {
    pub sensor_enabled: u32,
    pub sensor_interval_s: f32,
    pub gen_ms_buckets: u32,
    #[serde(default = "default_foliage_coverage_warn_threshold")]
    pub foliage_coverage_warn_threshold: u32,
    #[serde(default = "default_foliage_coverage_sustained_s")]
    pub foliage_coverage_sustained_s: f32,
}

fn default_foliage_coverage_warn_threshold() -> u32 {
    4
}
fn default_foliage_coverage_sustained_s() -> f32 {
    3.0
}

impl Default for StreamingGenome {
    fn default() -> Self {
        // Fallback graceful pattern Forgia (cf ArenaBotsGenome). Si TOML genome
        // pas encore chargé, on tourne avec ces valeurs sourcées industrie.
        Self {
            schema_version: 1,
            radii: RadiiGenome {
                simulation_m: 64.0,
                view_m: 96.0,
                unload_m: 128.0,
            },
            hysteresis: HysteresisGenome {
                min_residence_secs: 2.0,
            },
            budget: BudgetGenome {
                max_mb: 512.0,
                max_chunks: 256,
            },
            async_pipeline: AsyncPipelineGenome {
                max_queue_depth: 8,
                chunks_per_frame: 2,
            },
            debug: DebugGenome {
                sensor_enabled: 1,
                sensor_interval_s: 1.0,
                gen_ms_buckets: 16,
                foliage_coverage_warn_threshold: default_foliage_coverage_warn_threshold(),
                foliage_coverage_sustained_s: default_foliage_coverage_sustained_s(),
            },
        }
    }
}

#[derive(Debug, Error)]
pub enum StreamingGenomeError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: Box<std::io::Error>,
    },
    #[error("failed to parse TOML at {path}: {source}")]
    Parse {
        path: String,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("schema version {found} not supported (expected {expected})")]
    SchemaVersion { found: u32, expected: u32 },
    #[error("invariant violation: {0}")]
    Invariant(String),
}

impl StreamingGenome {
    pub const SCHEMA_VERSION: u32 = 1;

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, StreamingGenomeError> {
        let p = path.as_ref();
        let raw = std::fs::read_to_string(p).map_err(|e| StreamingGenomeError::Io {
            path: p.display().to_string(),
            source: Box::new(e),
        })?;
        let g: Self = toml::from_str(&raw).map_err(|e| StreamingGenomeError::Parse {
            path: p.display().to_string(),
            source: Box::new(e),
        })?;
        g.validate()?;
        Ok(g)
    }

    pub fn parse(s: &str) -> Result<Self, StreamingGenomeError> {
        let g: Self = toml::from_str(s).map_err(|e| StreamingGenomeError::Parse {
            path: "<string>".into(),
            source: Box::new(e),
        })?;
        g.validate()?;
        Ok(g)
    }

    pub fn validate(&self) -> Result<(), StreamingGenomeError> {
        if self.schema_version != Self::SCHEMA_VERSION {
            return Err(StreamingGenomeError::SchemaVersion {
                found: self.schema_version,
                expected: Self::SCHEMA_VERSION,
            });
        }
        let r = &self.radii;
        if !(r.simulation_m > 0.0 && r.view_m > 0.0 && r.unload_m > 0.0) {
            return Err(StreamingGenomeError::Invariant(
                "radii must be positive".into(),
            ));
        }
        if r.simulation_m > r.view_m {
            return Err(StreamingGenomeError::Invariant(format!(
                "simulation_m ({}) must be <= view_m ({})",
                r.simulation_m, r.view_m
            )));
        }
        if r.view_m > r.unload_m {
            return Err(StreamingGenomeError::Invariant(format!(
                "view_m ({}) must be <= unload_m ({}) for hysteresis",
                r.view_m, r.unload_m
            )));
        }
        if self.hysteresis.min_residence_secs < 0.0 {
            return Err(StreamingGenomeError::Invariant(
                "min_residence_secs must be >= 0".into(),
            ));
        }
        if self.budget.max_mb <= 0.0 || self.budget.max_chunks == 0 {
            return Err(StreamingGenomeError::Invariant(
                "budget max_mb and max_chunks must be > 0".into(),
            ));
        }
        Ok(())
    }
}

// ── Runtime Resources ───────────────────────────────────────────────────────

#[derive(Resource, Debug, Clone)]
pub struct StreamingConfig {
    pub radii: StreamingRadii,
    pub hysteresis: UnloadHysteresis,
    pub budget: MemoryBudget,
    pub async_pipeline: AsyncPipeline,
    pub debug: DebugConfig,
    pub source: ConfigSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    Default,
    TomlGenome,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamingRadii {
    pub simulation_m: f32,
    pub view_m: f32,
    pub unload_m: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct UnloadHysteresis {
    pub min_residence_secs: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryBudget {
    pub max_mb: f32,
    pub max_chunks: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct AsyncPipeline {
    pub max_queue_depth: u32,
    pub chunks_per_frame: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct DebugConfig {
    pub sensor_enabled: bool,
    pub sensor_interval_s: f32,
    pub gen_ms_buckets: u32,
    pub foliage_coverage_warn_threshold: u32,
    pub foliage_coverage_sustained_s: f32,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self::from_genome(&StreamingGenome::default(), ConfigSource::Default)
    }
}

impl StreamingConfig {
    pub fn from_genome(g: &StreamingGenome, source: ConfigSource) -> Self {
        Self {
            radii: StreamingRadii {
                simulation_m: g.radii.simulation_m,
                view_m: g.radii.view_m,
                unload_m: g.radii.unload_m,
            },
            hysteresis: UnloadHysteresis {
                min_residence_secs: g.hysteresis.min_residence_secs,
            },
            budget: MemoryBudget {
                max_mb: g.budget.max_mb,
                max_chunks: g.budget.max_chunks,
            },
            async_pipeline: AsyncPipeline {
                max_queue_depth: g.async_pipeline.max_queue_depth,
                chunks_per_frame: g.async_pipeline.chunks_per_frame,
            },
            debug: DebugConfig {
                sensor_enabled: g.debug.sensor_enabled != 0,
                sensor_interval_s: g.debug.sensor_interval_s.max(0.1),
                gen_ms_buckets: g.debug.gen_ms_buckets.max(4),
                foliage_coverage_warn_threshold: g.debug.foliage_coverage_warn_threshold,
                foliage_coverage_sustained_s: g.debug.foliage_coverage_sustained_s.max(0.0),
            },
            source,
        }
    }
}

// ── StreamingStats Resource (sensor data) ───────────────────────────────────

#[derive(Resource, Debug, Default)]
pub struct StreamingStats {
    pub loaded_count: u32,
    pub loaded_mb_est: f32,
    pub pending_load_count: u32,
    pub pending_gen_count: u32,
    /// PLACEHOLDER : la génération de chunks est SYNCHRONE (main thread, cf
    /// forgia-rpg::stream_chunks_around_player) — aucun AsyncComputeTaskPool
    /// n'est câblé, donc ce champ reste TOUJOURS 0. Le sensor le signale via
    /// `"mode": "synchronous"` (audit 2026-06-05). À alimenter quand le pipeline
    /// async sera implémenté (P2).
    pub async_queue_depth: u32,
    pub lod0_count: u32,
    pub lod1_count: u32,
    pub lod2_count: u32,
    pub hysteresis_blocked_unloads: u32,
    pub evictions_10s: EvictionCounters,
    pub gen_ms_hist: GenMsHistogram,
    pub recent_evictions: VecDeque<EvictionEvent>,
    pub last_window_reset_secs: f32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EvictionCounters {
    pub distance: u32,
    pub budget: u32,
    pub lod_demotion: u32,
    pub manual: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum EvictionReason {
    Distance,
    Budget,
    LodDemotion,
    Manual,
}

#[derive(Debug, Clone)]
pub struct EvictionEvent {
    pub reason: EvictionReason,
    pub coord: [i32; 2],
    pub timestamp_secs: f32,
    pub distance_m: f32,
}

const RECENT_EVICTIONS_CAP: usize = 32;

impl StreamingStats {
    pub fn record_eviction(&mut self, ev: EvictionEvent) {
        match ev.reason {
            EvictionReason::Distance => self.evictions_10s.distance += 1,
            EvictionReason::Budget => self.evictions_10s.budget += 1,
            EvictionReason::LodDemotion => self.evictions_10s.lod_demotion += 1,
            EvictionReason::Manual => self.evictions_10s.manual += 1,
        }
        if self.recent_evictions.len() >= RECENT_EVICTIONS_CAP {
            self.recent_evictions.pop_front();
        }
        self.recent_evictions.push_back(ev);
    }

    pub fn record_gen_ms(&mut self, ms: f32) {
        self.gen_ms_hist.record(ms);
    }

    pub fn tick_window(&mut self, now_secs: f32) {
        if now_secs - self.last_window_reset_secs >= 10.0 {
            self.evictions_10s = EvictionCounters::default();
            self.last_window_reset_secs = now_secs;
        }
    }

    pub fn total_evictions_10s(&self) -> u32 {
        let e = &self.evictions_10s;
        e.distance + e.budget + e.lod_demotion + e.manual
    }
}

/// Histogram log2-scale pour gen_ms timings. 16 buckets : 0-1, 1-2, 2-4, ...
/// 32768+ms. Pattern Prometheus histogram_quantile mais embedded no-alloc.
#[derive(Debug, Default, Clone)]
pub struct GenMsHistogram {
    pub buckets: [u32; 16],
    pub sample_count: u64,
    pub sum_ms: f64,
    pub max_ms: f32,
}

impl GenMsHistogram {
    pub fn record(&mut self, ms: f32) {
        if !ms.is_finite() || ms < 0.0 {
            return;
        }
        let bucket = if ms < 1.0 {
            0
        } else {
            (ms.log2().floor() as usize).min(15)
        };
        self.buckets[bucket] = self.buckets[bucket].saturating_add(1);
        self.sample_count = self.sample_count.saturating_add(1);
        self.sum_ms += f64::from(ms);
        if ms > self.max_ms {
            self.max_ms = ms;
        }
    }

    pub fn mean_ms(&self) -> f32 {
        if self.sample_count == 0 {
            return 0.0;
        }
        (self.sum_ms / self.sample_count as f64) as f32
    }

    pub fn percentile_ms(&self, p: f32) -> f32 {
        if self.sample_count == 0 {
            return 0.0;
        }
        let target = (self.sample_count as f32 * p.clamp(0.0, 1.0)) as u64;
        let mut cumul: u64 = 0;
        for (i, &c) in self.buckets.iter().enumerate() {
            cumul = cumul.saturating_add(u64::from(c));
            if cumul >= target {
                return if i == 0 { 1.0 } else { 2f32.powi(i as i32) };
            }
        }
        self.max_ms
    }
}

// ── StreamingPause Resource (Roblox-style) ──────────────────────────────────

#[derive(Resource, Debug, Default)]
pub struct StreamingPause {
    pub active: bool,
    pub reason: String,
    pub waiting_chunks: u32,
}

// ── FoliageCoverageReport Resource (public, populé par producteurs externes) ──
//
// Cross-check chunks chargés vs foliage placée. Couvre G2+G3 audit V2 2026-05-21
// (« veg charge mal »). Default vide → innocuous tant que producteur (story-502-B
// côté forgia-foliage) pas wiré. Lecture seule côté `forgia-streaming`.

#[derive(Resource, Debug, Default, Clone, Copy)]
pub struct FoliageCoverageReport {
    pub chunks_loaded: u32,
    pub chunks_with_veg: u32,
}

impl FoliageCoverageReport {
    pub fn without_veg(&self) -> u32 {
        self.chunks_loaded.saturating_sub(self.chunks_with_veg)
    }
}

#[derive(Resource, Debug, Default)]
struct FoliageCoverageState {
    sustained_s: f32,
}

// ── Sensor system ───────────────────────────────────────────────────────────

const SENSOR_PATH: &str = "forgia_chunk_stream.json";
const HEALTH_PATH: &str = "forgia_chunk_stream_health.json";
const STREAMING_GENOME_PATH: &str = "config/genomes/streaming.toml";

#[derive(Resource, Default)]
struct SensorTimer {
    accum_s: f32,
}

fn load_streaming_genome(mut commands: Commands) {
    let cfg = match StreamingGenome::load_from_path(STREAMING_GENOME_PATH) {
        Ok(g) => {
            info!(
                "[forgia-streaming] loaded {} (sim={:.0}m view={:.0}m unload={:.0}m budget={:.0}MB)",
                STREAMING_GENOME_PATH,
                g.radii.simulation_m,
                g.radii.view_m,
                g.radii.unload_m,
                g.budget.max_mb
            );
            StreamingConfig::from_genome(&g, ConfigSource::TomlGenome)
        }
        Err(e) => {
            warn!(
                "[forgia-streaming] no genome at {} ({}), using defaults",
                STREAMING_GENOME_PATH, e
            );
            StreamingConfig::default()
        }
    };
    commands.insert_resource(cfg);
}

fn write_chunk_stream_sensor(
    time: Res<Time>,
    cfg: Option<Res<StreamingConfig>>,
    mut stats: ResMut<StreamingStats>,
    pause: Res<StreamingPause>,
    coverage: Res<FoliageCoverageReport>,
    mut coverage_state: ResMut<FoliageCoverageState>,
    mut timer: ResMut<SensorTimer>,
) {
    let Some(cfg) = cfg else { return };
    if !cfg.debug.sensor_enabled {
        return;
    }
    timer.accum_s += time.delta_secs();
    if timer.accum_s < cfg.debug.sensor_interval_s {
        return;
    }
    let dt = timer.accum_s;
    timer.accum_s = 0.0;
    let now = time.elapsed_secs();
    stats.tick_window(now);

    let cov = *coverage;
    if cov.without_veg() > cfg.debug.foliage_coverage_warn_threshold {
        coverage_state.sustained_s += dt;
    } else {
        coverage_state.sustained_s = 0.0;
    }

    let severity = compute_severity(&cfg, &stats, &pause, &cov, coverage_state.sustained_s);
    let json = build_sensor_json(
        now,
        &cfg,
        &stats,
        &pause,
        &cov,
        coverage_state.sustained_s,
        severity,
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);

    if severity == "ok" {
        let _ = std::fs::remove_file(HEALTH_PATH);
    } else {
        let health = build_health_json(
            now,
            &cfg,
            &stats,
            &pause,
            &cov,
            coverage_state.sustained_s,
            severity,
        );
        let _ = forgia_core::sensor_io::enqueue(HEALTH_PATH, health);
    }
}

fn compute_severity(
    cfg: &StreamingConfig,
    stats: &StreamingStats,
    pause: &StreamingPause,
    coverage: &FoliageCoverageReport,
    coverage_sustained_s: f32,
) -> &'static str {
    if pause.active && stats.pending_load_count > 0 {
        return "info";
    }
    if stats.loaded_mb_est > cfg.budget.max_mb {
        return "warning";
    }
    if stats.async_queue_depth >= cfg.async_pipeline.max_queue_depth {
        return "warning";
    }
    if stats.gen_ms_hist.percentile_ms(0.99) > 50.0 && stats.gen_ms_hist.sample_count > 10 {
        return "warning";
    }
    if coverage.without_veg() > cfg.debug.foliage_coverage_warn_threshold
        && coverage_sustained_s >= cfg.debug.foliage_coverage_sustained_s
    {
        return "warning";
    }
    "ok"
}

fn build_sensor_json(
    now: f32,
    cfg: &StreamingConfig,
    stats: &StreamingStats,
    pause: &StreamingPause,
    coverage: &FoliageCoverageReport,
    coverage_sustained_s: f32,
    severity: &str,
) -> String {
    let h = &stats.gen_ms_hist;
    let buckets_json: String = h
        .buckets
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let recent_json: String = stats
        .recent_evictions
        .iter()
        .map(|e| {
            format!(
                r#"{{"reason":"{:?}","coord":[{},{}],"t":{:.1},"dist_m":{:.1}}}"#,
                e.reason, e.coord[0], e.coord[1], e.timestamp_secs, e.distance_m
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        r#"{{
  "timestamp_secs": {:.1},
  "schema_version": 1,
  "config_source": "{}",
  "radii": {{ "simulation_m": {:.1}, "view_m": {:.1}, "unload_m": {:.1} }},
  "hysteresis": {{ "min_residence_secs": {:.2} }},
  "budget": {{ "max_mb": {:.1}, "max_chunks": {}, "current_mb_est": {:.1}, "current_chunks": {} }},
  "async_pipeline": {{ "max_queue_depth": {}, "current_depth": {}, "chunks_per_frame": {}, "mode": "synchronous", "_note": "generation sync main-thread; async pool not implemented -> current_depth always 0 (placeholder)" }},
  "counts": {{ "loaded": {}, "pending_load": {}, "pending_gen": {} }},
  "lod_histogram": {{ "lod0": {}, "lod1": {}, "lod2": {} }},
  "hysteresis_blocked_unloads": {},
  "evictions_10s": {{ "distance": {}, "budget": {}, "lod_demotion": {}, "manual": {}, "total": {} }},
  "gen_ms": {{ "sample_count": {}, "mean": {:.2}, "max": {:.2}, "p50": {:.2}, "p95": {:.2}, "p99": {:.2}, "buckets_log2": [{}] }},
  "recent_evictions": [{}],
  "pause": {{ "active": {}, "reason": "{}", "waiting_chunks": {} }},
  "foliage_coverage": {{ "loaded": {}, "with_veg": {}, "without_veg": {}, "threshold": {}, "sustained_s": {:.2} }},
  "severity": "{}"
}}"#,
        now,
        cfg.source,
        cfg.radii.simulation_m,
        cfg.radii.view_m,
        cfg.radii.unload_m,
        cfg.hysteresis.min_residence_secs,
        cfg.budget.max_mb,
        cfg.budget.max_chunks,
        stats.loaded_mb_est,
        stats.loaded_count,
        cfg.async_pipeline.max_queue_depth,
        stats.async_queue_depth,
        cfg.async_pipeline.chunks_per_frame,
        stats.loaded_count,
        stats.pending_load_count,
        stats.pending_gen_count,
        stats.lod0_count,
        stats.lod1_count,
        stats.lod2_count,
        stats.hysteresis_blocked_unloads,
        stats.evictions_10s.distance,
        stats.evictions_10s.budget,
        stats.evictions_10s.lod_demotion,
        stats.evictions_10s.manual,
        stats.total_evictions_10s(),
        h.sample_count,
        h.mean_ms(),
        h.max_ms,
        h.percentile_ms(0.50),
        h.percentile_ms(0.95),
        h.percentile_ms(0.99),
        buckets_json,
        recent_json,
        pause.active,
        pause.reason.replace('"', "'"),
        pause.waiting_chunks,
        coverage.chunks_loaded,
        coverage.chunks_with_veg,
        coverage.without_veg(),
        cfg.debug.foliage_coverage_warn_threshold,
        coverage_sustained_s,
        severity,
    )
}

fn build_health_json(
    now: f32,
    cfg: &StreamingConfig,
    stats: &StreamingStats,
    pause: &StreamingPause,
    coverage: &FoliageCoverageReport,
    coverage_sustained_s: f32,
    severity: &str,
) -> String {
    let (message, next_step) = diagnose(cfg, stats, pause, coverage, coverage_sustained_s);
    format!(
        r#"{{
  "timestamp_secs": {:.1},
  "severity": "{}",
  "message": "{}",
  "next_step": "{}",
  "read_more": "Read forgia_chunk_stream.json"
}}"#,
        now, severity, message, next_step
    )
}

fn diagnose(
    cfg: &StreamingConfig,
    stats: &StreamingStats,
    pause: &StreamingPause,
    coverage: &FoliageCoverageReport,
    coverage_sustained_s: f32,
) -> (String, String) {
    if pause.active && stats.pending_load_count > 0 {
        return (
            format!(
                "StreamingPause active ({}): {} chunks pending in simulation radius",
                pause.reason, stats.pending_load_count
            ),
            "Normal at boot/teleport — should resolve in <2s. If sustained >5s, investigate forgia-terrain chunk loader async pipeline".into(),
        );
    }
    if stats.loaded_mb_est > cfg.budget.max_mb {
        return (
            format!(
                "Memory budget exceeded: {:.0}MB loaded vs {:.0}MB cap",
                stats.loaded_mb_est, cfg.budget.max_mb
            ),
            "Either raise budget.max_mb in config/genomes/streaming.toml, or shrink view_m to load fewer chunks. Eviction LRU should kick in (check evictions_10s.budget)".into(),
        );
    }
    if stats.async_queue_depth >= cfg.async_pipeline.max_queue_depth {
        return (
            format!(
                "Async chunk gen queue saturated: {}/{} tasks",
                stats.async_queue_depth, cfg.async_pipeline.max_queue_depth
            ),
            "Player moving faster than chunk gen pipeline can keep up. Reduce view_m, or raise max_queue_depth, or profile mesh gen with forgia_terrain_pipeline.json (story-450 wave 3)".into(),
        );
    }
    if stats.gen_ms_hist.percentile_ms(0.99) > 50.0 {
        return (
            format!(
                "Chunk gen p99 = {:.1}ms (>50ms threshold) — frame budget at risk",
                stats.gen_ms_hist.percentile_ms(0.99)
            ),
            "Profile meshing_heightmap + biome generation. Move to async pool if sync. Check forgia_terrain_pipeline.json buckets".into(),
        );
    }
    if coverage.without_veg() > cfg.debug.foliage_coverage_warn_threshold
        && coverage_sustained_s >= cfg.debug.foliage_coverage_sustained_s
    {
        return (
            format!(
                "Foliage coverage gap: {} chunks loaded sans veg sustained {:.1}s (threshold {})",
                coverage.without_veg(),
                coverage_sustained_s,
                cfg.debug.foliage_coverage_warn_threshold
            ),
            "Si producteur foliage non wiré (story-502-B pas livrée) ce warning est attendu. Sinon : check forgia-foliage::populate_new_chunks (AssetRegistry::is_ready au boot ?). Voir story-502 plan G2+G3".into(),
        );
    }
    ("All streaming invariants OK".into(), String::new())
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct ForgiaStreamingPlugin;

impl Plugin for ForgiaStreamingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StreamingStats>()
            .init_resource::<StreamingPause>()
            .init_resource::<FoliageCoverageReport>()
            .init_resource::<FoliageCoverageState>()
            .init_resource::<SensorTimer>()
            .add_systems(Startup, load_streaming_genome)
            .add_systems(Update, write_chunk_stream_sensor);
    }
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Default => write!(f, "default"),
            Self::TomlGenome => write!(f, "toml"),
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_GENOME: &str = r#"
schema_version = 1

[radii]
simulation_m = 64.0
view_m = 96.0
unload_m = 128.0

[hysteresis]
min_residence_secs = 2.0

[budget]
max_mb = 512.0
max_chunks = 256

[async_pipeline]
max_queue_depth = 8
chunks_per_frame = 2

[debug]
sensor_enabled = 1
sensor_interval_s = 1.0
gen_ms_buckets = 16
"#;

    #[test]
    fn parse_minimal_genome_ok() {
        let g = StreamingGenome::parse(MINIMAL_GENOME).unwrap();
        assert_eq!(g.radii.simulation_m, 64.0);
        assert_eq!(g.budget.max_mb, 512.0);
    }

    #[test]
    fn default_genome_passes_validation() {
        StreamingGenome::default().validate().unwrap();
    }

    #[test]
    fn radii_inverted_rejected() {
        let mut g = StreamingGenome::default();
        g.radii.simulation_m = 200.0;
        let err = g.validate().unwrap_err();
        matches!(err, StreamingGenomeError::Invariant(_));
    }

    #[test]
    fn view_greater_than_unload_rejected() {
        let mut g = StreamingGenome::default();
        g.radii.view_m = 200.0;
        g.radii.unload_m = 100.0;
        assert!(g.validate().is_err());
    }

    #[test]
    fn schema_version_mismatch_rejected() {
        let bad = MINIMAL_GENOME.replace("schema_version = 1", "schema_version = 99");
        let err = StreamingGenome::parse(&bad).unwrap_err();
        matches!(err, StreamingGenomeError::SchemaVersion { .. });
    }

    #[test]
    fn from_genome_propagates_values() {
        let g = StreamingGenome::default();
        let cfg = StreamingConfig::from_genome(&g, ConfigSource::Default);
        assert_eq!(cfg.radii.view_m, g.radii.view_m);
        assert!(cfg.debug.sensor_enabled);
    }

    #[test]
    fn histogram_record_and_percentile() {
        let mut h = GenMsHistogram::default();
        for ms in [0.5, 0.8, 2.0, 3.0, 5.0, 10.0, 50.0] {
            h.record(ms);
        }
        assert_eq!(h.sample_count, 7);
        assert!(h.max_ms >= 50.0);
        let p99 = h.percentile_ms(0.99);
        assert!(p99 > 0.0);
        let p50 = h.percentile_ms(0.50);
        assert!(p50 <= p99);
    }

    #[test]
    fn histogram_rejects_nan_negative() {
        let mut h = GenMsHistogram::default();
        h.record(f32::NAN);
        h.record(-5.0);
        assert_eq!(h.sample_count, 0);
    }

    #[test]
    fn stats_record_eviction_caps_recent() {
        let mut s = StreamingStats::default();
        for i in 0..50 {
            s.record_eviction(EvictionEvent {
                reason: EvictionReason::Distance,
                coord: [i, 0],
                timestamp_secs: i as f32,
                distance_m: 100.0,
            });
        }
        assert_eq!(s.recent_evictions.len(), RECENT_EVICTIONS_CAP);
        assert_eq!(s.evictions_10s.distance, 50);
    }

    #[test]
    fn stats_tick_window_resets_counters() {
        let mut s = StreamingStats::default();
        s.record_eviction(EvictionEvent {
            reason: EvictionReason::Budget,
            coord: [0, 0],
            timestamp_secs: 0.0,
            distance_m: 0.0,
        });
        assert_eq!(s.total_evictions_10s(), 1);
        s.tick_window(15.0);
        assert_eq!(s.total_evictions_10s(), 0);
        assert_eq!(s.last_window_reset_secs, 15.0);
    }

    #[test]
    fn severity_ok_when_idle() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats::default();
        let pause = StreamingPause::default();
        let cov = FoliageCoverageReport::default();
        assert_eq!(compute_severity(&cfg, &stats, &pause, &cov, 0.0), "ok");
    }

    #[test]
    fn severity_info_when_pause_with_pending() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats {
            pending_load_count: 3,
            ..Default::default()
        };
        let pause = StreamingPause {
            active: true,
            reason: "boot".into(),
            waiting_chunks: 3,
        };
        let cov = FoliageCoverageReport::default();
        assert_eq!(compute_severity(&cfg, &stats, &pause, &cov, 0.0), "info");
    }

    #[test]
    fn severity_warning_when_budget_exceeded() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats {
            loaded_mb_est: cfg.budget.max_mb + 100.0,
            ..Default::default()
        };
        let pause = StreamingPause::default();
        let cov = FoliageCoverageReport::default();
        assert_eq!(compute_severity(&cfg, &stats, &pause, &cov, 0.0), "warning");
    }

    #[test]
    fn severity_warning_when_queue_saturated() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats {
            async_queue_depth: cfg.async_pipeline.max_queue_depth,
            ..Default::default()
        };
        let pause = StreamingPause::default();
        let cov = FoliageCoverageReport::default();
        assert_eq!(compute_severity(&cfg, &stats, &pause, &cov, 0.0), "warning");
    }

    #[test]
    fn diagnose_pause_returns_next_step() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats {
            pending_load_count: 5,
            ..Default::default()
        };
        let pause = StreamingPause {
            active: true,
            reason: "teleport".into(),
            waiting_chunks: 5,
        };
        let cov = FoliageCoverageReport::default();
        let (msg, next) = diagnose(&cfg, &stats, &pause, &cov, 0.0);
        assert!(msg.contains("teleport"));
        assert!(!next.is_empty());
    }

    #[test]
    fn build_sensor_json_well_formed() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats::default();
        let pause = StreamingPause::default();
        let cov = FoliageCoverageReport::default();
        let json = build_sensor_json(123.4, &cfg, &stats, &pause, &cov, 0.0, "ok");
        assert!(json.contains(r#""schema_version": 1"#));
        assert!(json.contains(r#""severity": "ok""#));
        assert!(json.contains(r#""buckets_log2": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]"#));
    }

    // ── Story-502-A : FoliageCoverageReport ───────────────────────────────

    #[test]
    fn foliage_coverage_default_is_zero() {
        let cov = FoliageCoverageReport::default();
        assert_eq!(cov.chunks_loaded, 0);
        assert_eq!(cov.chunks_with_veg, 0);
        assert_eq!(cov.without_veg(), 0);
    }

    #[test]
    fn foliage_coverage_without_veg_saturating() {
        // Producteur incohérent (with_veg > loaded) ne doit pas underflow.
        let cov = FoliageCoverageReport {
            chunks_loaded: 2,
            chunks_with_veg: 10,
        };
        assert_eq!(cov.without_veg(), 0);
    }

    #[test]
    fn foliage_coverage_without_veg_arithmetic() {
        let cov = FoliageCoverageReport {
            chunks_loaded: 12,
            chunks_with_veg: 5,
        };
        assert_eq!(cov.without_veg(), 7);
    }

    #[test]
    fn severity_warning_when_coverage_gap_sustained() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats::default();
        let pause = StreamingPause::default();
        let cov = FoliageCoverageReport {
            chunks_loaded: 10,
            chunks_with_veg: 0,
        };
        // without_veg = 10 > threshold (4) ET sustained 3.0s >= cfg sustained 3.0s
        assert_eq!(compute_severity(&cfg, &stats, &pause, &cov, 3.0), "warning");
    }

    #[test]
    fn severity_ok_when_coverage_gap_below_sustained() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats::default();
        let pause = StreamingPause::default();
        let cov = FoliageCoverageReport {
            chunks_loaded: 10,
            chunks_with_veg: 0,
        };
        // gap au-dessus du threshold mais sustained 1s < cfg sustained 3s
        assert_eq!(compute_severity(&cfg, &stats, &pause, &cov, 1.0), "ok");
    }

    #[test]
    fn severity_ok_when_coverage_full() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats::default();
        let pause = StreamingPause::default();
        let cov = FoliageCoverageReport {
            chunks_loaded: 100,
            chunks_with_veg: 100,
        };
        // 0 gap même sustained long
        assert_eq!(compute_severity(&cfg, &stats, &pause, &cov, 60.0), "ok");
    }

    #[test]
    fn build_sensor_json_contains_foliage_coverage_block() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats::default();
        let pause = StreamingPause::default();
        let cov = FoliageCoverageReport {
            chunks_loaded: 12,
            chunks_with_veg: 3,
        };
        let json = build_sensor_json(0.0, &cfg, &stats, &pause, &cov, 2.5, "ok");
        assert!(json.contains(r#""foliage_coverage""#));
        assert!(json.contains(r#""loaded": 12"#));
        assert!(json.contains(r#""with_veg": 3"#));
        assert!(json.contains(r#""without_veg": 9"#));
        assert!(json.contains(r#""threshold": 4"#));
        assert!(json.contains(r#""sustained_s": 2.50"#));
    }

    #[test]
    fn diagnose_coverage_gap_returns_actionable_next_step() {
        let cfg = StreamingConfig::default();
        let stats = StreamingStats::default();
        let pause = StreamingPause::default();
        let cov = FoliageCoverageReport {
            chunks_loaded: 20,
            chunks_with_veg: 0,
        };
        let (msg, next) = diagnose(&cfg, &stats, &pause, &cov, 5.0);
        assert!(msg.contains("Foliage coverage gap"));
        assert!(msg.contains("20"));
        assert!(next.contains("story-502"));
    }

    #[test]
    fn legacy_genome_without_coverage_fields_parses_with_defaults() {
        // Backward compat : un TOML pré-story-502-A doit parser via serde(default).
        let legacy = MINIMAL_GENOME; // n'a pas les 2 nouveaux champs
        let g = StreamingGenome::parse(legacy).unwrap();
        assert_eq!(
            g.debug.foliage_coverage_warn_threshold,
            default_foliage_coverage_warn_threshold()
        );
        assert!(
            (g.debug.foliage_coverage_sustained_s - default_foliage_coverage_sustained_s()).abs()
                < 1e-6
        );
    }

    #[test]
    fn genome_with_coverage_overrides_propagates_to_config() {
        let with_overrides = format!(
            "{}\nfoliage_coverage_warn_threshold = 12\nfoliage_coverage_sustained_s = 7.5\n",
            MINIMAL_GENOME.trim_end()
        );
        let g = StreamingGenome::parse(&with_overrides).unwrap();
        assert_eq!(g.debug.foliage_coverage_warn_threshold, 12);
        assert_eq!(g.debug.foliage_coverage_sustained_s, 7.5);
        let cfg = StreamingConfig::from_genome(&g, ConfigSource::TomlGenome);
        assert_eq!(cfg.debug.foliage_coverage_warn_threshold, 12);
        assert_eq!(cfg.debug.foliage_coverage_sustained_s, 7.5);
    }
}
