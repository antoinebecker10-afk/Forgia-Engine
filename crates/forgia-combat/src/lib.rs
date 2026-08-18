//! # forgia-combat
//!
//! Gunfeel V5-F partagé FPS et RPG :
//! - Weapons (hitscan, melee, projectile rocket)
//! - Viewmodel (sway/bob/recoil coordonnés caméra 1P)
//! - Hit-stop genome (Time<Virtual> pause/resume)
//! - Camera recoil event Apex
//! - Hitmarker visuel
//! - Hit flash emissive en place (zéro swap de matériau per-hit)
//! - Damage numbers + falloff
//! - Tracer cache (-55% freeze 1er tir)
//!
//! Phase 2 : portage VERBATIM des fichiers V1 (exigence game-maker non négociable).

use bevy::prelude::*;
use forgia_core::prelude::*;

pub mod ammo;
pub mod combat_juice;
pub mod combat_mods;
pub mod combat_rng;
pub mod confidence;
pub mod melee;
pub mod sensor;
pub mod ultimate;
pub mod weapons;

// Inventaire V1, pas une liste de TODO : la plupart de ces responsabilités ont
// été remplacées en V2 ou ne concernent pas le Roguelite. Décision détaillée :
// docs/audits/v1-nonported-classification-2026-08-05.md.
// pub mod viewmodel;
// pub mod reload;
// pub mod health;
// pub mod rpg_systems;
// pub mod targeting;
// pub mod boss;
// pub mod gcd;

/// Ce qui peut ETRE TIRE. C'est le filtre du tir hitscan, pas un detail d'arene.
///
/// # Pourquoi ce marqueur a demenage (2026-08-18)
///
/// Il vivait dans `forgia-mode-fps-arena`, une crate de ZONE — si bien que
/// `forgia-fps`, `forgia-mode-roguelite` et `forgia-observability` dependaient
/// toutes de cette zone pour savoir ce qu'une balle touche. Une zone que, par
/// ailleurs, aucun menu n'atteint : elle n'est joignable que par la variable
/// d'environnement `FORGIA_BOOT_MODE=fps`.
///
/// Sa place est ici, avec le tir qui le lit. `forgia-mode-fps-arena` le
/// re-exporte pour que rien ne bouge chez ses appelants.
///
/// Story-461 (Vague 3) : `#[require(Transform, Visibility)]` garantit que tout
/// spawn insere Transform + Visibility avec Default si non fournis.
#[derive(Component)]
#[require(Transform, Visibility)]
pub struct TargetCube;

pub mod prelude {
    pub use crate::ammo::{
        sync_ammo_slot_from_config, AmmoChangeKind, AmmoChanged, AmmoConfig, AmmoSlot, ReloadKind,
        ReloadState,
    };
    pub use crate::combat_juice::{
        CameraTrauma, CombatHitEvent, HitFlashTimer, WeaponRecoilDebt, WeaponRecoilImpulse,
        HIT_FLASH_EMISSIVE,
    };
    pub use crate::combat_rng::{CombatRng, CRIT_SALT};
    pub use crate::confidence::{PepinConfidence, ShotResolved};
    pub use crate::melee::MeleeCooldown;
    pub use crate::sensor::{CombatSensorCounters, LocalPlayerMarker};
    pub use crate::ultimate::{UltimateState, ULTIMATE_COOLDOWN_SECS, ULTIMATE_DURATION_SECS};
    pub use crate::weapons::{
        damage_falloff, CasingResources, EquippedWeapons, WeaponFireCooldown, WeaponType,
        ARENA_V1_WEAPONS,
    };
    pub use crate::{ForgiaCombatPlugin, Health, TargetCube};
    // ⛔ HitStopState : RETIRÉ DÉFINITIVEMENT (décision Antoine 2026-08-12).
    // Ne pas l'importer, ne pas le recréer. Motif et historique complets dans
    // `combat_juice.rs`, en-tête « HITSTOP ».
}

// =============================================================================
// Player Score / Level (from V1 combat/mod.rs)
// =============================================================================

#[derive(Resource, Default)]
pub struct PlayerScore {
    pub gold: u32,
    pub kills: u32,
}

#[derive(Resource)]
pub struct PlayerLevel {
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
}

impl Default for PlayerLevel {
    fn default() -> Self {
        Self {
            level: 1,
            xp: 0,
            xp_to_next: 100,
        }
    }
}

impl PlayerLevel {
    /// Returns true if levelled up.
    pub fn award_xp(&mut self, xp: u32) -> bool {
        self.xp += xp;
        if self.xp >= self.xp_to_next {
            self.xp -= self.xp_to_next;
            self.level += 1;
            self.xp_to_next = (self.xp_to_next as f32 * 1.25) as u32;
            true
        } else {
            false
        }
    }
}

// =============================================================================
// Health component (shared player/bots/NPCs)
// =============================================================================

/// Health — composant combat partagé player/bots/NPCs.
#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }
}

/// ease-out quadratic (deceleration naturelle) — partagé avec goblin_death_system
pub fn ease_out_quad(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

// =============================================================================
// Plugin
// =============================================================================

pub struct ForgiaCombatPlugin;

impl Plugin for ForgiaCombatPlugin {
    fn build(&self, app: &mut App) {
        // Recoil data (WeaponRecoilImpulse Message + WeaponRecoilDebt Resource) — crate dédié.
        if !app.is_plugin_added::<forgia_juice_lib::recoil::ForgiaJuiceRecoilPlugin>() {
            app.add_plugins(forgia_juice_lib::recoil::ForgiaJuiceRecoilPlugin);
        }
        // Story-650 — knockback ennemi à l'impact (composant + tick, crate dédié).
        if !app.is_plugin_added::<forgia_juice_lib::knockback::ForgiaJuiceKnockbackPlugin>() {
            app.add_plugins(forgia_juice_lib::knockback::ForgiaJuiceKnockbackPlugin);
        }
        app.init_resource::<PlayerScore>()
            .init_resource::<PlayerLevel>()
            .init_resource::<sensor::CombatSensorCounters>()
            .init_resource::<weapons::EquippedWeapons>()
            .init_resource::<combat_juice::CameraTrauma>()
            // Story-558 Phase 4 — PlayerCombatMods Resource (boons multipliers).
            // Default neutre 1.0/1.0/0.0. Muté par forgia-mode-roguelite et lu
            // par forgia-fps (damage_mul + fire_rate_mul) et forgia-damage
            // (damage_reduction — Phase 4b).
            .init_resource::<combat_mods::PlayerCombatMods>()
            // Keystone 0.1b (story-634) — flux RNG combat déterministe (crit, …).
            // Reseedé depuis RunSeed au StartRunEvent par forgia-mode-roguelite.
            .init_resource::<combat_rng::CombatRng>()
            // État Ultime (touche F) — timer/cooldown partagé fps↔roguelite↔HUD.
            // L'input F + le branchement technique-par-arme vivent dans
            // forgia-mode-roguelite (combat = dep root, neutre ici).
            .init_resource::<ultimate::UltimateState>()
            // Story-531 AC9 — jauge de confiance Pépin (state partagé fps↔HUD).
            .init_resource::<confidence::PepinConfidence>()
            .add_message::<confidence::ShotResolved>()
            .add_message::<combat_juice::CombatHitEvent>()
            .add_message::<combat_juice::WeaponFiredEvent>()
            .add_message::<ammo::AmmoChanged>()
            // setup_hit_flash_cache retiré (audit fire-path 2026-07-20) : le
            // flash mute l'emissive en place, plus de flash-material partagé.
            .add_systems(Startup, weapons::setup_casing_resources)
            // Keystone 0.1a-2 slice 2 (story-634) — cooldowns d'arme/melee = timers
            // PURS (Res<Time> seul, 0 input) → migrés en FixedUpdate (sim déterministe).
            // En FixedUpdate, les cooldowns suivent uniquement la simulation fixe.
            // Ordre via la chaîne GameSet aussi en FixedUpdate (0.1a-1).
            .add_systems(
                FixedUpdate,
                (
                    weapons::weapon_cooldown_tick_system
                        .run_if(|cd: Option<Res<weapons::WeaponFireCooldown>>| cd.is_some())
                        .in_set(GameSet::Combat),
                    melee::melee_cooldown_tick_system
                        .run_if(resource_exists::<melee::MeleeCooldown>)
                        .in_set(GameSet::Combat),
                    // Story-596 — sys_tick_ultimate = timer PUR (comme les cooldowns
                    // d'arme/melee) → FixedUpdate aussi, pour le déterminisme keystone
                    // (story-634). is_active() consommé par les techniques (Effects).
                    ultimate::sys_tick_ultimate.in_set(GameSet::Combat),
                ),
            )
            .add_systems(
                Update,
                (
                    // Trauma/hit_flash restent Update ; sensor reste Update (télémétrie).
                    combat_juice::trauma_decay_system.in_set(GameSet::Effects),
                    combat_juice::hit_flash_tick_system.in_set(GameSet::Effects),
                    // Story-650 — pousse l'ennemi à chaque CombatHitEvent (Vlambeer).
                    combat_juice::sys_apply_hit_knockback.in_set(GameSet::Effects),
                    sensor::sys_write_combat_sensor.in_set(GameSet::Sensors),
                    ultimate::sys_write_ultimate_sensor.in_set(GameSet::Sensors),
                ),
            );
        // TODO: wire weapon_fire_system, doom_projectile_system, melee_attack_system,
        //       weapon_switch_system, stagger systems, glory_kill_system, pickup systems,
        //       detect_combat_hits, weapon_recoil_system, weapon_fire_flash_system
        //       once their cross-crate deps are ported.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_dies_at_zero() {
        let mut h = Health::new(100.0);
        assert!(!h.is_dead());
        h.current = 0.0;
        assert!(h.is_dead());
        h.current = -10.0;
        assert!(h.is_dead());
    }

    #[test]
    fn player_level_award_xp_levels_up() {
        let mut level = PlayerLevel::default();
        // Need 100 XP
        let levelled = level.award_xp(100);
        assert!(levelled);
        assert_eq!(level.level, 2);
        assert_eq!(level.xp, 0);
    }

    #[test]
    fn player_level_award_xp_no_levelup_below_threshold() {
        let mut level = PlayerLevel::default();
        let levelled = level.award_xp(50);
        assert!(!levelled);
        assert_eq!(level.level, 1);
        assert_eq!(level.xp, 50);
    }

    #[test]
    fn ease_out_quad_bounds() {
        assert!((ease_out_quad(0.0) - 0.0).abs() < 1e-5);
        assert!((ease_out_quad(1.0) - 1.0).abs() < 1e-5);
        // At 0.5 → 1 - 0.25 = 0.75
        assert!((ease_out_quad(0.5) - 0.75).abs() < 1e-5);
    }
}
