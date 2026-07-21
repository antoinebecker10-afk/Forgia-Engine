//! forgia-damage-numbers — Floating damage numbers, 3D world-space billboards.
//!
//! Story-457 (2026-05-19) — refacto : lit `CombatHitEvent` (au lieu de
//! `DamageEvent`) car c'est l'event source-of-truth pour le fire path FPS V2.
//! Couleur + taille pilotées par `HitFeedbackTuning` (genome hot-reload) selon
//! `body_zone`. Position depuis `hit_world_pos` (vs `target.translation + Y`).
//!
//! Audit fire-path 2026-07-20 (suspect n°6) — **pool round-robin de
//! [`DAMAGE_NUMBER_POOL_SIZE`] Text2d persistants** : remplace le spawn/despawn
//! d'une entité par hit (archetype churn + layout glyphes sur entité fraîche à
//! 11 hits/s Bourrasque). Le texte est réécrit EN PLACE (`fmt::Write` dans la
//! String existante → 0 alloc/hit) ; un slot expiré est caché, jamais despawné.

use std::fmt::Write as _;

use bevy::prelude::*;
use forgia_combat::prelude::CombatHitEvent;
use forgia_damage::{HitFeedback, HitZone};

/// Taille du pool (borne de dimensionnement interne, pas une valeur gameplay —
/// no-hardcode exempt) : ~11 hits/s Bourrasque × TTL 1 s + marge. Saturation →
/// le slot le plus ancien est recyclé (round-robin), pas de chiffre perdu
/// gameplay (le HUD/killfeed portent l'info).
const DAMAGE_NUMBER_POOL_SIZE: usize = 16;

#[derive(Component)]
pub struct FloatingNumber {
    pub ttl: f32,
    pub initial_ttl: f32,
    pub vel_y: f32,
}

/// Pool des Text2d de dégâts — entités persistantes cachées entre deux hits.
#[derive(Resource)]
pub struct DamageNumberPool {
    slots: [Entity; DAMAGE_NUMBER_POOL_SIZE],
    cursor: usize,
}

pub struct ForgiaDamageNumbersPlugin;

impl Plugin for ForgiaDamageNumbersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_damage_number_pool);
        app.add_systems(Update, (spawn_on_hit, tick_numbers));
    }
}

/// Startup — pré-spawn les slots cachés (`ttl <= 0` = libre, `tick_numbers`
/// les ignore). Le layout glyphes ne coûte rien tant que la String est vide.
fn setup_damage_number_pool(mut commands: Commands) {
    let slots: [Entity; DAMAGE_NUMBER_POOL_SIZE] = std::array::from_fn(|_| {
        commands
            .spawn((
                Name::new("DamageNumberSlot"),
                FloatingNumber {
                    ttl: 0.0,
                    initial_ttl: 1.0,
                    vel_y: 0.0,
                },
                Text2d::new(String::new()),
                TextFont::default(),
                TextColor(Color::NONE),
                Transform::default(),
                GlobalTransform::default(),
                Visibility::Hidden,
            ))
            .id()
    });
    commands.insert_resource(DamageNumberPool { slots, cursor: 0 });
}

#[allow(clippy::type_complexity)]
fn spawn_on_hit(
    mut events: MessageReader<CombatHitEvent>,
    pool: Option<ResMut<DamageNumberPool>>,
    feedback: Res<HitFeedback>,
    mut q_slots: Query<(
        &mut FloatingNumber,
        &mut Text2d,
        &mut TextFont,
        &mut TextColor,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    let Some(mut pool) = pool else {
        return;
    };
    for ev in events.read() {
        let rgb = feedback.0.color(ev.body_zone);
        let font_size = feedback.0.font_size(ev.body_zone);
        let color = Color::linear_rgb(rgb[0], rgb[1], rgb[2]);

        // Headshot pop : monte plus vite (lecture "important").
        let vel_y = match ev.body_zone {
            HitZone::Head => 2.4,
            HitZone::Body => 1.6,
            HitZone::Limb => 1.2,
        };

        // Round-robin : le slot le plus ancien est recyclé même s'il est
        // encore visible (16 >> ~11 vivants au pire à cadence Bourrasque).
        let slot = pool.slots[pool.cursor];
        pool.cursor = (pool.cursor + 1) % DAMAGE_NUMBER_POOL_SIZE;
        let Ok((mut fnum, mut text, mut font, mut tcolor, mut tf, mut vis)) =
            q_slots.get_mut(slot)
        else {
            continue;
        };
        *fnum = FloatingNumber {
            ttl: 1.0,
            initial_ttl: 1.0,
            vel_y,
        };
        // Réécriture EN PLACE de la String (0 alloc/hit une fois la capacité
        // atteinte) — la mutation de Text2d déclenche le relayout glyphes.
        text.0.clear();
        let _ = write!(text.0, "{:.0}", ev.damage);
        font.font_size = font_size;
        tcolor.0 = color;
        tf.translation = ev.hit_world_pos + Vec3::Y * 0.1;
        *vis = Visibility::Visible;
    }
}

fn tick_numbers(
    time: Res<Time>,
    mut q: Query<(
        &mut FloatingNumber,
        &mut Transform,
        &mut TextColor,
        &mut Visibility,
    )>,
) {
    let dt = time.delta_secs();
    for (mut fn_, mut xf, mut color, mut vis) in &mut q {
        if fn_.ttl <= 0.0 {
            continue; // slot libre du pool — rien à ticker
        }
        fn_.ttl -= dt;
        xf.translation.y += fn_.vel_y * dt;
        fn_.vel_y = (fn_.vel_y - 2.0 * dt).max(0.2);
        let alpha = (fn_.ttl / fn_.initial_ttl).clamp(0.0, 1.0);
        let c = color.0.to_linear();
        color.0 = Color::linear_rgba(c.red, c.green, c.blue, alpha);
        if fn_.ttl <= 0.0 {
            // Retour au pool : caché, jamais despawné (audit fire-path 2026-07-20).
            *vis = Visibility::Hidden;
        }
    }
}
