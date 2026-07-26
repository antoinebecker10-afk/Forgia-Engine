//! forgia-juice-lib — Consolidated game-feel library (story-522 fusion).
//!
//! 4 modules ex-crates :
//! - `recoil` (weapon recoil pattern + decay)
//! - `fov_punch` (FOV zoom feedback)
//! - `camera_shake` (procedural shake)
//! - `screen_flash` (full-screen color flash on damage/kill)

pub mod recoil;
pub mod fov_punch;
pub mod camera_shake;
pub mod knockback;

use bevy::prelude::*;

/// Meta-plugin bundling 4 juice sub-systems (screen_flash reste séparée car elle
/// dépend de forgia-combat → cycle si fusionnée ici, story-522).
pub struct ForgiaJuiceLibPlugin;

impl Plugin for ForgiaJuiceLibPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            recoil::ForgiaJuiceRecoilPlugin,
            fov_punch::ForgiaJuiceFovPunchPlugin,
            camera_shake::ForgiaJuiceCameraShakePlugin,
            knockback::ForgiaJuiceKnockbackPlugin,
        ));
    }
}
