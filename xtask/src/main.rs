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
        "sensor-audit" => sensor_audit(&args),
        "story-gate" => story_gate(&args),
        "no-scaffold" => no_scaffold(&args),
        "asset-load" => asset_load(&args),
        "arch-drift" => arch_drift(),
        "story-ids" => story_ids(),
        "validate-genomes" => validate_genomes(&args),
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
    println!("  sensor-audit [--strict]  Cross-check sensor producers (crates/**/*.rs) vs docs/observability/SENSOR_REGISTRY.md");
    println!("  story-gate [--all-done|--story <id>]   Verify DONE stories claims vs git/code");
    println!("  no-scaffold [--fix]      Fail if any crate is a scaffold (<50 effective LOC or >80% TODO comments). Allowlist in xtask/no-scaffold-allowlist.toml.");
    println!("  asset-load [--fix]       Lock L1 ratchet : fail if asset-load call-sites drift above per-file baseline. Allowlist in xtask/asset-load-allowlist.toml.");
    println!("  arch-drift               Fail si ARCHITECTURE.md ne liste pas exactement les crates members de Cargo.toml (story-593).");
    println!("  story-ids                Fail sur tout NOUVEAU doublon d'ID story dans docs/stories/ (9 collisions historiques grandfathered).");
    println!("  validate-genomes         Gate QA couche 1 : parse tous les assets/genomes/**/*.toml + bornes gènes (min<=default<=max) + ids uniques + cross-refs déclarées.");
}

// ─────────────────────────── validate-genomes (story-619, M0.5 QA) ───────────────────────────

/// Gate de validation du contenu genome (couche 1 du QA intégré, cf
/// docs/plan/rpg-qa-integrated-plan-2026-06-24.md §2). Trois niveaux :
///   L1  — chaque `assets/genomes/**/*.toml` parse en TOML valide (rien ne le
///         vérifiait : une erreur de syntaxe/NBSP ne sortait qu'au runtime).
///   L1b — pour les fichiers à `[[genes]]` : chaque gène a un `id` unique dans le
///         fichier, et si min/max/default sont numériques alors min<=default<=max
///         (classe de bug réelle : default hors bornes = balance cassée).
///   L2  — cross-références inter/intra-fichier DÉCLARÉES explicitement. Jamais
///         inventées : chaque règle est vérifiée green sur le contenu réel avant
///         ajout. Étendre via `check_*` ci-dessous + l'appel dans validate_genomes.
fn validate_genomes(_args: &[String]) -> i32 {
    let root = Path::new("assets/genomes");
    if !root.is_dir() {
        eprintln!(
            "[validate-genomes] FAIL — dossier `assets/genomes` introuvable (lancer depuis la racine workspace ?)"
        );
        return 1;
    }
    let mut files = Vec::new();
    collect_toml(root, &mut files);
    if files.is_empty() {
        eprintln!("[validate-genomes] FAIL — `assets/genomes` ne contient aucun .toml");
        return 1;
    }
    // Tri normalisé `/` : ordre de rapport identique Windows/Linux.
    files.sort_by_cached_key(|p| norm_path(p));

    let mut problems = 0usize;
    let mut gene_count = 0usize;

    for path in &files {
        let rel = norm_path(path);
        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                let hint = if e.kind() == std::io::ErrorKind::InvalidData {
                    "encodage non-UTF-8 (NBSP U+00A0 ?)"
                } else {
                    "fichier illisible (droits ?)"
                };
                eprintln!("[validate-genomes] FAIL lecture {rel} : {hint} ({e})");
                problems += 1;
                continue;
            }
        };
        // L1 — parse TOML
        let value: toml::Value = match toml::from_str(&src) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[validate-genomes] FAIL parse {rel} : {e}");
                problems += 1;
                continue;
            }
        };
        // L1b — validation des gènes [[genes]]
        if let Some(genes) = value.get("genes").and_then(toml::Value::as_array) {
            let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for (i, gene) in genes.iter().enumerate() {
                gene_count += 1;
                let Some(id) = gene.get("id").and_then(toml::Value::as_str) else {
                    eprintln!("[validate-genomes] FAIL {rel} : gène #{i} sans champ `id`");
                    problems += 1;
                    continue;
                };
                if !seen.insert(id) {
                    eprintln!("[validate-genomes] FAIL {rel} : id de gène dupliqué `{id}`");
                    problems += 1;
                }
                // bornes : valeurs finies, puis min<=max (si les 2 présents), puis
                // min<=default<=max (si les 3 présents). Chaque check est indépendant
                // pour ne pas avaler une inversion min>max quand `default` est absent.
                let min = gene.get("min").and_then(toml_num);
                let max = gene.get("max").and_then(toml_num);
                let def = gene.get("default").and_then(toml_num);
                for (field, val) in [("min", min), ("max", max), ("default", def)] {
                    if let Some(v) = val {
                        if !v.is_finite() {
                            eprintln!(
                                "[validate-genomes] FAIL {rel} : gène `{id}` {field} non fini ({v})"
                            );
                            problems += 1;
                        }
                    }
                }
                if let (Some(min), Some(max)) = (min, max) {
                    if min > max {
                        eprintln!(
                            "[validate-genomes] FAIL {rel} : gène `{id}` min ({min}) > max ({max})"
                        );
                        problems += 1;
                    } else if let Some(def) = def {
                        if def < min || def > max {
                            eprintln!(
                                "[validate-genomes] FAIL {rel} : gène `{id}` default ({def}) hors [{min}, {max}]"
                            );
                            problems += 1;
                        }
                    }
                }
            }
        }
    }

    // L2 — cross-refs déclarées (vérifiées green sur le contenu réel le 2026-06-24)
    problems += check_elements_matchup(root);

    if problems == 0 {
        println!(
            "[validate-genomes] OK — {} fichiers parsés, {} gènes validés (bornes + ids uniques), cross-refs elements OK",
            files.len(),
            gene_count
        );
        0
    } else {
        eprintln!(
            "[validate-genomes] FAIL — {problems} problème(s). Corriger avant DONE (gate QA couche 1)."
        );
        1
    }
}

/// L2 : dans `roguelite_elements.toml`, chaque élément mappé sur une arme dans
/// `[mapping]` doit avoir sa table `[matchup.<element>]` (sinon le « fort contre »
/// retombe silencieusement sur le défaut code). Vérifié green 4/4 le 2026-06-24.
/// Retourne le nombre de problèmes trouvés.
fn check_elements_matchup(root: &Path) -> usize {
    let path = root.join("roguelite/roguelite_elements.toml");
    let Ok(src) = fs::read_to_string(&path) else {
        // Absence du fichier = pas une faille de ce gate (peut être retiré volontairement).
        return 0;
    };
    let Ok(value) = toml::from_str::<toml::Value>(&src) else {
        return 0; // fichier invalide — non vérifiable ici, L1 l'a déjà compté en FAIL
    };
    let Some(mapping) = value.get("mapping").and_then(toml::Value::as_table) else {
        // Schéma changé (renommage de [mapping]) : visible mais non bloquant.
        eprintln!(
            "[validate-genomes] WARN roguelite_elements.toml : section [mapping] absente, cross-ref L2 non vérifiée"
        );
        return 0;
    };
    let matchup = value.get("matchup").and_then(toml::Value::as_table);
    let mut problems = 0usize;
    for (weapon, elem) in mapping {
        let Some(elem) = elem.as_str() else { continue };
        if !matchup.is_some_and(|m| m.contains_key(elem)) {
            eprintln!(
                "[validate-genomes] FAIL roguelite_elements.toml : arme `{weapon}` mappée sur élément `{elem}` sans table [matchup.{elem}]"
            );
            problems += 1;
        }
    }
    problems
}

/// f64 depuis un `toml::Value` qu'il soit float (`1.0`) ou integer (`1`).
fn toml_num(v: &toml::Value) -> Option<f64> {
    v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
}

/// Chemin en `/` : messages cliquables + tri identique Windows/Linux.
fn norm_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Collecte récursivement tous les `*.toml` sous `dir`.
fn collect_toml(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_toml(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            out.push(path);
        }
    }
}

// ─────────────────────────── story-ids (story-593 M1.5) ───────────────────────────

/// IDs en collision AVANT la création du gate (audit 2026-06-10). Grandfathered :
/// on ne renumérote jamais une story existante (règle tr-registry). Tout NOUVEAU
/// doublon échoue. Origine : collisions multi-terminal récurrentes (578/579, 588/589).
const GRANDFATHERED_DUP_IDS: &[u32] = &[449, 450, 453, 481, 482, 539, 571, 588, 589];

fn story_ids() -> i32 {
    let dir = Path::new("docs/stories");
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[story-ids] FAIL — docs/stories illisible : {e}");
            return 1;
        }
    };

    let mut by_id: std::collections::BTreeMap<u32, Vec<String>> = std::collections::BTreeMap::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix("story-") else {
            continue;
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if let Ok(id) = digits.parse::<u32>() {
            by_id.entry(id).or_default().push(name);
        }
    }

    let mut new_dups = 0usize;
    for (id, files) in &by_id {
        if files.len() < 2 {
            continue;
        }
        if GRANDFATHERED_DUP_IDS.contains(id) {
            continue;
        }
        new_dups += 1;
        eprintln!("[story-ids] NOUVEAU doublon story-{id} :");
        for f in files {
            eprintln!("    docs/stories/{f}");
        }
    }

    let max_id = by_id.keys().max().copied().unwrap_or(0);
    if new_dups == 0 {
        println!(
            "[story-ids] OK — {} stories, prochain ID libre : story-{}",
            by_id.values().map(Vec::len).sum::<usize>(),
            max_id + 1
        );
        0
    } else {
        eprintln!(
            "[story-ids] FAIL — {new_dups} nouveau(x) doublon(s). Prochain ID libre : story-{}. Renommer la story la plus récente (jamais renuméroter l'ancienne).",
            max_id + 1
        );
        1
    }
}

// ─────────────────────────── arch-drift (story-593 M1.1) ───────────────────────────

/// Gate anti-dérive documentaire : ARCHITECTURE.md §3 doit lister exactement les
/// crates membres du workspace. Origine : audit 2026-06-10 — le doc décrivait
/// 258 crates pour 62 réelles pendant 3 semaines (toxique pour un moteur IA-natif).
fn arch_drift() -> i32 {
    // 1. Membres réels depuis Cargo.toml racine (section [workspace] members).
    let cargo = match fs::read_to_string("Cargo.toml") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[arch-drift] FAIL — Cargo.toml illisible : {e} (lancer depuis la racine workspace)");
            return 1;
        }
    };
    let mut real: Vec<String> = Vec::new();
    let mut in_members = false;
    for line in cargo.lines() {
        let t = line.trim();
        if t.starts_with("members") && t.contains('[') {
            in_members = true;
            continue;
        }
        if in_members {
            if t.starts_with(']') {
                break;
            }
            if let Some(name) = t
                .trim_matches(|c| c == '"' || c == ',' || c == ' ')
                .strip_prefix("crates/")
            {
                real.push(name.to_string());
            }
        }
    }

    // 2. Crates documentées : 1res cellules de tables `| forgia-xxx |` dans ARCHITECTURE.md.
    let arch = match fs::read_to_string("ARCHITECTURE.md") {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[arch-drift] FAIL — ARCHITECTURE.md illisible : {e}");
            return 1;
        }
    };
    let mut documented: Vec<String> = Vec::new();
    for line in arch.lines() {
        if let Some(rest) = line.strip_prefix("| ") {
            if let Some(cell) = rest.split('|').next() {
                let name = cell.trim();
                // Cellule = exactement un nom de crate (pas de prose multi-mots, cf §6 dettes).
                if name.starts_with("forgia-")
                    && !name.contains(' ')
                    && !documented.contains(&name.to_string())
                {
                    documented.push(name.to_string());
                }
            }
        }
    }

    // 3. Diff bidirectionnel.
    let missing_in_doc: Vec<&String> = real.iter().filter(|c| !documented.contains(c)).collect();
    let ghost_in_doc: Vec<&String> = documented.iter().filter(|c| !real.contains(c)).collect();

    if missing_in_doc.is_empty() && ghost_in_doc.is_empty() {
        println!(
            "[arch-drift] OK — ARCHITECTURE.md liste exactement les {} crates du workspace",
            real.len()
        );
        0
    } else {
        for c in &missing_in_doc {
            eprintln!("[arch-drift] MANQUANTE dans ARCHITECTURE.md : {c}");
        }
        for c in &ghost_in_doc {
            eprintln!("[arch-drift] FANTÔME dans ARCHITECTURE.md (crate inexistante) : {c}");
        }
        eprintln!(
            "[arch-drift] FAIL — {} manquante(s), {} fantôme(s). Mettre à jour ARCHITECTURE.md §3.",
            missing_in_doc.len(),
            ghost_in_doc.len()
        );
        1
    }
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
    // Story-528 phase 1 — FPS feel (dash uses, hit feedbacks, aim assist).
    "forgia2_fps_feel.json",
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
    // Cherche le premier token `forgia-X` du header qui correspond à une VRAIE
    // crate (`crates/forgia-X/` existe). Un nom de doc cité — ex.
    // `forgia-gunfire-masterplan-2026-07-01.md` (masterplan référencé en tête de
    // story) — n'est PAS une crate : sans ce check, G3 le prenait pour une crate
    // fantôme (0 LOC) → faux FAIL (2026-07-01, stories 640/641). Scanne toutes les
    // occurrences par ligne (une ligne peut citer doc + crate réelle).
    for line in head.lines().take(30) {
        let mut rest = line;
        while let Some(idx) = rest.find("forgia-") {
            let tail = &rest[idx..];
            let end = tail
                .find(|c: char| !(c.is_alphanumeric() || c == '-'))
                .unwrap_or(tail.len());
            let candidate = &tail[..end];
            if candidate.len() > "forgia-".len()
                && candidate.matches('-').count() >= 1
                && Path::new(&format!("crates/{candidate}")).is_dir()
            {
                return Some(candidate.to_string());
            }
            // Avance après ce token pour scanner les suivants de la même ligne.
            rest = &tail[end..];
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

// ─────────────────────────── sensor-audit (story-546) ───────────────────────────
//
// Cross-check : tous les `"forgia*.json"` écrits dans crates/**/*.rs DOIVENT être
// déclarés dans docs/observability/SENSOR_REGISTRY.md. Et vice versa.
//
// Modes :
//   default : report orphans (produit, non-déclaré) + duplicates ; exit 1 si orphans > 0
//   --strict : report aussi missing (déclaré, jamais produit) ; exit 1 si total > 0
//
// Origine : story-546 (2026-05-28). Motivé par story-545 (diagnostic player invincible
// ralenti par dispersion ~72 sensors sur 25+ crates sans index).

fn sensor_audit(args: &[String]) -> i32 {
    let strict = args.iter().any(|a| a == "--strict");

    let registry_path = Path::new("docs/observability/SENSOR_REGISTRY.md");
    let registry_content = match fs::read_to_string(registry_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "[xtask] sensor-audit: FAIL — cannot read {}: {e}",
                registry_path.display()
            );
            return 1;
        }
    };

    let declared = parse_registry_filenames(&registry_content);
    let (produced, duplicates) = scan_sensor_producers(Path::new("crates"));

    let orphans: Vec<&String> = produced
        .keys()
        .filter(|n| !declared.contains(*n))
        .collect();
    let missing: Vec<&String> = declared
        .iter()
        .filter(|n| !produced.contains_key(*n))
        .collect();

    println!("[xtask] sensor-audit — story-546");
    println!("  declared in registry : {}", declared.len());
    println!("  produced in code     : {}", produced.len());
    println!("  duplicate writers    : {}", duplicates.len());
    println!("  orphans (produced, not declared)  : {}", orphans.len());
    println!("  missing  (declared, not produced) : {}", missing.len());

    if !duplicates.is_empty() {
        println!();
        println!("Duplicate writers (status=duplicate-writer in registry expected) :");
        for (name, sites) in &duplicates {
            println!("  - {name} ({} writers):", sites.len());
            for s in sites {
                println!("      {s}");
            }
        }
    }

    if !orphans.is_empty() {
        println!();
        println!("Orphans — add to SENSOR_REGISTRY.md :");
        let mut sorted: Vec<&&String> = orphans.iter().collect();
        sorted.sort();
        for o in sorted {
            if let Some(sites) = produced.get(*o) {
                println!("  - {o}");
                for s in sites {
                    println!("      {s}");
                }
            }
        }
    }

    if strict && !missing.is_empty() {
        println!();
        println!("Missing — declared in registry but no producer found in crates/ :");
        let mut sorted: Vec<&&String> = missing.iter().collect();
        sorted.sort();
        for m in sorted {
            println!("  - {m}");
        }
    }

    let fail = !orphans.is_empty() || (strict && !missing.is_empty());
    if fail {
        eprintln!();
        eprintln!("[xtask] sensor-audit: FAIL");
        1
    } else {
        println!();
        println!("[xtask] sensor-audit: OK");
        0
    }
}

/// Parse les filenames `forgia*.json` listés dans le registry (markdown table).
/// Look-up des occurrences `` `forgia*_*.json` `` (backticks inline-code).
fn parse_registry_filenames(content: &str) -> std::collections::HashSet<String> {
    let mut set = std::collections::HashSet::new();
    // Match `forgia*.json` ou `forgia2_*.json` à l'intérieur de backticks markdown.
    for line in content.lines() {
        let mut rest = line;
        while let Some(start) = rest.find('`') {
            rest = &rest[start + 1..];
            let Some(end) = rest.find('`') else { break };
            let inner = &rest[..end];
            if inner.starts_with("forgia")
                && inner.ends_with(".json")
                && !inner.contains(' ')
                && !inner.contains('*')
                && !inner.contains('{')
            {
                set.insert(inner.to_string());
            }
            rest = &rest[end + 1..];
        }
    }
    set
}

/// Scan récursif `crates/` pour tout literal `"forgiaX_*.json"` et retourne
/// (sensor_name → liste de `path:line`) + duplicates (≥2 producteurs).
#[allow(clippy::type_complexity)]
fn scan_sensor_producers(
    root: &Path,
) -> (
    std::collections::BTreeMap<String, Vec<String>>,
    std::collections::BTreeMap<String, Vec<String>>,
) {
    let mut producers: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();

    fn visit(
        dir: &Path,
        producers: &mut std::collections::BTreeMap<String, Vec<String>>,
    ) -> std::io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                // Skip target/ et tests/ pour bruit
                let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if name == "target" || name == "tests" {
                    continue;
                }
                visit(&path, producers)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                let Ok(content) = fs::read_to_string(&path) else {
                    continue;
                };
                let lines: Vec<&str> = content.lines().collect();
                let mut matched_lines: Vec<usize> = Vec::new();
                for (idx, line) in lines.iter().enumerate() {
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("//") || trimmed.starts_with("///") {
                        continue;
                    }
                    // Write context detection : literal counts as a write only if
                    // `fs::write` / `write_all` / `serde_json::to_string` apparait
                    // dans une fenêtre de 3 lignes (line + 2 suivantes).
                    let window_end = (idx + 3).min(lines.len());
                    let window = lines[idx..window_end].join(" ");
                    let is_write_ctx = window.contains("fs::write")
                        || window.contains(".write_all")
                        || window.contains("write_atomic")
                        || window.contains("serde_json::to_string");
                    if !is_write_ctx {
                        continue;
                    }
                    matched_lines.push(idx);
                    extract_sensor_literals(line, &path, idx + 1, producers);
                }
                // Passe 2 (story-593 M1.5) : sensors déclarés via const en tête de
                // fichier (`const SENSOR_PATH: &str = "forgia2_x.json";`) et écrits
                // plus loin — la fenêtre 3-lignes les ratait (29 faux « missing »
                // mesurés à l'audit 2026-06-10). On compte le const si son ident est
                // référencé quelque part dans un contexte d'écriture du même fichier.
                for (idx, line) in lines.iter().enumerate() {
                    if matched_lines.contains(&idx) {
                        continue; // déjà compté en passe 1
                    }
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("//") || !line.contains(".json") {
                        continue;
                    }
                    let after = trimmed
                        .trim_start_matches("pub(crate) ")
                        .trim_start_matches("pub ");
                    let Some(decl) = after
                        .strip_prefix("const ")
                        .or_else(|| after.strip_prefix("static "))
                    else {
                        continue;
                    };
                    let Some(ident) = decl.split(':').next().map(str::trim) else {
                        continue;
                    };
                    if ident.is_empty() {
                        continue;
                    }
                    let used_in_write = lines.iter().enumerate().any(|(j, l)| {
                        j != idx && l.contains(ident) && {
                            let we = (j + 3).min(lines.len());
                            let w = lines[j.saturating_sub(1)..we].join(" ");
                            w.contains("fs::write")
                                || w.contains(".write_all")
                                || w.contains("write_atomic")
                                || w.contains("serde_json::to_string")
                        }
                    });
                    if used_in_write {
                        extract_sensor_literals(line, &path, idx + 1, producers);
                    }
                }
            }
        }
        Ok(())
    }

    let _ = visit(root, &mut producers);

    let duplicates: std::collections::BTreeMap<String, Vec<String>> = producers
        .iter()
        .filter(|(_, v)| v.len() >= 2)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    (producers, duplicates)
}

fn extract_sensor_literals(
    line: &str,
    path: &Path,
    lineno: usize,
    out: &mut std::collections::BTreeMap<String, Vec<String>>,
) {
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('"') else { break };
        let inner = &rest[..end];
        if inner.starts_with("forgia")
            && inner.ends_with(".json")
            && !inner.contains(' ')
            && !inner.contains('/')
            && !inner.contains('\\')
            && !inner.contains("{}")
        {
            let site = format!("{}:{lineno}", path.display());
            out.entry(inner.to_string()).or_default().push(site);
        }
        rest = &rest[end + 1..];
    }
}

// ─────────────────────────── asset-load ratchet (story-528, Lock L1) ───────────────────────────
//
// Ratchet anti-drift sur les call-sites de chargement d'asset (GameAssets, Lock L1).
// Cible V2 : <= 30 call-sites. Baseline 2026-05-29 = 69, figée PAR FICHIER dans
// `xtask/asset-load-allowlist.toml`. Le ratchet FAIL si un fichier dépasse son budget
// (régression : nouveau call-site ajouté) ou si un nouveau fichier charge des assets
// sans entrée allowlist.
//
// Détection (receiver-agnostic, fidèle à l'intent L1 = budget de handles) :
//   `.load(` / `.load::<T>(` / `.load_with_settings(` / `.load_folder(` avec parens
//   NON-vides (exclut le faux-positif `.load()` dans un message d'erreur), hors lignes
//   de commentaire, en EXCLUANT les genome configs (`genomes/`, `Genome<`, `.toml`) qui
//   relèvent du pattern data-driven sanctionné, pas du budget de handles GameAssets.
//
// Usage : `cargo xtask asset-load`        → CI gate (exit 1 si drift)
//         `cargo xtask asset-load --fix`  → régénère la baseline (à committer)

const ASSET_LOAD_TARGET: usize = 30; // Cible V2 Lock L1

fn asset_load(args: &[String]) -> i32 {
    let fix = args.iter().any(|a| a == "--fix");

    let crates_dir = Path::new("crates");
    if !crates_dir.exists() {
        eprintln!("ERROR: crates/ directory not found (run from workspace root)");
        return 2;
    }

    let mut counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    scan_asset_loads(crates_dir, &mut counts);
    let total: usize = counts.values().sum();

    if fix {
        write_asset_load_allowlist(&counts, total);
        println!(
            "[xtask] asset-load --fix : baseline written ({} files, {total} call-sites)",
            counts.len()
        );
        println!("  Target V2 (Lock L1) : <= {ASSET_LOAD_TARGET}. Current : {total}.");
        return 0;
    }

    let budgets = load_asset_load_allowlist();
    if budgets.is_empty() {
        eprintln!("[xtask] asset-load: no baseline found.");
        eprintln!("  Run `cargo xtask asset-load --fix` to generate xtask/asset-load-allowlist.toml.");
        return 2;
    }

    let mut violations: Vec<String> = Vec::new();
    for (file, n) in &counts {
        match budgets.get(file) {
            Some(max) if n <= max => {}
            Some(max) => violations.push(format!(
                "  ❌ {file} : {n} asset loads (budget {max}) — NEW call-site(s) added"
            )),
            None => violations.push(format!(
                "  ❌ {file} : {n} asset loads (no allowlist entry) — new file loading assets"
            )),
        }
    }

    let baseline_total: usize = budgets.values().sum();

    println!("[xtask] asset-load — Lock L1 ratchet (story-528)");
    println!("  files loading assets : {}", counts.len());
    println!(
        "  total call-sites     : {total} (baseline {baseline_total}, target <= {ASSET_LOAD_TARGET})"
    );

    if violations.is_empty() {
        println!("✅ asset-load : 0 violations (no drift vs baseline)");
        if total < baseline_total {
            println!(
                "  ℹ️  total dropped {baseline_total} -> {total}. Run `cargo xtask asset-load --fix` to tighten the ratchet."
            );
        }
        0
    } else {
        eprintln!("❌ asset-load : {} violation(s)", violations.len());
        for v in &violations {
            eprintln!("{v}");
        }
        eprintln!();
        eprintln!("Fix : remove the new asset load(s), preload via forgia-assets::GameAssets,");
        eprintln!("or (if legitimate) run `cargo xtask asset-load --fix` to rebaseline + commit.");
        1
    }
}

/// Scan récursif `crates/` et compte les call-sites d'asset-load par fichier
/// (clé = path normalisé en forward-slash pour stabilité cross-platform).
fn scan_asset_loads(root: &Path, counts: &mut std::collections::BTreeMap<String, usize>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "target" {
                continue;
            }
            scan_asset_loads(&path, counts);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let n = content.lines().filter(|l| is_asset_load_line(l)).count();
            if n > 0 {
                let key = path.to_string_lossy().replace('\\', "/");
                *counts.entry(key).or_default() += n;
            }
        }
    }
}

/// Une ligne compte comme asset-load si elle appelle une des 4 formes de `.load`
/// avec des parens non-vides, hors commentaire et hors genome config.
fn is_asset_load_line(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') {
        return false;
    }
    // Genome config loads (Handle<Genome<T>> sur .toml) = pattern data-driven, pas un handle.
    if line.contains("genomes/") || line.contains("Genome<") || line.contains(".toml") {
        return false;
    }
    const FORMS: [&str; 4] = [
        ".load(",
        ".load_with_settings(",
        ".load_folder(",
        ".load::<",
    ];
    for form in FORMS {
        let mut search = line;
        while let Some(idx) = search.find(form) {
            // rest pointe sur le '<' (forme ::<) ou sur le '(' (formes paren).
            let rest = &search[idx + form.len() - 1..];
            let open = if form == ".load::<" {
                rest.find('(')
            } else {
                Some(0)
            };
            if let Some(p) = open {
                let tail = &rest[p..];
                if tail.starts_with('(') && !tail.starts_with("()") {
                    return true;
                }
            }
            search = &search[idx + form.len()..];
        }
    }
    false
}

/// Parse `xtask/asset-load-allowlist.toml` → map (file → budget).
/// Hand-rolled (cohérent avec load_no_scaffold_allowlist, évite la dep toml).
fn load_asset_load_allowlist() -> std::collections::BTreeMap<String, usize> {
    let mut out = std::collections::BTreeMap::new();
    let path = Path::new("xtask/asset-load-allowlist.toml");
    let Ok(content) = fs::read_to_string(path) else {
        return out;
    };
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with('#') || t.is_empty() || t.starts_with('[') {
            continue;
        }
        if let Some((k, v)) = t.split_once('=') {
            let key = k.trim().trim_matches('"').to_string();
            if let Ok(n) = v.trim().parse::<usize>() {
                if !key.is_empty() {
                    out.insert(key, n);
                }
            }
        }
    }
    out
}

/// Réécrit la baseline (trié par count décroissant puis nom).
fn write_asset_load_allowlist(
    counts: &std::collections::BTreeMap<String, usize>,
    total: usize,
) {
    let mut s = String::new();
    s.push_str("# asset-load-allowlist.toml — Lock L1 ratchet baseline (story-528)\n");
    s.push_str("#\n");
    s.push_str("# Per-file budget of asset-load call-sites. CI gate : `cargo xtask asset-load`.\n");
    s.push_str("# Regenerate after removing loads : `cargo xtask asset-load --fix`.\n");
    s.push_str(&format!(
        "# Baseline total : {total} (target V2 Lock L1 : <= {ASSET_LOAD_TARGET}).\n"
    ));
    s.push_str("#\n");
    s.push_str("# Detection : .load( / .load::<> / .load_with_settings( / .load_folder( with\n");
    s.push_str("# non-empty parens, excluding genome configs (genomes/, Genome<, .toml).\n\n");
    s.push_str("[budgets]\n");
    let mut entries: Vec<(&String, &usize)> = counts.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (file, n) in entries {
        s.push_str(&format!("\"{file}\" = {n}\n"));
    }
    if let Err(e) = fs::write("xtask/asset-load-allowlist.toml", s) {
        eprintln!("ERROR writing xtask/asset-load-allowlist.toml : {e}");
    }
}

