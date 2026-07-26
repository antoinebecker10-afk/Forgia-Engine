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
//! La lumière n'est attachée qu'aux **impacts** (les pulses DoT n'en ont pas).
//! LOT B (audit fire-path 2026-07-20) : les bursts + lumières d'impact sont
//! des **pools persistants** ([`ElementBurstPool`], `ELEMENT_BURST_POOL_SIZE`
//! slots) — le round-robin borne nativement la concurrence, plus de
//! spawn/despawn par hit (anti-pattern hanabi #255).

use bevy::prelude::*;
use bevy_hanabi::Gradient as HanabiGradient;
use bevy_hanabi::{
    AccelModifier, Attribute, ColorOverLifetimeModifier, EffectAsset, EffectSpawner, ExprWriter,
    ImageSampleMapping, LinearDragModifier, ParticleTextureModifier, SetAttributeModifier,
    SetPositionSphereModifier, SetVelocitySphereModifier, ShapeDimension, SpawnerSettings,
};
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_effects::prelude::{EffectMaterial, ParticleEffect, VfxTuning, WeaponVfxEffects};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::elements::{Element, ElementConfig, ElementUnlocks, ReactionEvent};

const SENSOR_PATH: &str = "forgia2_element_vfx.json";
const POLL_PERIOD_SEC: f32 = 1.0;
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
    m.emissive = LinearRgba::new(
        r * EMISSIVE_BOOST,
        g * EMISSIVE_BOOST,
        b * EMISSIVE_BOOST,
        1.0,
    );
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
}

/// Texture du burst par élément (partage les textures Kenney de weapon_vfx).
fn element_burst_texture(w: &WeaponVfxEffects, e: Element) -> Handle<Image> {
    match e {
        Element::Fire => w.tex_flame.clone(),
        Element::Poison => w.tex_poison.clone(),
        Element::Shock | Element::ArmorPierce => w.tex_spark.clone(),
    }
}

// ─── LOT B (audit fire-path 2026-07-20) — pool persistant des bursts ────────
// Best practice bevy_hanabi (issue #255) : réutiliser un spawner existant
// (repositionner + retrigger) plutôt que spawn/despawn une entité par hit.

/// Nombre de slots round-robin PARTAGÉS entre tous les éléments (perf interne,
/// pas exposé créateur). Le hot-swap `ParticleEffect.handle`/`EffectMaterial`
/// est acceptable ici : les 4 bursts élément partagent la MÊME particle-layout
/// (mêmes attributs/capacité, seule couleur/texture diffère) → pas de
/// réallocation EffectCache, juste une reconfiguration légère du
/// `CompiledParticleEffect` (cf `compile_effects` dans bevy_hanabi). Un pool
/// dédié par élément (4×2) aurait évité ce swap mais sature sous Bourrasque
/// (11 tirs/s, élément Feu unique) — 8 slots PARTAGÉS absorbent la cadence
/// réelle sans troncature visuelle prématurée.
const ELEMENT_BURST_POOL_SIZE: usize = 8;

/// Pool persistant des bursts élément : `particles[i]` et `lights[i]` forment
/// TOUJOURS une paire (même position). Jamais despawné.
#[derive(Resource)]
pub struct ElementBurstPool {
    particles: [Entity; ELEMENT_BURST_POOL_SIZE],
    lights: [Entity; ELEMENT_BURST_POOL_SIZE],
    cursor: AtomicUsize,
}

impl ElementBurstPool {
    /// Pioche le prochain index round-robin (thread-safe : `Res<>` immuable).
    fn next_index(&self) -> usize {
        self.cursor.fetch_add(1, Ordering::Relaxed) % ELEMENT_BURST_POOL_SIZE
    }
}

/// Reconfigure une entité de pool (déjà vivante) pour émettre un NOUVEAU burst
/// élémentaire : swap handle+texture, repositionne, puis force une réémission
/// via `Commands` (retire `EffectSpawner` — hanabi le ré-ajoute frais au
/// prochain tick, cf `tick_spawners()`, comportement identique à un spawn
/// neuf sans jamais recréer l'entité ni son slot GPU). LOT B, 2026-07-20.
#[allow(clippy::type_complexity)]
fn retrigger_element_particle(
    commands: &mut Commands,
    q_particle: &mut Query<
        (
            &mut ParticleEffect,
            &mut EffectMaterial,
            &mut Transform,
            &mut Visibility,
        ),
        Without<PointLight>,
    >,
    entity: Entity,
    handle: Handle<EffectAsset>,
    tex: Handle<Image>,
    pos: Vec3,
    scale: f32,
) {
    if let Ok((mut effect, mut material, mut tf, mut vis)) = q_particle.get_mut(entity) {
        effect.handle = handle;
        material.images = vec![tex];
        *tf = Transform::from_translation(pos).with_scale(Vec3::splat(scale));
        *vis = Visibility::Visible;
    }
    commands.entity(entity).remove::<EffectSpawner>();
}

/// Repositionne + relance le decay (`ElementSpark`) d'une lumière de pool —
/// jamais despawnée (LOT B).
#[allow(clippy::too_many_arguments)]
fn retrigger_element_light(
    q_light: &mut Query<(
        &mut Transform,
        &mut PointLight,
        &mut Visibility,
        &mut ElementSpark,
    )>,
    entity: Entity,
    pos: Vec3,
    rgb: [f32; 3],
    intensity: f32,
    range: f32,
    ttl: f32,
    start_scale: f32,
) {
    if let Ok((mut tf, mut light, mut vis, mut spark)) = q_light.get_mut(entity) {
        *tf = Transform::from_translation(pos);
        light.color = Color::srgb(rgb[0], rgb[1], rgb[2]);
        light.intensity = intensity;
        light.range = range;
        *vis = Visibility::Visible;
        spark.age = 0.0;
        spark.ttl = ttl;
        spark.start_scale = start_scale;
        spark.light0 = intensity;
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
/// les 4 bursts ET le pool persistant de `ELEMENT_BURST_POOL_SIZE` slots
/// (LOT B, 2026-07-20). Les slots naissent cachés à Y=-10000 : leur toute
/// première émission (invisible) paie AUSSI le warmup shader (leçon
/// anti-freeze story-594 — remplace l'ancien dummy jetable).
pub fn sys_init_element_bursts(
    mut commands: Commands,
    mut effect_assets: ResMut<Assets<EffectAsset>>,
    config: Option<Res<ElementConfig>>,
    tuning: Option<Res<VfxTuning>>,
    weapon_vfx: Option<Res<WeaponVfxEffects>>,
) {
    let vfx = config.map(|c| c.vfx.clone()).unwrap_or_default();
    let t = tuning.as_deref().copied().unwrap_or_default();
    let names = [
        "element_burst_fire",
        "element_burst_poison",
        "element_burst_shock",
        "element_burst_pierce",
    ];
    let handles = ALL_ELEMENTS
        .map(|e| create_element_burst(&mut effect_assets, &t, e.rgb(&vfx), names[e.idx()]));
    if let Some(w) = weapon_vfx.as_deref() {
        let hidden = Transform::from_xyz(0.0, -10_000.0, 0.0);
        let particles: [Entity; ELEMENT_BURST_POOL_SIZE] = std::array::from_fn(|i| {
            let e = ALL_ELEMENTS[i % ALL_ELEMENTS.len()];
            commands
                .spawn((
                    ParticleEffect::new(handles[e.idx()].clone()),
                    EffectMaterial {
                        images: vec![element_burst_texture(w, e)],
                    },
                    hidden,
                    Visibility::Hidden,
                ))
                .id()
        });
        let lights: [Entity; ELEMENT_BURST_POOL_SIZE] = std::array::from_fn(|_| {
            commands
                .spawn((
                    hidden,
                    PointLight {
                        intensity: 0.0,
                        shadows_enabled: false,
                        ..default()
                    },
                    Visibility::Hidden,
                    ElementSpark {
                        age: 1.0,
                        ttl: 1.0,
                        start_scale: 0.0,
                        light0: 0.0,
                    },
                ))
                .id()
        });
        commands.insert_resource(ElementBurstPool {
            particles,
            lights,
            cursor: AtomicUsize::new(0),
        });
    }
    commands.insert_resource(ElementBurstAssets { effects: handles });
    info!(
        "[element-vfx] pool persistant de {ELEMENT_BURST_POOL_SIZE} bursts hanabi construit (LOT B, audit fire-path 2026-07-20 — remplace spawn/despawn par hit)"
    );
}

// ─── Spawn : flash à l'impact ───────────────────────────────────────────────

/// Lit `CombatHitEvent` (même producteur que `sys_apply_elements_on_hit`) et
/// spawn un flash coloré à `hit_world_pos`. L'explosif est plus gros (splash) et
/// 2× plus lumineux. Gaté par le MÊME check que l'application réelle
/// (`unlocks.is_unlocked`, story-589) + `vfx.enabled` — sinon flash sans dégâts.
#[allow(clippy::type_complexity)]
pub fn sys_spawn_element_impact(
    mut events: MessageReader<CombatHitEvent>,
    config: Res<ElementConfig>,
    unlocks: Res<ElementUnlocks>,
    vfx_tuning: Option<Res<VfxTuning>>,
    q_pos: Query<&GlobalTransform>,
    mut stats: ResMut<ElementVfxStats>,
    mut commands: Commands,
    // Story-655 — vrais bursts texturés (remplacent les sphères procédurales).
    bursts: Option<Res<ElementBurstAssets>>,
    // LOT B (2026-07-20) — pool persistant, plus de spawn/despawn par hit.
    pool: Option<Res<ElementBurstPool>>,
    weapon_vfx: Option<Res<WeaponVfxEffects>>,
    mut q_particle: Query<
        (
            &mut ParticleEffect,
            &mut EffectMaterial,
            &mut Transform,
            &mut Visibility,
        ),
        Without<PointLight>,
    >,
    mut q_light: Query<(
        &mut Transform,
        &mut PointLight,
        &mut Visibility,
        &mut ElementSpark,
    )>,
) {
    let (Some(bursts), Some(pool), Some(weapon_vfx)) = (bursts, pool, weapon_vfx) else {
        return;
    };
    // Story-652 Inc.2 — échelle + offset unifiés avec les VFX d'armes
    // (roguelite_vfx.toml : un seul curseur pour tout le feedback de combat).
    let tuning = vfx_tuning.as_deref().copied().unwrap_or_default();
    for ev in events.read() {
        if !config.vfx.enabled {
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
        let rgb = element.rgb(&config.vfx);
        // Offset HORS de la surface, vers le tireur (architecture standard —
        // sinon la sphère naît à moitié DANS le mesh du mob, occluse).
        let spawn_pos = match ev.attacker.and_then(|a| q_pos.get(a).ok()) {
            Some(a_gt) => {
                let toward_shooter = (a_gt.translation() - ev.hit_world_pos).normalize_or_zero();
                ev.hit_world_pos + toward_shooter * tuning.impact_offset_m
            }
            None => ev.hit_world_pos,
        };
        // LOT B — pioche le prochain slot round-robin (particule + lumière
        // PAIRÉE) et le retrigger, au lieu de spawn un burst texturé neuf.
        let idx = pool.next_index();
        retrigger_element_particle(
            &mut commands,
            &mut q_particle,
            pool.particles[idx],
            bursts.effects[element.idx()].clone(),
            element_burst_texture(&weapon_vfx, element),
            spawn_pos,
            scale,
        );
        retrigger_element_light(
            &mut q_light,
            pool.lights[idx],
            spawn_pos,
            rgb,
            light0,
            config.vfx.light_range,
            config.vfx.impact_ttl,
            scale,
        );
        stats.sparks_spawned = stats.sparks_spawned.saturating_add(1);
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
#[allow(clippy::type_complexity)]
pub fn sys_spawn_reaction_vfx(
    mut events: MessageReader<ReactionEvent>,
    config: Res<ElementConfig>,
    mut stats: ResMut<ElementVfxStats>,
    mut commands: Commands,
    // Story-655 — vrais bursts texturés (remplacent les 2 sphères de fusion).
    bursts: Option<Res<ElementBurstAssets>>,
    // LOT B (2026-07-20) — pool persistant, plus de spawn/despawn par événement.
    pool: Option<Res<ElementBurstPool>>,
    weapon_vfx: Option<Res<WeaponVfxEffects>>,
    mut q_particle: Query<
        (
            &mut ParticleEffect,
            &mut EffectMaterial,
            &mut Transform,
            &mut Visibility,
        ),
        Without<PointLight>,
    >,
    mut q_light: Query<(
        &mut Transform,
        &mut PointLight,
        &mut Visibility,
        &mut ElementSpark,
    )>,
) {
    let (Some(bursts), Some(pool), Some(weapon_vfx)) = (bursts, pool, weapon_vfx) else {
        return;
    };
    if !config.vfx.enabled {
        return;
    }
    let ttl = config.vfx.impact_ttl * 2.0;
    for ev in events.read() {
        let (primary, secondary) = ev.kind.pair();
        let rgb = primary.rgb(&config.vfx);
        let light0 = config.vfx.light_intensity * 3.0;
        // Burst texturé du 1er élément (grand) + halo du 2e (plus petit) : la
        // fusion se lit par la superposition des deux couleurs. LOT B : 2
        // slots du pool partagé sont retrigger (pas de spawn).
        let idx_primary = pool.next_index();
        retrigger_element_particle(
            &mut commands,
            &mut q_particle,
            pool.particles[idx_primary],
            bursts.effects[primary.idx()].clone(),
            element_burst_texture(&weapon_vfx, primary),
            ev.pos,
            ev.radius,
        );
        let halo = ev.radius * 0.6;
        let idx_secondary = pool.next_index();
        retrigger_element_particle(
            &mut commands,
            &mut q_particle,
            pool.particles[idx_secondary],
            bursts.effects[secondary.idx()].clone(),
            element_burst_texture(&weapon_vfx, secondary),
            ev.pos,
            halo,
        );
        // Lumière de fusion : réutilise le slot PAIRÉ de la particule primaire
        // (pas de pool lumière dédié pour les réactions — throttlées en amont
        // par le cooldown/ciblage de `sys_apply_elements_on_hit`).
        retrigger_element_light(
            &mut q_light,
            pool.lights[idx_primary],
            ev.pos,
            rgb,
            light0,
            ev.radius * 2.0,
            ttl,
            ev.radius,
        );
        stats.reaction_bursts = stats.reaction_bursts.saturating_add(1);
    }
}

// ─── Flammes de bouche de Bourrasque (élément Feu) ──────────────────────────

// ─── Tick : fade + despawn ──────────────────────────────────────────────────

/// LOT B (audit fire-path 2026-07-20) : ces lumières appartiennent TOUJOURS à
/// un pool persistant (`ElementBurstPool`) — plus de despawn ici, seulement un
/// decay vers 0. L'entité attend la prochaine réutilisation par le round-robin
/// (`retrigger_element_light` remet `age=0` à ce moment-là).
pub fn sys_tick_element_sparks(
    time: Res<Time>,
    mut q: Query<(&mut Transform, &mut ElementSpark, Option<&mut PointLight>)>,
) {
    let dt = time.delta_secs();
    for (mut tf, mut spark, light) in &mut q {
        if spark.age >= spark.ttl {
            continue; // déjà retombé à 0 — pas de despawn, pool persistant
        }
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
    }
}

// ─── Sensor forgia2_element_vfx.json ────────────────────────────────────────

pub fn sys_write_element_vfx_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    config: Res<ElementConfig>,
    stats: Res<ElementVfxStats>,
    q_sparks: Query<&ElementSpark>,
) {
    *accum += time.delta_secs();
    if *accum < POLL_PERIOD_SEC {
        return;
    }
    *accum = 0.0;

    // LOT B — le pool est persistant (toujours ELEMENT_BURST_POOL_SIZE
    // entités) : "active" = celles encore dans leur fenêtre de decay, pas le
    // total du pool.
    let active = q_sparks.iter().filter(|s| s.age < s.ttl).count();
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
        v.fire_rgb[0],
        v.fire_rgb[1],
        v.fire_rgb[2],
        v.poison_rgb[0],
        v.poison_rgb[1],
        v.poison_rgb[2],
        v.shock_rgb[0],
        v.shock_rgb[1],
        v.shock_rgb[2],
        v.armor_pierce_rgb[0],
        v.armor_pierce_rgb[1],
        v.armor_pierce_rgb[2],
    );

    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
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
