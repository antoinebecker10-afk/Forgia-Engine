//! worldmap.rs — STUB pending V1 port.

#![allow(dead_code, unused_imports)]

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldMap;

#[derive(Resource, Debug, Clone, Default)]
pub struct WorldMapResource(pub WorldMap);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorldMapIntent;
