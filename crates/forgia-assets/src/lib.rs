//! # forgia-assets
//!
//! GameAssets Resource : tous les handles préchargés au Startup.
//! Ratchet E1/E2 : whitelist `asset_load_whitelist.txt` enforcée par `xtask check-orphans`.
//!
//! Lock L1 V2 baseline objectif : ≤ 50 handles, ≤ 30 call-sites `asset_server.load()`.

use bevy::asset::AssetPath;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;

pub mod prelude {
    pub use crate::{AssetPreloadSet, ForgiaAssetsPlugin, GameAssets};
}

/// GameAssets — handles précharges centralisés.
/// AUCUN `asset_server.load()` hors de ce module sans entry whitelist.
#[derive(Resource, Default)]
pub struct GameAssets {
    /// Bras GLB préchargés au boot pour ne pas charger au premier spawn FPS.
    pub viewmodel_arm_right: Handle<Scene>,
    pub viewmodel_arm_left: Handle<Scene>,
    /// Squelettes, sols et portails rendus au Lobby afin de préchauffer le PBR.
    pub warmup_scenes: Vec<Handle<Scene>>,
    /// Cubemap cartoon empilé, lu CPU-side lors de la génération du skybox.
    pub sky_overlay: Handle<Image>,
    /// Les quatre scènes utilisées par l'aperçu RTT du hub, dans l'ordre des
    /// slots d'arme (Pépin, Bourrasque, Madame Lenoir, Boucherie).
    pub weapon_preview_scenes: [Handle<Scene>; 4],
}

/// Les consommateurs de [`GameAssets`] qui doivent lire un handle préchargé au
/// boot s'ordonnent après ce set : aucun `Handle::default()` ne fuit dans une
/// ressource métier.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetPreloadSet;

pub struct ForgiaAssetsPlugin;

impl Plugin for ForgiaAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameAssets>()
            .add_systems(Startup, preload_assets.in_set(AssetPreloadSet));
    }
}

fn preload_assets(mut assets: ResMut<GameAssets>, asset_server: Res<AssetServer>) {
    let load_scene = |path: &'static str| {
        asset_server.load(GltfAssetLabel::Scene(0).from_asset(AssetPath::parse(path)))
    };
    assets.viewmodel_arm_right = load_scene("models/arms/fps_arm_R.glb");
    assets.viewmodel_arm_left = load_scene("models/arms/fps_arm_L.glb");
    assets.sky_overlay = asset_server.load("hdri/sky_129_stacked.png");
    assets.weapon_preview_scenes = [
        "models/weapons/forgia/pepin.glb",
        "models/weapons/forgia/bourrasque.glb",
        "models/weapons/forgia/madame_lenoir.glb",
        "models/weapons/forgia/boucherie.glb",
    ]
    .map(load_scene);
    assets.warmup_scenes = [
        "models/kaykit/skeletons/Skeleton_Warrior.glb",
        "models/kaykit/skeletons/Skeleton_Minion.glb",
        "models/kaykit/skeletons/Skeleton_Mage.glb",
        "models/kaykit/dungeon/floor.glb",
        "models/kaykit/dungeon/floor_dirt.glb",
        "models/kaykit/dungeon/floor_rocks.glb",
        "models/environment/portal/portal_closed.glb",
        "models/environment/portal/portal_open.glb",
    ]
    .into_iter()
    .map(load_scene)
    .collect();
    info!(
        "[forgia-assets] preloaded {} warmup scenes + viewmodel arms + sky + weapon previews",
        assets.warmup_scenes.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_builds() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default());
        app.add_plugins(ForgiaAssetsPlugin);
    }
}
