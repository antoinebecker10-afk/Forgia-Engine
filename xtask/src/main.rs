//! # xtask — Forgia V2 automation
//!
//! Tasks :
//! - `check-orphans` : détecte plugins définis non wirés, sensors sans producteur, fields FpsTuning jamais lus
//! - `schedule-dump` : dump Bevy schedule en .dot/.svg pour audit GameSet ordering
//! - `baseline-e1-e2` : génère asset_load_whitelist.txt baseline
//! - `verify-sensors-format` : CI gate validation 13 forgia2_*.json canoniques + format conforme
//! - `validate-pcg` : valide les contrats PCG publiables (content-spec + kit-manifest)

use forgia_pcg_core::{ContentSpec, KitManifest, PcgRegistryLock};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");

    let exit_code = match cmd {
        "check-orphans" => check_orphans(&args),
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
        "sensor-sync-writes" => sensor_sync_writes(&args),
        "capteur-gate" => capteur_gate(&args),
        "strates" => strates(&args),
        "story-gate" => story_gate(&args),
        "no-scaffold" => no_scaffold(&args),
        "context-budget" => context_budget(&args),
        "asset-load" => asset_load(&args),
        "arch-drift" => arch_drift(),
        "plugin-gate" => plugin_gate(&args),
        "deps-mortes" => deps_mortes(&args),
        "story-ids" => story_ids(),
        "validate-genomes" => validate_genomes(&args),
        "validate-pcg" => validate_pcg(&args),
        "story-index" => story_index(&args),
        "wip-check" => wip_check(&args),
        "dist-roguelite" => dist_roguelite(&args),
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
    println!(
        "  check-orphans [--strict] Report workspace crates without an internal Cargo consumer; --strict fails on unexpected ones"
    );
    println!("  schedule-dump            Dump Bevy schedule for GameSet audit");
    println!("  baseline-e1-e2           Regenerate asset_load_whitelist.txt baseline");
    println!("  verify-sensors-format    Validate forgia2_*.json canonical sensors");
    println!("  sensor-audit [--strict]  Cross-check sensor producers (crates/**/*.rs) vs docs/observability/SENSOR_REGISTRY.md");
    println!("  sensor-sync-writes [--strict] Report synchronous sensor JSON writes; --strict fails if any remain");
    println!("  story-gate [--all-done|--story <id>]   Verify DONE stories claims vs git/code");
    println!("  no-scaffold [--fix]      Fail if any crate is a scaffold (<50 effective LOC or >80% TODO comments). Allowlist in xtask/no-scaffold-allowlist.toml.");
    println!("  context-budget           Fail if CLAUDE.md + .claude/rules/ exceed the per-session context budget (measured 2026-08-09 : ~27k tokens).");
    println!("  asset-load [--fix]       Lock L1 ratchet : fail if asset-load call-sites drift above per-file baseline. Allowlist in xtask/asset-load-allowlist.toml.");
    println!("  arch-drift               Fail si ARCHITECTURE.md ne liste pas exactement les crates members de Cargo.toml (story-593).");
    println!("  story-ids                Fail sur tout NOUVEAU doublon d'ID story dans docs/stories/ (9 collisions historiques grandfathered).");
    println!("  validate-genomes         Gate QA couche 1 : parse tous les assets/genomes/**/*.toml + bornes gènes (min<=default<=max) + ids uniques + cross-refs déclarées.");
    println!("  validate-pcg [--root <dir>] [--require-any]  Valide les content-spec et kit-manifest PCG publiables (assets/pcg par défaut).");
    println!("  story-index [--check]    Régénère docs/stories/_index.md (board unique, statuts normalisés). --check = gate anti-drift (fail si périmé).");
    println!("  wip-check                Fail si > 3 stories IN_PROGRESS (limite WIP — « stop starting, start finishing »).");
    println!("  dist-roguelite [--out <dir>] [--full] [--skip-build]   Assemble le build joueur Roguelite (exe + assets élagués + config) prêt à pousser sur itch/butler.");
}

// ───────────────────────────── validate-pcg ─────────────────────────────

/// Gate de publication PCG. Elle ne devine pas l'intention depuis le nom du
/// fichier : le discriminant est exclusivement `schema_version`. Ainsi un TOML
/// non-PCG voisin reste hors périmètre, tandis qu'un contrat PCG inconnu échoue
/// avant d'arriver dans Blender ou Bevy.
fn validate_pcg(args: &[String]) -> i32 {
    let mut root = PathBuf::from("assets/pcg");
    let mut require_any = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--root" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("[validate-pcg] FAIL — --root exige un chemin");
                    return 2;
                };
                root = PathBuf::from(value);
                index += 1;
            }
            "--require-any" => require_any = true,
            other => {
                eprintln!("[validate-pcg] FAIL — argument inconnu `{other}`");
                return 2;
            }
        }
        index += 1;
    }

    if !root.is_dir() {
        if require_any {
            eprintln!(
                "[validate-pcg] FAIL — aucun dossier PCG à publier : {}",
                norm_path(&root)
            );
            return 1;
        }
        println!(
            "[validate-pcg] SKIP — {} absent (aucun contrat PCG publié)",
            norm_path(&root)
        );
        return 0;
    }

    let mut files = Vec::new();
    collect_toml(&root, &mut files);
    files.sort_by_cached_key(|path| norm_path(path));
    let mut content_specs = 0;
    let mut manifests = 0;
    let mut lockfiles = 0;
    let mut failures = 0;

    for file in files {
        let source = match fs::read_to_string(&file) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("[validate-pcg] FAIL lecture {} : {error}", norm_path(&file));
                failures += 1;
                continue;
            }
        };
        let table = match source.parse::<toml::Table>() {
            Ok(table) => table,
            Err(error) => {
                eprintln!("[validate-pcg] FAIL TOML {} : {error}", norm_path(&file));
                failures += 1;
                continue;
            }
        };
        let schema = table
            .get("schema_version")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        match schema {
            forgia_pcg_core::CONTENT_SPEC_SCHEMA_V1 => match ContentSpec::parse_toml(&source) {
                Ok(_) => content_specs += 1,
                Err(error) => {
                    eprintln!(
                        "[validate-pcg] FAIL content-spec {} : {error}",
                        norm_path(&file)
                    );
                    failures += 1;
                }
            },
            forgia_pcg_core::kit::KIT_MANIFEST_SCHEMA_V1 => {
                match KitManifest::parse_toml(&source) {
                    Ok(_) => manifests += 1,
                    Err(error) => {
                        eprintln!(
                            "[validate-pcg] FAIL kit-manifest {} : {error}",
                            norm_path(&file)
                        );
                        failures += 1;
                    }
                }
            }
            forgia_pcg_core::registry::REGISTRY_LOCK_SCHEMA_V1 => {
                match PcgRegistryLock::parse_toml(&source) {
                    Ok(lock) => {
                        lockfiles += 1;
                        failures += verify_lock_hashes(&file, &lock);
                    }
                    Err(error) => {
                        eprintln!(
                            "[validate-pcg] FAIL registry lock {} : {error}",
                            norm_path(&file)
                        );
                        failures += 1;
                    }
                }
            }
            value if value.starts_with("forgia.") && value.contains("/v") => {
                eprintln!(
                    "[validate-pcg] FAIL schema PCG inconnu `{value}` dans {}",
                    norm_path(&file)
                );
                failures += 1;
            }
            _ => {}
        }
    }

    let validated = content_specs + manifests + lockfiles;
    if require_any && validated == 0 {
        eprintln!("[validate-pcg] FAIL — aucun content-spec ou kit-manifest trouvé");
        return 1;
    }
    println!(
        "[validate-pcg] content-specs={content_specs} kit-manifests={manifests} registry-locks={lockfiles} failures={failures}"
    );
    i32::from(failures != 0)
}

/// Publisher PCG : recalcule le SHA-256 de chaque manifest référencé par un
/// lockfile et refuse toute divergence — une seed ne reproduit un monde que si
/// les contenus résolus sont exactement ceux figés (`forgia-pcg-core::registry`
/// ne vérifie que la forme ; l'IO/hash appartient au publisher, donc ici).
/// Hash sur les octets bruts : les TOML PCG sont écrits en LF par l'outillage ;
/// si un checkout CRLF cassait ce hash, normaliser via .gitattributes, pas ici.
fn verify_lock_hashes(lock_path: &Path, lock: &PcgRegistryLock) -> usize {
    let root = lock_path.parent().unwrap_or_else(|| Path::new("."));
    let mut failures = 0;
    for entry in &lock.entries {
        let manifest = entry.manifest.as_str();
        if Path::new(manifest).is_absolute() || manifest.split(['/', '\\']).any(|part| part == "..")
        {
            eprintln!(
                "[validate-pcg] FAIL lock {} : entrée `{}` — chemin manifest hors racine `{manifest}`",
                norm_path(lock_path),
                entry.id
            );
            failures += 1;
            continue;
        }
        let path = root.join(manifest);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!(
                    "[validate-pcg] FAIL lock {} : entrée `{}` — manifest `{}` illisible : {error}",
                    norm_path(lock_path),
                    entry.id,
                    norm_path(&path)
                );
                failures += 1;
                continue;
            }
        };
        let actual = format!("{:x}", Sha256::digest(&bytes));
        let expected = entry
            .content_hash
            .strip_prefix("sha256:")
            .unwrap_or(entry.content_hash.as_str());
        if actual != expected {
            eprintln!(
                "[validate-pcg] FAIL lock {} : entrée `{}` — hash divergent (déclaré sha256:{expected}, réel sha256:{actual})",
                norm_path(lock_path),
                entry.id
            );
            failures += 1;
        }
    }
    failures
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
/// Plugins montés par AUCUN `App::new()` de test, et pourquoi c'est admis.
///
/// Une exception NOMMÉE vaut mieux qu'un périmètre implicite : la liste ci-dessous
/// se lit, se discute et se réduit. Un plugin absent du harnais sans figurer ici
/// fait échouer le cliquet.
const PLUGIN_GATE_SANS_HARNAIS: &[(&str, &str)] = &[(
    "ExpeditionVentPlugin",
    "son MaterialPlugin exige l'app de rendu, qu'un test ne monte pas",
)];

/// Plugins qui n'ont légitimement pas de capteur.
const PLUGIN_GATE_SANS_CAPTEUR: &[(&str, &str)] = &[
    (
        "ExpeditionMatierePlugin",
        "reagit aux materiaux qui arrivent par le streaming — couvert par forgia2_textures",
    ),
    (
        "ExpeditionVentPlugin",
        "pose un shader et avance son horloge : aucun etat qui puisse diverger",
    ),
];

/// Un plugin doit avoir un GARDE : monté dans un test qui tourne, et un capteur.
///
/// # Pourquoi ce cliquet existe
///
/// `check-orphans` vérifie qu'une crate a un consommateur. Il manquait le
/// contrôle symétrique, un cran plus bas : qu'un **plugin** a de quoi le
/// surveiller. Deux trous mesurés le 2026-08-18 sur le seul mode Expédition :
///
/// - 4 plugins sur 6 n'étaient montés par aucun `App::new()`. Or un conflit
///   d'accès (`error[B0002]`) panique à la CONSTRUCTION du schedule : `cargo
///   check`, clippy et 110 tests passent au vert sur un binaire qui ne démarre
///   pas. Seul un `app.update()` l'attrape, et il ne couvrait qu'un tiers du mode.
/// - `posture` était branché, data-driven, testé — et ne publiait RIEN. Son gène
///   `accroupi_facteur_vitesse` n'avait aucun consommateur, ce qui ne pouvait se
///   découvrir qu'en lisant le code trois heures plus tard.
///
/// Le cliquet ne juge pas la qualité du garde. Il juge son EXISTENCE, ce qui est
/// mécanique — donc fiable — et suffisant pour que l'oubli devienne impossible.
fn plugin_gate(args: &[String]) -> i32 {
    let racine = arg_valeur(args, "--crates").unwrap_or_else(|| "crates".to_string());
    let cibles: Vec<String> = arg_valeur(args, "--prefixe")
        .unwrap_or_else(|| "forgia-mode-".to_string())
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let mut plugins: Vec<(String, String)> = Vec::new(); // (nom, crate)
    let mut sources: Vec<(String, String)> = Vec::new(); // (crate, tout le src concaténé)
    let mut tests_par_crate: Vec<(String, String)> = Vec::new(); // (crate, blocs #[cfg(test)])

    let Ok(entrees) = fs::read_dir(&racine) else {
        eprintln!("[plugin-gate] FAIL — {racine} illisible (lancer depuis la racine workspace)");
        return 1;
    };
    for e in entrees.flatten() {
        let nom_crate = e.file_name().to_string_lossy().to_string();
        if !cibles.iter().any(|p| nom_crate.starts_with(p.as_str())) {
            continue;
        }
        // 🚨 Fichier par fichier, JAMAIS sur la concaténation de la crate.
        //
        // La première version concaténait tout puis découpait sur `#[cfg(test)]`.
        // Le dernier bloc s'étendait alors jusqu'à la fin de la concaténation,
        // donc jusque dans les fichiers SUIVANTS : un plugin simplement nommé
        // ailleurs passait pour monté dès qu'un `app.update()` traînait dans le
        // même bloc. Le cliquet aurait acquitté en silence exactement ce qu'il
        // devait attraper — la faute qu'il documente, commise dans son propre code.
        let fichiers_rs = collecter_fichiers(&e.path().join("src"));
        let mut blocs_de_test: Vec<String> = Vec::new();
        for contenu in &fichiers_rs {
            if let Some(i) = contenu.find("#[cfg(test)]") {
                blocs_de_test.push(contenu[i..].to_string());
            }
        }
        tests_par_crate.push((nom_crate.clone(), blocs_de_test.join("\n")));
        let tout = fichiers_rs.join("\n");
        for ligne in tout.lines() {
            let t = ligne.trim();
            if let Some(reste) = t.strip_prefix("pub struct ") {
                if let Some(nom) = reste.strip_suffix("Plugin;") {
                    plugins.push((format!("{nom}Plugin"), nom_crate.clone()));
                }
            }
        }
        sources.push((nom_crate, tout));
    }

    if plugins.is_empty() {
        // 🚨 « Zéro mesuré n'est pas vert, c'est aveugle. » Un cliquet qui ne
        // trouve rien à contrôler doit le DIRE, sinon il se lit comme un succès.
        println!("[plugin-gate] AVEUGLE — aucun plugin trouvé sous {racine}/{cibles:?} : le contrôle n'a rien mesuré");
        return 1;
    }

    // 🚨 La LIGNE DE BASE, sans laquelle ce cliquet ne servirait à rien.
    //
    // Au premier lancement il a trouvé 41 manquements, presque tous dans du code
    // antérieur. Un contrôle qui échoue sur tout l'existant se désactive dans la
    // semaine — c'est la panne classique du contrôle trop ambitieux, et elle
    // laisse le projet SANS contrôle du tout, ce qui est pire que la dette
    // qu'il voulait solder.
    //
    // Il interdit donc les régressions NOUVELLES, et publie ce qui reste dû.
    // Le fichier ne peut que rétrécir : une entrée réparée est signalée pour
    // être retirée, ce qui rend le progrès visible au lieu d'être suggéré.
    let chemin_base = arg_valeur(args, "--baseline")
        .unwrap_or_else(|| "docs/audit/plugin-gate-baseline.txt".to_string());
    let ecrire_base = args.iter().any(|a| a == "--ecrire-baseline");
    let base: Vec<String> = fs::read_to_string(&chemin_base)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect();
    let mut vus: Vec<String> = Vec::new();

    let mut fautes = 0;
    let mut dus = 0;
    let mut sans_harnais_admis = 0;
    let mut sans_capteur_admis = 0;

    for (plugin, nom_crate) in &plugins {
        let src = sources
            .iter()
            .find(|(c, _)| c == nom_crate)
            .map(|(_, s)| s.as_str())
            .unwrap_or("");
        // Le plugin racine du mode orchestre les autres : il n'a pas à être
        // monté lui-même, ses enfants le sont.
        let racine_du_mode = plugin.starts_with("Forgia") && plugin.ends_with("Plugin");

        // 1. Monté dans un test qui tourne ?
        let tests = tests_par_crate
            .iter()
            .find(|(c, _)| c == nom_crate)
            .map(|(_, s)| s.as_str())
            .unwrap_or("");
        let monte = tests.contains(&format!("add_plugins({plugin}"))
            || tests.contains(&format!("::{plugin}"))
            || tests.contains(&format!("{plugin},"));
        let monte = monte && tests.contains("app.update()");
        if !monte && !racine_du_mode {
            match PLUGIN_GATE_SANS_HARNAIS.iter().find(|(p, _)| p == plugin) {
                Some((_, pourquoi)) => {
                    sans_harnais_admis += 1;
                    println!("  · {plugin} hors harnais — admis : {pourquoi}");
                }
                None => {
                    let cle = format!("{plugin}|harnais");
                    vus.push(cle.clone());
                    if base.contains(&cle) {
                        dus += 1;
                    } else {
                        eprintln!(
                            "  REGRESSION {plugin} ({nom_crate}) — monté par aucun App::new() \
                             suivi d'un app.update(). Un error[B0002] y passerait inaperçu \
                             jusqu'au lancement."
                        );
                        fautes += 1;
                    }
                }
            }
        }

        // 2. Publie un capteur ?
        let fichier = plugin_vers_module(plugin);
        let a_capteur = src.contains(&format!("\"id\":\"{fichier}"))
            || src.contains("sensor_io::enqueue")
                && src.contains(plugin.as_str())
                && module_a_capteur(src, plugin);
        if !a_capteur && !racine_du_mode {
            match PLUGIN_GATE_SANS_CAPTEUR.iter().find(|(p, _)| p == plugin) {
                Some((_, pourquoi)) => {
                    sans_capteur_admis += 1;
                    println!("  · {plugin} sans capteur — admis : {pourquoi}");
                }
                None => {
                    let cle = format!("{plugin}|capteur");
                    vus.push(cle.clone());
                    if base.contains(&cle) {
                        dus += 1;
                    } else {
                        eprintln!(
                            "  REGRESSION {plugin} ({nom_crate}) — ne publie aucun capteur. Un \
                             gène déclaré mais jamais appliqué y resterait invisible \
                             (cf accroupi_facteur_vitesse)."
                        );
                        fautes += 1;
                    }
                }
            }
        }
    }

    if ecrire_base {
        vus.sort();
        let contenu = format!(
            "# Ligne de base de `xtask plugin-gate` — CE FICHIER NE DOIT QUE RETRECIR.\n\
             # Chaque ligne est un plugin sans garde, tolere parce qu'il preexiste.\n\
             # Un plugin NOUVEAU sans garde fait echouer le cliquet et n'a pas sa place ici.\n\
             # Regenerer : cargo run -p xtask -- plugin-gate --ecrire-baseline\n{}\n",
            vus.join("\n")
        );
        if let Some(p) = Path::new(&chemin_base).parent() {
            let _ = fs::create_dir_all(p);
        }
        match fs::write(&chemin_base, contenu) {
            Ok(()) => println!("[plugin-gate] ligne de base ecrite : {chemin_base} ({} entrées)", vus.len()),
            Err(e) => {
                eprintln!("[plugin-gate] FAIL — {chemin_base} non écrit : {e}");
                return 1;
            }
        }
    }

    // Une entrée de la base qui ne se reproduit plus : le progrès se DIT, sinon
    // la base enfle sans que personne ne remarque qu'elle pourrait maigrir.
    let reparees: Vec<&String> = base.iter().filter(|b| !vus.contains(b)).collect();

    // Le cliquet DIT ce qu'il a mesuré, toujours — pas seulement quand il échoue.
    println!(
        "[plugin-gate] {} plugin(s) contrôlé(s) sur {} crate(s) · {dus} manquement(s) tolere(s) \
         (ligne de base) · {} repare(s) · {sans_harnais_admis} hors harnais admis · \
         {sans_capteur_admis} sans capteur admis · {fautes} regression(s)",
        plugins.len(),
        sources.len(),
        reparees.len()
    );
    // Honnêteté de portée : le contrôle cherche le montage DANS LA MÊME crate.
    // Un plugin monté par le harnais d'une autre crate serait compté à tort.
    println!(
        "  portee : montage cherche dans les blocs #[cfg(test)] de la crate qui declare le plugin"
    );
    for r in &reparees {
        println!("  · REPARE {r} — retirer de {chemin_base}");
    }
    if fautes > 0 {
        eprintln!("[plugin-gate] FAIL — brancher le garde, ou justifier l'exception dans PLUGIN_GATE_* (xtask/src/main.rs)");
        return 1;
    }
    println!("[plugin-gate] OK");
    0
}

/// `ExpeditionArmeMainPlugin` → `expedition_arme` : le préfixe d'identifiant
/// probable d'un capteur du module. Heuristique assumée : elle sert seulement à
/// chercher, et l'échec renvoie vers la liste d'exceptions, pas vers un blocage.
fn plugin_vers_module(plugin: &str) -> String {
    let sans = plugin.trim_end_matches("Plugin");
    let mut out = String::new();
    for (i, c) in sans.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// Le module qui déclare ce plugin publie-t-il un capteur ?
fn module_a_capteur(src: &str, plugin: &str) -> bool {
    // On cherche dans une fenêtre autour de la déclaration : les modules de ce
    // dépôt tiennent leur plugin et leur capteur dans le même fichier, et `src`
    // est la concaténation de la crate.
    match src.find(&format!("pub struct {plugin};")) {
        Some(i) => {
            let fin = (i + 20_000).min(src.len());
            src[i..fin].contains("sensor_io::enqueue")
        }
        None => false,
    }
}

/// Le contenu de chaque `.rs`, **séparément**. Ce qui se raisonne par fichier —
/// les blocs `#[cfg(test)]`, qui vont jusqu'à la fin du leur — ne peut pas se
/// raisonner sur une concaténation.
fn collecter_fichiers(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entrees) = fs::read_dir(dir) else {
        return out;
    };
    for e in entrees.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(collecter_fichiers(&p));
        } else if p.extension().map(|x| x == "rs").unwrap_or(false) {
            if let Ok(s) = fs::read_to_string(&p) {
                out.push(s);
            }
        }
    }
    out
}

fn collecter_rs(dir: &Path, out: &mut String) {
    out.push_str(&collecter_fichiers(dir).join("\n"));
}

/// Dépendances déclarées au manifeste et jamais référencées dans `src/`.
///
/// # Pourquoi
///
/// `forgia-ik` est resté déclaré dans `forgia-mode-expedition` après que son
/// solveur eut été remplacé — avec, dans le manifeste, un commentaire qui
/// continuait d'expliquer un mécanisme que le code n'appelait plus. Une
/// dépendance morte ne coûte pas que du temps de compilation : elle **documente
/// faussement** l'architecture, et c'est le manifeste qu'on lit en premier.
///
/// Volontairement conservateur : un identifiant vu n'importe où dans les sources
/// suffit à l'acquitter. On cherche l'oubli franc, pas la subtilité.
fn deps_mortes(args: &[String]) -> i32 {
    let racine = arg_valeur(args, "--crates").unwrap_or_else(|| "crates".to_string());
    let Ok(entrees) = fs::read_dir(&racine) else {
        eprintln!("[deps-mortes] FAIL — {racine} illisible");
        return 1;
    };
    let mut examinees = 0usize;
    let mut mortes: Vec<(String, String)> = Vec::new();
    for e in entrees.flatten() {
        let dir = e.path();
        let Ok(tml) = fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let Some(bloc) = tml.split("[dependencies]").nth(1) else {
            continue;
        };
        let bloc = bloc.split("\n[").next().unwrap_or("");
        let mut src = String::new();
        collecter_rs(&dir.join("src"), &mut src);
        if src.is_empty() {
            continue;
        }
        examinees += 1;
        let nom_crate = e.file_name().to_string_lossy().to_string();
        for ligne in bloc.lines() {
            let t = ligne.trim();
            if t.starts_with('#') || !t.contains('=') {
                continue;
            }
            let dep = t.split('=').next().unwrap_or("").trim();
            if dep.is_empty() || !dep.starts_with("forgia-") {
                // Limité aux crates DU PROJET : les dépendances tierces
                // s'utilisent souvent via un `prelude` ou un derive, ce qui
                // produirait des faux positifs. Mieux vaut un contrôle étroit
                // et juste qu'un large et bruyant — mais on le DIT, §sortie.
                continue;
            }
            let ident = dep.replace('-', "_");
            if !src.contains(&ident) {
                mortes.push((nom_crate.clone(), dep.to_string()));
            }
        }
    }
    // Même dispositif que `plugin-gate` : on interdit les NOUVELLES, on publie
    // les anciennes. Six dettes existaient au premier lancement, dans des crates
    // du track RPG ; les traiter d'office reviendrait à modifier le travail d'un
    // autre chantier sous couvert d'outillage.
    let chemin_base = arg_valeur(args, "--baseline")
        .unwrap_or_else(|| "docs/audit/deps-mortes-baseline.txt".to_string());
    let base: Vec<String> = fs::read_to_string(&chemin_base)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect();
    let cles: Vec<String> = mortes.iter().map(|(c, d)| format!("{c}|{d}")).collect();

    if args.iter().any(|a| a == "--ecrire-baseline") {
        let mut tri = cles.clone();
        tri.sort();
        let contenu = format!(
            "# Ligne de base de `xtask deps-mortes` — CE FICHIER NE DOIT QUE RETRECIR.\n\
             # Une dependance declaree et jamais referencee, toleree parce qu'elle preexiste.\n\
             # Regenerer : cargo run -p xtask -- deps-mortes --ecrire-baseline\n{}\n",
            tri.join("\n")
        );
        if let Some(p) = Path::new(&chemin_base).parent() {
            let _ = fs::create_dir_all(p);
        }
        if let Err(e) = fs::write(&chemin_base, contenu) {
            eprintln!("[deps-mortes] FAIL — {chemin_base} non écrit : {e}");
            return 1;
        }
        println!("[deps-mortes] ligne de base ecrite : {chemin_base} ({} entrées)", tri.len());
    }

    let nouvelles: Vec<&(String, String)> = mortes
        .iter()
        .filter(|(c, d)| !base.contains(&format!("{c}|{d}")))
        .collect();
    let reparees: Vec<&String> = base.iter().filter(|b| !cles.contains(b)).collect();

    println!(
        "[deps-mortes] {examinees} crate(s) examinée(s), dépendances internes uniquement · {} morte(s) dont {} toleree(s) · {} reparee(s)",
        mortes.len(),
        mortes.len() - nouvelles.len(),
        reparees.len()
    );
    for r in &reparees {
        println!("  · REPARE {r} — retirer de {chemin_base}");
    }
    for (c, d) in &nouvelles {
        eprintln!("  REGRESSION {c} déclare « {d} » et ne le référence nulle part dans src/");
    }
    if !nouvelles.is_empty() {
        eprintln!("[deps-mortes] FAIL — retirer la ligne ET le commentaire qui la justifiait");
        return 1;
    }
    println!("[deps-mortes] OK");
    0
}

/// `--cle valeur` → `Some(valeur)`.
fn arg_valeur(args: &[String], cle: &str) -> Option<String> {
    args.iter()
        .position(|a| a == cle)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

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

/// Détecte les crates workspace sans aucun consommateur Cargo interne.
///
/// Ce gate ne prétend pas déterminer si chaque sous-plugin Bevy est wiré : un
/// `add_plugins` peut être indirect, conditionnel ou regroupé dans un bundle.
/// Il couvre le niveau fiable et mécanique : une crate de production sans aucune
/// dépendance interne ne peut pas être atteinte par le binaire jeu. `--strict`
/// transforme les orphelines inattendues en échec, ce qui permet de l'ajouter à
/// la CI après résolution de la baseline existante.
fn check_orphans(args: &[String]) -> i32 {
    let strict = args.iter().any(|arg| arg == "--strict");
    let crates = match workspace_crates() {
        Ok(crates) => crates,
        Err(e) => {
            eprintln!("[check-orphans] FAIL — {e}");
            return 1;
        }
    };

    let mut consumers: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for consumer in &crates {
        let manifest = Path::new("crates").join(consumer).join("Cargo.toml");
        let src = match fs::read_to_string(&manifest) {
            Ok(src) => src,
            Err(e) => {
                eprintln!(
                    "[check-orphans] FAIL lecture {} : {e}",
                    norm_path(&manifest)
                );
                return 1;
            }
        };
        let dependencies = match manifest_internal_dependencies(&src, &crates) {
            Ok(dependencies) => dependencies,
            Err(e) => {
                eprintln!("[check-orphans] FAIL parse {} : {e}", norm_path(&manifest));
                return 1;
            }
        };
        for dependency in dependencies {
            if dependency != *consumer {
                consumers
                    .entry(dependency)
                    .or_default()
                    .push(consumer.clone());
            }
        }
    }

    // Le binaire joueur est le package racine `forgia`, hors de `crates/`.
    // Sans lui `forgia-game` serait faussement classé orphelin alors qu'il est
    // précisément l'assemblage consommé par l'exécutable livré.
    let root_manifest = Path::new("Cargo.toml");
    let root_src = match fs::read_to_string(root_manifest) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("[check-orphans] FAIL lecture Cargo.toml : {e}");
            return 1;
        }
    };
    let root_dependencies = match manifest_internal_dependencies(&root_src, &crates) {
        Ok(dependencies) => dependencies,
        Err(e) => {
            eprintln!("[check-orphans] FAIL parse Cargo.toml : {e}");
            return 1;
        }
    };
    for dependency in root_dependencies {
        consumers
            .entry(dependency)
            .or_default()
            .push("forgia (root binary)".to_string());
    }

    // Ces crates sont des outils/frameworks exécutés directement par Cargo, pas
    // des dépendances du binaire joueur. Les garder explicites évite de masquer
    // une vraie crate gameplay orpheline derrière une allowlist vague.
    const EXPECTED_STANDALONE: &[&str] = &[
        "forgia-asset-cdn",
        "forgia-qa-autopilot",
        "forgia-qa-harness",
    ];

    let mut unexpected = Vec::new();
    let mut standalone = Vec::new();
    for name in &crates {
        if consumers.contains_key(name) {
            continue;
        }
        if EXPECTED_STANDALONE.contains(&name.as_str()) {
            standalone.push(name.clone());
        } else {
            unexpected.push(name.clone());
        }
    }
    unexpected.sort();
    standalone.sort();

    println!(
        "[check-orphans] scanned {} crates — {} unexpected orphan(s), {} standalone tool(s)",
        crates.len(),
        unexpected.len(),
        standalone.len()
    );
    if !standalone.is_empty() {
        println!("  standalone tools: {}", standalone.join(", "));
    }
    for name in &unexpected {
        println!(
            "  ORPHAN {name} — no internal Cargo consumer; wire it, remove it, or classify it as a standalone tool"
        );
    }

    if strict && !unexpected.is_empty() {
        eprintln!(
            "[check-orphans] FAIL — unexpected orphan crates found (run without --strict for the report)."
        );
        1
    } else {
        0
    }
}

/// Retourne les dépendances internes déclarées dans un manifeste Cargo.
///
/// Les dépendances Cargo peuvent être écrites sous forme courte
/// (`forgia-core.workspace = true`) ou renommées (`combat = { package =
/// "forgia-combat", ... }`). Lire le TOML évite les faux positifs causés par
/// les commentaires et couvre les sections `target.*.dependencies`.
fn manifest_internal_dependencies(
    src: &str,
    workspace_crates: &[String],
) -> Result<Vec<String>, toml::de::Error> {
    let manifest: toml::Value = toml::from_str(src)?;
    let known: std::collections::HashSet<&str> =
        workspace_crates.iter().map(String::as_str).collect();
    let mut found = std::collections::HashSet::new();
    collect_manifest_dependencies(&manifest, &known, &mut found);
    let mut dependencies: Vec<_> = found.into_iter().collect();
    dependencies.sort_unstable();
    Ok(dependencies)
}

fn collect_manifest_dependencies(
    value: &toml::Value,
    known: &std::collections::HashSet<&str>,
    found: &mut std::collections::HashSet<String>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    for (key, child) in table {
        // `[workspace.dependencies]` est un catalogue partagé, pas une preuve
        // qu'un package consomme effectivement la crate. C'est particulièrement
        // important pour le manifeste racine.
        if key == "workspace" {
            continue;
        }
        if matches!(
            key.as_str(),
            "dependencies" | "build-dependencies" | "dev-dependencies"
        ) {
            let Some(dependencies) = child.as_table() else {
                continue;
            };
            for (alias, spec) in dependencies {
                let package = spec
                    .as_table()
                    .and_then(|table| table.get("package"))
                    .and_then(toml::Value::as_str)
                    .unwrap_or(alias);
                if known.contains(package) {
                    found.insert(package.to_string());
                }
            }
        } else {
            collect_manifest_dependencies(child, known, found);
        }
    }
}

/// Noms `forgia-*` déclarés dans `[workspace].members`.
fn workspace_crates() -> Result<Vec<String>, String> {
    let cargo = fs::read_to_string("Cargo.toml")
        .map_err(|e| format!("Cargo.toml illisible (lancer depuis la racine workspace ?) : {e}"))?;
    let mut crates = Vec::new();
    let mut in_members = false;
    for line in cargo.lines() {
        let line = line.trim();
        if line.starts_with("members") && line.contains('[') {
            in_members = true;
            continue;
        }
        if !in_members {
            continue;
        }
        if line.starts_with(']') {
            break;
        }
        let Some(path) = line
            .trim_matches(|c| c == '"' || c == ',' || c == ' ')
            .strip_prefix("crates/")
        else {
            continue;
        };
        crates.push(path.to_string());
    }
    if crates.is_empty() {
        Err("aucune crate trouvée dans [workspace].members".to_string())
    } else {
        Ok(crates)
    }
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

// ────────────────────────── context-budget (2026-08-09) ──────────────────────────
//
// `CLAUDE.md` et tout `.claude/rules/*.md` sont chargés **à chaque session**,
// avant même le premier mot. Mesuré le 2026-08-09 : **119 060 o**, soit ~29 700
// tokens — dont 79 % pour les seules règles. Rien ne les retire jamais, et une
// règle périmée ne coûte pas que des tokens : elle **oriente mal**
// (`fine-grained-crates.md` a induit les sessions en erreur pendant des semaines,
// c'est écrit dans son propre en-tête). Personne ne teste une règle.
//
// D'où ce cliquet, du même esprit que `no-scaffold` : il ne juge pas le contenu,
// il empêche la dérive silencieuse. Le seuil est le mesuré du jour arrondi au
// millier supérieur — baisser le plafond après un dégraissage, jamais le monter
// pour faire passer un ajout.
// Mesuré 92 430 o le 2026-08-09, après le dégraissage d'`outillage.md`
// (14 468 → 4 927). Le plafond serre à ~2,5 Ko près : c'est un cliquet, pas une
// autorisation de croître.
//
// 2026-08-20 — deux corrections (déclencheur : article Anthropic « context
// engineering for Claude 5 », vérifié en session : le harnais charge
// .claude/rules/ RÉCURSIVEMENT, sous-dossiers compris) :
// 1. le scan descend dans les sous-dossiers — l'ancien read_dir plat ne voyait
//    pas `on-demand/` (27,8 Ko chargés chaque session, invisibles au gate : le
//    « contrôle qui passe à vide » de controle-de-la-sortie.md §1). Les règles
//    map sont sorties vers docs/design/ ; ce scan récursif garantit qu'aucun
//    sous-dossier ne pourra plus se glisser ici en silence.
// 2. plafond abaissé après le déménagement : 83 163 o mesurés (fs::metadata,
//    scan récursif) → 84 000. Abaissé de nouveau après chaque dégraissage.
const CONTEXT_BUDGET_BYTES: u64 = 84_000;

fn context_budget(_args: &[String]) -> i32 {
    println!("[xtask] context-budget — poids charge a CHAQUE session");

    let mut total: u64 = 0;
    let mut lignes: Vec<(u64, String)> = Vec::new();

    let ajoute = |chemin: &Path, lignes: &mut Vec<(u64, String)>, total: &mut u64| {
        if let Ok(m) = fs::metadata(chemin) {
            let n = m.len();
            *total += n;
            lignes.push((n, chemin.display().to_string()));
        }
    };

    ajoute(Path::new("CLAUDE.md"), &mut lignes, &mut total);

    let regles = Path::new(".claude/rules");
    if !regles.exists() {
        eprintln!("ERROR: .claude/rules/ introuvable (lancer depuis la racine du workspace)");
        return 2;
    }
    // Récursif : le harnais charge les sous-dossiers aussi (mesuré 2026-08-20,
    // `on-demand/` était chargé chaque session en échappant au gate).
    let mut a_visiter = vec![regles.to_path_buf()];
    while let Some(dossier) = a_visiter.pop() {
        let mut entrees: Vec<_> = match fs::read_dir(&dossier) {
            Ok(d) => d.filter_map(Result::ok).collect(),
            Err(e) => {
                eprintln!("ERROR: lecture de {} : {e}", dossier.display());
                return 2;
            }
        };
        entrees.sort_by_key(std::fs::DirEntry::file_name);
        for e in entrees {
            let p = e.path();
            if p.is_dir() {
                a_visiter.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                ajoute(&p, &mut lignes, &mut total);
            }
        }
    }

    lignes.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
    println!("  Les 5 plus lourds :");
    for (n, nom) in lignes.iter().take(5) {
        println!("    {n:>7} o  {nom}");
    }
    println!(
        "  TOTAL {total} o  (~{} tokens)  ·  plafond {CONTEXT_BUDGET_BYTES} o",
        total / 4
    );

    if total <= CONTEXT_BUDGET_BYTES {
        println!(
            "✅ context-budget : {} o de marge",
            CONTEXT_BUDGET_BYTES - total
        );
        0
    } else {
        eprintln!(
            "❌ context-budget : depassement de {} o",
            total - CONTEXT_BUDGET_BYTES
        );
        eprintln!();
        eprintln!("Une regle se paie a CHAQUE session ; un memory seulement quand on le lit ;");
        eprintln!("un hook en millisecondes. Deplacer le detail (mesures, historique, cas");
        eprintln!("d'echec) vers un memory, et ne garder dans la regle que ce qui CHANGE");
        eprintln!("une decision. Ne PAS monter le plafond pour faire passer un ajout.");
        1
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
        // checked_div : effective==0 → None → unwrap_or(0) (comportement identique à
        // l'ancien garde `if effective > 0`), sans le division-guard manuel (clippy).
        let todo_pct = (todo * 100).checked_div(effective).unwrap_or(0);
        if todo_pct > 80 {
            violations.push(format!(
                "  ❌ {name} : {todo_pct}% TODO comments ({todo}/{effective}). Implement or remove."
            ));
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

    let orphans: Vec<&String> = produced.keys().filter(|n| !declared.contains(*n)).collect();
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

/// Répertorie les écritures JSON de capteurs faites directement sur le thread
/// Bevy. Les capteurs doivent passer par `forgia_core::sensor_io::enqueue` :
/// une saturation disque ne doit jamais allonger une frame. Les sauvegardes
/// métier ne sont pas visées, car elles n'écrivent pas de `forgia*.json`.
fn sensor_sync_writes(args: &[String]) -> i32 {
    let strict = args.iter().any(|arg| arg == "--strict");
    let mut sites = Vec::new();

    fn visit(dir: &Path, sites: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, sites);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let Ok(source) = fs::read_to_string(&path) else {
                continue;
            };
            let is_sensor_file = source.contains("forgia") && source.contains(".json");
            if !is_sensor_file {
                continue;
            }
            for (line_no, line) in source.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") || !trimmed.contains("fs::write(") {
                    continue;
                }
                // Literal direct, ou constante de capteur du même fichier.
                if trimmed.contains("forgia")
                    || trimmed.contains("SENSOR_PATH")
                    || trimmed.contains("OUTPUT_PATH")
                    || trimmed.contains("VFX_SENSOR_PATH")
                {
                    sites.push(format!("{}:{}", norm_path(&path), line_no + 1));
                }
            }
        }
    }

    visit(Path::new("crates"), &mut sites);
    sites.sort();
    println!(
        "[sensor-sync-writes] {} synchronous sensor write(s)",
        sites.len()
    );
    for site in &sites {
        println!("  {site}");
    }
    if strict && !sites.is_empty() {
        eprintln!(
            "[sensor-sync-writes] FAIL — migrate these writers to forgia_core::sensor_io::enqueue"
        );
        1
    } else {
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
                    // Les producteurs peuvent écrire directement, ou déléguer la
                    // persistance à `sensor_io::enqueue` pour ne pas bloquer la
                    // frame. Les deux chemins sont des producteurs valides.
                    let window_end = (idx + 3).min(lines.len());
                    let window = lines[idx..window_end].join(" ");
                    let is_write_ctx = window.contains("fs::write")
                        || window.contains(".write_all")
                        || window.contains("write_atomic")
                        || window.contains("serde_json::to_string")
                        || window.contains("sensor_io::enqueue");
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
                                || w.contains("sensor_io::enqueue")
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
            // 🚨 Toute accolade = GABARIT, pas un capteur. Le garde ne couvrait
            // que `{}` et laissait passer `forgia2_{id}.json` — le modele de
            // `forgia_core::constat`, qui construit le nom depuis un identifiant.
            // Resultat le 2026-08-18 : l'audit declarait orphelin un capteur qui
            // n'existe pas. `parse_registry_filenames` excluait deja `{` en
            // entier ; les deux cotes se lisent enfin pareil.
            && !inner.contains('{')
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
        eprintln!(
            "  Run `cargo xtask asset-load --fix` to generate xtask/asset-load-allowlist.toml."
        );
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
    // Les atomiques (`Atomic*::load(Ordering::...)`) ne chargent pas d'asset.
    // Le gate est syntaxique et doit exclure ce faux positif lock-free.
    if line.contains(".load(Ordering::") {
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
fn write_asset_load_allowlist(counts: &std::collections::BTreeMap<String, usize>, total: usize) {
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

// ─────────────────────────── story-index + wip-check (consolidation roadmap 2026-07-03) ───────────────────────────
//
// story-index : génère docs/stories/_index.md — le board UNIQUE des stories, avec
//   statuts normalisés depuis des en-têtes hétérogènes (3 vocabulaires « en cours »
//   observés : IN_PROGRESS / EN COURS / IN PROGRESS → un seul). Remplit le fichier
//   fantôme que .claude/rules/story-done-gate.md référençait sans qu'il existe.
//   `--check` : échoue si le fichier sur disque diffère du généré (gate anti-drift).
// wip-check  : échoue si > WIP_LIMIT stories IN_PROGRESS. Principe Lean/Kanban
//   « stop starting, start finishing ». Complète story-gate (DONE mécanique) côté
//   ENTRÉE du flux. Origine : audit 2026-07-03 — ~60 chantiers ouverts sur 186
//   stories, aucune limite. Commande manuelle (PAS branchée en CI : elle FAIL fort
//   tant que le WIP n'est pas résorbé — c'est le signal, pas un blocage de build).

/// Plafond de work-in-progress. Aligné sur la règle de pilotage de docs/ROADMAP.md.
/// Même esprit que STORY_GATE_LOC_THRESHOLD / ASSET_LOAD_TARGET : un seuil de gate.
const WIP_LIMIT: usize = 3;

/// Statut normalisé d'une story (vocabulaire unique). Les libellés bruts des fichiers
/// (EN COURS, IN PROGRESS, CODE-COMPLETE…) sont mappés dessus par `classify_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoryStatus {
    Done,
    Review,     // code-complete / attente validation / ready-for-review
    InProgress, // in_progress / en cours / wip
    Ready,
    Draft, // draft / todo / backlog
    Blocked,
    Cancelled,
    Unknown, // aucune ligne de statut reconnue → à normaliser
}

impl StoryStatus {
    /// Libellé normalisé unique affiché dans le board.
    fn label(self) -> &'static str {
        match self {
            StoryStatus::Done => "DONE",
            StoryStatus::Review => "REVIEW",
            StoryStatus::InProgress => "IN_PROGRESS",
            StoryStatus::Ready => "READY",
            StoryStatus::Draft => "DRAFT",
            StoryStatus::Blocked => "BLOCKED",
            StoryStatus::Cancelled => "CANCELLED",
            StoryStatus::Unknown => "UNKNOWN",
        }
    }

    /// Ordre d'affichage : travail actif en haut, DONE/CANCELLED en bas.
    fn display_order() -> [StoryStatus; 8] {
        [
            StoryStatus::InProgress,
            StoryStatus::Review,
            StoryStatus::Ready,
            StoryStatus::Blocked,
            StoryStatus::Draft,
            StoryStatus::Unknown,
            StoryStatus::Done,
            StoryStatus::Cancelled,
        ]
    }
}

struct StoryMeta {
    id: u32,
    file: String,
    title: String,
    status: StoryStatus,
}

/// Tokens alphanumériques minusculés d'une ligne (comparaison mot-entier robuste :
/// « done » ne matche pas « abandonné »).
fn line_words(line: &str) -> Vec<String> {
    line.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Classe le statut depuis l'en-tête : on cherche d'abord une LIGNE de statut
/// explicite (contient « statut »/« status ») puis on la classe du signal le plus
/// fort au plus faible. Aucune ligne de statut → UNKNOWN (le board les remonte).
/// Une ligne DÉCLARE-t-elle un statut, ou mentionne-t-elle juste le mot ?
///
/// **Corrige un défaut constaté le 2026-08-12** : l'ancien test se contentait de
/// `contains("statut") || contains("status")`, et prenait donc la PREMIÈRE ligne
/// contenant le mot — souvent le titre. Trois stories étaient inclassables par
/// construction :
///
/// - `# Story-476 — \`forgia-status-effects\`` — le nom de la crate
/// - `# Story-654 — … icônes de statut` — le sujet de la story
/// - une ligne de corps citant le symbole `StatusShock` ou le fichier `status.rs`
///
/// Elles tombaient en `UNKNOWN` **quel que soit leur vrai statut**, y compris après
/// avoir été explicitement marquées. Un index qui ment sur 3 stories est un index
/// dont on doute sur les 208.
///
/// Règle : le mot doit être suivi d'un **deux-points**, à travers l'habillage
/// markdown (`**`, backticks, espaces). C'est ce qui distingue une déclaration
/// (`**Statut** : DONE`) d'une mention (`icônes de statut`).
fn is_status_declaration(line: &str) -> bool {
    let lc = line.to_lowercase();
    for key in ["statut", "status"] {
        let mut from = 0usize;
        while let Some(rel) = lc[from..].find(key) {
            let after = from + rel + key.len();
            // Saute l'habillage entre le mot et le deux-points.
            let rest = lc[after..].trim_start_matches(['*', '`', '_', ' ', ')', ']']);
            if rest.starts_with(':') || rest.starts_with('\u{ff1a}') {
                return true;
            }
            from = after;
        }
    }
    false
}

fn classify_status(head: &str) -> StoryStatus {
    let Some(line) = head.lines().take(40).find(|l| is_status_declaration(l)) else {
        return StoryStatus::Unknown;
    };
    let lc = line.to_lowercase();
    let words = line_words(line);
    let has = |w: &str| words.iter().any(|x| x.as_str() == w);

    if lc.contains("cancel")
        || lc.contains("annul")
        || lc.contains("abandon")
        || lc.contains("wontfix")
    {
        StoryStatus::Cancelled
    } else if line.contains('✅') || has("done") || lc.contains("**done") {
        StoryStatus::Done
    } else if has("blocked") || lc.contains("bloqu") {
        StoryStatus::Blocked
    } else if lc.contains("code-complete")
        || lc.contains("code complete")
        || has("review")
        || lc.contains("valid")
    {
        StoryStatus::Review
    } else if lc.contains("in_progress")
        || lc.contains("in progress")
        || lc.contains("en cours")
        || has("wip")
        || line.contains('🚧')
    {
        StoryStatus::InProgress
    } else if has("ready") || lc.contains("prêt") || lc.contains("pret") {
        StoryStatus::Ready
    } else if has("draft") || has("todo") || has("brouillon") || has("backlog") {
        StoryStatus::Draft
    } else {
        StoryStatus::Unknown
    }
}

/// Titre = 1er heading H1, débarrassé d'un préfixe « Story-NNN — » redondant avec la
/// colonne ID. Fallback = nom de fichier.
fn extract_title<'a>(content: &'a str, file: &'a str) -> &'a str {
    let raw = content
        .lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim())
        .unwrap_or(file);
    if let Some((pre, post)) = raw.split_once('—') {
        if pre.trim().to_lowercase().starts_with("story-") {
            return post.trim();
        }
    }
    raw
}

/// Scanne docs/stories/story-*.md → (id, fichier, titre, statut normalisé), trié par id.
fn collect_stories() -> Result<Vec<StoryMeta>, String> {
    let dir = Path::new("docs/stories");
    let entries = fs::read_dir(dir).map_err(|e| format!("docs/stories illisible : {e}"))?;
    let mut out: Vec<StoryMeta> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let file = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if !file.starts_with("story-") {
            continue;
        }
        let id: u32 = file
            .strip_prefix("story-")
            .unwrap_or("")
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        let content = fs::read_to_string(&path).unwrap_or_default();
        let head = content.lines().take(40).collect::<Vec<_>>().join("\n");
        let status = classify_status(&head);
        let title = extract_title(&content, &file).to_string();
        out.push(StoryMeta {
            id,
            file,
            title,
            status,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id).then(a.file.cmp(&b.file)));
    Ok(out)
}

/// Rend le markdown du board (déterministe → comparable en `--check`).
fn render_index(stories: &[StoryMeta]) -> String {
    let mut s = String::new();
    s.push_str("# Index des stories — Forgia\n\n");
    s.push_str(
        "> ⚙️ **Fichier généré par `cargo run -p xtask -- story-index`. Ne pas éditer à la main.**\n",
    );
    s.push_str("> Board unique, statuts normalisés. Pilotage : [`../ROADMAP.md`](../ROADMAP.md). Gate anti-drift : `story-index --check`.\n\n");

    s.push_str("## Résumé\n\n");
    s.push_str("| Statut | Nombre |\n| --- | --- |\n");
    for st in StoryStatus::display_order() {
        let n = stories.iter().filter(|x| x.status == st).count();
        if n > 0 {
            s.push_str(&format!("| {} | {n} |\n", st.label()));
        }
    }
    s.push_str(&format!("| **Total** | **{}** |\n\n", stories.len()));

    let ip = stories
        .iter()
        .filter(|x| x.status == StoryStatus::InProgress)
        .count();
    if ip > WIP_LIMIT {
        s.push_str(&format!(
            "> 🚨 **{ip} stories `IN_PROGRESS` > limite WIP {WIP_LIMIT}.** *Stop starting, start finishing.*\n\n"
        ));
    }

    for st in StoryStatus::display_order() {
        let group: Vec<&StoryMeta> = stories.iter().filter(|x| x.status == st).collect();
        if group.is_empty() {
            continue;
        }
        s.push_str(&format!("## {} ({})\n\n", st.label(), group.len()));
        s.push_str("| ID | Titre | Fichier |\n| --- | --- | --- |\n");
        for m in group {
            let title = m.title.replace('|', "\\|");
            s.push_str(&format!(
                "| {} | {title} | [{}](./{}) |\n",
                m.id, m.file, m.file
            ));
        }
        s.push('\n');
    }
    s
}

fn story_index(args: &[String]) -> i32 {
    let check = args.iter().any(|a| a == "--check");
    let stories = match collect_stories() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[story-index] FAIL — {e}");
            return 1;
        }
    };
    let generated = render_index(&stories);
    let out_path = Path::new("docs/stories/_index.md");
    let ip = stories
        .iter()
        .filter(|x| x.status == StoryStatus::InProgress)
        .count();

    if check {
        let current = fs::read_to_string(out_path).unwrap_or_default();
        if current == generated {
            println!(
                "[story-index] OK — _index.md à jour ({} stories)",
                stories.len()
            );
            0
        } else {
            eprintln!(
                "[story-index] FAIL — docs/stories/_index.md périmé. Régénérer : cargo run -p xtask -- story-index"
            );
            1
        }
    } else {
        if let Err(e) = fs::write(out_path, &generated) {
            eprintln!("[story-index] FAIL écriture {} : {e}", out_path.display());
            return 1;
        }
        println!(
            "[story-index] OK — docs/stories/_index.md régénéré ({} stories, {ip} IN_PROGRESS)",
            stories.len()
        );
        if ip > WIP_LIMIT {
            println!(
                "  ⚠️  {ip} IN_PROGRESS > limite WIP {WIP_LIMIT} — voir `cargo run -p xtask -- wip-check`"
            );
        }
        0
    }
}

fn wip_check(_args: &[String]) -> i32 {
    let stories = match collect_stories() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[wip-check] FAIL — {e}");
            return 1;
        }
    };
    let in_progress: Vec<&StoryMeta> = stories
        .iter()
        .filter(|s| s.status == StoryStatus::InProgress)
        .collect();
    let n = in_progress.len();
    println!("[wip-check] {n} stories IN_PROGRESS (limite WIP {WIP_LIMIT})");
    for m in &in_progress {
        println!("   story-{} — {}", m.id, m.title);
    }
    if n > WIP_LIMIT {
        eprintln!();
        eprintln!(
            "[wip-check] FAIL — {n} > {WIP_LIMIT}. Finis (ou repasse en REVIEW/DRAFT) avant d'ouvrir un nouveau chantier. « Stop starting, start finishing. »"
        );
        1
    } else {
        println!("[wip-check] OK — sous la limite.");
        0
    }
}

// ─────────────────────────── dist-roguelite (build joueur) ───────────────────────────

/// Assemble un dossier de distribution « joueur » du Roguelite : build release +
/// copie de l'exe, des assets (élagués) et de `config/`, prêt à pousser sur
/// itch.io via butler.
///
/// Layout produit — l'exe cherche `assets/` à côté de lui (AssetPlugin) et
/// `config/biomes/` en remontant depuis l'exe (cf `persist::legacy_config_dir`) :
/// ```text
/// <out>/
///   forgia.exe
///   assets/          (moins ASSET_BLOCKLIST : démo Cyber City, etc.)
///   config/biomes/
///   LISEZ-MOI.txt
/// ```
/// Les saves vivent dans `%APPDATA%\Forgia\` (cf `forgia-mode-roguelite::persist`)
/// → remplacer ce dossier lors d'une MàJ n'efface JAMAIS la progression.
///
/// Flags :
///   --out <dir>   dossier cible (défaut : `<workspace>/../Forgia Roguelite`)
///   --full        recopie les assets même s'ils existent déjà (sinon skip = itération rapide)
///   --skip-build  n'exécute pas `cargo build --release` (assemble avec l'exe courant)
fn dist_roguelite(args: &[String]) -> i32 {
    // Assets exclus du build joueur (chemins RELATIFS à assets/, séparateur `/`).
    // Sûr = confirmé non chargé par le Roguelite. La démo Cyber City est masquée
    // du menu joueur → son GLB 185 Mo n'a aucun consommateur ici. Étendre au fil
    // des confirmations (allowlist du réel via forgia_textures.json après playtest).
    const ASSET_BLOCKLIST: &[&str] = &["models/environment/cyberpunk_city.glb"];

    let flag = |name: &str| args.iter().any(|a| a == name);
    let opt = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1).cloned())
    };

    let root = match env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[dist] current_dir: {e}");
            return 1;
        }
    };
    let out = opt("--out").map(PathBuf::from).unwrap_or_else(|| {
        root.parent()
            .unwrap_or(root.as_path())
            .join("Forgia Roguelite")
    });
    let full = flag("--full");
    let skip_build = flag("--skip-build");

    println!("[dist] workspace = {}", root.display());
    println!("[dist] cible     = {}", out.display());

    // 1. Build release (sauf --skip-build). En release, debug_assertions est OFF
    //    → le menu masque automatiquement la ligne « Démos moteur (dev) ».
    if !skip_build {
        // `--jobs N` borne le parallélisme du linker (le build release du binaire
        // complet peut OOM sur machine mémoire-limitée ; `-j 4` sécurise).
        let mut cargo_args = vec!["build", "--release", "--bin", "forgia"];
        if let Some(j) = opt("--jobs") {
            cargo_args.push("--jobs");
            // Fuite volontaire (leak) : `&'static str` requis par `args()`, process
            // court-vivant (l'OS récupère à l'exit) → aucun impact.
            cargo_args.push(Box::leak(j.into_boxed_str()));
        }
        println!("[dist] cargo {} …", cargo_args.join(" "));
        match Command::new("cargo")
            .args(&cargo_args)
            .current_dir(&root)
            .status()
        {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("[dist] build échoué (code {:?})", s.code());
                return 1;
            }
            Err(e) => {
                eprintln!("[dist] impossible de lancer cargo: {e}");
                return 1;
            }
        }
    }

    // 2. Exe.
    let exe_src = root.join("target/release/forgia.exe");
    if !exe_src.exists() {
        eprintln!(
            "[dist] introuvable: {} (relance sans --skip-build)",
            exe_src.display()
        );
        return 1;
    }
    if let Err(e) = fs::create_dir_all(&out) {
        eprintln!("[dist] create {}: {e}", out.display());
        return 1;
    }
    if let Err(e) = fs::copy(&exe_src, out.join("forgia.exe")) {
        eprintln!("[dist] copie exe: {e}");
        return 1;
    }
    println!("[dist] ✓ forgia.exe");

    // 3. config/ (biomes + TOML runtime) — SANS les saves dev ni les .tmp.
    match copy_tree(&root.join("config"), &out.join("config"), &|rel| {
        let s = rel.to_string_lossy();
        s.ends_with("_save.toml") || s.ends_with(".tmp")
    }) {
        Ok(_) => println!("[dist] ✓ config/"),
        Err(e) => {
            eprintln!("[dist] copie config: {e}");
            return 1;
        }
    }

    // 4. assets/ (moins ASSET_BLOCKLIST). Skip si déjà présents et pas --full
    //    (itération rapide : un patch code ne recopie pas 1 Go d'assets).
    let assets_dst = out.join("assets");
    if assets_dst.exists() && !full {
        println!("[dist] assets/ déjà présents — skip (--full pour recopier)");
    } else {
        println!("[dist] copie assets/ (peut prendre une minute) …");
        match copy_tree(&root.join("assets"), &assets_dst, &|rel| {
            let s = rel.to_string_lossy().replace('\\', "/");
            ASSET_BLOCKLIST.contains(&s.as_ref())
        }) {
            Ok(bytes) => println!("[dist] ✓ assets/ ({:.0} Mo)", bytes as f64 / 1_048_576.0),
            Err(e) => {
                eprintln!("[dist] copie assets: {e}");
                return 1;
            }
        }
    }

    // 5. Note pour les joueurs.
    let readme = "FORGIA ROGUELITE — build de test\n\
        \n\
        Lancer : double-clique forgia.exe\n\
        (Windows SmartScreen peut avertir : « Informations complémentaires » > « Exécuter quand même ».)\n\
        \n\
        Ta progression est sauvegardée dans %APPDATA%\\Forgia\\ : une mise à jour ne l'efface pas.\n\
        \n\
        Un souci / crash ? Le fichier forgia2_crash.json est créé à côté du jeu — envoie-le à Antoine.\n";
    if let Err(e) = fs::write(out.join("LISEZ-MOI.txt"), readme) {
        eprintln!("[dist] écriture LISEZ-MOI: {e}");
    }

    println!("[dist] ✅ prêt : {}", out.display());
    println!("[dist] Push itch (butler installé + `butler login` faits) :");
    println!(
        "       butler push \"{}\" antoinebecker10-afk/forgia-roguelite:windows",
        out.display()
    );
    0
}

/// Copie récursive `src`→`dst` en excluant toute entrée dont le chemin RELATIF
/// (à `src`) satisfait `exclude`. Suit les junctions/symlinks pointant sur un
/// dossier réel (un pack V1 peut être monté ainsi) afin de matérialiser les vrais
/// fichiers côté distribution. Retourne le nombre d'octets copiés.
fn copy_tree(src: &Path, dst: &Path, exclude: &dyn Fn(&Path) -> bool) -> std::io::Result<u64> {
    fn rec(
        src: &Path,
        dst: &Path,
        rel: &Path,
        exclude: &dyn Fn(&Path) -> bool,
    ) -> std::io::Result<u64> {
        fs::create_dir_all(dst)?;
        let mut total = 0u64;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let name = entry.file_name();
            let child_rel = rel.join(&name);
            if exclude(&child_rel) {
                continue;
            }
            let child_src = entry.path();
            let child_dst = dst.join(&name);
            let ft = entry.file_type()?;
            // `metadata` déréférence junctions/symlinks : un pack monté en junction
            // se comporte comme un vrai dossier pour la copie.
            let is_dir = ft.is_dir()
                || (ft.is_symlink()
                    && fs::metadata(&child_src)
                        .map(|m| m.is_dir())
                        .unwrap_or(false));
            if is_dir {
                total += rec(&child_src, &child_dst, &child_rel, exclude)?;
            } else if child_src.is_file() {
                total += fs::copy(&child_src, &child_dst)?;
            }
        }
        Ok(total)
    }
    rec(src, dst, Path::new(""), exclude)
}

// ─────────────────────── Cliquet des capteurs (2026-08-18) ───────────────────
//
// Trois défauts se répétaient 45 fois sur 132 capteurs : écriture synchrone qui
// bloque la frame, verdict absent, vert sur échantillon vide. Le troisième est
// désormais impossible (`forgia_core::constat` dégrade tout seul). Les deux
// premiers restent détectables, donc CLIQUETABLES.
//
// 🚨 Le seuil n'est pas zéro : il est la DETTE MESURÉE, et elle ne peut que
// décroître. Un cliquet à zéro sur une dette existante n'est jamais branché —
// c'est ce qui est arrivé à `sensor-sync-writes --strict`, qui compte depuis
// des mois sans que personne ne puisse l'activer.

/// Un capteur est un `.rs` qui écrit un fichier `forgia*.json`.
fn fichiers_de_capteurs() -> Vec<(String, String)> {
    fn visit(dir: &Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, out);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let Ok(src) = fs::read_to_string(&path) else {
                continue;
            };
            let ecrit_capteur = (src.contains("sensor_io::enqueue")
                || src.contains("fs::write(")
                || src.contains(".publier("))
                && src.contains("forgia")
                && src.contains(".json");
            if ecrit_capteur {
                out.push((norm_path(&path), src));
            }
        }
    }
    let mut out = Vec::new();
    visit(Path::new("crates"), &mut out);
    out.sort();
    out
}

fn capteur_gate(args: &[String]) -> i32 {
    let verbeux = args.iter().any(|a| a == "--liste");
    let mut synchrones: Vec<String> = Vec::new();
    let mut sans_verdict: Vec<String> = Vec::new();

    for (chemin, src) in fichiers_de_capteurs() {
        for (n, ligne) in src.lines().enumerate() {
            let t = ligne.trim_start();
            if t.starts_with("//") || !t.contains("fs::write(") {
                continue;
            }
            if t.contains("forgia")
                || t.contains("SENSOR_PATH")
                || t.contains("OUTPUT_PATH")
                || t.contains("VFX_SENSOR_PATH")
            {
                synchrones.push(format!("{chemin}:{}", n + 1));
            }
        }
        // Signal GROSSIER mais JUSTE (leçon du 2026-08-17 : l'analyse fine des
        // blocs `add_systems` a produit 15 faux positifs). Un capteur publie un
        // verdict s'il écrit le champ, ou s'il passe par `constat` — qui le
        // rend obligatoire.
        if !src.contains("\"severity\"") && !src.contains("constat::") {
            sans_verdict.push(chemin.clone());
        }
    }

    let dette = Path::new("xtask/capteur-dette.toml");
    let lu = fs::read_to_string(dette).unwrap_or_default();
    let plafond = |cle: &str| -> usize {
        lu.lines()
            .find(|l| l.trim_start().starts_with(cle))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(usize::MAX)
    };
    let max_sync = plafond("ecritures_synchrones");
    let max_muet = plafond("sans_verdict");

    println!(
        "[capteur-gate] ecritures synchrones {} (plafond {max_sync}) · sans verdict {} (plafond {max_muet})",
        synchrones.len(),
        sans_verdict.len()
    );
    if verbeux {
        for s in &synchrones {
            println!("  [sync]  {s}");
        }
        for s in &sans_verdict {
            println!("  [muet]  {s}");
        }
    }

    let mut code = 0;
    if synchrones.len() > max_sync {
        eprintln!(
            "[capteur-gate] FAIL — {} ecriture(s) synchrone(s) pour un plafond de {max_sync}. \
             Un capteur ecrit par `forgia_core::constat::Constat::publier`, jamais par \
             `fs::write` : l'ecriture directe bloque la frame de jeu.",
            synchrones.len()
        );
        code = 1;
    }
    if sans_verdict.len() > max_muet {
        eprintln!(
            "[capteur-gate] FAIL — {} capteur(s) sans verdict pour un plafond de {max_muet}. \
             Utiliser `forgia_core::constat` : il rend la severite obligatoire et degrade \
             un `ok` sans echantillon.",
            sans_verdict.len()
        );
        code = 1;
    }
    if code == 0 && (synchrones.len() < max_sync || sans_verdict.len() < max_muet) {
        println!(
            "[capteur-gate] la dette a BAISSE — abaisser les plafonds dans \
             xtask/capteur-dette.toml pour la verrouiller."
        );
    }
    code
}

// ─────────────────────── Cliquet des strates (2026-08-18) ────────────────────
//
// « Une crate ne dépend jamais d'une strate supérieure à la sienne. » Le
// contrat vit dans `xtask/strates.toml`, avec sa dette. Voir l'en-tête de ce
// fichier pour le pourquoi — notamment pourquoi l'intra-strate est LÉGAL.

/// Les dépendances `forgia-*` déclarées par chaque crate du workspace.
fn deps_forgia() -> std::collections::BTreeMap<String, Vec<String>> {
    let mut out = std::collections::BTreeMap::new();
    let Ok(entries) = fs::read_dir("crates") else {
        return out;
    };
    for e in entries.flatten() {
        let manifeste = e.path().join("Cargo.toml");
        let Ok(src) = fs::read_to_string(&manifeste) else {
            continue;
        };
        let Some(bloc) = src.split("[dependencies]").nth(1) else {
            continue;
        };
        let bloc = bloc.split("\n[").next().unwrap_or("");
        let mut d: Vec<String> = bloc
            .lines()
            .filter_map(|l| {
                let l = l.trim_start();
                if l.starts_with('#') {
                    return None;
                }
                let nom = l.split('=').next()?.trim();
                (nom.starts_with("forgia-")).then(|| nom.to_string())
            })
            .collect();
        d.sort();
        d.dedup();
        out.insert(e.file_name().to_string_lossy().to_string(), d);
    }
    out
}

fn strates(args: &[String]) -> i32 {
    let verbeux = args.iter().any(|a| a == "--liste");
    let Ok(brut) = fs::read_to_string("xtask/strates.toml") else {
        eprintln!("[strates] FAIL — xtask/strates.toml introuvable");
        return 1;
    };
    let Ok(doc) = brut.parse::<toml::Value>() else {
        eprintln!("[strates] FAIL — xtask/strates.toml illisible");
        return 1;
    };

    let ordre: Vec<String> = doc
        .get("ordre")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let rang = |s: &str| ordre.iter().position(|x| x == s);
    // Non listée = `brique`, la strate par défaut.
    let defaut = rang("brique").unwrap_or(1);

    let mut appartenance: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    if let Some(m) = doc.get("membres").and_then(|v| v.as_table()) {
        for (strate, liste) in m {
            let Some(r) = rang(strate) else { continue };
            for c in liste.as_array().into_iter().flatten() {
                if let Some(nom) = c.as_str() {
                    appartenance.insert(nom.to_string(), r);
                }
            }
        }
    }
    let strate_de = |c: &str| *appartenance.get(c).unwrap_or(&defaut);

    let deps = deps_forgia();
    let mut violations: Vec<(String, Vec<String>)> = Vec::new();
    let mut dettes: Vec<(String, usize, usize, Vec<String>)> = Vec::new();

    for (crate_, ds) in &deps {
        let dette = doc
            .get("dette")
            .and_then(|d| d.get(crate_.as_str()))
            .and_then(|v| v.as_table());
        if let Some(d) = dette {
            // Crate qui VISE une strate plus basse : on compte l'écart au but.
            let cible = d
                .get("cible")
                .and_then(|v| v.as_str())
                .and_then(rang)
                .unwrap_or(defaut);
            let plafond = d
                .get("plafond")
                .and_then(toml::Value::as_integer)
                .unwrap_or(0) as usize;
            let trop_haut: Vec<String> = ds
                .iter()
                .filter(|x| deps.contains_key(*x) && strate_de(x) > cible)
                .cloned()
                .collect();
            dettes.push((crate_.clone(), trop_haut.len(), plafond, trop_haut));
            continue;
        }
        let sc = strate_de(crate_);
        let mauvais: Vec<String> = ds
            .iter()
            .filter(|x| deps.contains_key(*x) && strate_de(x) > sc)
            .cloned()
            .collect();
        if !mauvais.is_empty() {
            violations.push((crate_.clone(), mauvais));
        }
    }

    println!(
        "[strates] {} crates · {} violation(s) vers le haut · {} dette(s) declaree(s)",
        deps.len(),
        violations.len(),
        dettes.len()
    );
    let mut code = 0;
    for (c, m) in &violations {
        eprintln!(
            "[strates] FAIL — `{c}` depend d'une strate SUPERIEURE : {}. Une crate ne \
             dependant que de sa strate ou plus bas, soit `{c}` monte de strate, soit le \
             type partage descend, soit l'appel s'inverse par un evenement.",
            m.join(", ")
        );
        code = 1;
    }
    for (c, actuel, plafond, liste) in &dettes {
        if actuel > plafond {
            eprintln!(
                "[strates] FAIL — `{c}` : {actuel} dep(s) hors de sa strate cible pour un \
                 plafond de {plafond} ({}). La dette ne peut que BAISSER.",
                liste.join(", ")
            );
            code = 1;
        } else {
            let etat = if actuel < plafond {
                " — a BAISSE, abaisser le plafond dans xtask/strates.toml"
            } else {
                ""
            };
            println!("  dette `{c}` : {actuel}/{plafond}{etat}");
            if verbeux && !liste.is_empty() {
                println!("      {}", liste.join(", "));
            }
        }
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_normalizes_three_in_progress_vocabs() {
        // Les 3 vocabulaires observés dans docs/stories/ tombent sur le même statut.
        assert_eq!(
            classify_status("**Statut** : IN_PROGRESS"),
            StoryStatus::InProgress
        );
        assert_eq!(
            classify_status("Statut : EN COURS"),
            StoryStatus::InProgress
        );
        assert_eq!(
            classify_status("Status: In Progress"),
            StoryStatus::InProgress
        );
    }

    #[test]
    fn classify_done_and_cancelled() {
        assert_eq!(
            classify_status("**Statut** : ✅ DONE 2026-07-01"),
            StoryStatus::Done
        );
        assert_eq!(classify_status("Status: DONE"), StoryStatus::Done);
        assert_eq!(
            classify_status("# story-514\nStatut : ABANDONNÉ"),
            StoryStatus::Cancelled
        );
    }

    #[test]
    fn classify_review_ready_draft() {
        assert_eq!(
            classify_status("Statut : CODE-COMPLETE (attente validation)"),
            StoryStatus::Review
        );
        assert_eq!(classify_status("Status: READY"), StoryStatus::Ready);
        assert_eq!(classify_status("Statut : DRAFT"), StoryStatus::Draft);
    }

    /// Les trois stories inclassables du 2026-08-12 : le mot « statut » apparaît
    /// AVANT la vraie déclaration, dans un titre, un nom de crate ou un symbole.
    #[test]
    fn une_mention_du_mot_statut_nest_pas_une_declaration() {
        assert!(!is_status_declaration(
            "# Story-476 — `forgia-status-effects` (P0 V7)"
        ));
        assert!(!is_status_declaration(
            "# Story-654 — Nameplate v2 : icônes de statut"
        ));
        assert!(!is_status_declaration("> … symbole `StatusShock`."));
        assert!(!is_status_declaration("(fichier `status.rs`)"));

        assert!(is_status_declaration("> **Statut** : CANCELLED"));
        assert!(is_status_declaration("Statut : EN COURS"));
        assert!(is_status_declaration("Status: DONE"));
        assert!(is_status_declaration("**Status**: Review"));
    }

    /// Le cas complet : un titre piégeur suivi d'une vraie déclaration. Avant le
    /// correctif, cette story ressortait UNKNOWN quel que soit son statut.
    #[test]
    fn un_titre_piegeur_ne_masque_plus_le_vrai_statut() {
        let head = "# Story-476 — `forgia-status-effects` (P0 V7)\n\n> **Statut** : CANCELLED";
        assert_eq!(classify_status(head), StoryStatus::Cancelled);
    }

    #[test]
    fn classify_unknown_without_status_line() {
        assert_eq!(
            classify_status("# Story-596 — Roguefight UI\n\nUn paragraphe sans statut."),
            StoryStatus::Unknown
        );
    }

    #[test]
    fn title_strips_story_prefix() {
        assert_eq!(
            extract_title(
                "# Story-596 — Roguefight UI Modernization\n",
                "story-596-x.md"
            ),
            "Roguefight UI Modernization"
        );
    }

    #[test]
    fn orphan_check_parses_workspace_and_renamed_dependencies() {
        let workspace = vec![
            "forgia-core".to_string(),
            "forgia-combat".to_string(),
            "forgia-ui".to_string(),
        ];
        let manifest = r#"
            [dependencies]
            forgia-core = { workspace = true }
            combat_alias = { package = "forgia-combat", workspace = true }

            [target.'cfg(windows)'.build-dependencies]
            forgia-ui = { workspace = true }

            # forgia-ui = { workspace = true } -- commentaire, pas une dépendance
        "#;

        let deps = manifest_internal_dependencies(manifest, &workspace).unwrap();
        assert_eq!(deps, vec!["forgia-combat", "forgia-core", "forgia-ui"]);
    }

    #[test]
    fn orphan_check_ignores_external_dependencies() {
        let workspace = vec!["forgia-core".to_string()];
        let deps = manifest_internal_dependencies(
            "[dependencies]\nbevy = \"0.18\"\nserde = { version = \"1\" }\n",
            &workspace,
        )
        .unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn orphan_check_ignores_workspace_dependency_catalogue() {
        let workspace = vec!["forgia-core".to_string()];
        let deps = manifest_internal_dependencies(
            "[workspace.dependencies]\nforgia-core = { path = \"crates/forgia-core\" }\n",
            &workspace,
        )
        .unwrap();
        assert!(deps.is_empty());
    }
}
