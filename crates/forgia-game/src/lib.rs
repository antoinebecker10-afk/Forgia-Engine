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
#[cfg(feature = "dev-brp")]
use bevy::remote::{http::RemoteHttpPlugin, RemotePlugin};
use bevy_rapier3d::prelude::*;
use forgia_core::prelude::*;

/// Banc de blockout d'arène (story-667, 2026-07-27) — l'étape « greybox » du
/// process de level design, isolée du Roguelite. Géométrie grise pilotée par
/// `assets/genomes/arena_test.toml`, grille au sol à l'échelle des metrics joueur
/// mesurées. On joue la forme avant de l'habiller.
mod arena_test;
/// Vue 3ᵉ personne du Hall + avatar portant l'armure débloquée.
mod castle_avatar;
/// Éclairage par image du Hall (2026-07-26) — Forgia n'en avait aucun, d'où l'aspect
/// « pierre mouillée » : une surface PBR sans environnement à réfléchir tombe sur un
/// reflet plat. La cubemap vient des 27 sondes d'intérieur cuites par le créateur.
mod castle_envmap;
/// Inventaire d'armure du Hall (touche I) — même contenu qu'au menu.
mod castle_equipment_panel;
/// Bougies allumées + éclairage du Hall (2026-07-26) — les lumières et particules
/// du pack Unity ont été perdues à l'import (elles ne portent pas de mesh) : ce
/// module rend les ~300 bougeoirs vivants et remplace l'ambiante à 900, qui
/// compensait leur absence en aplatissant tout le modelé.
mod castle_flames;
/// Sol gazon du Hall de Forgia — terrain reconstruit depuis le Unity Terrain
/// (heightmap + splatmap chemins pavés + 21k tree-instances). RÉACTIVÉ 2026-07-25 :
/// orientation calée via `yaw_deg` du tune live `castle_ground_tune.json` (le
/// chemin pointe vers l'AVANT du château). Détail : reference_castle_terrain_unity_reconstruction.
mod castle_ground;
/// Hall de Forgia (2026-07-22) — hub social 3D walkable : château importé
/// (`castle_highlands.glb`), zone neutre sans combat. Entrée debug F10.
mod castle_hub;
/// Lumière cuite du créateur (2026-07-26) — 11 atlas portant 2 rebonds et une
/// occlusion ambiante. C'est le seul apport de lumière **indirecte** du Hall :
/// aucun éclairage temps réel ne produit de rebond.
mod castle_lightmaps;
/// Instances écartées par la reconstruction du château (2026-07-26) — les prefabs
/// composites (`_static`, `_lit`, `_comp`) n'avaient pas de FBX 1:1 et sont tombés
/// en silence : 50 bannières murales manquaient alors que leur mesh était chargé.
mod castle_props;
/// Color grading filmique par GameMode (story-602) — mood par mode (chaud/froid/
/// saturation), hot-reload genome. Orthogonal au tonemapping (composant distinct).
mod color_grading;
/// Démo perf moteur (2026-06-15) — "Cyber City démo" : charge un GLB lourd +
/// flycam libre pour stress-tester le rendu. Entrée via le menu principal.
mod cyber_city;

/// Build the App with all Forgia plugins wired, then run it. Returns `AppExit`.
pub fn run_game() -> AppExit {
    // Plusieurs lecteurs historiques utilisent encore `assets/...` ou
    // `config/...` via std::fs. Normaliser une seule fois le CWD rend le binaire
    // lançable depuis Explorer, target/debug ou un outil externe, tandis que
    // AssetPlugin reçoit toujours le chemin absolu canonique ci-dessous.
    #[cfg(not(target_arch = "wasm32"))]
    let asset_root = forgia_core::asset_paths::asset_root();
    let runtime_root = forgia_core::asset_paths::runtime_root();
    if let Err(error) = std::env::set_current_dir(&runtime_root) {
        eprintln!(
            "[forgia-game] impossible de fixer le répertoire runtime {}: {error}",
            runtime_root.display()
        );
    }

    let mut app = App::new();

    // Story-695 (portage web) : gardes wasm — les tonemappers a LUT embarquent
    // des KTX2 Rgba16Unorm sans equivalent WebGPU (panic wgpu au premier upload,
    // avant meme qu'une camera existe) ; et FPS/frame_time en console navigateur,
    // seule telemetrie possible sans fs tant que le sink web (inc.2) n'existe pas.
    #[cfg(target_arch = "wasm32")]
    {
        app.add_systems(
            Update,
            (wasm_neutralize_rgba16unorm_images, wasm_safe_tonemapping),
        );
        app.add_plugins((
            bevy::diagnostic::FrameTimeDiagnosticsPlugin::default(),
            bevy::diagnostic::LogDiagnosticsPlugin::default(),
        ));
    }

    // 1. Bevy DefaultPlugins EN PREMIER (fournit StatesPlugin requis par ForgiaCorePlugin)
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                // wasm : fetch HTTP relatif a la page — un chemin disque absolu
                // produirait des fetch file:// bloques par le navigateur.
                #[cfg(target_arch = "wasm32")]
                file_path: "assets".to_string(),
                #[cfg(not(target_arch = "wasm32"))]
                file_path: asset_root.to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some({
                    // Story-692 — le mode fenêtre vient des settings AVANT la
                    // création de la fenêtre : le défaut Windowed en dur
                    // flashait une fenêtre 1920×1080 (clampée à 1009 par l'OS)
                    // à chaque boot, le temps qu'`apply_window_settings`
                    // rattrape le choix du joueur. Défaut sans TOML =
                    // borderless, la cible officielle.
                    let (mode, w, h) = forgia_ui_lib::pause_menu::initial_window_config();
                    Window {
                        title: "Forgia V2".to_string(),
                        resolution: (w, h).into(),
                        mode,
                        ..default()
                    }
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
            })
            // Story-695 : sur wasm, Bevy demande les limites MINIMALES de la spec
            // WebGPU (8 storage buffers/stage) — insuffisant pour les layouts
            // hanabi. Functionality = demander les limites reelles de l'adaptateur.
            // En natif on garde le defaut (Compatibility) : comportement inchange.
            .set(bevy::render::RenderPlugin {
                render_creation: bevy::render::settings::WgpuSettings {
                    priority: if cfg!(target_arch = "wasm32") {
                        bevy::render::settings::WgpuSettingsPriority::Functionality
                    } else {
                        bevy::render::settings::WgpuSettings::default().priority
                    },
                    ..default()
                }
                .into(),
                ..default()
            }),
    );

    // Pont d'inspection ECS pour les agents de développement. La feature est
    // absente des builds normaux et release : le port BRP 15702 n'est donc
    // jamais ouvert par défaut.
    #[cfg(feature = "dev-brp")]
    app.add_plugins((RemotePlugin::default(), RemoteHttpPlugin::default()));

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

    // 5a. Le hub-menu du Roguelite, sorti du shell neutre par la story-694
    //     incrément 5. Son propre bloc : le tuple du bloc 5 était PLEIN (16 =
    //     le maximum de l'impl `Plugins` de Bevy), et y glisser une 17ᵉ entrée
    //     ne produit pas « tuple trop long » mais un `Plugins<_> is not
    //     satisfied` illisible. Anti-trap V1 « add_systems tuple > 20 », même
    //     cause côté plugins.
    //
    //     Il ajoute `ForgiaUiPlugin` lui-même si besoin (idempotent) — le bloc 5
    //     l'a déjà fait, cet appel est donc un no-op ; il documente la dépendance.
    app.add_plugins(forgia_menu_hub::prelude::ForgiaMenuHubPlugin);

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

    // 7e-bis. Hall de Forgia (2026-07-22) — hub social 3D walkable (château
    // importé). Zone neutre sans combat. Mode self-contained (GameMode::CastleHub),
    // entrée debug F10 depuis le menu. Aucune dépendance à forgia-mode-roguelite.
    app.add_plugins(castle_hub::CastleHubPlugin);
    // Le Hall se joue à la 3ᵉ personne : on y voit le personnage et son armure.
    app.add_plugins(castle_avatar::CastleAvatarPlugin);
    app.add_plugins(castle_equipment_panel::CastleEquipmentPanelPlugin);
    // 7e-ter. Sol/terrain du Hall — RÉACTIVÉ 2026-07-25 : reconstruction du Unity
    // Terrain (heightmap + splatmap chemins pavés + 21k tree-instances). L'orientation
    // (le chemin/la pente doit pointer vers l'AVANT du château) se cale via `yaw_deg`
    // du tune LIVE `castle_ground_tune.json` — hot-reload 1×/s, zéro rebuild. Le calage
    // fin position/relief (align, vscale) passe par le même fichier. Findings :
    // memory/reference_castle_terrain_unity_reconstruction.md.
    app.add_plugins(castle_ground::CastleGroundPlugin);
    // 7e-quinquies. Bougies allumées du Hall + éclairage data-driven
    // (`assets/genomes/castle_hub_lighting.toml`, hot-reload). Remplace l'ambiante
    // en dur à 900. Détail : docs/audits/audit-2026-07-26-comparaison-interieur-createur.md
    app.add_plugins(castle_flames::CastleFlamesPlugin);
    // 7e-sexies. Réimport des instances écartées par la reconstruction du château :
    // les variantes composites du pack (`_static`, `_lit`) n'ayant pas de FBX 1:1,
    // 50 bannières murales manquaient. Leur mesh est déjà chargé — on clone ses
    // handles. Détail : docs/audits/audit-2026-07-26-diff-complet-map-createur.md
    app.add_plugins(castle_props::CastlePropsPlugin);
    // 7e-septies. Éclairage par image du Hall : le projet n'en avait aucun, ce qui
    // laissait toute surface PBR sans rien à réfléchir. La cubemap est moyennée
    // depuis les 27 sondes d'intérieur cuites par le créateur du pack.
    app.add_plugins(castle_envmap::CastleEnvMapPlugin);
    // 7e-octies. Lumière CUITE du pack : 11 atlas portant deux rebonds et une
    // occlusion ambiante, associés pièce par pièce via la table extraite du binaire
    // Unity. C'est le seul apport de lumière indirecte du Hall — sans lui, tout ce
    // qui n'est pas frappé directement tombe au plancher de l'ambiante.
    // Étude : docs/audits/audit-2026-07-26-etude-eclairage.md
    app.add_plugins(castle_lightmaps::CastleLightmapsPlugin);
    // 7e-quater. Éditeur de scène in-game (story-665) — `.` du pavé numérique dans
    // le Hall : sélection, déplacement/rotation/échelle façon Blender, bibliothèque
    // d'assets, aimant au sol. Persistance non destructive dans
    // `castle_hub_edits.json`. Gaté `GameMode::CastleHub` en interne : les autres
    // modes ne paient que le boot du plugin. Les pinceaux de sol et la peinture
    // sont les lots suivants.
    app.add_plugins(forgia_editor::prelude::ForgiaEditorPlugin);

    // 7e-nonies. Banc de blockout d'arène (story-667) — onglet « Arena Test » du
    // menu. Mode self-contained (`GameMode::ArenaTest`), isolé du Roguelite pour
    // ne rien casser de ce qui tourne : aucune arène existante n'est touchée.
    // Géométrie 100 % data-driven (`assets/genomes/arena_test.toml`, hot-reload).
    app.add_plugins(arena_test::ArenaTestPlugin);

    // 7e-decies. Expédition (2026-08-14) — la première carte AUTORÉE du projet,
    // « Le Vallon » (280 × 200 m), bâtie sous Blender et chargée depuis ses deux
    // manifestes au lieu d'être générée. C'est le mode E2 du GDD, celui que
    // story-704 garde verrouillé au menu tant qu'il n'existe pas.
    //
    // Self-contained comme Arena Test : `GameMode::Expedition` et rien d'autre,
    // donc aucun risque pour ce qui tourne déjà.
    app.add_plugins(forgia_mode_expedition::ForgiaExpeditionPlugin);

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

/// Amorçage. Par défaut le menu ; `FORGIA_BOOT_MODE` entre directement dans un
/// mode, pour que le jeu puisse être PILOTÉ sans souris.
///
/// Sans cette porte, le seul chemin vers un mode passe par des clics dans le
/// menu : aucune vérification runtime n'est automatisable, et « lance et
/// ping-moi » reste la seule option — ce qui n'est pas une vérification, c'est
/// une délégation. Variable absente ou inconnue : comportement inchangé.
///
/// ```text
/// FORGIA_BOOT_MODE=arena_test cargo run -p forgia-game --release
/// ```
fn boot_to_menu(mut app_mode: ResMut<NextState<AppMode>>, mut game: ResMut<NextState<GameMode>>) {
    let demande = std::env::var("FORGIA_BOOT_MODE").unwrap_or_default();
    let direct = match demande.as_str() {
        "arena_test" => Some(GameMode::ArenaTest),
        "roguelite" => Some(GameMode::Roguelite),
        "castle_hub" => Some(GameMode::CastleHub),
        "fps" => Some(GameMode::Fps),
        "rpg" => Some(GameMode::Rpg),
        "" => None,
        autre => {
            warn!("[forgia-game] FORGIA_BOOT_MODE=\"{autre}\" inconnu — démarrage au menu");
            None
        }
    };
    match direct {
        Some(mode) => {
            info!("[forgia-game] amorçage direct sur {mode:?} (FORGIA_BOOT_MODE)");
            game.set(mode);
            app_mode.set(AppMode::InGame);
        }
        None => app_mode.set(AppMode::Menu),
    }
}

// ─── Gardes wasm (story-695, portage web) ────────────────────────────────────

/// Remplace toute image Rgba16Unorm (LUT tonemapping embarquees par Bevy) par le
/// placeholder 1x1x1 D3, avant que le renderer tente un upload WebGPU qui panique
/// (format sans equivalent dans la spec). Couple avec `wasm_safe_tonemapping` :
/// les cameras basculent sur un tonemapper pur shader, la LUT n'est jamais lue.
#[cfg(target_arch = "wasm32")]
fn wasm_neutralize_rgba16unorm_images(mut images: ResMut<Assets<bevy::image::Image>>) {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let ids: Vec<_> = images
        .iter()
        .filter(|(_, img)| img.texture_descriptor.format == TextureFormat::Rgba16Unorm)
        .map(|(id, _)| id)
        .collect();
    for id in ids {
        if let Some(img) = images.get_mut(id) {
            *img = bevy::image::Image::new_fill(
                Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D3,
                &[128, 128, 128, 255],
                TextureFormat::Rgba8Unorm,
                bevy::asset::RenderAssetUsages::RENDER_WORLD,
            );
        }
    }
}

/// Bascule les tonemappers a LUT (TonyMcMapface/AgX/BlenderFilmic) vers
/// AcesFitted (pur shader) sur toute camera, y compris via le menu pause.
#[cfg(target_arch = "wasm32")]
fn wasm_safe_tonemapping(
    mut tonemappings: Query<
        &mut bevy::core_pipeline::tonemapping::Tonemapping,
        Changed<bevy::core_pipeline::tonemapping::Tonemapping>,
    >,
) {
    use bevy::core_pipeline::tonemapping::Tonemapping as T;
    for mut t in &mut tonemappings {
        if matches!(*t, T::TonyMcMapface | T::AgX | T::BlenderFilmic) {
            *t = T::AcesFitted;
        }
    }
}
