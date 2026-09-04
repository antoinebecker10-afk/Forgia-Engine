//! asset_handles.rs — Resource préchargée pour CHK-4 critical assets (story-453).
//!
//! Avant story-453, `chk_critical_assets` appelait `asset_server.load(path)` à
//! chaque tick → handles `Handle<Image>` immédiatement droppés → leak de
//! requêtes de chargement + LoadState peu fiable (BUG-452-03 qa-lead).
//!
//! Fix : précharger les handles à `OnEnter(GameMode::Rpg)` dans une Resource
//! dédiée, les conserver vivants tant que le mode RPG est actif. CHK-4 lit
//! les `LoadState` via les `id()` des handles stockés. Aucune allocation
//! par-tick, refcount stable.

use bevy::prelude::*;
use forgia_core::prelude::GameMode;

use crate::config::RpgMonitorConfig;

/// Resource conservant les handles d'assets critiques pendant le mode RPG.
/// Préchargée à `OnEnter(GameMode::Rpg)`, vidée à `OnExit`.
#[derive(Resource, Default)]
pub struct CriticalAssetHandles {
    /// Couples (path, handle). L'ordre suit `config.critical_assets.paths`.
    pub handles: Vec<(String, Handle<Image>)>,
}

/// System `OnEnter(GameMode::Rpg)` : précharge les assets critiques listés
/// dans `config.critical_assets.paths`. Handles stockés vivants → load_state
/// reflète l'état réel sans déclencher de re-chargement.
pub fn sys_preload_critical_assets(
    asset_server: Res<AssetServer>,
    config: Res<RpgMonitorConfig>,
    mut handles: ResMut<CriticalAssetHandles>,
) {
    handles.handles.clear();
    if !config.critical_assets.enabled {
        return;
    }
    for path in &config.critical_assets.paths {
        let h: Handle<Image> = asset_server.load(path);
        handles.handles.push((path.clone(), h));
    }
    info!(
        "[rpg-monitor] Preloaded {} critical asset handle(s)",
        handles.handles.len()
    );
}

/// System `OnExit(GameMode::Rpg)` : libère les handles pour permettre l'unload
/// des textures par Bevy si nécessaire.
pub fn sys_release_critical_assets(mut handles: ResMut<CriticalAssetHandles>) {
    handles.handles.clear();
}

/// Plugin appendix : enregistre la Resource + systems on_enter/on_exit Rpg.
pub(crate) fn register(app: &mut App) {
    app.init_resource::<CriticalAssetHandles>()
        .add_systems(OnEnter(GameMode::Rpg), sys_preload_critical_assets)
        .add_systems(OnExit(GameMode::Rpg), sys_release_critical_assets);
}
