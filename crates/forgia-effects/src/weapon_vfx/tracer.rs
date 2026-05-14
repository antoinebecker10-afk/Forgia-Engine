//! Tracer VFX — dual-layer bullet tracer (core + glow envelope).
//!
//! Hitscan weapons: stretched billboard mesh (CoD/Battlefield pattern).
//!
//! - Core beam: thin, bright HDR, additive blend
//! - Glow envelope: wider, softer, additive, lower opacity
//!
//! Both fade via EmissiveFade over wfx_tracer_fade seconds.
//!
//! ## Perf (story-432 V1 — gunfeel anti-régression)
//!
//! Mesh + materials sont pré-construits au Startup via `setup_tracer_resources`
//! et stockés dans `TracerResources` (Resource). Le hot path `spawn_hitscan_tracer`
//! ne fait que `Handle::clone` (Arc atomic increment, ~ns) — zéro `Assets::add()`
//! par tir. Avant le fix : 4 assets/tir × 15 tirs/s = 60 assets/s en auto-fire →
//! freeze 82ms observé dans `forgia_lag_events.json` (corrélation
//! `mouse_pressed → damage_player → spike`).
//!
//! Limitation V1 : hot-reload de `wfx_tracer_width` ne refresh PAS les meshes
//! (lus une fois au Startup). Restart requis. Valeur visuelle peu touchée en
//! production — acceptable phase V1.

#![allow(dead_code, unused_imports)]
// Port verbatim de `forgia-game/src/effects/weapon_vfx/tracer.rs` (V1).

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

// TODO: port from V1 — components::EmissiveFade
// use forgia_combat::components::EmissiveFade;

// TODO: port from V1 — combat::weapons::WeaponType → now in forgia-combat
use forgia_combat::weapons::WeaponType;

// TODO: port from V1 — resources::FpsTuning → now in forgia-core
// use forgia_core::resources::FpsTuning;

/// EmissiveFade — ported inline until forgia-combat exports it.
/// Fades emissive HDR material to zero over timer duration.
#[derive(Component)]
pub struct EmissiveFade {
    pub timer: Timer,
    pub initial: LinearRgba,
}

/// Pair core+glow handles for a tracer color preset.
#[derive(Clone)]
pub struct TracerMatPair {
    pub core: Handle<StandardMaterial>,
    pub glow: Handle<StandardMaterial>,
    pub core_color: LinearRgba,
    pub glow_color: LinearRgba,
}

/// Pre-built tracer mesh + per-weapon material handles.
/// Init au Startup, lu en hot path (Handle::clone = atomic Arc inc).
///
/// Pattern aligné sur `CasingResources` (audit-2026-05-02 #43).
#[derive(Resource)]
pub struct TracerResources {
    pub core_mesh: Handle<Mesh>,
    pub glow_mesh: Handle<Mesh>,
    /// Lookup par WeaponType. Chainsaw (mêlée) → absent.
    pub pairs: HashMap<WeaponType, TracerMatPair>,
}

impl TracerResources {
    /// Retourne la paire mat pour cette arme, None si mêlée / absent.
    pub fn pair_for(&self, weapon: &WeaponType) -> Option<&TracerMatPair> {
        self.pairs.get(weapon)
    }
}

/// Weapon-specific tracer color presets (HDR emissive).
/// Conservé public pour init `TracerResources` + tests.
pub fn tracer_color(weapon: &WeaponType) -> (LinearRgba, LinearRgba) {
    match weapon {
        // Hitscan — warm orange-yellow
        WeaponType::ModernAR | WeaponType::AssaultRifle | WeaponType::AK47 =>
            (LinearRgba::new(10.0, 7.0, 2.5, 1.0), LinearRgba::new(4.0, 2.5, 0.8, 0.4)),
        WeaponType::Shotgun =>
            (LinearRgba::new(12.0, 8.0, 2.0, 1.0), LinearRgba::new(5.0, 3.0, 0.6, 0.35)),
        // Plasma — blue-cyan
        WeaponType::PlasmaRifle =>
            (LinearRgba::new(2.0, 6.0, 12.0, 1.0), LinearRgba::new(0.8, 2.5, 5.0, 0.4)),
        // Rocket — orange-red
        WeaponType::RocketLauncher =>
            (LinearRgba::new(12.0, 4.0, 1.0, 1.0), LinearRgba::new(5.0, 1.5, 0.3, 0.35)),
        // Melee — no tracer
        WeaponType::Chainsaw =>
            (LinearRgba::new(0.0, 0.0, 0.0, 0.0), LinearRgba::new(0.0, 0.0, 0.0, 0.0)),
    }
}

/// Startup : pré-construit les 2 meshes + 4 paires de materials.
///
/// Lit `wfx_tracer_width` depuis `FpsTuning` une seule fois (cf module doc
/// pour limitation hot-reload).
pub fn setup_tracer_resources(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    // TODO: replace with forgia_core::resources::FpsTuning when ported
    // tuning: Res<FpsTuning>,
) {
    // TODO: read wfx_tracer_width from FpsTuning; using placeholder 0.004 until ported
    let core_width = 0.004_f32; // placeholder — V1 read from tuning.wfx_tracer_width
    let glow_width = core_width * 3.5;

    let core_mesh = meshes.add(Cuboid::new(core_width, core_width, 1.0));
    let glow_mesh = meshes.add(Cuboid::new(glow_width, glow_width, 1.0));

    // 4 presets non-mêlée. Chainsaw exclu (is_melee returns true).
    let weapons_with_tracer = [
        WeaponType::ModernAR,
        WeaponType::AssaultRifle,
        WeaponType::AK47,
        WeaponType::Shotgun,
        WeaponType::PlasmaRifle,
        WeaponType::RocketLauncher,
    ];

    let mut pairs: HashMap<WeaponType, TracerMatPair> = HashMap::default();
    for w in weapons_with_tracer {
        let (core_color, glow_color) = tracer_color(&w);
        let core = materials.add(StandardMaterial {
            emissive: core_color,
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        });
        let glow = materials.add(StandardMaterial {
            emissive: glow_color,
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        });
        pairs.insert(w, TracerMatPair { core, glow, core_color, glow_color });
    }

    commands.insert_resource(TracerResources {
        core_mesh,
        glow_mesh,
        pairs,
    });
}

/// Spawn a dual-layer hitscan tracer between barrel_tip and hit point.
///
/// HOT PATH : zero `Assets::add()`. Tous les handles viennent de
/// `TracerResources` (pré-construit Startup). Coût = 2× spawn entity + Handle
/// clone (Arc inc).
pub fn spawn_hitscan_tracer(
    commands: &mut Commands,
    tracer_res: &TracerResources,
    barrel_tip: Vec3,
    shot_dir: Vec3,
    hit_dist: f32,
    weapon: &WeaponType,
    // TODO: replace with &FpsTuning when forgia-core ported
    tracer_max_length: f32, // V1: tuning.wfx_tracer_max_length
    tracer_fade: f32,       // V1: tuning.wfx_tracer_fade
) {
    if weapon.is_melee() { return; }
    let Some(pair) = tracer_res.pair_for(weapon) else { return };

    let tracer_seg_len = tracer_max_length.min(hit_dist * 0.3);
    let tracer_start_offset = hit_dist * 0.35;
    let tracer_mid = barrel_tip + shot_dir * (tracer_start_offset + tracer_seg_len * 0.5);

    let base_tf = Transform::from_translation(tracer_mid)
        .looking_to(shot_dir, Vec3::Y)
        .with_scale(Vec3::new(1.0, 1.0, tracer_seg_len));

    // Layer 1: Core beam
    commands.spawn((
        Mesh3d(tracer_res.core_mesh.clone()),
        MeshMaterial3d(pair.core.clone()),
        base_tf,
        EmissiveFade {
            timer: Timer::from_seconds(tracer_fade, TimerMode::Once),
            initial: pair.core_color,
        },
    ));

    // Layer 2: Glow envelope
    commands.spawn((
        Mesh3d(tracer_res.glow_mesh.clone()),
        MeshMaterial3d(pair.glow.clone()),
        base_tf,
        EmissiveFade {
            timer: Timer::from_seconds(tracer_fade * 1.3, TimerMode::Once),
            initial: pair.glow_color,
        },
    ));
}
