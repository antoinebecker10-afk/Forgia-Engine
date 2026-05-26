//! forgia-rpg-data — RPG data layer (fused 2026-05-26).
//!
//! Issu de la fusion de 5 single-file scaffold crates :
//! - `forgia-inventory` (199 LOC, LOCK-INV-1 80 slots)
//! - `forgia-quests` (203 LOC)
//! - `forgia-loot-tables` (129 LOC, drop tables genome-driven)
//! - `forgia-xp-curves` (146 LOC, level curves)
//! - `forgia-dialogue` (174 LOC, dialogue trees living_weapons)
//!
//! Toutes partageaient `bevy + forgia-core + serde` sans inter-dep. Coalescence
//! sous un même crate data-layer évite 5 unité-de-build distinctes pour 851 LOC.
//!
//! ## Modules
//!
//! Consommer via `forgia_rpg_data::dialogue::X`, `forgia_rpg_data::loot_tables::X`,
//! etc.

pub mod dialogue;
pub mod inventory;
pub mod loot_tables;
pub mod quests;
pub mod xp_curves;
