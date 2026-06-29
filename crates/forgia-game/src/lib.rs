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

use bevy::image::{ImagePlugin, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use forgia_core::prelude::*;

/// Démo perf moteur (2026-06-15) — "Cyber City démo" : charge un GLB lourd +
/// flycam libre pour stress-tester le rendu. Entrée via le menu principal.
mod cyber_city;
/// Color grading filmique par GameMode (story-602) — mood par mode (chaud/froid/
/// saturation), hot-reload genome. Orthogonal au tonemapping (composant distinct).
mod color_grading;

/// Build the App with all Forgia plugins wired, then run it. Returns `AppExit`.
pub fn run_game() -> AppExit {
    let mut app = App::new();

    // 1. Bevy DefaultPlugins EN PREMIER (fournit StatesPlugin requis par ForgiaCorePlugin)
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Forgia V2".to_string(),
                    resolution: (1920u32, 1080u32).into(),
                    ..default()
                }),
                ..default()
            })
            // B1 (audit story-598) — filtrage anisotrope 16× sur le sampler par
            // défaut : nette les textures vues en oblique (rues/murs en
            // perspective = l'essentiel d'une vue 3P de ville). Effet maximal
            // sur textures mippées ; sans mipmaps le gain reste limité (cf B2
            // KTX2/mipmaps). Les samplers custom (terrain/foliage) ne sont pas
            // affectés (ils overrident via ImageLoaderSettings).
            .set(ImagePlugin {
                default_sampler: ImageSamplerDescriptor {
                    anisotropy_clamp: 16,
                    ..ImageSamplerDescriptor::linear()
                },
            }),
    );

    // 2. Forgia Core (init_state nécessite StatesPlugin déjà chargé)
    app.add_plugins(ForgiaCorePlugin);

    // 3. Rapier physique (après DefaultPlugins).
    // Keystone 0.1a-2 slice 3 (story-634) — physique en FixedUpdate (au lieu du
    // défaut PostUpdate) pour la sim déterministe : alignée sur la chaîne GameSet
    // FixedUpdate (0.1a-1) et sur le mouvement joueur. `TimestepMode::Fixed` est requis
    // quand la physique est en FixedUpdate (sinon dt variable dans un step fixe). dt
    // physique = période `Time<Fixed>` → physique et FixedUpdate avancent au même pas.
    const PHYSICS_HZ: f64 = 64.0; // = défaut Bevy Time<Fixed> ; invariant tick sim.
    app.insert_resource(Time::<Fixed>::from_hz(PHYSICS_HZ));
    app.add_plugins(RapierPhysicsPlugin::<NoUserData>::default().in_fixed_schedule());
    app.insert_resource(TimestepMode::Fixed {
        dt: (1.0 / PHYSICS_HZ) as f32,
        substeps: 1,
    });
    // La chaîne gameplay DOIT précéder le step Rapier en FixedUpdate : `player_movement`
    // (GameSet::Movement) écrit `kcc.translation`, consommé par `PhysicsSet::SyncBackend`.
    // forgia-core (DAG-libre) ignore PhysicsSet → ordre câblé ici (forgia-game a rapier).
    app.configure_sets(
        FixedUpdate,
        GameSet::Movement.before(PhysicsSet::SyncBackend),
    );

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
        forgia_observability::prelude::ForgiaObservabilityPlugin,
        // Story-521 port V1 QA : bus BugReport + replay (recorder+player).
        // forgia-qa-harness + autopilot sont des frameworks de test (TestApp),
        // pas wirés runtime — usage via cargo test.
        forgia_qa_core::ForgiaQaCorePlugin,
        forgia_qa_replay::ForgiaQaReplayPlugin,
    ));

    // 5b. Story-457 (2026-05-19) — damage types + nameplate (split du bloc 5
    //     pour rester sous la limite tuple Bevy 15).
    // story-542 (2026-05-27) — ForgiaDamagePlugin guard symmetric avec ai-arena-bot:167.
    if !app.is_plugin_added::<forgia_damage::ForgiaDamagePlugin>() {
        app.add_plugins(forgia_damage::ForgiaDamagePlugin);
    }
    app.add_plugins((
        forgia_effects::damage_numbers::ForgiaDamageNumbersPlugin,
        forgia_enemy_nameplate::prelude::ForgiaEnemyNameplatePlugin,
    ));

    // 6. Cross-mode systems (utilisés par forgia-rpg, requis init_resource avant Startup)
    // Story-570 (2026-06-02) : meta-plugin data-layer RPG complet (inventory +
    // quests + xp + dialogue) — câble la boucle dialogue → inventaire/quête → XP.
    app.add_plugins(forgia_rpg_data::ForgiaRpgDataPlugin);
    // UI modale de dialogue (rend la DialogueSession + choix cliquables).
    app.add_plugins(forgia_ui_lib::dialogue::ForgiaUiDialoguePlugin);
    // Journal de quêtes (J) + tracker bord d'écran (story-58x Phase 2).
    app.add_plugins(forgia_ui_lib::quest_journal::ForgiaUiQuestPlugin);
    // Sacs à icônes + tooltips (I) (story-58x Phase 3).
    app.add_plugins(forgia_ui_lib::inventory_panel::ForgiaUiInventoryPlugin);
    // Fenêtre vendeur (achat/vente + or) (story-58x Phase 5).
    app.add_plugins(forgia_ui_lib::shop_panel::ForgiaUiShopPlugin);

    // 7. Mode-specific plugins (run_if interne par GameMode)
    app.add_plugins((
        forgia_asset_registry::prelude::ForgiaAssetRegistryPlugin,
        forgia_streaming::ForgiaStreamingPlugin, // story-450 chunk streaming foundation
        forgia_fps::prelude::ForgiaFpsPlugin,
        forgia_viewmodel::calibration_sensor::ForgiaViewmodelCalibrationPlugin,
        forgia_rpg::prelude::ForgiaRpgPlugin,
        forgia_mode_roguelite::prelude::ForgiaModeRoguelitePlugin,
        forgia_terrain::prelude::ForgiaTerrainPlugin,
        forgia_foliage::prelude::ForgiaFoliagePlugin,
        forgia_water::prelude::ForgiaWaterPlugin,
        forgia_audio::prelude::ForgiaAudioBiomePlugin,
        forgia_worldgen::ForgiaWorldgenPlugin, // story-578 P1 — procgen registry + spawn demo (F7/F8)
    ));

    // 7b. Anim Layer (story-437) + 3P camera (story-438) — utilisés par forgia-rpg
    app.add_plugins((
        forgia_anim_debug::prelude::ForgiaAnimDebugPlugin,
        forgia_camera_orbit::prelude::ForgiaCameraOrbitPlugin,
        forgia_secondary_motion::prelude::ForgiaSecondaryMotionPlugin,
    ));

    // 7d. Debug overlay dev-loop (story-547 + story-581) — monitor perf/mémoire/VRAM
    // egui multi-catégories. Master toggle = F2 (F3 reste les gizmos chunks RPG).
    // Brancher ce plugin était l'étape manquante : la crate forgia-debug existait
    // mais n'était ajoutée nulle part → monitor in-game invisible.
    app.add_plugins(forgia_debug::prelude::ForgiaDebugPlugin);

    // 7e. Démo perf "Cyber City" (2026-06-15) — GLB lourd + flycam libre,
    // entrée menu dédiée. Mode self-contained (GameMode::CyberCity).
    app.add_plugins(cyber_city::CyberCityDemoPlugin);

    // 7f. Color grading filmique par mode (story-602) — ColorGrading par GameMode,
    // hot-reload assets/genomes/color_grading.toml. Sensor forgia2_color_grading.json.
    app.add_plugins(color_grading::ColorGradingPlugin);

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
