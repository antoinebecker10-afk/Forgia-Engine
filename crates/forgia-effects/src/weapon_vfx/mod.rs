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
    // Story-450 (2026-05-18) — softening user request : VFX trop gros bloquaient la
    // visée. Réduction ~35% sur tous les scales, baseline ModernAR passe 1.0 → 0.65.
    // Signature gameplay préservée (shotgun/rocket toujours plus gros).
    match w {
        WeaponType::Shotgun => 1.05,       // Madame Lenoir (sniper-shotgun hybride V2)
        WeaponType::RocketLauncher => 1.20, // Boucherie (heavy mais réduit)
        WeaponType::AK47 => 0.80,
        WeaponType::AssaultRifle => 0.75,  // Bourrasque
        WeaponType::ModernAR => 0.65,      // Pépin (baseline shrink)
        WeaponType::PlasmaRifle => 0.85,
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

/// Tint LinearRgba du muzzle flash per-arme (Phase 5 dette tech 2026-05-18).
///
/// Mapping Arena V1 (WeaponType legacy → arme V2 réelle) :
/// - `ModernAR` (Pépin pistolet)        → blanc-jaune cordite standard
/// - `AssaultRifle` (Bourrasque SMG)    → blanc-jaune chaud
/// - `Shotgun` (Madame Lenoir sniper)   → blanc froid, signature flash long
/// - `RocketLauncher` (Boucherie pump)  → rouge-orange poudre brute
/// - Autres : fallback blanc neutre
///
/// Valeurs HDR (>1.0 sur composantes pour bloom catch). Source : refs Valorant /
/// Apex tuning sheets — muzzle = warm white avec twist per-weapon.
pub fn weapon_muzzle_tint(w: &WeaponType) -> LinearRgba {
    match w {
        WeaponType::ModernAR => LinearRgba::new(3.0, 2.6, 1.4, 1.0),       // blanc-jaune
        WeaponType::AssaultRifle => LinearRgba::new(3.2, 2.5, 1.2, 1.0),   // blanc-jaune chaud
        WeaponType::Shotgun => LinearRgba::new(3.0, 3.0, 3.5, 1.0),        // blanc froid sniper
        WeaponType::RocketLauncher => LinearRgba::new(3.5, 1.6, 0.6, 1.0), // rouge-orange poudre
        WeaponType::PlasmaRifle => LinearRgba::new(0.8, 1.8, 3.5, 1.0),    // cyan plasma
        _ => LinearRgba::new(3.0, 2.6, 1.4, 1.0),
    }
}

/// Tint impact VFX per-arme. Plus sobre que muzzle (impact = poussière/spark, pas flash).
pub fn weapon_impact_tint(w: &WeaponType) -> LinearRgba {
    match w {
        WeaponType::RocketLauncher => LinearRgba::new(3.0, 1.4, 0.5, 1.0),  // orange explosif
        WeaponType::Shotgun => LinearRgba::new(2.0, 2.0, 2.4, 1.0),         // blanc froid sniper
        WeaponType::PlasmaRifle => LinearRgba::new(0.6, 1.6, 3.0, 1.0),     // cyan plasma
        _ => LinearRgba::new(2.4, 2.0, 1.2, 1.0),                            // étincelles standard
    }
}

/// Lifetime smoke per-arme. Sniper Lenoir = trail long (signature one-shot).
/// Shotgun = court (déjà compensé par spawn larger).
pub fn weapon_muzzle_smoke_lifetime(w: &WeaponType) -> f32 {
    match w {
        WeaponType::Shotgun => 2.5,         // sniper Lenoir : long trail
        WeaponType::RocketLauncher => 1.6,  // shotgun Boucherie : smoke lourd
        WeaponType::AssaultRifle => 0.9,    // SMG full-auto : court pour pas accumuler
        _ => 1.2,
    }
}

/// PointLight intensité brève muzzle (lumens). Reste sous threshold "too bright with
/// Atmosphere" (cf comment historique L167) : 0 = skip (mêlée), sniper plus brillant.
pub fn weapon_muzzle_light_intensity(w: &WeaponType) -> f32 {
    match w {
        WeaponType::Chainsaw => 0.0,
        WeaponType::RocketLauncher => 8_000.0,  // shotgun boom
        WeaponType::Shotgun => 6_000.0,         // sniper one-shot
        WeaponType::AssaultRifle => 2_500.0,    // SMG sustained = faible (anti eye-strain)
        WeaponType::ModernAR => 3_500.0,        // pistolet
        _ => 3_000.0,
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

    // Layer 3: Smoke puff (world-space, lingers — per-arme : sniper = trail long)
    commands.spawn((
        ParticleEffect::new(effects.muzzle_smoke.clone()),
        Transform::from_translation(barrel_tip + shot_dir * 0.05).with_scale(scale_v),
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(weapon_muzzle_smoke_lifetime(weapon), TimerMode::Once)),
    ));

    // Layer 4: Heat glow — story-450 (2026-05-18) : RE-ENABLED après shrink x3
    // (0.10-0.20 vs 0.30-0.60) + alpha /2. Halo bloom subtil qui ajoute la
    // qualité AAA "weight feel" sans bloquer la cible. Coût hanabi acceptable
    // (12 particles × 0.07s = ~0.84 particles/s steady-state en auto-fire).
    commands.spawn((
        ParticleEffect::new(effects.muzzle_heat_glow.clone()),
        Transform::from_translation(barrel_tip).with_scale(scale_v),
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(0.12, TimerMode::Once)),
    ));

    // Layer 5: Forward flash tongue (oriented along barrel)
    commands.spawn((
        ParticleEffect::new(effects.muzzle_forward_flash.clone()),
        flash_tf,
        MuzzleVfxMarker,
        Lifetime(Timer::from_seconds(0.15, TimerMode::Once)),
    ));

    // Layer 6: PointLight bref tinté per-arme (Phase 5 dette tech 2026-05-18).
    // Historique : un PointLight plein-intensité cassait l'Atmosphere. Solution :
    // intensity faible (2k-8k lumens, vs 100k+ "réaliste"), lifetime ultra-court
    // (50-80ms), range borné. Donne signature couleur sans bloom run-away.
    let light_intensity = weapon_muzzle_light_intensity(weapon);
    if light_intensity > 0.0 {
        let tint = weapon_muzzle_tint(weapon);
        commands.spawn((
            PointLight {
                color: Color::LinearRgba(tint),
                intensity: light_intensity,
                range: 4.0,
                radius: 0.05,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_translation(barrel_tip),
            MuzzleVfxMarker,
            Lifetime(Timer::from_seconds(0.07, TimerMode::Once)),
        ));
    }
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

    // Layer 4: PointLight bref tinté per-arme (Phase 5 dette tech 2026-05-18).
    // Intensity ~50% du muzzle (impact = secondaire feedback), lifetime 40ms.
    let tint = weapon_impact_tint(weapon);
    let intensity = match weapon {
        WeaponType::RocketLauncher => 5_000.0,
        WeaponType::Shotgun => 3_000.0,
        _ => 1_500.0,
    };
    commands.spawn((
        PointLight {
            color: Color::LinearRgba(tint),
            intensity,
            range: 3.0,
            radius: 0.05,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_translation(impact_pos),
        ImpactVfxMarker,
        Lifetime(Timer::from_seconds(0.04, TimerMode::Once)),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Phase 5 (dette tech 2026-05-18) — VFX colors per-arme tests purs ──

    #[test]
    fn muzzle_tint_distinct_per_v1_weapon() {
        // 4 armes V1 doivent avoir des signatures couleur distinguables.
        let pepin = weapon_muzzle_tint(&WeaponType::ModernAR);
        let bourrasque = weapon_muzzle_tint(&WeaponType::AssaultRifle);
        let lenoir = weapon_muzzle_tint(&WeaponType::Shotgun);
        let boucherie = weapon_muzzle_tint(&WeaponType::RocketLauncher);
        // Lenoir (sniper) = blanc froid → bleu > rouge ; Boucherie (shotgun)
        // = rouge-orange → rouge > bleu. Inversion garantit lecture distincte.
        assert!(lenoir.blue > lenoir.red, "Lenoir doit être bleu-froid");
        assert!(boucherie.red > boucherie.blue, "Boucherie doit être rouge-orange");
        // Pépin et Bourrasque sont proches (deux gunfeels chauds) mais doivent rester
        // dans la même famille warm white.
        assert!(pepin.red > pepin.blue, "Pépin warm");
        assert!(bourrasque.red > bourrasque.blue, "Bourrasque warm");
    }

    #[test]
    fn impact_tint_per_v1_weapon() {
        // Rocket/shotgun pump → orange chaud, sniper → blanc froid.
        let boucherie = weapon_impact_tint(&WeaponType::RocketLauncher);
        let lenoir = weapon_impact_tint(&WeaponType::Shotgun);
        assert!(boucherie.red > boucherie.blue, "Boucherie impact warm");
        assert!(lenoir.blue >= lenoir.red, "Lenoir impact cold");
    }

    #[test]
    fn smoke_lifetime_sniper_longest() {
        // Lenoir (Shotgun enum) doit avoir le trail le plus long — signature one-shot.
        let lenoir = weapon_muzzle_smoke_lifetime(&WeaponType::Shotgun);
        let pepin = weapon_muzzle_smoke_lifetime(&WeaponType::ModernAR);
        let bourrasque = weapon_muzzle_smoke_lifetime(&WeaponType::AssaultRifle);
        assert!(lenoir > pepin, "sniper trail > pistolet");
        assert!(lenoir > bourrasque, "sniper trail > SMG");
        assert!(bourrasque < pepin, "SMG plus court que pistolet (anti accumulation)");
    }

    #[test]
    fn muzzle_light_intensity_melee_zero() {
        assert_eq!(weapon_muzzle_light_intensity(&WeaponType::Chainsaw), 0.0,
            "mêlée : pas de PointLight");
    }

    #[test]
    fn muzzle_light_intensity_smg_lowest_of_firearms() {
        // Anti eye-strain : SMG full-auto 16Hz × intensity élevée = strobe désagréable.
        let smg = weapon_muzzle_light_intensity(&WeaponType::AssaultRifle);
        let pepin = weapon_muzzle_light_intensity(&WeaponType::ModernAR);
        let lenoir = weapon_muzzle_light_intensity(&WeaponType::Shotgun);
        let boucherie = weapon_muzzle_light_intensity(&WeaponType::RocketLauncher);
        assert!(smg < pepin, "SMG < pistolet (anti strobe 16Hz)");
        assert!(smg < lenoir);
        assert!(smg < boucherie);
        // Et tous restent sous le threshold "casse Atmosphere" (~10k lumens empirique).
        for v in [smg, pepin, lenoir, boucherie] {
            assert!(v < 10_000.0, "intensité doit rester subtle, eut {v}");
        }
    }

    #[test]
    fn muzzle_scale_melee_zero() {
        assert_eq!(weapon_muzzle_scale(&WeaponType::Chainsaw), 0.0);
    }
}
