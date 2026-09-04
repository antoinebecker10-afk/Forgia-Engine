//! Forgia V2 — Root binary entry-point (Renzora-style).
//!
//! `cargo run` from workspace root launches the game. Real assembly lives in
//! the `forgia-game` library crate.

fn main() {
    install_crash_sensor();
    forgia_game::run_game();
}

/// Story-592 (audit 2026-06-10, M0.4) : panic hook → `forgia2_crash.json`.
///
/// Sans ce hook, un crash chez un playtesteur ne laisse AUCUNE trace post-mortem
/// (le panic part sur stderr, perdu hors `run_debug.bat`). Le sensor de crash se
/// croise avec les autres `forgia2_*.json` écrits à 1 Hz par le jeu :
/// `forgia2_roguelite_state.json` porte run/wave/seed au moment du crash.
///
/// Le hook délègue ensuite au hook par défaut (affichage console préservé).
fn install_crash_sensor() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "panic avec payload non-string".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "localisation inconnue".to_string());
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        // force_capture : indépendant de RUST_BACKTRACE (les playtesteurs ne le settent pas).
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let timestamp_unix_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let payload = build_crash_payload(
            &message,
            &location,
            &thread,
            &backtrace,
            timestamp_unix_secs,
        );
        // Best-effort : on est en train de crasher, aucune erreur ne doit masquer le panic.
        let _ = std::fs::write("forgia2_crash.json", payload);

        default_hook(info);
    }));
}

/// Construit le JSON du sensor crash (pure, testable). Convention sensor V2 :
/// `{id, severity, next_step, ...}` — serde_json gère l'échappement des
/// backslashes Windows et des retours à la ligne du backtrace.
fn build_crash_payload(
    message: &str,
    location: &str,
    thread: &str,
    backtrace: &str,
    timestamp_unix_secs: u64,
) -> String {
    let value = serde_json::json!({
        "id": "crash",
        "severity": "critical",
        "next_step": "lire message+location ; croiser forgia2_roguelite_state.json (run/wave/seed au crash) et forgia2_run.log ; reproduire avec le meme seed",
        "timestamp_unix_secs": timestamp_unix_secs,
        "message": message,
        "location": location,
        "thread": thread,
        "backtrace": backtrace,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| {
        // Fallback impossible en pratique (types primitifs), mais aucun panic
        // ne doit naître DANS le hook de panic.
        format!("{{\"id\":\"crash\",\"severity\":\"critical\",\"message\":\"serialisation echouee\",\"timestamp_unix_secs\":{timestamp_unix_secs}}}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_payload_is_valid_json_with_required_fields() {
        let json = build_crash_payload(
            "index out of bounds: the len is 3 but the index is 7",
            "crates\\forgia-fps\\src\\lib.rs:612:9",
            "main",
            "   0: std::backtrace_rs::backtrace\n   1: forgia_game::run_game",
            1_780_000_000,
        );
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON valide");
        assert_eq!(parsed["id"], "crash");
        assert_eq!(parsed["severity"], "critical");
        assert!(parsed["message"]
            .as_str()
            .unwrap()
            .contains("index out of bounds"));
        assert!(parsed["location"].as_str().unwrap().contains("lib.rs:612"));
        assert!(parsed["backtrace"].as_str().unwrap().contains("run_game"));
        assert!(!parsed["next_step"].as_str().unwrap().is_empty());
    }

    #[test]
    fn crash_payload_escapes_quotes_and_newlines() {
        let json = build_crash_payload(
            "message avec \"quotes\" et\nretour ligne",
            "loc",
            "main",
            "bt",
            0,
        );
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("JSON valide");
        assert!(parsed["message"].as_str().unwrap().contains("\"quotes\""));
    }
}
