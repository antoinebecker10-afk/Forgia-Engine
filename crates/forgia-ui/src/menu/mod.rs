//! Hub-menu — découpe mécanique de l'ancien god-file `lib.rs` (story-694, incrément 2).
//!
//! Zéro changement de comportement : chaque module reprend verbatim les items du
//! `lib.rs` d'origine ; `lib.rs` ne garde que le wiring (`ForgiaUiPlugin`) et les
//! re-exports pour les consommateurs internes (`menu_hub_sensor`, `weapon_preview`).

pub(crate) mod chrome;
pub(crate) mod cursor;
pub(crate) mod lobby_gate;
pub(crate) mod nav;
pub(crate) mod pages;
pub(crate) mod registry;
pub(crate) mod shell;
