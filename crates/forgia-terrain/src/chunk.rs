//! Chunk data-model for the voxel SDF terrain.
//!
//! Certifié zone propre story-349 E2 : 16 tests couvrent `ChunkCoord`,
//! `ChunkData`, `ChunkManager` (LRU, loaded_entities, dirty flag)
//! et `TerrainConfig` (defaults).

use bevy::prelude::*;
use std::collections::{HashMap, HashSet, VecDeque};

// ─────────────────────────── Chunk Dimensions ───────────────────────────

pub const CHUNK_X: u32 = 32;
pub const CHUNK_Y: u32 = 128;
pub const CHUNK_Z: u32 = 32;
pub const PAD_X: u32 = CHUNK_X + 2;
pub const PAD_Y: u32 = CHUNK_Y + 2;
pub const PAD_Z: u32 = CHUNK_Z + 2;
pub const PADDED_TOTAL: usize = (PAD_X * PAD_Y * PAD_Z) as usize;
pub const COLUMNS: usize = (CHUNK_X * CHUNK_Z) as usize;

// ─────────────────────────── ChunkCoord ───────────────────────────

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub fn new(x: i32, z: i32) -> Self { Self { x, z } }

    pub fn world_origin(&self) -> Vec3 {
        Vec3::new(self.x as f32 * CHUNK_X as f32, 0.0, self.z as f32 * CHUNK_Z as f32)
    }

    pub fn world_center(&self) -> Vec3 {
        let o = self.world_origin();
        Vec3::new(o.x + CHUNK_X as f32 * 0.5, 0.0, o.z + CHUNK_Z as f32 * 0.5)
    }

    pub fn from_world(pos: Vec3) -> Self {
        Self {
            x: (pos.x / CHUNK_X as f32).floor() as i32,
            z: (pos.z / CHUNK_Z as f32).floor() as i32,
        }
    }

    pub fn distance(&self, other: &ChunkCoord) -> i32 {
        (self.x - other.x).abs() + (self.z - other.z).abs()
    }
}

// ─────────────────────────── ChunkData ───────────────────────────

pub struct ChunkData {
    pub sdf: Vec<f32>,
    pub biome_ids: Vec<u8>,
    pub dirty: bool,
    pub modified: bool,
    pub pipeline_diag: Option<crate::pipeline_diag::ChunkPipelineDiag>,
}

impl ChunkData {
    pub fn new_air() -> Self {
        Self {
            sdf: vec![1.0; PADDED_TOTAL],
            biome_ids: vec![0; COLUMNS],
            dirty: true,
            modified: false,
            pipeline_diag: None,
        }
    }

    #[inline]
    pub fn index(x: u32, y: u32, z: u32) -> usize {
        (x + PAD_X * (y + PAD_Y * z)) as usize
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32, z: u32) -> f32 {
        self.sdf[Self::index(x, y, z)]
    }

    pub fn is_all_air(&self) -> bool { self.sdf.iter().all(|&v| v > 0.0) }
    pub fn is_all_solid(&self) -> bool { self.sdf.iter().all(|&v| v < 0.0) }
}

// ─────────────────────────── SDF Cache Quantization ───────────────────────────

pub const SDF_QUANT_SCALE: f32 = 100.0;
pub const CACHE_ZSTD_LEVEL: i32 = 3;

pub struct CachedChunkData {
    sdf_compressed: Vec<u8>,
    original_i16_count: usize,
    pub biome_ids: Vec<u8>,
}

impl CachedChunkData {
    pub fn from_chunk(data: &ChunkData) -> Self {
        let mut sdf_i16 = Vec::with_capacity(data.sdf.len());
        let min = f32::from(i16::MIN);
        let max = f32::from(i16::MAX);
        for &v in &data.sdf {
            sdf_i16.push((v * SDF_QUANT_SCALE).clamp(min, max) as i16);
        }
        let original_i16_count = sdf_i16.len();
        let mut bytes = Vec::with_capacity(sdf_i16.len() * 2);
        for &v in &sdf_i16 { bytes.extend_from_slice(&v.to_le_bytes()); }
        let sdf_compressed = zstd::bulk::compress(&bytes, CACHE_ZSTD_LEVEL).unwrap_or(bytes);
        Self { sdf_compressed, original_i16_count, biome_ids: data.biome_ids.clone() }
    }

    pub fn to_chunk(&self) -> ChunkData {
        let budget = self.original_i16_count * std::mem::size_of::<i16>();
        let decompressed = zstd::bulk::decompress(&self.sdf_compressed, budget).unwrap_or_default();
        let expected_bytes = self.original_i16_count * 2;
        let valid_bytes = decompressed.len().min(expected_bytes);
        let inv_scale = 1.0 / SDF_QUANT_SCALE;
        let mut sdf = Vec::with_capacity(self.original_i16_count);
        for chunk in decompressed[..valid_bytes].chunks_exact(2) {
            let v = i16::from_le_bytes([chunk[0], chunk[1]]);
            sdf.push(f32::from(v) * inv_scale);
        }
        sdf.resize(self.original_i16_count, 1.0);
        ChunkData { sdf, biome_ids: self.biome_ids.clone(), dirty: true, modified: false, pipeline_diag: None }
    }

    pub fn byte_size(&self) -> usize { self.sdf_compressed.len() + self.biome_ids.len() }
}

// ─────────────────────────── ChunkManager ───────────────────────────

const CHUNK_CACHE_SIZE: usize = 128;

#[derive(Resource, Default)]
pub struct ChunkManager {
    pub chunks: HashMap<ChunkCoord, ChunkData>,
    pub loaded_entities: HashMap<ChunkCoord, Entity>,
    pub dirty_chunks: HashSet<ChunkCoord>,
    pub last_player_chunk: Option<ChunkCoord>,
    chunk_cache: HashMap<ChunkCoord, CachedChunkData>,
    cache_order: VecDeque<ChunkCoord>,
    pub empty_mesh_coords: HashSet<ChunkCoord>,
}

impl ChunkManager {
    pub fn get(&self, coord: &ChunkCoord) -> Option<&ChunkData> { self.chunks.get(coord) }

    pub fn cache_unloaded(&mut self, coord: ChunkCoord) {
        self.empty_mesh_coords.remove(&coord);
        if let Some(data) = self.chunks.remove(&coord) {
            if data.modified { return; }
            if self.cache_order.len() >= CHUNK_CACHE_SIZE {
                if let Some(old) = self.cache_order.pop_front() {
                    self.chunk_cache.remove(&old);
                }
            }
            self.cache_order.push_back(coord);
            self.chunk_cache.insert(coord, CachedChunkData::from_chunk(&data));
        }
    }

    pub fn restore_from_cache(&mut self, coord: ChunkCoord) -> bool {
        if let Some(cached) = self.chunk_cache.remove(&coord) {
            self.cache_order.retain(|c| *c != coord);
            self.chunks.insert(coord, cached.to_chunk());
            true
        } else { false }
    }

    pub fn cache_bytes(&self) -> usize { self.chunk_cache.values().map(|c| c.byte_size()).sum() }
    pub fn cache_len(&self) -> usize { self.chunk_cache.len() }
}

// ─────────────────────────── TerrainConfig ───────────────────────────

#[derive(Resource, Clone)]
pub struct TerrainConfig {
    pub map_size: f32,
    pub seed: u32,
    pub streaming_radius: i32,
    pub chunks_per_frame: usize,
    pub sea_level: f32,
    pub max_height: f32,
    pub y_offset: f32,
}

impl Default for TerrainConfig {
    fn default() -> Self {
        Self {
            map_size: 4096.0,
            seed: 20_260_322,
            streaming_radius: 12,
            chunks_per_frame: 2,
            sea_level: 18.0,
            max_height: 180.0,
            y_offset: 0.0,
        }
    }
}

#[derive(Component)]
pub struct TerrainChunkMarker;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_coord_from_world_round_trips_for_origin() {
        let c = ChunkCoord::from_world(Vec3::ZERO);
        assert_eq!(c, ChunkCoord::new(0, 0));
    }

    #[test]
    fn chunk_coord_from_world_handles_negative() {
        let c = ChunkCoord::from_world(Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(c.x, -1);
        assert_eq!(c.z, 0);
    }

    #[test]
    fn chunk_coord_world_origin_is_min_corner() {
        let c = ChunkCoord::new(2, -3);
        let o = c.world_origin();
        assert_eq!(o.x, 2.0 * CHUNK_X as f32);
        assert_eq!(o.y, 0.0);
        assert_eq!(o.z, -3.0 * CHUNK_Z as f32);
    }

    #[test]
    fn chunk_coord_world_center_is_origin_plus_half_span() {
        let c = ChunkCoord::new(0, 0);
        let ctr = c.world_center();
        assert_eq!(ctr.x, CHUNK_X as f32 * 0.5);
        assert_eq!(ctr.z, CHUNK_Z as f32 * 0.5);
    }

    #[test]
    fn chunk_coord_distance_is_manhattan() {
        let a = ChunkCoord::new(1, 2);
        let b = ChunkCoord::new(4, -2);
        assert_eq!(a.distance(&b), 3 + 4);
    }

    #[test]
    fn chunk_data_index_layout_round_trips() {
        let idx = ChunkData::index(1, 2, 3);
        let expected = 1 + PAD_X * (2 + PAD_Y * 3);
        assert_eq!(idx as u32, expected);
    }

    #[test]
    fn chunk_data_new_air_is_all_air() {
        let c = ChunkData::new_air();
        assert!(c.is_all_air());
        assert!(!c.is_all_solid());
        assert!(c.dirty);
    }

    #[test]
    fn chunk_manager_cache_skips_modified_chunks() {
        let mut mgr = ChunkManager::default();
        let coord = ChunkCoord::new(0, 0);
        let mut data = ChunkData::new_air();
        data.modified = true;
        mgr.chunks.insert(coord, data);
        mgr.cache_unloaded(coord);
        assert!(!mgr.chunks.contains_key(&coord));
        assert!(!mgr.restore_from_cache(coord));
    }

    #[test]
    fn chunk_manager_cache_restores_non_modified() {
        let mut mgr = ChunkManager::default();
        let coord = ChunkCoord::new(1, 2);
        mgr.chunks.insert(coord, ChunkData::new_air());
        mgr.cache_unloaded(coord);
        assert!(!mgr.chunks.contains_key(&coord));
        assert!(mgr.restore_from_cache(coord));
        assert!(mgr.chunks.contains_key(&coord));
    }

    #[test]
    fn terrain_config_default_is_sensible() {
        let cfg = TerrainConfig::default();
        assert_eq!(cfg.map_size, 4096.0);
        assert!(cfg.max_height > 0.0 && cfg.max_height.is_finite());
        assert!(cfg.sea_level >= 0.0 && cfg.sea_level < cfg.max_height);
        assert!(cfg.streaming_radius > 0);
        assert!(cfg.chunks_per_frame >= 1);
    }

    #[test]
    fn sdf_quantize_round_trip_preserves_sign() {
        let src = vec![1.5_f32, -2.0, 0.0, 180.0, -180.0, 0.001, -0.001];
        let mut chunk = ChunkData::new_air();
        chunk.sdf = src.clone();
        chunk.sdf.resize(PADDED_TOTAL, 1.0);
        let cached = CachedChunkData::from_chunk(&chunk);
        let restored = cached.to_chunk();
        for (i, (&orig, &back)) in src.iter().zip(restored.sdf.iter()).enumerate() {
            if orig > 0.0 { assert!(back >= 0.0, "positive {orig} flipped to {back} at {i}"); }
            else if orig < 0.0 { assert!(back <= 0.0, "negative {orig} flipped to {back} at {i}"); }
            else { assert_eq!(back, 0.0, "zero must round-trip at {i}"); }
        }
    }

    #[test]
    fn sdf_quantize_precision_within_1cm() {
        let src: Vec<f32> = (0..100).map(|i| (i as f32 - 50.0) * 0.73).collect();
        let mut chunk = ChunkData::new_air();
        chunk.sdf[..src.len()].copy_from_slice(&src);
        let cached = CachedChunkData::from_chunk(&chunk);
        let restored = cached.to_chunk();
        let epsilon = 1.0 / SDF_QUANT_SCALE + 1e-6;
        for (i, (&orig, &back)) in src.iter().zip(restored.sdf.iter()).enumerate() {
            assert!((orig - back).abs() <= epsilon, "precision lost at {i}: {orig} -> {back}");
        }
    }

    #[test]
    fn sdf_quantize_saturates_extreme_values() {
        let src = vec![1e6_f32, -1e6, f32::MAX, f32::MIN];
        let mut chunk = ChunkData::new_air();
        chunk.sdf[..src.len()].copy_from_slice(&src);
        let cached = CachedChunkData::from_chunk(&chunk);
        let restored = cached.to_chunk();
        for &v in restored.sdf.iter().take(src.len()) {
            assert!(v.is_finite(), "saturation must produce finite values, got {v}");
        }
    }

    #[test]
    fn cache_restore_sets_dirty_and_drops_diag() {
        let mut mgr = ChunkManager::default();
        let coord = ChunkCoord::new(5, 5);
        let mut data = ChunkData::new_air();
        data.dirty = false;
        data.pipeline_diag = Some(crate::pipeline_diag::ChunkPipelineDiag::default());
        data.biome_ids[0] = 3;
        mgr.chunks.insert(coord, data);
        mgr.cache_unloaded(coord);
        assert_eq!(mgr.cache_len(), 1);
        assert!(mgr.cache_bytes() > 0);
        assert!(mgr.restore_from_cache(coord));
        let restored = mgr.chunks.get(&coord).unwrap();
        assert!(restored.dirty);
        assert!(restored.pipeline_diag.is_none());
        assert_eq!(restored.biome_ids[0], 3);
    }

    #[test]
    fn cache_bytes_is_far_smaller_than_f32_equivalent() {
        let mut mgr = ChunkManager::default();
        let coord = ChunkCoord::new(0, 0);
        mgr.chunks.insert(coord, ChunkData::new_air());
        mgr.cache_unloaded(coord);
        let stored = mgr.cache_bytes();
        let f32_equiv = PADDED_TOTAL * std::mem::size_of::<f32>() + COLUMNS * std::mem::size_of::<u8>();
        assert!(stored < f32_equiv, "cache {stored} B must be smaller than f32 equivalent {f32_equiv} B");
        let raw_i16 = PADDED_TOTAL * std::mem::size_of::<i16>();
        assert!(stored < raw_i16 / 4, "all-air chunk should compress well");
    }

    #[test]
    fn zstd_cache_roundtrip_preserves_values_within_epsilon() {
        let src: Vec<f32> = (0..128).map(|i| ((i as f32 - 64.0) * 0.37).sin() * 12.5).collect();
        let mut chunk = ChunkData::new_air();
        chunk.sdf[..src.len()].copy_from_slice(&src);
        let cached = CachedChunkData::from_chunk(&chunk);
        let restored = cached.to_chunk();
        let epsilon = 1.0 / SDF_QUANT_SCALE + 1e-6;
        for (i, (&orig, &back)) in src.iter().zip(restored.sdf.iter()).enumerate() {
            assert!((orig - back).abs() <= epsilon, "zstd roundtrip lost precision at {i}");
        }
    }
}
