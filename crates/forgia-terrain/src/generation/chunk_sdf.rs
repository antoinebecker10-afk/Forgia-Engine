//! Chunk SDF Generation — pipeline orchestrator.
//!
//! Orchestre toutes les couches (noise + redistribution + erosion +
//! droplet + thermal + valley carve + slope limit + path/village/castle
//! flatten + SDF conversion + min thickness + smoothing + caves).
//!
//! API publique :
//! - [`GenDetail`] : 3-tier LOD (Full / Fast / Distant)
//! - [`generate_chunk`] : backward-compat wrapper
//! - [`generate_chunk_lod`] : full pipeline avec LOD + genome overrides
//! - [`generate_initial_chunks`] : bootstrap radius around a position

use bevy::prelude::*;
use ::noise::{NoiseFn, Perlin};
use rayon::prelude::*;

use crate::biomes::{BiomeMap, BiomeType};
use crate::chunk::{ChunkCoord, ChunkData, ChunkManager, TerrainConfig, CHUNK_X, CHUNK_Z, PAD_X, PAD_Y, PAD_Z};
use crate::map_gen_config::MapGenConfig;
use crate::paths::PathNetwork;
use crate::village_data::VillageNetwork;

use super::{BiomeGenomeOverrides, CastleFootprint};
use super::caves::{carve_cave_worms, carve_village_caves, cave_threshold_for_biome, CaveWormParams};
use super::droplet::{droplet_erosion_params, thermal_erosion};
use super::erosion::{
    erode_heightmap_variable, erosion_params, restore_padding_ring, save_padding_ring,
    slope_limit_variable, slope_max_for_biome, valley_carve,
};
use super::heightmap::{heightmap_at, heightmap_at_gen, heightmap_at_gen_ext, heightmap_at_gen_ext_fast, micro_roughness};

/// Detail level for chunk generation (3-tier LOD).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GenDetail { #[default] Full, Fast, Distant }

/// Backward-compatible wrapper — generates with Full detail.
pub fn generate_chunk(
    coord: ChunkCoord,
    config: &TerrainConfig,
    biome_map: &BiomeMap,
    gen_config: Option<&MapGenConfig>,
    path_network: Option<&PathNetwork>,
    village_network: Option<&VillageNetwork>,
    castle_footprint: Option<&CastleFootprint>,
) -> ChunkData {
    generate_chunk_lod(coord, config, biome_map, gen_config, path_network, village_network, castle_footprint, GenDetail::Full, None, None, None)
}

/// Generate a complete chunk from procedural noise.
#[allow(clippy::too_many_arguments)]
pub fn generate_chunk_lod(
    coord: ChunkCoord,
    config: &TerrainConfig,
    biome_map: &BiomeMap,
    gen_config: Option<&MapGenConfig>,
    path_network: Option<&PathNetwork>,
    village_network: Option<&VillageNetwork>,
    castle_footprint: Option<&CastleFootprint>,
    detail: GenDetail,
    genome_overrides: Option<&BiomeGenomeOverrides>,
    cave_worm_params: Option<&CaveWormParams>,
    cave_network: Option<&crate::cave_network::CaveNetworkTopology>,
) -> ChunkData {
    let _span = info_span!("terrain_generate_chunk", cx = coord.x, cz = coord.z).entered();
    let gen_start = web_time::Instant::now();
    let mut chunk = ChunkData::new_air();
    let origin = coord.world_origin();

    let mut diag = crate::pipeline_diag::ChunkPipelineDiag {
        chunk: [coord.x, coord.z],
        detail_level: format!("{:?}", detail),
        ..Default::default()
    };

    chunk.biome_ids = biome_map.assign_chunk_biomes(origin.x, origin.z);
    let center_biome = biome_map.biome_at(
        origin.x + (CHUNK_X as f32) * 0.5,
        origin.z + (CHUNK_Z as f32) * 0.5,
    );
    diag.biome = format!("{:?}", center_biome);

    let w = PAD_X as usize;
    let d = PAD_Z as usize;
    let mut height_buf = vec![0.0f32; w * d];
    let mut layer_start = web_time::Instant::now();
    let mut erosion_rate_buf = vec![0.0f32; w * d];
    let mut max_slope_buf = vec![1.8f32; w * d];
    let mut max_passes_buf = 0usize;

    // Pre-compute biome grid once (1156 Voronoi queries instead of 3468)
    let mut biome_buf: Vec<BiomeType> = Vec::with_capacity(w * d);
    for pz in 0..PAD_Z {
        for px in 0..PAD_X {
            let wx = origin.x + (px as f32 - 1.0);
            let wz = origin.z + (pz as f32 - 1.0);
            biome_buf.push(biome_map.biome_at(wx, wz));
        }
    }

    struct PerVoxel {
        height: f32,
        erosion_rate: f32,
        max_slope: f32,
        voxel_max_passes: usize,
    }

    let per_voxel: Vec<PerVoxel> = (0..(w * d))
        .into_par_iter()
        .map(|idx| {
            let px = (idx % w) as u32;
            let pz = (idx / w) as u32;
            let wx = origin.x + (px as f32 - 1.0);
            let wz = origin.z + (pz as f32 - 1.0);

            let mut erosion_rate = 0.0f32;
            let mut max_slope = 1.8f32;
            let mut voxel_max_passes = 0usize;

            let height = match gen_config {
                Some(gc) => {
                    if detail == GenDetail::Distant {
                        let biome = biome_buf[idx];
                        heightmap_at_gen_ext(wx, wz, config, gc, Some(biome), genome_overrides)
                    } else {
                        let bw = biome_map.biome_weights_at(wx, wz, 4, 240.0);

                        let mut erosion_total = 0.0f32;
                        let mut local_max_passes = 1usize;
                        let mut slope_blend = 0.0f32;
                        for i in 0..bw.count {
                            let (b, bw_weight) = bw.biomes[i];
                            let (passes, rate) = erosion_params(b, genome_overrides);
                            erosion_total += passes as f32 * rate * bw_weight;
                            local_max_passes = local_max_passes.max(passes);
                            slope_blend += slope_max_for_biome(b, genome_overrides) * bw_weight;
                        }
                        erosion_rate = erosion_total / local_max_passes as f32;
                        voxel_max_passes = local_max_passes;
                        max_slope = slope_blend;

                        if bw.count <= 1 {
                            heightmap_at_gen_ext(wx, wz, config, gc, Some(bw.biomes[0].0), genome_overrides)
                        } else {
                            let mut dominant_idx = 0;
                            let mut dominant_w = bw.biomes[0].1;
                            for i in 1..bw.count {
                                if bw.biomes[i].1 > dominant_w {
                                    dominant_w = bw.biomes[i].1;
                                    dominant_idx = i;
                                }
                            }
                            let mut h = 0.0f32;
                            for i in 0..bw.count {
                                let (b, bw_weight) = bw.biomes[i];
                                if bw_weight > 0.01 {
                                    let val = if i == dominant_idx {
                                        heightmap_at_gen_ext(wx, wz, config, gc, Some(b), genome_overrides)
                                    } else {
                                        heightmap_at_gen_ext_fast(wx, wz, config, gc, Some(b), genome_overrides)
                                    };
                                    h += val * bw_weight;
                                }
                            }
                            h
                        }
                    }
                }
                None => heightmap_at(wx, wz, config),
            };

            PerVoxel { height, erosion_rate, max_slope, voxel_max_passes }
        })
        .collect();

    for (idx, pv) in per_voxel.into_iter().enumerate() {
        height_buf[idx] = pv.height;
        erosion_rate_buf[idx] = pv.erosion_rate;
        max_slope_buf[idx] = pv.max_slope;
        max_passes_buf = max_passes_buf.max(pv.voxel_max_passes);
    }

    diag.layer_timings.heightmap_ms = layer_start.elapsed().as_secs_f32() * 1000.0;

    // Anomaly detection: NaN or out-of-range heights
    {
        use crate::pipeline_diag::AnomalyKind;
        let max_h = gen_config.map_or(config.max_height, |gc| gc.max_height);
        let mut nan_count = 0u32;
        let mut oor_count = 0u32;
        for h in &height_buf {
            if h.is_nan() { nan_count += 1; }
            else if *h < -10.0 || *h > max_h + 50.0 { oor_count += 1; }
        }
        if nan_count > 0 {
            diag.anomalies.push(format!(
                "{} chunk [{},{}] {} NaN cells in heightmap",
                AnomalyKind::HeightNan.prefix(), coord.x, coord.z, nan_count,
            ));
        }
        if oor_count > 0 {
            diag.anomalies.push(format!(
                "{} chunk [{},{}] {} cells out of band [-10..{:.0}+50] (max_height={:.0})",
                AnomalyKind::HeightOutOfRange.prefix(),
                coord.x, coord.z, oor_count, max_h, max_h,
            ));
        }
    }

    // R3/R7: skip remaining passes for dead chunks
    {
        let sea_level = gen_config.map_or(config.sea_level, |gc| gc.sea_level);
        let max_h = gen_config.map_or(config.max_height, |gc| gc.max_height);
        let (h_min, h_max) = height_buf.iter().copied().fold(
            (f32::INFINITY, f32::NEG_INFINITY),
            |(mn, mx), h| (mn.min(h), mx.max(h)),
        );

        let dead_underwater = h_max.is_finite() && h_max < sea_level - 5.0;
        let dead_saturated = h_max.is_finite() && h_min.is_finite()
            && h_max >= max_h * 0.99
            && (h_max - h_min) < 0.3;

        if dead_underwater || dead_saturated {
            let sea_floor_y = sea_level - 2.0;
            let y_off = config.y_offset;
            for py in 0..PAD_Y {
                let wy = py as f32 - 1.0 + y_off;
                let sdf_val = wy - sea_floor_y;
                for pz in 0..PAD_Z {
                    for px in 0..PAD_X {
                        let idx = ChunkData::index(px, py, pz);
                        chunk.sdf[idx] = sdf_val;
                    }
                }
            }

            diag.dead_skip = true;
            diag.gen_time_ms = gen_start.elapsed().as_secs_f32() * 1000.0;
            chunk.pipeline_diag = Some(diag);
            chunk.dirty = true;
            chunk.modified = false;
            return chunk;
        }
    }

    diag.noise.ran = true;
    if gen_config.is_some() {
        diag.domain_warp.ran = true;
        diag.domain_warp.detail = Some(format!("detail={:?}", detail));
        diag.redistribution.ran = true;
    }

    // Snapshot genome values effective for this chunk's biome
    {
        let bi = (center_biome as u8 as usize).min(9);
        let g = &mut diag.genome;
        g.overrides_total = 7;
        if let Some(ovr) = genome_overrides {
            if let Some((p, r)) = ovr.erosion[bi] {
                g.erosion = Some(format!("passes={} rate={:.2}", p, r));
                g.overrides_active += 1;
            }
            if let Some(v) = ovr.micro_roughness_amp[bi] {
                g.micro_roughness_amp = Some(v);
                g.overrides_active += 1;
            }
            if let Some(v) = ovr.warp_strength[bi] {
                g.warp_strength = Some(v);
                g.overrides_active += 1;
            }
            if let Some(v) = ovr.hydro_droplet_scale {
                g.hydro_droplet_scale = Some(v);
                g.overrides_active += 1;
            }
            if let Some(v) = ovr.thermal_talus_angle {
                g.thermal_talus_angle = Some(v);
                g.overrides_active += 1;
            }
            if let Some(v) = ovr.cave_probabilities[bi] {
                g.cave_probability = Some(v);
                g.overrides_active += 1;
            }
            if let Some(ref nl) = ovr.noise_layers[bi] {
                g.noise_weights = Some(format!("ridged={:.1} billow={:.1} worley={:.1} swiss={:.1}",
                    nl.ridged_weight, nl.billow_weight, nl.worley_weight, nl.swiss_weight));
                g.overrides_active += 1;
            }
        }
    }

    // 2. Erosion on the 2D buffer (padding ring invariant)
    if gen_config.is_some() && detail != GenDetail::Distant {
        let padding_backup = save_padding_ring(&height_buf, w, d);
        diag.padding_ring_saved = true;

        if max_passes_buf > 0 {
            layer_start = web_time::Instant::now();
            erode_heightmap_variable(&mut height_buf, w, d, max_passes_buf, &erosion_rate_buf);
            diag.layer_timings.erosion_variable_ms = layer_start.elapsed().as_secs_f32() * 1000.0;
            diag.erosion_variable.ran = true;
            diag.erosion_variable.detail = Some(format!("passes={}", max_passes_buf));
        }

        if detail == GenDetail::Full {
            layer_start = web_time::Instant::now();
            let base_count = (w * d) / 24;
            let scale = genome_overrides
                .and_then(|o| o.hydro_droplet_scale)
                .unwrap_or(1.0);
            let droplet_count = (base_count as f32 * scale).max(20.0) as usize;
            let erosion_seed = config.seed.wrapping_add(coord.x.wrapping_mul(73) as u32).wrapping_add(coord.z.wrapping_mul(97) as u32);
            let hydro_params = genome_overrides
                .and_then(|o| o.hydro_erosion.as_ref())
                .cloned()
                .unwrap_or_default();
            droplet_erosion_params(&mut height_buf, w, d, droplet_count, erosion_seed, &hydro_params);
            diag.layer_timings.erosion_hydraulic_ms = layer_start.elapsed().as_secs_f32() * 1000.0;
            diag.erosion_hydraulic.ran = true;
            diag.erosion_hydraulic.detail = Some(format!("droplets={}", droplet_count));
        }

        if detail == GenDetail::Full {
            layer_start = web_time::Instant::now();
            let talus = genome_overrides
                .and_then(|o| o.thermal_talus_angle)
                .unwrap_or(1.5);
            let center_biome = biome_map.biome_at(
                origin.x + (w as f32) * 0.5,
                origin.z + (d as f32) * 0.5,
            );
            let bi_center = (center_biome as u8 as usize).min(9);
            let thermal_passes = genome_overrides
                .and_then(|o| o.thermal_passes[bi_center])
                .map(|v| v as usize)
                .unwrap_or_else(|| match center_biome {
                    BiomeType::Canyon | BiomeType::Mountain => 3,
                    BiomeType::Volcanic => 2,
                    _ => 1,
                });
            thermal_erosion(&mut height_buf, w, d, thermal_passes, talus);
            diag.layer_timings.erosion_thermal_ms = layer_start.elapsed().as_secs_f32() * 1000.0;
            diag.erosion_thermal.ran = true;
            diag.erosion_thermal.detail = Some(format!("passes={} talus={:.1}", thermal_passes, talus));
        }

        if detail == GenDetail::Full {
            layer_start = web_time::Instant::now();
            let sea_level = gen_config.map_or(config.sea_level, |gc| gc.sea_level);
            valley_carve(&mut height_buf, w, d, sea_level);
            diag.layer_timings.valley_carving_ms = layer_start.elapsed().as_secs_f32() * 1000.0;
            diag.valley_carving.ran = true;
        }

        layer_start = web_time::Instant::now();
        let slope_passes = match detail {
            GenDetail::Full => 2,
            GenDetail::Fast => 1,
            GenDetail::Distant => 3,
        };
        slope_limit_variable(&mut height_buf, w, d, &max_slope_buf, slope_passes);
        diag.layer_timings.slope_limiting_ms = layer_start.elapsed().as_secs_f32() * 1000.0;
        diag.slope_limiting.ran = true;
        diag.slope_limiting.detail = Some(format!("passes={}", slope_passes));

        // 2.5. Path flattening
        layer_start = web_time::Instant::now();
        if let Some(paths) = path_network {
            if let Some(gc) = gen_config {
                for pz in 0..PAD_Z {
                    for px in 0..PAD_X {
                        let wx = origin.x + (px as f32 - 1.0);
                        let wz = origin.z + (pz as f32 - 1.0);
                        let influence = paths.path_influence(wx, wz);
                        if influence > 0.05 {
                            let idx = px as usize + w * pz as usize;
                            let biome = biome_buf[idx];

                            let raw_h = heightmap_at_gen(wx, wz, config, gc, Some(biome));
                            let micro_amp = genome_overrides
                                .and_then(|o| o.micro_roughness_amp[(biome as u8 as usize).min(9)])
                                .unwrap_or(1.0);
                            let micro = micro_roughness(wx, wz, config.seed, micro_amp);
                            diag.micro_roughness.ran = true;
                            let smooth_h = raw_h - micro;

                            let blend = influence.clamp(0.0, 1.0);
                            height_buf[idx] = height_buf[idx] * (1.0 - blend) + smooth_h * blend;

                            let depression = paths.depression_at(wx, wz);
                            height_buf[idx] -= depression * influence.powi(2);
                        }
                    }
                }
            }
        }

        diag.layer_timings.path_flatten_ms = layer_start.elapsed().as_secs_f32() * 1000.0;
        diag.path_flatten.ran = path_network.is_some() && gen_config.is_some();

        // 2.6. Village flattening
        layer_start = web_time::Instant::now();
        if let Some(villages) = village_network {
            for pz in 0..PAD_Z {
                for px in 0..PAD_X {
                    let wx = origin.x + (px as f32 - 1.0);
                    let wz = origin.z + (pz as f32 - 1.0);
                    if let Some((vi, norm_dist)) = villages.village_influence(wx, wz) {
                        let target = villages.villages[vi].target_height + 0.5;
                        let idx = px as usize + w * pz as usize;
                        let t = 1.0 - norm_dist;
                        let blend = t * t * (3.0 - 2.0 * t);
                        height_buf[idx] = height_buf[idx] * (1.0 - blend * 0.8) + target * blend * 0.8;
                    }
                }
            }
        }

        diag.layer_timings.village_flatten_ms = layer_start.elapsed().as_secs_f32() * 1000.0;
        diag.village_flatten.ran = village_network.is_some();

        // 2.7. Castle flattening
        layer_start = web_time::Instant::now();
        if let Some(castle) = castle_footprint {
            for pz in 0..PAD_Z {
                for px in 0..PAD_X {
                    let wx = origin.x + (px as f32 - 1.0);
                    let wz = origin.z + (pz as f32 - 1.0);
                    if let Some(norm_dist) = castle.influence(wx, wz) {
                        let idx = px as usize + w * pz as usize;
                        let blend = CastleFootprint::flatten_blend(norm_dist);
                        height_buf[idx] = height_buf[idx] * (1.0 - blend) + castle.target_height * blend;
                    }
                }
            }
        }

        diag.layer_timings.castle_flatten_ms = layer_start.elapsed().as_secs_f32() * 1000.0;
        diag.castle_flatten.ran = castle_footprint.is_some();

        restore_padding_ring(&mut height_buf, w, d, &padding_backup);
        diag.padding_ring_restored = true;
        diag.padding_restore.ran = true;

        // Re-apply castle flatten on the padding ring cells after restore.
        if let Some(castle) = castle_footprint {
            let ring_cells: Vec<usize> = {
                let mut r = Vec::with_capacity(2 * (w + d));
                for px in 0..PAD_X as usize { r.push(px); r.push(px + w * (d - 1)); }
                for pz in 1..PAD_Z as usize - 1 { r.push(w * pz); r.push(w - 1 + w * pz); }
                r
            };
            for idx in ring_cells {
                let px = (idx % w) as u32;
                let pz = (idx / w) as u32;
                let wx = origin.x + (px as f32 - 1.0);
                let wz = origin.z + (pz as f32 - 1.0);
                if let Some(norm_dist) = castle.influence(wx, wz) {
                    let blend = CastleFootprint::flatten_blend(norm_dist);
                    height_buf[idx] = height_buf[idx] * (1.0 - blend) + castle.target_height * blend;
                }
            }
        }
    }

    // Castle flatten for Distant tier
    if detail == GenDetail::Distant {
        if let Some(castle) = castle_footprint {
            for pz in 0..PAD_Z {
                for px in 0..PAD_X {
                    let wx = origin.x + (px as f32 - 1.0);
                    let wz = origin.z + (pz as f32 - 1.0);
                    if let Some(norm_dist) = castle.influence(wx, wz) {
                        let idx = px as usize + w * pz as usize;
                        let blend = CastleFootprint::flatten_blend(norm_dist);
                        height_buf[idx] = height_buf[idx] * (1.0 - blend) + castle.target_height * blend;
                    }
                }
            }
        }
    }

    // Clamp extreme slopes
    let (slope_clamp_deg, slope_clamp_passes) = gen_config
        .map(|gc| (gc.slope_clamp_deg, gc.slope_clamp_passes))
        .unwrap_or((65.0, 4));
    clamp_extreme_slopes(&mut height_buf, w, d, slope_clamp_deg, slope_clamp_passes);

    diag.compute_height_stats(&height_buf, w, d);
    diag.compute_maps_stats(&height_buf, w, d);

    {
        let max_h = gen_config.map_or(config.max_height, |gc| gc.max_height);
        let saturation_pct = crate::pipeline_diag::compute_saturation_pct(&height_buf, w, d, max_h);
        diag.detect_structural_anomalies(
            &crate::pipeline_diag::AnomalyThresholds::default(),
            saturation_pct,
        );
    }

    // 3. Convert heightmap -> SDF
    layer_start = web_time::Instant::now();
    diag.sdf_conversion.ran = true;
    let pad_x_usz = PAD_X as usize;
    for pz in 0..PAD_Z {
        let row_start = (pz as usize) * w;
        let height_row = &height_buf[row_start..row_start + pad_x_usz];
        for py in 0..PAD_Y {
            let wy = py as f32 - 1.0 + config.y_offset;
            let sdf_start = ChunkData::index(0, py, pz);
            let sdf_row = &mut chunk.sdf[sdf_start..sdf_start + pad_x_usz];
            for px in 0..pad_x_usz {
                sdf_row[px] = wy - height_row[px];
            }
        }
    }

    diag.layer_timings.sdf_conversion_ms = layer_start.elapsed().as_secs_f32() * 1000.0;

    // 3.5. Guarantee minimum terrain thickness (5 voxels below surface)
    layer_start = web_time::Instant::now();
    diag.min_thickness.ran = true;
    for pz in 0..PAD_Z {
        for px in 0..PAD_X {
            let mut surface_py: Option<u32> = None;
            for py in (0..PAD_Y).rev() {
                let idx = ChunkData::index(px, py, pz);
                if chunk.sdf[idx] < 0.0 {
                    surface_py = Some(py);
                    break;
                }
            }
            if let Some(spy) = surface_py {
                let min_solid = spy.saturating_sub(5);
                for py in min_solid..=spy {
                    let idx = ChunkData::index(px, py, pz);
                    if chunk.sdf[idx] > 0.0 {
                        chunk.sdf[idx] = -0.5;
                    }
                }
            }
        }
    }

    diag.layer_timings.min_thickness_ms = layer_start.elapsed().as_secs_f32() * 1000.0;

    // 3.6. Global SDF smoothing pass (2 iterations)
    layer_start = web_time::Instant::now();
    diag.sdf_smoothing.ran = true;
    {
        let smooth_passes = 2;
        for _ in 0..smooth_passes {
            let sdf_snapshot: Vec<f32> = chunk.sdf.to_vec();
            for pz in 1..PAD_Z - 1 {
                for px in 1..PAD_X - 1 {
                    let h = height_buf[px as usize + w * pz as usize];
                    let surface_py = ((h - config.y_offset) + 1.0) as i32;
                    let y_min = (surface_py - 4).max(1) as u32;
                    let y_max = (surface_py + 4)
                        .max(y_min as i32 + 8)
                        .min(PAD_Y as i32 - 2) as u32;

                    for py in y_min..=y_max {
                        let idx = ChunkData::index(px, py, pz);
                        let current = sdf_snapshot[idx];

                        let mut sum = 0.0f32;
                        sum += sdf_snapshot[ChunkData::index(px + 1, py, pz)];
                        sum += sdf_snapshot[ChunkData::index(px - 1, py, pz)];
                        sum += sdf_snapshot[ChunkData::index(px, py + 1, pz)];
                        sum += sdf_snapshot[ChunkData::index(px, py - 1, pz)];
                        sum += sdf_snapshot[ChunkData::index(px, py, pz + 1)];
                        sum += sdf_snapshot[ChunkData::index(px, py, pz - 1)];
                        let avg = sum / 6.0;

                        chunk.sdf[idx] = current + (avg - current) * 0.4;
                    }
                }
            }
        }
    }

    diag.layer_timings.sdf_smoothing_ms = layer_start.elapsed().as_secs_f32() * 1000.0;

    // 3.7. Cave carving via 3D Perlin noise overlay
    layer_start = web_time::Instant::now();
    if gen_config.is_some() && detail == GenDetail::Full && cave_worm_params.is_some() {
        let cave_perlin = Perlin::new(config.seed.wrapping_add(99999));
        let sea_level = gen_config.map_or(config.sea_level, |gc| gc.sea_level);
        let total_cols = (PAD_X * PAD_Z) as usize;
        let cp = &cave_perlin;
        let sdf_ref = &chunk.sdf;
        let hbuf = &height_buf;
        let bbuf = &biome_buf;
        let y_off = config.y_offset;
        let ox = origin.x;
        let oz = origin.z;

        let carves: Vec<(usize, f32)> = (0..total_cols)
            .into_par_iter()
            .flat_map_iter(move |col_idx| {
                let px = (col_idx as u32) % PAD_X;
                let pz = (col_idx as u32) / PAD_X;
                let wx = f64::from(ox + (px as f32 - 1.0));
                let wz = f64::from(oz + (pz as f32 - 1.0));
                let idx2d = px as usize + w * pz as usize;
                let height = hbuf[idx2d];
                let biome = bbuf[idx2d];
                let threshold = cave_threshold_for_biome(biome);

                let y_min_world = sea_level + 1.0;
                let y_max_world = height - 4.0;
                let mut out: Vec<(usize, f32)> = Vec::new();
                if y_min_world >= y_max_world {
                    return out.into_iter();
                }

                let py_start = ((y_min_world - y_off + 1.0).ceil() as u32).min(PAD_Y);
                let py_end = ((y_max_world - y_off + 1.0).floor() as u32 + 1).min(PAD_Y);
                if py_start >= py_end {
                    return out.into_iter();
                }

                for py in py_start..py_end {
                    let idx = ChunkData::index(px, py, pz);
                    if sdf_ref[idx] >= 0.0 {
                        continue;
                    }
                    let wy64 = f64::from(py as f32 - 1.0 + y_off);
                    let cave_val = cp.get([wx * 0.03, wy64 * 0.05, wz * 0.03]);
                    let cave2 = cp.get([wx * 0.07 + 99.0, wy64 * 0.12 + 99.0, wz * 0.07 + 99.0]) * 0.4;
                    let combined = cave_val + cave2;
                    if combined > threshold {
                        out.push((idx, (combined - threshold) as f32 * 3.0));
                    }
                }
                out.into_iter()
            })
            .collect();

        for (idx, v) in carves {
            chunk.sdf[idx] = v;
        }
    }

    diag.layer_timings.caves_perlin_ms = layer_start.elapsed().as_secs_f32() * 1000.0;

    // 3.8. Worm cave carving
    layer_start = web_time::Instant::now();
    if gen_config.is_some() && detail == GenDetail::Full {
        if let Some(worm_params) = cave_worm_params {
            let sea_level = gen_config.map_or(config.sea_level, |gc| gc.sea_level);
            carve_cave_worms(&mut chunk, config, coord, biome_map, worm_params, sea_level);
            diag.caves_worm.ran = true;
        }
    }

    diag.layer_timings.caves_worm_ms = layer_start.elapsed().as_secs_f32() * 1000.0;

    // 3.8b. Network tunnels
    layer_start = web_time::Instant::now();
    if detail == GenDetail::Full {
        if let Some(net) = cave_network {
            crate::cave_network::carve_network_tunnels(&mut chunk, config, coord, net);
            diag.caves_network.ran = true;
        }
    }
    diag.layer_timings.caves_network_ms = layer_start.elapsed().as_secs_f32() * 1000.0;

    // 3.9. Village caves
    layer_start = web_time::Instant::now();
    if detail == GenDetail::Full {
        if let Some(village_net) = village_network {
            carve_village_caves(&mut chunk, config, coord, village_net);
        }
    }

    if gen_config.is_some() && detail == GenDetail::Full {
        diag.caves_perlin.ran = true;
    }
    diag.layer_timings.village_caves_ms = layer_start.elapsed().as_secs_f32() * 1000.0;
    if detail == GenDetail::Full && village_network.is_some() {
        diag.village_caves.ran = true;
    }

    // Anomaly detection: SDF NaN check (sample center column)
    {
        use crate::pipeline_diag::AnomalyKind;
        let center_idx = (PAD_X / 2) as usize + w * (PAD_Z / 2) as usize;
        let center_h = height_buf.get(center_idx).copied().unwrap_or(0.0);
        if center_h.is_nan() || center_h.is_infinite() {
            diag.anomalies.push(format!(
                "{} chunk [{},{}] SDF source height NaN/Inf at column center",
                AnomalyKind::SdfCenterNan.prefix(), coord.x, coord.z,
            ));
        }
    }

    diag.gen_time_ms = gen_start.elapsed().as_secs_f32() * 1000.0;
    chunk.pipeline_diag = Some(diag);

    chunk.dirty = true;
    chunk.modified = false;
    chunk
}

/// Generate initial terrain chunks within streaming radius of a position.
#[allow(clippy::too_many_arguments)]
pub fn generate_initial_chunks(
    center: Vec3,
    config: &TerrainConfig,
    biome_map: &BiomeMap,
    chunk_manager: &mut ChunkManager,
    gen_config: Option<&MapGenConfig>,
    path_network: Option<&PathNetwork>,
    village_network: Option<&VillageNetwork>,
    castle_footprint: Option<&CastleFootprint>,
) {
    let center_coord = ChunkCoord::from_world(center);
    let r = config.streaming_radius.min(12);

    let mut count = 0u32;
    for dz in -r..=r {
        for dx in -r..=r {
            if dx * dx + dz * dz > r * r {
                continue;
            }

            let coord = ChunkCoord::new(center_coord.x + dx, center_coord.z + dz);

            if chunk_manager.chunks.contains_key(&coord) {
                continue;
            }

            let data = generate_chunk(coord, config, biome_map, gen_config, path_network, village_network, castle_footprint);
            chunk_manager.chunks.insert(coord, data);
            count += 1;
        }
    }

    info!("Generated {} initial terrain chunks around ({}, {})", count, center_coord.x, center_coord.z);
}

/// Clamp les pentes du heightmap > `max_slope_deg` en lissant les cellules
/// offensantes vers la moyenne des 4 voisins.
pub(crate) fn clamp_extreme_slopes(
    height_buf: &mut [f32],
    w: usize,
    d: usize,
    max_slope_deg: f32,
    passes: u32,
) {
    if w < 3 || d < 3 {
        return;
    }
    let max_grad = max_slope_deg.to_radians().tan();
    let max_grad_sq = max_grad * max_grad;

    for _pass in 0..passes {
        let mut changed = false;
        for z in 1..d - 1 {
            for x in 1..w - 1 {
                let idx = x + w * z;
                let hxp = height_buf[idx + 1];
                let hxm = height_buf[idx - 1];
                let hzp = height_buf[idx + w];
                let hzm = height_buf[idx - w];
                let gx = (hxp - hxm) * 0.5;
                let gz = (hzp - hzm) * 0.5;
                if gx * gx + gz * gz > max_grad_sq {
                    let avg = (hxp + hxm + hzp + hzm) * 0.25;
                    height_buf[idx] = avg;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_detail_default_is_full() {
        assert_eq!(GenDetail::default(), GenDetail::Full);
    }

    #[test]
    fn slope_clamp_smooths_spike() {
        let mut h = vec![0.0_f32; 25];
        h[12] = 50.0;
        super::clamp_extreme_slopes(&mut h, 5, 5, 75.0, 2);
        assert!(h[12] < 25.0, "center spike must be smoothed; got {}", h[12]);
    }

    #[test]
    fn slope_clamp_idempotent_on_flat() {
        let mut h = vec![10.0_f32; 16];
        let original = h.clone();
        super::clamp_extreme_slopes(&mut h, 4, 4, 75.0, 2);
        assert_eq!(h, original);
    }

    #[test]
    fn slope_clamp_passes_progressive_smoothing() {
        let make_grid = || {
            let mut h = vec![0.0_f32; 25];
            h[12] = 50.0;
            h
        };
        let mut h1 = make_grid();
        let mut h4 = make_grid();
        super::clamp_extreme_slopes(&mut h1, 5, 5, 65.0, 1);
        super::clamp_extreme_slopes(&mut h4, 5, 5, 65.0, 4);
        assert!(h4[12] <= h1[12], "4 passes ({}) doit être <= 1 passe ({})", h4[12], h1[12]);
    }

    #[test]
    fn slope_clamp_zero_passes_does_not_panic() {
        let mut h = vec![0.0_f32; 25];
        h[12] = 50.0;
        super::clamp_extreme_slopes(&mut h, 5, 5, 65.0, 0);
        assert!(h[12].is_finite());
    }

    #[test]
    fn gen_detail_variants_are_distinct() {
        assert_ne!(GenDetail::Full, GenDetail::Fast);
        assert_ne!(GenDetail::Full, GenDetail::Distant);
        assert_ne!(GenDetail::Fast, GenDetail::Distant);
    }
}
