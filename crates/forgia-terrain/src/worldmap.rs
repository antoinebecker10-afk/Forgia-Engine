//! worldmap.rs — STUB enrichi pour W1 (port V1 complet à venir).
//!
//! Champs minimaux fournis pour permettre la compilation du chemin dormant
//! `BiomeMode::LandmarkVoronoi` (jamais sélectionné en W1 — biome_mode = Voronoi).

#![allow(dead_code, unused_imports)]

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::biomes::BiomeType;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldMap;

#[derive(Resource, Debug, Clone, Default)]
pub struct WorldMapResource(pub WorldMap);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldMapIntent {
    pub seed: u64,
    pub landmarks: Vec<Landmark>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Landmark {
    pub x: f32,
    pub z: f32,
    pub footprint_radius_m: f32,
    pub required_biome: Option<BiomeType>,
}

impl Landmark {
    pub fn is_detached(&self) -> bool { false }
    pub fn world_pos(&self, map_size: f32) -> Vec3 {
        Vec3::new(self.x * map_size, 0.0, self.z * map_size)
    }
}
