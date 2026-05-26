//! # forgia-ui-style
//!
//! Palette HUD partagée + helpers egui painter (chunky cartoon, outlines noirs).
//! Source de vérité unique pour les couleurs et primitives de rendu HUD Forgia.
//!
//! Pattern : couleurs saturées, outlines chunky 2-3px, coins arrondis 6-8px,
//! fonts monospace pour effet arcade rétro.
//!
//! Originellement dans `forgia-ui-hud/src/style.rs` — extrait story-455 Phase B
//! pour réutilisation cross-crate (forgia-ui-hud, forgia-ui-hud-ammo,
//! forgia-killfeed Phase D, forgia-ui-damage-direction Phase E…).

use bevy_egui::egui::{self, Color32, Pos2, Rect, Stroke};

// ─── Palette Forgia HUD ─────────────────────────────────────────────────

/// Orange Forgia (accent primary). Boutons, bordures actives.
pub const C_PRIMARY: Color32 = Color32::from_rgb(255, 122, 26);
/// Cyan electric (accent secondary). Highlights, info.
pub const C_ACCENT: Color32 = Color32::from_rgb(0, 217, 255);

/// HP high (>50%) — vert vif.
pub const C_HP_HIGH: Color32 = Color32::from_rgb(80, 220, 100);
/// HP mid (~50%) — jaune.
pub const C_HP_MID: Color32 = Color32::from_rgb(255, 200, 50);
/// HP low (<25%) — rouge vif.
pub const C_HP_LOW: Color32 = Color32::from_rgb(230, 57, 70);

/// Bot HP bar (toujours rouge cartoon — convention ennemis).
pub const C_BOT_HP: Color32 = Color32::from_rgb(232, 70, 60);

/// Fond panel HUD (translucide).
pub const C_BG_DARK: Color32 = Color32::from_rgba_premultiplied(15, 18, 24, 200);
/// Outline systématique (noir chunky).
pub const C_OUTLINE: Color32 = Color32::from_rgb(8, 8, 12);
/// Texte clair principal.
pub const C_TEXT_LIGHT: Color32 = Color32::from_rgb(248, 250, 252);
/// Texte secondaire (gris doux).
pub const C_TEXT_MUTED: Color32 = Color32::from_rgb(168, 178, 192);

/// Damage popup number — jaune saturé cartoon.
pub const C_DAMAGE_NUMBER: Color32 = Color32::from_rgb(255, 240, 80);
/// Headshot popup — rouge vif.
pub const C_HEADSHOT_NUMBER: Color32 = Color32::from_rgb(255, 80, 80);

/// Ammo counter color (cartoon yellow). Story-455 Phase B.
pub const C_AMMO_TEXT: Color32 = Color32::from_rgb(255, 220, 80);
/// Ammo low-flash color (saturated red AAA). Story-455 Phase B.
pub const C_AMMO_LOW: Color32 = Color32::from_rgb(255, 34, 34);
/// Reload progress arc (cyan). Story-455 Phase B.
pub const C_RELOAD_ARC: Color32 = Color32::from_rgb(80, 220, 255);
/// Slot active outline (orange Forgia bright). Story-455 Phase B.
pub const C_SLOT_ACTIVE: Color32 = Color32::from_rgb(255, 180, 60);

// ─── Color helpers ──────────────────────────────────────────────────────

/// Lerp linéaire entre 2 couleurs (per channel).
pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let blend = |x: u8, y: u8| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        blend(a.r(), b.r()),
        blend(a.g(), b.g()),
        blend(a.b(), b.b()),
        blend(a.a(), b.a()),
    )
}

/// Couleur HP selon fraction [0..1] : rouge → jaune → vert.
pub fn hp_color(fraction: f32) -> Color32 {
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.5 {
        lerp_color(C_HP_MID, C_HP_HIGH, (f - 0.5) * 2.0)
    } else {
        lerp_color(C_HP_LOW, C_HP_MID, f * 2.0)
    }
}

/// Couleur ammo selon fraction mag [0..1]. low_threshold = seuil rouge (genome-driven).
/// Au-dessus : jaune Forgia. En-dessous : pulse rouge (alpha = appelant).
pub fn ammo_color(fraction: f32, low_threshold: f32) -> Color32 {
    if fraction <= low_threshold {
        C_AMMO_LOW
    } else {
        C_AMMO_TEXT
    }
}

// ─── Painter helpers ────────────────────────────────────────────────────

/// Rect arrondi avec outline noir chunky — pattern cartoon Fortnite.
pub fn chunky_rect_filled(
    painter: &egui::Painter,
    rect: Rect,
    fill: Color32,
    outline_width: f32,
    rounding: f32,
) {
    painter.rect_filled(rect, rounding, fill);
    painter.rect_stroke(
        rect,
        rounding,
        Stroke::new(outline_width, C_OUTLINE),
        egui::StrokeKind::Outside,
    );
}

/// Texte avec outline noir (8 passes) — effet cartoon lisible.
pub fn text_with_outline(
    painter: &egui::Painter,
    pos: Pos2,
    anchor: egui::Align2,
    text: &str,
    font: egui::FontId,
    fill: Color32,
    outline_thickness: f32,
) {
    let offsets: [(f32, f32); 8] = [
        (-1.0, -1.0),
        (0.0, -1.0),
        (1.0, -1.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (0.0, 1.0),
        (1.0, 1.0),
    ];
    for (dx, dy) in offsets {
        painter.text(
            Pos2::new(
                pos.x + dx * outline_thickness,
                pos.y + dy * outline_thickness,
            ),
            anchor,
            text,
            font.clone(),
            C_OUTLINE,
        );
    }
    painter.text(pos, anchor, text, font, fill);
}

/// Arc circular (utilisé pour reload progress autour d'une icône, ou damage direction).
/// `progress` ∈ [0, 1] = fraction de l'arc remplie depuis l'angle de départ.
/// Source : pattern AAA CoD reload progress ring autour icône arme.
pub fn arc_stroke(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    start_rad: f32,
    span_rad: f32,
    progress: f32,
    color: Color32,
    thickness: f32,
    segments: usize,
) {
    let progress = progress.clamp(0.0, 1.0);
    if progress <= 0.0 || segments == 0 {
        return;
    }
    let total = span_rad * progress;
    // BUG-455-09 fix : skip si span/progress dégénéré (genome misconfigured ou progress=0).
    if total.abs() < 1e-6 {
        return;
    }
    let step = total / segments as f32;
    let mut prev = polar(center, radius, start_rad);
    for i in 1..=segments {
        let theta = start_rad + step * i as f32;
        let next = polar(center, radius, theta);
        painter.line_segment([prev, next], Stroke::new(thickness, color));
        prev = next;
    }
}

#[inline]
fn polar(center: Pos2, r: f32, theta: f32) -> Pos2 {
    Pos2::new(center.x + r * theta.cos(), center.y + r * theta.sin())
}
