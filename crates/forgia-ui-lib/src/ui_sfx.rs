//! File de sons d'UI, stockée dans la mémoire egui (story-678 Phase 1).
//!
//! Les helpers de style (`cartoon_btn`, `glass_btn`, le Coffre…) POUSSENT des
//! [`UiSfxKind`] ici au fil du dessin ; un système Bevy (le consommateur audio,
//! `forgia-mode-roguelite::audio::sys_ui_sfx`) DRAINE la file une fois par
//! frame et joue les sons. Ce découplage garde forgia-ui-lib côté egui pur :
//! aucun accès canal audio ici, et tout bouton du jeu (menu, HUD, overlays)
//! devient sonore sans instrumenter chaque call-site.
//!
//! Le hover est émis en FRONT MONTANT (une fois à l'entrée du widget) : en
//! immediate mode le survol est re-signalé 60×/s, un son par frame serait
//! inécoutable. L'état « était survolé » vit dans la mémoire temp egui,
//! par widget.

use bevy_egui::egui;

/// Les natures de sons d'UI. Le mapping vers un fichier/volume vit dans le
/// genome audio (`roguelite_audio.toml`), pas ici.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UiSfxKind {
    Hover,
    Click,
    Tab,
    Buy,
    Unlock,
    Denied,
}

/// File frame-courante. Type stocké en mémoire temp egui (Clone requis).
#[derive(Clone, Default)]
struct UiSfxQueue(Vec<UiSfxKind>);

/// Garde-fou : au-delà, on n'empile plus (un drain raté ne doit pas croître
/// sans borne ni produire une rafale au drain suivant).
const QUEUE_CAP: usize = 16;

fn queue_id() -> egui::Id {
    egui::Id::new("forgia_ui_sfx_queue")
}

/// Pousse un son d'UI à jouer cette frame.
pub fn push_ui_sfx(ctx: &egui::Context, kind: UiSfxKind) {
    ctx.data_mut(|d| {
        let q = d.get_temp_mut_or_default::<UiSfxQueue>(queue_id());
        if q.0.len() < QUEUE_CAP {
            q.0.push(kind);
        }
    });
}

/// Vide la file et rend son contenu. Appelé une fois par frame par le
/// consommateur audio.
pub fn drain_ui_sfx(ctx: &egui::Context) -> Vec<UiSfxKind> {
    ctx.data_mut(|d| std::mem::take(&mut d.get_temp_mut_or_default::<UiSfxQueue>(queue_id()).0))
}

/// Le SURVOL seul, en front montant.
///
/// Séparé de [`instrument_button`] pour les widgets qui portent déjà leur propre
/// son de clic — les onglets de la sidebar jouent un [`UiSfxKind::Tab`], et les
/// instrumenter entièrement leur aurait ajouté un `Click` en doublon. Un widget
/// n'a qu'un son de clic, mais tout widget cliquable mérite son survol.
pub fn instrument_hover(response: &egui::Response) {
    let ctx = &response.ctx;
    let hover_id = response.id.with("forgia_sfx_hover");
    let was_hovered = ctx.data_mut(|d| d.get_temp::<bool>(hover_id).unwrap_or(false));
    let hovered = response.hovered();
    if hovered && !was_hovered {
        push_ui_sfx(ctx, UiSfxKind::Hover);
    }
    if hovered != was_hovered {
        ctx.data_mut(|d| d.insert_temp(hover_id, hovered));
    }
}

/// Instrumente la [`egui::Response`] d'un bouton : hover en front montant,
/// clic. À appeler depuis les helpers de style, jamais depuis les call-sites.
pub fn instrument_button(response: &egui::Response) {
    instrument_hover(response);
    if response.clicked() {
        push_ui_sfx(&response.ctx, UiSfxKind::Click);
    }
}
