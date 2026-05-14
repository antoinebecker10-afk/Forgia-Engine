#![allow(dead_code, unused_imports)]
// Port verbatim de `forgia-game/src/effects/weapon_vfx/muzzle.rs` (V1).
//! Muzzle flash VFX — 5-layer AAA composition (DOOM/CoD technique)
//!
//! Layers:
//! 1. Core flash — ultra-bright HDR burst, 1-2 frames
//! 2. Spark spray — hot debris ejected radially from barrel
//! 3. Smoke puff — light gray, drifts upward, slow dissipation
//! 4. Heat glow — large soft envelope around muzzle
//! 5. Forward flash — elongated tongue along barrel direction

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_hanabi::Gradient as HanabiGradient;

/// Layer 1: Core flash — intense white-yellow HDR burst, blooms hard.
/// Very short lifetime (~30-50ms), small radius, max HDR.
pub(super) fn create_muzzle_core_flash(effects: &mut ResMut<Assets<EffectAsset>>) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.03).expr(),
        dimension: ShapeDimension::Volume,
    };

    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(0.1).uniform(writer.lit(0.5)).expr(),
    };

    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.06).uniform(writer.lit(0.14)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.025).uniform(writer.lit(0.05)).expr(),
    );

    // Ultra-bright HDR — white core with bloom blowout
    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(12.0, 11.0, 8.0, 1.0));   // White-hot HDR
    color_gradient.add_key(0.3, Vec4::new(8.0, 5.0, 2.0, 0.9));     // Bright yellow
    color_gradient.add_key(0.6, Vec4::new(4.0, 1.5, 0.4, 0.5));     // Orange
    color_gradient.add_key(1.0, Vec4::new(1.0, 0.2, 0.05, 0.0));    // Fade

    let effect = EffectAsset::new(
        32,
        SpawnerSettings::burst(20.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("muzzle_core_flash")
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .render(ColorOverLifetimeModifier { gradient: color_gradient, ..default() });

    effects.add(effect)
}

/// Layer 2: Spark spray — hot metallic debris ejected from barrel.
/// Fast, gravity-affected, parabolic arcs.
pub(super) fn create_muzzle_sparks(effects: &mut ResMut<Assets<EffectAsset>>) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.02).expr(),
        dimension: ShapeDimension::Surface,
    };

    // Forward-biased cone: sparks fly mostly forward with some spread
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::new(0.0, 0.3, 0.0)).expr(),
        speed: writer.lit(3.0).uniform(writer.lit(8.0)).expr(),
    };

    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.008).uniform(writer.lit(0.025)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.08).uniform(writer.lit(0.25)).expr(),
    );

    let gravity = AccelModifier::new(writer.lit(Vec3::new(0.0, -6.0, 0.0)).expr());
    let drag = LinearDragModifier::new(writer.lit(2.0).expr());

    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(4.0, 3.2, 1.5, 1.0));    // Orange-white HDR
    color_gradient.add_key(0.15, Vec4::new(3.0, 1.2, 0.3, 0.95));   // Orange
    color_gradient.add_key(0.4, Vec4::new(1.5, 0.4, 0.08, 0.7));    // Red-orange
    color_gradient.add_key(0.7, Vec4::new(0.6, 0.1, 0.02, 0.3));    // Dark red
    color_gradient.add_key(1.0, Vec4::new(0.1, 0.02, 0.01, 0.0));   // Extinct

    let effect = EffectAsset::new(
        64,
        SpawnerSettings::burst(25.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("muzzle_sparks")
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .update(gravity)
    .update(drag)
    .render(ColorOverLifetimeModifier { gradient: color_gradient, ..default() });

    effects.add(effect)
}

/// Layer 3: Smoke puff — residual gunpowder smoke, slow drift upward.
pub(super) fn create_muzzle_smoke(effects: &mut ResMut<Assets<EffectAsset>>) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.05).expr(),
        dimension: ShapeDimension::Volume,
    };

    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::new(0.0, 0.3, 0.0)).expr(),
        speed: writer.lit(0.2).uniform(writer.lit(0.8)).expr(),
    };

    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.04).uniform(writer.lit(0.10)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.3).uniform(writer.lit(0.8)).expr(),
    );

    // Slow upward drift + lateral asymmetry
    let accel = AccelModifier::new(writer.lit(Vec3::new(0.15, 0.4, -0.1)).expr());
    let drag = LinearDragModifier::new(writer.lit(3.0).expr());

    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(0.6, 0.55, 0.5, 0.5));   // Light gray, semi-transparent
    color_gradient.add_key(0.2, Vec4::new(0.4, 0.38, 0.35, 0.35));
    color_gradient.add_key(0.5, Vec4::new(0.25, 0.22, 0.2, 0.2));
    color_gradient.add_key(1.0, Vec4::new(0.1, 0.09, 0.08, 0.0));   // Fade out

    let mut size_gradient = HanabiGradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.04));
    size_gradient.add_key(0.2, Vec3::splat(0.12));
    size_gradient.add_key(0.5, Vec3::splat(0.22));
    size_gradient.add_key(1.0, Vec3::splat(0.35));   // Grows as it dissipates

    let effect = EffectAsset::new(
        48,
        SpawnerSettings::burst(15.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("muzzle_smoke")
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .update(accel)
    .update(drag)
    .render(ColorOverLifetimeModifier { gradient: color_gradient, ..default() })
    .render(SizeOverLifetimeModifier {
        gradient: size_gradient,
        screen_space_size: false,
    });

    effects.add(effect)
}

/// Layer 4: Heat glow — large soft envelope simulating heat distortion.
/// Few particles, big, very short lifetime, low opacity.
pub(super) fn create_muzzle_heat_glow(effects: &mut ResMut<Assets<EffectAsset>>) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.02).expr(),
        dimension: ShapeDimension::Volume,
    };

    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(0.05).uniform(writer.lit(0.2)).expr(),
    };

    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.3).uniform(writer.lit(0.6)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.03).uniform(writer.lit(0.07)).expr(),
    );

    // Warm orange glow, subtle, mostly for bloom halo
    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(3.0, 1.8, 0.8, 0.4));
    color_gradient.add_key(0.4, Vec4::new(1.8, 0.8, 0.3, 0.25));
    color_gradient.add_key(1.0, Vec4::new(0.6, 0.2, 0.05, 0.0));

    let effect = EffectAsset::new(
        12,
        SpawnerSettings::burst(8.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("muzzle_heat_glow")
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .render(ColorOverLifetimeModifier { gradient: color_gradient, ..default() });

    effects.add(effect)
}

/// Layer 5: Forward flash tongue — elongated burst along barrel direction.
/// Fast particles shot forward, drag-decelerated, bright.
pub(super) fn create_muzzle_forward_flash(effects: &mut ResMut<Assets<EffectAsset>>) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.01).expr(),
        dimension: ShapeDimension::Volume,
    };

    // Strong forward velocity — will be oriented by transform at spawn
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::new(0.0, 0.0, 1.0)).expr(), // +Z forward (oriented at spawn)
        speed: writer.lit(5.0).uniform(writer.lit(12.0)).expr(),
    };

    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.02).uniform(writer.lit(0.06)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.02).uniform(writer.lit(0.05)).expr(),
    );

    let drag = LinearDragModifier::new(writer.lit(8.0).expr()); // Strong drag = short tongue

    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(10.0, 8.0, 4.0, 1.0));   // White-yellow HDR
    color_gradient.add_key(0.2, Vec4::new(6.0, 3.0, 0.8, 0.9));    // Bright orange
    // Blue/purple accent (DOOM technique — pops against any background)
    color_gradient.add_key(0.5, Vec4::new(2.0, 0.8, 1.2, 0.5));    // Purple tint
    color_gradient.add_key(1.0, Vec4::new(0.3, 0.05, 0.1, 0.0));   // Fade

    let effect = EffectAsset::new(
        48,
        SpawnerSettings::burst(30.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("muzzle_forward_flash")
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .update(drag)
    .render(ColorOverLifetimeModifier { gradient: color_gradient, ..default() });

    effects.add(effect)
}
