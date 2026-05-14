//! # forgia-assets
//!
//! GameAssets Resource : tous les handles préchargés au Startup.
//! Ratchet E1/E2 : whitelist `asset_load_whitelist.txt` enforcée par `xtask check-orphans`.
//!
//! Lock L1 V2 baseline objectif : ≤ 50 handles, ≤ 30 call-sites `asset_server.load()`.

use bevy::prelude::*;

pub mod prelude {
    pub use crate::{ForgiaAssetsPlugin, GameAssets};
}

/// GameAssets — handles précharges centralisés.
/// AUCUN `asset_server.load()` hors de ce module sans entry whitelist.
#[derive(Resource, Default)]
pub struct GameAssets {
    // Phase 0 : vide. Phase 2 ajoute viewmodel + 3 armes + impacts + audio.
}

pub struct ForgiaAssetsPlugin;

impl Plugin for ForgiaAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameAssets>()
            .add_systems(Startup, preload_assets);
    }
}

fn preload_assets(mut _assets: ResMut<GameAssets>, _asset_server: Res<AssetServer>) {
    // Phase 0 : pas d'asset à précharger.
    // Phase 2 ajoute les handles ici (viewmodel, 3 armes, impacts, audio combat).
    info!("[forgia-assets] preload_assets — Phase 0 (no assets yet)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_builds() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(AssetPlugin::default());
        app.add_plugins(ForgiaAssetsPlugin);
    }
}
