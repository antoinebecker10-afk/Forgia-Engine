//! # pack_registry
//!
//! Bevy integration layer over `forgia-assets-bundle` (manifest + lockfile)
//! and `forgia-asset-cdn` (fetch/verify/install).
//!
//! Charge `assets/packs.toml` au Startup, reconcile vs disque, expose une
//! Resource `PackRegistry` queryable, emit sensor JSON `forgia_pack_registry.json`
//! et health side-file selon convention QUALITY_GATE.
//!
//! ## Pattern Unity Addressables + UE Asset Registry
//!
//! - Async metadata-only discovery au boot (jamais charger les bytes pour
//!   savoir ce qui existe — UE Asset Registry pattern).
//! - Sensor `forgia_pack_registry.json` = "catalog.hash sidecar" cheap-polled
//!   par Claude (convention V2 + observability-required.md).
//! - Health side-file = next_step Quality Gate (story-384) avec commande
//!   exacte à lancer pour fix.

use bevy::prelude::*;
use forgia_assets_bundle::{PackEntry, PackManifest};
use std::path::{Path, PathBuf};

const MANIFEST_PATH: &str = "assets/packs.toml";
const SENSOR_PATH: &str = "forgia_pack_registry.json";
const HEALTH_PATH: &str = "forgia_pack_registry_health.json";
const SENSOR_INTERVAL_S: f32 = 1.0;

// ── Resources ───────────────────────────────────────────────────────────────

/// État runtime du pack registry. Initialisé au Startup, queryable par les
/// systèmes qui ont besoin de savoir si tel pack est dispo (ex: village_loader
/// qui dépend de kaykit-medieval, ai/skeletons sur kaykit-skeletons).
#[derive(Resource, Debug, Default)]
pub struct PackRegistry {
    pub packs: Vec<PackRuntimeEntry>,
    pub manifest_loaded: bool,
    pub manifest_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackRuntimeEntry {
    pub name: String,
    pub version: String,
    pub install_dir: String,
    pub install_path_abs: PathBuf,
    pub kit_type: String,
    pub license: String,
    pub source_page: String,
    pub installed: bool,
}

impl PackRegistry {
    /// Cherche un pack par nom.
    pub fn find(&self, name: &str) -> Option<&PackRuntimeEntry> {
        self.packs.iter().find(|p| p.name == name)
    }

    /// True si le pack est déclaré ET son install_dir existe.
    pub fn is_installed(&self, name: &str) -> bool {
        self.find(name).is_some_and(|p| p.installed)
    }

    /// Itère uniquement les packs installés (consommateur typique).
    pub fn installed(&self) -> impl Iterator<Item = &PackRuntimeEntry> {
        self.packs.iter().filter(|p| p.installed)
    }

    /// Itère les packs manquants (déclarés mais install_dir absent).
    pub fn missing(&self) -> impl Iterator<Item = &PackRuntimeEntry> {
        self.packs.iter().filter(|p| !p.installed)
    }

    pub fn count(&self) -> (usize, usize, usize) {
        let total = self.packs.len();
        let installed = self.installed().count();
        let missing = total - installed;
        (total, installed, missing)
    }
}

#[derive(Resource, Default)]
pub(crate) struct SensorTimer {
    accum: f32,
}

// ── Systems ─────────────────────────────────────────────────────────────────

/// Startup : charge packs.toml et fait le reconcile disque.
pub(crate) fn load_pack_registry(mut registry: ResMut<PackRegistry>) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match PackManifest::load_from_path(MANIFEST_PATH) {
        Ok(manifest) => {
            registry.packs = manifest
                .packs
                .iter()
                .map(|p| build_runtime_entry(&manifest, p, &cwd))
                .collect();
            registry.manifest_loaded = true;
            registry.manifest_error = None;
            let (total, installed, missing) = registry.count();
            info!(
                "[forgia-asset-registry::packs] loaded {} ({} installed, {} missing)",
                total, installed, missing
            );
        }
        Err(e) => {
            registry.manifest_loaded = false;
            registry.manifest_error = Some(e.to_string());
            // Pas critique si pas de manifest : V1 ship sans packs externes possible.
            warn!(
                "[forgia-asset-registry::packs] no manifest at {} ({}), pack-aware features disabled",
                MANIFEST_PATH, e
            );
        }
    }
}

fn build_runtime_entry(
    manifest: &PackManifest,
    pack: &PackEntry,
    workspace_root: &Path,
) -> PackRuntimeEntry {
    let install_path_abs = manifest.install_path(pack, workspace_root);
    let installed = install_path_abs.is_dir();
    PackRuntimeEntry {
        name: pack.name.clone(),
        version: pack.version.clone(),
        install_dir: pack.install_dir.clone(),
        install_path_abs,
        kit_type: pack.kit_type.clone(),
        license: pack.license.clone(),
        source_page: pack.source_page.clone(),
        installed,
    }
}

/// Update 1Hz : écrit `forgia_pack_registry.json` (sensor) + health side-file.
/// Convention V2 observability : sensor toujours, health side-file uniquement
/// si severity != "ok" (Claude lit fichier absent = OK).
pub(crate) fn write_pack_registry_sensor(
    time: Res<Time>,
    mut timer: ResMut<SensorTimer>,
    registry: Res<PackRegistry>,
) {
    timer.accum += time.delta_secs();
    if timer.accum < SENSOR_INTERVAL_S {
        return;
    }
    timer.accum = 0.0;

    let (total, installed, missing) = registry.count();
    let now = time.elapsed_secs();
    let severity = severity_for(&registry);

    let mut packs_json = String::new();
    for (i, p) in registry.packs.iter().enumerate() {
        if i > 0 {
            packs_json.push_str(", ");
        }
        packs_json.push_str(&format!(
            r#"{{ "name": "{}", "version": "{}", "kit_type": "{}", "installed": {} }}"#,
            p.name, p.version, p.kit_type, p.installed
        ));
    }

    let json = format!(
        r#"{{
  "timestamp_secs": {:.1},
  "schema_version": 1,
  "manifest_loaded": {},
  "manifest_error": {},
  "total": {},
  "installed": {},
  "missing": {},
  "packs": [{}],
  "severity": "{}"
}}"#,
        now,
        registry.manifest_loaded,
        registry
            .manifest_error
            .as_deref()
            .map(|e| format!(r#""{}""#, e.replace('"', r#"\""#)))
            .unwrap_or_else(|| "null".into()),
        total,
        installed,
        missing,
        packs_json,
        severity,
    );
    if let Err(e) = forgia_core::sensor_io::enqueue(SENSOR_PATH, json) {
        warn!("[forgia-asset-registry::packs] sensor write failed: {e}");
    }

    if severity == "ok" {
        let _ = std::fs::remove_file(HEALTH_PATH);
    } else {
        let missing_names: Vec<String> = registry.missing().map(|p| p.name.clone()).collect();
        let next_step = if registry.manifest_loaded {
            format!(
                "Run `cargo run -p forgia-asset-cdn --bin forgia-pack-cdn install` to install {} missing pack(s): {}",
                missing_names.len(),
                missing_names.join(", ")
            )
        } else {
            "Create assets/packs.toml — see story-449 for schema, or run an existing template"
                .to_string()
        };
        let health = format!(
            r#"{{
  "timestamp_secs": {:.1},
  "severity": "{}",
  "message": "Pack registry: {}/{} packs missing or manifest not loaded",
  "next_step": "{}",
  "read_more": "Read forgia_pack_registry.json"
}}"#,
            now,
            severity,
            missing,
            total,
            next_step.replace('"', r#"\""#),
        );
        if let Err(e) = std::fs::write(HEALTH_PATH, health) {
            warn!("[forgia-asset-registry::packs] health write failed: {e}");
        }
    }
}

fn severity_for(registry: &PackRegistry) -> &'static str {
    if !registry.manifest_loaded {
        // Pas critique : peut être un dev sans pack externes du tout.
        return "info";
    }
    let (_, _, missing) = registry.count();
    match missing {
        0 => "ok",
        _ => "warning",
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────────

pub struct ForgiaPackRegistryPlugin;

impl Plugin for ForgiaPackRegistryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PackRegistry>()
            .init_resource::<SensorTimer>()
            .add_systems(Startup, load_pack_registry)
            .add_systems(Update, write_pack_registry_sensor);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_assets_bundle::{ManifestMeta, PackEntry};

    fn fixture_manifest() -> PackManifest {
        PackManifest {
            meta: ManifestMeta {
                schema_version: 1,
                default_install_root: "assets/models-v1/packs".into(),
            },
            packs: vec![PackEntry {
                name: "kaykit-medieval".into(),
                version: "1.0".into(),
                source_page: "https://example.com".into(),
                download_url: "https://example.com/m.zip".into(),
                sha256: None,
                size_bytes: None,
                install_dir: "kaykit-medieval".into(),
                license: "CC0-1.0".into(),
                kit_type: "medieval".into(),
                strip_prefix: String::new(),
            }],
        }
    }

    #[test]
    fn build_runtime_entry_marks_missing() {
        let manifest = fixture_manifest();
        let entry = build_runtime_entry(
            &manifest,
            &manifest.packs[0],
            Path::new("/nonexistent_workspace"),
        );
        assert!(!entry.installed);
        assert_eq!(entry.name, "kaykit-medieval");
    }

    #[test]
    fn registry_count_and_iter() {
        let mut r = PackRegistry::default();
        r.packs.push(PackRuntimeEntry {
            name: "a".into(),
            version: "1".into(),
            install_dir: "a".into(),
            install_path_abs: PathBuf::from("/no"),
            kit_type: "x".into(),
            license: "CC0-1.0".into(),
            source_page: "https://example.com".into(),
            installed: true,
        });
        r.packs.push(PackRuntimeEntry {
            name: "b".into(),
            version: "1".into(),
            install_dir: "b".into(),
            install_path_abs: PathBuf::from("/no"),
            kit_type: "x".into(),
            license: "CC0-1.0".into(),
            source_page: "https://example.com".into(),
            installed: false,
        });
        let (total, installed, missing) = r.count();
        assert_eq!((total, installed, missing), (2, 1, 1));
        assert!(r.is_installed("a"));
        assert!(!r.is_installed("b"));
        assert!(!r.is_installed("c"));
        assert_eq!(r.installed().count(), 1);
        assert_eq!(r.missing().count(), 1);
    }

    #[test]
    fn severity_ok_when_all_installed() {
        let mut r = PackRegistry {
            manifest_loaded: true,
            ..Default::default()
        };
        r.packs.push(PackRuntimeEntry {
            installed: true,
            name: "x".into(),
            version: "1".into(),
            install_dir: "x".into(),
            install_path_abs: PathBuf::new(),
            kit_type: "x".into(),
            license: "CC0-1.0".into(),
            source_page: "https://example.com".into(),
        });
        assert_eq!(severity_for(&r), "ok");
    }

    #[test]
    fn severity_warning_when_missing() {
        let mut r = PackRegistry {
            manifest_loaded: true,
            ..Default::default()
        };
        r.packs.push(PackRuntimeEntry {
            installed: false,
            name: "x".into(),
            version: "1".into(),
            install_dir: "x".into(),
            install_path_abs: PathBuf::new(),
            kit_type: "x".into(),
            license: "CC0-1.0".into(),
            source_page: "https://example.com".into(),
        });
        assert_eq!(severity_for(&r), "warning");
    }

    #[test]
    fn severity_info_when_no_manifest() {
        let r = PackRegistry::default();
        assert_eq!(severity_for(&r), "info");
    }
}
