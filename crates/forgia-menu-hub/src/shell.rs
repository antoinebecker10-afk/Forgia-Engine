//! Le dessin du hub : chrome persistant, fond (arène ou vidéo), et le dispatch
//! de la page courante par le registre.
//!
//! Vivait dans `forgia-ui/src/menu/shell.rs` jusqu'à la story-694 incrément 5 ;
//! déplacé verbatim, à trois exceptions près, toutes signalées sur place :
//! le chemin des modules (un cran de moins), le fond vidéo qui se
//! lit maintenant via l'API publique du shell, et le retour de navigation qui
//! arrive par message au lieu d'un appel direct.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use forgia_core::prelude::*;
use forgia_mode_roguelite::identity::IdentitySave;
use forgia_mode_roguelite::meta_shop::MetaShopSave;
use forgia_mode_roguelite::progress::PlayerProgress;
use forgia_ui::menu_video::{self, MenuVideoState};
use forgia_ui::MenuBackRequested;
use forgia_ui_lib::pause_menu::UserSettings;

use crate::arena_backdrop::ArenaBackdropRtt;
use crate::chrome::{draw_hub_smith_chip, draw_hub_souls_chip, hub_section_panel};
use crate::currency_icons::CurrencyIcons;
use crate::nav::{draw_hub_nav, HubBadges, MenuAction, MenuPage, NavStack};
use crate::pages::root::{draw_options_page, draw_root_landing};
use crate::registry::{self, InlinePageCtx, PageDraw};

/// Applique la demande de retour émise par l'unique handler ESC/B du shell.
///
/// Le shell possède la touche, le hub possède la pile (story-694 incrément 5).
/// Le garde « egui édite du texte » a DÉJÀ été évalué à l'émission — ne pas le
/// re-tester ici : à ce moment de la frame, egui a vu la touche et la réponse
/// aurait changé.
pub(crate) fn sys_apply_menu_back(
    mut requests: MessageReader<MenuBackRequested>,
    mut nav: ResMut<NavStack>,
) {
    // `read()` draine : plusieurs demandes la même frame (ESC + B via Steam
    // Input, déjà gardé côté émission) ne doivent remonter que d'un cran.
    if requests.read().next().is_none() {
        return;
    }
    if nav.back() {
        info!("[menu-hub] retour nav (ESC/B) → {:?}", nav.current());
    }
}

/// Dit au shell que le diorama couvre le fond, pour qu'il gèle son pipeline
/// vidéo (story-691). Tourne en `PreUpdate` : `menu_video_tick` vit en
/// `Update`, l'ordre est donc garanti sans ancre supplémentaire.
///
/// Compare-and-write — un `ResMut` touché chaque frame réveillerait pour rien
/// tout système gardé par `Changed`.
pub(crate) fn sys_publish_backdrop_covered(
    backdrop: Option<Res<ArenaBackdropRtt>>,
    mut covered: ResMut<forgia_ui::MenuBackdropCovered>,
) {
    let now = backdrop.as_deref().is_some_and(|b| b.is_showing());
    if covered.0 != now {
        covered.0 = now;
    }
}

/// Story-693 — nourrit le capteur d'identité : « l'édition du nom/couleur a
/// été montrée ». Son poseur historique (panneau Lobby) est supprimé — la
/// fiche Forgeron du menu est LA surface d'édition. Marqué à la page plutôt
/// qu'au call-site : le système de la fiche est au plafond des 16 params Bevy.
pub(crate) fn sys_mark_identity_shown(
    app_state: Res<State<AppMode>>,
    nav: Res<NavStack>,
    shown: Option<ResMut<forgia_mode_roguelite::identity::IdentityPanelShown>>,
) {
    if *app_state.get() != AppMode::Menu || nav.current() != MenuPage::Forgeron {
        return;
    }
    if let Some(mut s) = shown {
        if !s.0 {
            s.0 = true;
        }
    }
}

pub(crate) fn main_menu_ui(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    mut next_app: ResMut<NextState<AppMode>>,
    mut next_game: ResMut<NextState<GameMode>>,
    mut exit: MessageWriter<AppExit>,
    mut video: Option<ResMut<MenuVideoState>>,
    asset_server: Res<AssetServer>,
    mut nav: ResMut<NavStack>,
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
        info!("[menu-hub] main_menu_ui state = {current:?}");
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
        warn!("[menu-hub] main_menu_ui: egui ctx not found (no Camera2d?)");
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
    draw_hub_nav(ctx, &mut nav, *badges);
    draw_hub_souls_chip(ctx, souls_n, shards_n, icons.as_deref());

    // ── Panneau de la section active — piloté par LA table (story-694 incr. 4).
    // Root et Options restent câblés ici (Root rend une MenuAction appliquée
    // plus bas — évite de passer NextState/MessageWriter dans chaque helper ;
    // Options mute la pile) ; les pages INLINE tirent leur dessinateur du
    // registre ; les pages OwnSystem sont dessinées par leur système auto-gaté
    // — rien à faire dans ce match. Ajouter une page inline = 1 PageDecl + 1 fn.
    let action = match nav.current() {
        MenuPage::Root => draw_root_landing(ctx),
        MenuPage::Options => {
            draw_options_page(ctx, &mut nav, &mut settings);
            MenuAction::None
        }
        page => {
            let d = registry::decl(page);
            if let PageDraw::Inline {
                panel_id,
                panel_width,
                draw,
            } = d.draw
            {
                let ictx = InlinePageCtx {
                    level,
                    ranks_remaining: remaining,
                    meta_save: meta_save.as_deref(),
                };
                hub_section_panel(ctx, panel_id, d.section_title, panel_width, |ui| {
                    draw(&ictx, ui)
                });
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_mode_roguelite::identity::IdentityPanelShown;

    /// Monte le minimum pour faire TOURNER le système : un état, la pile, le
    /// drapeau. Les tests de pile voisins n'appellent que des méthodes ; celui-ci
    /// exerce le système lui-même, câblage `Res`/`ResMut` compris.
    fn app_au_menu(page: MenuPage) -> App {
        let mut app = App::new();
        // `init_state` exige le schedule `StateTransition` — sans ce plugin, le
        // montage panique avant d'avoir rien prouvé.
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.insert_state(AppMode::Menu);
        let mut nav = NavStack::default();
        if page != MenuPage::Root {
            nav.switch_tab(page);
        }
        app.insert_resource(nav);
        app.insert_resource(IdentityPanelShown(false));
        app.add_systems(Update, sys_mark_identity_shown);
        app
    }

    /// Régression story-694 incrément 5 : ce système a changé de CRATE, et le
    /// capteur `identity` a rallumé son avertissement le soir même. Ce test
    /// sépare les deux causes possibles — « la fiche n'a pas été ouverte » et
    /// « le déménagement a cassé le marquage » — pour qu'on n'ait plus jamais à
    /// le déduire d'un souvenir de playtest.
    #[test]
    fn la_fiche_forgeron_marque_l_identite_comme_montree() {
        let mut app = app_au_menu(MenuPage::Forgeron);
        app.update();
        assert!(
            app.world().resource::<IdentityPanelShown>().0,
            "sur la fiche Forgeron, le drapeau doit passer à vrai"
        );
    }

    /// Le pendant : ailleurs qu'à la fiche, rien ne se marque. Sans lui, un
    /// système qui écrirait `true` inconditionnellement passerait le test
    /// ci-dessus — et le capteur ne dirait plus jamais rien d'utile.
    #[test]
    fn ailleurs_qu_a_la_fiche_rien_n_est_marque() {
        let mut app = app_au_menu(MenuPage::Root);
        app.update();
        assert!(
            !app.world().resource::<IdentityPanelShown>().0,
            "hors de la fiche Forgeron, le drapeau doit rester faux"
        );
    }
}

// `draw_arena_test_section` retirée avec l'onglet (2026-07-30, temporaire) — elle
// n'avait plus qu'un appelant, supprimé lui aussi. Son contenu est dans
// l'historique git de `forgia-ui/src/menu/shell.rs` ; la restaurer suffit à
// rouvrir le banc. Rien n'a été touché côté moteur.
