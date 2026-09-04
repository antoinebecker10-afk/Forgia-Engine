//! # forgia-juice-recoil
//!
//! Apex/COD-style camera recoil — data types partagés FPS/RPG.
//!
//! Le **système** `weapon_recoil_apply` reste dans `forgia-player` car il dépend
//! des types `Player` + `FpsCamera` (sinon dep cycle). Ce crate expose les
//! contrats data : `WeaponRecoilImpulse` (Message émis par fire) et
//! `WeaponRecoilDebt` (Resource decay state).
//!
//! Source de vérité Forgia V2 — extrait depuis `forgia-combat::combat_juice`
//! (Tier 1E refacto fine-grained-crates, 2026-05-17).

use bevy::prelude::*;

pub mod prelude {
    pub use super::{ForgiaJuiceRecoilPlugin, WeaponRecoilDebt, WeaponRecoilImpulse};
}

/// Impulse de recoil émis par le système de tir (1 par fire confirmé).
/// Pattern Apex/COD : push pitch ↑ + petit yaw random.
#[derive(Message)]
pub struct WeaponRecoilImpulse {
    pub pitch_rad: f32,
    pub yaw_rad: f32,
}

/// Dette de recoil en cours de remboursement (decay vers 0).
/// Auto-recenter caméra si pas d'input joueur.
#[derive(Resource, Default)]
pub struct WeaponRecoilDebt {
    pub pitch_rad: f32,
    pub yaw_rad: f32,
}

pub struct ForgiaJuiceRecoilPlugin;

impl Plugin for ForgiaJuiceRecoilPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponRecoilDebt>()
            .add_message::<WeaponRecoilImpulse>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaJuiceRecoilPlugin;
    }

    #[test]
    fn debt_default_zero() {
        let d = WeaponRecoilDebt::default();
        assert_eq!(d.pitch_rad, 0.0);
        assert_eq!(d.yaw_rad, 0.0);
    }
}
