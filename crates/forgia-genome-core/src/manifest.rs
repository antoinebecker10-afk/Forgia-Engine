//! Capability discovery — parse `manifest.toml` from every crate to build an
//! index for AI assembly agents.
//!
//! Fused from `forgia-manifest` (2026-05-26) — was a 0-consumer scaffold; folded
//! here because both this module and the genome loader share TOML/serde
//! foundations and address the data layer.
//!
//! Each Forgia crate ships a `manifest.toml` declaring :
//! - `name`, `category`, `intent`, `version`, `status`
//! - `genes` (hot-reload exposed parameters)
//! - `emits` / `consumes` (ECS events / components)
//! - `sensor` (forgia_*.json file)
//! - `shader` (WGSL asset path, if applicable)
//! - `depends` (workspace deps)
//!
//! An AI agent assembling a game scans the workspace, filters by category /
//! status, and composes a `Cargo.toml` + `Plugin` list.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityManifest {
    pub name: String,
    pub category: String,
    pub intent: String,
    pub version: String,
    pub status: String,

    #[serde(default)]
    pub genes: Vec<GeneSpec>,
    #[serde(default)]
    pub emits: Vec<String>,
    #[serde(default)]
    pub consumes: Vec<String>,
    #[serde(default)]
    pub sensor: String,
    #[serde(default)]
    pub shader: String,
    #[serde(default)]
    pub depends: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneSpec {
    pub name: String,
    #[serde(default)]
    pub range: Option<[f64; 2]>,
    #[serde(default)]
    pub default: Option<toml::Value>,
    #[serde(default)]
    pub doc: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ManifestError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("crates dir not found: {0}")]
    NotFound(PathBuf),
}

/// Scan `<workspace>/crates/*/manifest.toml` and parse each. Returns sorted by name.
pub fn scan_workspace(crates_dir: &Path) -> Result<Vec<CapabilityManifest>, ManifestError> {
    if !crates_dir.exists() {
        return Err(ManifestError::NotFound(crates_dir.to_path_buf()));
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(crates_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("manifest.toml");
        if !manifest_path.exists() {
            continue;
        }
        let body = fs::read_to_string(&manifest_path)?;
        let m: CapabilityManifest = toml::from_str(&body)?;
        out.push(m);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Resource — index of all parsed manifests, kept for runtime introspection.
#[derive(Resource, Default)]
pub struct ManifestIndex {
    pub all: Vec<CapabilityManifest>,
}

impl ManifestIndex {
    pub fn by_category(&self, category: &str) -> Vec<&CapabilityManifest> {
        self.all.iter().filter(|m| m.category == category).collect()
    }
    pub fn ready(&self) -> Vec<&CapabilityManifest> {
        self.all
            .iter()
            .filter(|m| m.status == "ready" || m.status == "stable")
            .collect()
    }
}

pub struct ForgiaManifestPlugin {
    pub crates_dir: PathBuf,
}

impl Default for ForgiaManifestPlugin {
    fn default() -> Self {
        Self {
            crates_dir: PathBuf::from("crates"),
        }
    }
}

impl Plugin for ForgiaManifestPlugin {
    fn build(&self, app: &mut App) {
        let mut index = ManifestIndex::default();
        match scan_workspace(&self.crates_dir) {
            Ok(all) => index.all = all,
            Err(e) => warn!("forgia-genome-core::manifest: scan failed: {}", e),
        }
        app.insert_resource(index);
    }
}
