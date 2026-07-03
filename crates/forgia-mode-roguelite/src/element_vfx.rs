//! element_vfx.rs — Story-588 (2026-06-09). VFX colorés des éléments.
//!
//! Rend le système d'éléments (story-582) VISIBLE :
//! - **flash coloré à l'impact** de chaque hit élémentaire (couleur par élément,
//!   plus gros pour l'explosif = splash),
//! - **pulse coloré** périodique sur les ennemis en DoT (burn = orange,
//!   poison = vert).
//!
//! ## Perf (chemin par-hit, haute fréquence SMG)
//!
//! Contrairement à `shockwave.rs` (1 matériau alloué par cast, OK car cooldown
//! long), les impacts sont **par-hit**. On partage donc **1 mesh sphère + 4
//! matériaux** (1 par élément), construits une fois ([`ElementVfxAssets`]).
//! Le fade se fait **par scale** (sphère → 0) + intensité de lumière par-entité
//! → **zéro allocation matériau/hit**. Le hot-reload des couleurs ré-applique
//! en place (`materials.get_mut`, mêmes handles) donc les sparks vivants
//! changent aussi.
//!
//! La lumière n'est attachée qu'aux **impacts** (les pulses DoT n'en ont pas) →
//! le nombre de `PointLight` simultanés reste borné. Cap global d'instances
//! ([`MAX_ACTIVE_SPARKS`]) en garde-fou anti-spam.

use bevy::prelude::*;
use bevy::state::state_scoped::DespawnOnExit;
use bevy_hanabi::Gradient as HanabiGradient;
use bevy_hanabi::{
    AccelModifier, Attribute, ColorOverLifetimeModifier, EffectAsset, ExprWriter,
    ImageSampleMapping, LinearDragModifier, ParticleTextureModifier, SetAttributeModifier,
    SetPositionSphereModifier, SetVelocitySphereModifier, ShapeDimension, SpawnerSettings,
};
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_core::prelude::*;
use forgia_effects::prelude::{
    EffectMaterial, Lifetime as VfxLifetime, ParticleEffect, VfxTuning, WeaponVfxEffects,
};

use crate::elements::{Element, ElementConfig, ElementUnlocks, ReactionEvent};

const SENSOR_PATH: &str = "forgia2_element_vfx.json";
const POLL_PERIOD_SEC: f32 = 1.0;
/// Garde-fou : au-delà, on ne spawn plus de flash (anti-spam cadence SMG).
const MAX_ACTIVE_SPARKS: usize = 64;
/// Boost émissif (bloom-friendly) appliqué à la couleur de base de l'élément.
const EMISSIVE_BOOST: f32 = 3.0;

// ─── Assets partagés (1 mesh + 4 matériaux) ─────────────────────────────────

#[derive(Resource)]
pub struct ElementVfxAssets {
    pub sphere: Handle<Mesh>,
    /// Indexé par [`Element::idx`] (Fire=0, Poison=1, Shock=2, ArmorPierce=3).
    pub mats: [Handle<StandardMaterial>; 4],
}

/// Flash/pulse élémentaire : fade par scale (sphère → 0) + intensité lumière.
#[derive(Component)]
pub struct ElementSpark {
    pub age: f32,
    pub ttl: f32,
    pub start_scale: f32,
    /// Intensité initiale de la lumière (0 = pas de lumière, ex. pulse DoT).
    pub light0: f32,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct ElementVfxStats {
    pub sparks_spawned: u32,
    pub dot_pulses: u32,
    /// Bursts de réaction spawnés (Combustion + Surcharge). Miasma (DoT) n'en émet pas.
    pub reaction_bursts: u32,
}

/// Configure un matériau d'élément (unlit + émissif coloré + blend). Partagé →
/// appelé à l'init ET au hot-reload (mêmes handles).
fn apply_element_material(m: &mut StandardMaterial, rgb: [f32; 3]) {
    let [r, g, b] = rgb;
    m.base_color = Color::srgba(r, g, b, 0.85);
    m.emissive = LinearRgba::new(r * EMISSIVE_BOOST, g * EMISSIVE_BOOST, b * EMISSIVE_BOOST, 1.0);
    m.unlit = true;
    m.alpha_mode = AlphaMode::Blend;
}

const ALL_ELEMENTS: [Element; 4] = [
    Element::Fire,
    Element::Poison,
    Element::Shock,
    Element::ArmorPierce,
];

// ─── Init / hot-reload des matériaux ────────────────────────────────────────

/// Startup (après `elements::sys_init_element_genome`) : construit le mesh +
/// les 4 matériaux depuis les couleurs du genome. `Option<Res>` + fallback
/// `default` → robuste à l'ordre d'init des Commands.
pub fn sys_init_vfx_assets(
    mut commands: Commands,
    config: Option<Res<ElementConfig>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let vfx = config.map(|c| c.vfx.clone()).unwrap_or_default();
    let sphere = meshes.add(Sphere::new(1.0));
    let mats = ALL_ELEMENTS.map(|e| {
        let mut m = StandardMaterial::default();
        apply_element_material(&mut m, e.rgb(&vfx));
        materials.add(m)
    });
    commands.insert_resource(ElementVfxAssets { sphere, mats });
}

/// Ré-applique les couleurs en place quand le genome change (hot-reload). Les
/// handles sont partagés → tous les sparks vivants changent aussi.
pub fn sys_refresh_vfx_materials(
    config: Res<ElementConfig>,
    assets: Option<Res<ElementVfxAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !config.is_changed() {
        return;
    }
    let Some(assets) = assets else {
        return;
    };
    for e in ALL_ELEMENTS {
        if let Some(m) = materials.get_mut(&assets.mats[e.idx()]) {
            apply_element_material(m, e.rgb(&config.vfx));
        }
    }
}

/// OnEnter Roguelite — reset des compteurs sensor (run fraîche).
pub fn sys_reset_vfx_stats(mut stats: ResMut<ElementVfxStats>) {
    *stats = ElementVfxStats::default();
}

// ─── Bursts hanabi par élément (story-655 : fin des sphères procédurales) ────

/// VRAIS bursts texturés par élément — remplacent les sphères émissives
/// (demande user 2026-07-03 « remplace les boules procédurales »). Couleurs
/// baked depuis le genome élément au build (contrairement aux sphères
/// hot-reload : tradeoff accepté, les couleurs sont stables). La `PointLight`
/// par impact est CONSERVÉE (elle peint le décor à la couleur de l'arme).
#[derive(Resource)]
pub struct ElementBurstAssets {
    /// Indexé par [`Element::idx`].
    pub effects: [Handle<EffectAsset>; 4],
    /// TTL de l'entité burst — DOIT dépasser le lifetime max des particules
    /// (despawn de l'entité hanabi = particules coupées net).
    pub entity_ttl: f32,
}

/// Texture du burst par élément (partage les textures Kenney de weapon_vfx).
fn element_burst_texture(w: &WeaponVfxEffects, e: Element) -> Handle<Image> {
    match e {
        Element::Fire => w.tex_flame.clone(),
        Element::Poison => w.tex_poison.clone(),
        Element::Shock | Element::ArmorPierce => w.tex_spark.clone(),
    }
}

/// Builder d'un burst d'impact élémentaire (texturé, additif, radial + biais haut).
fn create_element_burst(
    effects: &mut Assets<EffectAsset>,
    t: &VfxTuning,
    rgb: [f32; 3],
    name: &str,
) -> Handle<EffectAsset> {
    let [r, g, b] = rgb;
    let writer = ExprWriter::new();
    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.06).expr(),
        dimension: ShapeDimension::Volume,
    };
    let init_vel = SetVelocitySphereModifier {
        center: writer.lit(Vec3::new(0.0, 0.6, 0.0)).expr(),
        speed: writer.lit(1.2).uniform(writer.lit(3.0)).expr(),
    };
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        writer
            .lit(0.05 * t.size_mult)
            .uniform(writer.lit(0.11 * t.size_mult))
            .expr(),
    );
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer
            .lit(0.22 * t.lifetime_mult)
            .uniform(writer.lit(0.4 * t.lifetime_mult))
            .expr(),
    );
    let gravity = AccelModifier::new(writer.lit(Vec3::new(0.0, -2.0, 0.0)).expr());
    let drag = LinearDragModifier::new(writer.lit(3.5).expr());
    // Couleur = l'élément, HDR décroissant (additif : brille sans blob).
    let mut color_gradient = HanabiGradient::new();
    color_gradient.add_key(0.0, Vec4::new(r * 2.8, g * 2.8, b * 2.8, 1.0));
    color_gradient.add_key(0.4, Vec4::new(r * 1.6, g * 1.6, b * 1.6, 0.85));
    color_gradient.add_key(0.8, Vec4::new(r * 0.5, g * 0.5, b * 0.5, 0.4));
    color_gradient.add_key(1.0, Vec4::new(0.0, 0.0, 0.0, 0.0));
    let texture_slot = writer.lit(0u32).expr();
    let mut module = writer.finish();
    module.add_texture_slot("color");

    let effect = EffectAsset::new(
        (48.0_f32 * t.count_mult).ceil() as u32,
        SpawnerSettings::burst((12.0 * t.count_mult).into(), 99999.0.into()),
        module,
    )
    .with_name(name.to_string())
    .with_alpha_mode(bevy_hanabi::AlphaMode::Add)
    .init(init_pos)
    .init(init_vel)
    .init(init_size)
    .init(init_lifetime)
    .update(gravity)
    .update(drag)
    .render(ParticleTextureModifier {
        texture_slot,
        sample_mapping: ImageSampleMapping::Modulate,
    })
    .render(ColorOverLifetimeModifier {
        gradient: color_gradient,
        ..default()
    });
    effects.add(effect)
}

/// PostStartup — après le genome éléments et les textures weapon_vfx : construit
/// les 4 bursts + warmup shader (1 dummy caché par asset, leçon anti-freeze
/// story-594 : la 1re compile hanabi coûte des secondes).
pub fn sys_init_element_bursts(
    mut commands: Commands,
    mut effect_assets: ResMut<Assets<EffectAsset>>,
    config: Option<Res<ElementConfig>>,
    tuning: Option<Res<VfxTuning>>,
    weapon_vfx: Option<Res<WeaponVfxEffects>>,
) {
    let vfx = config.map(|c| c.vfx.clone()).unwrap_or_default();
    let t = tuning.as_deref().copied().unwrap_or_default();
    let names = ["element_burst_fire", "element_burst_poison", "element_burst_shock", "element_burst_pierce"];
    let handles =
        ALL_ELEMENTS.map(|e| create_element_burst(&mut effect_assets, &t, e.rgb(&vfx), names[e.idx()]));
    if let Some(w) = weapon_vfx.as_deref() {
        for e in ALL_ELEMENTS {
            commands.spawn((
                ParticleEffect::new(handles[e.idx()].clone()),
                EffectMaterial {
                    images: vec![element_burst_texture(w, e)],
                },
                Transform::from_xyz(0.0, -10_000.0, 0.0),
                Visibility::Hidden,
                VfxLifetime(Timer::from_seconds(5.0, TimerMode::Once)),
            ));
        }
    }
    commands.insert_resource(ElementBurstAssets {
        effects: handles,
        entity_ttl: 0.4 * t.lifetime_mult + 0.3,
    });
    info!("[element-vfx] 4 bursts hanabi construits (story-655 — fin des sphères)");
}

// ─── Spawn : flash à l'impact ───────────────────────────────────────────────

/// Lit `CombatHitEvent` (même producteur que `sys_apply_elements_on_hit`) et
/// spawn un flash coloré à `hit_world_pos`. L'explosif est plus gros (splash) et
/// 2× plus lumineux. Gaté par le MÊME check que l'application réelle
/// (`unlocks.is_unlocked`, story-589) + `vfx.enabled` — sinon flash sans dégâts.
pub fn sys_spawn_element_impact(
    mut events: MessageReader<CombatHitEvent>,
    config: Res<ElementConfig>,
    unlocks: Res<ElementUnlocks>,
    vfx_tuning: Option<Res<VfxTuning>>,
    q_sparks: Query<(), With<ElementSpark>>,
    q_pos: Query<&GlobalTransform>,
    mut stats: ResMut<ElementVfxStats>,
    mut commands: Commands,
    // Story-655 — vrais bursts texturés (remplacent les sphères procédurales).
    bursts: Option<Res<ElementBurstAssets>>,
    weapon_vfx: Option<Res<WeaponVfxEffects>>,
) {
    let (Some(bursts), Some(weapon_vfx)) = (bursts, weapon_vfx) else {
        return;
    };
    // Story-652 Inc.2 — échelle + offset unifiés avec les VFX d'armes
    // (roguelite_vfx.toml : un seul curseur pour tout le feedback de combat).
    let tuning = vfx_tuning.as_deref().copied().unwrap_or_default();
    let mut live = q_sparks.iter().count();
    for ev in events.read() {
        if !config.vfx.enabled || live >= MAX_ACTIVE_SPARKS {
            continue;
        }
        let Some(weapon) = ev.weapon else {
            continue;
        };
        let Some(element) = config.element_for(weapon) else {
            continue;
        };
        // Cohérence VFX↔dégâts : même gate que sys_apply_elements_on_hit.
        if !unlocks.is_unlocked(element) {
            continue;
        }
        // Arc électrique (ex-explosif) : flash plus large + lumière ×2 (remap story-641).
        let arc = element == Element::Shock;
        let scale = config.vfx.impact_scale
            * if arc { config.vfx.arc_scale } else { 1.0 }
            * tuning.size_mult;
        let light0 = config.vfx.light_intensity * if arc { 2.0 } else { 1.0 };
        let [r, g, b] = element.rgb(&config.vfx);
        // Offset HORS de la surface, vers le tireur (architecture standard —
        // sinon la sphère naît à moitié DANS le mesh du mob, occluse).
        let spawn_pos = match ev.attacker.and_then(|a| q_pos.get(a).ok()) {
            Some(a_gt) => {
                let toward_shooter =
                    (a_gt.translation() - ev.hit_world_pos).normalize_or_zero();
                ev.hit_world_pos + toward_shooter * tuning.impact_offset_m
            }
            None => ev.hit_world_pos,
        };
        // Story-655 — VRAI burst texturé (flamme/volutes/étincelles selon
        // l'élément) au lieu de la sphère émissive.
        commands.spawn((
            ParticleEffect::new(bursts.effects[element.idx()].clone()),
            EffectMaterial {
                images: vec![element_burst_texture(&weapon_vfx, element)],
            },
            Transform::from_translation(spawn_pos).with_scale(Vec3::splat(scale)),
            VfxLifetime(Timer::from_seconds(bursts.entity_ttl, TimerMode::Once)),
            DespawnOnExit(GameMode::Roguelite),
        ));
        // La lumière colorée est CONSERVÉE (entité dédiée — `ElementSpark` gère
        // son fade/despawn ; le scale tick est sans effet, plus de mesh).
        commands.spawn((
            Transform::from_translation(spawn_pos),
            PointLight {
                color: Color::srgb(r, g, b),
                intensity: light0,
                range: config.vfx.light_range,
                shadows_enabled: false,
                ..default()
            },
            ElementSpark { age: 0.0, ttl: config.vfx.impact_ttl, start_scale: scale, light0 },
            DespawnOnExit(GameMode::Roguelite),
        ));
        stats.sparks_spawned = stats.sparks_spawned.saturating_add(1);
        live += 1;
    }
}

// ─── Pulse DoT sur l'ennemi ─────────────────────────────────────────────────
// REMPLACÉ (story-611) par `status_vfx.rs` : vraies particules hanabi continues
// (flamme sur StatusBurn, nuage toxique sur StatusPoison, attachées à l'ennemi).
// L'ancien dot-pulse sphère jetable est retiré. `stats.dot_pulses` est ré-
// incrémenté par les systèmes d'attache → le sensor reste honnête.

// ─── Burst de réaction (Combustion / Surcharge) ─────────────────────────────

/// Lit `ReactionEvent` (émis par `elements::sys_apply_elements_on_hit` pour les
/// réactions de type **décharge** : Combustion, Surcharge) et spawn un burst avec
/// les 2 couleurs du couple d'éléments (`ReactionKind::pair`) : sphère grande +
/// lumineuse pour le 1er élément, halo plus petit pour le 2e → la fusion est
/// lisible. Throttlé en amont (cooldown/cible) donc pas de cap anti-spam ici.
/// Miasma (DoT stackant) ne passe pas ici — son visuel viendra de `status_vfx` (P1).
pub fn sys_spawn_reaction_vfx(
    mut events: MessageReader<ReactionEvent>,
    config: Res<ElementConfig>,
    mut stats: ResMut<ElementVfxStats>,
    mut commands: Commands,
    // Story-655 — vrais bursts texturés (remplacent les 2 sphères de fusion).
    bursts: Option<Res<ElementBurstAssets>>,
    weapon_vfx: Option<Res<WeaponVfxEffects>>,
) {
    let (Some(bursts), Some(weapon_vfx)) = (bursts, weapon_vfx) else {
        return;
    };
    if !config.vfx.enabled {
        return;
    }
    let ttl = config.vfx.impact_ttl * 2.0;
    for ev in events.read() {
        let (primary, secondary) = ev.kind.pair();
        let [r, g, b] = primary.rgb(&config.vfx);
        let light0 = config.vfx.light_intensity * 3.0;
        // Story-655 — burst texturé du 1er élément (grand) + halo du 2e (plus
        // petit) : la fusion se lit par la superposition des deux couleurs.
        commands.spawn((
            ParticleEffect::new(bursts.effects[primary.idx()].clone()),
            EffectMaterial {
                images: vec![element_burst_texture(&weapon_vfx, primary)],
            },
            Transform::from_translation(ev.pos).with_scale(Vec3::splat(ev.radius)),
            VfxLifetime(Timer::from_seconds(bursts.entity_ttl, TimerMode::Once)),
            DespawnOnExit(GameMode::Roguelite),
        ));
        let halo = ev.radius * 0.6;
        commands.spawn((
            ParticleEffect::new(bursts.effects[secondary.idx()].clone()),
            EffectMaterial {
                images: vec![element_burst_texture(&weapon_vfx, secondary)],
            },
            Transform::from_translation(ev.pos).with_scale(Vec3::splat(halo)),
            VfxLifetime(Timer::from_seconds(bursts.entity_ttl, TimerMode::Once)),
            DespawnOnExit(GameMode::Roguelite),
        ));
        // Lumière de fusion conservée (entité dédiée, fade par ElementSpark).
        commands.spawn((
            Transform::from_translation(ev.pos),
            PointLight {
                color: Color::srgb(r, g, b),
                intensity: light0,
                range: ev.radius * 2.0,
                shadows_enabled: false,
                ..default()
            },
            ElementSpark { age: 0.0, ttl, start_scale: ev.radius, light0 },
            DespawnOnExit(GameMode::Roguelite),
        ));
        stats.reaction_bursts = stats.reaction_bursts.saturating_add(1);
    }
}

// ─── Flammes de bouche de Bourrasque (élément Feu) ──────────────────────────

// ─── Tick : fade + despawn ──────────────────────────────────────────────────

pub fn sys_tick_element_sparks(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut ElementSpark, Option<&mut PointLight>)>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut spark, light) in &mut q {
        spark.age += dt;
        let t = if spark.ttl > 0.0 {
            (spark.age / spark.ttl).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let k = 1.0 - t;
        tf.scale = Vec3::splat(spark.start_scale * k);
        if let Some(mut l) = light {
            l.intensity = spark.light0 * k;
        }
        if spark.age >= spark.ttl {
            commands.entity(e).despawn();
        }
    }
}

// ─── Sensor forgia2_element_vfx.json ────────────────────────────────────────

pub fn sys_write_element_vfx_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    config: Res<ElementConfig>,
    stats: Res<ElementVfxStats>,
    q_sparks: Query<(), With<ElementSpark>>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;

    let active = q_sparks.iter().count();
    let (severity, next_step) = if config.vfx.enabled {
        ("ok", "")
    } else {
        (
            "warn",
            "vfx.enabled=0 — éléments invisibles (set 1 dans roguelite_elements.toml [vfx])",
        )
    };

    let v = &config.vfx;
    let json = format!(
        r#"{{"id":"element_vfx","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"enabled":{},"sparks_spawned":{},"dot_pulses":{},"reaction_bursts":{},"active_sparks":{active},"colors":{{"fire":[{:.2},{:.2},{:.2}],"poison":[{:.2},{:.2},{:.2}],"shock":[{:.2},{:.2},{:.2}],"armor_pierce":[{:.2},{:.2},{:.2}]}}}}"#,
        time.elapsed_secs(),
        v.enabled,
        stats.sparks_spawned,
        stats.dot_pulses,
        stats.reaction_bursts,
        v.fire_rgb[0], v.fire_rgb[1], v.fire_rgb[2],
        v.poison_rgb[0], v.poison_rgb[1], v.poison_rgb[2],
        v.shock_rgb[0], v.shock_rgb[1], v.shock_rgb[2],
        v.armor_pierce_rgb[0], v.armor_pierce_rgb[1], v.armor_pierce_rgb[2],
    );

    if let Err(e) = std::fs::write(SENSOR_PATH, &json) {
        warn!("[element-vfx] sensor write failed: {e}");
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_apply_sets_emissive_from_rgb() {
        let mut m = StandardMaterial::default();
        apply_element_material(&mut m, [1.0, 0.5, 0.0]);
        assert!(m.unlit);
        assert_eq!(m.emissive.red, 1.0 * EMISSIVE_BOOST);
        assert_eq!(m.emissive.green, 0.5 * EMISSIVE_BOOST);
    }

    #[test]
    fn all_elements_index_into_four_mats() {
        let idx: Vec<usize> = ALL_ELEMENTS.iter().map(|e| e.idx()).collect();
        assert_eq!(idx, vec![0, 1, 2, 3]);
    }
}
