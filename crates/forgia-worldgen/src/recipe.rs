//! Recipe layer — a settlement described as **data** (story-578 P2).
//!
//! A [`HamletRecipe`] is the genome of a small settlement: grid size, spacing, which module
//! roles to use, density, seed. It is loaded from TOML and **hot-reloadable** — edit the file,
//! press F7 (or Shift+F12) and the hamlet regenerates. This is the "recipe → world" brick:
//! no hardcoded layout, variety comes from the data.
//!
//! P2 keeps the layout deliberately simple (a jittered grid). Roads / parcels / grammar land
//! in P3-P5.

use crate::registry::AssetRole;
use serde::Deserialize;

/// Data-driven description of a small grid hamlet.
///
/// `#[serde(default)]` makes every field optional in the TOML — a partial or missing file
/// still yields a sane hamlet via [`Default`].
#[derive(Deserialize, Clone, Debug)]
#[serde(default)]
pub struct HamletRecipe {
    /// Deterministic seed (same seed → same hamlet).
    pub seed: u64,
    /// Grid columns / rows.
    pub grid_cols: u32,
    pub grid_rows: u32,
    /// Distance between cell centers (meters).
    pub cell_size: f32,
    /// Max random per-module offset within its cell (meters).
    pub jitter: f32,
    /// Uniform module scale.
    pub scale: f32,
    /// Probability an interior cell receives a building.
    pub fill_chance: f32,
    /// Random 90° yaw per module.
    pub yaw_random: bool,
    /// Roles drawn for the body of the hamlet (buildings / props).
    pub building_roles: Vec<AssetRole>,
    /// Place a module from `border_role` on the perimeter (a fence/wall ring).
    pub border: bool,
    /// Role used for the perimeter ring.
    pub border_role: AssetRole,
}

impl Default for HamletRecipe {
    fn default() -> Self {
        Self {
            seed: 1337,
            grid_cols: 6,
            grid_rows: 5,
            cell_size: 13.0,
            jitter: 2.5,
            scale: 0.6,
            fill_chance: 0.65,
            yaw_random: true,
            building_roles: vec![AssetRole::Prop, AssetRole::Pillar],
            border: true,
            border_role: AssetRole::Wall,
        }
    }
}

/// Load a recipe from a TOML file. Falls back to [`HamletRecipe::default`] on any read/parse
/// error (never blocks generation — the demo always produces something).
pub fn load_recipe(path: &str) -> HamletRecipe {
    match std::fs::read_to_string(path) {
        Ok(s) => match toml::from_str::<HamletRecipe>(&s) {
            Ok(r) => r,
            Err(e) => {
                bevy::log::error!("[worldgen] parse {path}: {e} → default recipe");
                HamletRecipe::default()
            }
        },
        Err(e) => {
            bevy::log::warn!("[worldgen] read {path}: {e} → default recipe");
            HamletRecipe::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_recipe_is_sane() {
        let r = HamletRecipe::default();
        assert!(r.grid_cols >= 1 && r.grid_rows >= 1);
        assert!(r.cell_size > 0.0 && r.scale > 0.0);
        assert!(!r.building_roles.is_empty());
        assert!((0.0..=1.0).contains(&r.fill_chance));
    }

    #[test]
    fn shipped_hamlet_toml_parses() {
        let s = include_str!("../../../assets/genomes/worldgen/hamlet.toml");
        let r: HamletRecipe = toml::from_str(s).expect("hamlet.toml must parse");
        assert!(!r.building_roles.is_empty());
        assert!(r.grid_cols >= 1 && r.grid_rows >= 1);
    }
}
