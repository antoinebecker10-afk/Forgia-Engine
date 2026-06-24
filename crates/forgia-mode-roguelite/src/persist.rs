//! Écriture disque ATOMIQUE pour les saves roguelite.
//!
//! `std::fs::write` direct = corruption si le process crash en plein milieu
//! (fichier tronqué ou vide). On écrit dans un fichier temp voisin puis on
//! `rename` par-dessus la cible : sur la plupart des OS le rename est atomique,
//! donc un crash laisse l'ancien save intact au lieu d'un fichier à moitié écrit.

use bevy::log::warn;
use serde::Serialize;
use std::path::Path;

/// Sérialise `value` en TOML pretty puis l'écrit atomiquement dans `path`
/// (write `<path>.tmp` → `rename` over `path`). `tag` ne sert qu'au libellé des
/// warns. Échec silencieux (warn) — un save raté ne doit jamais crasher le jeu.
pub(crate) fn save_toml_atomic<T: Serialize>(path: &Path, value: &T, tag: &str) {
    let s = match toml::to_string_pretty(value) {
        Ok(s) => s,
        Err(e) => {
            warn!("[{tag}] serialize failed: {e}");
            return;
        }
    };
    let tmp = path.with_extension("tmp");
    if let Err(e) = std::fs::write(&tmp, &s) {
        warn!("[{tag}] save failed (write tmp {}): {e}", tmp.display());
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, path) {
        warn!(
            "[{tag}] save failed (rename {} -> {}): {e}",
            tmp.display(),
            path.display()
        );
        let _ = std::fs::remove_file(&tmp); // pas de tmp orphelin qui traîne
    }
}
