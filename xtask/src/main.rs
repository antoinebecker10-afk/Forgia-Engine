//! # xtask — Forgia V2 automation
//!
//! Tasks :
//! - `check-orphans` : détecte plugins définis non wirés, sensors sans producteur, fields FpsTuning jamais lus
//! - `schedule-dump` : dump Bevy schedule en .dot/.svg pour audit GameSet ordering
//! - `baseline-e1-e2` : génère asset_load_whitelist.txt baseline
//! - `verify-sensors-format` : CI gate validation 13 forgia2_*.json canoniques + format conforme

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");

    let exit_code = match cmd {
        "check-orphans" => {
            check_orphans();
            0
        }
        "schedule-dump" => {
            schedule_dump();
            0
        }
        "baseline-e1-e2" => {
            baseline_e1_e2();
            0
        }
        "verify-sensors-format" => verify_sensors_format(),
        "story-gate" => story_gate(&args),
        "no-scaffold" => no_scaffold(&args),
        _ => {
            print_help();
            0
        }
    };

    std::process::exit(exit_code);
}

fn print_help() {
    println!("xtask — Forgia V2 automation");
    println!();
    println!("Commands :");
    println!("  check-orphans            Detect plugins/sensors/fields orphans");
    println!("  schedule-dump            Dump Bevy schedule for GameSet audit");
    println!("  baseline-e1-e2           Regenerate asset_load_whitelist.txt baseline");
    println!("  verify-sensors-format    Validate forgia2_*.json canonical sensors");
    println!("  story-gate [--all-done|--story <id>]   Verify DONE stories claims vs git/code");
    println!("  no-scaffold [--fix]      Fail if any crate is a scaffold (<50 effective LOC or >80% TODO comments). Allowlist in xtask/no-scaffold-allowlist.toml.");
}

fn check_orphans() {
    println!("[xtask] check-orphans — Phase 0 placeholder");
    // Phase 5 : scan workspace pour `impl Plugin for X` vs `add_plugins(X)` diff.
}

fn schedule_dump() {
    println!("[xtask] schedule-dump — Phase 0 placeholder");
    // Phase 1+ : utilise bevy_mod_debugdump si nécessaire.
}

fn baseline_e1_e2() {
    println!("[xtask] baseline-e1-e2 — Phase 0 placeholder");
    // Phase 3 : scan asset_server.load() call-sites + génère whitelist.
}

// ─────────────────────────── verify-sensors-format (Vague 5 Phase 5b/5c) ───────────────────────────

/// Liste canonique forgia2_*.json attendus à la racine workspace (cible Phase 5).
///
/// Phase 5b Session A — 2 sensors livrés (health + rpg_health renames).
/// Phase 5b Session A étape 2/3 — ajoute arena + chunks + combat (5 total).
/// Phase 5b Session B/C — ajoute perf + entities + memory + lifecycle + watchdog
///   + audio + input + sensor_health (13 total final).
///
/// Le binary tolère les sensors manquants pour ne pas bloquer CI pendant migration
/// progressive. Mode strict (`--strict` flag futur) vérifierait count == 13 exact.
const CANONICAL_SENSORS: &[&str] = &[
    // Tier 0 (Session A Étape 1, story-457+rename) — health sensors
    "forgia2_health.json",
    "forgia2_rpg_health.json",
    // Tier 1 (story-465 sensor fusion) — gameplay aggregators
    "forgia2_arena.json",
    "forgia2_combat.json",
    // Tier 1bis (Session A étape 3 — futur) :
    // "forgia2_chunks.json",
    // Tier 2 (Session B — story-467 DONE)
    "forgia2_perf.json",
    "forgia2_entities.json",
    "forgia2_memory.json",
    // Tier 2 (Session C — Story-469 DONE)
    "forgia2_lifecycle.json",
    "forgia2_watchdog.json",
    "forgia2_audio.json",
    "forgia2_input.json",
    "forgia2_sensor_health.json",
    // V7 M1 (Story-470 DONE) — 13e sensor canonique, cible 13/13 atteinte
    "forgia2_roguelite_state.json",
];

const VALID_SEVERITIES: &[&str] = &["ok", "warn", "critical", "info"];

/// Vérifie présence + format des sensors canoniques à la racine workspace.
/// Returns exit code (0 = success, 1 = fail).
fn verify_sensors_format() -> i32 {
    let workspace_root = Path::new(".");
    let mut errors: Vec<String> = Vec::new();
    let mut verified = 0;

    for sensor_name in CANONICAL_SENSORS {
        let path = workspace_root.join(sensor_name);
        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(val) => {
                    let Some(obj) = val.as_object() else {
                        errors.push(format!("{sensor_name}: not a JSON object"));
                        continue;
                    };
                    let mut sensor_ok = true;
                    for required in ["id", "severity", "next_step"] {
                        if !obj.contains_key(required) {
                            errors.push(format!("{sensor_name}: missing '{required}' field"));
                            sensor_ok = false;
                        }
                    }
                    if let Some(sev) = obj.get("severity").and_then(|v| v.as_str()) {
                        if !VALID_SEVERITIES.contains(&sev) {
                            errors.push(format!(
                                "{sensor_name}: invalid severity '{sev}' (expected: {VALID_SEVERITIES:?})"
                            ));
                            sensor_ok = false;
                        }
                    }
                    if sensor_ok {
                        verified += 1;
                    }
                }
                Err(e) => errors.push(format!("{sensor_name}: invalid JSON ({e})")),
            },
            Err(_) => errors.push(format!("{sensor_name}: file not found at workspace root")),
        }
    }

    if errors.is_empty() {
        println!(
            "[xtask] verify-sensors-format: OK ({verified}/{} canonical sensors validated)",
            CANONICAL_SENSORS.len()
        );
        0
    } else {
        eprintln!(
            "[xtask] verify-sensors-format: FAIL ({} errors, {verified}/{} passed)",
            errors.len(),
            CANONICAL_SENSORS.len()
        );
        for err in &errors {
            eprintln!("  - {err}");
        }
        1
    }
}

// ─────────────────────────── story-gate (story-495) ───────────────────────────
//
// Vérifie que les stories marquées DONE correspondent à la réalité git/code.
// Origine : audit 2026-05-21 → 7/9 stories du batch 471-479 fictives.
// Voir story-495 + .claude/rules/story-done-gate.md.
//
// Gates appliqués :
//   G1 git-tracked : `git ls-files <story-path>` retourne le fichier
//   G3 crate-LOC   : si story mentionne `forgia-X`, alors crates/forgia-X total LOC > 50
//   G4 tests-count : si story claim "N tests verts", grep '#[test]' retourne >= N hits
//
// G2 (committed recent) et G5 (AC cochés) et G6 (memory cross-check) reportés.

const STORY_GATE_LOC_THRESHOLD: usize = 50;
const STORY_GATE_SKIP_LIST: &[&str] = &[
    // Stories sans crate dédiée (orchestration, docs, multi-crate) — exemptées G3/G4
    "story-441-spawn-village-v1",
    "story-447-village-terrain-leveling-debug",
    "story-450-wave5-phase3-audit",
    "story-486-jolcham-oak-bark-wireup",
    // Multi-crate orchestration : story-483 livre forgia-mode-roguelite + forgia-stage-arena
    // (88 tests = somme des 2 crates ~38 + 58 = 96), gate single-crate sous-compte.
    "story-483-roguelite-stage-arena-foundations",
];

#[derive(Debug)]
struct StoryGateResult {
    story_id: String,
    file_name: String,
    #[allow(dead_code)]
    status_done: bool,
    g1_tracked: Option<bool>,
    g3_crate: Option<(String, usize, bool)>, // (crate_name, loc, pass)
    g4_tests: Option<(usize, usize, bool)>,  // (claimed, actual, pass)
    pass: bool,
    notes: Vec<String>,
}

fn story_gate(args: &[String]) -> i32 {
    let mode = args.get(2).map(String::as_str).unwrap_or("--all-done");
    let story_filter: Option<String> = if mode == "--story" {
        args.get(3).cloned()
    } else {
        None
    };

    let stories_dir = Path::new("docs/stories");
    let Ok(entries) = fs::read_dir(stories_dir) else {
        eprintln!("[xtask] story-gate: cannot read docs/stories/");
        return 1;
    };

    let mut results: Vec<StoryGateResult> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if !file_name.starts_with("story-") {
            continue;
        }
        if let Some(ref filter) = story_filter {
            if !file_name.contains(filter) {
                continue;
            }
        }

        let story_id = file_name
            .strip_prefix("story-")
            .and_then(|s| s.split('-').next())
            .unwrap_or("?")
            .to_string();
        let stem = file_name.trim_end_matches(".md").to_string();

        let content = fs::read_to_string(&path).unwrap_or_default();
        let head = content.lines().take(80).collect::<Vec<_>>().join("\n");

        let status_done = detect_done(&head);
        if mode == "--all-done" && !status_done {
            continue;
        }

        let mut notes: Vec<String> = Vec::new();
        let skipped = STORY_GATE_SKIP_LIST.iter().any(|s| stem.contains(s));
        if skipped {
            notes.push("skip-list — gate G3/G4 exemptés (orchestration story)".into());
        }

        // G1 — git-tracked
        let g1_tracked = check_git_tracked(&path);
        // G3 — crate LOC
        let crate_name = extract_crate_name(&head);
        let g3_crate = match (&crate_name, skipped) {
            (Some(name), false) => {
                let loc = total_loc(&format!("crates/{name}/src"));
                Some((name.clone(), loc, loc > STORY_GATE_LOC_THRESHOLD))
            }
            _ => None,
        };
        // G4 — tests count
        let g4_tests = match (&crate_name, claimed_tests(&head), skipped) {
            (Some(name), Some(claimed), false) => {
                let actual = count_tests(&format!("crates/{name}/src"));
                Some((claimed, actual, actual >= claimed))
            }
            _ => None,
        };

        let pass = g1_tracked.unwrap_or(false)
            && g3_crate.as_ref().map(|t| t.2).unwrap_or(true)
            && g4_tests.as_ref().map(|t| t.2).unwrap_or(true);

        results.push(StoryGateResult {
            story_id,
            file_name,
            status_done,
            g1_tracked,
            g3_crate,
            g4_tests,
            pass,
            notes,
        });
    }

    results.sort_by(|a, b| a.story_id.cmp(&b.story_id));

    let total = results.len();
    let passed = results.iter().filter(|r| r.pass).count();
    let failed = total - passed;

    println!("[xtask] story-gate — {passed}/{total} DONE stories pass all gates");
    println!();
    for r in &results {
        let icon = if r.pass { "✅" } else { "🚨" };
        println!("{icon} story-{}", r.story_id);
        println!("   file:   {}", r.file_name);
        if let Some(t) = r.g1_tracked {
            println!(
                "   G1 git-tracked: {}",
                if t {
                    "PASS"
                } else {
                    "FAIL — file is ?? (untracked)"
                }
            );
        }
        if let Some((name, loc, ok)) = &r.g3_crate {
            println!(
                "   G3 crate-LOC:   {} crates/{name}/src = {loc} LOC (threshold {STORY_GATE_LOC_THRESHOLD})",
                if *ok { "PASS" } else { "FAIL" }
            );
        }
        if let Some((claim, actual, ok)) = &r.g4_tests {
            println!(
                "   G4 tests:       {} claim={claim}, actual={actual}",
                if *ok { "PASS" } else { "FAIL" }
            );
        }
        for n in &r.notes {
            println!("   note: {n}");
        }
    }

    if failed > 0 {
        eprintln!();
        eprintln!("[xtask] story-gate: {failed} story(ies) FAILED — see above");
        1
    } else {
        0
    }
}

fn detect_done(head: &str) -> bool {
    head.lines().any(|l| {
        let lc = l.to_lowercase();
        (lc.contains("statut") || lc.contains("status"))
            && (l.contains("✅") || lc.contains(": done") || lc.contains("**done"))
    })
}

fn check_git_tracked(path: &Path) -> Option<bool> {
    let out = Command::new("git")
        .arg("ls-files")
        .arg("--error-unmatch")
        .arg(path)
        .output()
        .ok()?;
    Some(out.status.success())
}

fn extract_crate_name(head: &str) -> Option<String> {
    // Cherche premier token `forgia-X` dans le header (title H1 + premières lignes)
    for line in head.lines().take(30) {
        if let Some(idx) = line.find("forgia-") {
            let tail = &line[idx..];
            let end = tail
                .find(|c: char| !(c.is_alphanumeric() || c == '-'))
                .unwrap_or(tail.len());
            let candidate = &tail[..end];
            if candidate.len() > "forgia-".len() && candidate.matches('-').count() >= 1 {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn claimed_tests(head: &str) -> Option<usize> {
    // Détecte explicitement les patterns "N/N tests" ou "N tests verts/passing/livrés"
    // Évite faux positifs sur "2h prévu", "3 fichiers", etc.
    for line in head.lines() {
        let lc = line.to_lowercase();
        if !lc.contains("test") {
            continue;
        }
        // Pattern fort : "N/N tests" ou "N/N tests verts"
        for token in line.split_whitespace() {
            if let Some((a, b)) = token.split_once('/') {
                if let (Ok(n), Ok(m)) = (a.parse::<usize>(), b.parse::<usize>()) {
                    if n == m && (1..=500).contains(&n) {
                        return Some(n);
                    }
                }
            }
        }
        // Pattern faible : "N tests" mais SEULEMENT si suivi de "verts|passing|livrés|claim"
        // pour éviter "2 tests `fn` to refactor"
        let lower = line.to_lowercase();
        let tokens: Vec<&str> = lower.split_whitespace().collect();
        for i in 0..tokens.len().saturating_sub(2) {
            if let Ok(n) = tokens[i].parse::<usize>() {
                if (1..=500).contains(&n) && tokens[i + 1].starts_with("test") {
                    let next2 = tokens.get(i + 2).copied().unwrap_or("");
                    if next2.starts_with("vert")
                        || next2.starts_with("pass")
                        || next2.starts_with("livré")
                        || next2.contains("clippy")
                    {
                        return Some(n);
                    }
                }
            }
        }
    }
    None
}

fn total_loc(dir: &str) -> usize {
    let path = Path::new(dir);
    if !path.exists() {
        return 0;
    }
    let mut total = 0;
    walk_loc(path, &mut total);
    total
}

fn walk_loc(p: &Path, total: &mut usize) {
    let Ok(entries) = fs::read_dir(p) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_loc(&path, total);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(s) = fs::read_to_string(&path) {
                *total += s.lines().count();
            }
        }
    }
}

fn count_tests(dir: &str) -> usize {
    let path = Path::new(dir);
    if !path.exists() {
        return 0;
    }
    let mut total = 0;
    walk_test_count(path, &mut total);
    total
}

fn walk_test_count(p: &Path, total: &mut usize) {
    let Ok(entries) = fs::read_dir(p) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_test_count(&path, total);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(s) = fs::read_to_string(&path) {
                *total += s.matches("#[test]").count();
            }
        }
    }
}

// ─────────────────────────── no-scaffold (story-515) ───────────────────────────
//
// Story-512+513 ont supprime 99 crates scaffolds. Cette commande empeche la
// regression : fail si une crate `crates/forgia-*` a moins de 50 LOC
// effectives (non-blank, non-comment) OU si plus de 80% de ses lignes
// non-blanches sont des TODO comments.
//
// Allowlist : `xtask/no-scaffold-allowlist.toml` pour les foundation crates
// legitimes (forgia-core 121 LOC, forgia-rng, etc.).
//
// Usage CI : `cargo xtask no-scaffold` exit code != 0 si violations.

fn no_scaffold(_args: &[String]) -> i32 {
    println!("[xtask] no-scaffold — checking workspace for scaffold crates");

    let allowlist = load_no_scaffold_allowlist();
    println!("  Allowlist : {} entries (skip)", allowlist.len());

    let crates_dir = Path::new("crates");
    if !crates_dir.exists() {
        eprintln!("ERROR: crates/ directory not found (run from workspace root)");
        return 2;
    }

    let mut violations: Vec<String> = Vec::new();
    let mut checked = 0usize;

    let entries = match fs::read_dir(crates_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("ERROR: cannot read crates/ : {e}");
            return 2;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.starts_with("forgia-") {
            continue;
        }
        if allowlist.contains(&name) {
            continue;
        }

        let src_dir = path.join("src");
        if !src_dir.exists() {
            continue;
        }

        let (effective, todo) = count_effective_and_todo(&src_dir);
        checked += 1;

        if effective < 50 {
            violations.push(format!(
                "  ❌ {name} : {effective} effective LOC (< 50). Implement or add to allowlist."
            ));
            continue;
        }
        if effective > 0 {
            let todo_pct = (todo * 100) / effective;
            if todo_pct > 80 {
                violations.push(format!(
                    "  ❌ {name} : {todo_pct}% TODO comments ({todo}/{effective}). Implement or remove."
                ));
            }
        }
    }

    println!("  Checked {checked} crates (allowlist skipped)");

    if violations.is_empty() {
        println!("✅ no-scaffold : 0 violations");
        0
    } else {
        eprintln!("❌ no-scaffold : {} violations", violations.len());
        for v in &violations {
            eprintln!("{v}");
        }
        eprintln!();
        eprintln!("Fix : implement the crate, or add to `xtask/no-scaffold-allowlist.toml` with justification.");
        1
    }
}

fn load_no_scaffold_allowlist() -> Vec<String> {
    let path = Path::new("xtask/no-scaffold-allowlist.toml");
    if !path.exists() {
        return Vec::new();
    }
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    // Minimal TOML parse : look for `allowed = ["a", "b", ...]` line.
    // Avoid adding `toml` crate dep complexity for this single use.
    let mut out = Vec::new();
    let mut in_array = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("allowed") && trimmed.contains('[') {
            in_array = true;
        }
        if in_array {
            for tok in trimmed.split([',', '[', ']']) {
                let s = tok.trim().trim_matches('"').trim_matches('\'');
                if !s.is_empty() && !s.starts_with("allowed") && !s.contains('=') {
                    out.push(s.to_string());
                }
            }
            if trimmed.contains(']') {
                in_array = false;
            }
        }
    }
    out
}

/// Returns (effective_loc, todo_loc).
/// effective = lines non-blank non-pure-comment.
/// todo = lines containing TODO / FIXME / XXX markers.
fn count_effective_and_todo(dir: &Path) -> (usize, usize) {
    let mut effective = 0usize;
    let mut todo = 0usize;
    walk_loc_and_todo(dir, &mut effective, &mut todo);
    (effective, todo)
}

fn walk_loc_and_todo(p: &Path, effective: &mut usize, todo: &mut usize) {
    let entries = match fs::read_dir(p) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_loc_and_todo(&path, effective, todo);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(s) = fs::read_to_string(&path) {
                for line in s.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.starts_with("//")
                        || trimmed.starts_with("/*")
                        || trimmed.starts_with('*')
                    {
                        // Comment line — does it contain TODO marker ?
                        if trimmed.contains("TODO")
                            || trimmed.contains("FIXME")
                            || trimmed.contains("XXX")
                        {
                            *todo += 1;
                        }
                        continue;
                    }
                    *effective += 1;
                    if trimmed.contains("TODO")
                        || trimmed.contains("FIXME")
                        || trimmed.contains("XXX")
                    {
                        *todo += 1;
                    }
                }
            }
        }
    }
}
