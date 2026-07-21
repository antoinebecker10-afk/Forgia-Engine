//! Status VFX continus (DoT) : flamme sur brûlure, nuage toxique sur poison.
//! Diffèrent des muzzle/impact (burst one-shot, period énorme) : ici burst à
//! PÉRIODE COURTE = flux continu (la doc 0.18 : burst boucle à l'infini). On
//! évite `SpawnerSettings::rate` qui ne rendait rien dans ce repo. World-space +
//! suivi manuel (`status_vfx::sys_follow_status_vfx`). L'on/off = lease d'un slot
//! du pool persistant `StatusVfxPool` (attach = retrigger du spawner, detach =
//! `spawner.active = false` — plus AUCUN spawn/despawn d'entité par statut,
//! audit fire-path 2026-07-20 — cf `forgia-mode-roguelite/src/status_vfx.rs`).

use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_hanabi::Gradient as HanabiGradient;

use super::VfxTuning;

/// Capacité UNIFIÉE des 3 effets de statut (flamme/poison/shock) — condition du
/// pool partagé d'auras (`status_vfx::StatusVfxPool`) : le swap de handle sur un
/// slot vivant ne ré-alloue pas le slice EffectCache si capacité + attributs
/// sont identiques (précédent validé : pool élément, `element_vfx.rs`).
/// 128 = l'ancien max des trois (flamme), marge large pour chacun.
fn status_capacity(t: &VfxTuning) -> u32 {
    (128.0_f32 * t.count_mult).ceil() as u32
}

/// Flamme orange soutenue qui colle au corps d'un ennemi en feu.
/// HDR MODÉRÉ (pic ~2.6, leçon story-450 `muzzle.rs:23-47` : HDR fort + bloom
/// soutenu = blob blanc). Lifetime court 0.3-0.6 s = flicker.
pub(super) fn create_status_flame(
    effects: &mut ResMut<Assets<EffectAsset>>,
    t: &VfxTuning,
) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::new(0.0, 0.0, 0.0)).expr(),
        radius: writer.lit(0.5 * t.aura_width_mult).expr(), // enveloppe × largeur genome (anti-occlusion + « plus large autour du mob »)
        dimension: ShapeDimension::Volume,
    };
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::new(0.0, 1.4, 0.0)).expr(),
        speed: writer.lit(0.4).uniform(writer.lit(1.1)).expr(),
    };
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer
            .lit(0.14 * t.size_mult)
            .uniform(writer.lit(0.26 * t.size_mult))
            .expr(),
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

    // Story-652 : × size_mult — c'est CE gradient qui pilote la taille réelle
    // (il écrase init_size chaque frame), pas l'init. Oubli corrigé (les auras
    // restaient à l'ancienne taille alors que muzzle/impacts grossissaient).
    let mut size_gradient = HanabiGradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.10 * t.size_mult));
    size_gradient.add_key(0.3, Vec3::splat(0.28 * t.size_mult));
    size_gradient.add_key(0.7, Vec3::splat(0.20 * t.size_mult));
    size_gradient.add_key(1.0, Vec3::splat(0.05 * t.size_mult));

    let texture_slot = writer.lit(0u32).expr();

    let effect = EffectAsset::new(
        status_capacity(t), // ~30/s × lifetime 0.6s ≈ 18 vivants, marge confortable
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
    // Story-654 : additif — les flammes BRILLENT au lieu de faire un blob opaque.
    .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
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
    t: &VfxTuning,
) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::new(0.0, 0.0, 0.0)).expr(),
        radius: writer.lit(0.5 * t.aura_width_mult).expr(),
        dimension: ShapeDimension::Volume,
    };
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(0.1).uniform(writer.lit(0.45)).expr(),
    };
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer
            .lit(0.08 * t.size_mult)
            .uniform(writer.lit(0.16 * t.size_mult))
            .expr(),
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

    // Story-652 : × size_mult (cf flamme — le gradient pilote la taille réelle).
    let mut size_gradient = HanabiGradient::new();
    size_gradient.add_key(0.0, Vec3::splat(0.05 * t.size_mult));
    size_gradient.add_key(0.3, Vec3::splat(0.12 * t.size_mult));
    size_gradient.add_key(0.7, Vec3::splat(0.17 * t.size_mult));
    size_gradient.add_key(1.0, Vec3::splat(0.20 * t.size_mult));

    let texture_slot = writer.lit(0u32).expr();

    let effect = EffectAsset::new(
        status_capacity(t), // ~16/s × lifetime 1.0s ≈ 16 vivants (capacité unifiée pool)
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
    // Story-654 : additif — le nuage lumineux se lit mieux sur la scène sombre.
    .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
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

/// Arcs électriques crépitants sur un ennemi choqué (`StatusShock`) — story-653.
/// Contrairement à la flamme (léchures continues qui MONTENT), le shock =
/// CRÉPITEMENT : bursts fréquents d'étincelles très brèves à la SURFACE du corps,
/// stoppées net (drag fort), bleu-blanc HDR (couleur shock du genome elements
/// (0.35,0.65,1.0), boostée HDR modéré — leçon anti-blob story-450).
/// Texture : spark 4 branches (Kenney, partagée avec muzzle/impact sparks).
pub(super) fn create_status_shock(
    effects: &mut ResMut<Assets<EffectAsset>>,
    t: &VfxTuning,
) -> Handle<EffectAsset> {
    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::new(0.0, 0.0, 0.0)).expr(),
        radius: writer.lit(0.55 * t.aura_width_mult).expr(), // surface × largeur genome
        dimension: ShapeDimension::Surface,
    };
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        speed: writer.lit(0.8).uniform(writer.lit(2.2)).expr(),
    };
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer
            .lit(0.08 * t.size_mult)
            .uniform(writer.lit(0.16 * t.size_mult))
            .expr(),
    );
    // Très bref = crépitement (vs flamme 0.3-0.6 s).
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer
            .lit(0.06 * t.lifetime_mult)
            .uniform(writer.lit(0.16 * t.lifetime_mult))
            .expr(),
    );

    // Drag fort : l'arc jaillit puis se fige (pas de dérive de fumée).
    let drag = LinearDragModifier::new(writer.lit(6.0).expr());

    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(1.2, 1.8, 3.0, 1.0)); // blanc-bleu vif
    color_gradient.add_key(0.4, Vec4::new(0.5, 0.9, 2.4, 0.8)); // bleu électrique
    color_gradient.add_key(0.8, Vec4::new(0.15, 0.3, 1.0, 0.3)); // bleu profond
    color_gradient.add_key(1.0, Vec4::new(0.05, 0.1, 0.4, 0.0)); // extinction

    let texture_slot = writer.lit(0u32).expr();

    let effect = EffectAsset::new(
        status_capacity(t), // ~(4×count)/0.07s × lifetime ~0.2s ≈ 17×count vivants (capacité unifiée pool)
        // CONTINU via burst répété rapide (cf flamme : burst prouvé, PAS rate) :
        // 4 étincelles toutes les 70 ms = grésillement, pas un flux.
        SpawnerSettings::burst((4.0 * t.count_mult).into(), 0.07.into()),
        {
            // Story-653 : texture étincelle (slot "color", partagée tex_spark).
            let mut module = writer.finish();
            module.add_texture_slot("color");
            module
        },
    )
    .with_name("status_shock")
    .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
    // World-space (défaut) : repositionné chaque frame sur l'ennemi (cf flamme).
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .update(drag)
    .render(ParticleTextureModifier {
        texture_slot,
        sample_mapping: ImageSampleMapping::Modulate,
    })
    .render(ColorOverLifetimeModifier { gradient: color_gradient, ..default() });

    effects.add(effect)
}
