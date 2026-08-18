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

pub mod cape;
pub mod arme_main;
pub mod avatar_vfx;
pub mod campements;
pub mod capteur;
pub mod cycle;
pub mod eau;
pub mod lampes;
pub mod lighting;
pub mod manifest;
pub mod matiere;
pub mod plugin;
/// S'accroupir et glisser — la glissade remplace le dash en Expédition.
pub mod posture;
/// Le vent : le masque de raideur est cuit par Blender, le mouvement est ici.
pub mod vent;
pub mod visee;

pub use arme_main::{ArmeMainGenome, EtatArmeMain, ExpeditionArmeMainPlugin};
pub use avatar_vfx::{AvatarExpedition, ExpeditionAvatarPlugin};
pub use lampes::{Brasero, LampesConfig};
pub use cycle::{CycleConfig, EtatCycle};
pub use lighting::CycleState;
pub use matiere::{ExpeditionMatierePlugin, MatiereConfig};
pub use capteur::{EtatCarte, Verdict};
pub use manifest::{AbriDef, ExpeditionManifest, ExpeditionManifestError, LampeDef};
pub use plugin::{ActiveExpedition, ExpeditionMarker, ForgiaExpeditionPlugin};
pub use posture::{ExpeditionPosturePlugin, PostureConfig};
pub use cape::{CapeConfig, EtatCape, ExpeditionCapePlugin};
pub use vent::{ExpeditionVentPlugin, VentConfig};
pub use visee::{ExpeditionViseePlugin, Visee, ViseeGenome};
