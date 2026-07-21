//! Écriture disque ATOMIQUE pour les saves roguelite.
//!
//! `std::fs::write` direct = corruption si le process crash en plein milieu
//! (fichier tronqué ou vide). On écrit dans un fichier temp voisin puis on
//! `rename` par-dessus la cible : sur la plupart des OS le rename est atomique,
//! donc un crash laisse l'ancien save intact au lieu d'un fichier à moitié écrit.

use bevy::log::warn;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Répertoire de sauvegarde STABLE, découplé du dossier d'installation :
/// `%APPDATA%\Forgia\` sur Windows (créé si absent). Objectif distribution
/// joueur : une mise à jour du build (remplacement du dossier de jeu, re-dézip,
/// patch itch/butler) ne peut PLUS effacer la progression — les saves ne vivent
/// pas dans le dossier remplacé. Fallback : ancien emplacement `config/`
/// (walk-up depuis l'exe) si `APPDATA` est absent (dev sous shell exotique /
/// non-Windows).
pub(crate) fn save_dir() -> PathBuf {
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let dir = PathBuf::from(appdata).join("Forgia");
        if std::fs::create_dir_all(&dir).is_ok() {
            return dir;
        }
    }
    legacy_config_dir()
}

/// Ancien emplacement des saves : `config/` trouvé en remontant depuis l'exe
/// (marqueur `config/biomes/`), fallback CWD `config/`. Conservé pour (1) la
/// migration one-shot des saves pré-`%APPDATA%` (cf `load_toml_migrating`) et
/// (2) le fallback si `APPDATA` manque.
pub(crate) fn legacy_config_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let mut cursor: Option<&Path> = exe.parent();
        while let Some(d) = cursor {
            if d.join("config").join("biomes").exists() {
                return d.join("config");
            }
            cursor = d.parent();
        }
    }
    PathBuf::from("config")
}

/// Charge un save TOML avec MIGRATION transparente : lit d'abord le nouvel
/// emplacement (`save_dir()/file_name`) ; si absent, retombe sur l'ancien
/// (`legacy_config_dir()/file_name`) pour ne pas perdre une progression écrite
/// avant la relocalisation. Toute erreur (fichier absent, TOML corrompu) →
/// `Default`. Le prochain `save()` réécrira dans `save_dir()`, rendant l'ancien
/// fichier inerte (source de vérité unique dès la 1re sauvegarde).
pub(crate) fn load_toml_migrating<T: DeserializeOwned + Default>(file_name: &str) -> T {
    if let Ok(c) = std::fs::read_to_string(save_dir().join(file_name)) {
        return toml::from_str(&c).unwrap_or_default();
    }
    if let Ok(c) = std::fs::read_to_string(legacy_config_dir().join(file_name)) {
        if let Ok(v) = toml::from_str::<T>(&c) {
            return v;
        }
    }
    T::default()
}

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
