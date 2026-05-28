//! Console runtime — modifier l'état sans recompile.
//!
//! Pattern V1 `forgia-game/src/debug/console.rs` (600 LOC) porté en V2 avec
//! découplage 3-layer : forgia-debug émet `ConsoleEvent`, les crates gameplay
//! consomment optionnellement via `EventReader<ConsoleEvent>`.
//!
//! ## Activation
//!
//! Touche `~` (Backquote, AZERTY-friendly) ou F1 — registrable via `DebugBindings`.
//!
//! ## Commandes MVP
//!
//! ```text
//! :help                      — list commands
//! :teleport <x> <y> <z>      — move player
//! :heal [amount]             — restore HP (default = full)
//! :god                       — toggle invulnerable
//! :respawn                   — trigger respawn
//! :wave <n>                  — Roguelite jump wave
//! :set <key> <value>         — modify FpsTuning runtime
//! :spawn <archetype>         — spawn entity
//! :dump_sensors              — force re-poll sensors
//! :clear                     — clear console history
//! ```
//!
//! ## Découplage
//!
//! `forgia-debug` n'a aucune dep gameplay. Les events `ConsoleEvent` sont
//! diffusés via `EventWriter` ; tout consommateur est opt-in.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::collections::VecDeque;

const HISTORY_CAP: usize = 100;
const INPUT_HISTORY_CAP: usize = 50;
const HELP_TEXT: &str = "\
Commands:
  :help                     - list commands
  :teleport <x> <y> <z>     - move player
  :heal [amount=full]       - restore HP
  :god                      - toggle invulnerable
  :respawn                  - trigger respawn
  :wave <n>                 - Roguelite jump wave
  :set <key> <value>        - modify FpsTuning runtime
  :spawn <archetype>        - spawn entity
  :dump_sensors             - force re-poll sensors
  :clear                    - clear console history
";

/// Messages émis par la console. Consommés optionnellement par les crates
/// métier via `MessageReader<ConsoleEvent>`. `forgia-debug` ne consomme PAS
/// lui-même — pure source de découplage.
#[derive(Message, Debug, Clone)]
pub enum ConsoleEvent {
    Teleport { x: f32, y: f32, z: f32 },
    Heal { amount: Option<f32> },
    ToggleGod,
    Respawn,
    SetWave(u32),
    SetTuning { key: String, value: f32 },
    Spawn { archetype: String },
    DumpSensors,
    /// Commande inconnue — handlers custom peuvent matcher.
    Custom { raw: String, args: Vec<String> },
}

/// Résultat du parsing — soit un event à émettre, soit un feedback texte
/// (echo, erreur, clear).
#[derive(Debug)]
enum ParseResult {
    Event(ConsoleEvent),
    Echo(String),
    Clear,
    Empty,
}

/// État runtime de la console.
#[derive(Resource, Debug, Default)]
pub struct ConsoleState {
    pub visible: bool,
    pub input_buffer: String,
    pub history: VecDeque<String>,
    /// Liste des commandes envoyées (pour recall up/down).
    pub input_history: VecDeque<String>,
    /// Curseur dans input_history quand l'user navigue avec ↑/↓.
    pub input_cursor: Option<usize>,
    /// Marker : focus console activé cette frame. Les systèmes gameplay
    /// peuvent gate via `run_if` pour bloquer l'input.
    pub had_focus_last_frame: bool,
}

impl ConsoleState {
    pub fn has_focus(&self) -> bool {
        self.visible
    }

    fn push_history(&mut self, line: impl Into<String>) {
        if self.history.len() >= HISTORY_CAP {
            self.history.pop_front();
        }
        self.history.push_back(line.into());
    }

    fn push_input_history(&mut self, cmd: String) {
        // Évite doublon consécutif.
        if self.input_history.back().is_some_and(|last| last == &cmd) {
            return;
        }
        if self.input_history.len() >= INPUT_HISTORY_CAP {
            self.input_history.pop_front();
        }
        self.input_history.push_back(cmd);
    }
}

/// Toggle visibility — appelé depuis `handle_bindings_system` quand
/// `DebugAction::ToggleConsole` détecté.
pub fn toggle_console(state: &mut ConsoleState) {
    state.visible = !state.visible;
    if state.visible {
        state.input_cursor = None;
    }
}

/// System de rendu egui + input handling.
pub fn draw_console_system(
    mut contexts: EguiContexts,
    mut state: ResMut<ConsoleState>,
    mut events: MessageWriter<ConsoleEvent>,
) {
    let was_focused = state.had_focus_last_frame;
    state.had_focus_last_frame = state.visible;

    if !state.visible {
        return;
    }
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let mut submit = false;
    egui::Window::new("Forgia Console")
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -40.0])
        .default_width(560.0)
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height(220.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &state.history {
                        ui.label(line);
                    }
                });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(">");
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut state.input_buffer)
                        .desired_width(f32::INFINITY)
                        .hint_text("type :help"),
                );
                if !was_focused {
                    resp.request_focus();
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
            });
        });

    if submit {
        let cmd = std::mem::take(&mut state.input_buffer).trim().to_string();
        if cmd.is_empty() {
            return;
        }
        state.push_history(format!("> {cmd}"));
        state.push_input_history(cmd.clone());
        state.input_cursor = None;

        match parse_command(&cmd) {
            ParseResult::Event(ev) => {
                state.push_history(format!("  → {ev:?}"));
                events.write(ev);
            }
            ParseResult::Echo(msg) => state.push_history(msg),
            ParseResult::Clear => state.history.clear(),
            ParseResult::Empty => {}
        }
    }
}

/// System de navigation ↑/↓ dans input_history. Séparé pour clarté.
pub fn console_history_navigation_system(
    mut state: ResMut<ConsoleState>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if !state.visible || state.input_history.is_empty() {
        return;
    }
    let len = state.input_history.len();
    if keys.just_pressed(KeyCode::ArrowUp) {
        let next = match state.input_cursor {
            None => len - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        state.input_cursor = Some(next);
        if let Some(cmd) = state.input_history.get(next).cloned() {
            state.input_buffer = cmd;
        }
    } else if keys.just_pressed(KeyCode::ArrowDown) {
        let next = state.input_cursor.map(|i| i + 1);
        match next {
            Some(i) if i >= len => {
                state.input_cursor = None;
                state.input_buffer.clear();
            }
            Some(i) => {
                state.input_cursor = Some(i);
                if let Some(cmd) = state.input_history.get(i).cloned() {
                    state.input_buffer = cmd;
                }
            }
            None => {}
        }
    }
}

/// Parser pur — testable indépendamment.
fn parse_command(input: &str) -> ParseResult {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return ParseResult::Empty;
    }
    let body = trimmed.strip_prefix(':').unwrap_or(trimmed);
    let mut parts = body.split_whitespace();
    let Some(cmd) = parts.next() else {
        return ParseResult::Empty;
    };
    let args: Vec<&str> = parts.collect();

    match cmd {
        "help" | "?" => ParseResult::Echo(HELP_TEXT.to_string()),
        "clear" => ParseResult::Clear,
        "teleport" | "tp" => {
            if args.len() != 3 {
                return ParseResult::Echo("error: :teleport <x> <y> <z>".into());
            }
            match (args[0].parse(), args[1].parse(), args[2].parse()) {
                (Ok(x), Ok(y), Ok(z)) => ParseResult::Event(ConsoleEvent::Teleport { x, y, z }),
                _ => ParseResult::Echo("error: x y z must be numbers".into()),
            }
        }
        "heal" => {
            let amount = match args.first() {
                None => None,
                Some(s) => match s.parse() {
                    Ok(v) => Some(v),
                    Err(_) => return ParseResult::Echo("error: heal amount must be number".into()),
                },
            };
            ParseResult::Event(ConsoleEvent::Heal { amount })
        }
        "god" => ParseResult::Event(ConsoleEvent::ToggleGod),
        "respawn" => ParseResult::Event(ConsoleEvent::Respawn),
        "wave" => {
            if args.len() != 1 {
                return ParseResult::Echo("error: :wave <n>".into());
            }
            match args[0].parse() {
                Ok(n) => ParseResult::Event(ConsoleEvent::SetWave(n)),
                Err(_) => ParseResult::Echo("error: wave n must be u32".into()),
            }
        }
        "set" => {
            if args.len() != 2 {
                return ParseResult::Echo("error: :set <key> <value>".into());
            }
            match args[1].parse() {
                Ok(value) => ParseResult::Event(ConsoleEvent::SetTuning {
                    key: args[0].to_string(),
                    value,
                }),
                Err(_) => ParseResult::Echo("error: value must be number".into()),
            }
        }
        "spawn" => {
            if args.len() != 1 {
                return ParseResult::Echo("error: :spawn <archetype>".into());
            }
            ParseResult::Event(ConsoleEvent::Spawn {
                archetype: args[0].to_string(),
            })
        }
        "dump_sensors" | "dump" => ParseResult::Event(ConsoleEvent::DumpSensors),
        _ => ParseResult::Event(ConsoleEvent::Custom {
            raw: cmd.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_to_event(s: &str) -> Option<ConsoleEvent> {
        match parse_command(s) {
            ParseResult::Event(e) => Some(e),
            _ => None,
        }
    }

    #[test]
    fn teleport_parses_floats() {
        let ev = parse_to_event(":teleport 10.5 -3 20").unwrap();
        match ev {
            ConsoleEvent::Teleport { x, y, z } => {
                assert!((x - 10.5).abs() < 1e-3);
                assert!((y - -3.0).abs() < 1e-3);
                assert!((z - 20.0).abs() < 1e-3);
            }
            _ => panic!("expected Teleport"),
        }
    }

    #[test]
    fn heal_optional_amount() {
        match parse_to_event(":heal").unwrap() {
            ConsoleEvent::Heal { amount: None } => {}
            other => panic!("expected Heal None, got {other:?}"),
        }
        match parse_to_event(":heal 25").unwrap() {
            ConsoleEvent::Heal { amount: Some(v) } if (v - 25.0).abs() < 1e-3 => {}
            other => panic!("expected Heal 25, got {other:?}"),
        }
    }

    #[test]
    fn god_toggle_no_args() {
        match parse_to_event(":god").unwrap() {
            ConsoleEvent::ToggleGod => {}
            _ => panic!(),
        }
    }

    #[test]
    fn wave_parses_u32() {
        match parse_to_event(":wave 3").unwrap() {
            ConsoleEvent::SetWave(3) => {}
            _ => panic!(),
        }
    }

    #[test]
    fn set_tuning_key_value() {
        match parse_to_event(":set gravity 5.0").unwrap() {
            ConsoleEvent::SetTuning { key, value } => {
                assert_eq!(key, "gravity");
                assert!((value - 5.0).abs() < 1e-3);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn unknown_command_is_custom() {
        match parse_to_event(":foobar arg1 arg2").unwrap() {
            ConsoleEvent::Custom { raw, args } => {
                assert_eq!(raw, "foobar");
                assert_eq!(args, vec!["arg1", "arg2"]);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn errors_dont_emit_events() {
        assert!(parse_to_event(":teleport 1 2").is_none()); // missing arg
        assert!(parse_to_event(":wave abc").is_none()); // bad type
        assert!(parse_to_event(":set foo").is_none()); // missing value
    }

    #[test]
    fn help_and_clear_no_event() {
        assert!(matches!(parse_command(":help"), ParseResult::Echo(_)));
        assert!(matches!(parse_command(":clear"), ParseResult::Clear));
    }

    #[test]
    fn colon_optional() {
        assert!(parse_to_event("god").is_some());
        assert!(parse_to_event("teleport 1 2 3").is_some());
    }
}
