//! # forgia-ui
//!
//! Menu (Start + choix FPS/RPG + Pause + Settings) + HUD partagé.
//!
//! **Anti-traps V1 enforced** :
//! - 1 seul handler ESC
//! - `MenuCamera2d` isolé OnEnter(Menu)/OnExit(Menu)
//! - `Time<Real>` pour sensors UI
//!
//! Crates atomiques wire-up :
//! - `forgia-crosshair` : crosshair + sniper scope overlay
//! - `forgia-hitmarker` : hit confirm visual

use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_input::prelude::InputBlockers;
use forgia_ui_lib::pause_menu::{draw_settings_controls, save_user_settings, UserSettings};
use forgia_ui_lib::style::{
    cartoon_btn, glass_btn, glass_frame_hero, C_PRIMARY, C_TEXT_MUTED, FORGE_AME, FORGE_CREME,
    FORGE_ECLAT, FORGE_OR, FORGE_PANEL, HAIR_GOLD_STRONG,
};
use forgia_ui_lib::theme::display_text;
// Hub-menu (story-menu-hub) : le menu-titre devient le hub roguelite complet.
// `forgia-ui` → `forgia-mode-roguelite` dep existe (pas de cycle) → on lit les Res
// persistées au Startup (présentes dès le menu) et on réutilise les helpers de
// sections déjà écrits dans `hub.rs` (zéro duplication).
use forgia_mode_roguelite::chapters::{chapter_da_name, chapter_select_content};
use forgia_mode_roguelite::decor_palettes::DecorPalettesConfig;
use forgia_mode_roguelite::elements::ElementConfig;
use forgia_mode_roguelite::equipment::{
    power_score, EquipmentConfig, EquipmentMods, EquipmentPanelShown, EquipmentSave,
};
use forgia_mode_roguelite::hub::{draw_codex_section, section_intro};
use forgia_mode_roguelite::identity::{draw_identity_content, IdentityConfig, IdentitySave};
use forgia_mode_roguelite::meta_shop::{
    apply_meta_purchase, chapter_unlocked, draw_enclume_panel, MetaShopCatalogue, MetaShopSave,
    SelectedChapter, CHAPTERS_PER_BOOK,
};
use forgia_mode_roguelite::rounds::RoundsConfig;
use forgia_mode_roguelite::pipeline_warmup::WarmupState;
use forgia_mode_roguelite::progress::PlayerProgress;
use forgia_mode_roguelite::run::MetaSouls;
use forgia_mode_roguelite::weapon_select::{
    draw_weapon_menu_panel, StartingWeaponChoice, WeaponCards,
};
use forgia_mode_roguelite::StartRunEvent;
// Re-exports backward compat (déplacés vers crates atomiques 2026-05-16)
pub use forgia_crosshair::CrosshairMode;
pub use forgia_effects::hitmarker::HitmarkerState;

pub mod prelude {
    pub use crate::ForgiaUiPlugin;
    /// Re-export backward compat — préférer `forgia_crosshair::CrosshairMode` direct.
    pub use forgia_crosshair::CrosshairMode;
    /// Re-export backward compat — préférer `forgia_effects::hitmarker::HitmarkerState` direct.
    pub use forgia_effects::hitmarker::HitmarkerState;
}

/// Fond du menu = l'arène du chapitre atteint (diorama RTT). Story-678 Phase 5.
mod arena_backdrop;
/// Icônes des deux monnaies (Âmes / Éclats) — story-678.
mod currency_icons;
/// Silhouettes des cinq types d'équipement, tracées en egui — story-678.
mod slot_glyph;
/// Fond vidéo du menu (frames webp pré-extraites → cache LRU egui). Porté V1.
/// Reste le REPLI quand `ui_backdrop_enabled = 0` ou que le diorama n'a rien posé.
mod gamepad_nav;
mod menu_hub_sensor;
mod menu_video;
use arena_backdrop::{ArenaBackdropPlugin, ArenaBackdropRtt};
use currency_icons::{CurrencyIcons, CurrencyIconsPlugin, CURRENCY_ICON, CURRENCY_ICON_SMALL};
use menu_video::MenuVideoState;

/// Aperçus 3D (arme + bras) au hub-menu (render-to-texture). Story-menu-hub étape 5b.
mod weapon_preview;
use weapon_preview::{WeaponPreviewPlugin, WeaponPreviewRtt};

pub struct ForgiaUiPlugin;

impl Plugin for ForgiaUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }
        // Crates atomiques (règle fine-grained-crates) — idempotent.
        if !app.is_plugin_added::<forgia_crosshair::ForgiaCrosshairPlugin>() {
            app.add_plugins(forgia_crosshair::ForgiaCrosshairPlugin);
        }
        if !app.is_plugin_added::<forgia_effects::hitmarker::ForgiaHitmarkerPlugin>() {
            app.add_plugins(forgia_effects::hitmarker::ForgiaHitmarkerPlugin);
        }
        // Aperçu 3D d'arme au hub-menu (RTT) — cycle de vie sur AppMode::Menu.
        if !app.is_plugin_added::<WeaponPreviewPlugin>() {
            app.add_plugins(WeaponPreviewPlugin);
        }
        // Fond d'arène du menu (RTT). APRÈS l'aperçu personnage : sa caméra rend
        // le layer du personnage, qui doit donc exister.
        if !app.is_plugin_added::<ArenaBackdropPlugin>() {
            app.add_plugins(ArenaBackdropPlugin);
        }
        if !app.is_plugin_added::<CurrencyIconsPlugin>() {
            app.add_plugins(CurrencyIconsPlugin);
        }
        // MenuCamera2d permanente : spawn 1 fois Startup, JAMAIS despawn.
        // Ordre explicite high pour render egui par-dessus la Camera3d gameplay.
        // Anti-trap V1 : éviter le frame où aucune caméra n'existe (ESC bug).
        app.init_resource::<MenuPage>()
            .init_resource::<HubBadges>()
            .init_resource::<gamepad_nav::LastInputKind>()
            // Story-678 Phase 6 — manette : traduction en événements clavier
            // egui, injectée entre ProcessInput et BeginPass (hook documenté).
            .add_systems(
                PreUpdate,
                gamepad_nav::sys_gamepad_menu_nav
                    .after(bevy_egui::EguiPreUpdateSet::ProcessInput)
                    .before(bevy_egui::EguiPreUpdateSet::BeginPass),
            )
            .add_systems(Update, gamepad_nav::sys_track_input_kind)
            .init_resource::<menu_hub_sensor::MenuHubSensorState>()
            .add_systems(
                Update,
                menu_hub_sensor::sys_write_menu_hub_sensor.in_set(GameSet::Sensors),
            )
            .add_systems(OnEnter(AppMode::Menu), gamepad_nav::sys_style_gamepad_focus)
            .add_systems(Startup, (spawn_menu_camera_permanent, menu_video::setup_menu_video))
            // Fond vidéo menu : tick (avance frame) + sensor (forgia2_menu_video.json).
            .add_systems(
                Update,
                (menu_video::menu_video_tick, menu_video::menu_video_sensor),
            )
            // Story-678 Phase 2 — miroir UserSettings → mémoire egui, chaque
            // frame et sans garde is_changed (leçon « boons inertes ») : les
            // impulsions de boutons vivent aussi in-game (coffre, pause).
            // Phase 4 — pastilles de la sidebar (état réel, marquage « vu »).
            .add_systems(Update, (sys_mirror_ui_motion, sys_hub_badges))
            // Menu titre : curseur libre + reset à la page racine à chaque retour menu.
            .add_systems(OnEnter(AppMode::Menu), (release_cursor, reset_menu_page))
            .add_systems(OnEnter(AppMode::InGame), grab_cursor)
            .add_systems(OnEnter(AppMode::Paused), (release_cursor, pause_time))
            .add_systems(OnExit(AppMode::Paused), resume_time)
            // Story-528 follow-up — Roguelite Defeat/Victory : cursor libre pour
            // cliquer "Nouvelle Run" / "Retour Menu" du defeat_overlay. Sans ça,
            // mouse_look continue de pivoter la caméra pendant l'écran fin de run.
            .add_systems(
                OnEnter(forgia_mode_roguelite::RunState::Defeat),
                (release_cursor, block_look_on),
            )
            .add_systems(
                OnEnter(forgia_mode_roguelite::RunState::Victory),
                (release_cursor, block_look_on),
            )
            .add_systems(
                OnExit(forgia_mode_roguelite::RunState::Defeat),
                (grab_cursor, block_look_off),
            )
            .add_systems(
                OnExit(forgia_mode_roguelite::RunState::Victory),
                (grab_cursor, block_look_off),
            )
            // Story-596 Phase B — Lobby (Enclume) : curseur libre pour cliquer
            // cartes d'upgrade + FORGER. Gated Roguelite : RunState est global,
            // au boot/RPG GameMode ≠ Roguelite → no-op (sinon block_look
            // fuiterait dans le RPG).
            .add_systems(
                OnEnter(forgia_mode_roguelite::RunState::Lobby),
                (release_cursor, block_look_on).run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(
                OnExit(forgia_mode_roguelite::RunState::Lobby),
                (grab_cursor, block_look_off).run_if(in_state(GameMode::Roguelite)),
            )
            // Story-558 Phase 7 follow-up (2026-05-29) — sync cursor avec
            // CoffreSession.is_open : pendant le break Coffre, libérer la
            // souris pour cliquer Skip/Reroll/cartes sans pivoter la caméra.
            .add_systems(Update, (sys_sync_cursor_with_coffre, sys_regrab_cursor_on_focus))
            // Fix « pas de souris au lancement » (design: roguelite-home-hub-proposal
            // 2026-06-26, P1) : à l'entrée Roguelite, OnEnter(InGame)→grab_cursor (LOCK)
            // et OnEnter(RunState::Lobby)→release_cursor (FREE) tirent la même frame sur
            // deux schedules SANS ordre → le grab gagnait, curseur verrouillé alors que
            // le wizard d'arme est affiché. Ce réconciliateur par-frame est l'unique
            // source de vérité du curseur au Lobby (set-if-different, zéro churn).
            .add_systems(
                Update,
                sys_force_lobby_cursor_free
                    .run_if(in_state(AppMode::InGame))
                    .run_if(in_state(GameMode::Roguelite))
                    .run_if(in_state(forgia_mode_roguelite::RunState::Lobby)),
            )
            // Story-455 Phase G — paused_overlay_ui retiré (remplacé par forgia-ui-pause-menu
            // cliquable Resume / Settings / Quit). Le handler ESC/Q reste ici (escape_handler).
            // Hub-menu : menu principal + sections interactives (Enclume cliquable,
            // Forgeron/identité). Areas indépendantes, chaînées pour un ordre de
            // dessin déterministe.
            .add_systems(
                EguiPrimaryContextPass,
                (
                    sys_publish_viewport_h,
                    main_menu_ui,
                    sys_menu_root_dashboard,
                    gamepad_nav::sys_menu_gamepad_hints,
                    sys_menu_livre,
                    sys_menu_enclume,
                    sys_menu_forgeron,
                    sys_menu_marketplace,
                    sys_menu_armes,
                )
                    .chain(),
            )
            .add_systems(Update, escape_handler.in_set(GameSet::UI))
            // Étape 6 hub-menu — lancement direct : le Lobby est un GATE DE
            // CHARGEMENT (le hub est au menu). Auto-start dès warmup PBR prêt +
            // overlay de chargement qui couvre l'ancien hub le temps du warmup.
            .add_systems(
                Update,
                sys_auto_start_when_warm
                    // Ordonné AVANT le reader `sys_start_run` (autre crate) : le
                    // `StartRunEvent` écrit est lu la même frame → transition Lobby→
                    // InRun immédiate (lève l'ambiguïté d'ordre intra-frame, qa-lead M1).
                    .before(forgia_mode_roguelite::run::sys_start_run)
                    .run_if(in_state(AppMode::InGame))
                    .run_if(in_state(GameMode::Roguelite))
                    .run_if(in_state(forgia_mode_roguelite::RunState::Lobby)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                sys_lobby_loading_overlay
                    .run_if(in_state(AppMode::InGame))
                    .run_if(in_state(GameMode::Roguelite))
                    .run_if(in_state(forgia_mode_roguelite::RunState::Lobby)),
            );
    }
}

/// Étape 6 hub-menu — lancement direct : le Lobby n'est plus un hub interactif mais
/// un **gate de chargement**. Dès que le warmup PBR est prêt (`WarmupState.done`),
/// auto-fire `StartRunEvent` → combat, sans action utilisateur (tout est configuré
/// au menu). L'anti-double-spawn de `sys_start_run` + `run_if(Lobby)` (qui coupe
/// après la transition InRun) bornent le tir. Replay (Defeat/Victory → Lobby) :
/// `done` reste vrai → lancement instantané.
fn sys_auto_start_when_warm(
    warmup: Option<Res<WarmupState>>,
    mut start_run: MessageWriter<StartRunEvent>,
) {
    if warmup.as_ref().is_some_and(|w| w.done) {
        start_run.write(StartRunEvent { seed: None });
    }
}

/// Overlay de chargement plein écran pendant le gate Lobby — couvre l'ancien hub
/// interactif le temps du warmup, avant l'auto-lancement. Layer Foreground (au-
/// dessus des panneaux du hub, ordre Middle).
fn sys_lobby_loading_overlay(mut contexts: EguiContexts, warmup: Option<Res<WarmupState>>) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let ready = warmup.as_ref().is_some_and(|w| w.done);
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("lobby_loading_overlay"),
    ));
    painter.rect_filled(screen, 0.0, egui::Color32::from_rgb(12, 9, 16));
    painter.text(
        screen.center(),
        egui::Align2::CENTER_CENTER,
        if ready {
            "La Forge est prête…"
        } else {
            "Préparation de la Forge…"
        },
        egui::FontId::proportional(34.0),
        FORGE_CREME,
    );
    painter.text(
        screen.center() + egui::vec2(0.0, 44.0),
        egui::Align2::CENTER_CENTER,
        "◆ ◆ ◆",
        egui::FontId::proportional(18.0),
        FORGE_OR,
    );
}

#[derive(Component)]
struct MenuCamera2d;

/// MenuCamera2d permanente — spawn une fois au Startup, JAMAIS despawn.
fn spawn_menu_camera_permanent(mut commands: Commands, q: Query<Entity, With<MenuCamera2d>>) {
    if q.is_empty() {
        commands.spawn((
            Camera2d,
            Camera {
                order: 10,
                clear_color: ClearColorConfig::None,
                ..default()
            },
            MenuCamera2d,
            Name::new("MenuCamera2d (permanent)"),
        ));
        info!("[forgia-ui] MenuCamera2d spawned (permanent, order=10, clear=None)");
    }
}

/// Sous-page du menu titre — devenu **hub roguelite complet** (story-menu-hub).
/// Navigation purement UI-locale : PAS un variant d'`AppMode` (qui vit dans
/// forgia-core et est partagé). Reset à `Root` sur OnEnter(AppMode::Menu).
///
/// `Root` = accueil (titre FORGIA + CONTINUER/NOUVELLE PARTIE). Les autres = les
/// sections du hub, navigables depuis la sidebar verre sans lancer de run.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
enum MenuPage {
    #[default]
    Root,
    Forgeron,
    Armes,
    Talents,
    Enclume,
    Codex,
    Missions,
    Succes,
    Stats,
    // `ArenaTest` (banc de blockout, story-667) RETIRÉ DU MENU le 2026-07-30 —
    // temporaire, et rien n'est supprimé côté moteur : `GameMode::ArenaTest`, le
    // plugin, le génome, le générateur et les 26 tests restent en place.
    // Accès entre-temps : `FORGIA_BOOT_MODE=arena_test`.
    // Pour rouvrir l'onglet : restaurer la variante ici, son `nav_label`, son
    // `section_title`, son bras de `draw_page` et `draw_arena_test_section`
    // (voir l'historique git de ce fichier).
    /// Le Livre — les dix chapitres, leurs verrous, celui qu'on va jouer.
    ///
    /// Sa place est ICI et pas au Lobby : le Lobby est un gate de chargement
    /// traversé par le démarrage automatique, le hub est au menu.
    Livre,
    /// Le MARKETPLACE — décors, couleurs, bras, musique (story-678).
    ///
    /// Hors sidebar comme Forgeron et Livre : on y entre depuis le Forgeron,
    /// qui est la page « qui je suis ». Elle a son propre système parce qu'une
    /// page riche tient mal sous le plafond de 16 paramètres Bevy quand elle
    /// partage le sien.
    Marketplace,
    /// **Le SAC** — les pièces d'armure possédées (story-678).
    Sac,
    Options,
}

impl MenuPage {
    /// Ordre de la sidebar de navigation (Accueil en tête, Options en pied).
    // Livre + Forgeron RETIRÉS de la sidebar (2026-08-05, story-678) : leur
    // contenu vit sur l'ACCUEIL (écran de préparation, modèle Dicero!). Les
    // pages existent toujours — Forgeron via « Personnaliser », Livre absorbé
    // par le carrousel de chapitres.
    const NAV: [MenuPage; 11] = [
        MenuPage::Root,
        MenuPage::Armes,
        // Story-678 — le Sac et le Marketplace montent dans la barre : ce sont
        // les deux écrans où l'on PREND quelque chose, ils ne doivent pas être
        // enfouis derrière la page Forgeron.
        MenuPage::Sac,
        MenuPage::Marketplace,
        MenuPage::Talents,
        MenuPage::Enclume,
        MenuPage::Codex,
        MenuPage::Missions,
        MenuPage::Succes,
        MenuPage::Stats,
        // `MenuPage::ArenaTest` RETIRÉ DE LA NAVIGATION (2026-07-30, temporaire).
        // Le banc de blockout reste entièrement en place — page, section, mode,
        // génome, générateur, tests. Seule son entrée de menu est masquée, le
        // temps que le sujet redevienne d'actualité.
        // Pour le rouvrir : remettre `MenuPage::ArenaTest,` sur cette ligne.
        // Accès entre-temps : `FORGIA_BOOT_MODE=arena_test`.
        MenuPage::Options,
    ];

    /// Libellé de l'onglet dans la sidebar (icône + nom court).
    fn nav_label(self) -> &'static str {
        match self {
            MenuPage::Root => "🏠  Accueil",
            MenuPage::Livre => "📕  Le Livre",
            MenuPage::Forgeron => "⚒  Forgeron",
            MenuPage::Armes => "🗡  Armes",
            MenuPage::Talents => "✨  Talents",
            MenuPage::Enclume => "🔨  Enclume",
            MenuPage::Codex => "📖  Codex",
            MenuPage::Missions => "🎯  Missions",
            MenuPage::Succes => "🏆  Succès",
            MenuPage::Stats => "📊  Stats",
            MenuPage::Marketplace => "💰  Marketplace",
            MenuPage::Sac => "🎒  Sac",
            MenuPage::Options => "⚙  Options",
        }
    }

    /// Titre display (Lilita) affiché en tête du panneau de section.
    fn section_title(self) -> &'static str {
        match self {
            MenuPage::Root => "FORGIA",
            MenuPage::Livre => "LE LIVRE",
            MenuPage::Forgeron => "TON FORGERON",
            MenuPage::Armes => "TES ARMES",
            MenuPage::Talents => "TALENTS",
            MenuPage::Enclume => "L'ENCLUME DES ÂMES",
            MenuPage::Codex => "CODEX · BESTIAIRE",
            MenuPage::Missions => "MISSIONS",
            MenuPage::Succes => "HAUTS FAITS",
            MenuPage::Stats => "STATISTIQUES",
            MenuPage::Marketplace => "MARKETPLACE",
            MenuPage::Sac => "TON SAC",
            MenuPage::Options => "OPTIONS",
        }
    }
}

/// Revient à la page racine du menu à chaque entrée dans le menu (retour jeu→menu).
fn reset_menu_page(mut page: ResMut<MenuPage>) {
    *page = MenuPage::Root;
}

/// Story-678 Phase 2 — pousse le toggle motion persisté (UserSettings) vers la
/// mémoire egui où vivent les helpers (`forgia_ui_lib::motion`). Un insert de
/// bool par frame : idempotent, trop bon marché pour mériter une garde.
fn sys_mirror_ui_motion(
    mut contexts: EguiContexts,
    settings: Option<Res<forgia_ui_lib::pause_menu::UserSettings>>,
) {
    let Some(settings) = settings else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    forgia_ui_lib::motion::set_motion_enabled(ctx, settings.ui_motion_enabled);
}

/// Hauteur logique de la fenêtre, publiée dans la mémoire egui.
///
/// La hauteur vient de la SOURCE (`Window` côté Bevy) plutôt que d'une API de
/// contexte egui, dont la valeur dépend des panels déjà posés cette frame.
///
/// ⚠️ Ce n'est PAS ce qui coupait les pages à mi-écran — cf. `set_max_height`
/// dans `hub_section_panel`. Trois correctifs successifs ont visé cette mesure
/// alors que le limiteur était ailleurs ; c'est une sonde, pas un raisonnement,
/// qui l'a montré.
fn sys_publish_viewport_h(
    mut contexts: EguiContexts,
    q_win: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut last: Local<f32>,
) {
    let Ok(win) = q_win.single() else { return };
    let h = win.height();
    let Ok(ctx) = contexts.ctx_mut() else { return };
    if (h - *last).abs() > 1.0 {
        info!("[hub] hauteur de fenêtre = {h:.0}");
        *last = h;
    }
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("forgia_viewport_h"), h));
}

fn main_menu_ui(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut exit: MessageWriter<AppExit>,
    mut video: Option<ResMut<MenuVideoState>>,
    asset_server: Res<AssetServer>,
    mut page: ResMut<MenuPage>,
    mut settings: ResMut<UserSettings>,
    // Données persistées au Startup → présentes dès le menu (hub roguelite).
    meta_save: Option<Res<MetaShopSave>>,
    progress: Option<Res<PlayerProgress>>,
    identity: Option<Res<IdentitySave>>,
    // Story-678 Phase 4 — pastilles calculées par `sys_hub_badges`.
    badges: Res<HubBadges>,
    // Story-678 Phase 5 — le fond d'arène du chapitre atteint (diorama RTT).
    backdrop: Option<Res<ArenaBackdropRtt>>,
    // Story-678 — icônes des monnaies (absentes tant qu'egui n'a pas de contexte).
    icons: Option<Res<CurrencyIcons>>,
    mut last_state: Local<Option<AppMode>>,
) {
    let current = app_state.get().clone();
    if last_state.as_ref() != Some(&current) {
        info!("[forgia-ui] main_menu_ui state = {current:?}");
        *last_state = Some(current.clone());
    }
    if current != AppMode::Menu {
        return;
    }

    // ── Quel fond ? ──
    // Story-678 Phase 5 — l'arène du chapitre atteint remplace la vidéo de
    // branding. On ne bascule QUE si le diorama a réellement posé des props :
    // une palette vide donnerait un écran nu, et le repli vidéo vaut mieux
    // qu'un fond noir. Le capteur publie ce compte.
    let arena_bg = backdrop
        .as_deref()
        .filter(|b| b.is_showing())
        .map(|b| b.tex_id);
    // Fond vidéo : maintient le cache LRU + preroll, renvoie le TextureId de la
    // frame courante. None → fallback fond sombre uni (preroll en cours / asset
    // absent). Calculé AVANT `ctx_mut` (libère le &mut EguiContexts).
    //
    // Sous l'arène, le pipeline vidéo est GELÉ (story-691) : le tick n'avance
    // plus la frame et on ne touche plus au cache — les 8 frames restent en
    // VRAM et `prerolled` reste vrai, donc couper `ui_backdrop_enabled` à chaud
    // réaffiche la vidéo immédiatement, sans re-preroll ni décodage à vide.
    let video_bg = if arena_bg.is_some() {
        None
    } else {
        video
            .as_deref_mut()
            .and_then(|v| menu_video::ensure_menu_video_frame(&mut contexts, v, &asset_server))
    };
    let bg_id = arena_bg.or(video_bg);
    // Le scrim s'adapte : sur l'arène il épargne le tiers droit (le personnage
    // y est), sur la vidéo il reste le dégradé vertical d'origine.
    let bg_is_arena = arena_bg.is_some();

    let Ok(ctx) = contexts.ctx_mut() else {
        warn!("[forgia-ui] main_menu_ui: egui ctx not found (no Camera2d?)");
        return;
    };

    // ── Couche background : fond vidéo plein écran + scrim dégradé vertical ──
    // (story-596 — haut léger, bas dense pour asseoir l'UI sans éteindre la vidéo).
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(8, 8, 12)))
        .show(ctx, |ui| {
            if let Some(id) = bg_id {
                let rect = ui.max_rect();
                ui.painter().image(
                    id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                // Scrim : assied l'UI sans éteindre le fond.
                //
                // Sur l'ARÈNE il devient DIAGONAL — dense à gauche, où vivent
                // la carte de chapitre et son texte ; presque nul à droite, où
                // le personnage se tient. Un dégradé purement vertical
                // (celui de la vidéo) noyait le personnage dans le même voile
                // que le texte, alors qu'il est le sujet de ce côté-là.
                let mut scrim = egui::Mesh::default();
                let (lt, rt, rb, lb) = if bg_is_arena {
                    (
                        egui::Color32::from_black_alpha(130),
                        egui::Color32::from_black_alpha(25),
                        egui::Color32::from_black_alpha(85),
                        egui::Color32::from_black_alpha(185),
                    )
                } else {
                    let top = egui::Color32::from_black_alpha(40);
                    let bottom = egui::Color32::from_black_alpha(170);
                    (top, top, bottom, bottom)
                };
                scrim.colored_vertex(rect.left_top(), lt);
                scrim.colored_vertex(rect.right_top(), rt);
                scrim.colored_vertex(rect.right_bottom(), rb);
                scrim.colored_vertex(rect.left_bottom(), lb);
                scrim.add_triangle(0, 1, 2);
                scrim.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(scrim));
            }
            // Story-678 — le fantôme plein écran de l'avatar a été remplacé par
            // le portrait CADRÉ de l'écran de préparation (le personnage géant
            // débordait du cadre, retour utilisateur 2026-08-05). Le portrait
            // vit dans `sys_menu_root_dashboard` (colonne droite).
        });

    // ── Données persistées (chargées au Startup → lisibles dès le menu) ──
    let souls_n = meta_save.as_ref().map(|s| s.souls_total).unwrap_or(0);
    let shards_n = meta_save.as_ref().map(|s| s.shards_total).unwrap_or(0);
    let name = identity
        .as_ref()
        .map(|i| {
            if i.player_name.is_empty() {
                "Forgeron"
            } else {
                i.player_name.as_str()
            }
        })
        .unwrap_or("Forgeron");
    // Story-680 cran 1 — le niveau est la SOMME DES RANGS achetés à l'Enclume
    // (modèle Gunfire Reborn). Plus d'XP « 40 + secondes de run », qui payait le
    // temps passé et rien d'autre. La barre mesure la part de l'Enclume
    // réellement débloquée — une information vraie.
    let (level, remaining, frac) = progress
        .as_ref()
        .map(|p| (p.level, p.ranks_remaining, p.completion()))
        .unwrap_or((1, 0, 0.0));

    // ── Chrome persistant du haut d'écran ──
    // Il se lit de gauche à droite comme une phrase : QUI je suis · OÙ je vais ·
    // CE QUE j'ai. Les trois vivent sur la même bande depuis que la navigation
    // est passée à l'horizontale.
    draw_hub_smith_chip(ctx, name, level, remaining, frac);
    draw_hub_nav(ctx, &mut page, *badges);
    draw_hub_souls_chip(ctx, souls_n, shards_n, icons.as_deref());

    // ── Panneau de la section active ──
    // Les sections rendent leur contenu et renvoient une action de navigation
    // d'état (lancer une run / quitter) que l'on applique ici — évite de passer
    // NextState/MessageWriter dans chaque helper.
    let action = match *page {
        MenuPage::Root => draw_root_landing(ctx),
        MenuPage::Codex => {
            hub_section_panel(
                ctx,
                "hub_sec_codex",
                MenuPage::Codex.section_title(),
                720.0,
                |ui| {
                    draw_codex_section(ui);
                },
            );
            MenuAction::None
        }
        MenuPage::Talents => {
            // Story-680 cran 1 — cet écran ANNONÇAIT « Choisis ton style — Feu ·
            // Givre · Éclair · Poison » et « N point(s) de talent en attente »
            // alors qu'il n'existait ni arbre, ni style, ni combo, et que RIEN
            // dans le workspace ne dépensait ces points. Un système vide est
            // neutre ; un système qui promet ce qu'il ne fait pas est une
            // déception programmée. On dit donc ce qui est vrai.
            hub_section_panel(
                ctx,
                "hub_sec_talents",
                MenuPage::Talents.section_title(),
                640.0,
                |ui| {
                    section_intro(
                        ui,
                        "Ton niveau",
                        "Ton niveau est la somme de ce que tu as débloqué à l'Enclume.                          Rien à farmer : chaque niveau est un choix que tu as fait.",
                    );
                    ui.add_space(8.0);
                    ui.label(display_text(format!("Niveau {level}"), 22.0, FORGE_OR));
                    ui.add_space(4.0);
                    ui.label(display_text(
                        if remaining == 0 {
                            "Enclume complète — tout est débloqué.".to_string()
                        } else {
                            format!("{remaining} amélioration(s) t'attendent à l'Enclume.")
                        },
                        16.0,
                        FORGE_CREME,
                    ));
                },
            );
            MenuAction::None
        }
        MenuPage::Missions => {
            // Même doctrine que Talents (story-680 cran 1) : ces deux pages
            // promettaient des Âmes et des titres qu'aucun système ne verse.
            hub_section_panel(
                ctx,
                "hub_sec_missions",
                MenuPage::Missions.section_title(),
                640.0,
                |ui| {
                    section_intro(
                        ui,
                        "Missions",
                        "Rien à accomplir pour l'instant — les défis quotidiens et \
                         hebdomadaires n'existent pas encore. Quand ils arriveront, \
                         c'est ici qu'ils t'attendront.",
                    );
                },
            );
            MenuAction::None
        }
        MenuPage::Succes => {
            hub_section_panel(
                ctx,
                "hub_sec_succes",
                MenuPage::Succes.section_title(),
                640.0,
                |ui| {
                    section_intro(
                        ui,
                        "Hauts faits",
                        "Aucun haut fait n'est encore suivi. Cette page s'ouvrira \
                         quand tes exploits seront comptés.",
                    );
                },
            );
            MenuAction::None
        }
        MenuPage::Stats => {
            hub_section_panel(
                ctx,
                "hub_sec_stats",
                MenuPage::Stats.section_title(),
                560.0,
                |ui| draw_stats_section(ui, meta_save.as_deref(), level),
            );
            MenuAction::None
        }
        // Forgeron / Armes / Enclume : panneaux interactifs dessinés par leurs
        // systèmes dédiés (`sys_menu_forgeron` / `sys_menu_armes` / `sys_menu_enclume`).
        MenuPage::Forgeron
        | MenuPage::Armes
        | MenuPage::Enclume
        | MenuPage::Livre
        | MenuPage::Marketplace
        | MenuPage::Sac => MenuAction::None,
        MenuPage::Options => {
            draw_options_page(ctx, &mut page, &mut settings);
            MenuAction::None
        }
    };

    match action {
        MenuAction::Launch(mode) => {
            next_game.set(mode);
            next_app.set(AppMode::InGame);
        }
        MenuAction::Quit => {
            exit.write(AppExit::Success);
        }
        MenuAction::None => {}
    }
}

/// Action de navigation d'état demandée par une section du hub-menu (appliquée par
/// `main_menu_ui`). Découple le rendu des sections des `NextState`/`MessageWriter`.
enum MenuAction {
    None,
    /// Entrer InGame dans le mode donné (Roguelite = run, Rpg/CyberCity = démos dev).
    Launch(GameMode),
    Quit,
}

// `draw_arena_test_section` retirée avec l'onglet (2026-07-30, temporaire) — elle
// n'avait plus qu'un appelant, supprimé lui aussi. Son contenu est dans
// l'historique git de ce fichier ; la restaurer suffit à rouvrir le banc.
// Rien n'a été touché côté moteur.

/// Hauteur réservée en haut de l'écran par le chrome du hub (barre de
/// navigation horizontale + chips d'état qui l'encadrent).
///
/// **Source unique.** La barre, les chips et TOUTES les pages s'y réfèrent :
/// une page qui recopierait sa propre marge finirait par passer sous la barre
/// le jour où celle-ci change de taille (`feedback_une_grandeur_ecrite_deux_fois`).
const HUB_TOP_BAR_H: f32 = 84.0;

/// Respiration entre le chrome du haut et le contenu de la page.
const HUB_CONTENT_GAP: f32 = 18.0;

/// Ordonnée du haut de tout contenu de page.
fn hub_content_top() -> f32 {
    HUB_TOP_BAR_H + HUB_CONTENT_GAP
}

/// Cadre « chip » cohérent avec le hub (verre aubergine + liseré or).
fn hub_chip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(FORGE_PANEL)
        .inner_margin(egui::Margin::symmetric(14, 8))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
}

/// Bandeau des deux monnaies — haut-droite du hub-menu.
///
/// Story-678 : les **Éclats** (cosmétique) sont affichés SOUS les Âmes
/// (puissance). Les montrer ensemble est le seul moyen de faire comprendre
/// qu'elles ne se remplacent pas — une seule ligne « 1729 » laisserait croire
/// qu'un achat au Marketplace coûte des rangs d'Enclume.
fn draw_hub_souls_chip(
    ctx: &egui::Context,
    souls_n: u32,
    shards: u32,
    icons: Option<&CurrencyIcons>,
) {
    let (i_souls, i_shards) = icons.map_or((None, None), |i| (i.souls, i.shards));
    egui::Area::new(egui::Id::new("menu_hub_souls"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-24.0, 18.0))
        .show(ctx, |ui| {
            hub_chip_frame().show(ui, |ui| {
                ui.vertical(|ui| {
                    // Largeur réservée : sans elle le chip se dimensionne sur son
                    // contenu, et l'ajout de l'icône a fait passer « 1852 Âmes »
                    // à la ligne (vu à l'inspection). Assez large pour 5 chiffres
                    // + le mot + l'icône.
                    ui.set_min_width(150.0);
                    ui.horizontal(|ui| {
                        CurrencyIcons::show(ui, i_souls, CURRENCY_ICON);
                        ui.label(
                            // FORGE_AME et pas FORGE_OR : l'or est la couleur
                            // des pièces de run — le texte suit le saphir.
                            egui::RichText::new(format!("{souls_n}  Âmes"))
                                .size(20.0)
                                .color(FORGE_AME)
                                .strong(),
                        );
                    });
                    ui.horizontal(|ui| {
                        CurrencyIcons::show(ui, i_shards, CURRENCY_ICON_SMALL);
                        ui.label(
                            // FORGE_ECLAT et pas C_PRIMARY : celui-ci est un
                            // alias de FORGE_OR, les deux monnaies sortaient
                            // dans le même or.
                            egui::RichText::new(format!("{shards}  Éclats"))
                                .size(15.0)
                                .color(FORGE_ECLAT)
                                .strong(),
                        );
                    });
                });
            });
        });
}

/// Chip forgeron (haut-GAUCHE) : nom + niveau + avancement de l'Enclume.
///
/// Il vivait en tête de la sidebar verticale. La barre de navigation est passée
/// à l'horizontale (2026-08-06) et ne peut plus porter trois lignes de texte :
/// l'identité prend donc son propre chip, en miroir du chip Âmes à droite. Le
/// haut de l'écran se lit alors comme un bandeau : QUI je suis · OÙ je vais ·
/// CE QUE j'ai.
fn draw_hub_smith_chip(ctx: &egui::Context, name: &str, level: u32, remaining: u32, frac: f32) {
    egui::Area::new(egui::Id::new("menu_hub_smith"))
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(24.0, 18.0))
        .show(ctx, |ui| {
            hub_chip_frame().show(ui, |ui| {
                ui.set_min_width(180.0);
                ui.label(
                    egui::RichText::new(name)
                        .size(17.0)
                        .color(FORGE_CREME)
                        .strong(),
                );
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("Niveau {level}"))
                            .size(12.0)
                            .color(C_TEXT_MUTED),
                    );
                });
                ui.add_space(3.0);
                // 14 px de haut, pas 8 : la barre porte un texte de 9 px, et
                // une barre plus basse que son propre libellé le coupe en deux
                // (vu à l'inspection du 2026-08-06).
                ui.add_sized(
                    egui::vec2(180.0, 14.0),
                    egui::ProgressBar::new(frac).fill(FORGE_OR).text(
                        egui::RichText::new(if remaining == 0 {
                            "COMPLET".to_string()
                        } else {
                            format!("{remaining} À DÉBLOQUER")
                        })
                        .size(10.0)
                        // Texte SOMBRE : il est posé sur le remplissage doré de
                        // la barre, et le crème d'origine s'y noyait.
                        .color(egui::Color32::from_rgb(40, 26, 10))
                        .strong(),
                    ),
                );
            });
        });
}

/// Barre de navigation HORIZONTALE, centrée en haut. `page` est muté au clic.
///
/// Elle était verticale à gauche et mangeait 228 px de large sur toute la
/// hauteur — dont l'écran de préparation avait besoin pour respirer, et qui
/// obligeait chaque page à se décaler de +100 px pour ne pas passer dessous.
/// À l'horizontale elle coûte [`HUB_TOP_BAR_H`] px de haut, une seule fois, et
/// les pages retrouvent le centre de l'écran.
fn draw_hub_nav(ctx: &egui::Context, page: &mut MenuPage, badges: HubBadges) {
    egui::Area::new(egui::Id::new("menu_hub_nav"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 18.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(FORGE_PANEL)
                .inner_margin(egui::Margin::symmetric(10, 8))
                .corner_radius(egui::CornerRadius::same(14))
                .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                    // Onglets.
                    for tab in MenuPage::NAV {
                        let selected = *page == tab;
                        let resp = ui.add_sized(
                            egui::vec2(tab_width(tab.nav_label()), 34.0),
                            egui::Button::selectable(
                                selected,
                                egui::RichText::new(tab.nav_label())
                                    .size(16.0)
                                    .color(if selected { C_PRIMARY } else { FORGE_CREME })
                                    .strong(),
                            ),
                        );
                        // La sidebar est dessinée avec des `Button::selectable`
                        // bruts, donc hors des helpers de style qui sonnent —
                        // elle était MUETTE au survol (retour en jeu 2026-08-05).
                        // Seul le survol est instrumenté : le clic joue déjà son
                        // `Tab` plus bas.
                        forgia_ui_lib::ui_sfx::instrument_hover(&resp);
                        // Story-678 Phase 4 — pastille dorée : quelque chose
                        // t'attend derrière cet onglet (état réel, jamais décoratif).
                        //
                        // Seule l'Enclume est encore concernée ICI : Livre et
                        // Forgeron ont quitté la sidebar avec la refonte Dicero,
                        // et leurs pastilles vivent désormais sur les blocs de
                        // l'Accueil qui ont repris leur contenu (carrousel du
                        // Livre, bouton « Personnaliser »). Les tester ici était
                        // du code mort — donc deux signaux calculés que rien
                        // n'affichait.
                        let dot = matches!(tab, MenuPage::Enclume) && badges.enclume;
                        if dot {
                            ui.painter().circle_filled(
                                egui::pos2(resp.rect.right() - 10.0, resp.rect.center().y),
                                4.0,
                                C_PRIMARY,
                            );
                        }
                        if resp.clicked() {
                            if *page != tab {
                                forgia_ui_lib::ui_sfx::push_ui_sfx(
                                    &resp.ctx,
                                    forgia_ui_lib::ui_sfx::UiSfxKind::Tab,
                                );
                            }
                            *page = tab;
                        }
                        ui.add_space(3.0);
                    }
                    });
                });
        });
}

/// Largeur d'un onglet de la barre horizontale, dérivée de son libellé.
///
/// Une largeur fixe (les 200 px de la sidebar) donnait une barre de 1 800 px
/// pour neuf onglets — plus large que l'écran. On dimensionne donc au texte,
/// avec un plancher pour que les libellés courts restent cliquables.
fn tab_width(label: &str) -> f32 {
    /// Largeur moyenne d'un caractère de la fonte d'UI à 16 px, mesurée sur les
    /// libellés existants. Approximation assumée : egui recentre le texte, un
    /// écart de quelques pixels ne se voit pas.
    const CHAR_W: f32 = 9.0;
    const PADDING: f32 = 22.0;
    const MIN_W: f32 = 84.0;
    (label.chars().count() as f32 * CHAR_W + PADDING).max(MIN_W)
}

/// Panneau de section centré (verre + liseré or) — chrome commun aux sections
/// data du hub.
///
/// Ancré en HAUT depuis le passage de la nav à l'horizontale : centré
/// verticalement, une page haute remontait sous la barre. Il part donc sous
/// [`hub_content_top`] et descend — la seule ancre qui garantit qu'aucune
/// section ne passe derrière le chrome, quelle que soit sa hauteur.
fn hub_section_panel(
    ctx: &egui::Context,
    id: &'static str,
    title: &str,
    max_width: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    // Story-678 Phase 2 — entrée de section en fondu + glissement (150 ms).
    // La clé = l'id de section : naviguer relance l'anim, re-render non.
    let (opacity, slide) = forgia_ui_lib::motion::section_enter_anim(ctx, id);
    // ── Plafond de hauteur + défilement (audit 2026-08-06, bloquant n°2) ──
    // Une `Area` dont le contenu dépasse l'écran est REMONTÉE par egui : le
    // titre passait par-dessus la barre de navigation, et le panneau invisible
    // MANGEAIT ses clics — depuis Options, la moitié des onglets ne répondait
    // plus. Le contenu défile désormais dans le panneau ; le panneau, lui, ne
    // dépasse jamais.
    // ⚠ viewport_rect, PAS content_rect : le fond vidéo du menu est un
    // CentralPanel qui CONSOMME l'espace du contexte — content_rect mesurait
    // ce qui restait APRÈS lui, et le panneau s'arrêtait à mi-écran (vu le
    // 2026-08-06, « plus adapté à la taille de mon écran »). Les Areas
    // flottent au-dessus des panels : c'est bien le viewport qui fait foi.
    let ecran_h = ctx
        .data(|d| d.get_temp::<f32>(egui::Id::new("forgia_viewport_h")))
        .unwrap_or(1080.0);
    let max_h = (ecran_h - hub_content_top() - 24.0).max(220.0);
    // Réserve du chrome interne : titre (40 px) + respirations + marges du cadre.
    let scroll_h = max_h - 130.0;
    egui::Area::new(egui::Id::new(id))
        .anchor(
            egui::Align2::CENTER_TOP,
            egui::vec2(slide, hub_content_top()),
        )
        .show(ctx, |ui| {
            ui.set_opacity(opacity);
            // 🚨 LA LIGNE QUI MANQUAIT (mesurée le 2026-08-07, sonde ci-dessous).
            //
            // Une `Area` egui ne devine pas la place qu'on veut lui laisser :
            // elle crée son `Ui` avec un `max_rect` de 400 px de haut, quoi
            // qu'on calcule par ailleurs. Mon plafond de 824 n'était donc
            // JAMAIS atteint — trois correctifs successifs (content_rect,
            // viewport_rect, hauteur de fenêtre Bevy) ont réparé un chiffre
            // qui n'a jamais été le limiteur. `set_max_height` assigne le
            // `max_rect` : il ÉLARGIT autant qu'il restreint.
            ui.set_max_height(max_h);
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(40, 30))
                .show(ui, |ui| {
                    ui.set_max_width(max_width);
                    ui.vertical_centered(|ui| {
                        ui.label(display_text(title, 40.0, FORGE_OR).strong());
                    });
                    ui.add_space(16.0);
                    egui::ScrollArea::vertical()
                        .max_height(scroll_h)
                        .show(ui, |ui| {
                            ui.set_max_width(max_width);
                            ui.vertical_centered(|ui| add_contents(ui));
                        });
                });
        });
}

/// Accueil (page racine) : le titre de l'enseigne. Tout le reste (carrousel de
/// chapitre, dernière run, boutons de départ, équipement) est dessiné par
/// `sys_menu_root_dashboard`, et le personnage vit dans le FOND
/// (`arena_backdrop`), plus dans une carte.
///
/// Le titre était centré en haut ; la barre de navigation horizontale occupe
/// désormais cette place. Il passe donc à gauche, en tête de la colonne de
/// préparation qu'il coiffe — et il cesse d'être une bannière posée sur rien.
/// NOUVELLE PARTIE a fusionné avec CONTINUER ; les démos moteur RPG/Cyber
/// restent hors menu (2026-07-22), Hall de Forgia est dans la carte.
fn draw_root_landing(ctx: &egui::Context) -> MenuAction {
    let (opacity, slide) = forgia_ui_lib::motion::section_enter_anim(ctx, "menu_hub_root");
    egui::Area::new(egui::Id::new("menu_hub_root"))
        .anchor(
            egui::Align2::LEFT_TOP,
            egui::vec2(ROOT_COL_X + slide, hub_content_top()),
        )
        .show(ctx, |ui| {
            ui.set_opacity(opacity);
            ui.vertical(|ui| {
                ui.heading(display_text("FORGIA", 64.0, C_PRIMARY).strong());
                ui.label(
                    egui::RichText::new("ROGUELITE")
                        .size(19.0)
                        .color(FORGE_CREME)
                        .strong(),
                );
            });
        });
    MenuAction::None
}

/// Abscisse de la colonne de préparation (carte de chapitre + titre).
///
/// Source unique : le titre, la carte et le panneau d'équipement s'y réfèrent.
/// Le personnage se tient dans le tiers droit du FOND — cette colonne occupe
/// donc la gauche, et rien ne doit venir la chevaucher.
const ROOT_COL_X: f32 = 56.0;

/// Hauteur du bloc-titre, au-dessus de la carte de chapitre.
const ROOT_TITLE_H: f32 = 104.0;

/// Section Stats — synthèse de la méta-progression (records + compteurs de runs).
fn draw_stats_section(ui: &mut egui::Ui, save: Option<&MetaShopSave>, level: u32) {
    let (runs, wins, best, souls, weapons) = save
        .map(|s| {
            (
                s.runs_played,
                s.victories,
                s.best_victory_secs,
                s.souls_total,
                s.unlocked_weapons.len(),
            )
        })
        .unwrap_or((0, 0, 0.0, 0, 1));
    let win_rate = if runs > 0 {
        (wins as f32 / runs as f32 * 100.0).round() as u32
    } else {
        0
    };
    let best_str = if best > 0.0 {
        let m = (best / 60.0).floor() as u32;
        let s = (best % 60.0).floor() as u32;
        format!("{m} min {s:02} s")
    } else {
        "—".to_string()
    };
    stat_row(ui, "Runs jouées", runs.to_string());
    stat_row(ui, "Victoires", format!("{wins}  ({win_rate} %)"));
    stat_row(ui, "Meilleure victoire", best_str);
    stat_row(ui, "Âmes accumulées", souls.to_string());
    stat_row(ui, "Armes débloquées", weapons.to_string());
    stat_row(ui, "Niveau", level.to_string());
}

/// Une ligne « libellé … valeur » de la section Stats (verre + liseré or).
fn stat_row(ui: &mut egui::Ui, label: &str, value: String) {
    egui::Frame::new()
        .fill(FORGE_PANEL)
        .inner_margin(egui::Margin::symmetric(16, 9))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
        .show(ui, |ui| {
            ui.set_min_width(460.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).size(16.0).color(FORGE_CREME));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(value)
                            .size(18.0)
                            .color(FORGE_OR)
                            .strong(),
                    );
                });
            });
        });
    ui.add_space(8.0);
}

/// Page Options — réutilise les contrôles du pause menu (DRY), dans un panneau
/// verre centré. « Retour » ramène à l'accueil.
fn draw_options_page(
    ctx: &egui::Context,
    page: &mut ResMut<MenuPage>,
    settings: &mut ResMut<UserSettings>,
) {
    // Story-678 — chrome commun. Sauvegarde IMMÉDIATE au changement : un
    // réglage qu'on perd en quittant est un réglage cassé.
    let mut retour = false;
    hub_section_panel(
        ctx,
        "hub_sec_options",
        MenuPage::Options.section_title(),
        680.0,
        |ui| {
            if draw_settings_controls(ui, settings) {
                save_user_settings(settings);
            }
            ui.add_space(14.0);
            if glass_btn(ui, "‹  Retour").clicked() {
                retour = true;
            }
        },
    );
    if retour {
        **page = MenuPage::Root;
    }
}

/// Pastilles « quelque chose t'attend » du hub (story-678 Phase 4) — pilotées
/// par l'état RÉEL, jamais décoratives.
///
/// Reconstruites après l'incident de découpe du 2026-08-06 (une édition par
/// script a remplacé un span trop large ; ce fichier n'était pas commité).
#[derive(Resource, Clone, Copy, Default, PartialEq)]
struct HubBadges {
    /// Au moins un achat possible à l'Enclume avec les Âmes courantes.
    enclume: bool,
    /// Une pièce d'armure ramassée depuis la dernière visite de la fiche.
    forgeron: bool,
    /// Un chapitre battu que le Livre n'a pas encore montré.
    livre: bool,
}

/// Calcule les pastilles, et éteint celle de la fiche quand on la VISITE :
/// « vu » = la page a été ouverte. Écritures gardées — pas de churn à vide.
fn sys_hub_badges(
    app_state: Res<State<AppMode>>,
    page: Res<MenuPage>,
    cat: Option<Res<MetaShopCatalogue>>,
    souls: Option<Res<MetaSouls>>,
    meta: Option<Res<MetaShopSave>>,
    mut eq_save: Option<ResMut<EquipmentSave>>,
    mut badges: ResMut<HubBadges>,
) {
    if *app_state.get() != AppMode::Menu {
        return;
    }
    if let Some(eq) = eq_save.as_deref_mut() {
        if matches!(*page, MenuPage::Forgeron | MenuPage::Sac) {
            let total = eq.owned_total();
            if eq.seen_owned_total != total {
                eq.seen_owned_total = total;
                // Sans écriture disque, le marquage ne survit pas à la session :
                // la pastille éteinte se rallume au lancement suivant. Écriture
                // rare — une fois par visite qui découvre de nouvelles pièces.
                eq.save();
            }
        }
    }
    let next = HubBadges {
        enclume: match (cat.as_deref(), meta.as_deref(), souls.as_deref()) {
            (Some(c), Some(m), Some(s)) => {
                forgia_mode_roguelite::meta_shop::enclume_affordable(c, m, s.current)
            }
            _ => false,
        },
        forgeron: eq_save
            .as_deref()
            .map(|e| e.owned_total() > e.seen_owned_total)
            .unwrap_or(false),
        livre: meta
            .as_deref()
            .map(|m| m.chapters_cleared > m.seen_chapters_cleared)
            .unwrap_or(false),
    };
    if *badges != next {
        *badges = next;
    }
}

/// Story-678 Phase 3 — le tableau de bord de l'accueil : carte « CHAPITRE EN
/// COURS » (carrousel du Livre), bandeau « DERNIÈRE RUN » (gelé par
/// `sys_record_run_stats` aux deux sorties de run), boutons de départ, et le
/// panneau d'équipement (bas-droite).
///
/// Système à part (même motif que `sys_menu_livre`) : `main_menu_ui` frôle le
/// plafond de 16 paramètres Bevy. Toutes les Resources roguelite sont
/// optionnelles — l'accueil s'affiche même sans elles.
#[allow(clippy::too_many_arguments)]
fn sys_menu_root_dashboard(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut page: ResMut<MenuPage>,
    // `ResMut` : actionner le carrousel vaut « j'ai vu les nouveaux chapitres »
    // et éteint la pastille du Livre (cf. `sys_hub_badges`).
    save: Option<ResMut<MetaShopSave>>,
    mut selected: Option<ResMut<SelectedChapter>>,
    palettes: Option<Res<DecorPalettesConfig>>,
    rounds_cfg: Option<Res<RoundsConfig>>,
    equip_cfg: Option<Res<EquipmentConfig>>,
    equip_save: Option<Res<EquipmentSave>>,
    badges: Res<HubBadges>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut exit: MessageWriter<AppExit>,
) {
    if *app_state.get() != AppMode::Menu || *page != MenuPage::Root {
        return;
    }
    let Some(mut save) = save else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let chapitre = selected
        .as_deref()
        .map(|s| s.clamped(save.chapters_cleared))
        .unwrap_or(1);
    let da = chapter_da_name(palettes.as_deref(), chapitre);
    let menace = rounds_cfg
        .map(|c| c.threat_at(chapitre, 0).hp)
        .unwrap_or(1.0);

    // Sorties de l'UI, appliquées APRÈS les closures (emprunts propres).
    let mut new_chap = chapitre;
    let mut action = MenuAction::None;
    let mut goto_forgeron = false;
    let mut goto_livre = false;
    let mut carrousel_actionne = false;

    let (opacity, slide) = forgia_ui_lib::motion::section_enter_anim(ctx, "menu_hub_root");

    // ── Colonne GAUCHE : LE LIVRE (carrousel) + dernière run + départ ──
    // Sous le bloc-titre, alignée sur la même abscisse : l'écran se lit en deux
    // temps — ce que je prépare à gauche, qui je suis à droite (dans le décor).
    egui::Area::new(egui::Id::new("menu_hub_root_dashboard"))
        .anchor(
            egui::Align2::LEFT_TOP,
            egui::vec2(ROOT_COL_X + slide, hub_content_top() + ROOT_TITLE_H),
        )
        .show(ctx, |ui| {
            ui.set_opacity(opacity);
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(28, 22))
                .show(ui, |ui| {
                    ui.set_width(340.0);
                    ui.vertical_centered(|ui| {
                        // Pastille « un chapitre s'est ouvert » — posée sur le
                        // titre du bloc, qui est aussi la PORTE de la page Livre
                        // complète (même modèle que « Personnaliser » → Forgeron :
                        // hors barre de nav, accessible depuis le bloc de
                        // l'Accueil qui a repris son contenu — sans ce clic, la
                        // vue d'ensemble des chapitres était orpheline).
                        let titre = ui
                            .add(
                                egui::Label::new(display_text("LE LIVRE", 18.0, FORGE_OR))
                                    .sense(egui::Sense::click()),
                            )
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text("Ouvrir le Livre — tous les chapitres");
                        forgia_ui_lib::ui_sfx::instrument_hover(&titre);
                        if titre.clicked() {
                            goto_livre = true;
                        }
                        if badges.livre {
                            ui.painter().circle_filled(
                                egui::pos2(titre.rect.right() + 12.0, titre.rect.center().y),
                                5.0,
                                C_PRIMARY,
                            );
                        }
                        ui.add_space(6.0);
                        // Carrousel ‹ CHAPITRE N › — borné aux chapitres ouverts.
                        ui.horizontal(|ui| {
                            let w = ui.available_width();
                            let side = 36.0;
                            let prev_ok = chapitre > 1;
                            let next_ok =
                                chapter_unlocked(chapitre + 1, save.chapters_cleared);
                            ui.add_space((w - 340.0).max(0.0) / 2.0);
                            let prev = ui.add_enabled(
                                prev_ok,
                                egui::Button::new(
                                    egui::RichText::new("<").size(24.0).color(FORGE_OR),
                                )
                                .fill(FORGE_PANEL)
                                .min_size(egui::vec2(side, side)),
                            );
                            ui.add_space(8.0);
                            ui.add_sized(
                                egui::vec2(220.0, side),
                                egui::Label::new(
                                    display_text(format!("CHAPITRE {chapitre}"), 28.0, FORGE_OR)
                                        .strong(),
                                ),
                            );
                            ui.add_space(8.0);
                            let next = ui.add_enabled(
                                next_ok,
                                egui::Button::new(
                                    egui::RichText::new(">").size(24.0).color(FORGE_OR),
                                )
                                .fill(FORGE_PANEL)
                                .min_size(egui::vec2(side, side)),
                            );
                            if prev.clicked() {
                                new_chap = chapitre - 1;
                            }
                            if next.clicked() {
                                new_chap = chapitre + 1;
                            }
                            // Feuilleter le Livre, c'est l'avoir consulté : la
                            // pastille « nouveau chapitre » s'éteint ici, et
                            // nulle part ailleurs sur cet écran (cf. le pourquoi
                            // dans `sys_hub_badges`).
                            if prev.clicked() || next.clicked() {
                                carrousel_actionne = true;
                            }
                        });
                        if !da.is_empty() {
                            ui.label(egui::RichText::new(da).size(15.0).color(FORGE_CREME));
                        }
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("Menace ×{menace:.2}"))
                                .size(13.0)
                                .color(C_TEXT_MUTED),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{} / {} chapitres battus",
                                save.chapters_cleared, CHAPTERS_PER_BOOK
                            ))
                            .size(13.0)
                            .color(C_TEXT_MUTED),
                        );
                        // ── Bandeau DERNIÈRE RUN ──
                        if let Some(run) = &save.last_run {
                            ui.add_space(12.0);
                            ui.separator();
                            ui.add_space(6.0);
                            ui.label(display_text("DERNIÈRE RUN", 16.0, FORGE_OR));
                            let (verdict, couleur) = if run.victory {
                                ("Victoire", FORGE_OR)
                            } else {
                                ("Défaite", C_TEXT_MUTED)
                            };
                            let mins = (run.duration_secs / 60.0) as u32;
                            let secs = (run.duration_secs % 60.0) as u32;
                            ui.label(
                                egui::RichText::new(format!(
                                    "Chapitre {} — {verdict} · round {} · {mins}:{secs:02}",
                                    run.chapter, run.rounds_reached
                                ))
                                .size(13.0)
                                .color(couleur),
                            );
                            ui.label(
                                egui::RichText::new(format!(
                                    "+{} Âmes rapportées",
                                    run.souls_earned
                                ))
                                .size(13.0)
                                .color(FORGE_CREME),
                            );
                            if run.new_best {
                                ui.label(
                                    egui::RichText::new("NOUVEAU RECORD !")
                                        .size(13.0)
                                        .color(FORGE_OR)
                                        .strong(),
                                );
                            }
                        }
                        // ── Boutons de départ (tout sur l'écran, modèle Dicero) ──
                        ui.add_space(14.0);
                        ui.separator();
                        ui.add_space(10.0);
                        if cartoon_btn(ui, "▶  CONTINUER", FORGE_OR).clicked() {
                            action = MenuAction::Launch(GameMode::Roguelite);
                        }
                        ui.add_space(8.0);
                        if glass_btn(ui, "🏰  Hall de Forgia").clicked() {
                            action = MenuAction::Launch(GameMode::CastleHub);
                        }
                        ui.add_space(8.0);
                        if glass_btn(ui, "QUITTER").clicked() {
                            action = MenuAction::Quit;
                        }
                    });
                });
        });

    // ── Colonne DROITE : ce que porte le personnage ──
    //
    // Le portrait en carte a disparu : le personnage est DANS le décor du fond,
    // à droite de l'écran (`arena_backdrop`). Ne reste ici que ce qu'une image
    // ne dit pas : le score de puissance et la liste des pièces avec leur
    // rareté. Ancré en BAS à droite pour laisser le buste dégagé au-dessus.
    egui::Area::new(egui::Id::new("menu_hub_root_gear"))
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-56.0 - slide, -48.0))
        .show(ctx, |ui| {
            ui.set_opacity(opacity);
            glass_frame_hero()
                .inner_margin(egui::Margin::symmetric(22, 16))
                .show(ui, |ui| {
                    ui.set_width(290.0);
                    ui.vertical_centered(|ui| {
                        if let (Some(cfg), Some(esave)) =
                            (equip_cfg.as_deref(), equip_save.as_deref())
                        {
                            let score = power_score(cfg, esave);
                            ui.label(
                                egui::RichText::new(format!(
                                    "Puissance {score}  ·  record {}",
                                    esave.power_record
                                ))
                                .size(14.0)
                                .color(FORGE_CREME),
                            );
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(6.0);
                            // Pièces portées : pastille couleur rareté + emplacement.
                            for slot in &cfg.slots {
                                let equipped = esave.equipped.get(&slot.id);
                                let (col, texte) = match equipped {
                                    Some(rid) => {
                                        let rgb =
                                            cfg.rarity(rid).map(|r| r.rgb).unwrap_or([0.5; 3]);
                                        (
                                            egui::Color32::from_rgb(
                                                (rgb[0] * 255.0) as u8,
                                                (rgb[1] * 255.0) as u8,
                                                (rgb[2] * 255.0) as u8,
                                            ),
                                            format!(
                                                "{} — {}",
                                                slot.label,
                                                cfg.rarity(rid)
                                                    .map(|r| r.label.as_str())
                                                    .unwrap_or(rid)
                                            ),
                                        )
                                    }
                                    None => (
                                        egui::Color32::from_gray(90),
                                        format!("{} — vide", slot.label),
                                    ),
                                };
                                ui.horizontal(|ui| {
                                    let (dot, _) = ui.allocate_exact_size(
                                        egui::vec2(14.0, 14.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().circle_filled(dot.center(), 5.0, col);
                                    ui.label(
                                        egui::RichText::new(texte).size(13.0).color(FORGE_CREME),
                                    );
                                });
                            }
                        }
                        ui.add_space(10.0);
                        // Pastille « une pièce que tu n'as jamais regardée » —
                        // sur le bouton qui mène à la fiche.
                        let perso = glass_btn(ui, "⚒  Personnaliser");
                        if badges.forgeron {
                            ui.painter().circle_filled(
                                egui::pos2(perso.rect.right() - 12.0, perso.rect.center().y),
                                5.0,
                                C_PRIMARY,
                            );
                        }
                        if perso.clicked() {
                            goto_forgeron = true;
                        }
                    });
                });
        });

    // ── Application des sorties ──
    if new_chap != chapitre {
        if let Some(sel) = selected.as_deref_mut() {
            sel.0 = new_chap;
        }
    }
    if carrousel_actionne && save.seen_chapters_cleared != save.chapters_cleared {
        save.seen_chapters_cleared = save.chapters_cleared;
        save.save();
    }
    if goto_forgeron {
        *page = MenuPage::Forgeron;
    }
    if goto_livre {
        *page = MenuPage::Livre;
    }
    match action {
        MenuAction::Launch(mode) => {
            next_game.set(mode);
            next_app.set(AppMode::InGame);
        }
        MenuAction::Quit => {
            exit.write(AppExit::Success);
        }
        MenuAction::None => {}
    }
}

/// Le LIVRE en page pleine — la même `chapter_select_content` que le carrousel
/// de l'accueil, pour la vue d'ensemble des dix chapitres.
fn sys_menu_livre(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut page: ResMut<MenuPage>,
    save: Option<Res<MetaShopSave>>,
    palettes: Option<Res<DecorPalettesConfig>>,
    selected: Option<ResMut<SelectedChapter>>,
) {
    if *app_state.get() != AppMode::Menu || *page != MenuPage::Livre {
        return;
    }
    let (Some(save), Some(mut selected)) = (save, selected) else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let mut retour = false;
    hub_section_panel(
        ctx,
        "hub_sec_livre",
        MenuPage::Livre.section_title(),
        640.0,
        |ui| {
            chapter_select_content(ui, &save, palettes.as_deref(), &mut selected);
            ui.add_space(12.0);
            if glass_btn(ui, "‹  Retour").clicked() {
                retour = true;
            }
        },
    );
    if retour {
        *page = MenuPage::Root;
    }
}

/// L'ENCLUME au menu — les cartes cliquables de `draw_enclume_panel`, l'achat
/// appliqué par `apply_meta_purchase` (une seule règle de dépense).
fn sys_menu_enclume(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    page: Res<MenuPage>,
    cat: Option<Res<MetaShopCatalogue>>,
    save: Option<ResMut<MetaShopSave>>,
    meta: Option<ResMut<MetaSouls>>,
) {
    if *app_state.get() != AppMode::Menu || *page != MenuPage::Enclume {
        return;
    }
    let (Some(cat), Some(mut save), Some(mut meta)) = (cat, save, meta) else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    let souls = meta.current;
    let mut purchase = None;
    hub_section_panel(
        ctx,
        "hub_sec_enclume",
        MenuPage::Enclume.section_title(),
        640.0,
        |ui| {
            ui.label(
                egui::RichText::new("Dépense tes Âmes en améliorations permanentes.")
                    .size(16.0)
                    .color(FORGE_CREME),
            );
            ui.add_space(10.0);
            purchase = draw_enclume_panel(ui, &cat, &save, souls);
        },
    );
    if let Some(p) = purchase {
        let ok = apply_meta_purchase(&cat, &mut save, &mut meta, p);
        forgia_ui_lib::ui_sfx::push_ui_sfx(
            ctx,
            if ok {
                forgia_ui_lib::ui_sfx::UiSfxKind::Buy
            } else {
                forgia_ui_lib::ui_sfx::UiSfxKind::Denied
            },
        );
    }
}

/// LA FICHE — le personnage encadré par ses emplacements, le sac en grille à
/// côté, les caractéristiques dessous (structure de la référence « Classic RPG
/// UI », dans notre DA). « Sac » et « Forgeron » ouvrent ce MÊME écran : la
/// fiche et l'inventaire se regardent, les séparer obligerait à mémoriser ce
/// qu'on porte en changeant d'onglet.
#[allow(clippy::too_many_arguments)]
fn sys_menu_forgeron(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut page: ResMut<MenuPage>,
    cfg: Res<IdentityConfig>,
    mut save: ResMut<IdentitySave>,
    mut arm_cosmetics: ResMut<ArmCosmetics>,
    mut editing: Local<bool>,
    eq_cfg: Res<EquipmentConfig>,
    mut eq_save: ResMut<EquipmentSave>,
    // Le bloc CARACTÉRISTIQUES lit les bonus agrégés du build.
    eq_mods: Res<EquipmentMods>,
    // La case choisie d'un simple clic (emplacement, rareté) — le double-clic
    // équipe. `Local` : c'est un état d'écran, rien à faire en Resource.
    mut selection: Local<Option<(String, String)>>,
    mut eq_shown: ResMut<EquipmentPanelShown>,
    rtt: Option<Res<weapon_preview::CharacterPreviewRtt>>,
) {
    if *app_state.get() != AppMode::Menu
        || !matches!(*page, MenuPage::Forgeron | MenuPage::Sac)
    {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    // Posé seulement une fois le panneau RÉELLEMENT dessiné : le capteur ne doit
    // pas annoncer « affiché » pour quelque chose d'invisible.
    eq_shown.0 = true;

    let character = rtt.as_ref().map(|r| r.tex_id);
    // Repli quand le rendu hors écran n'a pas encore spawné : la pastille de la
    // couleur portée, pour que la place ne soit jamais vide.
    let rgb = cfg
        .colors
        .iter()
        .find(|c| c.id == save.equipped_color)
        .map(|c| c.rgb)
        .unwrap_or([0.6, 0.6, 0.6]);
    let disc_col = egui::Color32::from_rgb(
        (rgb[0] * 255.0) as u8,
        (rgb[1] * 255.0) as u8,
        (rgb[2] * 255.0) as u8,
    );

    // Story-678 — chrome commun (titre/marges/transition standard). Le titre
    // suit l'onglet d'entrée : même écran, mais « TON SAC » quand on vient
    // pour l'inventaire.
    let titre = if *page == MenuPage::Sac {
        MenuPage::Sac.section_title()
    } else {
        MenuPage::Forgeron.section_title()
    };
    let mut back = false;
    let mut goto_decors = false;
    // Clic sur un emplacement de la poupée → SÉLECTIONNE sa pièce dans le sac
    // (surbrillance or). Même écran, donc pas de navigation : on guide l'œil.
    let mut choisir_slot: Option<String> = None;
    let mut pose: Option<(String, Option<String>)> = None;
    hub_section_panel(ctx, "hub_sec_forgeron", titre, 1000.0, |ui| {
        // ── La fiche, structurée d'après la référence « Classic RPG UI » :
        // le personnage ENCADRÉ par ses cellules d'équipement, l'inventaire en
        // grille à côté, les caractéristiques dessous. Chaque colonne a une
        // largeur FIXE : un `vertical_centered` lâché dans l'horizontal
        // réclamait toute la largeur restante et poussait le sac hors de
        // l'écran, écrasé en colonne de lettres (audit 2026-08-06, bloquant n°1).
        ui.horizontal_top(|ui| {
            // ── Haut du corps, collé au personnage ──
            ui.vertical(|ui| {
                ui.set_min_width(PAPERDOLL_COL_W);
                ui.set_max_width(PAPERDOLL_COL_W);
                for (i, slot) in eq_cfg.slots.iter().enumerate() {
                    if i % 2 != 0 {
                        continue;
                    }
                    if paperdoll_slot(ui, &eq_cfg, &eq_save, slot) {
                        choisir_slot = Some(slot.id.clone());
                    }
                }
            });
            ui.add_space(6.0);
            // ── Le personnage ──
            ui.vertical(|ui| {
                ui.set_min_width(280.0);
                ui.set_max_width(280.0);
                match character {
                    Some(tex) => {
                        ui.add(egui::Image::new(egui::load::SizedTexture::new(
                            tex,
                            egui::vec2(280.0, 340.0),
                        )));
                    }
                    None => {
                        let (r, _) = ui.allocate_exact_size(
                            egui::vec2(280.0, 340.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().circle_filled(r.center(), 60.0, disc_col);
                        ui.painter().circle_stroke(
                            r.center(),
                            60.0,
                            egui::Stroke::new(2.5, HAIR_GOLD_STRONG),
                        );
                    }
                }
                ui.vertical_centered(|ui| {
                    let p = power_score(&eq_cfg, &eq_save);
                    ui.label(display_text(format!("Puissance {p}"), 22.0, FORGE_OR).strong());
                    if eq_save.power_record > p {
                        ui.label(
                            egui::RichText::new(format!("record {}", eq_save.power_record))
                                .size(12.0)
                                .color(C_TEXT_MUTED),
                        );
                    }
                });
            });
            ui.add_space(6.0);
            // ── Bas du corps ──
            ui.vertical(|ui| {
                ui.set_min_width(PAPERDOLL_COL_W);
                ui.set_max_width(PAPERDOLL_COL_W);
                for (i, slot) in eq_cfg.slots.iter().enumerate() {
                    if i % 2 == 0 {
                        continue;
                    }
                    if paperdoll_slot(ui, &eq_cfg, &eq_save, slot) {
                        choisir_slot = Some(slot.id.clone());
                    }
                }
            });
            ui.add_space(12.0);
            ui.separator();
            ui.add_space(12.0);
            // ── LE SAC, à côté de la fiche ──
            if let Some(a) = draw_sac(ui, &eq_cfg, &eq_save, &mut selection) {
                pose = Some(a);
            }
        });

        // ── CARACTÉRISTIQUES — le bloc « Characteristic » de la référence ──
        // La seule vue qui dit le PROFIL du build (où portent les bonus), là où
        // la Puissance n'en dit que la somme.
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let stats: [(&str, f32); 5] = [
                ("Dégâts", (eq_mods.damage_mul - 1.0) * 100.0),
                ("Cadence", (eq_mods.fire_rate_mul - 1.0) * 100.0),
                ("Blindage", eq_mods.damage_reduction * 100.0),
                ("Critique", eq_mods.crit_chance * 100.0),
                ("Visée", (eq_mods.headshot_bonus_mul - 1.0) * 100.0),
            ];
            for (nom, valeur) in stats {
                ui.vertical(|ui| {
                    ui.set_min_width(120.0);
                    ui.label(egui::RichText::new(nom).size(11.0).color(C_TEXT_MUTED));
                    let txt = egui::RichText::new(format!("+{valeur:.0}%")).size(16.0).strong();
                    ui.label(if valeur > 0.05 {
                        txt.color(FORGE_OR)
                    } else {
                        txt.color(C_TEXT_MUTED)
                    });
                });
            }
        });

        // ── Qui il est : nom, couleur, bras ──
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(8.0);
        draw_identity_content(ui, &cfg, &mut save, &mut arm_cosmetics, &mut editing);
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if glass_btn(ui, "‹  Retour").clicked() {
                back = true;
            }
            // Le Marketplace est cosmétique : sa porte est sur la page
            // « qui je suis », en plus de la barre de navigation.
            if glass_btn(ui, "💰  Marketplace").clicked() {
                goto_decors = true;
            }
        });
    });
    if back {
        *page = MenuPage::Root;
    }
    if goto_decors {
        *page = MenuPage::Marketplace;
    }
    if let Some(slot_id) = choisir_slot {
        // La pièce montrée en priorité : celle qu'on porte, sinon la meilleure
        // possédée, sinon la première de l'échelle — jamais rien.
        let rarity = eq_save
            .equipped
            .get(&slot_id)
            .cloned()
            .or_else(|| {
                eq_save.owned.get(&slot_id).and_then(|v| {
                    v.iter()
                        .max_by_key(|r| eq_cfg.rarity_rank(r))
                        .cloned()
                })
            })
            .or_else(|| eq_cfg.rarities.first().map(|r| r.id.clone()));
        if let Some(r) = rarity {
            *selection = Some((slot_id, r));
        }
    }
    if let Some((slot_id, rarity)) = pose {
        match rarity {
            Some(r) => {
                eq_save.equipped.insert(slot_id, r);
                forgia_ui_lib::ui_sfx::push_ui_sfx(ctx, forgia_ui_lib::ui_sfx::UiSfxKind::Buy);
            }
            None => {
                eq_save.equipped.remove(&slot_id);
                forgia_ui_lib::ui_sfx::push_ui_sfx(ctx, forgia_ui_lib::ui_sfx::UiSfxKind::Tab);
            }
        }
        // 🚨 PERSISTER ICI, et NE PAS toucher `power_record`.
        //
        // Confirmé par l'audit du 2026-08-07 : `sys_track_power_record` est le
        // seul écrivain du disque pour cette sauvegarde, et sa condition est
        // « score > power_record ». En rehaussant le record moi-même juste
        // avant, je DÉSARMAIS son déclencheur : équiper au menu n'écrivait
        // jamais rien, et le choix était perdu au relancement. Équiper une
        // pièce MOINS bonne ne déclenchait rien non plus.
        //
        // Le suivi du record reste sa responsabilité — mutation + écriture
        // explicites ici, calcul du pic là-bas. Même classe que le défaut
        // « achat sans effet » côté Âmes : un garde d'autosave qui ne pousse
        // que les gains ne persiste pas les changements.
        eq_save.save();
    }
}

/// Colonnes du Marketplace — fixes, pour que les lignes s'alignent.
const DECOR_NAME_W: f32 = 290.0;
const DECOR_ACTION_W: f32 = 210.0;
const DECOR_ROW_H: f32 = 28.0;

/// Le MARKETPLACE — tout ce qui se porte et ne se joue pas (story-678).
///
/// Quatre familles sous un même toit : décors du menu, couleur du forgeron,
/// bras (ceux qu'on voit à chaque tir), musique du hub. Un onglet par famille,
/// une seule règle de possession (`cosmetics::OwnedCosmetics`).
///
/// ## Pourquoi ce n'est pas l'Enclume
///
/// L'Enclume vend de la PUISSANCE, en Âmes. Mêler du cosmétique à ses rangs
/// forcerait un arbitrage « je tape plus fort » contre « c'est plus joli » dans
/// la même liste — le pire endroit pour ce choix. Le Marketplace a donc sa
/// propre monnaie, les **Éclats**, gagnés à la profondeur atteinte.
///
/// ## Ce qui est possédé
///
/// Chaque famille est possédée dans le stock qui lui sert déjà ailleurs (les
/// couleurs dans `unlocked_colors`, que le panneau Forgeron filtre et que le
/// boot fait respecter). Le catalogue le sait ; cette page ne fait que
/// l'afficher.
fn sys_menu_marketplace(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut page: ResMut<MenuPage>,
    mut tab: Local<usize>,
    catalogue: Option<Res<forgia_mode_roguelite::cosmetics::CosmeticsConfig>>,
    mut identity: Option<ResMut<IdentitySave>>,
    mut meta_save: Option<ResMut<MetaShopSave>>,
    icons: Option<Res<CurrencyIcons>>,
) {
    use forgia_mode_roguelite::cosmetics::{self, CosmeticKind, CosmeticSource, OwnedCosmetics};

    if *app_state.get() != AppMode::Menu || *page != MenuPage::Marketplace {
        return;
    }
    let (Some(catalogue), Some(identity), Some(save)) =
        (catalogue.as_deref(), identity.as_mut(), meta_save.as_mut())
    else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let famille = CosmeticKind::ALL[(*tab).min(CosmeticKind::ALL.len() - 1)];
    let eclats = save.shards_total;
    let icone_eclat = icons.as_deref().and_then(|i| i.shards);

    // Instantané de la famille courante : (id, libellé, possédé, porté, prix,
    // chapitre requis). Pris AVANT de dessiner pour libérer l'emprunt sur
    // `identity`, que les actions muteront après la closure.
    let lignes: Vec<(String, String, bool, bool, Option<u32>, u32)> = {
        let owned = OwnedCosmetics {
            chapters_cleared: save.chapters_cleared,
            identity,
        };
        catalogue
            .of_kind(famille, &owned)
            .into_iter()
            .map(|(c, possede)| {
                let chapitre = match c.source {
                    CosmeticSource::Chapter(n) => n,
                    _ => 0,
                };
                (
                    c.id.clone(),
                    c.label.clone(),
                    possede,
                    cosmetics::is_equipped(identity, c),
                    c.source.price(),
                    chapitre,
                )
            })
            .collect()
    };
    let possedes = lignes.iter().filter(|l| l.2).count();

    let mut equiper: Option<String> = None;
    let mut acheter: Option<(String, u32)> = None;
    let mut retour = false;
    let mut nouvel_onglet: Option<usize> = None;

    hub_section_panel(
        ctx,
        "hub_sec_marketplace",
        MenuPage::Marketplace.section_title(),
        820.0,
        |ui| {
            // Bourse — dite en tête, parce que c'est ce qui décide de ce qu'on
            // peut faire sur cette page.
            ui.horizontal(|ui| {
                CurrencyIcons::show(ui, icone_eclat, CURRENCY_ICON);
                ui.label(
                    egui::RichText::new(format!("{eclats}  Éclats"))
                        .size(22.0)
                        .color(FORGE_ECLAT)
                        .strong(),
                );
            });
            ui.label(
                egui::RichText::new("Gagnés en descendant loin dans un chapitre.")
                    .size(13.0)
                    .color(C_TEXT_MUTED),
            );
            ui.add_space(12.0);

            // Onglets de famille.
            ui.horizontal(|ui| {
                for (i, k) in CosmeticKind::ALL.iter().enumerate() {
                    let actif = *k == famille;
                    let resp = ui.add_sized(
                        egui::vec2(150.0, 30.0),
                        egui::Button::selectable(
                            actif,
                            egui::RichText::new(k.tab_label())
                                .size(15.0)
                                .color(if actif { C_PRIMARY } else { FORGE_CREME })
                                .strong(),
                        ),
                    );
                    forgia_ui_lib::ui_sfx::instrument_hover(&resp);
                    if resp.clicked() && !actif {
                        nouvel_onglet = Some(i);
                        forgia_ui_lib::ui_sfx::push_ui_sfx(
                            &resp.ctx,
                            forgia_ui_lib::ui_sfx::UiSfxKind::Tab,
                        );
                    }
                }
            });
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(famille.tab_help())
                    .size(14.0)
                    .color(FORGE_CREME),
            );
            ui.label(
                egui::RichText::new(format!("{possedes} / {} débloqués", lignes.len()))
                    .size(13.0)
                    .color(C_TEXT_MUTED),
            );
            ui.add_space(12.0);

            for (id, label, possede, porte, prix, chapitre) in &lignes {
                ui.horizontal(|ui| {
                    // Pastille d'état : porté (or plein), possédé (or creux),
                    // verrouillé (gris). Se lit sans lire le texte.
                    let (dot, _) =
                        ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                    let centre = dot.center();
                    if *porte {
                        ui.painter().circle_filled(centre, 6.0, FORGE_OR);
                    } else if *possede {
                        ui.painter()
                            .circle_stroke(centre, 6.0, egui::Stroke::new(2.0, FORGE_OR));
                    } else {
                        ui.painter()
                            .circle_filled(centre, 5.0, egui::Color32::from_gray(80));
                    }

                    ui.add_sized(
                        egui::vec2(DECOR_NAME_W, DECOR_ROW_H),
                        egui::Label::new(
                            egui::RichText::new(label)
                                .size(16.0)
                                .color(if *possede { FORGE_CREME } else { C_TEXT_MUTED }),
                        )
                        .halign(egui::Align::LEFT),
                    );

                    if *porte {
                        ui.add_sized(
                            egui::vec2(DECOR_ACTION_W, DECOR_ROW_H),
                            egui::Label::new(
                                egui::RichText::new("PORTÉ")
                                    .size(13.0)
                                    .color(FORGE_OR)
                                    .strong(),
                            ),
                        );
                    } else if *possede {
                        let resp = ui.add_sized(
                            egui::vec2(DECOR_ACTION_W, DECOR_ROW_H),
                            egui::Button::new(
                                egui::RichText::new("Porter")
                                    .size(14.0)
                                    .color(FORGE_OR)
                                    .strong(),
                            )
                            .fill(FORGE_PANEL),
                        );
                        forgia_ui_lib::ui_sfx::instrument_hover(&resp);
                        if resp.clicked() {
                            equiper = Some(id.clone());
                        }
                    } else if let Some(p) = prix {
                        // Verrouillé mais achetable. Le bouton est DÉSACTIVÉ,
                        // pas caché, quand la bourse ne suit pas — « trop cher »
                        // et « inerte » ne doivent jamais se ressembler.
                        let assez = eclats >= *p;
                        let resp = ui
                            .add_enabled_ui(assez, |ui| {
                                ui.add_sized(
                                    egui::vec2(DECOR_ACTION_W, DECOR_ROW_H),
                                    egui::Button::new(
                                        egui::RichText::new(format!("Débloquer — {p}"))
                                            .size(14.0)
                                            .color(if assez { FORGE_ECLAT } else { C_TEXT_MUTED }),
                                    )
                                    .fill(FORGE_PANEL),
                                )
                            })
                            .inner;
                        forgia_ui_lib::ui_sfx::instrument_hover(&resp);
                        if resp.clicked() {
                            acheter = Some((id.clone(), *p));
                        }
                        if !assez {
                            ui.label(
                                egui::RichText::new(format!("il te manque {}", p - eclats))
                                    .size(12.0)
                                    .color(C_TEXT_MUTED),
                            );
                        }
                    } else {
                        let texte = if *chapitre > 0 {
                            format!("Bats le chapitre {chapitre}")
                        } else {
                            "Haut fait".to_string()
                        };
                        ui.add_sized(
                            egui::vec2(DECOR_ACTION_W, DECOR_ROW_H),
                            egui::Label::new(
                                egui::RichText::new(texte).size(13.0).color(C_TEXT_MUTED),
                            ),
                        );
                    }
                });
                ui.add_space(5.0);
            }

            ui.add_space(10.0);
            if glass_btn(ui, "‹  Retour").clicked() {
                retour = true;
            }
        },
    );

    if let Some(i) = nouvel_onglet {
        *tab = i;
    }
    if let Some(id) = equiper {
        if let Some(c) = catalogue.get(&id) {
            cosmetics::equip(identity, c);
        }
    }
    if let Some((id, prix)) = acheter {
        // Ordre voulu : possession → débit → déblocage. Débloquer avant de
        // débiter obligerait à défaire un déblocage déjà écrit sur le disque si
        // la bourse ne suivait pas.
        //
        // Les Éclats n'ont PAS de miroir vif (contrairement aux Âmes) : une
        // seule vérité, `shards_total`. C'est le miroir des Âmes qui avait
        // produit le défaut « achat sans effet ».
        let paye = match catalogue.get(&id) {
            Some(c)
                if save.shards_total >= prix && cosmetics::grant_and_equip(identity, c) =>
            {
                save.shards_total -= prix;
                save.save();
                true
            }
            _ => false,
        };
        let son = if paye {
            forgia_ui_lib::ui_sfx::UiSfxKind::Unlock
        } else {
            forgia_ui_lib::ui_sfx::UiSfxKind::Denied
        };
        forgia_ui_lib::ui_sfx::push_ui_sfx(ctx, son);
    }
    if retour {
        *page = MenuPage::Forgeron;
    }
}


/// Largeur d'une colonne d'emplacements de la poupée.
const PAPERDOLL_COL_W: f32 = 96.0;
/// Côté du cadre d'un emplacement.
const PAPERDOLL_SLOT: f32 = 64.0;

/// Un emplacement de la poupée : la cellule (silhouette du type à la couleur de
/// la rareté portée), son nom dessous. Rend `true` au clic — l'appelant met la
/// pièce en surbrillance dans le sac.
///
/// Cellule VERTICALE compacte, comme la référence « Classic RPG UI » : les
/// colonnes se collent au personnage au lieu d'étaler un libellé à côté.
fn paperdoll_slot(
    ui: &mut egui::Ui,
    cfg: &EquipmentConfig,
    save: &EquipmentSave,
    slot: &forgia_mode_roguelite::equipment::SlotDef,
) -> bool {
    let porte = save.equipped.get(&slot.id);
    let col = porte
        .map(|id| cfg.color32(id))
        .unwrap_or(egui::Color32::from_gray(70));
    let mut clique = false;

    ui.vertical_centered(|ui| {
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(PAPERDOLL_SLOT, PAPERDOLL_SLOT),
            egui::Sense::click(),
        );
        let p = ui.painter();
        p.rect_filled(
            rect,
            egui::CornerRadius::same(10),
            egui::Color32::from_black_alpha(120),
        );
        // La SILHOUETTE du type, à la couleur de la rareté portée. Un
        // emplacement vide la garde, éteinte — la place se lit même libre.
        slot_glyph::draw(
            p,
            rect.shrink(10.0),
            &slot.id,
            if porte.is_some() {
                col
            } else {
                col.gamma_multiply(0.35)
            },
        );
        p.rect_stroke(
            rect,
            egui::CornerRadius::same(10),
            egui::Stroke::new(if resp.hovered() { 2.5 } else { 1.5 }, col),
            egui::StrokeKind::Inside,
        );
        let bulle = match porte {
            Some(id) => format!(
                "{} — {}
{} +{:.0}%

Clic : voir dans le sac",
                slot.label,
                cfg.rarity(id).map(|r| r.label.as_str()).unwrap_or(id),
                slot.stat_label,
                cfg.rarity(id)
                    .map(|r| slot.per_tier * r.bonus_mul * 100.0)
                    .unwrap_or(0.0)
            ),
            None => format!(
                "{} — vide
{}

Clic : voir dans le sac",
                slot.label, slot.stat_label
            ),
        };
        if resp.on_hover_text(bulle).clicked() {
            clique = true;
        }
        ui.label(
            egui::RichText::new(&slot.label)
                .size(12.0)
                .strong()
                .color(if porte.is_some() {
                    FORGE_CREME
                } else {
                    C_TEXT_MUTED
                }),
        );
        ui.label(
            egui::RichText::new(match porte {
                Some(id) => cfg
                    .rarity(id)
                    .map(|r| r.label.clone())
                    .unwrap_or_else(|| id.clone()),
                None => "vide".to_string(),
            })
            .size(10.0)
            .color(col),
        );
    });
    ui.add_space(8.0);
    clique
}

/// Côté d'une case du sac.
const SAC_CASE: f32 = 52.0;

/// **LE SAC**, dessiné À CÔTÉ de la poupée (story-678, 2026-08-06).
///
/// Pas une page à part : la fiche de personnage et l'inventaire se regardent,
/// c'est tout l'intérêt. On voit ce qu'on porte et ce qu'on pourrait porter dans
/// le même écran — la convention Diablo, et la raison pour laquelle on n'a pas
/// à mémoriser ce qu'on portait en ouvrant un autre onglet.
///
/// **Double-clic pour équiper.** Le simple clic SÉLECTIONNE (et affiche la
/// comparaison) : sur une grille dense, un clic qui équipe transforme le moindre
/// survol maladroit en changement de build.
///
/// Rend l'action à appliquer, `None` si rien n'a été demandé. La mutation est
/// faite par l'appelant : muter la sauvegarde pendant qu'on itère dessus
/// emprunterait deux fois la même donnée.
fn draw_sac(
    ui: &mut egui::Ui,
    cfg: &EquipmentConfig,
    save: &EquipmentSave,
    selection: &mut Option<(String, String)>,
) -> Option<(String, Option<String>)> {
    let mut action = None;
    let possedees: usize = cfg
        .slots
        .iter()
        .map(|s| save.owned.get(&s.id).map_or(0, |v| v.len()))
        .sum();
    let total = cfg.slots.len() * cfg.rarities.len();

    ui.vertical(|ui| {
        ui.label(display_text("LE SAC", 22.0, FORGE_OR).strong());
        ui.label(
            egui::RichText::new(format!("{possedees} / {total} pièces trouvées"))
                .size(12.0)
                .color(C_TEXT_MUTED),
        );
        ui.label(
            egui::RichText::new("Double-clic pour équiper.")
                .size(12.0)
                .color(C_TEXT_MUTED),
        );
        ui.add_space(10.0);

        for slot in &cfg.slots {
            let porte = save.equipped.get(&slot.id).cloned();
            let porte_gain = porte
                .as_deref()
                .and_then(|id| cfg.rarity(id))
                .map(|r| slot.per_tier * r.bonus_mul)
                .unwrap_or(0.0);
            let porte_power = porte.as_deref().map(|id| cfg.power_of(id)).unwrap_or(0);

            ui.label(
                egui::RichText::new(format!("{}  ·  {}", slot.label, slot.stat_label))
                    .size(12.0)
                    .color(C_TEXT_MUTED),
            );
            ui.horizontal(|ui| {
                for rarity in &cfg.rarities {
                    let a_soi = save
                        .owned
                        .get(&slot.id)
                        .is_some_and(|v| v.iter().any(|r| r == &rarity.id));
                    let sur_soi = porte.as_deref() == Some(rarity.id.as_str());
                    let choisie = selection.as_ref()
                        == Some(&(slot.id.clone(), rarity.id.clone()));
                    let sense = if a_soi {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    };
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(SAC_CASE, SAC_CASE), sense);
                    let col = cfg.color32(&rarity.id);

                    // Le fond dit la POSSESSION, la silhouette dit le TYPE, sa
                    // couleur dit la RARETÉ. Trois informations, aucun mot.
                    ui.painter().rect_filled(
                        rect,
                        egui::CornerRadius::same(8),
                        if a_soi {
                            egui::Color32::from_black_alpha(150)
                        } else {
                            egui::Color32::from_black_alpha(70)
                        },
                    );
                    slot_glyph::draw(
                        ui.painter(),
                        rect.shrink(9.0),
                        &slot.id,
                        if a_soi {
                            col
                        } else {
                            // Non trouvée : la place est réservée et la couleur
                            // annoncée, mais l'absence se lit sans comparer deux
                            // listes.
                            col.gamma_multiply(0.22)
                        },
                    );
                    if sur_soi {
                        ui.painter().rect_stroke(
                            rect,
                            egui::CornerRadius::same(8),
                            egui::Stroke::new(2.5, egui::Color32::WHITE),
                            egui::StrokeKind::Outside,
                        );
                    } else if choisie {
                        ui.painter().rect_stroke(
                            rect,
                            egui::CornerRadius::same(8),
                            egui::Stroke::new(2.0, FORGE_OR),
                            egui::StrokeKind::Outside,
                        );
                    }

                    let gain = slot.per_tier * rarity.bonus_mul;
                    let delta = cfg.power_of(&rarity.id) as i32 - porte_power as i32;
                    let bulle = if !a_soi {
                        format!(
                            "{} — {}\npas encore trouvée\n{} +{:.0}% si tu l'obtiens",
                            slot.label,
                            rarity.label,
                            slot.stat_label,
                            gain * 100.0
                        )
                    } else if sur_soi {
                        format!(
                            "{} — {}\nPORTÉE · {} +{:.0}%\n\nDouble-clic pour la retirer ({:+} Puissance)",
                            slot.label,
                            rarity.label,
                            slot.stat_label,
                            gain * 100.0,
                            -(cfg.power_of(&rarity.id) as i32)
                        )
                    } else {
                        format!(
                            "{} — {}\n{} +{:.0}%  ({:+.0}% vs portée)\n{delta:+} Puissance\n\nDouble-clic pour l'équiper",
                            slot.label,
                            rarity.label,
                            slot.stat_label,
                            gain * 100.0,
                            (gain - porte_gain) * 100.0
                        )
                    };
                    let resp = resp.on_hover_text(bulle);
                    if resp.clicked() {
                        *selection = Some((slot.id.clone(), rarity.id.clone()));
                    }
                    if resp.double_clicked() {
                        action = Some((
                            slot.id.clone(),
                            if sur_soi {
                                None
                            } else {
                                Some(rarity.id.clone())
                            },
                        ));
                    }
                    ui.add_space(4.0);
                }
            });
            ui.add_space(8.0);
        }
    });
    action
}

/// Section Armes au menu-titre — carte d'arme (stats / élément / matchup +
/// sélecteur ‹ › + déblocage) avec **aperçu 3D live** : l'image RTT de
/// `weapon_preview` (l'arme tourne). Réutilise `draw_weapon_menu_panel` de
/// forgia-mode-roguelite. Système séparé (params + `WeaponPreviewRtt`).
fn sys_menu_armes(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    page: Res<MenuPage>,
    mut choice: ResMut<StartingWeaponChoice>,
    cards: Res<WeaponCards>,
    elem_cfg: Res<ElementConfig>,
    mut save: ResMut<MetaShopSave>,
    cat: Res<MetaShopCatalogue>,
    mut meta: ResMut<MetaSouls>,
    rtt: Option<Res<WeaponPreviewRtt>>,
) {
    if *app_state.get() != AppMode::Menu || *page != MenuPage::Armes {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };
    // Image RTT de l'aperçu 3D (None si le plugin n'a pas encore spawné la caméra).
    let weapon_image = rtt.as_ref().map(|r| r.tex_id);
    // Story-678 — chrome commun (titre/marges/transition standard).
    hub_section_panel(
        ctx,
        "hub_sec_armes",
        MenuPage::Armes.section_title(),
        500.0,
        |ui| {
            draw_weapon_menu_panel(
                ui,
                &mut choice,
                &cards,
                &elem_cfg,
                &mut save,
                &cat,
                &mut meta,
                weapon_image,
                220.0,
            );
        },
    );
}

/// Overlay PAUSED legacy (remplacé story-455 Phase G par forgia-ui-pause-menu cliquable).
/// Conservé en `#[allow(dead_code)]` pour référence courte ; à supprimer story-457.
#[allow(dead_code)]
fn paused_overlay_ui(app_state: Res<State<AppMode>>, mut ctx: EguiContexts) {
    if !matches!(app_state.get(), AppMode::Paused) {
        return;
    }
    let Ok(ctx) = ctx.ctx_mut() else { return };
    egui::Area::new(egui::Id::new("paused_overlay"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(180))
                .inner_margin(egui::Margin::symmetric(48, 32))
                .corner_radius(egui::CornerRadius::same(8))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(4.0);
                        ui.heading(
                            egui::RichText::new("PAUSED")
                                .size(56.0)
                                .color(egui::Color32::from_rgb(255, 230, 100))
                                .strong(),
                        );
                        ui.add_space(18.0);
                        ui.label(
                            egui::RichText::new("ESC — Resume")
                                .size(20.0)
                                .color(egui::Color32::WHITE),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Q   — Quit to Menu")
                                .size(20.0)
                                .color(egui::Color32::from_gray(200)),
                        );
                        ui.add_space(4.0);
                    });
                });
        });
}

/// Handler ESC unique :
///  - InGame  → Paused (pause gameplay, libère curseur)
///  - Paused  → InGame (resume)
///  - Paused + Q → Menu (quit to menu)
fn escape_handler(
    keys: Res<ButtonInput<KeyCode>>,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let from = app_state.get().clone();

    // Q during Paused = quit to Menu
    if matches!(from, AppMode::Paused) && keys.just_pressed(KeyCode::KeyQ) {
        info!("[forgia-ui] Q pressed (Paused → Menu)");
        next_app.set(AppMode::Menu);
        next_game.set(GameMode::None);
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        match &from {
            AppMode::InGame => {
                info!("[forgia-ui] ESC pressed (InGame → Paused)");
                next_app.set(AppMode::Paused);
                if let Ok(mut opts) = q_cursor.single_mut() {
                    opts.grab_mode = CursorGrabMode::None;
                    opts.visible = true;
                }
            }
            AppMode::Paused => {
                info!("[forgia-ui] ESC pressed (Paused → InGame)");
                next_app.set(AppMode::InGame);
                if let Ok(mut opts) = q_cursor.single_mut() {
                    opts.grab_mode = FPS_GRAB_MODE;
                    opts.visible = false;
                }
            }
            other => {
                info!("[forgia-ui] ESC pressed in {other:?} — no transition");
            }
        }
    }
    // Q en Paused → quitte au Menu.
    if keys.just_pressed(KeyCode::KeyQ) && matches!(from, AppMode::Paused) {
        info!("[forgia-ui] Q pressed (Paused → Menu)");
        next_app.set(AppMode::Menu);
        next_game.set(GameMode::None);
    }
}

/// Freeze `Time<Virtual>` quand on entre en Paused (animations + locomotion
/// + physics gated par Time<Virtual> stoppent). Pattern Bevy standard.
fn pause_time(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.pause();
    info!("[forgia-ui] Time<Virtual> paused");
}

/// Reprend le temps virtuel quand on sort de Paused.
fn resume_time(mut virtual_time: ResMut<Time<Virtual>>) {
    virtual_time.unpause();
    info!("[forgia-ui] Time<Virtual> resumed");
}

/// Le mode de capture du curseur qui MARCHE sur la plateforme courante.
///
/// ## Pourquoi ce n'est pas `Locked` partout (2026-08-04)
///
/// winit **ne supporte pas `Locked` sur Windows** : la demande échoue, et comme
/// rien ne remonte l'erreur côté jeu, la souris sortait simplement de la fenêtre.
/// Rapporté deux fois en playtest — « la souris ne doit jamais pouvoir sortir de
/// l'écran ».
///
/// `Confined` est le mode supporté sur Windows : le curseur est borné au cadre.
/// Le mouse-look n'en souffre pas, il lit le mouvement **brut** du périphérique,
/// qui continue d'arriver même curseur collé à un bord.
///
/// macOS / Wayland / X11 gardent `Locked` (verrouillage au centre), plus précis
/// là où il existe.
#[cfg(target_os = "windows")]
const FPS_GRAB_MODE: CursorGrabMode = CursorGrabMode::Confined;
#[cfg(not(target_os = "windows"))]
const FPS_GRAB_MODE: CursorGrabMode = CursorGrabMode::Locked;

/// Capture le curseur + invisible quand on entre InGame (pour mouse_look).
fn grab_cursor(mut q: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut opts) = q.single_mut() {
        opts.grab_mode = FPS_GRAB_MODE;
        opts.visible = false;
        info!("[forgia-ui] Cursor grabbed (Locked + invisible)");
    } else {
        warn!("[forgia-ui] grab_cursor: PrimaryWindow CursorOptions not found");
    }
}

/// Story-528 follow-up — bloque mouse_look + block fire pendant Roguelite
/// Defeat/Victory pour que la souris puisse cliquer les boutons end-of-run
/// sans pivoter la caméra ni tirer.
fn block_look_on(mut blockers: ResMut<InputBlockers>) {
    blockers.block_look = true;
    blockers.block_fire = true;
    info!("[forgia-ui] InputBlockers: look+fire ON (Roguelite end-of-run)");
}

fn block_look_off(mut blockers: ResMut<InputBlockers>) {
    blockers.block_look = false;
    blockers.block_fire = false;
    info!("[forgia-ui] InputBlockers: look+fire OFF");
}

/// Story-558 Phase 7 follow-up (2026-05-29) — toggle cursor + InputBlockers
/// selon `CoffreSession.is_open`. Quand le Coffre s'ouvre (fin de wave),
/// libère la souris pour cliquer cartes/Skip/Reroll. Quand il se ferme
/// (pick ou skip), re-grab pour reprendre l'aim FPS.
///
/// Tracked via `Local<bool>` (front montant/descendant) — évite spam each
/// frame. Gated AppMode::InGame uniquement (pas perturber Menu/Paused).
fn sys_sync_cursor_with_coffre(
    app_state: Res<State<AppMode>>,
    session: Option<Res<forgia_rpg_data::boons::CoffreSession>>,
    // Story-646 Inc.2 fix caméra (2026-07-02) — le CHOIX DE PORTE est un modal au
    // même titre que le Coffre : curseur libre pendant, re-grab + look/fire rendus
    // au pick. Sans ça : personne ne re-verrouillait après le portail (caméra morte)
    // et ce sync re-grabbait PENDANT le choix (bagarre avec l'override du hud).
    wave: Option<Res<forgia_mode_roguelite::RogueliteWave>>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut blockers: ResMut<InputBlockers>,
    mut was_open: Local<bool>,
) {
    if *app_state.get() != AppMode::InGame {
        return;
    }
    let coffre_open = session.as_ref().is_some_and(|s| s.is_open);
    let portal_open = wave
        .as_ref()
        .is_some_and(|w| !w.portal_choices.is_empty() && w.portal_pick.is_none());
    let is_open = coffre_open || portal_open;
    if is_open == *was_open {
        return;
    }
    *was_open = is_open;
    if let Ok(mut opts) = q_cursor.single_mut() {
        if is_open {
            opts.grab_mode = CursorGrabMode::None;
            opts.visible = true;
            blockers.block_look = true;
            blockers.block_fire = true;
            info!("[forgia-ui] Modal (coffre/portail) OPEN — cursor released, look+fire blocked");
        } else {
            opts.grab_mode = FPS_GRAB_MODE;
            opts.visible = false;
            blockers.block_look = false;
            blockers.block_fire = false;
            info!("[forgia-ui] Modal CLOSED — cursor grabbed, look+fire unblocked");
        }
    }
}

/// Re-grab le curseur au RETOUR DE FOCUS fenêtre (alt-tab). winit relâche le
/// grab `Locked` à la perte de focus, mais `CursorOptions.grab_mode` reste à
/// `Locked` → Bevy ne détecte aucun changement et ne ré-pousse rien à winit → le
/// curseur reste libre au retour (il « sort de l'écran »). On force la ré-
/// application (l'accès `&mut` marque le composant changé) UNIQUEMENT en gameplay
/// actif : `AppMode::InGame` + `!block_look` — ce qui exclut Pause / Coffre /
/// Lobby / écran fin-de-run, où le curseur DOIT rester libre (`block_look` y est
/// déjà à `true`).
fn sys_regrab_cursor_on_focus(
    mut focus: MessageReader<bevy::window::WindowFocused>,
    app_state: Res<State<AppMode>>,
    blockers: Res<InputBlockers>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let regained = focus.read().any(|ev| ev.focused);
    if !regained || *app_state.get() != AppMode::InGame || blockers.block_look {
        return;
    }
    if let Ok(mut opts) = q_cursor.single_mut() {
        opts.grab_mode = FPS_GRAB_MODE;
        opts.visible = false;
        info!("[forgia-ui] focus regagné — curseur re-grabbed (anti alt-tab)");
    }
}

/// Release cursor (visible + free) quand on entre Menu.
fn release_cursor(mut q: Query<&mut CursorOptions, With<PrimaryWindow>>) {
    if let Ok(mut opts) = q.single_mut() {
        opts.grab_mode = CursorGrabMode::None;
        opts.visible = true;
        info!("[forgia-ui] Cursor released (None + visible)");
    } else {
        warn!("[forgia-ui] release_cursor: PrimaryWindow CursorOptions not found");
    }
}

/// Réconciliateur curseur du Lobby Roguelite — fix « pas de souris au lancement »
/// (design home-hub 2026-06-26, P1). À l'entrée Roguelite, `grab_cursor`
/// (OnEnter InGame) et `release_cursor` (OnEnter RunState::Lobby) tirent la même
/// frame sur deux schedules SANS ordre → le grab pouvait gagner, curseur
/// verrouillé sous le wizard d'arme. Ce système est l'unique source de vérité du
/// curseur AU LOBBY : par-frame, set-if-different (zéro churn), il garantit
/// curseur libre + look/fire bloqués quelle que soit l'ordre des OnEnter ou le
/// timing de 1ʳᵉ activation du SubState. Gaté (InGame + Roguelite + Lobby) au
/// wire-up → no-op partout ailleurs.
fn sys_force_lobby_cursor_free(
    mut q: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut blockers: ResMut<InputBlockers>,
) {
    if let Ok(mut opts) = q.single_mut() {
        if opts.grab_mode != CursorGrabMode::None || !opts.visible {
            opts.grab_mode = CursorGrabMode::None;
            opts.visible = true;
        }
    }
    if !blockers.block_look {
        blockers.block_look = true;
    }
    if !blockers.block_fire {
        blockers.block_fire = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_constructible() {
        let _p = ForgiaUiPlugin;
    }
}
