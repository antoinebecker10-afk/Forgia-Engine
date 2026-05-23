#![allow(dead_code, unused_imports)]
// Port verbatim de `forgia-game/src/effects/weapon_vfx/impact.rs` (V1).
//! Impact VFX — AAA bullet impact effects
//!
//! Layers:
//! 1. Impact sparks — hot metallic debris bouncing off surface
//! 2. Dust cloud — dust/concrete kicked up at impact point
//! 3. Impact flash — brief bright flash at hit point
//! + PointLight (brief illumination)
//! + ClusteredDecal bullet hole (when texture available)

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_hanabi::Gradient as HanabiGradient;

/// Layer 1: Impact sparks — directional debris bouncing off surface.
/// Gravity-affected, parabolic arcs, longer lived than muzzle sparks.
pub(super) fn create_impact_sparks(
    effects: &mut ResMut<Assets<EffectAsset>>,
) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.05).expr(),
        dimension: ShapeDimension::Surface,
    };

    // Hemisphere burst — sparks fly outward from surface
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::new(0.0, 1.0, 0.0)).expr(),
        speed: writer.lit(2.0).uniform(writer.lit(6.0)).expr(),
    };

    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.01).uniform(writer.lit(0.03)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.1).uniform(writer.lit(0.35)).expr(),
    );

    let gravity = AccelModifier::new(writer.lit(Vec3::new(0.0, -8.0, 0.0)).expr());
    let drag = LinearDragModifier::new(writer.lit(1.5).expr());

    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(5.0, 4.0, 2.0, 1.0)); // Bright orange-white
    color_gradient.add_key(0.1, Vec4::new(3.5, 1.5, 0.3, 0.95)); // Orange
    color_gradient.add_key(0.35, Vec4::new(1.8, 0.5, 0.08, 0.7)); // Red-orange
    color_gradient.add_key(0.6, Vec4::new(0.7, 0.15, 0.03, 0.35)); // Dark red
    color_gradient.add_key(1.0, Vec4::new(0.1, 0.02, 0.01, 0.0)); // Extinct

    let effect = EffectAsset::new(
        96,
        SpawnerSettings::burst(40.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("impact_sparks")
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

/// Layer 2: Dust cloud — kicked up at impact, expands and fades.
pub(super) fn create_impact_dust(effects: &mut ResMut<Assets<EffectAsset>>) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.08).expr(),
        dimension: ShapeDimension::Volume,
    };

    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::new(0.0, 0.5, 0.0)).expr(),
        speed: writer.lit(0.3).uniform(writer.lit(1.2)).expr(),
    };

    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.03).uniform(writer.lit(0.08)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.2).uniform(writer.lit(0.6)).expr(),
    );

    let accel = AccelModifier::new(writer.lit(Vec3::new(0.1, 0.3, -0.05)).expr());
    let drag = LinearDragModifier::new(writer.lit(4.0).expr());

    // Neutral brown-gray dust
    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(0.5, 0.4, 0.3, 0.6));
    color_gradient.add_key(0.2, Vec4::new(0.4, 0.35, 0.28, 0.45));
    color_gradient.add_key(0.5, Vec4::new(0.3, 0.25, 0.2, 0.25));
    color_gradient.add_key(1.0, Vec4::new(0.2, 0.18, 0.15, 0.0));

    let mut size_gradient = HanabiGradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.03));
    size_gradient.add_key(0.3, Vec3::splat(0.12));
    size_gradient.add_key(0.7, Vec3::splat(0.2));
    size_gradient.add_key(1.0, Vec3::splat(0.28));

    let effect = EffectAsset::new(
        48,
        SpawnerSettings::burst(20.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("impact_dust")
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

/// Layer 3: Impact flash — very brief bright burst at hit point.
pub(super) fn create_impact_flash(
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
        speed: writer.lit(0.1).uniform(writer.lit(0.3)).expr(),
    };

    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.05).uniform(writer.lit(0.12)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.02).uniform(writer.lit(0.04)).expr(),
    );

    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(10.0, 8.0, 5.0, 1.0)); // White-hot
    color_gradient.add_key(0.4, Vec4::new(5.0, 2.5, 0.8, 0.7)); // Yellow
    color_gradient.add_key(1.0, Vec4::new(1.0, 0.3, 0.05, 0.0)); // Fade

    let effect = EffectAsset::new(
        16,
        SpawnerSettings::burst(10.0.into(), 99999.0.into()),
        writer.finish(),
    )
    .with_name("impact_flash")
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
