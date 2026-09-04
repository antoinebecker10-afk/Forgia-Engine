//! # forgia-assets-bundle
//!
//! Asset pack **manifest** + **lockfile** schema. Pattern Cargo.lock adapté
//! aux packs d'assets externes (KayKit, Quaternius, Poly Haven CC0).
//!
//! ## Vue d'ensemble
//!
//! - `packs.toml` (manifest, committed) déclare *qui* sont les packs : nom,
//!   version, URL de téléchargement, install_dir, license.
//! - `packs.lock` (lockfile, committed) gèle l'état exact : SHA256 réel,
//!   size_bytes, file_count après extraction.
//! - Le contenu (`assets/models-v1/packs/<install_dir>/`) est gitignored.
//!
//! Ce crate ne fait QUE la couche données : parsing TOML, validation,
//! reconcile manifest↔lockfile. Pas de I/O réseau ni d'extraction zip — c'est
//! le rôle de `forgia-asset-cdn` (couche fetch+verify+install).
//!
//! ## Industry references
//!
//! - Cargo.lock : content-addressed pinning (Rust)
//! - Unity Addressables `catalog.json` + `catalog.hash` sidecar
//! - cargo-binstall metadata block + checksum strategy
//!
//! ## Exemple
//!
//! ```no_run
//! use forgia_assets_bundle::{PackManifest, PackLockfile, reconcile};
//!
//! let manifest = PackManifest::load_from_path("assets/packs.toml").unwrap();
//! let lockfile = PackLockfile::load_from_path("assets/packs.lock").unwrap();
//! let diff = reconcile(&manifest, &lockfile);
//! for entry in diff.missing_from_lock {
//!     println!("Pack '{}' in manifest but not locked", entry.name);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use thiserror::Error;

// ── Errors ──────────────────────────────────────────────────────────────────

/// Erreurs unifiées du parser manifest + lockfile. Box des sources lourdes
/// (toml::de::Error fait ~280 bytes) pour garder l'enum compact et éviter
/// `result_large_err` clippy.
#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("failed to read file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: Box<std::io::Error>,
    },
    #[error("failed to parse TOML at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    #[error("failed to serialize TOML: {0}")]
    Serialize(Box<toml::ser::Error>),
    #[error("schema version {found} not supported (expected {expected})")]
    SchemaVersion { found: u32, expected: u32 },
    #[error("duplicate pack name '{0}' in manifest")]
    DuplicatePack(String),
    #[error("pack '{name}' has invalid SHA256 hex (expected 64 chars, got {len})")]
    InvalidSha256 { name: String, len: usize },
    #[error("pack '{name}' has empty {field}")]
    EmptyField { name: String, field: &'static str },
}

impl From<toml::ser::Error> for ManifestError {
    fn from(e: toml::ser::Error) -> Self {
        Self::Serialize(Box::new(e))
    }
}

// ── Manifest schema ─────────────────────────────────────────────────────────

/// Version actuelle du schema manifest + lockfile. Bumper si breaking change
/// (champ obligatoire ajouté, sémantique modifiée). Backward-compat via
/// migration explicite, jamais silencieuse.
pub const SCHEMA_VERSION: u32 = 1;

/// Racine `assets/packs.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    pub meta: ManifestMeta,
    /// Au moins 1 pack normalement, peut être vide en dev.
    #[serde(default, rename = "pack")]
    pub packs: Vec<PackEntry>,
}

/// Bloc `[meta]` du manifest. Schema version + defaults globaux.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestMeta {
    pub schema_version: u32,
    /// Racine d'install par défaut, relative au workspace. Override par-pack
    /// via `install_dir` (qui peut être chemin absolu OU relatif à cette racine).
    pub default_install_root: String,
}

/// Une entrée pack dans `[[pack]]`. Donnée déclarative pure ; le fetcher s'en
/// sert pour télécharger + verify + extraire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackEntry {
    /// Identifiant unique (kebab-case). Sert de clé HashMap + nom de dir cache.
    pub name: String,
    /// Version sémantique ou tag fournisseur (libre format).
    pub version: String,
    /// Page humaine du pack (itch.io, polyhaven.com, etc.). Affichée si
    /// l'installer échoue, pour permettre à l'user de retélécharger manuellement.
    pub source_page: String,
    /// URL directe du .zip téléchargeable. Phase 1 = single URL ; Phase 2 ajoutera
    /// `mirror_urls: Vec<String>` pour hedge contre lien mort.
    pub download_url: String,
    /// SHA256 hex 64 chars du .zip. Optionnel en manifest (peut être rempli par
    /// `forgia-asset-cdn verify --emit-manifest`), **obligatoire** dans le lockfile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Taille attendue du .zip en bytes (sanity check pré-download).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Sous-dossier d'install (relatif à `default_install_root`). Par
    /// exemple "kaykit-medieval" extrait vers `assets/models-v1/packs/kaykit-medieval/`.
    pub install_dir: String,
    /// Identifiant SPDX (`CC0-1.0`, `CC-BY-4.0`, etc.). Obligatoire pour respect
    /// attribution + audit licences.
    pub license: String,
    /// Type de kit pour grouper en UI ("medieval", "dungeon", "characters",
    /// "nature", "weapons"...). Pas typé enum exprès (extensible user-side).
    pub kit_type: String,
    /// Si non vide, retire les N premiers composants de chaque entry zip avant
    /// extraction. Utile quand le zip contient un dossier wrapper (`KayKit_v1/...`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub strip_prefix: String,
}

impl PackManifest {
    /// Charge depuis disque.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path).map_err(|e| ManifestError::Io {
            path: path.to_path_buf(),
            source: Box::new(e),
        })?;
        let manifest: Self = toml::from_str(&raw).map_err(|e| ManifestError::Parse {
            path: path.to_path_buf(),
            source: Box::new(e),
        })?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Parse depuis string. Pratique pour tests sans I/O.
    pub fn parse(toml_str: &str) -> Result<Self, ManifestError> {
        let manifest: Self = toml::from_str(toml_str).map_err(|e| ManifestError::Parse {
            path: PathBuf::from("<string>"),
            source: Box::new(e),
        })?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Sérialise en TOML pretty.
    pub fn to_toml_string(&self) -> Result<String, ManifestError> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Cherche un pack par nom.
    pub fn find(&self, name: &str) -> Option<&PackEntry> {
        self.packs.iter().find(|p| p.name == name)
    }

    /// Validation des invariants. Appelée auto par `load_from_path` + `parse`.
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.meta.schema_version != SCHEMA_VERSION {
            return Err(ManifestError::SchemaVersion {
                found: self.meta.schema_version,
                expected: SCHEMA_VERSION,
            });
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for pack in &self.packs {
            if !seen.insert(&pack.name) {
                return Err(ManifestError::DuplicatePack(pack.name.clone()));
            }
            ensure_non_empty(&pack.name, "name", &pack.name)?;
            ensure_non_empty(&pack.name, "version", &pack.version)?;
            ensure_non_empty(&pack.name, "source_page", &pack.source_page)?;
            ensure_non_empty(&pack.name, "download_url", &pack.download_url)?;
            ensure_non_empty(&pack.name, "install_dir", &pack.install_dir)?;
            ensure_non_empty(&pack.name, "license", &pack.license)?;
            ensure_non_empty(&pack.name, "kit_type", &pack.kit_type)?;
            if let Some(sha) = &pack.sha256 {
                if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(ManifestError::InvalidSha256 {
                        name: pack.name.clone(),
                        len: sha.len(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Calcule le chemin absolu d'install pour un pack relatif au workspace_root.
    pub fn install_path(&self, pack: &PackEntry, workspace_root: &Path) -> PathBuf {
        workspace_root
            .join(&self.meta.default_install_root)
            .join(&pack.install_dir)
    }
}

fn ensure_non_empty(name: &str, field: &'static str, value: &str) -> Result<(), ManifestError> {
    if value.trim().is_empty() {
        Err(ManifestError::EmptyField {
            name: name.to_string(),
            field,
        })
    } else {
        Ok(())
    }
}

// ── Lockfile schema ─────────────────────────────────────────────────────────

/// Racine `assets/packs.lock`. Pinning content-addressed style Cargo.lock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackLockfile {
    pub schema_version: u32,
    /// Timestamp ISO8601 UTC de génération. Lisible humain pour diff PR.
    pub generated_at: String,
    #[serde(default, rename = "locked")]
    pub entries: Vec<LockEntry>,
}

/// Une entrée verrouillée. SHA256 et size_bytes sont obligatoires (vs Option
/// dans `PackEntry`). C'est la promesse : "ce contenu exact a été vérifié".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockEntry {
    pub name: String,
    pub version: String,
    pub download_url: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub install_dir: String,
    /// Nombre de fichiers extraits (sanity check post-install).
    pub file_count: u32,
}

impl PackLockfile {
    /// Crée un lockfile vide (utilisé par `cdn install --regen-lock`).
    pub fn empty(generated_at: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            generated_at: generated_at.into(),
            entries: Vec::new(),
        }
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ManifestError> {
        let path = path.as_ref();
        let raw = std::fs::read_to_string(path).map_err(|e| ManifestError::Io {
            path: path.to_path_buf(),
            source: Box::new(e),
        })?;
        let lock: Self = toml::from_str(&raw).map_err(|e| ManifestError::Parse {
            path: path.to_path_buf(),
            source: Box::new(e),
        })?;
        lock.validate()?;
        Ok(lock)
    }

    pub fn parse(toml_str: &str) -> Result<Self, ManifestError> {
        let lock: Self = toml::from_str(toml_str).map_err(|e| ManifestError::Parse {
            path: PathBuf::from("<string>"),
            source: Box::new(e),
        })?;
        lock.validate()?;
        Ok(lock)
    }

    pub fn to_toml_string(&self) -> Result<String, ManifestError> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn find(&self, name: &str) -> Option<&LockEntry> {
        self.entries.iter().find(|p| p.name == name)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ManifestError::SchemaVersion {
                found: self.schema_version,
                expected: SCHEMA_VERSION,
            });
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for entry in &self.entries {
            if !seen.insert(&entry.name) {
                return Err(ManifestError::DuplicatePack(entry.name.clone()));
            }
            if entry.sha256.len() != 64 || !entry.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(ManifestError::InvalidSha256 {
                    name: entry.name.clone(),
                    len: entry.sha256.len(),
                });
            }
        }
        Ok(())
    }

    /// Upsert : remplace l'entrée par nom, ou push si absent.
    pub fn upsert(&mut self, entry: LockEntry) {
        if let Some(pos) = self.entries.iter().position(|e| e.name == entry.name) {
            self.entries[pos] = entry;
        } else {
            self.entries.push(entry);
        }
    }
}

// ── Reconciliation ──────────────────────────────────────────────────────────

/// Diff entre manifest et lockfile. Permet à l'installer de savoir quoi faire.
#[derive(Debug, Default)]
pub struct ReconcileReport<'a> {
    /// Packs déclarés mais pas encore lockés (à installer).
    pub missing_from_lock: Vec<&'a PackEntry>,
    /// Packs lockés mais retirés du manifest (à supprimer du disque).
    pub orphan_in_lock: Vec<&'a LockEntry>,
    /// Packs présents des deux côtés mais version différente (à réinstaller).
    pub version_mismatch: Vec<(&'a PackEntry, &'a LockEntry)>,
    /// Packs présents des deux côtés avec version identique mais sha256
    /// déclaré dans le manifest qui diffère du lockfile (manifest changé).
    pub sha_mismatch: Vec<(&'a PackEntry, &'a LockEntry)>,
    /// Packs OK des deux côtés (aucune action requise).
    pub ok: Vec<(&'a PackEntry, &'a LockEntry)>,
}

impl<'a> ReconcileReport<'a> {
    pub fn needs_action(&self) -> bool {
        !self.missing_from_lock.is_empty()
            || !self.orphan_in_lock.is_empty()
            || !self.version_mismatch.is_empty()
            || !self.sha_mismatch.is_empty()
    }
}

/// Compare le manifest source-of-truth vs le lockfile content-addressed.
pub fn reconcile<'a>(
    manifest: &'a PackManifest,
    lockfile: &'a PackLockfile,
) -> ReconcileReport<'a> {
    let mut report = ReconcileReport::default();
    let lock_names: HashSet<&str> = lockfile.entries.iter().map(|e| e.name.as_str()).collect();

    for pack in &manifest.packs {
        match lockfile.find(&pack.name) {
            None => report.missing_from_lock.push(pack),
            Some(lock) => {
                if pack.version != lock.version {
                    report.version_mismatch.push((pack, lock));
                } else if matches!(&pack.sha256, Some(s) if s != &lock.sha256) {
                    report.sha_mismatch.push((pack, lock));
                } else {
                    report.ok.push((pack, lock));
                }
            }
        }
    }

    let manifest_names: HashSet<&str> = manifest.packs.iter().map(|p| p.name.as_str()).collect();
    for entry in &lockfile.entries {
        if !lock_names.contains(entry.name.as_str()) {
            continue; // unreachable but defensive
        }
        if !manifest_names.contains(entry.name.as_str()) {
            report.orphan_in_lock.push(entry);
        }
    }

    report
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_MANIFEST: &str = r#"
[meta]
schema_version = 1
default_install_root = "assets/models-v1/packs"

[[pack]]
name = "kaykit-medieval"
version = "1.0.2"
source_page = "https://kaylousberg.itch.io/kit-medieval-hexagon"
download_url = "https://example.com/kaykit-medieval.zip"
install_dir = "kaykit-medieval"
license = "CC0-1.0"
kit_type = "medieval"
"#;

    const MINIMAL_LOCKFILE: &str = r#"
schema_version = 1
generated_at = "2026-05-18T09:30:00Z"

[[locked]]
name = "kaykit-medieval"
version = "1.0.2"
download_url = "https://example.com/kaykit-medieval.zip"
sha256 = "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
size_bytes = 98566144
install_dir = "kaykit-medieval"
file_count = 152
"#;

    #[test]
    fn parse_minimal_manifest_ok() {
        let m = PackManifest::parse(MINIMAL_MANIFEST).unwrap();
        assert_eq!(m.packs.len(), 1);
        assert_eq!(m.packs[0].name, "kaykit-medieval");
        assert_eq!(m.packs[0].license, "CC0-1.0");
    }

    #[test]
    fn parse_minimal_lockfile_ok() {
        let l = PackLockfile::parse(MINIMAL_LOCKFILE).unwrap();
        assert_eq!(l.entries.len(), 1);
        assert_eq!(l.entries[0].sha256.len(), 64);
    }

    #[test]
    fn round_trip_manifest() {
        let m = PackManifest::parse(MINIMAL_MANIFEST).unwrap();
        let serialized = m.to_toml_string().unwrap();
        let reparsed = PackManifest::parse(&serialized).unwrap();
        assert_eq!(m.packs, reparsed.packs);
    }

    #[test]
    fn schema_version_mismatch_rejected() {
        let bad = MINIMAL_MANIFEST.replace("schema_version = 1", "schema_version = 999");
        let err = PackManifest::parse(&bad).unwrap_err();
        matches!(err, ManifestError::SchemaVersion { .. });
    }

    #[test]
    fn duplicate_pack_name_rejected() {
        let dup = format!(
            "{MINIMAL_MANIFEST}\n[[pack]]\nname = \"kaykit-medieval\"\n\
             version = \"1.0.0\"\nsource_page = \"x\"\ndownload_url = \"x\"\n\
             install_dir = \"x\"\nlicense = \"CC0-1.0\"\nkit_type = \"x\"\n"
        );
        let err = PackManifest::parse(&dup).unwrap_err();
        matches!(err, ManifestError::DuplicatePack(_));
    }

    #[test]
    fn invalid_sha256_rejected() {
        let bad = r#"
schema_version = 1
generated_at = "2026-05-18T09:30:00Z"

[[locked]]
name = "kaykit-medieval"
version = "1.0.2"
download_url = "https://example.com/x.zip"
sha256 = "not-a-hash"
size_bytes = 1
install_dir = "x"
file_count = 1
"#;
        let err = PackLockfile::parse(bad).unwrap_err();
        matches!(err, ManifestError::InvalidSha256 { .. });
    }

    #[test]
    fn empty_field_rejected() {
        let bad = MINIMAL_MANIFEST.replace(r#"license = "CC0-1.0""#, r#"license = """#);
        let err = PackManifest::parse(&bad).unwrap_err();
        matches!(
            err,
            ManifestError::EmptyField {
                field: "license",
                ..
            }
        );
    }

    #[test]
    fn reconcile_all_ok() {
        let m = PackManifest::parse(MINIMAL_MANIFEST).unwrap();
        let l = PackLockfile::parse(MINIMAL_LOCKFILE).unwrap();
        let r = reconcile(&m, &l);
        assert_eq!(r.ok.len(), 1);
        assert!(!r.needs_action());
    }

    #[test]
    fn reconcile_missing_from_lock() {
        let m = PackManifest::parse(MINIMAL_MANIFEST).unwrap();
        let l = PackLockfile::empty("now");
        let r = reconcile(&m, &l);
        assert_eq!(r.missing_from_lock.len(), 1);
        assert_eq!(r.missing_from_lock[0].name, "kaykit-medieval");
        assert!(r.needs_action());
    }

    #[test]
    fn reconcile_orphan_in_lock() {
        let empty_manifest = r#"
[meta]
schema_version = 1
default_install_root = "assets/models-v1/packs"
"#;
        let m = PackManifest::parse(empty_manifest).unwrap();
        let l = PackLockfile::parse(MINIMAL_LOCKFILE).unwrap();
        let r = reconcile(&m, &l);
        assert_eq!(r.orphan_in_lock.len(), 1);
        assert!(r.needs_action());
    }

    #[test]
    fn reconcile_version_mismatch() {
        let m = PackManifest::parse(MINIMAL_MANIFEST).unwrap();
        let bumped = MINIMAL_LOCKFILE.replace(r#"version = "1.0.2""#, r#"version = "1.0.3""#);
        let l = PackLockfile::parse(&bumped).unwrap();
        let r = reconcile(&m, &l);
        assert_eq!(r.version_mismatch.len(), 1);
        assert!(r.needs_action());
    }

    #[test]
    fn lockfile_upsert_replaces_existing() {
        let mut l = PackLockfile::parse(MINIMAL_LOCKFILE).unwrap();
        let mut updated = l.entries[0].clone();
        updated.file_count = 999;
        l.upsert(updated);
        assert_eq!(l.entries.len(), 1);
        assert_eq!(l.entries[0].file_count, 999);
    }

    #[test]
    fn lockfile_upsert_adds_new() {
        let mut l = PackLockfile::parse(MINIMAL_LOCKFILE).unwrap();
        l.upsert(LockEntry {
            name: "kaykit-dungeon".into(),
            version: "1.0.0".into(),
            download_url: "https://example.com/d.zip".into(),
            sha256: "0".repeat(64),
            size_bytes: 0,
            install_dir: "kaykit-dungeon".into(),
            file_count: 0,
        });
        assert_eq!(l.entries.len(), 2);
    }

    #[test]
    fn install_path_relative_to_workspace() {
        let m = PackManifest::parse(MINIMAL_MANIFEST).unwrap();
        let p = m.install_path(&m.packs[0], Path::new("/workspace"));
        assert_eq!(
            p,
            Path::new("/workspace/assets/models-v1/packs/kaykit-medieval")
        );
    }
}
