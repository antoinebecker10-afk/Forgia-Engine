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
///
/// Story-451 (2026-05-18) — palette refresh AAA :
/// - Core blanc-chaud très bright (15-20 HDR) → "tracer head" lisible
/// - Glow color per-arme modeste (2-4 HDR) → trail subtil sans bloom blowout
/// - Pépin jaune, Bourrasque orange FEU (élément), plasma cyan electric ;
///   rocket (Boucherie) unused car projectile (visuel vert poison côté
///   `forgia-mode-roguelite/boucherie_rocket.rs`).
pub fn tracer_color(weapon: &WeaponType) -> (LinearRgba, LinearRgba) {
    match weapon {
        // Pépin (ModernAR) / AK47 — yellow-white head, warm orange trail
        WeaponType::ModernAR | WeaponType::AK47 => (
            LinearRgba::new(15.0, 12.0, 5.0, 1.0),
            LinearRgba::new(3.0, 1.8, 0.5, 0.55),
        ),
        // Bourrasque (AssaultRifle) = élément FEU → orange chaud saturé, distinct
        // du jaune de Pépin (story-611 follow-up : tracer aligné sur l'élément).
        WeaponType::AssaultRifle => (
            LinearRgba::new(16.0, 7.0, 1.5, 1.0),
            LinearRgba::new(4.0, 1.4, 0.3, 0.55),
        ),
        // Shotgun (Boucherie) — warm orange pellet trail
        WeaponType::Shotgun => (
            LinearRgba::new(16.0, 10.0, 3.0, 1.0),
            LinearRgba::new(3.5, 1.4, 0.3, 0.50),
        ),
        // Plasma — electric cyan-blue
        WeaponType::PlasmaRifle => (
            LinearRgba::new(5.0, 13.0, 18.0, 1.0),
            LinearRgba::new(0.8, 2.2, 4.5, 0.55),
        ),
        // Rocket — red-yellow (legacy, projectile remplace)
        WeaponType::RocketLauncher => (
            LinearRgba::new(18.0, 6.0, 1.5, 1.0),
            LinearRgba::new(4.0, 1.2, 0.2, 0.50),
        ),
        WeaponType::Chainsaw => (
            LinearRgba::new(0.0, 0.0, 0.0, 0.0),
            LinearRgba::new(0.0, 0.0, 0.0, 0.0),
        ),
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
    // TODO: read wfx_tracer_width from FpsTuning; using placeholder x10 V1 default
    // V1 default 0.004m = 4mm = invisible >5m distance. V2 placeholder = 5cm core / 17cm glow
    // jusqu'au portage FpsTuning (genome combat.toml wfx_tracer_width).
    let core_width = 0.05_f32; // 5cm
    let glow_width = core_width * 3.5; // 17.5cm

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
        pairs.insert(
            w,
            TracerMatPair {
                core,
                glow,
                core_color,
                glow_color,
            },
        );
    }

    commands.insert_resource(TracerResources {
        core_mesh,
        glow_mesh,
        pairs,
    });
}

/// Component : balle visuelle en vol (story-451, 2026-05-18).
///
/// Pattern AAA (CS2/Valorant) — damage hitscan calculé instantanément côté
/// gameplay, projectile visuel volant en parallèle pour feedback. Despawn
/// quand `distance_traveled >= max_distance` ou lifetime expiré (sécurité).
#[derive(Component)]
pub struct BulletInFlight {
    pub velocity: Vec3,
    pub max_distance: f32,
    pub traveled: f32,
    pub life: Timer,
}

/// Spawn une balle qui vole du canon vers la cible (ou max range si miss).
///
/// Speed = max(120 m/s, max_distance / 0.30) — garantit travel ≤ 300ms même
/// pour sniper longue distance (anti "tracer qui rame").
pub fn spawn_hitscan_tracer(
    commands: &mut Commands,
    tracer_res: &TracerResources,
    barrel_tip: Vec3,
    shot_dir: Vec3,
    hit_dist: f32,
    weapon: &WeaponType,
    _tracer_max_length: f32, // legacy — gardé pour API stable
    tracer_fade: f32,
) {
    if weapon.is_melee() {
        return;
    }
    let Some(pair) = tracer_res.pair_for(weapon) else {
        return;
    };

    // Speed adapté à la portée pour rester sous 300ms travel.
    let speed = (hit_dist / 0.30).max(120.0);
    let velocity = shot_dir.normalize_or_zero() * speed;

    // Bullet length : ~25cm pour SMG/AR, plus court pour shotgun (pellet),
    // plus long pour plasma (signature gameplay).
    let bullet_length = match weapon {
        WeaponType::PlasmaRifle => 0.45,
        WeaponType::Shotgun => 0.15,
        _ => 0.25,
    };

    let base_tf = Transform::from_translation(barrel_tip)
        .looking_to(shot_dir, Vec3::Y)
        .with_scale(Vec3::new(1.0, 1.0, bullet_length));

    // Sécurité : lifetime hard cap pour éviter fuite si max_distance énorme.
    let life_secs = (hit_dist / speed).clamp(0.05, 2.0);

    // Layer 1: Core projectile (très bright, fine).
    commands.spawn((
        Mesh3d(tracer_res.core_mesh.clone()),
        MeshMaterial3d(pair.core.clone()),
        base_tf,
        bevy::light::NotShadowCaster,
        BulletInFlight {
            velocity,
            max_distance: hit_dist,
            traveled: 0.0,
            life: Timer::from_seconds(life_secs + 0.1, TimerMode::Once),
        },
        EmissiveFade {
            timer: Timer::from_seconds(tracer_fade.max(0.3), TimerMode::Once),
            initial: pair.core_color,
        },
    ));

    // Layer 2: Glow envelope plus court (trail subtil derrière la tête).
    let glow_tf = Transform::from_translation(barrel_tip)
        .looking_to(shot_dir, Vec3::Y)
        .with_scale(Vec3::new(1.0, 1.0, bullet_length * 0.7));
    commands.spawn((
        Mesh3d(tracer_res.glow_mesh.clone()),
        MeshMaterial3d(pair.glow.clone()),
        glow_tf,
        bevy::light::NotShadowCaster,
        BulletInFlight {
            velocity,
            max_distance: hit_dist,
            traveled: 0.0,
            life: Timer::from_seconds(life_secs + 0.1, TimerMode::Once),
        },
        EmissiveFade {
            timer: Timer::from_seconds(tracer_fade.max(0.3) * 1.2, TimerMode::Once),
            initial: pair.glow_color,
        },
    ));
}

/// System Update : avance les balles en vol, despawn à l'impact ou expiration.
pub fn tick_bullets_in_flight(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut BulletInFlight)>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut bullet) in &mut q {
        let step = bullet.velocity * dt;
        let step_len = step.length();
        tf.translation += step;
        bullet.traveled += step_len;
        bullet.life.tick(time.delta());

        if bullet.traveled >= bullet.max_distance || bullet.life.is_finished() {
            if let Ok(mut ec) = commands.get_entity(e) {
                ec.try_despawn();
            }
        }
    }
}
