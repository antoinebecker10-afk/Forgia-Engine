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
use web_time::{SystemTime, UNIX_EPOCH};

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
/// avant la relocalisation. Un TOML corrompu est préservé sous un nom
/// `.corrupt-<timestamp>` avant le fallback : il ne doit jamais provoquer une
/// perte de progression silencieuse. Le prochain `save()` réécrira dans
/// `save_dir()`, rendant l'ancien fichier inerte (source de vérité unique dès
/// la première sauvegarde réussie).
pub(crate) fn load_toml_migrating<T: DeserializeOwned + Default>(file_name: &str) -> T {
    for path in [
        save_dir().join(file_name),
        legacy_config_dir().join(file_name),
    ] {
        let Ok(contents) = std::fs::read_to_string(&path) else {
            continue;
        };
        match toml::from_str(&contents) {
            Ok(value) => return value,
            Err(error) => {
                let backup = corrupt_backup_path(&path, now_unix_secs());
                match std::fs::rename(&path, &backup) {
                    Ok(()) => warn!(
                        "[save] invalid TOML in {}: {error}; preserved as {}",
                        path.display(),
                        backup.display()
                    ),
                    Err(rename_error) => warn!(
                        "[save] invalid TOML in {}: {error}; failed to preserve backup: {rename_error}",
                        path.display()
                    ),
                }
            }
        }
    }
    T::default()
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn corrupt_backup_path(path: &Path, timestamp_unix_secs: u64) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("save.toml");
    path.with_file_name(format!("{file_name}.corrupt-{timestamp_unix_secs}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_backup_keeps_the_original_file_name() {
        let path = Path::new("C:/saves/meta.toml");
        assert_eq!(
            corrupt_backup_path(path, 1_780_000_000),
            PathBuf::from("C:/saves/meta.toml.corrupt-1780000000")
        );
    }
}
