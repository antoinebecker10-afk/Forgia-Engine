//! Immutable PCG registry lockfiles.
//!
//! A seed only reproduces a world when every kit/grammar/pattern resolved by
//! that generation is fixed too. This module validates the lockfile shape and
//! dependency graph; fetching and hashing the referenced files belongs to the
//! offline publisher, not the runtime solver.

use crate::{PcgError, StableId};
use serde::{Deserialize, Serialize};

pub const REGISTRY_LOCK_SCHEMA_V1: &str = "forgia.pcg-registry-lock/v1";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("registry lock TOML invalid: {0}")]
    Parse(String),
    #[error("unsupported registry lock schema `{found}`; expected `{REGISTRY_LOCK_SCHEMA_V1}`")]
    UnsupportedSchema { found: String },
    #[error("invalid registry id: {0}")]
    InvalidId(#[from] PcgError),
    #[error("registry entry `{0}` is duplicated")]
    DuplicateEntry(String),
    #[error("registry entry `{id}` has an invalid SHA-256 content hash")]
    InvalidHash { id: String },
    #[error("registry entry `{id}` depends on missing locked entry `{dependency}`")]
    MissingDependency { id: String, dependency: String },
    #[error("registry dependency cycle: {0}")]
    DependencyCycle(String),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PcgRegistryLock {
    pub schema_version: String,
    #[serde(default)]
    pub entries: Vec<RegistryLockEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RegistryLockEntry {
    pub id: String,
    pub kind: RegistryEntryKind,
    pub manifest: String,
    pub content_hash: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RegistryEntryKind {
    Kit,
    Grammar,
    Pattern,
}

impl PcgRegistryLock {
    pub fn parse_toml(source: &str) -> Result<Self, RegistryError> {
        let lock: Self =
            toml::from_str(source).map_err(|error| RegistryError::Parse(error.to_string()))?;
        lock.validate()?;
        Ok(lock)
    }

    pub fn validate(&self) -> Result<(), RegistryError> {
        if self.schema_version != REGISTRY_LOCK_SCHEMA_V1 {
            return Err(RegistryError::UnsupportedSchema {
                found: self.schema_version.clone(),
            });
        }
        let mut ids = self
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        if let Some(duplicate) = ids
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then_some(pair[0]))
        {
            return Err(RegistryError::DuplicateEntry(duplicate.to_string()));
        }
        for entry in &self.entries {
            StableId::parse(entry.id.clone())?;
            if !is_sha256_hash(&entry.content_hash) {
                return Err(RegistryError::InvalidHash {
                    id: entry.id.clone(),
                });
            }
            for dependency in &entry.dependencies {
                StableId::parse(dependency.clone())?;
                if !self
                    .entries
                    .iter()
                    .any(|candidate| candidate.id == *dependency)
                {
                    return Err(RegistryError::MissingDependency {
                        id: entry.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
            }
        }
        self.validate_acyclic()
    }

    fn validate_acyclic(&self) -> Result<(), RegistryError> {
        let mut visiting = Vec::new();
        let mut visited = Vec::new();
        let mut entries = self.entries.iter().collect::<Vec<_>>();
        entries.sort_by(|left, right| left.id.cmp(&right.id));
        for entry in entries {
            self.visit(entry, &mut visiting, &mut visited)?;
        }
        Ok(())
    }

    fn visit(
        &self,
        entry: &RegistryLockEntry,
        visiting: &mut Vec<String>,
        visited: &mut Vec<String>,
    ) -> Result<(), RegistryError> {
        if visited.iter().any(|id| id == &entry.id) {
            return Ok(());
        }
        if let Some(start) = visiting.iter().position(|id| id == &entry.id) {
            let mut cycle = visiting[start..].to_vec();
            cycle.push(entry.id.clone());
            return Err(RegistryError::DependencyCycle(cycle.join(" -> ")));
        }
        visiting.push(entry.id.clone());
        let mut dependencies = entry.dependencies.clone();
        dependencies.sort();
        for id in dependencies {
            let dependency = self
                .entries
                .iter()
                .find(|candidate| candidate.id == id)
                .expect("dependencies were checked before cycle traversal");
            self.visit(dependency, visiting, visited)?;
        }
        visiting.pop();
        visited.push(entry.id.clone());
        Ok(())
    }
}

fn is_sha256_hash(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH_A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    #[test]
    fn parses_a_resolved_lockfile() {
        let source = format!(
            r#"
schema_version = "forgia.pcg-registry-lock/v1"
[[entries]]
id = "forgia.castle.stone@1.0.0"
kind = "kit"
manifest = "kits/forgia/castle/stone/1.0.0/kit.toml"
content_hash = "{HASH_A}"
[[entries]]
id = "forgia.castle.hall_pattern@1.0.0"
kind = "pattern"
manifest = "patterns/forgia/castle/hall/1.0.0/pattern.toml"
content_hash = "{HASH_B}"
dependencies = ["forgia.castle.stone@1.0.0"]
"#
        );
        let lock = PcgRegistryLock::parse_toml(&source).expect("valid lockfile");
        assert_eq!(lock.entries.len(), 2);
    }

    #[test]
    fn rejects_dependency_cycles() {
        let source = format!(
            r#"
schema_version = "forgia.pcg-registry-lock/v1"
[[entries]]
id = "forgia.a@1.0.0"
kind = "kit"
manifest = "a.toml"
content_hash = "{HASH_A}"
dependencies = ["forgia.b@1.0.0"]
[[entries]]
id = "forgia.b@1.0.0"
kind = "grammar"
manifest = "b.toml"
content_hash = "{HASH_B}"
dependencies = ["forgia.a@1.0.0"]
"#
        );
        assert_eq!(
            PcgRegistryLock::parse_toml(&source),
            Err(RegistryError::DependencyCycle(
                "forgia.a@1.0.0 -> forgia.b@1.0.0 -> forgia.a@1.0.0".into()
            ))
        );
    }
}
