//! pipeline_diag.rs — STUB pending V1 port.

#![allow(dead_code, unused_imports, unused_variables)]

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Resource, Default)]
pub struct PipelineDiagnostics;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum AnomalyKind {
    #[default]
    None,
    Heightmap,
    Mesh,
    Cave,
    Biome,
    Path,
    Streaming,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnomalyThresholds {
    pub max_chunk_ms: f32,
    pub min_fps: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChunkPipelineDiag {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub time_ms: f32,
    pub triangle_count: u32,
}

pub fn record_event(_label: &str) {}
pub fn record_chunk_metric(_x: i32, _z: i32, _metric: f64) {}
pub fn flush_diagnostics() {}
pub fn compute_saturation_pct(_used: u32, _total: u32) -> f32 { 0.0 }
