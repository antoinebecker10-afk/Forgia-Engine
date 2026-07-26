//! forgia-genome-core — Genome TOML loading + hot-reload primitives.
//!
//! Forgia is **data-driven** : balance, biome params, weapon stats live in TOML
//! genomes, not Rust constants. This crate provides the core types and asset
//! loader infrastructure.
//!
//! Genome categories live under `assets/genomes/<category>/<name>.toml`.
//! Each consumer crate (forgia-fps, forgia-terrain, etc.) declares
//! its own typed genome struct + uses `Genome::<MyGenome>` asset handle.

use bevy::asset::{io::Reader, Asset, AssetLoader, LoadContext};
use bevy::prelude::*;
use serde::de::DeserializeOwned;
use std::marker::PhantomData;

pub mod manifest;

/// Generic typed genome asset. `T` is the user-defined Serde struct.
#[derive(Asset, TypePath)]
pub struct Genome<T: Send + Sync + 'static + TypePath> {
    pub data: T,
}

/// Asset loader for `.toml` genomes typed as `Genome<T>`.
#[derive(TypePath)]
pub struct GenomeLoader<T: Send + Sync + 'static + TypePath + DeserializeOwned> {
    _marker: PhantomData<T>,
}

impl<T: Send + Sync + 'static + TypePath + DeserializeOwned> Default for GenomeLoader<T> {
    fn default() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GenomeLoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),
}

impl<T> AssetLoader for GenomeLoader<T>
where
    T: Send + Sync + 'static + TypePath + DeserializeOwned,
{
    type Asset = Genome<T>;
    type Settings = ();
    type Error = GenomeLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut buf = String::new();
        use bevy::asset::AsyncReadExt;
        reader.read_to_string(&mut buf).await?;
        let data: T = parse_genome(&buf)?;
        Ok(Genome { data })
    }

    fn extensions(&self) -> &[&str] {
        &["toml"]
    }
}

/// Parse pur d'un genome TOML — extrait du loader async pour être testable
/// headless (story-594 M2-B7 : le socle data-driven n'avait AUCUN test).
///
/// Contrat : un TOML invalide retourne `Err` (le chargement asset échoue, la
/// Resource consommatrice garde son `Default`) — il ne panique JAMAIS le jeu.
pub fn parse_genome<T: DeserializeOwned>(toml_src: &str) -> Result<T, toml::de::Error> {
    toml::from_str(toml_src)
}

/// Helper trait that consumer crates implement to register a typed genome.
pub trait RegisterGenome {
    fn register_genome<T>(&mut self) -> &mut Self
    where
        T: Send + Sync + 'static + TypePath + DeserializeOwned + bevy::reflect::FromReflect;
}

impl RegisterGenome for App {
    fn register_genome<T>(&mut self) -> &mut Self
    where
        T: Send + Sync + 'static + TypePath + DeserializeOwned + bevy::reflect::FromReflect,
    {
        self.init_asset::<Genome<T>>()
            .register_asset_loader(GenomeLoader::<T>::default());
        self
    }
}

pub struct ForgiaGenomeCorePlugin;

impl Plugin for ForgiaGenomeCorePlugin {
    fn build(&self, _app: &mut App) {
        // No-op : per-type registration is done by consumer crates via
        // `app.register_genome::<MyGenome>()`. This plugin is a marker for
        // documentation purposes.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, TypePath, Debug, PartialEq)]
    #[serde(default)]
    struct TestTuning {
        speed: f32,
        name: String,
    }

    impl Default for TestTuning {
        fn default() -> Self {
            Self {
                speed: 5.0,
                name: "default".to_string(),
            }
        }
    }

    #[derive(Deserialize, TypePath, Debug)]
    struct StrictTuning {
        #[allow(dead_code)]
        required_field: f32,
    }

    #[test]
    fn parse_valid_toml() {
        let t: TestTuning = parse_genome("speed = 9.5\nname = \"pepin\"").expect("TOML valide");
        assert_eq!(t.speed, 9.5);
        assert_eq!(t.name, "pepin");
    }

    #[test]
    fn parse_invalid_toml_is_err_not_panic() {
        // Le contrat anti-crash : syntaxe cassée → Err propre, jamais de panic.
        let r: Result<TestTuning, _> = parse_genome("speed = = broken [[[");
        assert!(r.is_err());
    }

    #[test]
    fn parse_missing_fields_use_serde_defaults() {
        // Forward-compat : un TOML partiel (vieux fichier) garde les défauts.
        let t: TestTuning = parse_genome("speed = 7.0").expect("TOML partiel valide");
        assert_eq!(t.speed, 7.0);
        assert_eq!(t.name, "default");
    }

    #[test]
    fn parse_missing_required_field_is_err() {
        // Sans #[serde(default)], un champ manquant doit échouer explicitement
        // (pas de valeur fantôme silencieuse).
        let r: Result<StrictTuning, _> = parse_genome("other = 1.0");
        assert!(r.is_err());
    }

    #[test]
    fn parse_wrong_type_is_err() {
        let r: Result<TestTuning, _> = parse_genome("speed = \"pas un nombre\"");
        assert!(r.is_err());
    }

    #[test]
    fn loader_accepts_toml_extension_only() {
        let loader = GenomeLoader::<TestTuning>::default();
        assert_eq!(loader.extensions(), &["toml"]);
    }
}
