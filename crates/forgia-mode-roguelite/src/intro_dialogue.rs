//! # Roguelite — dialogue d'arrivée (bulle BD cartoon)
//!
//! Quand le joueur **arrive en Roguelite**, une courte séquence de répliques
//! (Maître Forgeron + L'Apprenti + une âme-arme) s'affiche dans une **bulle de
//! dialogue cartoon** (style BD : portrait + plaque-nom + texte machine-à-écrire),
//! pour ancrer l'immersion dans le monde de Forgia (bible v1 kid-friendly).
//!
//! Auto-contenu et **data-driven** : la séquence est lue depuis
//! `assets/genomes/roguelite/roguelite_intro.toml` (fallback intégré si absent).
//! Ne ré-implémente PAS la pipeline `BarkEvent` (dormante depuis le refactor
//! story-471..479) — c'est un système simple et indépendant.
//!
//! ## Pipeline
//! `Startup: sys_load_intro` (charge le TOML) → `OnEnter(Roguelite): sys_start_intro`
//! (lance la séquence) → `Update: sys_advance_intro` (machine à écrire + ESPACE pour
//! avancer + auto-advance) → `EguiPrimaryContextPass: draw_intro_bubble` (rendu).
//! Sensor `forgia2_roguelite_intro.json`.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use forgia_core::prelude::*;
use forgia_ui_lib::style::*;
use serde::Deserialize;

use crate::run::RunState;

/// Fichier de données (relatif à la racine workspace = cwd de `cargo run`).
const INTRO_PATH: &str = "assets/genomes/roguelite/roguelite_intro.toml";
/// Vitesse de révélation machine à écrire.
const CHARS_PER_SEC: f32 = 38.0;
/// Temps d'attente après révélation complète avant auto-avance.
const AUTO_HOLD_SEC: f32 = 2.4;
/// Largeur de wrap du texte (caractères).
const WRAP_CHARS: usize = 46;

// ─── Données ──────────────────────────────────────────────────────────────

/// Une réplique de la séquence d'arrivée.
#[derive(Deserialize, Clone, Debug)]
pub struct IntroLine {
    /// Id persona (couleur via [`forge_persona_color`]).
    pub speaker: String,
    /// Libellé affiché (plaque-nom).
    pub name: String,
    /// Texte de la réplique.
    pub text: String,
}

/// Document TOML : `[[line]]` répétés.
#[derive(Deserialize, Default)]
struct IntroFile {
    #[serde(default)]
    line: Vec<IntroLine>,
}

/// État runtime de la séquence d'arrivée.
#[derive(Resource, Default)]
pub struct IntroDialogue {
    /// Séquence chargée (ordonnée).
    lines: Vec<IntroLine>,
    /// La séquence est en cours d'affichage.
    active: bool,
    /// Index de la réplique courante.
    index: usize,
    /// `elapsed_secs` au début de la réplique courante.
    line_started: f32,
    /// Caractères révélés (machine à écrire).
    revealed: usize,
}

/// Séquence de secours si le TOML est absent/invalide (jamais d'écran muet).
fn fallback_lines() -> Vec<IntroLine> {
    [
        ("maitre_forgeron", "Maître Forgeron", "Te revoilà, apprenti. La Forge t'attendait."),
        ("apprenti", "L'Apprenti", "Mmh. Je vais ramener les âmes."),
        ("maitre_forgeron", "Maître Forgeron", "Casse les cages. Et reviens entier."),
    ]
    .into_iter()
    .map(|(s, n, t)| IntroLine {
        speaker: s.to_string(),
        name: n.to_string(),
        text: t.to_string(),
    })
    .collect()
}

// ─── Systèmes ───────────────────────────────────────────────────────────────

/// Charge la séquence depuis le TOML (Startup). Fallback intégré si lecture/parse KO.
fn sys_load_intro(mut intro: ResMut<IntroDialogue>) {
    match std::fs::read_to_string(INTRO_PATH) {
        Ok(s) => match toml::from_str::<IntroFile>(&s) {
            Ok(f) if !f.line.is_empty() => {
                info!("[roguelite-intro] {} répliques chargées", f.line.len());
                intro.lines = f.line;
            }
            Ok(_) => {
                warn!("[roguelite-intro] {INTRO_PATH} vide → fallback");
                intro.lines = fallback_lines();
            }
            Err(e) => {
                error!("[roguelite-intro] parse {INTRO_PATH}: {e} → fallback");
                intro.lines = fallback_lines();
            }
        },
        Err(e) => {
            warn!("[roguelite-intro] lecture {INTRO_PATH}: {e} → fallback");
            intro.lines = fallback_lines();
        }
    }
}

/// Lance la séquence à l'arrivée en Roguelite.
fn sys_start_intro(time: Res<Time>, mut intro: ResMut<IntroDialogue>) {
    if intro.lines.is_empty() {
        return;
    }
    intro.active = true;
    intro.index = 0;
    intro.revealed = 0;
    intro.line_started = time.elapsed_secs();
}

/// Machine à écrire + avance (ESPACE/ENTRÉE accélère ou passe, sinon auto-advance).
fn sys_advance_intro(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut intro: ResMut<IntroDialogue>,
) {
    if !intro.active {
        return;
    }
    let now = time.elapsed_secs();
    let total = match intro.lines.get(intro.index) {
        Some(l) => l.text.chars().count(),
        None => {
            intro.active = false;
            return;
        }
    };

    let elapsed = (now - intro.line_started).max(0.0);
    intro.revealed = ((elapsed * CHARS_PER_SEC) as usize).min(total);
    let fully = intro.revealed >= total;

    let pressed = keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter);

    // Première pression sur une ligne en cours de frappe → tout révéler.
    if pressed && !fully {
        intro.revealed = total;
        intro.line_started = now - (total as f32 / CHARS_PER_SEC) - 0.01;
        return;
    }

    let auto_advance = fully && elapsed > (total as f32 / CHARS_PER_SEC) + AUTO_HOLD_SEC;
    if (pressed && fully) || auto_advance {
        intro.index += 1;
        if intro.index >= intro.lines.len() {
            intro.active = false;
        } else {
            intro.line_started = now;
            intro.revealed = 0;
        }
    }
}

/// Sensor `forgia2_roguelite_intro.json` (1 Hz).
fn sys_write_intro_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    intro: Res<IntroDialogue>,
) {
    *accum += time.delta_secs();
    if *accum < 1.0 {
        return;
    }
    *accum = 0.0;
    let severity = if intro.lines.is_empty() { "warn" } else { "ok" };
    let json = format!(
        r#"{{"id":"roguelite_intro","severity":"{severity}","timestamp_secs":{:.1},"lines_total":{},"active":{},"index":{},"revealed":{}}}"#,
        time.elapsed_secs(),
        intro.lines.len(),
        intro.active,
        intro.index,
        intro.revealed,
    );
    if let Err(e) = std::fs::write("forgia2_roguelite_intro.json", &json) {
        warn!("[roguelite-intro] sensor write failed: {e}");
    }
}

// ─── Rendu — bulle BD cartoon ───────────────────────────────────────────────

/// Dessine la bulle de dialogue cartoon (bas-centre) : portrait persona + plaque-nom +
/// texte machine-à-écrire, sur fond bois clair + bordure or + drop-shadow.
pub(crate) fn draw_intro_bubble(
    mut contexts: EguiContexts,
    app_state: Res<State<AppMode>>,
    game_mode: Res<State<GameMode>>,
    run_state: Option<Res<State<RunState>>>,
    intro: Res<IntroDialogue>,
) {
    if *app_state.get() != AppMode::InGame || *game_mode.get() != GameMode::Roguelite {
        return;
    }
    if !intro.active {
        return;
    }
    // Seulement pendant le run réel (pas Lobby / Defeat / Victory).
    if !matches!(
        run_state.as_deref().map(|s| s.get()),
        Some(RunState::InRun { .. }) | Some(RunState::Boss { .. })
    ) {
        return;
    }
    let Some(line) = intro.lines.get(intro.index) else {
        return;
    };
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let total = line.text.chars().count();
    let fully = intro.revealed >= total;
    let shown: String = line.text.chars().take(intro.revealed).collect();
    let wrapped = wrap_text(&shown, WRAP_CHARS);
    let persona = forge_persona_color(&line.speaker);

    egui::Area::new(egui::Id::new("forgia_roguelite_intro"))
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -130.0))
        .interactable(false)
        .show(ctx, |ui| {
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(780.0, 172.0), egui::Sense::hover());
            let p = ui.painter_at(rect);

            // Boîte cartoon : shadow + fond bois clair + double bordure (charbon + or).
            cartoon_drop_shadow(&p, rect, 22.0, egui::vec2(7.0, 9.0));
            chunky_rect_filled(&p, rect, FORGE_BOIS_CLAIR, 5.0, 22.0);
            p.rect_stroke(
                rect.shrink(4.0),
                18.0,
                egui::Stroke::new(3.0, FORGE_OR),
                egui::StrokeKind::Inside,
            );

            // Portrait persona (disque + anneau charbon + reflet).
            let pc = egui::pos2(rect.left() + 78.0, rect.center().y);
            let r = 54.0;
            p.circle_filled(pc, r + 5.0, FORGE_CHARBON);
            p.circle_filled(pc, r, persona);
            p.circle_filled(
                pc + egui::vec2(-r * 0.32, -r * 0.32),
                r * 0.32,
                lerp_color(persona, egui::Color32::WHITE, 0.45),
            );
            let initial = line
                .name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            text_with_outline(
                &p,
                pc,
                egui::Align2::CENTER_CENTER,
                &initial,
                egui::FontId::proportional(50.0),
                FORGE_CREME,
                3.0,
            );

            // Plaque-nom (couleur persona, outline pour pop sur parchemin).
            text_with_outline(
                &p,
                egui::pos2(rect.left() + 156.0, rect.top() + 32.0),
                egui::Align2::LEFT_CENTER,
                &line.name,
                egui::FontId::proportional(24.0),
                persona,
                3.0,
            );

            // Réplique (charbon sur bois clair → pas d'outline, lisibilité directe).
            p.text(
                egui::pos2(rect.left() + 156.0, rect.top() + 58.0),
                egui::Align2::LEFT_TOP,
                &wrapped,
                egui::FontId::proportional(25.0),
                FORGE_CHARBON,
            );

            // Indice "ESPACE ▶" une fois la ligne révélée.
            if fully {
                text_with_outline(
                    &p,
                    rect.right_bottom() + egui::vec2(-18.0, -14.0),
                    egui::Align2::RIGHT_BOTTOM,
                    "ESPACE  \u{25B6}",
                    egui::FontId::proportional(15.0),
                    FORGE_OR,
                    2.0,
                );
            }
        });
}

/// Word-wrap glouton à `max_chars` colonnes (insère des `\n`). Le painter egui gère
/// le multi-ligne ; on révèle d'abord, on wrappe ensuite → le retour reste stable.
fn wrap_text(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    let mut line_len = 0usize;
    for word in s.split_whitespace() {
        let wl = word.chars().count();
        if line_len > 0 && line_len + 1 + wl > max_chars {
            out.push('\n');
            line_len = 0;
        } else if line_len > 0 {
            out.push(' ');
            line_len += 1;
        }
        out.push_str(word);
        line_len += wl;
    }
    out
}

// ─── Plugin ─────────────────────────────────────────────────────────────────

/// Branche le système de dialogue d'arrivée Roguelite.
pub struct IntroDialoguePlugin;

impl Plugin for IntroDialoguePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<IntroDialogue>()
            .add_systems(Startup, sys_load_intro)
            .add_systems(OnEnter(GameMode::Roguelite), sys_start_intro)
            .add_systems(
                Update,
                sys_advance_intro
                    .in_set(GameSet::UI)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(
                Update,
                sys_write_intro_sensor
                    .in_set(GameSet::Sensors)
                    .run_if(in_state(GameMode::Roguelite)),
            )
            .add_systems(EguiPrimaryContextPass, draw_intro_bubble);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_is_non_empty_and_valid() {
        let lines = fallback_lines();
        assert!(!lines.is_empty());
        for l in &lines {
            assert!(!l.speaker.is_empty());
            assert!(!l.text.is_empty());
        }
    }

    #[test]
    fn wrap_inserts_breaks() {
        let wrapped = wrap_text("un deux trois quatre cinq six sept huit neuf dix", 12);
        assert!(wrapped.contains('\n'), "long text should wrap");
        // chaque ligne <= 12 + 1 mot qui déborde (greedy)
        for line in wrapped.lines() {
            assert!(line.chars().count() <= 14, "line too long: {line:?}");
        }
    }

    #[test]
    fn ships_intro_toml_parses() {
        // Le TOML livré doit parser et être non vide.
        let s = include_str!("../../../assets/genomes/roguelite/roguelite_intro.toml");
        let f: IntroFile = toml::from_str(s).expect("roguelite_intro.toml must parse");
        assert!(!f.line.is_empty());
        assert!(f.line.iter().all(|l| !l.text.is_empty()));
    }
}
