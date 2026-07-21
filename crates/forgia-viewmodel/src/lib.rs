//! # forgia-viewmodel
//!
//! FPS arms viewmodel (1P weapon render) — équivalent `CBaseViewModel` du Source SDK
//! ([`developer.valvesoftware.com/wiki/Viewmodel`](https://developer.valvesoftware.com/wiki/Viewmodel)),
//! découplé du firing/damage qui restent dans `forgia-fps` (orchestrator) et
//! `forgia-combat` (équivalent `CBaseCombatWeapon`).
//!
//! Architecture mirror Source SDK + Bevy official `first-person-view-model` example
//! (`bevy.org/examples/camera/first-person-view-model/`) :
//!
//! | Module | Responsabilité | Équivalent Source |
//! |---|---|---|
//! | [`genome`] | Data layer TOML hot-reloadable (`viewmodel_arena.toml`) | `weapon_script.txt` |
//! | [`calibration`] | Helpers purs : `viewmodel_transform`, `viewmodel_rotation_*` | — (helpers internes) |
//! | [`attach`] | Spawn/swap/auto-scale viewmodel enfant de `FpsCamera` | `CBaseViewModel::SetWeaponModel` |
//! | [`pose`] | ADS state, lerp FOV/transform/rotation/speed/sensitivity | `cl_ads*` family |
//! | [`fade`] | Semi-transparence ADS via `forgia-mesh-fader` | (custom Forgia) |
//!
//! Le firing path (`fire_weapon_minimal`) reste dans `forgia-fps` parce qu'il
//! orchestre une douzaine de SystemParams cross-crate (juice, VFX, ammo, hitscan,
//! camera, rapier) — pattern Source SDK respecté : viewmodel = render client-only,
//! firing = gameplay autoritaire.
//!
//! ## Usage
//!
//! ```ignore
//! use forgia_viewmodel::ForgiaViewmodelPlugin;
//! app.add_plugins(ForgiaViewmodelPlugin);
//! ```
//!
//! `ForgiaViewmodelPlugin` ajoute les 3 sous-plugins (attach + pose + fade) et
//! check idempotent `MeshFaderPlugin` (pour éviter double-add si déjà ajouté
//! ailleurs dans le projet).

use bevy::prelude::*;

pub mod arms;
pub mod arms_sensor;
pub mod attach;
pub mod calibration;
pub mod vm_camera;
pub mod calibration_sensor;
pub mod fade;
pub mod genome;
pub mod pose;

// Re-exports clés pour faciliter `use forgia_viewmodel::{...}`.
pub use arms::{ForgiaViewmodelArmsPlugin, ViewmodelArms, ViewmodelArmsTuning};
pub use vm_camera::{ForgiaViewmodelCameraPlugin, ViewmodelFovTuning, VIEWMODEL_LAYER};
pub use attach::{
    attach_viewmodel_to_camera, auto_scale_viewmodel, despawn_viewmodel,
    ensure_camera_shake_component, load_weapon_models, update_viewmodel_on_switch,
    ForgiaViewmodelAttachPlugin, NeedsAutoScale, ViewmodelBaseScale, WeaponModelAssets,
    WeaponViewmodel,
};
pub use calibration::{
    viewmodel_fallback_scale, viewmodel_rotation_ads, viewmodel_rotation_hipfire,
    viewmodel_target_size, viewmodel_transform,
};
pub use fade::{ForgiaViewmodelFadePlugin, ScopeGlassFader, ScopeGlassScanned, ViewmodelBodyFader};
pub use genome::{
    load_viewmodel_genome, lookup_genome_entry, weapon_genome_key, ViewmodelGenome,
    ViewmodelGenomeCtx, ViewmodelGenomeEntry, ViewmodelGenomeHandle,
};
pub use pose::{
    apply_ads_camera_fov, apply_ads_viewmodel, apply_viewmodel_sway_bob, track_right_mouse_state,
    update_ads_progress, AdsState, AdsTuning, ForgiaViewmodelPosePlugin, RightMouseState,
    ViewmodelMotionOffset, ViewmodelMotionTuning,
};

/// Plugin global : compose attach + pose + fade, idempotent sur MeshFaderPlugin.
///
/// Ne fournit PAS le firing — celui-ci reste dans `forgia-fps::ForgiaFpsPlugin`
/// qui consomme `ViewmodelGenomeCtx` / `AdsState` exposés par cette crate.
pub struct ForgiaViewmodelPlugin;

impl Plugin for ForgiaViewmodelPlugin {
    fn build(&self, app: &mut App) {
        // forgia-mesh-fader plugin (idempotent — d'autres crates peuvent l'utiliser).
        if !app.is_plugin_added::<forgia_effects::mesh_fader::MeshFaderPlugin>() {
            app.add_plugins(forgia_effects::mesh_fader::MeshFaderPlugin);
        }
        // Fix V5 Session C smoke test (commit 4f4506559) : Tier 2B extraction
        // (commit 6a45c6322) avait oublié d'enregistrer le type d'asset Genome<ViewmodelGenome>.
        // `load_viewmodel_genome` panic au boot sans ça (bevy_asset::server::info:819).
        // ViewmodelGenome n'impl pas Reflect (HashMap fields) → init_asset direct au lieu
        // de register_genome (qui requiert FromReflect). Pattern miroir : forgia-damage
        // lib.rs:202, forgia-enemy-nameplate tuning.rs:55, forgia-auto-rig lib.rs:1106.
        use forgia_genome_core::{Genome, GenomeLoader};
        app.init_asset::<Genome<genome::ViewmodelGenome>>()
            .register_asset_loader(GenomeLoader::<genome::ViewmodelGenome>::default());

        // Genome loading reste un système Startup global (pas gated FPS — l'asset
        // se charge en arrière-plan dès le boot, même en menu).
        app.add_systems(Startup, genome::load_viewmodel_genome)
            .add_plugins((
                ForgiaViewmodelAttachPlugin,
                ForgiaViewmodelPosePlugin,
                ForgiaViewmodelFadePlugin,
                arms::ForgiaViewmodelArmsPlugin,
                vm_camera::ForgiaViewmodelCameraPlugin,
            ));
    }
}

pub mod prelude {
    //! Re-exports pour `use forgia_viewmodel::prelude::*;`.
    pub use super::{
        AdsState, AdsTuning, ForgiaViewmodelPlugin, NeedsAutoScale, ScopeGlassFader,
        ViewmodelBaseScale, ViewmodelBodyFader, ViewmodelGenome, ViewmodelGenomeCtx,
        ViewmodelGenomeEntry, ViewmodelGenomeHandle, WeaponModelAssets, WeaponViewmodel,
    };
}
