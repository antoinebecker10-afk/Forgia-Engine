//! village_data.rs — STUB pending V1 port.

#![allow(dead_code, unused_imports)]

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VillageData;

#[derive(Resource, Debug, Clone, Default)]
pub struct VillageRegistry;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VillageNetwork;
