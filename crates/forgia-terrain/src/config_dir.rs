//! Locate the runtime `config/` directory (hot-reload TOMLs).
//!
//! Certifié zone propre story-349 E1 : walk-up logic pure extraite pour test.

use std::path::{Path, PathBuf};

/// Walk up from `start` until we find a directory that contains `config/biomes/`.
/// Returns the `<ancestor>/config` path, or `None` if no match.
///
/// Pure sur le filesystem : l'appelant fournit `start` + un prédicat `has_biomes`
/// (injecté en test pour ne pas toucher le disque réel).
pub fn find_config_dir_from(start: &Path, has_biomes: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let mut cursor: Option<PathBuf> = Some(start.to_path_buf());
    while let Some(d) = cursor {
        if has_biomes(&d) {
            return Some(d.join("config"));
        }
        cursor = d.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Find the `config/` directory by walking up from the exe path (same logic as AssetPlugin).
/// Uses `config/biomes/` as a specific marker to avoid false positives (e.g. target/debug/config/).
/// Falls back to CWD-relative path if no match found.
pub fn find_config_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if let Some(found) =
                find_config_dir_from(parent, |d| d.join("config").join("biomes").exists())
            {
                return found;
            }
        }
    }
    PathBuf::from("config")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_up_and_finds_matching_ancestor() {
        let marker_dir = PathBuf::from("/fake/root");
        let start = PathBuf::from("/fake/root/target/debug/deps");
        let has_biomes = |d: &Path| d == marker_dir;
        let found = find_config_dir_from(&start, has_biomes);
        assert_eq!(found, Some(PathBuf::from("/fake/root/config")));
    }

    #[test]
    fn returns_none_when_no_ancestor_matches() {
        let start = PathBuf::from("/fake/a/b/c");
        let has_biomes = |_: &Path| false;
        assert_eq!(find_config_dir_from(&start, has_biomes), None);
    }

    #[test]
    fn stops_at_first_match_not_deepest() {
        let start = PathBuf::from("/fake/outer/inner");
        // Both outer and inner would match — must pick the deepest first (start itself).
        let has_biomes =
            |d: &Path| d == Path::new("/fake/outer/inner") || d == Path::new("/fake/outer");
        let found = find_config_dir_from(&start, has_biomes);
        assert_eq!(found, Some(PathBuf::from("/fake/outer/inner/config")));
    }
}
