#![allow(dead_code, unused_imports)]
//! Port verbatim de `forgia-game/src/effects/weapon_vfx/mod.rs` (V1).
//! Weapon VFX module — AAA-quality particle effects for hitscan weapons.
//!
//! Architecture mirrors fireball_vfx: resource cache of EffectAsset handles,
//! setup at Startup, spawned at fire time via event or direct spawn.

pub mod muzzle;
pub mod impact;
pub mod tracer;

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use forgia_combat::weapons::WeaponType;

// TODO: port from V1 — components::Lifetime
// use forgia_core::components::Lifetime;

/// Multiplier scale appliqué au Transform des particules muzzle, par arme.
/// Snipers / shotguns / rockets ont un muzzle visiblement plus gros — feedback gameplay
/// distinct (pattern V1 confirmé : Boucherie shotgun = flash large, Lenoir sniper = long
/// flash, Pépin / Bourrasque = standard SMG).
pub fn weapon_muzzle_scale(w: &WeaponType) -> f32 {
    match w {
        WeaponType::Shotgun => 1.5,        // Madame Lenoir (V2 mapping : sniper-shotgun hybride)
        WeaponType::RocketLauncher => 1.7, // Boucherie (heavy)
        WeaponType::AK47 => 1.15,
        WeaponType::AssaultRifle => 1.1,   // Bourrasque
        WeaponType::ModernAR => 1.0,       // Pépin (baseline)
        WeaponType::PlasmaRifle => 1.2,
        WeaponType::Chainsaw => 0.0,       // pas de muzzle (mêlée)
    }
}

/// Multiplier scale impact VFX. Rocket = gros plume, sniper = impact net, SMG = standard.
pub fn weapon_impact_scale(w: &WeaponType) -> f32 {
    match w {
        WeaponType::RocketLauncher => 2.0,
        WeaponType::Shotgun => 0.8,        // par pellet plus petit (8 pellets cumulés)
        WeaponType::PlasmaRifle => 1.3,
        _ => 1.0,
    }
}

/// Lifetime component — ported inline until forgia-core exports it.
#[derive(Component)]
pub struct Lifetime(pub Timer);

/// Cached handles for all weapon VFX particle effects.
/// Inserted at Startup, consumed by weapon_fire_system.
#[derive(Resource)]
pub struct WeaponVfxEffects {
    // Muzzle flash (5 layers)
    pub muzzle_core_flash: Handle<EffectAsset>,
    pub muzzle_sparks: Handle<EffectAsset>,
    pub muzzle_smoke: Handle<EffectAsset>,
    pub muzzle_heat_glow: Handle<EffectAsset>,
    pub muzzle_forward_flash: Handle<EffectAsset>,
    // Impact (3 layers)
    pub impact_sparks: Handle<EffectAsset>,
    pub impact_dust: Handle<EffectAsset>,
    pub impact_flash: Handle<EffectAsset>,
}

/// Marker: muzzle VFX entity (for cleanup)
#[derive(Component)]
pub struct MuzzleVfxMarker;

/// Marker: impact VFX entity (for cleanup)
#[derive(Component)]
pub struct ImpactVfxMarker;

pub fn setup_weapon_vfx(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
) {
    let muzzle_core_flash = muzzle::create_muzzle_core_flash(&mut effects);
    let muzzle_sparks = muzzle::create_muzzle_sparks(&mut effects);
    let muzzle_smoke = muzzle::create_muzzle_smoke(&mut effects);
    let muzzle_heat_glow = muzzle::create_muzzle_heat_glow(&mut effects);
    let muzzle_forward_flash = muzzle::create_muzzle_forward_flash(&mut effects);
    let impact_sparks = impact::create_impact_sparks(&mut effects);
    let impact_dust = impact::create_impact_dust(&mut effects);
    let impact_flash = impact::create_impact_flash(&mut effects);

    commands.insert_resource(WeaponVfxEffects {
        muzzle_core_flash,
        muzzle_sparks,
        muzzle_smoke,
        muzzle_heat_glow,
        muzzle_forward_flash,
        impact_sparks,
        impact_dust,
        impact_flash,
    });

    info!("Weapon VFX initialises (5-layer muzzle + 3-layer impact)");
}

/// Spawn all 5 muzzle flash layers at the given barrel tip position.
/// `shot_dir` orients the forward flash tongue.
/// `weapon` détermine le scale (Shotgun/Rocket = plus gros, mêlée = skip).
pub fn spawn_muzzle_flash(
    commands: &mut Commands,
    effects: &WeaponVfxEffects,
    barrel_tip: Vec3,
    shot_dir: Vec3,
    weapon: &WeaponType,
) {
    let scale = weapon_muzzle_scale(weapon);
    if scale <= 0.0 {
        return; // mêlée — pas de muzzle
    }
    let scale_v = Vec3::splat(scale);

    // Transform oriented along shot direction
    let flash_tf = Transform::from_translation(barrel_tip)
        .looking_to(shot_dir, Vec3::Y)
        .with_scale(scale_v);

    // Layer 1: Core flash (world-space, at barrel tip)
    commands.spawn((
        ParticleEffect::new(effects.muzzle_core_flash.clone()),
        Transform::from_translation(barrel_tip).with_scale(scale_v),
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(0.15, TimerMode::Once)),
    ));

    // Layer 2: Spark spray (world-space)
    commands.spawn((
        ParticleEffect::new(effects.muzzle_sparks.clone()),
        Transform::from_translation(barrel_tip).with_scale(scale_v),
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(0.5, TimerMode::Once)),
    ));

    // Layer 3: Smoke puff (world-space, lingers)
    commands.spawn((
        ParticleEffect::new(effects.muzzle_smoke.clone()),
        Transform::from_translation(barrel_tip + shot_dir * 0.05).with_scale(scale_v),
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(1.2, TimerMode::Once)),
    ));

    // Layer 4: Heat glow — story-432 V5-A (2026-05-13) : DISABLED pour
    // réduire le coût spawn hanabi en auto-fire (11 tirs/sec × 5 layers = 55
    // spawn entity/sec + EffectAsset init). Layer marginal visuel (0.15s
    // bloom halo), drop = -20% cost muzzle sans perte ressentie. Layer 5
    // forward_flash conservé (visuel signature DOOM blue/purple).
    // commands.spawn((
    //     ParticleEffect::new(effects.muzzle_heat_glow.clone()),
    //     Transform::from_translation(barrel_tip),
    //     MuzzleVfxMarker,
    //     Lifetime(Timer::from_seconds(0.15, TimerMode::Once)),
    // ));

    // Layer 5: Forward flash tongue (oriented along barrel)
    commands.spawn((
        ParticleEffect::new(effects.muzzle_forward_flash.clone()),
        flash_tf,
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(0.15, TimerMode::Once)),
    ));

    // Muzzle PointLight removed — too bright with Atmosphere, breaks immersion
}

/// Spawn impact VFX at hit point: 3 particle layers + point light.
/// `weapon` détermine le scale (Rocket = gros plume, sniper = précis, SMG = standard).
/// Bullet hole decal will be added when textures are available (Phase 3b).
pub fn spawn_impact_vfx(
    commands: &mut Commands,
    effects: &WeaponVfxEffects,
    impact_pos: Vec3,
    weapon: &WeaponType,
) {
    let scale_v = Vec3::splat(weapon_impact_scale(weapon));

    // Layer 1: Sparks (hemisphere burst)
    commands.spawn((
        ParticleEffect::new(effects.impact_sparks.clone()),
        Transform::from_translation(impact_pos).with_scale(scale_v),
        ImpactVfxMarker,
        Lifetime(Timer::from_seconds(0.6, TimerMode::Once)),
    ));

    // Layer 2: Dust cloud
    commands.spawn((
        ParticleEffect::new(effects.impact_dust.clone()),
        Transform::from_translation(impact_pos).with_scale(scale_v),
        ImpactVfxMarker,
        Lifetime(Timer::from_seconds(1.0, TimerMode::Once)),
    ));

    // Layer 3: Flash burst — story-432 V5-A (2026-05-13) : DISABLED pour
    // réduire le coût spawn hanabi sur burst hits (combat_hit×3 dans 1 frame
    // → 9 spawn). Lifetime 0.1s ultra court = marginal visuel, drop = -33%
    // cost impact. Sparks + dust suffisent au feedback.
    // commands.spawn((
    //     ParticleEffect::new(effects.impact_flash.clone()),
    //     Transform::from_translation(impact_pos),
    //     ImpactVfxMarker,
    //     Lifetime(Timer::from_seconds(0.1, TimerMode::Once)),
    // ));

    // Impact PointLight removed — too bright with Atmosphere, breaks immersion
}
