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
#[allow(clippy::too_many_arguments)]
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

// ─── Story-558 Phase 7 (2026-05-29) — Palette "Forge" cartoon kid-friendly ──
//
// Bible v1 cartoon family-friendly (Overwatch×Hadès×Borderlands, cible
// enfants+femmes). Recherche industry 2024-2026 (Hadès UI, Cult of the Lamb
// 4.5M ventes, NN/g, IxDF Disney 12 principes UI).
//
// Convergence : fond chaud saturé + accent doré + 1 chaud + 1 froid pour
// respirer. À utiliser en Roguelite uniquement (Arena/RPG gardent palette HUD
// standard ci-dessus).
//
// Référence : docs/audit/roguelite-engagement-audit-2026-05-29.md +
// reference_bible_forgia_roguelite_v1.

/// Background panels (parchemin / bois clair). Fond du Coffre du Forgeron.
pub const FORGE_BOIS_CLAIR: Color32 = Color32::from_rgb(212, 165, 116);
/// Or saffron — accents, souls counter, rare boons, CTA primary.
pub const FORGE_OR: Color32 = Color32::from_rgb(244, 196, 48);
/// Rouge braise — HP bar, damage, Defeat overlay.
pub const FORGE_BRAISE: Color32 = Color32::from_rgb(231, 76, 60);
/// Métal forgé — borders neutres.
pub const FORGE_METAL_CHAUD: Color32 = Color32::from_rgb(168, 162, 158);
/// Texte principal — haut contraste sur bois (ratio 7.8:1 vs FORGE_BOIS_CLAIR).
pub const FORGE_CHARBON: Color32 = Color32::from_rgb(43, 24, 16);
/// Texte sur fond sombre, highlights.
pub const FORGE_CREME: Color32 = Color32::from_rgb(255, 244, 220);
/// Healing, mana, qualité commune-plus.
pub const FORGE_TEAL: Color32 = Color32::from_rgb(60, 174, 163);
/// Fond panneau « charbon chaud » (modaux sombres : Enclume, reward cards).
/// Story-596 — remplace les littéraux (28,26,34)/(28,24,20) dispersés.
pub const FORGE_PANEL: Color32 = Color32::from_rgb(36, 28, 22);
/// Variante éclaircie de [`FORGE_PANEL`] (cartes internes d'un modal).
pub const FORGE_PANEL_LIGHT: Color32 = Color32::from_rgb(52, 42, 34);

// Rarity colors — convention Hearthstone/Diablo (universellement lisible
// enfants). Override des couleurs HUD générique pour le Coffre.
pub const FORGE_RARITY_COMMON: Color32 = Color32::from_rgb(157, 157, 157);
pub const FORGE_RARITY_UNCOMMON: Color32 = Color32::from_rgb(30, 255, 0);
pub const FORGE_RARITY_RARE: Color32 = Color32::from_rgb(0, 112, 221);
pub const FORGE_RARITY_EPIC: Color32 = Color32::from_rgb(163, 53, 238);
pub const FORGE_RARITY_LEGENDARY: Color32 = Color32::from_rgb(255, 128, 0);

/// Dessine un drop-shadow soft cartoon en empilant 3 rects offset+alpha
/// décroissant derrière une zone. egui n'a pas de blur natif — approximation
/// classique (cf egui discussions + Hadès dialogue boxes).
///
/// `offset` = direction shadow (typique `vec2(6.0, 6.0)`).
pub fn cartoon_drop_shadow(
    painter: &egui::Painter,
    rect: Rect,
    corner_radius: f32,
    offset: egui::Vec2,
) {
    for (i, alpha) in [80u8, 50u8, 25u8].iter().enumerate() {
        let factor = (i + 1) as f32;
        let shadow_rect = rect.translate(offset * factor);
        painter.rect_filled(
            shadow_rect,
            corner_radius,
            Color32::from_black_alpha(*alpha),
        );
    }
}

// ─── Persona colors (dialogue Roguelite — cast bible v1) ─────────────────
//
// Couleurs cartoon des âmes-armes / PNJ pour les bulles de dialogue. Saturées,
// distinctes au premier coup d'œil (bible : timide vert / vent bleu / noble violet
// / boucher rouge / mentor cuivre / héros teal). Source unique : ici.

/// Pépin (pistolet timide) — vert frais.
pub const FORGE_PERSONA_PEPIN: Color32 = Color32::from_rgb(122, 201, 130);
/// Bourrasque (SMG vent) — bleu vif.
pub const FORGE_PERSONA_BOURRASQUE: Color32 = Color32::from_rgb(96, 165, 235);
/// Madame Lenoir (sniper) — violet noble.
pub const FORGE_PERSONA_LENOIR: Color32 = Color32::from_rgb(168, 116, 214);
/// Boucherie / Maurice (shotgun) — rouge sang chaud.
pub const FORGE_PERSONA_BOUCHERIE: Color32 = Color32::from_rgb(208, 78, 66);
/// Maître Forgeron (mentor) — cuivre chaud.
pub const FORGE_PERSONA_FORGERON: Color32 = Color32::from_rgb(214, 146, 74);
/// L'Apprenti (héros) — teal doux.
pub const FORGE_PERSONA_APPRENTI: Color32 = Color32::from_rgb(110, 184, 196);
/// Le Forgeron Noir (boss) — charbon violacé.
pub const FORGE_PERSONA_NOIR: Color32 = Color32::from_rgb(92, 80, 104);

/// Couleur cartoon d'un persona depuis son `speaker` id (dialogue Roguelite).
/// Fallback métal neutre pour `"any"` / inconnu.
pub fn forge_persona_color(speaker: &str) -> Color32 {
    match speaker {
        "pepin" => FORGE_PERSONA_PEPIN,
        "bourrasque" => FORGE_PERSONA_BOURRASQUE,
        "lenoir" => FORGE_PERSONA_LENOIR,
        "boucherie" => FORGE_PERSONA_BOUCHERIE,
        "maitre_forgeron" => FORGE_PERSONA_FORGERON,
        "apprenti" => FORGE_PERSONA_APPRENTI,
        "forgeron_noir" => FORGE_PERSONA_NOIR,
        _ => FORGE_METAL_CHAUD,
    }
}

// ─── Composants partagés (story-596 Phase A) ────────────────────────────

/// Ease-out-back canonique (overshoot ~1.1 puis settle). Source : IxDF Disney
/// 12 UI principles. Centralisé ici — était copié dans kill_popup + hud enrage.
pub fn ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    let t = t.clamp(0.0, 1.0);
    let x = t - 1.0;
    1.0 + C3 * x * x * x + C1 * x * x
}

/// Bouton cartoon Forge : display font, texte charbon, stroke charbon 4px,
/// coins 14, 280×52. Extrait du Defeat overlay (story-558 Phase 7) pour
/// réutilisation Victory / menu principal / Enclume.
pub fn cartoon_btn(ui: &mut egui::Ui, label: &str, fill: Color32) -> egui::Response {
    ui.add(
        egui::Button::new(
            crate::theme::display_text(label, 22.0, FORGE_CHARBON).strong(),
        )
        .fill(fill)
        .stroke(Stroke::new(4.0, FORGE_CHARBON))
        .corner_radius(egui::CornerRadius::same(14))
        .min_size(egui::vec2(280.0, 52.0)),
    )
}

/// Frame modal cartoon « bois + or » (pattern Defeat/Coffre, bible v1).
pub fn forge_panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(FORGE_BOIS_CLAIR)
        .inner_margin(egui::Margin::symmetric(80, 48))
        .corner_radius(egui::CornerRadius::same(20))
        .stroke(Stroke::new(5.0, FORGE_OR))
}
