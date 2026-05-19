//! Couche **fade** — semi-transparence du viewmodel en ADS via `forgia-mesh-fader`.
//!
//! Stratégie 2-en-1 :
//! 1. `identify_viewmodel_faders` : au spawn viewmodel, BFS descendants. Tag chaque
//!    entity selon son rôle :
//!    - Name match `lens|scope|glass|optic|sight|dot` → `ScopeGlassFader` (lentille,
//!      alpha = `scope_glass_alpha_ads`).
//!    - Autre entity material-bearing → `ViewmodelBodyFader` (body, alpha =
//!      `ads_viewmodel_fade_alpha`). Skippé si alpha >= 0.99 (sniper qui a son
//!      overlay fullscreen).
//! 2. `sync_fader_progress` : copie `AdsState.progress` dans tous les `MaterialFader`
//!    taggés (scope + body). `forgia-mesh-fader` handle le clone + alpha lerp.
//!
//! Rationale : en ADS sur armes non-sniper, le mesh du gun masque la cible derrière
//! le crosshair. Le body fader rend le gun semi-transparent (alpha 0.4 default)
//! pour laisser voir à travers. Sniper Lenoir exclu (`ads_viewmodel_fade_alpha = 1.0`
//! dans TOML) car il a son scope_fullscreen overlay.

use bevy::prelude::*;
use forgia_combat::weapons::EquippedWeapons;
use forgia_core::prelude::GameMode;
use forgia_genome_core::Genome;
use forgia_mesh_fader::MaterialFader;

use crate::attach::WeaponViewmodel;
use crate::genome::{lookup_genome_entry, ViewmodelGenome, ViewmodelGenomeHandle};
use crate::pose::AdsState;

/// Marker : viewmodel déjà scanné pour scope/body faders.
#[derive(Component)]
pub struct ScopeGlassScanned;

/// Marker : ce MaterialFader appartient à un scope (lentille / sight / red dot).
#[derive(Component)]
pub struct ScopeGlassFader;

/// Marker : ce MaterialFader appartient au BODY du viewmodel (mesh principal,
/// hors lentille). En ADS, fade vers `ads_viewmodel_fade_alpha`.
#[derive(Component)]
pub struct ViewmodelBodyFader;

/// Patterns Name nodes considérés comme "scope / viseur" (case-insensitive).
const SCOPE_GLASS_PATTERNS: &[&str] = &[
    "lens", "glass", "optic", "reticle",
    "scope", "sight", "dot",
];

/// Sockets / placeholders à EXCLURE (faux positifs : SOCKET_Sight_Front_Default).
const EXCLUDE_PATTERNS: &[&str] = &[
    "socket", "muzzle", "laser", "barrel", "stock", "grip", "magazine",
];

fn is_scope_glass_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    if EXCLUDE_PATTERNS.iter().any(|p| lower.contains(p)) {
        return false;
    }
    SCOPE_GLASS_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Update : pour chaque viewmodel sans tag scanned, BFS descendants. Tag scope
/// (Name match) + body (entity material-bearing non-scope si fade activé).
#[allow(clippy::too_many_arguments)]
pub fn identify_viewmodel_faders(
    mut commands: Commands,
    q_vm: Query<Entity, (With<WeaponViewmodel>, Without<ScopeGlassScanned>)>,
    q_children: Query<&Children>,
    q_name: Query<&Name>,
    q_has_material: Query<(), With<MeshMaterial3d<StandardMaterial>>>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
) {
    for vm in q_vm.iter() {
        let entry = genome_handle
            .as_deref()
            .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current));
        let scope_alpha = entry.map(|e| e.scope_glass_alpha_ads).unwrap_or(0.25);
        let body_alpha = entry.map(|e| e.ads_viewmodel_fade_alpha).unwrap_or(0.4);
        let body_fade_enabled = body_alpha < 0.99;

        let mut stack = vec![vm];
        let mut all_names: Vec<String> = Vec::new();
        let mut scope_count = 0;
        let mut body_count = 0;

        while let Some(e) = stack.pop() {
            let name_str = q_name.get(e).map(|n| n.as_str().to_string()).unwrap_or_default();
            if !name_str.is_empty() {
                all_names.push(name_str.clone());
            }

            let is_scope = !name_str.is_empty() && is_scope_glass_name(&name_str);
            let has_material = q_has_material.get(e).is_ok();

            if is_scope {
                commands
                    .entity(e)
                    .insert(MaterialFader::new(scope_alpha))
                    .insert(ScopeGlassFader);
                scope_count += 1;
            } else if has_material && body_fade_enabled {
                commands
                    .entity(e)
                    .insert(MaterialFader::new(body_alpha))
                    .insert(ViewmodelBodyFader);
                body_count += 1;
            }

            if let Ok(children) = q_children.get(e) {
                for child in children.iter() {
                    stack.push(child);
                }
            }
        }

        commands.entity(vm).insert(ScopeGlassScanned);

        info!(
            "[forgia-viewmodel/fade] scan terminé : {} scope, {} body (fade_enabled={}, body_alpha={:.2}, scope_alpha={:.2}, total_named={})",
            scope_count, body_count, body_fade_enabled, body_alpha, scope_alpha, all_names.len()
        );
        if scope_count == 0 {
            info!("[forgia-viewmodel/fade] aucun scope match parmi : {:?}", all_names);
        }
    }
}

/// Update : copie AdsState.progress dans MaterialFader.progress de toutes les
/// entities taggées (scope OU body). forgia-mesh-fader::apply_material_fader_alpha
/// fait le lerp réel + render.
pub fn sync_fader_progress(
    ads: Res<AdsState>,
    mut q_scope: Query<&mut MaterialFader, (With<ScopeGlassFader>, Without<ViewmodelBodyFader>)>,
    mut q_body: Query<&mut MaterialFader, (With<ViewmodelBodyFader>, Without<ScopeGlassFader>)>,
) {
    for mut fader in &mut q_scope {
        fader.progress = ads.progress;
    }
    for mut fader in &mut q_body {
        fader.progress = ads.progress;
    }
}

/// Plugin fade : scan + sync. Gated `run_if(in_state(GameMode::Fps))`.
/// `MeshFaderPlugin` doit être ajouté séparément (1× global) — typiquement par
/// `ForgiaViewmodelPlugin` qui le check idempotent.
pub struct ForgiaViewmodelFadePlugin;

impl Plugin for ForgiaViewmodelFadePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (identify_viewmodel_faders, sync_fader_progress)
                .run_if(in_state(GameMode::Fps).or(in_state(GameMode::Roguelite))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_scope_glass_name_basic_patterns() {
        assert!(is_scope_glass_name("scope_lens_01"));
        assert!(is_scope_glass_name("Lens"));
        assert!(is_scope_glass_name("red_dot"));
        assert!(is_scope_glass_name("sight_front"));
        assert!(is_scope_glass_name("optical_sight"));
    }

    #[test]
    fn is_scope_glass_name_excludes_sockets() {
        // SOCKET_Sight_Front_Default doit retourner false malgré "sight" inside.
        assert!(!is_scope_glass_name("SOCKET_Sight_Front_Default"));
        assert!(!is_scope_glass_name("muzzle_flash_socket"));
        assert!(!is_scope_glass_name("barrel_long"));
        assert!(!is_scope_glass_name("grip_handle"));
    }

    #[test]
    fn is_scope_glass_name_negatives() {
        assert!(!is_scope_glass_name("body_main"));
        assert!(!is_scope_glass_name(""));
        assert!(!is_scope_glass_name("trigger"));
    }
}
