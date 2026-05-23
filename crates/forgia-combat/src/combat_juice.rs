#![allow(dead_code, unused_imports)]
//! Port verbatim de `forgia-game/src/effects/combat_juice.rs` (V1).
//! Combat Juice â€” Hitstop, Trauma Shake, Hit Flash, Kill SlowMo
//!
//! Transforms combat from "numbers going down" to visceral impact.
//! Uses observer pattern: detects health changes on goblins each frame.

use bevy::prelude::*;

// TODO: port from V1 â€” ChromaticAberration
// use bevy::post_process::effect_stack::ChromaticAberration;

// TODO: port from V1 â€” components (GoblinGuard, FpsCamera, CameraState)
// use forgia_player::components::{FpsCamera, CameraState};

// TODO: port from V1 â€” ai::arena_bot::BotHealth
// use forgia_ai_arena_bot::BotHealth;

// TODO: port from V1 â€” resources::FpsTuning, WeaponFireFlash
// use forgia_core::resources::{FpsTuning, WeaponFireFlash};

/// Message emitted when damage is detected on an enemy.
///
/// Story-455 Phase C (2026-05-18) — étendu pour alimenter kill feed (Phase D) et
/// damage direction indicator (Phase E) sans pull-based queries.
///
/// Fields :
/// - `target` : entité qui a reçu le coup (porte `Health`).
/// - `attacker` : entité qui a tiré (Player ou Bot ; None pour world damage futur).
/// - `damage` : dégâts effectifs appliqués (après falloff/headshot mul).
/// - `is_kill` : true si HP atteint 0 après ce coup.
/// - `is_headshot` : true si le ray a touché une `HitZoneHead`. ⚠ Story-455 Phase C
///   reste à `false` jusqu'à hitzone Head/Body split (story-456 deferred).
/// - `hit_world_pos` : position monde du point d'impact (pour DDI angle + popup spawn).
/// - `weapon` : arme utilisée (pour kill feed icon mapping). None = world/melee.
/// - `body_zone` : zone du corps touchée (story-457, 2026-05-19). Pilote
///   damage multiplier + visual style (couleur/taille floating number +
///   label nameplate). `Body` par défaut si aucun `HitZoneTag` rencontré.
#[derive(Message)]
pub struct CombatHitEvent {
    pub target: Entity,
    pub attacker: Option<Entity>,
    pub damage: f32,
    pub is_kill: bool,
    pub is_headshot: bool,
    pub hit_world_pos: Vec3,
    pub weapon: Option<crate::weapons::WeaponType>,
    pub body_zone: forgia_damage::HitZone,
}

/// Tracks previous health to detect damage via change detection.
#[derive(Component)]
pub struct PrevHealth(pub f32);

// TODO: detect_combat_hits requires GoblinGuard + BotHealth (cross-crate deps)
// pub fn detect_combat_hits(...) { ... }

// â”€â”€ Resources â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

// HitStopState : migré vers `forgia-juice-hit-stop` (Tier 1D, 2026-05-17).
// Migration legacy re-export retirée 2026-05-18 — consommateurs DOIVENT importer direct :
//   use forgia_juice_hit_stop::HitStopState;

#[derive(Resource, Default)]
pub struct CameraTrauma {
    pub trauma: f32,
    pub time_acc: f32,
}

impl CameraTrauma {
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }
    pub fn shake_amount(&self) -> f32 {
        self.trauma * self.trauma
    }
}

#[derive(Component)]
pub struct HitFlashTimer {
    pub timer: Timer,
    pub original_emissive: LinearRgba,
    /// 2026-04-27 â€” handle of the *original* (shared) StandardMaterial. On
    /// expiry we restore this handle so the entity rejoins the GPU-instancing
    /// batch of its peers. `None` if the entity has no `MeshMaterial3d`.
    ///
    /// story-432 V4 (2026-05-13) : la stratÃ©gie de clone-per-hit a Ã©tÃ©
    /// remplacÃ©e par un swap vers `HitFlashCache.flash_material` partagÃ©.
    /// Plusieurs entitÃ©s en flash simultanÃ©ment pointent vers le MÃŠME handle
    /// (pas de mutation du shared = pas de peer-flash bug du 2026-04-27).
    /// Cost hot path : `Handle::clone` (Arc atomic inc, ~ns) vs ~6 KB
    /// per-hit material clone prÃ©cÃ©demment (suspect freeze sensors).
    pub original_handle: Option<Handle<StandardMaterial>>,
}

/// Pre-built shared flash material â€” white emissive HDR (8.0, 8.0, 8.0).
/// InsÃ©rÃ© au Startup, swappÃ© sur l'entity touchÃ©e pour la durÃ©e du flash.
///
/// story-432 V4 : remplace le pattern clone-per-hit pour Ã©liminer un suspect
/// majeur des freezes 100-200ms corrÃ©lÃ©s `damage_player â†’ 3 hits` observÃ©s
/// dans `forgia_lag_events.json`.
#[derive(Resource)]
pub struct HitFlashCache {
    pub flash_material: Handle<StandardMaterial>,
}

/// Startup : prÃ©-construit le material flash blanc partagÃ©.
pub fn setup_hit_flash_cache(
    mut commands: Commands,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(HitFlashCache {
        flash_material: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: LinearRgba::new(8.0, 8.0, 8.0, 1.0),
            unlit: false,
            ..default()
        }),
    });
}

// â”€â”€ Systems â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

// TODO: combat_juice_event_system requires FpsTuning + ChromaticAberration (Bevy pp) + HitFlashCache
// pub fn combat_juice_event_system(...) { ... }

// hitstop_tick_system : extrait vers `forgia-juice-hit-stop` (Tier 1D, 2026-05-17).
// Wiring : `forgia_juice_hit_stop::ForgiaJuiceHitStopPlugin` ajouté idempotent dans `ForgiaCombatPlugin`.

pub fn trauma_decay_system(time: Res<Time>, mut trauma: ResMut<CameraTrauma>) {
    if trauma.trauma > 0.001 {
        trauma.trauma *= (-4.0 * time.delta_secs()).exp();
        trauma.time_acc += time.delta_secs();
        if trauma.trauma < 0.001 {
            trauma.trauma = 0.0;
        }
    }
}

/// Camera shake offset from trauma (call from camera system).
pub fn compute_trauma_offset(trauma: &CameraTrauma) -> Vec3 {
    if trauma.trauma < 0.001 {
        return Vec3::ZERO;
    }
    let shake = trauma.shake_amount();
    let t = trauma.time_acc;
    let offset_x = (t * 23.7).sin() * (t * 41.3).cos();
    let offset_y = (t * 31.1).sin() * (t * 53.7).cos();
    let max_offset = 0.15;
    Vec3::new(
        offset_x * shake * max_offset,
        offset_y * shake * max_offset,
        0.0,
    )
}

pub fn hit_flash_tick_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut HitFlashTimer)>,
) {
    // story-432 V4 : plus de mutation per-frame du material (le shared
    // `HitFlashCache.flash_material` a dÃ©jÃ  emissive=8.0 figÃ©). Tick juste le
    // timer + restore le handle original Ã  expiry.
    for (entity, mut flash) in &mut query {
        flash.timer.tick(time.delta());
        if flash.timer.is_finished() {
            if let Some(orig) = flash.original_handle.take() {
                commands.entity(entity).insert(MeshMaterial3d(orig));
            }
            commands.entity(entity).remove::<HitFlashTimer>();
        }
    }
}

// â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€ Camera Recoil (story-432 V2) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
//
// Pattern Apex/COD : sur tir, push pitch â†‘ + petit yaw random, puis "dette"
// dÃ©croÃ®t exponentiellement â†’ camÃ©ra recentre auto si pas d'input joueur.
// Modifie `CameraState.pitch/yaw` directement (aim authentique), pas un overlay
// visuel. Le joueur peut compenser activement en pullant le mouse (skill).
//
// Event-driven : `weapon_fire_system` Ã©met `WeaponRecoilImpulse`, lu ici.

// WeaponRecoilImpulse + WeaponRecoilDebt : extraits vers `forgia-juice-recoil` (Tier 1E, 2026-05-17).
// Re-export backward compat (prelude). Preferer `forgia_juice_recoil::*` direct dans le nouveau code.
pub use forgia_juice_recoil::{WeaponRecoilDebt, WeaponRecoilImpulse};

// TODO: weapon_recoil_system requires CameraState + FpsCamera (forgia-camera-fps)
//       + WeaponRecoilImpulse MessageReader (Bevy 0.18 message API)
// pub fn weapon_recoil_system(...) { ... }

// TODO: weapon_fire_flash_system requires ChromaticAberration (bevy::post_process)
//       + FpsTuning + WeaponFireFlash + FpsCamera
// pub fn weapon_fire_flash_system(...) { ... }

#[cfg(test)]
mod tests {
    use super::*;

    // â”€â”€ CameraTrauma::default â”€â”€

    #[test]
    fn camera_trauma_default_is_zero() {
        let t = CameraTrauma::default();
        assert_eq!(t.trauma, 0.0);
        assert_eq!(t.time_acc, 0.0);
    }

    // â”€â”€ CameraTrauma::add â”€â”€

    #[test]
    fn camera_trauma_add_accumulates() {
        let mut t = CameraTrauma::default();
        t.add(0.3);
        assert!((t.trauma - 0.3).abs() < 1e-5);
        t.add(0.4);
        assert!((t.trauma - 0.7).abs() < 1e-5);
    }

    #[test]
    fn camera_trauma_add_clamps_to_one() {
        let mut t = CameraTrauma::default();
        t.add(0.6);
        t.add(0.6); // would overshoot to 1.2 â†’ must clamp.
        assert_eq!(t.trauma, 1.0);
    }

    #[test]
    fn camera_trauma_add_single_huge_value_clamps_too() {
        let mut t = CameraTrauma::default();
        t.add(5.0);
        assert_eq!(t.trauma, 1.0);
    }

    #[test]
    fn camera_trauma_add_zero_is_noop() {
        let mut t = CameraTrauma::default();
        t.add(0.5);
        let before = t.trauma;
        t.add(0.0);
        assert_eq!(t.trauma, before);
    }

    // â”€â”€ CameraTrauma::shake_amount â”€â”€

    #[test]
    fn shake_amount_is_quadratic() {
        let mut t = CameraTrauma::default();
        // shake_amount = traumaÂ²
        assert_eq!(t.shake_amount(), 0.0);
        t.trauma = 0.5;
        assert!((t.shake_amount() - 0.25).abs() < 1e-5);
        t.trauma = 1.0;
        assert!((t.shake_amount() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn shake_amount_grows_faster_than_trauma() {
        // Quadratic curve: between 0 and 1, shake is always <= trauma.
        let mut t = CameraTrauma::default();
        for trauma in [0.1f32, 0.3, 0.5, 0.7, 0.9] {
            t.trauma = trauma;
            assert!(
                t.shake_amount() <= trauma,
                "quadratic shake must be <= linear trauma at {trauma}: got {}",
                t.shake_amount()
            );
        }
    }
}
