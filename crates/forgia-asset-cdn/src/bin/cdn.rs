//! # forgia-pack-cdn — CLI
//!
//! Subcommands:
//! - `status`        : compare manifest vs lockfile vs disk
//! - `install [name]`: fetch + verify + extract (all packs ou pack ciblé)
//! - `verify [name]` : recompute SHA256 du zip cache + check file_count installé
//! - `hash <file>`   : helper one-shot pour calculer un SHA256
//! - `regen-lock`    : pour chaque pack avec sha256 défini, install puis update lockfile

use clap::{Parser, Subcommand};
use forgia_asset_cdn::{
    compute_sha256, find_workspace_root, install_pack, verify_installed, CdnError, VerifyStatus,
};
use forgia_assets_bundle::{reconcile, PackLockfile, PackManifest, SCHEMA_VERSION};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "forgia-pack-cdn",
    version,
    about = "Asset pack pipeline CLI — fetch + verify + extract (Cargo.lock pattern)"
)]
struct Cli {
    /// Path to packs.toml (default: assets/packs.toml relative to workspace root)
    #[arg(long, global = true)]
    manifest: Option<PathBuf>,

    /// Path to packs.lock (default: assets/packs.lock relative to workspace root)
    #[arg(long, global = true)]
    lockfile: Option<PathBuf>,

    /// Path to download cache directory (default: target/forgia-pack-cache)
    #[arg(long, global = true)]
    cache: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Print status: manifest entries, locked entries, disk state.
    Status,
    /// Install one pack by name, or all if no name given.
    Install { name: Option<String> },
    /// Verify installed packs match lockfile (file_count + presence).
    Verify { name: Option<String> },
    /// Compute SHA256 of an arbitrary file (helper for filling manifest sha256).
    Hash { path: PathBuf },
    /// Re-generate the lockfile by installing all packs that have a manifest sha256.
    RegenLock,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), CdnError> {
    let workspace = find_workspace_root(&std::env::current_dir().map_err(io_main)?)
        .ok_or_else(|| io_other("workspace root not found (no Cargo.toml with [workspace])"))?;

    let manifest_path = cli
        .manifest
        .clone()
        .unwrap_or_else(|| workspace.join("assets/packs.toml"));
    let lockfile_path = cli
        .lockfile
        .clone()
        .unwrap_or_else(|| workspace.join("assets/packs.lock"));
    let cache_dir = cli
        .cache
        .clone()
        .unwrap_or_else(|| workspace.join("target/forgia-pack-cache"));

    match cli.cmd {
        Cmd::Hash { path } => {
            let h = compute_sha256(&path)?;
            println!("{h}  {}", path.display());
            Ok(())
        }
        Cmd::Status => {
            print_status(&manifest_path, &lockfile_path, &workspace)?;
            Ok(())
        }
        Cmd::Install { name } => {
            let manifest = PackManifest::load_from_path(&manifest_path)?;
            let mut lockfile = if lockfile_path.is_file() {
                PackLockfile::load_from_path(&lockfile_path)?
            } else {
                PackLockfile::empty(now_iso8601())
            };
            let targets: Vec<_> = match &name {
                Some(n) => manifest
                    .packs
                    .iter()
                    .filter(|p| &p.name == n)
                    .collect(),
                None => manifest.packs.iter().collect(),
            };
            if targets.is_empty() {
                return Err(CdnError::PackNotFound(
                    name.unwrap_or_else(|| "<all>".into()),
                ));
            }
            for pack in targets {
                println!("→ installing '{}' ({})", pack.name, pack.version);
                let entry = install_pack(&manifest, pack, &cache_dir, &workspace)?;
                println!(
                    "  OK {} files, {:.1} MB",
                    entry.file_count,
                    entry.size_bytes as f64 / 1_048_576.0
                );
                lockfile.upsert(entry);
            }
            lockfile.generated_at = now_iso8601();
            std::fs::write(&lockfile_path, lockfile.to_toml_string()?)
                .map_err(|e| io_path(&lockfile_path, e))?;
            println!("✔ wrote {}", lockfile_path.display());
            Ok(())
        }
        Cmd::Verify { name } => {
            let manifest = PackManifest::load_from_path(&manifest_path)?;
            let lockfile = PackLockfile::load_from_path(&lockfile_path)?;
            let targets: Vec<_> = match &name {
                Some(n) => manifest.packs.iter().filter(|p| &p.name == n).collect(),
                None => manifest.packs.iter().collect(),
            };
            let mut all_ok = true;
            for pack in targets {
                let status = verify_installed(&manifest, pack, &lockfile, &workspace)?;
                let glyph = match &status {
                    VerifyStatus::Ok => "✓",
                    VerifyStatus::Missing | VerifyStatus::InstalledButUnlocked => "✗",
                    VerifyStatus::FileCountMismatch { .. } => "!",
                };
                println!("{glyph} {} → {:?}", pack.name, status);
                if !matches!(status, VerifyStatus::Ok) {
                    all_ok = false;
                }
            }
            if !all_ok {
                std::process::exit(2);
            }
            Ok(())
        }
        Cmd::RegenLock => {
            // Same as install --all but explicitly writes a fresh lockfile.
            run(Cli {
                manifest: cli.manifest,
                lockfile: cli.lockfile,
                cache: cli.cache,
                cmd: Cmd::Install { name: None },
            })
        }
    }
}

fn print_status(
    manifest_path: &std::path::Path,
    lockfile_path: &std::path::Path,
    workspace: &std::path::Path,
) -> Result<(), CdnError> {
    let manifest = PackManifest::load_from_path(manifest_path)?;
    println!(
        "manifest: {} ({} packs, schema v{})",
        manifest_path.display(),
        manifest.packs.len(),
        manifest.meta.schema_version
    );
    if lockfile_path.is_file() {
        let lockfile = PackLockfile::load_from_path(lockfile_path)?;
        println!(
            "lockfile: {} ({} entries, schema v{})",
            lockfile_path.display(),
            lockfile.entries.len(),
            SCHEMA_VERSION
        );
        let report = reconcile(&manifest, &lockfile);
        println!(
            "  ok={} missing_from_lock={} orphan_in_lock={} version_mismatch={} sha_mismatch={}",
            report.ok.len(),
            report.missing_from_lock.len(),
            report.orphan_in_lock.len(),
            report.version_mismatch.len(),
            report.sha_mismatch.len()
        );
    } else {
        println!("lockfile: <missing> ({})", lockfile_path.display());
    }
    println!("workspace: {}", workspace.display());
    println!("\nDisk state:");
    for pack in &manifest.packs {
        let install_dir = manifest.install_path(pack, workspace);
        let exists = install_dir.is_dir();
        println!(
            "  {} '{}' v{} → {}",
            if exists { "✓" } else { "·" },
            pack.name,
            pack.version,
            install_dir.display()
        );
    }
    Ok(())
}

fn now_iso8601() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn io_main(e: std::io::Error) -> CdnError {
    io_path(std::path::Path::new("."), e)
}

fn io_path(p: &std::path::Path, e: std::io::Error) -> CdnError {
    CdnError::Io {
        path: p.to_path_buf(),
        source: Box::new(e),
    }
}

fn io_other(msg: &str) -> CdnError {
    CdnError::Io {
        path: PathBuf::from("."),
        source: Box::new(std::io::Error::other(msg)),
    }
}
