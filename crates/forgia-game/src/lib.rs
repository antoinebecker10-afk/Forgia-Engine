//! # forgia-game
//!
//! Forgia V2 — Assembly library. Exposes `run_game()` that wires all plugins
//! (DefaultPlugins, Rapier, Hanabi, Forgia core + gameplay + mode plugins) and
//! runs the App.
//!
//! Two entry points :
//! - Root binary `src/main.rs` at workspace root (Renzora-style) — preferred.
//! - Local binary `crates/forgia-game/src/main.rs` — kept for `cargo run -p forgia-game`.
//!
//! Both call `forgia_game::run_game()`.

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_core::prelude::*;

/// Build the App with all Forgia plugins wired, then run it. Returns `AppExit`.
pub fn run_game() -> AppExit {
    let mut app = App::new();

    // 1. Bevy DefaultPlugins EN PREMIER (fournit StatesPlugin requis par ForgiaCorePlugin)
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Forgia V2".to_string(),
            resolution: (1920u32, 1080u32).into(),
            ..default()
        }),
        ..default()
    }));

    // 2. Forgia Core (init_state nécessite StatesPlugin déjà chargé)
    app.add_plugins(ForgiaCorePlugin);

    // 3. Rapier physique (après DefaultPlugins)
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default());

    // 4. Hanabi VFX (requis par forgia-effects::setup_weapon_vfx)
    app.add_plugins(bevy_hanabi::HanabiPlugin);

    // 5. Forgia plugins gameplay
    app.add_plugins((
        forgia_assets::prelude::ForgiaAssetsPlugin,
        forgia_input::prelude::ForgiaInputPlugin,
        forgia_player::prelude::ForgiaPlayerPlugin,
        forgia_effects::prelude::ForgiaEffectsPlugin,
        forgia_combat::prelude::ForgiaCombatPlugin,
        forgia_ui::prelude::ForgiaUiPlugin,
        forgia_ui_lib::hud::prelude::ForgiaUiHudPlugin,
        forgia_ui_lib::hud_ammo::prelude::ForgiaUiHudAmmoPlugin,
        forgia_killfeed::prelude::ForgiaKillfeedPlugin,
        forgia_ui_lib::damage_direction::prelude::ForgiaUiDamageDirectionPlugin,
        forgia_juice_screen_flash::prelude::ForgiaJuiceScreenFlashPlugin,
        forgia_ui_lib::pause_menu::prelude::ForgiaUiPauseMenuPlugin,
        forgia_sensors::prelude::ForgiaSensorsPlugin,
        forgia_observability::prelude::ForgiaObservabilityPlugin,
    ));

    // 5b. Story-457 (2026-05-19) — damage types + nameplate (split du bloc 5
    //     pour rester sous la limite tuple Bevy 15).
    app.add_plugins((
        forgia_damage::ForgiaDamagePlugin,
        forgia_damage_numbers::ForgiaDamageNumbersPlugin,
        forgia_enemy_nameplate::prelude::ForgiaEnemyNameplatePlugin,
    ));

    // 6. Cross-mode systems (utilisés par forgia-rpg, requis init_resource avant Startup)
    app.add_plugins(forgia_dialogue::ForgiaDialoguePlugin);

    // 7. Mode-specific plugins (run_if interne par GameMode)
    app.add_plugins((
        forgia_asset_registry::prelude::ForgiaAssetRegistryPlugin,
        forgia_streaming::ForgiaStreamingPlugin, // story-450 chunk streaming foundation
        forgia_fps::prelude::ForgiaFpsPlugin,
        forgia_viewmodel_calibration::ForgiaViewmodelCalibrationPlugin,
        forgia_rpg::prelude::ForgiaRpgPlugin,
        forgia_mode_roguelite::prelude::ForgiaModeRoguelitePlugin,
        forgia_terrain::prelude::ForgiaTerrainPlugin,
        forgia_foliage::prelude::ForgiaFoliagePlugin,
        forgia_water::prelude::ForgiaWaterPlugin,
        forgia_audio_biome::prelude::ForgiaAudioBiomePlugin,
        // Story-481 Tier 1.5 — wire-up barks (selection logic Tier 1 story-472).
        // Plugin auto-charge assets/genomes/roguelite/roguelite_dialogue.toml.
        // Émetteur BarkEvent côté forgia-mode-roguelite (obs_roguelite_enemy_death).
        forgia_audio_voicelines::ForgiaAudioVoicelinesPlugin,
    ));

    // 7b. Anim Layer (story-437) + 3P camera (story-438) — utilisés par forgia-rpg
    app.add_plugins((
        forgia_anim_debug::prelude::ForgiaAnimDebugPlugin,
        forgia_camera_orbit::prelude::ForgiaCameraOrbitPlugin,
        forgia_secondary_motion::prelude::ForgiaSecondaryMotionPlugin,
    ));

    // 7c. Village data-driven (story-441) — Prefab + Village Loader.
    // 2026-05-20 fix : ForgiaPrefabPlugin peut déjà être ajouté transitivement
    // via forgia-mode-roguelite → forgia-stage-arena (story-468/483).
    // Guard idempotent pour éviter "plugin was already added" panic.
    if !app.is_plugin_added::<forgia_prefab::ForgiaPrefabPlugin>() {
        app.add_plugins(forgia_prefab::ForgiaPrefabPlugin);
    }
    app.add_plugins(forgia_village_loader::ForgiaVillageLoaderPlugin);

    // ClearColor = skybox sunset/dusk warm — ambiance forge ruines (vs bleu ciel jour)
    app.insert_resource(ClearColor(Color::srgb(0.35, 0.22, 0.18)));

    // Boot transition (mode-spec plugins gèrent leur arène/world via OnEnter(GameMode))
    app.add_systems(Startup, boot_to_menu);

    info!("[forgia-game] Forgia V2 boot OK");
    app.run()
}

fn boot_to_menu(mut next: ResMut<NextState<AppMode>>) {
    next.set(AppMode::Menu);
}
