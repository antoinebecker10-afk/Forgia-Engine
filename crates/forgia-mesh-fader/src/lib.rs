//! # forgia-mesh-fader
//!
//! Pattern réutilisable pour fader runtime un material de mesh entre opaque et
//! transparent, piloté par un float `progress` gameplay-driven.
//!
//! ## Use cases prévus
//! - Scope lens ADS (forgia-fps) : opaque hipfire → see-through ADS
//! - Windshield véhicules : transparence + cracks shader
//! - Visières casques characters
//! - Potions/fioles RPG : alpha selon contenu
//! - Glass panes bâtiments destructibles
//! - Despawn fade-out elegant (entity mort progressive)
//! - Mirages/illusions (RPG magic)
//!
//! ## API
//! 1. Tag une entity avec [`MaterialFader`] (target alpha + opaque alpha)
//! 2. Set `MaterialFader::progress` à chaque frame depuis ton système gameplay
//! 3. Ce crate clone le `StandardMaterial` une fois (anti-trap mutation partagée),
//!    set `AlphaMode::Blend`, et lerp l'alpha selon progress.
//!
//! ```ignore
//! use forgia_mesh_fader::{MaterialFader, MeshFaderPlugin};
//!
//! app.add_plugins(MeshFaderPlugin);
//!
//! // Tag une entity (typiquement après scene load + Name match)
//! commands.entity(scope_lens).insert(MaterialFader {
//!     progress: 0.0,        // 0 = opaque, 1 = full fade
//!     alpha_opaque: 1.0,
//!     alpha_target: 0.25,
//! });
//!
//! // Dans ton system gameplay, set progress (lerp toi-même selon AdsState, etc.)
//! fader.progress = ads.progress;
//! ```

use bevy::prelude::*;

/// Component à insérer sur l'entity dont le material doit fade.
/// `progress` est piloté par le système gameplay (ADS, état vehicule, despawn, etc.).
#[derive(Component, Clone, Copy)]
pub struct MaterialFader {
    /// 0.0 = opaque, 1.0 = fade complet vers `alpha_target`.
    pub progress: f32,
    /// Alpha à `progress = 0.0` (typiquement 1.0).
    pub alpha_opaque: f32,
    /// Alpha à `progress = 1.0` (e.g. 0.25 pour scope lens, 0.0 pour despawn).
    pub alpha_target: f32,
}

impl Default for MaterialFader {
    fn default() -> Self {
        Self {
            progress: 0.0,
            alpha_opaque: 1.0,
            alpha_target: 0.5,
        }
    }
}

impl MaterialFader {
    /// Constructeur ergonomique : opaque (1.0) → target.
    pub fn new(alpha_target: f32) -> Self {
        Self {
            progress: 0.0,
            alpha_opaque: 1.0,
            alpha_target,
        }
    }

    /// Alpha actuel calculé depuis progress.
    pub fn current_alpha(&self) -> f32 {
        self.alpha_opaque + (self.alpha_target - self.alpha_opaque) * self.progress.clamp(0.0, 1.0)
    }
}

/// Marker interne : le material a déjà été cloné pour cette entity.
/// Évite re-clone à chaque frame.
#[derive(Component)]
pub struct MaterialFaderCloned;

/// Update : pour chaque entity avec MaterialFader sans tag Cloned, clone son
/// StandardMaterial (anti-trap mutation partagée), set AlphaMode::Blend, tag.
pub fn clone_material_on_fader_add(
    mut commands: Commands,
    q_new: Query<
        (Entity, &MeshMaterial3d<StandardMaterial>),
        (With<MaterialFader>, Without<MaterialFaderCloned>),
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mat_handle) in q_new.iter() {
        let Some(original) = materials.get(&mat_handle.0) else {
            continue; // material pas encore chargé, retry next frame
        };
        let mut cloned = original.clone();
        cloned.alpha_mode = AlphaMode::Blend;
        let new_handle = materials.add(cloned);
        commands
            .entity(entity)
            .insert(MeshMaterial3d(new_handle))
            .insert(MaterialFaderCloned);
    }
}

/// Update : applique l'alpha calculé depuis progress sur le material cloné.
pub fn apply_material_fader_alpha(
    q: Query<(&MaterialFader, &MeshMaterial3d<StandardMaterial>), With<MaterialFaderCloned>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (fader, mat_handle) in q.iter() {
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            let alpha = fader.current_alpha();
            let mut color = mat.base_color.to_srgba();
            color.alpha = alpha;
            mat.base_color = color.into();
        }
    }
}

/// Sensor JSON debug — count entities + materials clonés + avg progress.
/// Refresh 1Hz. Écrit `forgia_mesh_fader.json` racine workspace.
/// Respecte `observability-required.md` Forgia.
#[derive(Resource)]
pub struct MeshFaderDebugTimer(pub Timer);

impl Default for MeshFaderDebugTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(1.0, TimerMode::Repeating))
    }
}

pub fn mesh_fader_debug_sensor(
    time: Res<Time>,
    mut timer: ResMut<MeshFaderDebugTimer>,
    q_all: Query<&MaterialFader>,
    q_cloned: Query<&MaterialFader, With<MaterialFaderCloned>>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }
    let total = q_all.iter().count();
    let cloned = q_cloned.iter().count();
    let avg_progress: f32 = if cloned > 0 {
        q_cloned.iter().map(|f| f.progress).sum::<f32>() / cloned as f32
    } else {
        0.0
    };
    let avg_alpha: f32 = if cloned > 0 {
        q_cloned.iter().map(|f| f.current_alpha()).sum::<f32>() / cloned as f32
    } else {
        1.0
    };
    let json = format!(
        r#"{{
  "timestamp_secs": {:.1},
  "total_faders": {},
  "cloned_materials": {},
  "pending_clone": {},
  "avg_progress": {:.3},
  "avg_current_alpha": {:.3}
}}
"#,
        time.elapsed_secs(),
        total,
        cloned,
        total.saturating_sub(cloned),
        avg_progress,
        avg_alpha,
    );
    let _ = std::fs::write("forgia_mesh_fader.json", json);
}

/// Plugin qui wire les 3 systems + sensor.
pub struct MeshFaderPlugin;

impl Plugin for MeshFaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MeshFaderDebugTimer>().add_systems(
            Update,
            (
                clone_material_on_fader_add,
                apply_material_fader_alpha,
                mesh_fader_debug_sensor,
            )
                .chain(),
        );
    }
}

pub mod prelude {
    pub use crate::{MaterialFader, MaterialFaderCloned, MeshFaderPlugin};
}
