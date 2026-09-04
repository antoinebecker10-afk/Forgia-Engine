//! village_data.rs — STUB enrichi pour W1 (port V1 complet à venir).
//!
//! Champs minimaux pour permettre la compilation de `carve_village_caves`
//! (chemin dormant en W1 — la fonction n'est invoquée par aucun pipeline actif).

#![allow(dead_code, unused_imports)]

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VillageData;

#[derive(Resource, Debug, Clone, Default)]
pub struct VillageRegistry;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VillageNetwork {
    pub villages: Vec<Village>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Village {
    /// `center.x` = x monde, `center.y` = z monde (convention V1).
    pub center: Vec2,
    pub target_height: f32,
}
