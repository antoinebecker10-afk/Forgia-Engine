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
pub(super) fn create_muzzle_core_flash(
    effects: &mut ResMut<Assets<EffectAsset>>,
) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    // Story-450 v2 (2026-05-18 PM) — fix bloom blob screenshot user :
    // particles stackées au même point + HDR 12+ × bloom = blob blanc géant.
    // Solution : spread initial PLUS LARGE pour éviter stacking, burst /4,
    // HDR très réduit (max 3.0 vs 12.0).
    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.025).expr(),
        dimension: ShapeDimension::Volume,
    };

    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(0.2).uniform(writer.lit(0.8)).expr(),
    };

    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.015).uniform(writer.lit(0.035)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.025).uniform(writer.lit(0.05)).expr(),
    );

    // HDR MODÉRÉ — pas de blowout bloom. 3.0 max vs 12.0 avant.
    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(3.0, 2.5, 1.4, 1.0)); // Warm white doux
    color_gradient.add_key(0.3, Vec4::new(2.5, 1.5, 0.5, 0.85)); // Yellow
    color_gradient.add_key(0.6, Vec4::new(1.5, 0.6, 0.15, 0.45)); // Orange
    color_gradient.add_key(1.0, Vec4::new(0.4, 0.1, 0.02, 0.0)); // Fade

    let effect = EffectAsset::new(
        16,
        SpawnerSettings::burst(5.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("muzzle_core_flash")
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .render(ColorOverLifetimeModifier {
        gradient: color_gradient,
        ..default()
    });

    effects.add(effect)
}

/// Layer 2: Spark spray — hot metallic debris ejected from barrel.
/// Fast, gravity-affected, parabolic arcs.
pub(super) fn create_muzzle_sparks(
    effects: &mut ResMut<Assets<EffectAsset>>,
) -> Handle<EffectAsset> {
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

    // HDR doux pour sparks (était 4.0 → 2.0)
    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(2.0, 1.6, 0.75, 1.0)); // Orange-white
    color_gradient.add_key(0.15, Vec4::new(1.6, 0.7, 0.18, 0.95)); // Orange
    color_gradient.add_key(0.4, Vec4::new(0.9, 0.25, 0.05, 0.7)); // Red-orange
    color_gradient.add_key(0.7, Vec4::new(0.4, 0.07, 0.015, 0.3)); // Dark red
    color_gradient.add_key(1.0, Vec4::new(0.1, 0.02, 0.01, 0.0)); // Extinct

    let effect = EffectAsset::new(
        32,
        SpawnerSettings::burst(10.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("muzzle_sparks")
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .update(gravity)
    .update(drag)
    .render(ColorOverLifetimeModifier {
        gradient: color_gradient,
        ..default()
    });

    effects.add(effect)
}

/// Layer 3: Smoke puff — residual gunpowder smoke, slow drift upward.
pub(super) fn create_muzzle_smoke(
    effects: &mut ResMut<Assets<EffectAsset>>,
) -> Handle<EffectAsset> {
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

    // Story-450 — smoke sizes /2 + lifetime plus court : moins persistant devant la cible.
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.02).uniform(writer.lit(0.05)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.18).uniform(writer.lit(0.45)).expr(),
    );

    // Slow upward drift + lateral asymmetry
    let accel = AccelModifier::new(writer.lit(Vec3::new(0.15, 0.45, -0.1)).expr());
    let drag = LinearDragModifier::new(writer.lit(3.5).expr());

    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(0.6, 0.55, 0.5, 0.40)); // Plus transparent
    color_gradient.add_key(0.2, Vec4::new(0.4, 0.38, 0.35, 0.28));
    color_gradient.add_key(0.5, Vec4::new(0.25, 0.22, 0.2, 0.15));
    color_gradient.add_key(1.0, Vec4::new(0.1, 0.09, 0.08, 0.0));

    // Sizes /2 — grows mais reste discret.
    let mut size_gradient = HanabiGradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.02));
    size_gradient.add_key(0.2, Vec3::splat(0.06));
    size_gradient.add_key(0.5, Vec3::splat(0.11));
    size_gradient.add_key(1.0, Vec3::splat(0.17));

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
    .render(ColorOverLifetimeModifier {
        gradient: color_gradient,
        ..default()
    })
    .render(SizeOverLifetimeModifier {
        gradient: size_gradient,
        screen_space_size: false,
    });

    effects.add(effect)
}

/// Layer 4: Heat glow — large soft envelope simulating heat distortion.
/// Few particles, big, very short lifetime, low opacity.
pub(super) fn create_muzzle_heat_glow(
    effects: &mut ResMut<Assets<EffectAsset>>,
) -> Handle<EffectAsset> {
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

    // Story-450 v2 — heat glow /5 + HDR doux. Halo discret seulement.
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.06).uniform(writer.lit(0.12)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.03).uniform(writer.lit(0.07)).expr(),
    );

    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(1.5, 0.9, 0.4, 0.15));
    color_gradient.add_key(0.4, Vec4::new(0.9, 0.4, 0.15, 0.10));
    color_gradient.add_key(1.0, Vec4::new(0.3, 0.1, 0.025, 0.0));

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
    .render(ColorOverLifetimeModifier {
        gradient: color_gradient,
        ..default()
    });

    effects.add(effect)
}

/// Layer 5: Forward flash tongue — elongated burst along barrel direction.
/// Fast particles shot forward, drag-decelerated, bright.
pub(super) fn create_muzzle_forward_flash(
    effects: &mut ResMut<Assets<EffectAsset>>,
) -> Handle<EffectAsset> {
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

    // Story-450 v2 — forward flash : était le coupable principal du blob pink/yellow
    // screenshot user 2026-05-18 PM. Burst 30 + HDR 10 + purple gradient + bloom →
    // énorme blob solide centré. Solution : burst /4, HDR /3, drop le purple tint
    // qui créait le rendu rose pixelé.
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.008).uniform(writer.lit(0.025)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.02).uniform(writer.lit(0.05)).expr(),
    );

    let drag = LinearDragModifier::new(writer.lit(10.0).expr());

    // HDR doux, pas de purple tint (était le rose du screenshot).
    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(3.0, 2.4, 1.2, 1.0)); // Warm white
    color_gradient.add_key(0.3, Vec4::new(2.2, 1.1, 0.3, 0.85)); // Orange
    color_gradient.add_key(0.7, Vec4::new(1.0, 0.3, 0.05, 0.4)); // Deeper orange
    color_gradient.add_key(1.0, Vec4::new(0.2, 0.05, 0.01, 0.0)); // Fade

    let effect = EffectAsset::new(
        24,
        SpawnerSettings::burst(8.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("muzzle_forward_flash")
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .update(drag)
    .render(ColorOverLifetimeModifier {
        gradient: color_gradient,
        ..default()
    });

    effects.add(effect)
}
