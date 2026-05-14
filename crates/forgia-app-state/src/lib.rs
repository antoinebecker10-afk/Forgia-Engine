//! forgia-app-state — Thin alias re-exporting States from `forgia-core`.
//!
//! Canonical source of truth lives in `forgia-core::states`. This crate exists
//! for naming-consistency in the capability catalogue (1 capability = 1 crate)
//! but does NOT duplicate the type definitions.

pub use forgia_core::states::{AppMode, GameMode, WorldMode};
pub use forgia_core::ForgiaCorePlugin as ForgiaAppStatePlugin;
