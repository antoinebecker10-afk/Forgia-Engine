//! Adapter forgia-fps → forgia-mesh-fader pour la lentille scope en ADS.
//!
//! Stratégie :
//! 1. `identify_scope_glass_meshes` : au spawn viewmodel, BFS descendants. Pour chaque
//!    Name match pattern `lens|scope|glass|optic|reticle`, insert `MaterialFader`.
//! 2. `sync_scope_fader_progress` : copie `AdsState.progress` dans les `MaterialFader`
//!    taggés scope. Le crate forgia-mesh-fader handle le clone + alpha lerp automatiquement.

use bevy::prelude::*;
use forgia_combat::weapons::EquippedWeapons;
use forgia_genome_core::Genome;
use forgia_mesh_fader::MaterialFader;

use crate::ads::AdsState;
use crate::{lookup_genome_entry, ViewmodelGenome, ViewmodelGenomeHandle, WeaponViewmodel};

/// Marker : viewmodel déjà scanné pour scope glass meshes.
#[derive(Component)]
pub struct ScopeGlassScanned;

/// Marker : ce MaterialFader appartient à un scope (vs windshield, visor, etc.).
/// Permet à `sync_scope_fader_progress` de filtrer.
#[derive(Component)]
pub struct ScopeGlassFader;

/// Patterns Name nodes considérés comme "scope / viseur" (case-insensitive).
/// Inclut substring matches — donc "_scope", "scope_body", "iron_sight" matchent tous.
const SCOPE_GLASS_PATTERNS: &[&str] = &[
    "lens", "glass", "optic", "reticle",
    "scope",   // catch tout submesh nommé "*_scope*" ou "scope_*"
    "sight",   // catch "iron_sight", "front_sight", "sight_dot"
    "dot",     // red dot patterns
];

/// Sockets / placeholders à EXCLURE (faux positifs : SOCKET_Sight_Front_Default).
const EXCLUDE_PATTERNS: &[&str] = &[
    "socket", "muzzle", "laser", "barrel", "stock", "grip", "magazine",
];

fn is_scope_glass_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    // Exclusion d'abord (socket "Sight_Front" contient "sight" mais est un placeholder)
    if EXCLUDE_PATTERNS.iter().any(|p| lower.contains(p)) {
        return false;
    }
    SCOPE_GLASS_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Update : pour chaque viewmodel sans tag scanned, BFS descendants, log noms,
/// insert MaterialFader sur les nodes match pattern scope.
pub fn identify_scope_glass_meshes(
    mut commands: Commands,
    q_vm: Query<Entity, (With<WeaponViewmodel>, Without<ScopeGlassScanned>)>,
    q_children: Query<&Children>,
    q_name: Query<&Name>,
    equipped: Res<EquippedWeapons>,
    genome_handle: Option<Res<ViewmodelGenomeHandle>>,
    genome_assets: Res<Assets<Genome<ViewmodelGenome>>>,
) {
    for vm in q_vm.iter() {
        // Read alpha_target depuis genome (no-hardcode)
        let alpha_target = genome_handle
            .as_deref()
            .and_then(|h| lookup_genome_entry(&genome_assets, h, equipped.current))
            .map(|e| e.scope_glass_alpha_ads)
            .unwrap_or(0.25);

        let mut stack = vec![vm];
        let mut all_names: Vec<String> = Vec::new();
        let mut matched_count = 0;

        while let Some(e) = stack.pop() {
            let name_str = q_name.get(e).map(|n| n.as_str().to_string()).unwrap_or_default();
            if !name_str.is_empty() {
                all_names.push(name_str.clone());
            }

            if !name_str.is_empty() && is_scope_glass_name(&name_str) {
                commands
                    .entity(e)
                    .insert(MaterialFader::new(alpha_target))
                    .insert(ScopeGlassFader);
                matched_count += 1;
                info!(
                    "[scope_glass] Tagged scope mesh '{}' (alpha_target={:.2} from genome)",
                    name_str, alpha_target
                );
            }

            if let Ok(children) = q_children.get(e) {
                for child in children.iter() {
                    stack.push(child);
                }
            }
        }

        commands.entity(vm).insert(ScopeGlassScanned);

        if matched_count == 0 {
            info!(
                "[scope_glass] AUCUN match parmi {} nodes nommés : {:?}",
                all_names.len(),
                all_names
            );
        } else {
            info!("[scope_glass] {} lentille(s) tagged MaterialFader", matched_count);
        }
    }
}

/// Update : copie AdsState.progress dans MaterialFader.progress des nodes scope.
/// forgia-mesh-fader::apply_material_fader_alpha s'occupe du lerp + render.
pub fn sync_scope_fader_progress(
    ads: Res<AdsState>,
    mut q: Query<&mut MaterialFader, With<ScopeGlassFader>>,
) {
    for mut fader in &mut q {
        fader.progress = ads.progress;
    }
}

pub struct ScopeGlassPlugin;

impl Plugin for ScopeGlassPlugin {
    fn build(&self, app: &mut App) {
        // forgia-mesh-fader::MeshFaderPlugin doit être add séparément au niveau du game (1 fois global).
        app.add_systems(
            Update,
            (identify_scope_glass_meshes, sync_scope_fader_progress)
                .run_if(in_state(forgia_core::prelude::GameMode::Fps)),
        );
    }
}

