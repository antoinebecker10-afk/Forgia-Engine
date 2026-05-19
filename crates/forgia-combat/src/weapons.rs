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

use crate::ammo::AmmoSlot;

// Weapon VFX constants migrated to FpsTuning (wfx_fire_shake, wfx_impact_*, wfx_muzzle_*, wfx_tracer_*, wfx_sfx_volume)

// =============================================================================
// Arena V1 loadout — MAR / Shotgun / Rocket (story-421)
// Layout constant: authoritative list of Arena V1 weapons.
// Not a magic number — it is structural (which weapons exist in this mode).
// Balance values (damage, rate) live in FpsTuning as always.
// =============================================================================

pub const ARENA_V1_WEAPONS: [WeaponType; 4] = [
    WeaponType::ModernAR,      // Digit1 = Pépin
    WeaponType::AssaultRifle,  // Digit2 = Bourrasque
    WeaponType::Shotgun,       // Digit3 = Madame Lenoir
    WeaponType::RocketLauncher,// Digit4 = Boucherie
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

    /// V1 stats hardcoded — TOML genome port = Standard story.
    /// Only ARENA_V1_WEAPONS variants have differentiated values; others fall back to ModernAR.
    pub fn stats(self) -> WeaponData {
        match self {
            WeaponType::ModernAR => WeaponData {
                damage: 25.0,
                pellets: 1,
                fire_rate: 10.0, // 10 shots/s → cooldown 0.1s
                max_ammo: 999,
                range: 100.0,
                spread_deg: 0.0,
                projectile_speed: HITSCAN_SPEED,
                splash_radius: NO_SPLASH,
                is_auto: true,
            },
            WeaponType::Shotgun => WeaponData {
                damage: 80.0,
                pellets: 1, // V1 single hitscan, multi-pellet = Standard story
                fire_rate: 1.25, // 1.25 shots/s → cooldown 0.8s
                max_ammo: 999,
                range: 25.0,
                spread_deg: 0.0,
                projectile_speed: HITSCAN_SPEED,
                splash_radius: NO_SPLASH,
                is_auto: false,
            },
            WeaponType::RocketLauncher => WeaponData {
                damage: 150.0,
                pellets: 1, // V1 hitscan, projectile + splash = Standard story
                fire_rate: 0.67, // ~1.5s cooldown
                max_ammo: 999,
                range: 80.0,
                spread_deg: 0.0,
                projectile_speed: HITSCAN_SPEED,
                splash_radius: NO_SPLASH,
                is_auto: false,
            },
            // Fallback : non-Arena V1 weapons use ModernAR baseline
            _ => WeaponType::ModernAR.stats(),
        }
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
// Equipped Weapons Resource (story-455 Phase A — replaces V1 infinite-ammo stub)
// =============================================================================
//
// `slots` est populated lazy par `forgia-fps::sync_ammo_slots_from_genome` quand
// le genome `viewmodel_arena.toml` arrive (Asset Created/Modified). Tant qu'un
// slot n'est pas présent, `slot_or_default()` retourne un slot par défaut
// (mag=30, reserve=120, infinite=false) pour éviter les panics dans le UI.

#[derive(Resource, Default)]
pub struct EquippedWeapons {
    pub current: WeaponType,
    /// Ammo state par arme. Populated par genome sync system.
    pub slots: HashMap<WeaponType, AmmoSlot>,
}

impl EquippedWeapons {
    /// Slot de l'arme actuelle (immutable). None si pas encore initialisé par genome.
    pub fn current_slot(&self) -> Option<&AmmoSlot> {
        self.slots.get(&self.current)
    }

    /// Slot mutable de l'arme actuelle. None si pas encore initialisé.
    pub fn current_slot_mut(&mut self) -> Option<&mut AmmoSlot> {
        self.slots.get_mut(&self.current)
    }

    /// Slot d'une arme spécifique, fallback default si absent. **Lecture seule** —
    /// utile pour HUD qui doit afficher quelque chose avant init.
    pub fn slot_or_default(&self, w: WeaponType) -> AmmoSlot {
        self.slots.get(&w).copied().unwrap_or_default()
    }

    /// Iter slots existants (pour HUD slot strip).
    pub fn iter_slots(&self) -> impl Iterator<Item = (WeaponType, &AmmoSlot)> {
        self.slots.iter().map(|(w, s)| (*w, s))
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
    fn weapon_next_in_set_arena_v1_cycles_full() {
        let mut w = ARENA_V1_WEAPONS[0];
        for _ in 0..ARENA_V1_WEAPONS.len() {
            w = w.next_in_set(&ARENA_V1_WEAPONS);
        }
        assert_eq!(w, ARENA_V1_WEAPONS[0]); // back to start après len() itérations
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
