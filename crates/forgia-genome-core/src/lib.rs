//! forgia-genome-core — Genome TOML loading + hot-reload primitives.
//!
//! Forgia is **data-driven** : balance, biome params, weapon stats live in TOML
//! genomes, not Rust constants. This crate provides the core types and asset
//! loader infrastructure.
//!
//! Genome categories live under `assets/genomes/<category>/<name>.toml`.
//! Each consumer crate (forgia-weapon-hitscan, forgia-terrain, etc.) declares
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
        let data: T = toml::from_str(&buf)?;
        Ok(Genome { data })
    }

    fn extensions(&self) -> &[&str] {
        &["toml"]
    }
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
