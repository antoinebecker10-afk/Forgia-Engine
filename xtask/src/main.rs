//! # xtask — Forgia V2 automation
//!
//! Tasks :
//! - `check-orphans` : détecte plugins définis non wirés, sensors sans producteur, fields FpsTuning jamais lus
//! - `schedule-dump` : dump Bevy schedule en .dot/.svg pour audit GameSet ordering
//! - `baseline-e1-e2` : génère asset_load_whitelist.txt baseline

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");

    match cmd {
        "check-orphans" => check_orphans(),
        "schedule-dump" => schedule_dump(),
        "baseline-e1-e2" => baseline_e1_e2(),
        _ => print_help(),
    }
}

fn print_help() {
    println!("xtask — Forgia V2 automation");
    println!();
    println!("Commands :");
    println!("  check-orphans     Detect plugins/sensors/fields orphans");
    println!("  schedule-dump     Dump Bevy schedule for GameSet audit");
    println!("  baseline-e1-e2    Regenerate asset_load_whitelist.txt baseline");
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
