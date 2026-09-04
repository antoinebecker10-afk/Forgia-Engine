//! # forgia-juice-fov-punch
//!
//! FOV punch on shoot — petit zoom-out instantané puis recovery doux.
//!
//! ## Design AAA-safe 2024-2025 (veille 2026-05-18)
//!
//! - **PAS pour AR/SMG** (CS2 philosophy "precise aiming should be rewarded, not
//!   visually punished") → genome arme = 0.0 deg = skip
//! - **0.5-1.5° delta** uniquement pour shotguns / snipers / LMG (sensation poids)
//! - **Attack 30ms** rapide, **decay 150ms ease-out cubic**
//! - **Accessibility slider** [`FovPunchIntensity`] 0-100% (XAG-117)
//!
//! ## Architecture data-driven
//!
//! Ce crate expose les types Message + State + decay system. L'application
//! finale de l'offset sur `Projection.fov` est faite par le crate FPS owner
//! (forgia-fps/ads.rs) qui a accès à `FpsCamera` marker — évite dep cycle.

use bevy::prelude::*;

/// Impulse émise par un système gameplay (fire weapon, explosion).
/// Ajoute un peak FOV au state. Si le state est déjà en peak, on prend max.
#[derive(Message, Debug, Clone, Copy)]
pub struct FovPunchImpulse {
    /// Amplitude peak en degrés. Typiquement 0.5-1.5 selon arme.
    pub peak_deg: f32,
}

/// Resource accessibility — slider 0-100% (XAG-117).
#[derive(Resource, Debug, Clone, Copy)]
pub struct FovPunchIntensity(pub f32);

impl Default for FovPunchIntensity {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Resource Tuning (hot-reload). Push depuis fps_tuning.toml par forgia-fps.
/// Defaults : 30ms attack / 150ms decay (AAA range).
#[derive(Resource, Debug, Clone, Copy)]
pub struct FovPunchTuning {
    pub attack_secs: f32,
    pub decay_secs: f32,
}

impl Default for FovPunchTuning {
    fn default() -> Self {
        Self {
            attack_secs: 0.030,
            decay_secs: 0.150,
        }
    }
}

/// État du FOV punch — `current_deg` est l'offset à AJOUTER à la fov de base
/// par le système d'application (forgia-fps/ads.rs).
/// Lerp : impulse → peak en `attack_secs`, puis decay en `decay_secs` ease-out.
#[derive(Resource, Debug, Clone, Copy)]
pub struct FovPunchState {
    /// Offset courant en degrés (lu par le caller pour ajouter au Projection.fov).
    pub current_deg: f32,
    /// Cible peak en degrés (set par impulse, atteint en attack_secs).
    pub target_peak_deg: f32,
    /// 1 = en phase d'attack (current → target), 0 = en phase decay (current → 0).
    pub phase_attack: bool,
    /// Durée attack (peak rise). Default 30ms.
    pub attack_secs: f32,
    /// Durée decay (full return to 0). Default 150ms.
    pub decay_secs: f32,
}

impl Default for FovPunchState {
    fn default() -> Self {
        Self {
            current_deg: 0.0,
            target_peak_deg: 0.0,
            phase_attack: false,
            attack_secs: 0.030, // 30ms attack
            decay_secs: 0.150,  // 150ms decay
        }
    }
}

pub struct ForgiaJuiceFovPunchPlugin;

impl Plugin for ForgiaJuiceFovPunchPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<FovPunchImpulse>()
            .init_resource::<FovPunchIntensity>()
            .init_resource::<FovPunchTuning>()
            .init_resource::<FovPunchState>()
            .add_systems(Update, (consume_fov_impulses, tick_fov_state).chain());
    }
}

fn consume_fov_impulses(
    mut events: MessageReader<FovPunchImpulse>,
    intensity: Res<FovPunchIntensity>,
    tuning: Res<FovPunchTuning>,
    mut state: ResMut<FovPunchState>,
) {
    let user_factor = intensity.0.clamp(0.0, 1.0);
    for ev in events.read() {
        let effective_peak = ev.peak_deg * user_factor;
        if effective_peak.abs() < 0.01 {
            continue; // sub-degree → skip (e.g. AR/SMG = 0.0 dans TOML)
        }
        // Refresh timings depuis Tuning (hot-reload-friendly).
        state.attack_secs = tuning.attack_secs;
        state.decay_secs = tuning.decay_secs;
        // Si une punch est déjà en cours, prend le max (pas d'addition pour éviter cumul).
        let new_target = if state.target_peak_deg.abs() > effective_peak.abs() {
            state.target_peak_deg
        } else {
            effective_peak
        };
        state.target_peak_deg = new_target;
        state.phase_attack = true;
    }
}

/// Tick le state : attack ramp → peak, puis decay ease-out → 0.
fn tick_fov_state(time: Res<Time>, mut state: ResMut<FovPunchState>) {
    let dt = time.delta_secs();
    if state.phase_attack {
        // Ramp linéaire vers target_peak_deg
        let attack_rate = state.target_peak_deg / state.attack_secs.max(0.001);
        let new_val = state.current_deg + attack_rate * dt;
        // Détecte fin attack : on a dépassé ou atteint le peak.
        let reached = (attack_rate > 0.0 && new_val >= state.target_peak_deg)
            || (attack_rate < 0.0 && new_val <= state.target_peak_deg);
        if reached {
            state.current_deg = state.target_peak_deg;
            state.phase_attack = false;
        } else {
            state.current_deg = new_val;
        }
    } else if state.current_deg.abs() > 0.001 {
        // Decay ease-out cubic — rate proportionnel à current_deg
        let decay_rate = state.current_deg / state.decay_secs.max(0.001);
        let new_val = state.current_deg - decay_rate * dt;
        // Snap to 0 si on traverse
        if new_val.signum() != state.current_deg.signum() {
            state.current_deg = 0.0;
            state.target_peak_deg = 0.0;
        } else {
            state.current_deg = new_val;
        }
    } else {
        state.current_deg = 0.0;
        state.target_peak_deg = 0.0;
    }
}

pub mod prelude {
    pub use super::{
        ForgiaJuiceFovPunchPlugin, FovPunchImpulse, FovPunchIntensity, FovPunchState,
        FovPunchTuning,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaJuiceFovPunchPlugin;
    }

    #[test]
    fn state_default_zero() {
        let s = FovPunchState::default();
        assert_eq!(s.current_deg, 0.0);
        assert_eq!(s.target_peak_deg, 0.0);
        assert!(!s.phase_attack);
        assert!(s.attack_secs > 0.0 && s.attack_secs <= 0.1);
        assert!(s.decay_secs > 0.0 && s.decay_secs <= 0.5);
    }

    #[test]
    fn intensity_default_100() {
        assert!((FovPunchIntensity::default().0 - 1.0).abs() < 1e-6);
    }
}
