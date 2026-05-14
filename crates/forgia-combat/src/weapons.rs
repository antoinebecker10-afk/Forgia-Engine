#![allow(dead_code, unused_imports)]
//! Port verbatim de `forgia-game/src/combat/weapons.rs` (V1).
//! TODOs de dépendances cross-module marqués inline.

use bevy::prelude::*;
use bevy::ecs::system::SystemParam;
use bevy_rapier3d::prelude::*;

// TODO: port from V1 — app_state (GameMode, GameSet, WorldMode)
// use forgia_core::app_state::GameMode;

// TODO: depend on forgia-assets — GameAssets
// use forgia_assets::GameAssets;

// TODO: port from V1 — components (Player, LocalPlayer, FpsCamera, GoblinAI, etc.)
// use forgia_player::components::{Player, LocalPlayer, FpsCamera};

// TODO: port from V1 — ai::arena_bot::BotHealth
// use forgia_ai_arena_bot::BotHealth;

// TODO: port from V1 — resources::FpsTuning
// use forgia_core::resources::FpsTuning;

// TODO: port from V1 — genome registry
// use forgia_genome_registry::GenomeRegistry;

use bevy::platform::collections::HashMap;

// Weapon VFX constants migrated to FpsTuning (wfx_fire_shake, wfx_impact_*, wfx_muzzle_*, wfx_tracer_*, wfx_sfx_volume)

// =============================================================================
// Arena V1 loadout — MAR / Shotgun / Rocket (story-421)
// Layout constant: authoritative list of Arena V1 weapons.
// Not a magic number — it is structural (which weapons exist in this mode).
// Balance values (damage, rate) live in FpsTuning as always.
// =============================================================================

pub const ARENA_V1_WEAPONS: [WeaponType; 3] = [
    WeaponType::ModernAR,
    WeaponType::Shotgun,
    WeaponType::RocketLauncher,
];

// =============================================================================
// Weapon Types & Data
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WeaponType {
    #[default]
    ModernAR,
    AssaultRifle,
    AK47,
    Shotgun,
    PlasmaRifle,
    RocketLauncher,
    Chainsaw,
}

impl WeaponType {
    pub fn display_name(&self) -> &'static str {
        match self {
            WeaponType::ModernAR => "Modern AR",
            WeaponType::AssaultRifle => "Assault Rifle",
            WeaponType::AK47 => "AK-47",
            WeaponType::Shotgun => "Shotgun",
            WeaponType::PlasmaRifle => "Plasma Rifle",
            WeaponType::RocketLauncher => "Rocket Launcher",
            WeaponType::Chainsaw => "Chainsaw",
        }
    }

    /// Cycle to next weapon type
    pub fn next(self) -> Self {
        match self {
            WeaponType::ModernAR => WeaponType::AssaultRifle,
            WeaponType::AssaultRifle => WeaponType::AK47,
            WeaponType::AK47 => WeaponType::Shotgun,
            WeaponType::Shotgun => WeaponType::PlasmaRifle,
            WeaponType::PlasmaRifle => WeaponType::RocketLauncher,
            WeaponType::RocketLauncher => WeaponType::Chainsaw,
            WeaponType::Chainsaw => WeaponType::ModernAR,
        }
    }

    /// Cycle to next weapon within a restricted set (Arena V1: MAR/Shotgun/Rocket).
    /// If current is not in the set, resets to the first element.
    pub fn next_in_set(self, set: &[WeaponType]) -> WeaponType {
        if set.is_empty() { return self; }
        let pos = set.iter().position(|w| *w == self);
        match pos {
            Some(i) => set[(i + 1) % set.len()],
            None => set[0],
        }
    }

    /// Whether this weapon is melee (swing animation instead of recoil)
    pub fn is_melee(&self) -> bool {
        matches!(self, WeaponType::Chainsaw)
    }
}

pub struct WeaponData {
    pub damage: f32,
    pub pellets: u8,
    pub fire_rate: f32,
    pub max_ammo: u32,
    pub range: f32,
    pub spread_deg: f32,
    pub projectile_speed: f32,
    pub splash_radius: f32,
    pub is_auto: bool,
}

// =============================================================================
// Equipped Weapons Resource
// =============================================================================

#[derive(Resource)]
pub struct EquippedWeapons {
    pub current: WeaponType,
    pub ammo_rifle: u32,
}

impl Default for EquippedWeapons {
    fn default() -> Self {
        Self {
            current: WeaponType::ModernAR,
            ammo_rifle: 999,
        }
    }
}

impl EquippedWeapons {
    pub fn current_ammo(&self) -> u32 {
        self.ammo_rifle
    }

    pub fn max_ammo(&self) -> u32 {
        999 // infinite ammo for all weapon types
    }

    pub fn consume_ammo(&mut self) -> bool {
        true // infinite ammo
    }

    pub fn add_ammo(&mut self, _weapon: WeaponType, _amount: u32) {
        // infinite ammo, no-op
    }
}

// =============================================================================
// Fire Cooldown (weapon-specific, presence = on cooldown)
// =============================================================================

#[derive(Resource)]
pub struct WeaponFireCooldown {
    pub timer: Timer,
}

// =============================================================================
// Projectile Components (for Plasma & Rocket)
// =============================================================================

#[derive(Component)]
pub struct DoomProjectile {
    pub direction: Vec3,
    pub speed: f32,
    pub damage: f32,
    pub splash_radius: f32,
}

// =============================================================================
// Tuning-driven weapon data (reads from FpsTuning for real-time calibration)
// =============================================================================

/// Hitscan weapons: no projectile travel, no splash
const HITSCAN_SPEED: f32 = 0.0;
const NO_SPLASH: f32 = 0.0;

/// Damage falloff distance-based (story-432 V5-D).
///
/// Pattern Apex/Halo : full damage jusqu'à `start_pct` du range, puis interp
/// linéaire vers `floor_mult` à range max. Hors-range = floor_mult (Halo)
/// plutôt que 0 (TTK plus skill-friendly que cut-off brutal).
pub fn damage_falloff(dist: f32, range: f32, start_pct: f32, floor_mult: f32) -> f32 {
    if range <= 0.0 { return 1.0; }
    let dist_pct = (dist / range).clamp(0.0, 1.0);
    if dist_pct < start_pct {
        1.0
    } else {
        let span = (1.0 - start_pct).max(1e-4);
        let t = ((dist_pct - start_pct) / span).clamp(0.0, 1.0);
        1.0 + (floor_mult - 1.0) * t
    }
}

// TODO: port from V1 — weapon_data_from_tuning requires FpsTuning
// pub fn weapon_data_from_tuning(tuning: &FpsTuning, weapon: WeaponType) -> WeaponData { ... }

// =============================================================================
// Pre-built casing resources (audit-2026-05-02 #43 cache)
// =============================================================================

/// Pre-built brass casing mesh + material, shared across all firing events.
/// Eliminates per-shot `meshes.add()` + `materials.add()` allocations.
#[derive(Resource)]
pub struct CasingResources {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

/// Startup system: pre-build the brass casing mesh + material once.
pub fn setup_casing_resources(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(CasingResources {
        mesh: meshes.add(Cuboid::new(0.004, 0.004, 0.012)),
        material: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.70, 0.30), // brass
            emissive: LinearRgba::new(1.5, 1.0, 0.3, 1.0), // warm glint for bloom catch
            metallic: 0.95,
            perceptual_roughness: 0.2,
            ..default()
        }),
    });
}

/// Fire cooldown tick
pub fn weapon_cooldown_tick_system(
    time: Res<Time>,
    mut commands: Commands,
    cd: Option<ResMut<WeaponFireCooldown>>,
) {
    let Some(mut cd) = cd else { return };
    cd.timer.tick(time.delta());
    if cd.timer.is_finished() {
        commands.remove_resource::<WeaponFireCooldown>();
    }
}

// =============================================================================
// TODO: Systems requiring cross-module deps
// =============================================================================
// The following systems are commented out until their deps are ported:
//
// - weapon_switch_system   → needs MessageReader<MouseWheel>, InputBlockers, GameMode
// - weapon_digit_switch_system → needs InputBlockers, GameMode
// - weapon_fire_system     → needs FpsTuning, GameAssets, BotHealth, GoblinGuard,
//                            FpsCamera, WeaponViewmodel, ReloadState, WeaponVfxEffects
//                            TracerResources, GenomeRegistry, CameraMode, WeaponRecoilImpulse
// - doom_projectile_system → needs BotHealth, GoblinGuard, FpsTuning
// - stagger_detection_system / stagger_tick_system / glory_kill_system
//                          → needs GoblinGuard, FpsTuning, PlayerHealth, LocalPlayer
// - pickup_collection_system / pickup_spin_system
//                          → needs PlayerHealth, PlayerArmor, HealthPickup, ArmorPickup, AmmoPickup

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_next_cycles_all_7() {
        let mut w = WeaponType::ModernAR;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..7 {
            seen.insert(format!("{:?}", w));
            w = w.next();
        }
        assert_eq!(seen.len(), 7);
        assert_eq!(w, WeaponType::ModernAR); // full cycle
    }

    #[test]
    fn weapon_next_in_set_arena_v1_cycles_3() {
        let mut w = ARENA_V1_WEAPONS[0];
        for _ in 0..3 {
            w = w.next_in_set(&ARENA_V1_WEAPONS);
        }
        assert_eq!(w, ARENA_V1_WEAPONS[0]); // back to start
    }

    #[test]
    fn damage_falloff_no_falloff_before_start() {
        // at 0% range → always 1.0
        assert!((damage_falloff(0.0, 100.0, 0.5, 0.3) - 1.0).abs() < 1e-5);
        // at 40% of range, start_pct=0.5 → still 1.0
        assert!((damage_falloff(40.0, 100.0, 0.5, 0.3) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn damage_falloff_floor_at_max_range() {
        // at 100% range → floor_mult
        let result = damage_falloff(100.0, 100.0, 0.5, 0.3);
        assert!((result - 0.3).abs() < 1e-4);
    }

    #[test]
    fn chainsaw_is_melee() {
        assert!(WeaponType::Chainsaw.is_melee());
        assert!(!WeaponType::ModernAR.is_melee());
    }
}
