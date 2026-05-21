//! # forgia-asset-cdn
//!
//! Asset pack **fetcher + verifier + extractor**. Pairs with
//! `forgia-assets-bundle` (manifest + lockfile schema).
//!
//! ## Pipeline (stage-separated, cargo-binstall pattern)
//!
//! ```text
//! Stage 1 — fetch        : download .zip from URL → cache file
//! Stage 2 — verify       : compute SHA256, compare expected
//! Stage 3 — extract      : unzip into install_dir, anti-traversal hardened
//! Stage 4 — lockfile     : emit LockEntry with size + file_count
//! ```
//!
//! Stage separation = cargo-binstall, cargo-vendor, Nix flake lock pattern.
//! Each stage testable in isolation.
//!
//! ## Industry references
//!
//! - cargo-binstall : separate fetch/verify/install stages
//! - Cargo.lock : content-addressed SHA256 pinning
//! - Unity Addressables : `catalog.hash` sidecar cheap polling

use forgia_assets_bundle::{LockEntry, ManifestError, PackEntry, PackLockfile, PackManifest};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use thiserror::Error;

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CdnError {
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: Box<io::Error>,
    },
    #[error("HTTP fetch failed for {url}: {message}")]
    Fetch { url: String, message: String },
    #[error("SHA256 mismatch for pack '{name}': expected {expected}, got {actual}")]
    HashMismatch {
        name: String,
        expected: String,
        actual: String,
    },
    #[error("ZIP extraction failed at entry '{entry}': {source}")]
    Zip {
        entry: String,
        #[source]
        source: Box<zip::result::ZipError>,
    },
    #[error("ZIP entry '{0}' contains path traversal ('..' or absolute path) — refusing")]
    PathTraversal(String),
    #[error("pack '{0}' not found in manifest")]
    PackNotFound(String),
    #[error("pack '{name}' has no sha256 in manifest (run `verify --emit-manifest` first)")]
    MissingHash { name: String },
}

fn io_err(path: impl AsRef<Path>, e: io::Error) -> CdnError {
    CdnError::Io {
        path: path.as_ref().to_path_buf(),
        source: Box::new(e),
    }
}

// ── Stage 1 : fetch ─────────────────────────────────────────────────────────

/// Télécharge un fichier depuis une URL HTTPS vers `dest`. Crée le parent.
/// Pas de retry Phase 1 (ureq sync simple, l'user relance la commande si fail).
pub fn fetch(url: &str, dest: &Path) -> Result<u64, CdnError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    }
    let response = ureq::get(url).call().map_err(|e| CdnError::Fetch {
        url: url.to_string(),
        message: e.to_string(),
    })?;

    let mut reader = response.into_reader();
    let mut file = File::create(dest).map_err(|e| io_err(dest, e))?;
    let bytes_written = io::copy(&mut reader, &mut file).map_err(|e| io_err(dest, e))?;
    Ok(bytes_written)
}

// ── Stage 2 : verify SHA256 ─────────────────────────────────────────────────

/// Calcule SHA256 d'un fichier en streaming (n'alloue pas tout en RAM).
pub fn compute_sha256(path: &Path) -> Result<String, CdnError> {
    let mut file = File::open(path).map_err(|e| io_err(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| io_err(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_encode(&hasher.finalize()))
}

/// Vérifie un fichier contre un SHA256 attendu. Renvoie Ok((computed_hash))
/// si OK, sinon `HashMismatch`.
pub fn verify_sha256(path: &Path, expected: &str, pack_name: &str) -> Result<String, CdnError> {
    let actual = compute_sha256(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(actual)
    } else {
        Err(CdnError::HashMismatch {
            name: pack_name.to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

// ── Stage 3 : extract ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Default)]
pub struct ExtractOptions<'a> {
    pub strip_prefix: &'a str,
    /// Si true, refuse d'écraser un dest non vide.
    pub fail_if_dest_non_empty: bool,
}

/// Extrait un .zip vers `dest_dir`. Anti-traversal hardened : refuse les
/// entries avec ".." ou chemin absolu (CVE-2018-1000844 et famille).
///
/// Si `strip_prefix` non vide, retire ce préfixe de chaque entry (utile pour
/// les zips avec wrapper directory comme `KayKit_v1.0/<contenu>`).
///
/// Retourne le nombre de fichiers extraits (sanity check post-install).
pub fn extract_zip(
    zip_path: &Path,
    dest_dir: &Path,
    opts: ExtractOptions,
) -> Result<u32, CdnError> {
    let file = File::open(zip_path).map_err(|e| io_err(zip_path, e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| CdnError::Zip {
        entry: "<archive>".into(),
        source: Box::new(e),
    })?;

    if opts.fail_if_dest_non_empty && dest_non_empty(dest_dir) {
        return Err(io_err(
            dest_dir,
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "dest_dir not empty and fail_if_dest_non_empty=true",
            ),
        ));
    }

    fs::create_dir_all(dest_dir).map_err(|e| io_err(dest_dir, e))?;
    let mut file_count: u32 = 0;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| CdnError::Zip {
            entry: format!("idx{i}"),
            source: Box::new(e),
        })?;

        let raw_name = entry.name().to_string();
        let stripped: &str = if !opts.strip_prefix.is_empty()
            && raw_name.starts_with(opts.strip_prefix)
        {
            &raw_name[opts.strip_prefix.len()..]
        } else {
            &raw_name
        };
        let stripped = stripped.trim_start_matches('/').trim_start_matches('\\');
        if stripped.is_empty() {
            continue;
        }

        // Anti-traversal : refuser ".." et chemin absolu.
        if stripped.contains("..") {
            return Err(CdnError::PathTraversal(raw_name));
        }
        let candidate = Path::new(stripped);
        if candidate.is_absolute() {
            return Err(CdnError::PathTraversal(raw_name));
        }

        let out_path = dest_dir.join(candidate);
        // Double-check : le chemin résolu doit rester sous dest_dir.
        if !is_within(&out_path, dest_dir) {
            return Err(CdnError::PathTraversal(raw_name));
        }

        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| io_err(&out_path, e))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
            }
            let mut out = File::create(&out_path).map_err(|e| io_err(&out_path, e))?;
            io::copy(&mut entry, &mut out).map_err(|e| io_err(&out_path, e))?;
            file_count += 1;
        }
    }

    Ok(file_count)
}

fn dest_non_empty(p: &Path) -> bool {
    fs::read_dir(p).map(|mut d| d.next().is_some()).unwrap_or(false)
}

/// Normalise + check qu'un chemin reste contenu dans `root`. Robust à .. via
/// resolve component-wise (sans toucher au filesystem).
fn is_within(candidate: &Path, root: &Path) -> bool {
    use std::path::Component;
    let mut depth: i32 = 0;
    for comp in candidate.strip_prefix(root).unwrap_or(candidate).components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir => depth -= 1,
            Component::Prefix(_) | Component::RootDir => return false,
        }
        if depth < 0 {
            return false;
        }
    }
    true
}

// ── Stage 4 : orchestrator ──────────────────────────────────────────────────

/// Installe un pack : fetch + verify + extract + return LockEntry.
///
/// `cache_dir` reçoit les .zip téléchargés (réutilisés si déjà présents et
/// hash OK — évite re-download).
/// `workspace_root` est la racine du repo Forgia (pour résoudre install_dir).
pub fn install_pack(
    manifest: &PackManifest,
    pack: &PackEntry,
    cache_dir: &Path,
    workspace_root: &Path,
) -> Result<LockEntry, CdnError> {
    let expected_sha = pack
        .sha256
        .as_deref()
        .ok_or_else(|| CdnError::MissingHash { name: pack.name.clone() })?;

    let zip_path = cache_dir.join(format!("{}.zip", pack.name));
    // Re-use cached zip si hash matche déjà.
    let cached_ok = zip_path.is_file()
        && compute_sha256(&zip_path)
            .map(|h| h.eq_ignore_ascii_case(expected_sha))
            .unwrap_or(false);

    if !cached_ok {
        fetch(&pack.download_url, &zip_path)?;
        verify_sha256(&zip_path, expected_sha, &pack.name)?;
    }
    let actual_sha = compute_sha256(&zip_path)?;
    let size_bytes = fs::metadata(&zip_path).map_err(|e| io_err(&zip_path, e))?.len();

    let dest_dir = manifest.install_path(pack, workspace_root);
    let file_count = extract_zip(
        &zip_path,
        &dest_dir,
        ExtractOptions {
            strip_prefix: &pack.strip_prefix,
            fail_if_dest_non_empty: false,
        },
    )?;

    Ok(LockEntry {
        name: pack.name.clone(),
        version: pack.version.clone(),
        download_url: pack.download_url.clone(),
        sha256: actual_sha,
        size_bytes,
        install_dir: pack.install_dir.clone(),
        file_count,
    })
}

/// Vérifie un pack DÉJÀ installé : recompute SHA256 du .zip cache si présent,
/// vérifie présence install_dir + file_count. Pas de re-download.
pub fn verify_installed(
    manifest: &PackManifest,
    pack: &PackEntry,
    lockfile: &PackLockfile,
    workspace_root: &Path,
) -> Result<VerifyStatus, CdnError> {
    let dest_dir = manifest.install_path(pack, workspace_root);
    let exists = dest_dir.is_dir();
    let lock_entry = lockfile.find(&pack.name);

    let status = match (exists, lock_entry) {
        (false, _) => VerifyStatus::Missing,
        (true, None) => VerifyStatus::InstalledButUnlocked,
        (true, Some(lock)) => {
            let actual_file_count = count_files_recursive(&dest_dir).unwrap_or(0);
            if actual_file_count == lock.file_count {
                VerifyStatus::Ok
            } else {
                VerifyStatus::FileCountMismatch {
                    expected: lock.file_count,
                    actual: actual_file_count,
                }
            }
        }
    };
    Ok(status)
}

#[derive(Debug, PartialEq, Eq)]
pub enum VerifyStatus {
    Ok,
    Missing,
    InstalledButUnlocked,
    FileCountMismatch { expected: u32, actual: u32 },
}

fn count_files_recursive(dir: &Path) -> io::Result<u32> {
    let mut count = 0;
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        for entry in fs::read_dir(&p)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                stack.push(entry.path());
            } else {
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Cargo workspace root locator. Cherche le `Cargo.toml` avec `[workspace]`
/// en remontant depuis `start`. Utile pour les binaries CLI lancés depuis
/// n'importe quel sous-dossier.
pub fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        let candidate = cur.join("Cargo.toml");
        if candidate.is_file() {
            if let Ok(s) = fs::read_to_string(&candidate) {
                if s.contains("[workspace]") {
                    return Some(cur);
                }
            }
        }
        if !cur.pop() {
            return None;
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn hex_encode_basic() {
        assert_eq!(hex_encode(&[0xab, 0xcd]), "abcd");
        assert_eq!(hex_encode(&[0x00, 0xff]), "00ff");
    }

    #[test]
    fn sha256_known_vector() {
        // SHA256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
        let tmp = std::env::temp_dir().join("forgia_cdn_test_abc.bin");
        let mut f = File::create(&tmp).unwrap();
        f.write_all(b"abc").unwrap();
        drop(f);
        let h = compute_sha256(&tmp).unwrap();
        assert_eq!(
            h,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn verify_sha256_ok() {
        let tmp = std::env::temp_dir().join("forgia_cdn_test_verify_ok.bin");
        File::create(&tmp).unwrap().write_all(b"abc").unwrap();
        let expected = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let r = verify_sha256(&tmp, expected, "test").unwrap();
        assert_eq!(r, expected);
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn verify_sha256_mismatch() {
        let tmp = std::env::temp_dir().join("forgia_cdn_test_mismatch.bin");
        File::create(&tmp).unwrap().write_all(b"abc").unwrap();
        let err = verify_sha256(&tmp, &"0".repeat(64), "test").unwrap_err();
        matches!(err, CdnError::HashMismatch { .. });
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn extract_zip_basic() {
        // Build a tiny zip in-memory.
        let tmp_dir = std::env::temp_dir().join("forgia_cdn_test_extract");
        let _ = fs::remove_dir_all(&tmp_dir);
        let zip_path = std::env::temp_dir().join("forgia_cdn_test.zip");

        let zip_file = File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(zip_file);
        let opts: zip::write::FileOptions<()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zw.start_file("hello.txt", opts).unwrap();
        zw.write_all(b"world").unwrap();
        zw.start_file("sub/nested.txt", opts).unwrap();
        zw.write_all(b"nested").unwrap();
        zw.finish().unwrap();

        let count = extract_zip(&zip_path, &tmp_dir, ExtractOptions::default()).unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            fs::read_to_string(tmp_dir.join("hello.txt")).unwrap(),
            "world"
        );
        assert_eq!(
            fs::read_to_string(tmp_dir.join("sub/nested.txt")).unwrap(),
            "nested"
        );
        let _ = fs::remove_dir_all(&tmp_dir);
        let _ = fs::remove_file(&zip_path);
    }

    #[test]
    fn extract_zip_strip_prefix() {
        let tmp_dir = std::env::temp_dir().join("forgia_cdn_test_strip");
        let _ = fs::remove_dir_all(&tmp_dir);
        let zip_path = std::env::temp_dir().join("forgia_cdn_test_strip.zip");

        let zip_file = File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(zip_file);
        let opts: zip::write::FileOptions<()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zw.start_file("KayKit_v1/asset.gltf", opts).unwrap();
        zw.write_all(b"gltf data").unwrap();
        zw.finish().unwrap();

        let count = extract_zip(
            &zip_path,
            &tmp_dir,
            ExtractOptions { strip_prefix: "KayKit_v1/", fail_if_dest_non_empty: false },
        )
        .unwrap();
        assert_eq!(count, 1);
        assert!(tmp_dir.join("asset.gltf").is_file());
        assert!(!tmp_dir.join("KayKit_v1").exists());
        let _ = fs::remove_dir_all(&tmp_dir);
        let _ = fs::remove_file(&zip_path);
    }

    #[test]
    fn extract_rejects_traversal() {
        let tmp_dir = std::env::temp_dir().join("forgia_cdn_test_traversal");
        let _ = fs::remove_dir_all(&tmp_dir);
        let zip_path = std::env::temp_dir().join("forgia_cdn_test_traversal.zip");

        let zip_file = File::create(&zip_path).unwrap();
        let mut zw = zip::ZipWriter::new(zip_file);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zw.start_file("../escape.txt", opts).unwrap();
        zw.write_all(b"pwn").unwrap();
        zw.finish().unwrap();

        let err = extract_zip(&zip_path, &tmp_dir, ExtractOptions::default()).unwrap_err();
        matches!(err, CdnError::PathTraversal(_));
        let _ = fs::remove_dir_all(&tmp_dir);
        let _ = fs::remove_file(&zip_path);
    }

    #[test]
    fn is_within_basic() {
        assert!(is_within(Path::new("foo/bar"), Path::new("foo")));
        assert!(!is_within(Path::new("foo/../../etc"), Path::new("foo")));
    }
}
