//! forgia-mode-expedition — jouer une carte autorée sous Blender.
//!
//! Première carte : **« Le Vallon »**, 280 × 200 m, 48 cellules glTF, 943
//! colliders, 3 campements-verrous sur un chemin de 358,7 m.
//!
//! # Deux fichiers, deux rôles, deux repères
//!
//! | fichier | contenu | repère |
//! |---|---|---|
//! | `vallon_stream_cells.toml` | le décor, découpé en cellules | `bevy_y_up_meters` |
//! | `expedition_vallon.json` | le gameplay | `blender_z_up` ⚠️ |
//!
//! Le premier est lu par [`forgia_streaming::cells`], partagé avec le Château.
//! Le second par [`manifest`], qui porte **la seule conversion de repère du
//! projet** — cf. sa documentation pour la raison.

pub mod manifest;
pub mod plugin;

pub use manifest::{ExpeditionManifest, ExpeditionManifestError};
pub use plugin::{ActiveExpedition, ExpeditionMarker, ForgiaExpeditionPlugin};
