//! # forgia-water
//!
//! Thin wrapper autour de `bevy_water::WaterPlugin`. Maintient une nappe d'eau
//! au niveau de la mer, visible uniquement en `GameMode::Rpg` (cachée en Menu /
//! Arena pour ne pas noyer les scènes non concernées — trap V1).
//!
//! W1 vertical slice : pas de swim gate côté player (V1 lit `MapGenConfig.sea_level`
//! pour `is_swimming` ; à porter quand forgia-player aura un controller adapté).

use bevy::prelude::*;
use bevy_water::{WaterPlugin, WaterSettings, WaterTiles};
use forgia_core::prelude::*;

pub mod prelude {
    pub use crate::{ForgiaWaterPlugin, SeaLevel};
}

/// Hauteur monde de la nappe d'eau cross-crate.
///
/// Source of truth depuis 2026-05-28 (fix story-450 wave 2 TODO) : si insérée
/// par un mode (forgia-rpg `OnEnter(Rpg)`), elle remplace le fallback
/// `DEFAULT_SEA_LEVEL`. Permet à un mode RPG de varier `sea_level` sans
/// re-hardcoder dans forgia-water.
///
/// Usage : `app.insert_resource(SeaLevel(4.0))` dans le plugin du mode.
#[derive(Resource, Debug, Clone, Copy)]
pub struct SeaLevel(pub f32);

impl Default for SeaLevel {
    fn default() -> Self {
        Self(DEFAULT_SEA_LEVEL)
    }
}

/// Fallback si aucune `SeaLevel` Resource insérée (mode Menu/Arena).
const DEFAULT_SEA_LEVEL: f32 = 4.0;

const SENSOR_PATH: &str = "forgia_water.json";

#[derive(Resource, Default)]
struct WaterSensorState {
    last_write_secs: f32,
}

pub struct ForgiaWaterPlugin;

impl Plugin for ForgiaWaterPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WaterSettings {
            height: DEFAULT_SEA_LEVEL,
            ..default()
        })
        .init_resource::<WaterSensorState>()
        .add_plugins(WaterPlugin)
        .add_systems(Update, hide_water_on_spawn)
        .add_systems(Update, sync_sea_level_resource)
        .add_systems(Update, sys_write_water_sensor.in_set(GameSet::Sensors))
        .add_systems(OnEnter(GameMode::Rpg), show_water)
        .add_systems(OnExit(GameMode::Rpg), hide_water);
    }
}

/// Sync `WaterSettings.height` ← `SeaLevel` Resource si Changed/Added.
/// Permet aux modes (RPG) d'inserer `SeaLevel(N)` au OnEnter et que la nappe
/// reflète immédiatement. Pas de hot path : déclenche uniquement sur Changed.
fn sync_sea_level_resource(
    sea_level: Option<Res<SeaLevel>>,
    mut settings: ResMut<WaterSettings>,
) {
    let Some(sea_level) = sea_level else {
        return;
    };
    if !sea_level.is_changed() {
        return;
    }
    if (settings.height - sea_level.0).abs() > 1e-3 {
        settings.height = sea_level.0;
    }
}

/// Story-552 — comble observability-required violation (V1 25 genes water_advanced,
/// V2 hardcode DEFAULT_SEA_LEVEL=4.0, 0 sensor producteur jusqu'ici).
///
/// Schema T1 normalisé `{id, severity, next_step, ...}`. Severity `warn` si :
/// - RPG mode + tile_count=0 → WaterPlugin pas initialisé
/// - RPG mode + visible_count=0 → tiles cachés alors qu'on devrait nager
fn sys_write_water_sensor(
    time: Res<Time>,
    settings: Res<WaterSettings>,
    state: Res<State<GameMode>>,
    q_tiles: Query<&Visibility, With<WaterTiles>>,
    mut sensor_state: ResMut<WaterSensorState>,
) {
    let now = time.elapsed_secs();
    if now - sensor_state.last_write_secs < 1.0 {
        return;
    }
    sensor_state.last_write_secs = now;

    let tile_count = q_tiles.iter().count() as u32;
    let visible_count = q_tiles
        .iter()
        .filter(|v| matches!(**v, Visibility::Visible | Visibility::Inherited))
        .count() as u32;
    let game_mode = format!("{:?}", state.get());
    let is_rpg = *state.get() == GameMode::Rpg;

    let (severity, next_step) = if is_rpg && tile_count == 0 {
        (
            "warn",
            "WATER ZERO in RPG mode — WaterPlugin pas initialisé, swim impossible",
        )
    } else if is_rpg && visible_count == 0 {
        (
            "warn",
            "Water tiles hidden in RPG mode — show_water OnEnter(Rpg) failed",
        )
    } else if !is_rpg && visible_count > 0 {
        (
            "warn",
            "Water visible hors RPG — hide_water_on_spawn raté, scène noyée",
        )
    } else {
        ("ok", "")
    };

    let json = format!(
        r#"{{"id":"water","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"sea_level":{:.3},"tile_count":{tile_count},"visible_count":{visible_count},"game_mode":"{game_mode}"}}"#,
        now, settings.height,
    );

    if let Err(e) = std::fs::write(SENSOR_PATH, &json) {
        warn!("[forgia-water] sensor write failed: {e}");
    }
}

fn show_water(mut q: Query<&mut Visibility, With<WaterTiles>>) {
    for mut v in &mut q {
        *v = Visibility::Visible;
    }
}

fn hide_water(mut q: Query<&mut Visibility, With<WaterTiles>>) {
    for mut v in &mut q {
        *v = Visibility::Hidden;
    }
}

/// Anti-trap : `bevy_water::WaterTiles` spawn avec Visibility default = visible.
/// Si on entre directement Arena (sans passer par OnEnter/Exit Rpg), l'eau couvre
/// la scène → "mer au-dessus du sol". On force Hidden dès qu'un tile apparaît,
/// le show_water OnEnter(Rpg) le réactivera uniquement en mode RPG.
fn hide_water_on_spawn(
    state: Res<State<GameMode>>,
    mut q: Query<&mut Visibility, Added<WaterTiles>>,
) {
    if *state.get() == GameMode::Rpg {
        return;
    }
    for mut v in &mut q {
        *v = Visibility::Hidden;
    }
}
