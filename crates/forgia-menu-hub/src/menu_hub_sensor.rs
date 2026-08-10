//! Story-678 transverse — capteur `forgia2_menu_hub.json`.
//!
//! Le hub doit être diagnosticable d'UNE lecture (« regarde ») : page courante,
//! navigation cumulée (front montant — un capteur instantané ne valide rien),
//! pastilles, dernière famille d'entrée, sons d'UI joués, présence du fond
//! avatar et du résumé de dernière run. Deux health checks avec next-step :
//! MENU MUET (on navigue mais rien ne sonne) et BACKDROP MORT (l'avatar de
//! fond ne rend pas alors qu'on est au menu).

use bevy::prelude::*;
use forgia_core::prelude::*;
use forgia_mode_roguelite::audio::RogueliteAudioStats;
use forgia_mode_roguelite::meta_shop::MetaShopSave;
use forgia_ui_lib::pause_menu::UserSettings;

use crate::arena_backdrop::ArenaBackdropRtt;
use crate::gamepad_nav::LastInputKind;
use crate::weapon_preview::CharacterPreviewRtt;
use crate::{HubBadges, MenuPage, NavStack};

const SENSOR_PATH: &str = "forgia2_menu_hub.json";
const PERIOD_SECS: f32 = 1.0;
/// Hors menu, le capteur publie des données figées (`menu_active=false`, page
/// inchangée) : un heartbeat lent suffit — même politique que `menu_video`
/// (story-691 : il écrivait 1 fichier/s pendant toute une run pour rien).
const INACTIVE_PERIOD_SECS: f32 = 30.0;
/// Navigation minimale avant de juger le silence (3 changements de page sans
/// aucun son = quelque chose est cassé, pas un joueur immobile).
const MUTE_MIN_PAGE_CHANGES: u32 = 3;
/// Tolérance avant de déclarer le fond avatar mort (spawn OnEnter + chargement).
const BACKDROP_GRACE_SECS: f32 = 5.0;

#[derive(Resource, Default)]
pub struct MenuHubSensorState {
    timer: f32,
    last_page: Option<MenuPage>,
    /// Cumul session des changements de page (front montant).
    page_changes_session: u32,
    /// Temps passé au menu SANS `CharacterPreviewRtt` (reset dès qu'il existe).
    backdrop_absent_secs: f32,
    /// Dernier état publié — une transition menu ↔ jeu s'écrit immédiatement,
    /// sans attendre le heartbeat inactif de 30 s.
    last_menu_active: Option<bool>,
}

#[allow(clippy::too_many_arguments)]
pub fn sys_write_menu_hub_sensor(
    time: Res<Time<bevy::time::Real>>,
    app_state: Res<State<AppMode>>,
    nav: Res<NavStack>,
    mut state: ResMut<MenuHubSensorState>,
    badges: Res<HubBadges>,
    kind: Res<LastInputKind>,
    settings: Option<Res<UserSettings>>,
    audio_stats: Option<Res<RogueliteAudioStats>>,
    meta_save: Option<Res<MetaShopSave>>,
    rtt: Option<Res<CharacterPreviewRtt>>,
    backdrop: Option<Res<ArenaBackdropRtt>>,
    identity: Option<Res<forgia_mode_roguelite::identity::IdentitySave>>,
    catalogue: Option<Res<forgia_mode_roguelite::cosmetics::CosmeticsConfig>>,
) {
    let menu_active = *app_state.get() == AppMode::Menu;
    // Fronts montants — comptés à CHAQUE frame, indépendamment de la cadence
    // d'écriture (sinon on rate les changements entre deux écritures).
    let page = nav.current();
    if state.last_page != Some(page) {
        if state.last_page.is_some() && menu_active {
            state.page_changes_session = state.page_changes_session.saturating_add(1);
        }
        state.last_page = Some(page);
    }
    // Le fond est VIVANT quand son diorama a réellement posé des props.
    //
    // 🚨 Ce test mesurait `CharacterPreviewRtt.is_some()` — c'est-à-dire le
    // PORTRAIT, celui-là même qui avait REMPLACÉ le fond quand la Phase 5 a été
    // défaite. Le health check « BACKDROP MORT » ne pouvait donc pas voir la
    // panne qu'il surveillait : il restait vert alors qu'il n'y avait plus
    // aucun fond. Un capteur qui ne peut pas rougir ne mesure rien
    // (`reference_capteur_instantane_ne_valide_rien`).
    let backdrop_props = backdrop.as_deref().map(|b| b.props_spawned).unwrap_or(0);
    if menu_active && backdrop_props == 0 {
        state.backdrop_absent_secs += time.delta_secs();
    } else {
        state.backdrop_absent_secs = 0.0;
    }

    state.timer += time.delta_secs();
    let interval = if menu_active {
        PERIOD_SECS
    } else {
        INACTIVE_PERIOD_SECS
    };
    let state_changed = state.last_menu_active != Some(menu_active);
    if !state_changed && state.timer < interval {
        return;
    }
    state.timer = 0.0;
    state.last_menu_active = Some(menu_active);

    let ui_sfx = audio_stats.as_deref().map(|s| s.ui_sfx).unwrap_or(0);
    let motion = settings.as_deref().map(|s| s.ui_motion_enabled);
    let backdrop_ready = backdrop_props > 0;
    // L'avatar reste publié à part : c'est lui que le fond FILME (un seul
    // squelette). Sans lui, le décor est là mais le personnage manque —
    // symptôme distinct, donc champ distinct.
    let avatar_ready = rtt.is_some();
    let last_run_present = meta_save
        .as_deref()
        .is_some_and(|s| s.last_run.is_some());

    // Story-678 — la COLLECTION de décors. Deux nombres distincts, parce qu'ils
    // tombent en panne séparément : le décor RENDU (ce qu'on voit à l'écran) et
    // le décor DEMANDÉ (ce que la sauvegarde réclame). S'ils divergent, c'est
    // que le décor équipé n'est pas possédé — le repli a joué, et il faut le
    // voir dans le capteur plutôt que de se demander pourquoi le fond « ne
    // change pas ».
    let backdrop_shown = backdrop
        .as_deref()
        .map(|b| b.shown_backdrop().to_string())
        .unwrap_or_default();
    let backdrop_wanted = identity
        .as_deref()
        .map(|i| i.equipped_backdrop.clone())
        .unwrap_or_default();
    // La COLLECTION, toutes familles confondues — c'est la mesure du
    // Marketplace : combien d'articles le joueur a réellement ouverts.
    let (backdrops_owned, backdrops_total) = match (catalogue.as_deref(), identity.as_deref()) {
        (Some(cat), Some(id)) => {
            let owned = forgia_mode_roguelite::cosmetics::OwnedCosmetics {
                chapters_cleared: meta_save.as_deref().map(|s| s.chapters_cleared).unwrap_or(0),
                identity: id,
            };
            (
                cat.items.iter().filter(|c| owned.has(c)).count(),
                cat.items.len(),
            )
        }
        _ => (0, 0),
    };
    let shards = meta_save.as_deref().map(|s| s.shards_total).unwrap_or(0);

    let mute = menu_active
        && state.page_changes_session >= MUTE_MIN_PAGE_CHANGES
        && ui_sfx == 0;
    let backdrop_dead = menu_active && state.backdrop_absent_secs > BACKDROP_GRACE_SECS;
    let (severity, next_step) = if mute {
        (
            "warning",
            "MENU MUET : on navigue mais 0 son d'UI — grep 'Path not found' \
             (assets/audio/forgia_original/ui/*.ogg) puis ui_sfx_volume du genome",
        )
    } else if backdrop_dead {
        (
            "warning",
            "BACKDROP MORT : 0 prop posé dans le fond d'arène au menu — vérifier \
             ui_backdrop_enabled (roguelite_render.toml), puis que la palette du \
             chapitre atteint a bien des landmarks/big/scatter (roguelite_palettes.toml)",
        )
    } else {
        ("info", "-")
    };

    let json = format!(
        r#"{{"id":"menu_hub","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{:.1},"menu_active":{menu_active},"page":"{:?}","nav_depth":{},"nav_path":"{}","page_changes_session":{},"ui_sfx_session":{ui_sfx},"motion_enabled":{},"last_input":"{}","badge_enclume":{},"badge_forgeron":{},"badge_livre":{},"backdrop_ready":{backdrop_ready},"backdrop_props":{backdrop_props},"avatar_ready":{avatar_ready},"backdrop_shown":"{backdrop_shown}","backdrop_wanted":"{backdrop_wanted}","cosmetics_owned":{backdrops_owned},"cosmetics_total":{backdrops_total},"shards":{shards},"last_run_present":{last_run_present}}}"#,
        time.elapsed_secs(),
        page,
        nav.depth(),
        nav.path(),
        state.page_changes_session,
        motion.map(|m| m.to_string()).unwrap_or_else(|| "null".into()),
        match *kind {
            LastInputKind::Mouse => "mouse",
            LastInputKind::Gamepad => "gamepad",
        },
        badges.enclume,
        badges.forgeron,
        badges.livre,
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}
