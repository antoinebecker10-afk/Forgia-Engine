//! forgia-oxr — Thin wrapper around `bevy_mod_xr` 0.5 (semi-generic XR API).
//!
//! Provides VR/AR scaffold for V2/V3. Plugin is a no-op stub in V1.

pub use bevy_mod_xr::*;

use bevy::prelude::*;

pub struct ForgiaOxrPlugin;

impl Plugin for ForgiaOxrPlugin {
    fn build(&self, _app: &mut App) {
        // V1 no-op : XR backend selection (OpenXR / WebXR) is per-platform.
        // V2 : add `bevy_openxr::OpenXrPlugin` behind feature flag.
    }
}
