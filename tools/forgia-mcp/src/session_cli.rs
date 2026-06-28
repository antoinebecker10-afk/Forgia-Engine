/// forgia-session — CLI pour la gestion des sessions Claude Code multi-terminal.
/// Usage (appelé par les hooks Claude Code) :
///   forgia-session register  [--name <name>]
///   forgia-session release
///   forgia-session list
///   forgia-session claim <task> <scope>    — reserver une tache/scope
///   forgia-session recommend               — suggerer des taches sans conflit

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const SESSIONS_FILE: &str = "D:/Forgia/mcp_bridge/sessions.json";
const HISTORY_FILE:  &str = "D:/Forgia/mcp_bridge/task_history.json";
const BRIDGE_DIR:    &str = "D:/Forgia/mcp_bridge";
const SESSION_TTL:   u64  = 7200; // 2h

/// Taches connues avec leur scope (module) — permet les recommandations auto
const KNOWN_TASKS: &[(&str, &str, &str)] = &[
    ("AUDIT-1",  "cleanup",  "Dead code cleanup (~500 lignes, multi-module)"),
    ("AUDIT-3",  "terrain",  "Local<> terrain streaming (terrain/streaming.rs)"),
    ("AUDIT-12", "ui",       "creation_wheel HashMap cache (ui/creation_wheel.rs)"),
    ("MAJ-CLAUDE", "docs",   "Mise a jour CLAUDE.md stats"),
    ("HARDCODE-COMBAT", "combat", "91 magic numbers combat -> FpsTuning"),
    ("VEILLE",   "veille",   "Veille technologique (docs/veille/)"),
    ("AUDIT-10", "debug",    "Erreurs I/O silencieuses (warn!() manquants)"),
    ("AUDIT-8",  "ui",       "God functions UI (map_designer 903L, new_project 518L)"),
    ("AUDIT-5",  "persistence", "Centraliser chemins GLB (persistence/placed_objects.rs)"),
    ("AUDIT-7",  "terrain",  "Trait ChunkManager (5 managers identiques)"),
];

#[derive(Debug, Serialize, Deserialize, Clone)]
struct SessionEntry {
    pid:        u32,
    name:       String,
    project:    String,
    started_at: u64,
    #[serde(default)]
    task:       String,
    #[serde(default)]
    scope:      String,
    #[serde(default)]
    heartbeat:  u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TaskHistoryEntry {
    pid:        u32,
    name:       String,
    task:       String,
    scope:      String,
    started_at: u64,
    finished_at: u64,
    #[serde(default)]
    result:     String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("list");

    match cmd {
        "register"  => register(),
        "release"   => release(),
        "list"      => list(),
        "claim"     => claim(&args),
        "done"      => done(&args),
        "history"   => history(),
        "recommend" => recommend(),
        other       => eprintln!("Commande inconnue: {other}. Utilise: register | release | list | claim | done | history | recommend"),
    }
}

// ─── register ─────────────────────────────────────────────────────────────────

fn register() {
    let pid     = std::process::id();
    let project = std::env::current_dir()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| "unknown".to_string());
    let name    = std::env::var("TERM_PROGRAM")
        .or_else(|_| std::env::var("SESSIONNAME"))
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| format!("session-{pid}"));
    let now     = epoch_secs();

    let mut sessions = load();
    sessions.retain(|s| now.saturating_sub(s.heartbeat.max(s.started_at)) < SESSION_TTL);

    // Warn on conflict (same project, different PID)
    for conflict in sessions.iter().filter(|s| s.project == project && s.pid != pid) {
        let age = now.saturating_sub(conflict.heartbeat.max(conflict.started_at));
        let task_info = if conflict.task.is_empty() { String::new() }
                       else { format!(" [tache: {}, scope: {}]", conflict.task, conflict.scope) };
        eprintln!(
            "⚠️  CONFLIT DETECTE: '{}' (PID {}) travaille deja sur {} depuis {}s{}",
            conflict.name, conflict.pid, project, age, task_info
        );
    }

    // Update or insert
    if let Some(existing) = sessions.iter_mut().find(|s| s.pid == pid) {
        existing.heartbeat = now;
    } else {
        sessions.push(SessionEntry {
            pid, name: name.clone(), project: project.clone(),
            started_at: now, task: String::new(), scope: String::new(), heartbeat: now,
        });
    }
    save(&sessions);

    println!("Session '{}' (PID {}) enregistree: {}", name, pid, project);
}

// ─── claim ────────────────────────────────────────────────────────────────────

fn claim(args: &[String]) {
    let task  = args.get(2).map(|s| s.as_str()).unwrap_or("");
    let scope = args.get(3).map(|s| s.as_str()).unwrap_or("");

    if task.is_empty() || scope.is_empty() {
        eprintln!("Usage: forgia-session claim <task> <scope>");
        eprintln!("Scopes: terrain, combat, ui, player, ai, effects, debug, persistence, docs, config, cleanup, veille");
        std::process::exit(1);
    }

    let pid = std::process::id();
    let now = epoch_secs();
    let mut sessions = load();
    sessions.retain(|s| now.saturating_sub(s.heartbeat.max(s.started_at)) < SESSION_TTL);

    // Check scope conflict
    for s in &sessions {
        if s.pid != pid && s.scope == scope {
            eprintln!("CONFLIT SCOPE: '{}' (PID {}) travaille deja sur scope '{}' (tache: {})",
                s.name, s.pid, scope, s.task);
            eprintln!("Choisir un autre scope. Taper 'forgia-session recommend' pour des suggestions.");
            std::process::exit(1);
        }
    }

    // Claim
    if let Some(entry) = sessions.iter_mut().find(|s| s.pid == pid) {
        entry.task = task.to_string();
        entry.scope = scope.to_string();
        entry.heartbeat = now;
    } else {
        let name = std::env::var("TERM_PROGRAM")
            .or_else(|_| std::env::var("SESSIONNAME"))
            .unwrap_or_else(|_| format!("session-{pid}"));
        let project = std::env::current_dir()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| "unknown".to_string());
        sessions.push(SessionEntry {
            pid, name, project, started_at: now,
            task: task.to_string(), scope: scope.to_string(), heartbeat: now,
        });
    }
    save(&sessions);
    println!("OK: tache '{}' (scope: {}) reservee pour PID {}", task, scope, pid);
}

// ─── recommend ────────────────────────────────────────────────────────────────

fn recommend() {
    let pid = std::process::id();
    let now = epoch_secs();
    let sessions = load();

    let busy_scopes: Vec<&str> = sessions.iter()
        .filter(|s| s.pid != pid && now.saturating_sub(s.heartbeat.max(s.started_at)) < SESSION_TTL)
        .filter(|s| !s.scope.is_empty())
        .map(|s| s.scope.as_str())
        .collect();

    if busy_scopes.is_empty() {
        println!("Aucun autre terminal actif — toutes les taches sont disponibles.");
    } else {
        println!("Scopes occupes: {}", busy_scopes.join(", "));
    }

    println!("\nTaches recommandees (sans conflit):");
    for (name, scope, desc) in KNOWN_TASKS {
        if !busy_scopes.contains(scope) {
            println!("  -> {} [{}] — {}", name, scope, desc);
        }
    }

    let blocked: Vec<&&str> = KNOWN_TASKS.iter()
        .filter(|(_, scope, _)| busy_scopes.contains(scope))
        .map(|(name, _, _)| name)
        .collect();
    if !blocked.is_empty() {
        println!("\nTaches bloquees (scope occupe):");
        for name in blocked {
            println!("  X {}", name);
        }
    }
}

// ─── done ─────────────────────────────────────────────────────────────────────

fn done(args: &[String]) {
    let task_name = args.get(2).map(|s| s.as_str()).unwrap_or("");
    let result = args.get(3).map(|s| s.as_str()).unwrap_or("completed");
    let now = epoch_secs();
    let mut sessions = load();

    // Find by task name, or by PID if no name given
    let pid = std::process::id();
    let entry = if !task_name.is_empty() {
        sessions.iter().find(|s| s.task.eq_ignore_ascii_case(task_name))
    } else {
        sessions.iter().find(|s| s.pid == pid && !s.task.is_empty())
    };
    if entry.is_none() {
        let hint = if task_name.is_empty() { "Donne le nom: forgia-session done <task> [result]".to_string() }
                  else { format!("Tache '{}' introuvable dans les sessions actives.", task_name) };
        eprintln!("{hint}");
        std::process::exit(1);
    }
    let entry = entry.unwrap().clone();

    // Add to history
    let history_entry = TaskHistoryEntry {
        pid: entry.pid,
        name: entry.name.clone(),
        task: entry.task.clone(),
        scope: entry.scope.clone(),
        started_at: entry.started_at,
        finished_at: now,
        result: result.to_string(),
    };
    let mut hist = load_history();
    hist.push(history_entry);
    // Keep last 100 entries
    if hist.len() > 100 { hist.drain(..hist.len() - 100); }
    save_history(&hist);

    // Clear task from session (terminal stays registered but idle)
    if let Some(s) = sessions.iter_mut().find(|s| s.task == entry.task) {
        let duration = now.saturating_sub(s.started_at);
        println!("DONE: '{}' [{}] termine en {}m — {}", s.task, s.scope, duration / 60, result);
        s.task.clear();
        s.scope.clear();
        s.heartbeat = now;
    }
    save(&sessions);
}

// ─── history ──────────────────────────────────────────────────────────────────

fn history() {
    let hist = load_history();
    if hist.is_empty() {
        println!("Aucun historique de taches.");
        return;
    }
    let now = epoch_secs();
    println!("{} tache(s) dans l'historique:", hist.len());
    // Show most recent first (last 20)
    for h in hist.iter().rev().take(20) {
        let duration = h.finished_at.saturating_sub(h.started_at);
        let ago = now.saturating_sub(h.finished_at);
        let ago_str = if ago > 3600 { format!("{}h ago", ago / 3600) }
                     else { format!("{}m ago", ago / 60) };
        println!("  {:15} | {:20} [{}] | {}m | {} | {}",
            h.name, h.task, h.scope, duration / 60, h.result, ago_str);
    }
}

// ─── release ──────────────────────────────────────────────────────────────────

fn release() {
    let pid = std::process::id();
    let mut sessions = load();
    let before = sessions.len();
    sessions.retain(|s| s.pid != pid);
    save(&sessions);
    println!("{} session(s) liberee(s) pour PID {pid}", before - sessions.len());
}

// ─── list ─────────────────────────────────────────────────────────────────────

fn list() {
    let sessions = load();
    if sessions.is_empty() {
        println!("Aucune session active.");
        return;
    }
    let now = epoch_secs();
    println!("{} session(s) active(s):", sessions.len());
    for s in &sessions {
        let age = now.saturating_sub(s.heartbeat.max(s.started_at));
        let task_info = if s.task.is_empty() { "(libre)".to_string() }
                       else { format!("{} [{}]", s.task, s.scope) };
        println!("  PID {:6} | {:15} | {:30} | {} | {}s",
            s.pid, s.name, task_info, s.project, age);
    }
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn epoch_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn load() -> Vec<SessionEntry> {
    std::fs::read_to_string(SESSIONS_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(sessions: &[SessionEntry]) {
    let _ = std::fs::create_dir_all(BRIDGE_DIR);
    if let Ok(json) = serde_json::to_string_pretty(sessions) {
        let _ = std::fs::write(SESSIONS_FILE, json);
    }
}

fn load_history() -> Vec<TaskHistoryEntry> {
    std::fs::read_to_string(HISTORY_FILE)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_history(history: &[TaskHistoryEntry]) {
    let _ = std::fs::create_dir_all(BRIDGE_DIR);
    if let Ok(json) = serde_json::to_string_pretty(history) {
        let _ = std::fs::write(HISTORY_FILE, json);
    }
}
