use bevy::prelude::*;

use crate::chunk::{TerrainConfig, CHUNK_X, CHUNK_Z};
use crate::map_gen_config::{BiomeMode, BiomeWeights, MapGenConfig};
use crate::biome_registry::BiomeRegistry;
use crate::worldmap::WorldMapIntent;

// ─────────────────────────── Biome Types ───────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BiomeType {
    Plains   = 0,
    Forest   = 1,
    Desert   = 2,
    Mountain = 3,
    Swamp    = 4,
    Tundra   = 5,
    Savanna  = 6,
    Jungle   = 7,
    Volcanic = 8,
    Canyon   = 9,
}

impl BiomeType {
    pub fn from_id(id: u8) -> Self {
        match id {
            0 => Self::Plains, 1 => Self::Forest, 2 => Self::Desert,
            3 => Self::Mountain, 4 => Self::Swamp, 5 => Self::Tundra,
            6 => Self::Savanna, 7 => Self::Jungle, 8 => Self::Volcanic,
            9 => Self::Canyon, _ => Self::Plains,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "plains" => Self::Plains, "forest" => Self::Forest,
            "desert" => Self::Desert, "mountain" => Self::Mountain,
            "swamp" => Self::Swamp, "tundra" => Self::Tundra,
            "savanna" => Self::Savanna, "jungle" => Self::Jungle,
            "volcanic" => Self::Volcanic, "canyon" => Self::Canyon,
            _ => Self::Plains,
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Self::Plains   => Color::srgb(0.42, 0.62, 0.22),
            Self::Forest   => Color::srgb(0.18, 0.38, 0.12),
            Self::Desert   => Color::srgb(0.85, 0.72, 0.42),
            Self::Mountain => Color::srgb(0.55, 0.52, 0.48),
            Self::Swamp    => Color::srgb(0.32, 0.40, 0.20),
            Self::Tundra   => Color::srgb(0.68, 0.74, 0.78),
            Self::Savanna  => Color::srgb(0.78, 0.68, 0.32),
            Self::Jungle   => Color::srgb(0.12, 0.32, 0.10),
            Self::Volcanic => Color::srgb(0.22, 0.15, 0.12),
            Self::Canyon   => Color::srgb(0.72, 0.40, 0.22),
        }
    }

    pub fn linear_rgba(&self) -> [f32; 4] {
        let c = self.color().to_linear();
        [c.red, c.green, c.blue, 1.0]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plains => "Plains", Self::Forest => "Forest",
            Self::Desert => "Desert", Self::Mountain => "Mountain",
            Self::Swamp => "Swamp", Self::Tundra => "Tundra",
            Self::Savanna => "Savanna", Self::Jungle => "Jungle",
            Self::Volcanic => "Volcanic", Self::Canyon => "Canyon",
        }
    }

    pub fn roughness(&self) -> f32 {
        match self {
            Self::Plains => 0.90, Self::Forest => 0.85, Self::Desert => 0.95,
            Self::Mountain => 0.92, Self::Swamp => 0.88, Self::Tundra => 0.80,
            Self::Savanna => 0.92, Self::Jungle => 0.82, Self::Volcanic => 0.95,
            Self::Canyon => 0.93,
        }
    }
}

// ─────────────────────────── BiomeSeed & BiomeBlend ───────────────────────────

pub const MAX_BLEND_BIOMES: usize = 4;

#[derive(Debug, Clone)]
pub struct BiomeSeed {
    pub position: Vec2,
    pub biome: BiomeType,
}

#[derive(Debug, Clone)]
pub struct BiomeBlend {
    pub biomes: [(BiomeType, f32); MAX_BLEND_BIOMES],
    pub count: usize,
}

// ─────────────────────────── BiomeMap ───────────────────────────

#[derive(Resource, Clone)]
pub struct BiomeMap {
    pub seeds: Vec<BiomeSeed>,
}

impl BiomeMap {
    pub fn generate(config: &TerrainConfig, gen_config: Option<&MapGenConfig>) -> Self {
        Self::generate_with_intent(config, gen_config, None)
    }

    pub fn generate_with_intent(
        config: &TerrainConfig,
        gen_config: Option<&MapGenConfig>,
        intent: Option<&WorldMapIntent>,
    ) -> Self {
        if let Some(gc) = gen_config {
            if let BiomeMode::Single { ref biome } = gc.biome_mode {
                let bt = BiomeType::from_name(biome);
                info!("BiomeMap: Single biome mode — {} over {:.0}m²", biome, config.map_size * config.map_size);
                return Self {
                    seeds: vec![BiomeSeed {
                        position: Vec2::new(config.map_size * 0.5, config.map_size * 0.5),
                        biome: bt,
                    }],
                };
            }

            if gc.biome_mode == BiomeMode::Altitude {
                return Self::generate_altitude(config, gc);
            }

            if let BiomeMode::Directional {
                ref center, ref north_west, ref north_east, ref south_east, ref south_west,
            } = gc.biome_mode {
                return Self::generate_directional(config, gc, center, north_west, north_east, south_east, south_west);
            }

            if gc.biome_mode == BiomeMode::LandmarkVoronoi {
                if let Some(intent_ref) = intent {
                    return Self::generate_landmark_voronoi(config, gc, intent_ref);
                }
                warn!("BiomeMap: LandmarkVoronoi mode demandé mais WorldMapIntent absent — fallback Voronoi random");
            }
        }

        let map_size = config.map_size;
        let hex_spacing = gen_config.map(|gc| gc.biome_cell_size.clamp(64.0, 512.0)).unwrap_or(192.0);
        let mut weights = gen_config.map(|gc| gc.biome_weights.clone()).unwrap_or_default();

        if weights == BiomeWeights::default() {
            let reg = BiomeRegistry::load();
            weights = BiomeWeights {
                plains: reg.spawn_weight(BiomeType::Plains),
                forest: reg.spawn_weight(BiomeType::Forest),
                desert: reg.spawn_weight(BiomeType::Desert),
                mountain: reg.spawn_weight(BiomeType::Mountain),
                swamp: reg.spawn_weight(BiomeType::Swamp),
                tundra: reg.spawn_weight(BiomeType::Tundra),
                savanna: reg.spawn_weight(BiomeType::Savanna),
                jungle: reg.spawn_weight(BiomeType::Jungle),
                volcanic: reg.spawn_weight(BiomeType::Volcanic),
                canyon: reg.spawn_weight(BiomeType::Canyon),
            };
        }
        let mut seeds = Vec::new();
        let mut rng_state: u64 = u64::from(config.seed) ^ 0xDEAD_BEEF_CAFE;
        let mut next_rng = || -> f32 {
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            (rng_state as f32 / u64::MAX as f32) * 2.0 - 1.0
        };

        let row_height = hex_spacing * 0.866;
        let mut row = 0;
        let mut z = 0.0f32;
        while z < map_size {
            let x_offset = if row % 2 == 0 { 0.0 } else { hex_spacing * 0.5 };
            let mut x = x_offset;
            while x < map_size {
                let jx = (x + next_rng() * hex_spacing * 0.3).clamp(0.0, map_size);
                let jz = (z + next_rng() * row_height * 0.3).clamp(0.0, map_size);
                let biome = select_biome_weighted(&weights, &mut next_rng);
                seeds.push(BiomeSeed { position: Vec2::new(jx, jz), biome });
                x += hex_spacing;
            }
            z += row_height;
            row += 1;
        }

        info!("BiomeMap: generated {} Voronoi seeds (cell={:.0}m) over {:.0}m²",
            seeds.len(), hex_spacing, config.map_size * config.map_size);
        Self { seeds }
    }

    fn generate_altitude(config: &TerrainConfig, gen_config: &MapGenConfig) -> Self {
        let map_size = config.map_size;
        let spacing = 64.0f32;
        let sea = gen_config.sea_level;
        let max_h = gen_config.max_height;
        let range = (max_h - sea).max(1.0);
        let perlin = noise::Perlin::new(config.seed);
        let mut seeds = Vec::new();
        let mut z = 0.0f32;
        while z < map_size {
            let mut x = 0.0f32;
            while x < map_size {
                let half = map_size / 2.0;
                let freq = f64::from(gen_config.base_frequency);
                let warp = f64::from(gen_config.warp_strength);
                use noise::NoiseFn;
                let h_raw = perlin.get([
                    (f64::from(x) + warp * perlin.get([f64::from(x) * freq + 5.2, f64::from(z) * freq + 1.3])) * freq,
                    (f64::from(z) + warp * perlin.get([f64::from(x) * freq + 9.1, f64::from(z) * freq + 4.7])) * freq,
                ]);
                let h01 = (h_raw as f32 + 1.0) * 0.5;
                let mut h = sea + h01 * range;
                if gen_config.island_mode {
                    let dist = ((x - half).powi(2) + (z - half).powi(2)).sqrt();
                    let max_r = half * 0.85;
                    let shore = max_r * 0.6;
                    if dist > shore {
                        let t = ((dist - shore) / (max_r - shore)).clamp(0.0, 1.0);
                        h *= 1.0 - t * t * (3.0 - 2.0 * t);
                    }
                }
                let alt = ((h - sea) / range).clamp(0.0, 1.0);
                let biome = if alt < 0.15 { BiomeType::Desert }
                    else if alt < 0.30 { BiomeType::Plains }
                    else if alt < 0.45 { BiomeType::Savanna }
                    else if alt < 0.60 { BiomeType::Forest }
                    else if alt < 0.75 { BiomeType::Mountain }
                    else if alt < 0.88 { BiomeType::Volcanic }
                    else { BiomeType::Tundra };
                seeds.push(BiomeSeed { position: Vec2::new(x, z), biome });
                x += spacing;
            }
            z += spacing;
        }
        info!("BiomeMap: Altitude mode — {} seeds over {:.0}m²", seeds.len(), map_size * map_size);
        Self { seeds }
    }

    #[allow(clippy::too_many_arguments)]
    fn generate_directional(
        config: &TerrainConfig, gen_config: &MapGenConfig,
        center_biome: &str, nw_biome: &str, ne_biome: &str, se_biome: &str, sw_biome: &str,
    ) -> Self {
        let map_size = config.map_size;
        let half = map_size * 0.5;
        let hex_spacing = gen_config.biome_cell_size.clamp(64.0, 512.0);
        let bt_center = BiomeType::from_name(center_biome);
        let bt_nw = BiomeType::from_name(nw_biome);
        let bt_ne = BiomeType::from_name(ne_biome);
        let bt_se = BiomeType::from_name(se_biome);
        let bt_sw = BiomeType::from_name(sw_biome);
        let transitions: &[(BiomeType, BiomeType, BiomeType)] = &[
            (bt_nw, bt_ne, BiomeType::Mountain),
            (bt_ne, bt_se, BiomeType::Canyon),
            (bt_se, bt_sw, BiomeType::Savanna),
            (bt_sw, bt_nw, BiomeType::Swamp),
        ];
        let center_radius_frac = 0.30;
        let mut rng_state: u64 = u64::from(config.seed) ^ 0xCAFE_BABE_1337;
        let mut next_rng = || -> f32 {
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            (rng_state as f32 / u64::MAX as f32) * 2.0 - 1.0
        };
        let mut seeds = Vec::new();
        let row_height = hex_spacing * 0.866;
        let mut row = 0;
        let mut z = 0.0f32;
        while z < map_size {
            let x_offset = if row % 2 == 0 { 0.0 } else { hex_spacing * 0.5 };
            let mut x = x_offset;
            while x < map_size {
                let jx = (x + next_rng() * hex_spacing * 0.25).clamp(0.0, map_size);
                let jz = (z + next_rng() * row_height * 0.25).clamp(0.0, map_size);
                let dx = jx - half;
                let dz = jz - half;
                let dist_frac = (dx * dx + dz * dz).sqrt() / half;
                let angle = dz.atan2(dx);
                let biome = if dist_frac < center_radius_frac {
                    bt_center
                } else {
                    let transition_half = 0.25;
                    let in_transition = |a: f32, boundary: f32| -> bool {
                        let diff = (a - boundary).abs();
                        let diff = if diff > std::f32::consts::PI { std::f32::consts::TAU - diff } else { diff };
                        diff < transition_half
                    };
                    let at_n = in_transition(angle, -std::f32::consts::FRAC_PI_2);
                    let at_e = in_transition(angle, 0.0);
                    let at_s = in_transition(angle, std::f32::consts::FRAC_PI_2);
                    let at_w = in_transition(angle, std::f32::consts::PI) || in_transition(angle, -std::f32::consts::PI);
                    if at_n && dist_frac > 0.3 { transitions[0].2 }
                    else if at_e && dist_frac > 0.3 { transitions[1].2 }
                    else if at_s && dist_frac > 0.3 { transitions[2].2 }
                    else if at_w && dist_frac > 0.3 { transitions[3].2 }
                    else if angle < -std::f32::consts::FRAC_PI_2 { bt_nw }
                    else if angle < 0.0 { bt_ne }
                    else if angle < std::f32::consts::FRAC_PI_2 { bt_se }
                    else { bt_sw }
                };
                seeds.push(BiomeSeed { position: Vec2::new(jx, jz), biome });
                x += hex_spacing;
            }
            z += row_height;
            row += 1;
        }
        info!("BiomeMap: Directional mode — {} seeds", seeds.len());
        Self { seeds }
    }

    fn generate_landmark_voronoi(
        config: &TerrainConfig, _gen_config: &MapGenConfig, intent: &WorldMapIntent,
    ) -> Self {
        let map_size = config.map_size;
        let mut seeds = Vec::with_capacity(280);
        let mut rng_state: u64 = intent.seed ^ 0xB10E_F0CE_5EED;
        let mut next_rng = || -> f32 {
            rng_state ^= rng_state << 13;
            rng_state ^= rng_state >> 7;
            rng_state ^= rng_state << 17;
            (rng_state as f32 / u64::MAX as f32) * 2.0 - 1.0
        };

        for lm in &intent.landmarks {
            if lm.is_detached() { continue; }
            let anchor_biome = lm.required_biome.unwrap_or_else(|| fallback_biome_by_quadrant(lm.x, lm.z));
            let world_pos = lm.world_pos(map_size);
            let anchor_xz = Vec2::new(world_pos.x, world_pos.z);
            seeds.push(BiomeSeed { position: anchor_xz, biome: anchor_biome });
            let footprint = lm.footprint_radius_m.max(80.0);
            let satellite_r = footprint * 2.0;
            for i in 0..3 {
                let angle = (i as f32) * 2.094_395_1 + next_rng() * 0.4;
                let offset = Vec2::new(angle.cos(), angle.sin()) * satellite_r;
                let pos = (anchor_xz + offset).clamp(Vec2::splat(0.0), Vec2::splat(map_size));
                seeds.push(BiomeSeed { position: pos, biome: anchor_biome });
            }
        }

        const CURATED_SEEDS: &[(BiomeType, f32, f32)] = &[
            (BiomeType::Mountain, 0.12, 0.12), (BiomeType::Mountain, 0.25, 0.10),
            (BiomeType::Forest,   0.22, 0.28), (BiomeType::Forest,   0.10, 0.28),
            (BiomeType::Mountain, 0.88, 0.10), (BiomeType::Mountain, 0.75, 0.08),
            (BiomeType::Forest,   0.85, 0.45), (BiomeType::Forest,   0.75, 0.58),
            (BiomeType::Forest,   0.90, 0.60), (BiomeType::Plains,   0.40, 0.50),
            (BiomeType::Plains,   0.60, 0.50), (BiomeType::Plains,   0.50, 0.40),
            (BiomeType::Plains,   0.50, 0.62), (BiomeType::Savanna,  0.12, 0.75),
            (BiomeType::Savanna,  0.30, 0.88), (BiomeType::Savanna,  0.25, 0.70),
            (BiomeType::Volcanic, 0.90, 0.90), (BiomeType::Volcanic, 0.75, 0.88),
            (BiomeType::Swamp,    0.55, 0.92), (BiomeType::Swamp,    0.65, 0.95),
            (BiomeType::Canyon,   0.72, 0.75),
        ];
        for (biome, nx, nz) in CURATED_SEEDS {
            seeds.push(BiomeSeed { position: Vec2::new(nx * map_size, nz * map_size), biome: *biome });
        }

        let hex_spacing = 256.0_f32;
        let row_height = hex_spacing * 0.866;
        let mut row = 0;
        let mut z = 0.0_f32;
        while z < map_size {
            let x_offset = if row % 2 == 0 { 0.0 } else { hex_spacing * 0.5 };
            let mut x = x_offset;
            while x < map_size {
                let jx = (x + next_rng() * hex_spacing * 0.25).clamp(0.0, map_size);
                let jz = (z + next_rng() * row_height * 0.25).clamp(0.0, map_size);
                let point = Vec2::new(jx, jz);
                let mut nearest_d = f32::MAX;
                let mut nearest_biome = BiomeType::Plains;
                for lm in &intent.landmarks {
                    if lm.is_detached() { continue; }
                    let lp = lm.world_pos(map_size);
                    let d = (point - Vec2::new(lp.x, lp.z)).length();
                    if d < nearest_d {
                        nearest_d = d;
                        nearest_biome = lm.required_biome.unwrap_or_else(|| fallback_biome_by_quadrant(lm.x, lm.z));
                    }
                }
                if nearest_d < 400.0 { x += hex_spacing; continue; }
                let filler = if nearest_d < 800.0 { nearest_biome } else { BiomeType::Forest };
                seeds.push(BiomeSeed { position: point, biome: filler });
                x += hex_spacing;
            }
            z += row_height;
            row += 1;
        }

        info!("BiomeMap: LandmarkVoronoi mode — {} seeds", seeds.len());
        Self { seeds }
    }

    pub fn biome_at(&self, x: f32, z: f32) -> BiomeType {
        let mut best_dist = f32::MAX;
        let mut best_biome = BiomeType::Plains;
        let pos = Vec2::new(x, z);
        for seed in &self.seeds {
            let d = pos.distance_squared(seed.position);
            if d < best_dist { best_dist = d; best_biome = seed.biome; }
        }
        best_biome
    }

    pub fn biome_blend_at(&self, x: f32, z: f32) -> (BiomeType, BiomeType, f32) {
        let blend = self.biome_weights_at(x, z, 4, 240.0);
        match blend.count {
            0 => (BiomeType::Plains, BiomeType::Plains, 0.0),
            1 => (blend.biomes[0].0, blend.biomes[0].0, 0.0),
            _ => { let w1 = blend.biomes[1].1; (blend.biomes[0].0, blend.biomes[1].0, w1) }
        }
    }

    pub fn biome_weights_at(&self, x: f32, z: f32, max_neighbors: usize, blend_radius: f32) -> BiomeBlend {
        let pos = Vec2::new(x, z);
        let max_n = max_neighbors.clamp(1, MAX_BLEND_BIOMES);
        let mut nearest: [(f32, BiomeType); MAX_BLEND_BIOMES] = [(f32::MAX, BiomeType::Plains); MAX_BLEND_BIOMES];
        for seed in &self.seeds {
            let d = pos.distance_squared(seed.position);
            if d < nearest[max_n - 1].0 {
                nearest[max_n - 1] = (d, seed.biome);
                for i in (1..max_n).rev() {
                    if nearest[i].0 < nearest[i - 1].0 { nearest.swap(i, i - 1); } else { break; }
                }
            }
        }
        let d_nearest = nearest[0].0.sqrt();
        let mut weights = [0.0f32; MAX_BLEND_BIOMES];
        let mut count = 0usize;
        let mut weight_sum = 0.0f32;
        for i in 0..max_n {
            if nearest[i].0 >= f32::MAX * 0.5 { break; }
            let d = nearest[i].0.sqrt();
            let delta = d - d_nearest;
            if delta < 0.01 { weights[i] = 1.0; }
            else if delta < blend_radius {
                let t = delta / blend_radius;
                let s = 1.0 - t;
                weights[i] = s * s * (3.0 - 2.0 * s);
            } else { weights[i] = 0.0; continue; }
            weight_sum += weights[i];
            count = i + 1;
        }
        let mut result = BiomeBlend { biomes: [(BiomeType::Plains, 0.0); MAX_BLEND_BIOMES], count };
        if weight_sum > 0.001 {
            for i in 0..count { result.biomes[i] = (nearest[i].1, weights[i] / weight_sum); }
        } else if count > 0 {
            result.biomes[0] = (nearest[0].1, 1.0);
            result.count = 1;
        }
        result
    }

    pub fn assign_chunk_biomes(&self, chunk_origin_x: f32, chunk_origin_z: f32) -> Vec<u8> {
        let mut biome_ids = vec![0u8; (CHUNK_X * CHUNK_Z) as usize];
        for cz in 0..CHUNK_Z {
            for cx in 0..CHUNK_X {
                let wx = chunk_origin_x + cx as f32 + 0.5;
                let wz = chunk_origin_z + cz as f32 + 0.5;
                let biome = self.biome_at(wx, wz);
                biome_ids[(cx + CHUNK_X * cz) as usize] = biome as u8;
            }
        }
        biome_ids
    }
}

// ─────────────────────────── Biome Selection ───────────────────────────

pub fn fallback_biome_by_quadrant(x: f32, z: f32) -> BiomeType {
    let dist_center = ((x - 0.5).powi(2) + (z - 0.5).powi(2)).sqrt();
    if dist_center < 0.12 { return BiomeType::Plains; }
    let dx = x - 0.5;
    let dz = z - 0.5;
    let angle_deg = dz.atan2(dx).to_degrees();
    match angle_deg {
        a if (-45.0..=45.0).contains(&a) => BiomeType::Forest,
        a if (45.0..135.0).contains(&a) => BiomeType::Volcanic,
        a if (-135.0..-45.0).contains(&a) => BiomeType::Tundra,
        _ => BiomeType::Desert,
    }
}

fn select_biome_weighted(weights: &BiomeWeights, rng: &mut impl FnMut() -> f32) -> BiomeType {
    let total = weights.total();
    if total <= 0.0 { return BiomeType::Plains; }
    let r = (rng() + 1.0) * 0.5 * total;
    let mut accum = 0.0;
    let biomes = [
        (BiomeType::Plains, weights.plains), (BiomeType::Forest, weights.forest),
        (BiomeType::Desert, weights.desert), (BiomeType::Mountain, weights.mountain),
        (BiomeType::Swamp, weights.swamp), (BiomeType::Tundra, weights.tundra),
        (BiomeType::Savanna, weights.savanna), (BiomeType::Jungle, weights.jungle),
        (BiomeType::Volcanic, weights.volcanic), (BiomeType::Canyon, weights.canyon),
    ];
    for (biome, w) in biomes {
        accum += w;
        if r <= accum { return biome; }
    }
    BiomeType::Plains
}

// ─────────────────────────── Biome Materials ───────────────────────────

#[derive(Resource)]
pub struct BiomeMaterials {
    pub materials: [Handle<StandardMaterial>; 10],
}

pub fn setup_biome_materials(
    mut commands: Commands,
    mut mats: ResMut<Assets<StandardMaterial>>,
    registry: Res<BiomeRegistry>,
) {
    let mut make = |biome: BiomeType| -> Handle<StandardMaterial> {
        mats.add(StandardMaterial {
            base_color: registry.color(biome),
            perceptual_roughness: registry.roughness(biome),
            metallic: 0.05,
            ..default()
        })
    };
    let materials = [
        make(BiomeType::Plains), make(BiomeType::Forest), make(BiomeType::Desert),
        make(BiomeType::Mountain), make(BiomeType::Swamp), make(BiomeType::Tundra),
        make(BiomeType::Savanna), make(BiomeType::Jungle), make(BiomeType::Volcanic),
        make(BiomeType::Canyon),
    ];
    commands.insert_resource(BiomeMaterials { materials });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_id_covers_all_ten_biomes() {
        use BiomeType::*;
        let expected = [Plains, Forest, Desert, Mountain, Swamp, Tundra, Savanna, Jungle, Volcanic, Canyon];
        for (id, expected_biome) in expected.iter().enumerate() {
            let got = BiomeType::from_id(id as u8);
            assert_eq!(std::mem::discriminant(&got), std::mem::discriminant(expected_biome));
        }
    }

    #[test]
    fn from_id_out_of_range_defaults_to_plains() {
        for bad in [10u8, 100, 255] {
            assert!(matches!(BiomeType::from_id(bad), BiomeType::Plains));
        }
    }

    #[test]
    fn from_name_case_insensitive() {
        assert!(matches!(BiomeType::from_name("FOREST"), BiomeType::Forest));
        assert!(matches!(BiomeType::from_name("Volcanic"), BiomeType::Volcanic));
    }

    #[test]
    fn color_all_biomes_in_valid_srgb_range() {
        for id in 0u8..10 {
            let c = BiomeType::from_id(id).color().to_srgba();
            assert!(c.red >= 0.0 && c.red <= 1.0);
            assert!(c.green >= 0.0 && c.green <= 1.0);
            assert!(c.blue >= 0.0 && c.blue <= 1.0);
        }
    }

    #[test]
    fn roughness_all_biomes_in_valid_range() {
        for id in 0u8..10 {
            let r = BiomeType::from_id(id).roughness();
            assert!(r > 0.0 && r <= 1.0);
        }
    }

    #[test]
    fn linear_rgba_alpha_is_always_one() {
        for id in 0u8..10 {
            let rgba = BiomeType::from_id(id).linear_rgba();
            assert_eq!(rgba[3], 1.0);
        }
    }

    #[test]
    fn fallback_biome_center_is_plains() {
        assert!(matches!(fallback_biome_by_quadrant(0.5, 0.5), BiomeType::Plains));
    }
}
