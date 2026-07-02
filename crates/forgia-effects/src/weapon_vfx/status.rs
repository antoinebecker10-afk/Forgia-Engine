//! Status VFX continus (DoT) : flamme sur brûlure, nuage toxique sur poison.
//! Diffèrent des muzzle/impact (burst one-shot, period énorme) : ici burst à
//! PÉRIODE COURTE = flux continu (la doc 0.18 : burst boucle à l'infini). On
//! évite `SpawnerSettings::rate` qui ne rendait rien dans ce repo. World-space +
//! suivi manuel (`status_vfx::sys_follow_status_vfx`). L'on/off = cycle de vie de l'entité
//! `ParticleEffect` (spawn quand StatusBurn/Poison ajouté, despawn au retrait —
//! cf `forgia-mode-roguelite/src/status_vfx.rs`).

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_hanabi::Gradient as HanabiGradient;

/// Flamme orange soutenue qui colle au corps d'un ennemi en feu.
/// HDR MODÉRÉ (pic ~2.6, leçon story-450 `muzzle.rs:23-47` : HDR fort + bloom
/// soutenu = blob blanc). Lifetime court 0.3-0.6 s = flicker.
pub(super) fn create_status_flame(
    effects: &mut ResMut<Assets<EffectAsset>>,
) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::new(0.0, 0.0, 0.0)).expr(),
        radius: writer.lit(0.5).expr(), // large = enveloppe le corps, particules visibles aux bords (anti-occlusion)
        dimension: ShapeDimension::Volume,
    };
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::new(0.0, 1.4, 0.0)).expr(),
        speed: writer.lit(0.4).uniform(writer.lit(1.1)).expr(),
    };
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.14).uniform(writer.lit(0.26)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.3).uniform(writer.lit(0.6)).expr(),
    );

    let buoyancy = AccelModifier::new(writer.lit(Vec3::new(0.0, 1.2, 0.0)).expr());
    let drag = LinearDragModifier::new(writer.lit(2.5).expr());

    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(2.6, 1.1, 0.30, 0.95));
    color_gradient.add_key(0.25, Vec4::new(2.0, 0.65, 0.12, 0.9));
    color_gradient.add_key(0.55, Vec4::new(1.1, 0.25, 0.05, 0.7));
    color_gradient.add_key(0.8, Vec4::new(0.30, 0.10, 0.06, 0.35));
    color_gradient.add_key(1.0, Vec4::new(0.08, 0.07, 0.07, 0.0));

    let mut size_gradient = HanabiGradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.10));
    size_gradient.add_key(0.3, Vec3::splat(0.28));
    size_gradient.add_key(0.7, Vec3::splat(0.20));
    size_gradient.add_key(1.0, Vec3::splat(0.05));

    let texture_slot = writer.lit(0u32).expr();

    let effect = EffectAsset::new(
        128, // ~30/s × lifetime 0.6s ≈ 18 vivants, marge confortable
        // CONTINU via burst répété (3 particules toutes les 0.1s = 30/s). On
        // utilise `burst` (et PAS `rate`) car c'est l'API prouvée visible du repo
        // (muzzle/impact) ; `rate` ne rendait rien ici. La doc 0.18 confirme que
        // burst boucle à l'infini (count au début de chaque cycle, puis period).
        SpawnerSettings::burst(3.0.into(), 0.1.into()),
        {
            // Story-647 : texture léchure de flamme peinte (Kenney CC0).
            let mut module = writer.finish();
            module.add_texture_slot("color");
            module
        },
    )
    .with_name("status_flame")
    // World-space (défaut) : l'entité est repositionnée chaque frame sur l'ennemi
    // par `status_vfx::sys_follow_status_vfx` (le parenting ChildOf + Local ne
    // rendait rien). Léger trail si l'ennemi court — OK pour du feu.
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .update(buoyancy)
    .update(drag)
    .render(ParticleTextureModifier {
        texture_slot,
        sample_mapping: ImageSampleMapping::Modulate,
    })
    .render(ColorOverLifetimeModifier { gradient: color_gradient, ..default() })
    .render(SizeOverLifetimeModifier { gradient: size_gradient, screen_space_size: false });

    effects.add(effect)
}

/// Nuage toxique vert soutenu autour d'un ennemi empoisonné.
/// Plus gros, plus lent, plus longue durée que la flamme. HDR doux (pic vert
/// ~1.6) : un nuage continu empile plus de particules → plus sensible au blob.
pub(super) fn create_status_poison_cloud(
    effects: &mut ResMut<Assets<EffectAsset>>,
) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::new(0.0, 0.0, 0.0)).expr(),
        radius: writer.lit(0.5).expr(),
        dimension: ShapeDimension::Volume,
    };
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(0.1).uniform(writer.lit(0.45)).expr(),
    };
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer.lit(0.08).uniform(writer.lit(0.16)).expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer.lit(0.6).uniform(writer.lit(1.0)).expr(),
    );

    let buoyancy = AccelModifier::new(writer.lit(Vec3::new(0.05, 0.6, -0.05)).expr());
    let drag = LinearDragModifier::new(writer.lit(2.5).expr());

    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(0.25, 1.6, 0.25, 0.0));
    color_gradient.add_key(0.15, Vec4::new(0.30, 1.5, 0.20, 0.55));
    color_gradient.add_key(0.5, Vec4::new(0.55, 1.2, 0.15, 0.45));
    color_gradient.add_key(0.8, Vec4::new(0.30, 0.55, 0.10, 0.22));
    color_gradient.add_key(1.0, Vec4::new(0.12, 0.25, 0.05, 0.0));

    let mut size_gradient = HanabiGradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.05));
    size_gradient.add_key(0.3, Vec3::splat(0.12));
    size_gradient.add_key(0.7, Vec3::splat(0.17));
    size_gradient.add_key(1.0, Vec3::splat(0.20));

    let texture_slot = writer.lit(0u32).expr();

    let effect = EffectAsset::new(
        96, // ~16/s × lifetime 1.0s ≈ 16 vivants
        // CONTINU via burst répété (2 particules toutes les 0.12s ≈ 16/s) — cf flamme.
        SpawnerSettings::burst(2.0.into(), 0.12.into()),
        {
            // Story-647 : texture volutes de fumée (Kenney CC0).
            let mut module = writer.finish();
            module.add_texture_slot("color");
            module
        },
    )
    .with_name("status_poison_cloud")
    // World-space (défaut) : repositionné chaque frame sur l'ennemi (cf flamme).
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .update(buoyancy)
    .update(drag)
    .render(ParticleTextureModifier {
        texture_slot,
        sample_mapping: ImageSampleMapping::Modulate,
    })
    .render(ColorOverLifetimeModifier { gradient: color_gradient, ..default() })
    .render(SizeOverLifetimeModifier { gradient: size_gradient, screen_space_size: false });

    effects.add(effect)
}
