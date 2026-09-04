//! Micro-motion du menu (story-678 Phase 2) — fondu+glissement d'entrée des
//! sections et anneau d'impulsion des boutons. Tout en egui pur, coupable d'un
//! seul toggle (accessibilité : le motion peut donner la nausée à certains).
//!
//! Les durées sont des constantes nommées, pas des genes : valeurs de layout
//! purement cosmétiques (exception explicite de `no-hardcode.md`).

use bevy_egui::egui;

/// Durée du fondu+glissement d'entrée de section.
pub const SECTION_ENTER_SECS: f64 = 0.15;
/// Amplitude du glissement d'entrée (px, vient de la droite).
pub const SECTION_SLIDE_PX: f32 = 24.0;
/// Durée de l'anneau d'impulsion après un clic.
pub const CLICK_PULSE_SECS: f64 = 0.25;

fn motion_id() -> egui::Id {
    egui::Id::new("forgia_ui_motion_enabled")
}

/// Le motion est-il actif ? (défaut : oui ; session-only pour l'instant,
/// la persistance viendra avec le panneau Options.)
pub fn motion_enabled(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(motion_id(), || true))
}

/// Toggle accessibilité — branché sur la page Options.
pub fn set_motion_enabled(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(motion_id(), on));
}

/// (opacité 0..1, décalage px) d'entrée pour la section `key`. L'animation se
/// relance quand la clé change — c'est la navigation elle-même qui pilote.
pub fn section_enter_anim(ctx: &egui::Context, key: &str) -> (f32, f32) {
    if !motion_enabled(ctx) {
        return (1.0, 0.0);
    }
    let store = egui::Id::new("forgia_ui_section_enter");
    let key_id = egui::Id::new(key);
    let now = ctx.input(|i| i.time);
    let start = match ctx.data_mut(|d| d.get_temp::<(egui::Id, f64)>(store)) {
        Some((k, s)) if k == key_id => s,
        _ => {
            ctx.data_mut(|d| d.insert_temp(store, (key_id, now)));
            now
        }
    };
    let t = ((now - start) / SECTION_ENTER_SECS).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ctx.request_repaint();
    }
    let eased = 1.0 - (1.0 - t).powi(3);
    (eased, (1.0 - eased) * SECTION_SLIDE_PX)
}

/// Anneau doré qui s'étend et s'éteint après un clic. À appeler chaque frame
/// avec la Response du bouton (l'état vit dans la mémoire temp egui).
pub fn click_pulse(response: &egui::Response, color: egui::Color32) {
    let ctx = &response.ctx;
    if !motion_enabled(ctx) {
        return;
    }
    let id = response.id.with("forgia_click_pulse");
    let now = ctx.input(|i| i.time);
    if response.clicked() {
        ctx.data_mut(|d| d.insert_temp(id, now));
    }
    let Some(start) = ctx.data_mut(|d| d.get_temp::<f64>(id)) else {
        return;
    };
    let t = ((now - start) / CLICK_PULSE_SECS) as f32;
    if !(0.0..1.0).contains(&t) {
        return;
    }
    ctx.request_repaint();
    let eased = 1.0 - (1.0 - t).powi(2);
    let alpha = ((1.0 - eased) * 200.0) as u8;
    let stroke = egui::Stroke::new(
        1.0 + 3.0 * (1.0 - eased),
        egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha),
    );
    ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, id))
        .rect_stroke(
            response.rect.expand(4.0 + eased * 10.0),
            egui::CornerRadius::same(16),
            stroke,
            egui::StrokeKind::Outside,
        );
}
