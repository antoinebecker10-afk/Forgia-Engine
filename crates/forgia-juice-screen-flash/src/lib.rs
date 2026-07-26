//! # forgia-juice-screen-flash
//!
//! Story-455 Phase F (2026-05-18) — flash écran sur dmg / heal / kill.
//!
//! Anciennement le red-vignette low-HP était dupliqué inline dans
//! `forgia-ui-hud/player_hp.rs` (violation fine-grained-crates).
//! Migré ici comme stack de `FlashLayer` avec triggers data-driven :
//! - `OnDamage` : pulse rouge ~150ms à chaque `CombatHitEvent::target == player`
//! - `OnLowHp` : vignette rouge sustained quand player HP < threshold (genome-driven)
//! - `OnHeal` : pulse vert ~200ms (futur, hors scope V2)
//! - `OnKill` : pulse blanc bref ~80ms quand player kill un ennemi (juice OW2)
//!
//! Genome `assets/genomes/screen_flash_tuning.toml`. Sensor `forgia_screen_flash.json`.
//! Egui Foreground layer rect_filled fullscreen avec alpha lerp.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_combat::prelude::*;
use forgia_core::prelude::*;
use forgia_damage::DamageEvent;
use forgia_genome_core::{Genome, GenomeLoader};
use forgia_player::Player;
use serde::Deserialize;

pub mod prelude {
    pub use crate::{ForgiaJuiceScreenFlashPlugin, ScreenFlashTuning};
}

// ─── Tuning genome ─────────────────────────────────────────────────────

#[derive(Resource, Deserialize, Clone, Debug, bevy::reflect::TypePath)]
pub struct ScreenFlashTuning {
    // ─── On-damage flash (red pulse à chaque hit) ─────────────────
    pub damage_flash_duration_secs: f32,
    pub damage_flash_alpha_max: f32,
    pub damage_color_r: u8,
    pub damage_color_g: u8,
    pub damage_color_b: u8,

    // ─── Sustained low-HP vignette ────────────────────────────────
    /// Fraction HP en dessous de laquelle la vignette devient visible.
    pub low_hp_threshold: f32,
    /// Alpha max au HP = 0 (intensité full).
    pub low_hp_alpha_max: f32,

    // ─── On-kill flash (white brief pulse) ───────────────────────
    pub kill_flash_duration_secs: f32,
    pub kill_flash_alpha_max: f32,

    /// Cap simultané des layers actifs (anti-spam burst de tirs / lag spike events).
    /// Si dépassé, on drop le plus ancien (front).
    pub max_flash_layers: usize,

    pub sensor_period_secs: f32,
}

impl Default for ScreenFlashTuning {
    fn default() -> Self {
        Self {
            damage_flash_duration_secs: 0.18,
            damage_flash_alpha_max: 0.55,
            damage_color_r: 200,
            damage_color_g: 40,
            damage_color_b: 40,

            low_hp_threshold: 0.30,
            low_hp_alpha_max: 0.45,

            kill_flash_duration_secs: 0.08,
            kill_flash_alpha_max: 0.20,

            max_flash_layers: 8,

            sensor_period_secs: 1.0,
        }
    }
}

#[derive(Resource)]
pub struct ScreenFlashTuningHandle(pub Handle<Genome<ScreenFlashTuning>>);

pub fn load_screen_flash_tuning(mut commands: Commands, server: Res<AssetServer>) {
    let handle: Handle<Genome<ScreenFlashTuning>> = server.load("genomes/screen_flash_tuning.toml");
    commands.insert_resource(ScreenFlashTuningHandle(handle));
}

pub fn sync_screen_flash_tuning(
    mut events: MessageReader<AssetEvent<Genome<ScreenFlashTuning>>>,
    handle: Option<Res<ScreenFlashTuningHandle>>,
    assets: Res<Assets<Genome<ScreenFlashTuning>>>,
    mut tuning: ResMut<ScreenFlashTuning>,
) {
    let Some(handle) = handle else { return };
    let mut should = false;
    for ev in events.read() {
        match ev {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id }
                if *id == handle.0.id() =>
            {
                should = true;
            }
            _ => {}
        }
    }
    if !should {
        return;
    }
    if let Some(g) = assets.get(&handle.0) {
        *tuning = g.data.clone();
        info!(
            "[screen-flash] tuning synced (dmg {:.2}s, low {:.0}%)",
            tuning.damage_flash_duration_secs,
            tuning.low_hp_threshold * 100.0
        );
    }
}

// ─── Flash state ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum FlashKind {
    Damage,
    Kill,
}

#[derive(Debug, Clone, Copy)]
pub struct FlashLayer {
    pub kind: FlashKind,
    pub age_secs: f32,
    pub duration_secs: f32,
    pub alpha_max: f32,
    pub color: [u8; 3],
}

#[derive(Resource, Default)]
pub struct ScreenFlashState {
    pub layers: Vec<FlashLayer>,
}

#[derive(Resource, Default)]
pub struct ScreenFlashSensor {
    pub last_write_secs: f32,
    pub damage_flashes_session: u32,
    pub kill_flashes_session: u32,
    pub low_hp_active: bool,
}

// ─── Systems ──────────────────────────────────────────────────────────

pub(crate) fn ingest_flash_events(
    mut hits: MessageReader<CombatHitEvent>,
    mut damage_events: MessageReader<DamageEvent>,
    mut state: ResMut<ScreenFlashState>,
    mut sensor: ResMut<ScreenFlashSensor>,
    tuning: Res<ScreenFlashTuning>,
    q_player: Query<Entity, With<Player>>,
) {
    let Ok(player_entity) = q_player.single() else {
        return;
    };
    // BUG-455-06 fix : helper push_capped — drop front si cap atteint (anti-spam).
    let cap = tuning.max_flash_layers.max(1);

    // Player reçu un coup → red pulse. SOURCE = `DamageEvent` (forgia_damage) :
    // les dégâts ennemi→joueur passent par `apply_damage`, JAMAIS par
    // `CombatHitEvent` (dual-health, cf reference_two_health_types). Fix 2026-07-20
    // — avant, ce pulse lisait `CombatHitEvent.target == player` (branche morte
    // en Roguelite ET FPS → aucun flash à la prise de dégâts).
    for dmg in damage_events.read() {
        if dmg.target == player_entity && dmg.amount > 0.0 {
            if state.layers.len() >= cap {
                state.layers.remove(0);
            }
            state.layers.push(FlashLayer {
                kind: FlashKind::Damage,
                age_secs: 0.0,
                duration_secs: tuning.damage_flash_duration_secs,
                alpha_max: tuning.damage_flash_alpha_max,
                color: [
                    tuning.damage_color_r,
                    tuning.damage_color_g,
                    tuning.damage_color_b,
                ],
            });
            sensor.damage_flashes_session = sensor.damage_flashes_session.saturating_add(1);
        }
    }

    // Player a tué un ennemi → white pulse bref (source = CombatHitEvent,
    // kill côté joueur→ennemi).
    for hit in hits.read() {
        if hit.is_kill && hit.attacker == Some(player_entity) {
            if state.layers.len() >= cap {
                state.layers.remove(0);
            }
            state.layers.push(FlashLayer {
                kind: FlashKind::Kill,
                age_secs: 0.0,
                duration_secs: tuning.kill_flash_duration_secs,
                alpha_max: tuning.kill_flash_alpha_max,
                color: [255, 255, 255],
            });
            sensor.kill_flashes_session = sensor.kill_flashes_session.saturating_add(1);
        }
    }
}

pub(crate) fn tick_flash_layers(time: Res<Time>, mut state: ResMut<ScreenFlashState>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for l in &mut state.layers {
        l.age_secs += dt;
    }
    state.layers.retain(|l| l.age_secs < l.duration_secs);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_screen_flash(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    state: Res<ScreenFlashState>,
    tuning: Res<ScreenFlashTuning>,
    q_player_health: Query<&Health, With<Player>>,
    mut sensor: ResMut<ScreenFlashSensor>,
) {
    // Fix 2026-07-20 : était gaté `GameMode::Fps` seul → flash dégâts/kill +
    // vignette low-HP INACTIFS en Roguelite (le mode shippé). Étendu aux deux.
    if *app_state.get() != AppMode::InGame
        || !matches!(*game_mode.get(), GameMode::Fps | GameMode::Roguelite)
    {
        sensor.low_hp_active = false;
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else { return };

    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("forgia_screen_flash"),
    ));

    // ─── Sustained low-HP vignette ────────────────────────────────
    if let Ok(health) = q_player_health.single() {
        let frac = if health.max > 0.0 {
            health.current / health.max
        } else {
            1.0
        };
        let low_active = frac < tuning.low_hp_threshold;
        sensor.low_hp_active = low_active;
        if low_active {
            let t = ((tuning.low_hp_threshold - frac) / tuning.low_hp_threshold).clamp(0.0, 1.0);
            let alpha = (t * tuning.low_hp_alpha_max * 255.0) as u8;
            painter.rect_filled(
                screen,
                0.0,
                egui::Color32::from_rgba_unmultiplied(180, 30, 30, alpha),
            );
        }
    } else {
        sensor.low_hp_active = false;
    }

    // ─── Transient flash layers ───────────────────────────────────
    for layer in &state.layers {
        let t = (layer.age_secs / layer.duration_secs).clamp(0.0, 1.0);
        // Ease-out (1-t)² : pic alpha au début, fade rapide.
        let curve = (1.0 - t).powi(2);
        let alpha = (curve * layer.alpha_max * 255.0) as u8;
        if alpha == 0 {
            continue;
        }
        painter.rect_filled(
            screen,
            0.0,
            egui::Color32::from_rgba_unmultiplied(
                layer.color[0],
                layer.color[1],
                layer.color[2],
                alpha,
            ),
        );
    }
}

pub(crate) fn write_screen_flash_sensor(
    time: Res<Time>,
    tuning: Res<ScreenFlashTuning>,
    state: Res<ScreenFlashState>,
    mut sensor: ResMut<ScreenFlashSensor>,
) {
    let now = time.elapsed_secs();
    if now - sensor.last_write_secs < tuning.sensor_period_secs.max(0.1) {
        return;
    }
    sensor.last_write_secs = now;
    let json = format!(
        r#"{{"timestamp_secs":{:.2},"active_layers":{},"damage_flashes_session":{},"kill_flashes_session":{},"low_hp_active":{}}}"#,
        now,
        state.layers.len(),
        sensor.damage_flashes_session,
        sensor.kill_flashes_session,
        sensor.low_hp_active,
    );
    let _ = forgia_core::sensor_io::enqueue("forgia_screen_flash.json", json);
}

// ─── Plugin ───────────────────────────────────────────────────────────

pub struct ForgiaJuiceScreenFlashPlugin;

impl Plugin for ForgiaJuiceScreenFlashPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScreenFlashState>()
            .init_resource::<ScreenFlashSensor>()
            .init_resource::<ScreenFlashTuning>()
            .init_asset::<Genome<ScreenFlashTuning>>()
            .register_asset_loader(GenomeLoader::<ScreenFlashTuning>::default())
            .add_systems(Startup, load_screen_flash_tuning)
            .add_systems(
                Update,
                (
                    sync_screen_flash_tuning,
                    ingest_flash_events,
                    tick_flash_layers,
                    write_screen_flash_sensor,
                )
                    .chain(),
            )
            .add_systems(EguiPrimaryContextPass, draw_screen_flash);
    }
}
