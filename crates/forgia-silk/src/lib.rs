//! forgia-silk — Cloth physics (STUB).
//!
//! `bevy_silk` 0.10 targets Bevy 0.16. Forgia is on 0.18, so the upstream crate
//! cannot be used directly. Two options when needed :
//! 1. Wait for upstream 0.18 release
//! 2. Vendor the source from Renzora's `crates/bevy_silk/` and adapt to 0.18 API
//!
//! V1 status : STUB. Capes / banners use simpler animated meshes for now.

use bevy::prelude::*;

pub struct ForgiaSilkPlugin;

impl Plugin for ForgiaSilkPlugin {
    fn build(&self, _app: &mut App) {
        // No-op until upstream supports Bevy 0.18.
    }
}
