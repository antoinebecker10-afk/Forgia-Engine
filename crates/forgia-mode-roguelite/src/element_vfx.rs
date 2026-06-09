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
use forgia_combat::combat_juice::CombatHitEvent;
use forgia_core::prelude::*;

use crate::elements::{Element, ElementConfig, StatusBurn, StatusPoison};

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
    /// Indexé par [`Element::idx`] (Fire=0, Poison=1, Explosive=2, ArmorPierce=3).
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
    Element::Explosive,
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

// ─── Spawn : flash à l'impact ───────────────────────────────────────────────

/// Lit `CombatHitEvent` (même producteur que `sys_apply_elements_on_hit`) et
/// spawn un flash coloré à `hit_world_pos`. L'explosif est plus gros (splash) et
/// 2× plus lumineux. Gaté par `always_on` (cohérence avec l'application réelle)
/// + `vfx.enabled`.
pub fn sys_spawn_element_impact(
    mut events: MessageReader<CombatHitEvent>,
    config: Res<ElementConfig>,
    assets: Option<Res<ElementVfxAssets>>,
    q_sparks: Query<(), With<ElementSpark>>,
    mut stats: ResMut<ElementVfxStats>,
    mut commands: Commands,
) {
    let Some(assets) = assets else {
        return;
    };
    let mut live = q_sparks.iter().count();
    for ev in events.read() {
        if !config.always_on || !config.vfx.enabled || live >= MAX_ACTIVE_SPARKS {
            continue;
        }
        let Some(weapon) = ev.weapon else {
            continue;
        };
        let Some(element) = config.element_for(weapon) else {
            continue;
        };
        let explosive = element == Element::Explosive;
        let scale = config.vfx.impact_scale * if explosive { config.vfx.explosive_scale } else { 1.0 };
        let light0 = config.vfx.light_intensity * if explosive { 2.0 } else { 1.0 };
        let [r, g, b] = element.rgb(&config.vfx);
        commands.spawn((
            Mesh3d(assets.sphere.clone()),
            MeshMaterial3d(assets.mats[element.idx()].clone()),
            Transform::from_translation(ev.hit_world_pos).with_scale(Vec3::splat(scale)),
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

// ─── Pulse sur les ennemis en DoT ───────────────────────────────────────────

/// Tous les `dot_pulse_period`, spawn un petit pulse coloré (SANS lumière) sur
/// chaque ennemi en feu (orange) / empoisonné (vert) → l'état est lisible.
pub fn sys_dot_pulse_vfx(
    time: Res<Time>,
    mut accum: Local<f32>,
    config: Res<ElementConfig>,
    assets: Option<Res<ElementVfxAssets>>,
    mut stats: ResMut<ElementVfxStats>,
    q_burn: Query<&GlobalTransform, With<StatusBurn>>,
    q_poison: Query<&GlobalTransform, With<StatusPoison>>,
    mut commands: Commands,
) {
    if !config.vfx.enabled {
        return;
    }
    let Some(assets) = assets else {
        return;
    };
    *accum += time.delta_secs();
    if *accum < config.vfx.dot_pulse_period {
        return;
    }
    *accum = 0.0;

    let s = config.vfx.dot_pulse_scale;
    let ttl = config.vfx.impact_ttl;
    let spawn_pulse = |pos: Vec3, element: Element, commands: &mut Commands| {
        commands.spawn((
            Mesh3d(assets.sphere.clone()),
            MeshMaterial3d(assets.mats[element.idx()].clone()),
            Transform::from_translation(pos + Vec3::Y).with_scale(Vec3::splat(s)),
            ElementSpark { age: 0.0, ttl, start_scale: s, light0: 0.0 },
            DespawnOnExit(GameMode::Roguelite),
        ));
    };
    for gt in &q_burn {
        spawn_pulse(gt.translation(), Element::Fire, &mut commands);
        stats.dot_pulses = stats.dot_pulses.saturating_add(1);
    }
    for gt in &q_poison {
        spawn_pulse(gt.translation(), Element::Poison, &mut commands);
        stats.dot_pulses = stats.dot_pulses.saturating_add(1);
    }
}

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
        r#"{{"id":"element_vfx","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"enabled":{},"sparks_spawned":{},"dot_pulses":{},"active_sparks":{active},"colors":{{"fire":[{:.2},{:.2},{:.2}],"poison":[{:.2},{:.2},{:.2}],"explosive":[{:.2},{:.2},{:.2}],"armor_pierce":[{:.2},{:.2},{:.2}]}}}}"#,
        time.elapsed_secs(),
        v.enabled,
        stats.sparks_spawned,
        stats.dot_pulses,
        v.fire_rgb[0], v.fire_rgb[1], v.fire_rgb[2],
        v.poison_rgb[0], v.poison_rgb[1], v.poison_rgb[2],
        v.explosive_rgb[0], v.explosive_rgb[1], v.explosive_rgb[2],
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
