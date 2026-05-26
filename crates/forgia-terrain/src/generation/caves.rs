//! Caves — carving primitives + per-biome cave probability + worm tunnels +
//! village Skyrim-style cave systems.
//!
//! Sections :
//! - Thresholds : `cave_threshold_for_biome` pour le 3D noise carve pass
//! - Carving primitive : `carve_sphere` (utilise aussi par `cave_network.rs` →
//!   `pub(crate)` visibility)
//! - Worm network : `carve_cave_worms` + `CaveWormParams`, deterministic random
//!   walk avec chambres probabilistes
//! - Village caves : `carve_village_caves`, Skyrim-style entrance + shaft +
//!   chamber + 4 branches (garantit l'accessibilite des galleries underground)

use crate::biomes::BiomeType;
use crate::village_data::VillageNetwork;

/// Per-biome cave threshold for 3D noise carving.
/// Lower threshold = more caves. Values above 1.0 effectively disable caves.
pub(super) fn cave_threshold_for_biome(biome: BiomeType) -> f64 {
    match biome {
        BiomeType::Mountain => 0.35, // Very frequent caves
        BiomeType::Canyon => 0.40,   // Frequent — gorge cavities
        BiomeType::Volcanic => 0.45, // Lava tubes
        BiomeType::Forest => 0.55,   // Moderate
        BiomeType::Jungle => 0.55,   // Moderate
        BiomeType::Swamp => 0.60,    // Occasional
        _ => 0.65,                   // Rare (Plains, Desert, Tundra, Savanna)
    }
}

/// Cave worm parameters, read from FpsTuning genome fields.
#[derive(Clone, Debug)]
pub struct CaveWormParams {
    pub enabled: bool,
    pub worm_radius: f32,
    pub worm_length: f32,
    pub chamber_prob: f32,
    pub chamber_radius_min: f32,
    pub chamber_radius_max: f32,
    pub min_height: f32,
    pub max_height: f32,
    pub layers: u32,
}

impl Default for CaveWormParams {
    fn default() -> Self {
        Self {
            enabled: true,
            worm_radius: 3.5,
            worm_length: 80.0,
            chamber_prob: 0.15,
            chamber_radius_min: 6.0,
            chamber_radius_max: 20.0,
            min_height: 5.0,
            max_height: 60.0,
            layers: 2,
        }
    }
}

/// Carve worm tunnels + chambers into chunk SDF.
pub fn carve_cave_worms(
    chunk: &mut crate::chunk::ChunkData,
    config: &crate::chunk::TerrainConfig,
    coord: crate::chunk::ChunkCoord,
    biome_map: &crate::biomes::BiomeMap,
    params: &CaveWormParams,
    sea_level: f32,
) {
    if !params.enabled {
        return;
    }

    let origin = coord.world_origin();
    let pad_x = crate::chunk::PAD_X;
    let pad_y = crate::chunk::PAD_Y;
    let pad_z = crate::chunk::PAD_Z;

    let center_biome = biome_map.biome_at(
        origin.x + crate::chunk::CHUNK_X as f32 * 0.5,
        origin.z + crate::chunk::CHUNK_Z as f32 * 0.5,
    );
    let cave_prob = cave_probability_for_biome(center_biome);

    let chunk_seed = config
        .seed
        .wrapping_add(coord.x.wrapping_mul(1337) as u32)
        .wrapping_add(coord.z.wrapping_mul(7919) as u32)
        .wrapping_add(0xCAFE_0001);

    let mut rng = u64::from(chunk_seed);
    let next_f32 = |state: &mut u64| -> f32 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state & 0xFFFFFF) as f32 / 0xFFFFFF as f32
    };

    let base_worms = (cave_prob * params.layers as f32 * 2.0).ceil() as usize;
    if base_worms == 0 {
        return;
    }

    if next_f32(&mut rng) > cave_prob {
        return;
    }

    for layer in 0..params.layers {
        let layer_frac = layer as f32 / params.layers as f32;
        let y_center =
            params.min_height + (params.max_height - params.min_height) * (0.3 + layer_frac * 0.5);

        for _ in 0..base_worms.max(1) {
            let mut wx = origin.x + next_f32(&mut rng) * crate::chunk::CHUNK_X as f32;
            let mut wy = y_center + (next_f32(&mut rng) - 0.5) * 15.0;
            let mut wz = origin.z + next_f32(&mut rng) * crate::chunk::CHUNK_Z as f32;

            let mut dx = next_f32(&mut rng) - 0.5;
            let mut dy = (next_f32(&mut rng) - 0.5) * 0.3;
            let mut dz = next_f32(&mut rng) - 0.5;
            let dlen = (dx * dx + dy * dy + dz * dz).sqrt().max(0.001);
            dx /= dlen;
            dy /= dlen;
            dz /= dlen;

            let step_size = params.worm_radius * 0.8;
            let steps = (params.worm_length / step_size).ceil() as usize;

            for step in 0..steps {
                if wy < sea_level + 2.0 || wy > params.max_height {
                    break;
                }

                let radius = params.worm_radius * (0.8 + next_f32(&mut rng) * 0.4);
                carve_sphere(
                    chunk, config, origin, pad_x, pad_y, pad_z, wx, wy, wz, radius,
                );

                if step % 8 == 4 && next_f32(&mut rng) < params.chamber_prob {
                    let cr = params.chamber_radius_min
                        + next_f32(&mut rng)
                            * (params.chamber_radius_max - params.chamber_radius_min);
                    carve_sphere(chunk, config, origin, pad_x, pad_y, pad_z, wx, wy, wz, cr);
                }

                dx += (next_f32(&mut rng) - 0.5) * 0.4;
                dy += (next_f32(&mut rng) - 0.5) * 0.15;
                dz += (next_f32(&mut rng) - 0.5) * 0.4;
                let dlen = (dx * dx + dy * dy + dz * dz).sqrt().max(0.001);
                dx /= dlen;
                dy /= dlen;
                dz /= dlen;

                dy = dy.clamp(-0.3, 0.3);

                wx += dx * step_size;
                wy += dy * step_size;
                wz += dz * step_size;
            }
        }
    }
}

/// Carve a sphere into the SDF — sets voxels to positive (air) within radius.
#[allow(clippy::too_many_arguments)]
pub(crate) fn carve_sphere(
    chunk: &mut crate::chunk::ChunkData,
    config: &crate::chunk::TerrainConfig,
    origin: bevy::math::Vec3,
    pad_x: u32,
    pad_y: u32,
    pad_z: u32,
    cx: f32,
    cy: f32,
    cz: f32,
    radius: f32,
) {
    let r2 = radius * radius;
    let lx_center = cx - origin.x + 1.0;
    let ly_center = cy - config.y_offset + 1.0;
    let lz_center = cz - origin.z + 1.0;

    // CRITICAL: clamp to 0 BEFORE casting to u32 — a negative i32 cast to u32
    // wraps to a huge value (~4 billion), causing infinite loops.
    let px_min = ((lx_center - radius).floor() as i32).clamp(0, pad_x as i32) as u32;
    let px_max = ((lx_center + radius).ceil() as i32 + 1).clamp(0, pad_x as i32) as u32;
    let py_min = ((ly_center - radius).floor() as i32).clamp(0, pad_y as i32) as u32;
    let py_max = ((ly_center + radius).ceil() as i32 + 1).clamp(0, pad_y as i32) as u32;
    let pz_min = ((lz_center - radius).floor() as i32).clamp(0, pad_z as i32) as u32;
    let pz_max = ((lz_center + radius).ceil() as i32 + 1).clamp(0, pad_z as i32) as u32;

    for pz in pz_min..pz_max {
        for py in py_min..py_max {
            for px in px_min..px_max {
                let dx = px as f32 - lx_center;
                let dy = py as f32 - ly_center;
                let dz = pz as f32 - lz_center;
                let dist2 = dx * dx + dy * dy + dz * dz;
                if dist2 < r2 {
                    let idx = crate::chunk::ChunkData::index(px, py, pz);
                    let dist = dist2.sqrt();
                    let sdf_val = (dist - radius).max(0.1);
                    if chunk.sdf[idx] < sdf_val {
                        chunk.sdf[idx] = sdf_val;
                    }
                }
            }
        }
    }
}

/// Carve a Skyrim-style cave system near each village.
pub fn carve_village_caves(
    chunk: &mut crate::chunk::ChunkData,
    config: &crate::chunk::TerrainConfig,
    coord: crate::chunk::ChunkCoord,
    village_network: &VillageNetwork,
) {
    let origin = coord.world_origin();
    let pad_x = crate::chunk::PAD_X;
    let pad_y = crate::chunk::PAD_Y;
    let pad_z = crate::chunk::PAD_Z;
    let chunk_w = crate::chunk::CHUNK_X as f32;
    let chunk_d = crate::chunk::CHUNK_Z as f32;

    const CAVE_INFLUENCE_RADIUS: f32 = 120.0;

    let chunk_min = bevy::math::Vec2::new(origin.x, origin.z);
    let chunk_max = bevy::math::Vec2::new(origin.x + chunk_w, origin.z + chunk_d);

    for (idx, village) in village_network.villages.iter().enumerate() {
        let cx = village.center.x.clamp(chunk_min.x, chunk_max.x);
        let cz = village.center.y.clamp(chunk_min.y, chunk_max.y);
        let dx = village.center.x - cx;
        let dz = village.center.y - cz;
        if dx * dx + dz * dz > CAVE_INFLUENCE_RADIUS * CAVE_INFLUENCE_RADIUS {
            continue;
        }

        let village_seed = u64::from(
            config
                .seed
                .wrapping_add(idx as u32 * 31337)
                .wrapping_add(0xCAFE_BABE),
        );

        let mut rng = village_seed;
        let mut next_f32 = || -> f32 {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng & 0xFFFFFF) as f32 / 0xFFFFFF as f32
        };

        let entrance_angle = next_f32() * std::f32::consts::TAU;
        let entrance_dist = 35.0;
        let entrance_x = village.center.x + entrance_angle.cos() * entrance_dist;
        let entrance_z = village.center.y + entrance_angle.sin() * entrance_dist;
        let surface_y = village.target_height;

        let chamber_dist = 30.0;
        let chamber_angle = entrance_angle + std::f32::consts::PI;
        let chamber_x = village.center.x + chamber_angle.cos() * chamber_dist;
        let chamber_z = village.center.y + chamber_angle.sin() * chamber_dist;
        let chamber_y = surface_y - 18.0;

        // 1. ENTRANCE
        carve_sphere(
            chunk,
            config,
            origin,
            pad_x,
            pad_y,
            pad_z,
            entrance_x,
            surface_y - 1.0,
            entrance_z,
            5.0,
        );
        carve_sphere(
            chunk,
            config,
            origin,
            pad_x,
            pad_y,
            pad_z,
            entrance_x,
            surface_y + 1.5,
            entrance_z,
            4.0,
        );

        // 2. SLOPED TUNNEL
        const SLOPE_STEPS: usize = 18;
        let approach_x = chamber_x;
        let approach_z = chamber_z;
        let tunnel_top_y = surface_y - 4.0;
        let tunnel_bottom_y = chamber_y + 5.0;
        for step in 0..=SLOPE_STEPS {
            let t = step as f32 / SLOPE_STEPS as f32;
            let sx = entrance_x + (approach_x - entrance_x) * t;
            let sz = entrance_z + (approach_z - entrance_z) * t;
            let sy = tunnel_top_y + (tunnel_bottom_y - tunnel_top_y) * t;
            carve_sphere(chunk, config, origin, pad_x, pad_y, pad_z, sx, sy, sz, 3.5);
        }

        // 3. VERTICAL SHAFT
        const SHAFT_STEPS: usize = 8;
        let shaft_top_y = tunnel_bottom_y;
        let shaft_bottom_y = chamber_y;
        for step in 0..=SHAFT_STEPS {
            let t = step as f32 / SHAFT_STEPS as f32;
            let sy = shaft_top_y + (shaft_bottom_y - shaft_top_y) * t;
            carve_sphere(
                chunk, config, origin, pad_x, pad_y, pad_z, chamber_x, sy, chamber_z, 3.5,
            );
        }

        // 4. MAIN CHAMBER
        carve_sphere(
            chunk, config, origin, pad_x, pad_y, pad_z, chamber_x, chamber_y, chamber_z, 18.0,
        );
        carve_sphere(
            chunk,
            config,
            origin,
            pad_x,
            pad_y,
            pad_z,
            chamber_x,
            chamber_y + 6.0,
            chamber_z,
            12.0,
        );
        carve_sphere(
            chunk,
            config,
            origin,
            pad_x,
            pad_y,
            pad_z,
            chamber_x,
            chamber_y - 3.0,
            chamber_z,
            14.0,
        );

        // 5. BRANCH TUNNELS
        for branch in 0..4 {
            let branch_angle = (branch as f32) * std::f32::consts::FRAC_PI_2 + entrance_angle * 0.3;
            let bdx = branch_angle.cos();
            let bdz = branch_angle.sin();
            const TUNNEL_LENGTH: f32 = 60.0;
            const TUNNEL_STEPS: usize = 18;
            for step in 1..=TUNNEL_STEPS {
                let t = step as f32 / TUNNEL_STEPS as f32;
                let dist = t * TUNNEL_LENGTH;
                let wobble = (next_f32() - 0.5) * 4.0;
                let bx = chamber_x + bdx * dist + (-bdz) * wobble;
                let bz = chamber_z + bdz * dist + bdx * wobble;
                let by = chamber_y - t * 3.0;
                carve_sphere(chunk, config, origin, pad_x, pad_y, pad_z, bx, by, bz, 3.5);

                if step == TUNNEL_STEPS / 3 || step == 2 * TUNNEL_STEPS / 3 {
                    carve_sphere(chunk, config, origin, pad_x, pad_y, pad_z, bx, by, bz, 6.0);
                }
            }
        }
    }
}

/// Per-biome cave probability — reads from genome overrides if available.
pub(super) fn cave_probability_for_biome(biome: BiomeType) -> f32 {
    match biome {
        BiomeType::Volcanic => 0.8,
        BiomeType::Mountain => 0.7,
        BiomeType::Canyon => 0.6,
        BiomeType::Forest => 0.3,
        BiomeType::Jungle => 0.3,
        BiomeType::Tundra => 0.25,
        BiomeType::Plains => 0.2,
        BiomeType::Swamp => 0.15,
        BiomeType::Desert => 0.1,
        BiomeType::Savanna => 0.15,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cave_threshold_mountain_most_frequent() {
        assert!(
            cave_threshold_for_biome(BiomeType::Mountain)
                < cave_threshold_for_biome(BiomeType::Plains)
        );
        assert!(
            cave_threshold_for_biome(BiomeType::Volcanic)
                < cave_threshold_for_biome(BiomeType::Desert)
        );
    }

    #[test]
    fn cave_probability_ordering_matches_lore() {
        let volcanic = cave_probability_for_biome(BiomeType::Volcanic);
        let mountain = cave_probability_for_biome(BiomeType::Mountain);
        let plains = cave_probability_for_biome(BiomeType::Plains);
        let desert = cave_probability_for_biome(BiomeType::Desert);
        assert!(
            volcanic > mountain,
            "Volcanic should beat Mountain in cave freq"
        );
        assert!(mountain > plains, "Mountain should beat Plains");
        assert!(plains > desert, "Plains should beat Desert (driest biome)");
    }

    #[test]
    fn cave_probability_in_unit_range() {
        for b in [
            BiomeType::Plains,
            BiomeType::Forest,
            BiomeType::Desert,
            BiomeType::Mountain,
            BiomeType::Swamp,
            BiomeType::Tundra,
            BiomeType::Savanna,
            BiomeType::Jungle,
            BiomeType::Volcanic,
            BiomeType::Canyon,
        ] {
            let p = cave_probability_for_biome(b);
            assert!(
                (0.0..=1.0).contains(&p),
                "{b:?} cave_probability {p} outside [0,1]"
            );
        }
    }

    #[test]
    fn cave_worm_params_default_is_enabled_with_sensible_range() {
        let p = CaveWormParams::default();
        assert!(p.enabled);
        assert!(p.worm_radius > 0.0 && p.worm_length > 0.0);
        assert!(p.chamber_radius_min > 0.0 && p.chamber_radius_min < p.chamber_radius_max);
        assert!(p.min_height < p.max_height);
        assert!(p.layers > 0);
        assert!((0.0..=1.0).contains(&p.chamber_prob));
    }
}
