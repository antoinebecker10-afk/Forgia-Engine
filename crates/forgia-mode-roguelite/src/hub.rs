//! hub.rs — Hub d'accueil Roguelite à ONGLETS (design home-hub 2026-06-26, P2).
//!
//! Avant : les 3 panneaux du Lobby (Forgeron / Armes / Enclume) s'affichaient TOUS
//! en même temps, ancrés à des coins différents. Ici on les regroupe en **onglets**
//! (un seul panneau visible à la fois) + un **bandeau** (Âmes + nom + niveau
//! placeholder) + un bouton cliquable **« LANCER LA RUN »** (curseur libre depuis P1).
//!
//! Ce module n'absorbe PAS les panneaux : ils restent dessinés par leurs systèmes
//! (`weapon_select` / `meta_shop` / `identity`). Le hub ajoute le CHROME + un
//! `Resource HubTab` + des run-conditions (`on_*_tab`) qui gatent chaque panneau par
//! onglet. L'aperçu 3D de l'arme reste visible en permanence (hero du hub).
//!
//! Le bouton LANCER écrit `StartRunEvent` — équivalent souris de la touche Entrée
//! gérée par `meta_shop::sys_meta_shop_input` (les deux chemins coexistent).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_ui_lib::style::{
    cartoon_btn, glass_frame_hero, C_PRIMARY, C_TEXT_MUTED, FORGE_CREME, FORGE_OR, FORGE_PANEL,
    FORGE_PANEL_LIGHT, HAIR_GOLD_STRONG,
};
use forgia_ui_lib::theme::display_text;

use crate::identity::IdentitySave;
use crate::pipeline_warmup::WarmupState;
use crate::progress::PlayerProgress;
use crate::run::{MetaSouls, StartRunEvent};
use crate::RunState;

/// Onglet actif du hub Lobby. Reset à `Forge` à chaque entrée au Lobby.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum HubTab {
    /// Accueil : le forgeron (nom/couleur via `identity`) + résumé.
    #[default]
    Forge,
    /// Choix d'arme (wizard `weapon_select`).
    Armes,
    /// L'Enclume des Âmes (upgrades permanents, `meta_shop`).
    Enclume,
    /// Arbres de talents — placeholder (gameplay = story suivante).
    Talents,
    /// Bestiaire — archétypes ennemis (P1 shell : cartes archétypes réelles).
    Codex,
    /// Défis quotidiens/hebdo (P1 shell : placeholder stylé).
    Missions,
    /// Hauts faits / achievements (P1 shell : placeholder stylé).
    Succes,
}

impl HubTab {
    fn label(self) -> &'static str {
        match self {
            HubTab::Forge => "⚒ FORGE",
            HubTab::Armes => "🗡 ARMES",
            HubTab::Enclume => "🔨 ENCLUME",
            HubTab::Talents => "✦ TALENTS",
            HubTab::Codex => "📖 CODEX",
            HubTab::Missions => "🎯 MISSIONS",
            HubTab::Succes => "🏆 SUCCÈS",
        }
    }

    /// Titre (display font) affiché sous la barre d'onglets pour l'onglet actif.
    fn title(self) -> &'static str {
        match self {
            HubTab::Forge => "TON FORGERON",
            HubTab::Armes => "CHOISIS TON ARME",
            HubTab::Enclume => "L'ENCLUME DES ÂMES",
            HubTab::Talents => "TALENTS",
            HubTab::Codex => "CODEX · BESTIAIRE",
            HubTab::Missions => "MISSIONS",
            HubTab::Succes => "HAUTS FAITS",
        }
    }
}

// ── Run-conditions : gatent les panneaux existants par onglet ──────────────────
// `Option<Res>` → safe hors-état (HubTab est init_resource au plugin, donc Some en
// pratique ; le `.unwrap_or(false)` couvre l'ordre d'init).

/// Vrai quand l'onglet FORGE est actif (panneau identité).
pub fn on_forge_tab(hub: Option<Res<HubTab>>) -> bool {
    hub.map(|h| *h == HubTab::Forge).unwrap_or(false)
}
/// Vrai quand l'onglet ARMES est actif (wizard d'arme).
pub fn on_armes_tab(hub: Option<Res<HubTab>>) -> bool {
    hub.map(|h| *h == HubTab::Armes).unwrap_or(false)
}
/// Vrai quand l'onglet ENCLUME est actif (meta shop).
pub fn on_enclume_tab(hub: Option<Res<HubTab>>) -> bool {
    hub.map(|h| *h == HubTab::Enclume).unwrap_or(false)
}

/// Reset l'onglet à `Forge` à chaque entrée au Lobby (retour menu/fin de run).
fn reset_hub_tab(mut hub: ResMut<HubTab>) {
    *hub = HubTab::Forge;
}

/// Au Lobby : masque tout le HUD de gameplay (ammo/PV/énergie/confiance/viewmodel)
/// pour ne garder que l'UI de menu du hub. Le HUD partagé (forgia-ui-lib /
/// forgia-viewmodel) lit `GameplayHudVisible` (forgia-core) sans dépendre de ce crate.
fn lobby_hide_gameplay_hud(mut v: ResMut<GameplayHudVisible>) {
    v.0 = false;
}

/// À la sortie du Lobby (run lancée OU retour menu) : ré-affiche le HUD de gameplay
/// + coupe l'aperçu bras forcé (onglet Forge).
fn lobby_show_gameplay_hud(
    mut v: ResMut<GameplayHudVisible>,
    mut forced: ResMut<ViewmodelForcedVisible>,
) {
    v.0 = true;
    forced.0 = false;
}

/// Aperçu des bras dans l'onglet FORGE : force le viewmodel (bras) visible quand
/// l'onglet Forge est actif (sinon masqué au Lobby) → on voit la couleur/style choisis.
fn sync_forge_arm_preview(hub: Res<HubTab>, mut forced: ResMut<ViewmodelForcedVisible>) {
    let want = *hub == HubTab::Forge;
    if forced.0 != want {
        forced.0 = want;
    }
}

/// Petit cadre panneau cohérent avec les autres panneaux du Lobby (FORGE_PANEL).
fn chip_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(FORGE_PANEL)
        .inner_margin(egui::Margin::symmetric(14, 8))
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
}

/// Chrome du hub : bandeau (Âmes / nom / niveau) + barre d'onglets + bouton LANCER.
/// Gaté Lobby au wire-up → no-op partout ailleurs.
fn draw_hub_chrome(
    mut contexts: EguiContexts,
    mut hub: ResMut<HubTab>,
    souls: Option<Res<MetaSouls>>,
    identity: Option<Res<IdentitySave>>,
    progress: Option<Res<PlayerProgress>>,
    warmup: Option<Res<WarmupState>>,
    mut start_run: MessageWriter<StartRunEvent>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let souls_n = souls.as_ref().map(|s| s.current).unwrap_or(0);
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
    // Ne jamais faire basculer le joueur en combat pendant que les pipelines
    // PBR sont encore en compilation : ce serait déplacer les freezes du Lobby
    // vers le premier combat. L'absence de la ressource est volontairement un
    // gate fermé : le plugin de mode l'installe toujours en production.
    let warmup_ready = warmup.as_ref().is_some_and(|state| state.done);

    // ── Bandeau Âmes (haut-droite) ──
    egui::Area::new(egui::Id::new("hub_souls"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-20.0, 14.0))
        .show(ctx, |ui| {
            chip_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new(format!("◇ {souls_n}  Âmes"))
                        .size(20.0)
                        .color(FORGE_OR)
                        .strong(),
                );
            });
        });

    // ── Bandeau forgeron (haut-gauche) : nom + niveau + barre d'XP ──
    // Niveau & XP = placeholder (le système d'XP réel = phase P4).
    egui::Area::new(egui::Id::new("hub_identity_banner"))
        .anchor(egui::Align2::LEFT_TOP, egui::vec2(20.0, 14.0))
        .show(ctx, |ui| {
            chip_frame().show(ui, |ui| {
                ui.vertical(|ui| {
                    let (level, xp, xp_next, frac) = progress
                        .as_ref()
                        .map(|p| {
                            (
                                p.level,
                                p.xp,
                                PlayerProgress::xp_to_next(p.level),
                                p.xp_fraction(),
                            )
                        })
                        .unwrap_or((1, 0, 80, 0.0));
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(name)
                                .size(18.0)
                                .color(FORGE_CREME)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new(format!("· Niv. {level}"))
                                .size(14.0)
                                .color(C_TEXT_MUTED),
                        );
                    });
                    ui.add_space(4.0);
                    ui.add_sized(
                        egui::vec2(180.0, 10.0),
                        egui::ProgressBar::new(frac).fill(FORGE_OR).text(
                            egui::RichText::new(format!("{xp} / {xp_next} XP"))
                                .size(10.0)
                                .color(FORGE_CREME),
                        ),
                    );
                });
            });
        });

    // ── Barre d'onglets (sous le titre — demande user : titre AU-DESSUS de l'onglet) ──
    egui::Area::new(egui::Id::new("hub_tabs"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 74.0))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(FORGE_PANEL)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .corner_radius(egui::CornerRadius::same(10))
                .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for tab in [
                            HubTab::Forge,
                            HubTab::Armes,
                            HubTab::Talents,
                            HubTab::Enclume,
                            HubTab::Codex,
                            HubTab::Missions,
                            HubTab::Succes,
                        ] {
                            let selected = *hub == tab;
                            let resp = ui.add(egui::Button::selectable(
                                selected,
                                egui::RichText::new(tab.label())
                                    .size(18.0)
                                    .color(if selected { C_PRIMARY } else { FORGE_CREME })
                                    .strong(),
                            ));
                            if resp.clicked() {
                                *hub = tab;
                            }
                            ui.add_space(6.0);
                        }
                    });
                });
        });

    // ── Titre de l'onglet actif — AU-DESSUS de la barre d'onglets (demande user) ──
    // wrap_mode Extend → titre sur UNE seule ligne (sinon la 2e ligne « ARME » était
    // masquée par les onglets dessous). L'Area centre le label complet horizontalement.
    egui::Area::new(egui::Id::new("hub_tab_title"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 14.0))
        .show(ctx, |ui| {
            ui.add(
                egui::Label::new(display_text(hub.title(), 40.0, FORGE_OR).strong())
                    .wrap_mode(egui::TextWrapMode::Extend),
            );
        });

    // ── Sections « dashboard » (Talents / Codex / Missions / Succès) — panneau verre ──
    // P1 « shell » roguelite : la navigation complète est là tout de suite ; le
    // gameplay de chaque section arrive en stories suivantes. Codex affiche déjà les
    // vrais archétypes ennemis (données réelles, pas un placeholder).
    if matches!(
        *hub,
        HubTab::Talents | HubTab::Codex | HubTab::Missions | HubTab::Succes
    ) {
        let pts = progress.as_ref().map(|p| p.talent_points).unwrap_or(0);
        egui::Area::new(egui::Id::new("hub_section_panel"))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                glass_frame_hero()
                    .inner_margin(egui::Margin::symmetric(40, 30))
                    .show(ui, |ui| {
                        ui.set_max_width(700.0);
                        ui.vertical_centered(|ui| match *hub {
                            HubTab::Codex => draw_codex_section(ui),
                            HubTab::Talents => {
                                section_intro(
                                    ui,
                                    "Arbres de talents",
                                    "Choisis ton style — Feu · Givre · Éclair · Poison — et débloque \
                                     des combos signature en jouant.",
                                );
                                ui.add_space(8.0);
                                ui.label(display_text(
                                    format!("{pts} point(s) de talent en attente"),
                                    18.0,
                                    FORGE_OR,
                                ));
                            }
                            HubTab::Missions => section_intro(
                                ui,
                                "Missions",
                                "Des défis quotidiens & hebdomadaires. Accomplis-les pour gagner \
                                 des Âmes et débloquer des titres.",
                            ),
                            HubTab::Succes => section_intro(
                                ui,
                                "Hauts faits",
                                "Repousse tes limites — chaque haut fait débloqué rapporte des Âmes \
                                 et un titre à porter.",
                            ),
                            _ => {}
                        });
                    });
            });
    }

    // ── Bouton LANCER (bas-centre, TOUS les onglets) — équivalent souris de Entrée ──
    // La carte ARMES remonte (ws_card à -98) et les panneaux Forge/Enclume sont
    // centrés → le bas-centre est libre partout pour le CTA principal.
    egui::Area::new(egui::Id::new("hub_launch"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -24.0))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let launch_clicked = ui
                    .add_enabled_ui(warmup_ready, |ui| {
                        cartoon_btn(ui, "▶  LANCER LA RUN", FORGE_OR)
                    })
                    .inner
                    .clicked();
                if launch_clicked {
                    start_run.write(StartRunEvent { seed: None });
                    info!("[hub] LANCER cliqué → StartRunEvent");
                }
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(if warmup_ready {
                        "ou [Entrée]"
                    } else {
                        "Préparation de la Forge…"
                    })
                    .size(13.0)
                    .color(C_TEXT_MUTED),
                );
            });
        });
}

/// Intro d'une section « à venir » (Talents / Missions / Succès) : titre display +
/// pitch + tag discret. Le vrai contenu gameplay vient en stories suivantes.
fn section_intro(ui: &mut egui::Ui, title: &str, desc: &str) {
    ui.label(display_text(title, 30.0, C_PRIMARY).strong());
    ui.add_space(12.0);
    ui.label(egui::RichText::new(desc).size(16.0).color(FORGE_CREME));
    ui.add_space(14.0);
    ui.label(
        egui::RichText::new("Contenu à venir")
            .size(13.0)
            .italics()
            .color(C_TEXT_MUTED),
    );
}

/// Codex · Bestiaire — cartes des 4 archétypes ennemis. Contenu réel (comportement
/// aligné sur `enemies.rs`), pas un placeholder. Textes UI cosmétiques.
fn draw_codex_section(ui: &mut egui::Ui) {
    ui.label(display_text("Bestiaire", 28.0, C_PRIMARY).strong());
    ui.add_space(14.0);
    const ENTRIES: [(&str, &str); 4] = [
        (
            "Tank",
            "Gros, lent, lourdement blindé. Encaisse et charge dans le tas.",
        ),
        (
            "Coureur",
            "Rapide et fragile. Fonce sur toi, souvent en meute.",
        ),
        (
            "Tireur",
            "Se tient à distance et te canarde. Garde ses distances.",
        ),
        (
            "Boss — le Forgeron Noir",
            "S'enrage sous 50 % de PV. Le combat final de la run.",
        ),
    ];
    for (name, desc) in ENTRIES {
        egui::Frame::new()
            .fill(FORGE_PANEL_LIGHT)
            .inner_margin(egui::Margin::symmetric(16, 10))
            .corner_radius(egui::CornerRadius::same(10))
            .stroke(egui::Stroke::new(1.0, HAIR_GOLD_STRONG))
            .show(ui, |ui| {
                ui.set_min_width(560.0);
                ui.vertical(|ui| {
                    ui.label(display_text(name, 18.0, FORGE_OR).strong());
                    ui.label(egui::RichText::new(desc).size(14.0).color(FORGE_CREME));
                });
            });
        ui.add_space(8.0);
    }
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct HubPlugin;

impl Plugin for HubPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HubTab>()
            // Reset onglet + masque le HUD de gameplay à l'entrée Lobby (OnEnter(Lobby)
            // ne tire qu'en GameMode::Roguelite : RunState est un SubState de Roguelite).
            .add_systems(
                OnEnter(RunState::Lobby),
                (reset_hub_tab, lobby_hide_gameplay_hud),
            )
            // Sortie Lobby (run lancée ou retour menu) → ré-affiche le HUD de gameplay.
            .add_systems(OnExit(RunState::Lobby), lobby_show_gameplay_hud)
            // Aperçu bras dans l'onglet Forge (force le viewmodel visible).
            .add_systems(
                Update,
                sync_forge_arm_preview.run_if(in_state(RunState::Lobby)),
            )
            // Chrome du hub : visible au Lobby uniquement.
            .add_systems(
                EguiPrimaryContextPass,
                draw_hub_chrome
                    .run_if(in_state(AppMode::InGame))
                    .run_if(in_state(GameMode::Roguelite))
                    .run_if(in_state(RunState::Lobby)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tab_is_forge() {
        assert_eq!(HubTab::default(), HubTab::Forge);
    }

    #[test]
    fn tab_conditions_are_exclusive() {
        // Run-conditions safe hors-état : None → false (pas de Resource HubTab).
        assert!(!on_forge_tab(None));
        assert!(!on_armes_tab(None));
        assert!(!on_enclume_tab(None));
        // Labels distincts (UX : pas deux onglets identiques).
        let labels = [
            HubTab::Forge.label(),
            HubTab::Armes.label(),
            HubTab::Enclume.label(),
            HubTab::Talents.label(),
        ];
        for i in 0..labels.len() {
            for j in (i + 1)..labels.len() {
                assert_ne!(labels[i], labels[j]);
            }
        }
    }
}
