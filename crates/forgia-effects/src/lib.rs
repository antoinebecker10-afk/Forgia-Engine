//! # forgia-effects
//!
//! Hanabi VFX + audio combat + HitFlashCache.
//!
//! **Pattern obligatoire Phase 0** : pre-spawn dummy `ParticleEffect` `Visibility::Hidden`
//! au Startup pour payer le shader compile lazy AVANT le 1er tir.
//! Sinon freeze 25s confirmé V1 (story-436).
//!
//! Modules portés V1 :
//! - `weapon_vfx/` : muzzle flash (5-layer), impact (3-layer), tracers cache
//! - `arena_feedback` : SFX kill confirm + damage player (story-427)
//! - (combat_juice vit dans forgia-combat car couplé à CombatHitEvent)

use bevy::prelude::*;
use forgia_core::prelude::*;

pub mod weapon_vfx;
pub mod arena_feedback;

// TODO: port from V1 effects/ — les modules suivants nécessitent des deps supplémentaires
// pub mod fireball_vfx;   // needs forgia-player (Fireball, IceBolt, GoblinOnFire, etc.)
// pub mod ice_vfx;        // needs forgia-player
// pub mod shield_vfx;     // needs forgia-player
// pub mod levelup_vfx;    // needs forgia-combat (LevelUpEvent)
// pub mod boss_vfx;       // needs forgia-player (GoblinBoss)
// pub mod biome_particles; // needs forgia-terrain (BiomeMap)
// pub mod weather;         // needs forgia-terrain (BiomeMap)
// pub mod biome_ambiance;  // needs forgia-terrain (BiomeMap)
// pub mod particles;       // needs forgia-terrain (VillageBuildingInstance)
// pub mod vignette;        // needs forgia-core pp
// pub mod wind;            // needs genome
// pub mod fade_in;         // needs forgia-core (FadeIn component)

pub mod prelude {
    pub use crate::ForgiaEffectsPlugin;
    pub use crate::weapon_vfx::{WeaponVfxEffects, MuzzleVfxMarker, ImpactVfxMarker};
    pub use crate::arena_feedback::{ArenaFeedbackPlugin, ArenaFeedbackStats};
}

pub struct ForgiaEffectsPlugin;

impl Plugin for ForgiaEffectsPlugin {
    fn build(&self, app: &mut App) {
        // Pre-spawn hanabi dummies at Startup to pay shader compile cost
        // BEFORE first shot (pattern story-436 / reference_hanabi_shader_compile_lazy_pattern.md)
        app.add_systems(Startup, prespawn_hanabi_dummies)
            .add_systems(Startup, weapon_vfx::setup_weapon_vfx)
            .add_plugins(arena_feedback::ArenaFeedbackPlugin)
            .add_systems(Update, effects_tick.in_set(GameSet::Effects));
    }
}

fn prespawn_hanabi_dummies(mut _commands: Commands) {
    // TODO: Phase 2 — spawn 8 dummy `ParticleEffect` `Visibility::Hidden` at Y=-10000
    // to pay shader compile cost BEFORE first shot.
    // Pattern: memory `reference_hanabi_shader_compile_lazy_pattern.md`
    info!("[forgia-effects] prespawn_hanabi_dummies — placeholder (Phase 2 adds 8 dummies)");
}

fn effects_tick() {
    // TODO: Phase 2 — Lifetime cleanup tick for MuzzleVfxMarker + ImpactVfxMarker entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaEffectsPlugin;
    }
}
