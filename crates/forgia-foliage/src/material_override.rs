//! # material_override
//!
//! Override material trunk Jolcham Oak Bark sur tous les arbres (`AssetCategory::Tree`)
//! spawnes dans le monde RPG.
//!
//! ## Pipeline
//!
//! 1. `preload_bark_textures` (Startup) : charge les 3 textures PBR et insere `BarkTextures`.
//! 2. `populate_new_chunks` (foliage/lib.rs) : insere `NeedsTrunkOverride` sur chaque arbre
//!    si `BarkOverrideConfig.enabled`.
//! 3. `apply_trunk_bark_override` (Update, GameSet::Movement) : BFS polling sur les enfants
//!    de chaque entite marquee, cherche la primitive trunk par nom, applique le material.
//!    Fallback : dernier enfant par triangle count apres `frames_polled_max` frames.
//!
//! ## Textures
//!
//! Source : Poly Haven CC0, deposes `assets/textures/pbr/jolcham_oak_bark_01/`.

use bevy::prelude::*;

/// Chemin relatif aux 3 textures PBR Jolcham Oak Bark (depuis `assets/`).
pub const BARK_DIFF_PATH: &str =
    "textures/pbr/jolcham_oak_bark_01/jolcham_oak_bark_01_diff_2k.jpg";
pub const BARK_NOR_PATH: &str =
    "textures/pbr/jolcham_oak_bark_01/jolcham_oak_bark_01_nor_gl_2k.jpg";
pub const BARK_ARM_PATH: &str =
    "textures/pbr/jolcham_oak_bark_01/jolcham_oak_bark_01_arm_2k.jpg";

/// Noms de primitives consideres comme "tronc" (lowercase substring match).
pub const TRUNK_PATTERNS: &[&str] = &["bark", "trunk", "wood", "stem"];

/// Retourne `true` si `name` (insensible a la casse) contient l'un des patterns tronc.
pub fn is_trunk_primitive_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    TRUNK_PATTERNS.iter().any(|pat| lower.contains(pat))
}

/// Handles vers les textures Jolcham Oak Bark + material bake lazily (1ere execution).
#[derive(Resource, Default)]
pub struct BarkTextures {
    pub diff: Handle<Image>,
    pub normal: Handle<Image>,
    pub arm: Handle<Image>,
    /// Material bake lazily au 1er appel `apply_trunk_bark_override`.
    pub material: Option<Handle<StandardMaterial>>,
}

/// Configuration de l'override material trunk.
///
/// `Default` custom : `enabled = true`, `frames_polled_max = 30`
/// pour boot ON automatique sans code cote forgia-game.
#[derive(Resource, Debug)]
pub struct BarkOverrideConfig {
    /// Active/desactive l'override au runtime.
    pub enabled: bool,
    /// Nombre max de frames de polling avant fallback triangle count.
    pub frames_polled_max: u8,
    /// Arbres overrides par nom (statistiques sensor).
    pub overridden: u32,
    /// Arbres overrides par fallback triangle count.
    pub fallback: u32,
    /// Arbres abandonnes (aucune primitive trouvee apres double timeout).
    pub not_found: u32,
}

impl Default for BarkOverrideConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            frames_polled_max: 30,
            overridden: 0,
            fallback: 0,
            not_found: 0,
        }
    }
}

/// Marker pose sur chaque arbre necessitant un override trunk.
/// Retire une fois l'override applique (ou abandonne).
#[derive(Component, Default)]
pub struct NeedsTrunkOverride {
    pub frames_polled: u8,
}

/// Startup system : charge les 3 textures PBR et insere `BarkTextures`.
pub fn preload_bark_textures(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(BarkTextures {
        diff: asset_server.load(BARK_DIFF_PATH),
        normal: asset_server.load(BARK_NOR_PATH),
        arm: asset_server.load(BARK_ARM_PATH),
        material: None,
    });
}

/// Update system : applique l'override material trunk sur les arbres marques.
///
/// Pour chaque entite `NeedsTrunkOverride` :
/// - BFS via `Children` pour trouver un enfant dont le `Name` matche `is_trunk_primitive_name`
///   et qui possede un `Mesh3d`.
/// - Si trouve : insere `MeshMaterial3d(bark_mat)`, retire le marker.
/// - Sinon si `frames_polled > frames_polled_max` : fallback par triangle count
///   (enfant avec le plus grand nombre de triangles).
/// - Sinon si `frames_polled > frames_polled_max * 2` : abandon, retire le marker.
/// - Sinon : incremente `frames_polled`.
#[allow(clippy::too_many_arguments)]
pub fn apply_trunk_bark_override(
    mut commands: Commands,
    mut bark: ResMut<BarkTextures>,
    mut cfg: ResMut<BarkOverrideConfig>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    meshes: Res<Assets<Mesh>>,
    mut q_targets: Query<(Entity, &mut NeedsTrunkOverride)>,
    q_children: Query<&Children>,
    q_name: Query<&Name>,
    q_mesh_mat: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
) {
    // Lazy-build du material bark (une seule fois, premiere entite traitee).
    let bark_mat_handle = match &bark.material {
        Some(h) => h.clone(),
        None => {
            let mat = StandardMaterial {
                base_color_texture: Some(bark.diff.clone()),
                normal_map_texture: Some(bark.normal.clone()),
                occlusion_texture: Some(bark.arm.clone()),
                metallic_roughness_texture: Some(bark.arm.clone()),
                perceptual_roughness: 0.85,
                metallic: 0.0,
                ..default()
            };
            let handle = materials.add(mat);
            bark.material = Some(handle.clone());
            handle
        }
    };

    for (entity, mut marker) in &mut q_targets {
        // Collecte tous les descendants via BFS iteratif.
        let mut stack: Vec<Entity> = vec![entity];
        let mut name_match: Option<Entity> = None;
        let mut mesh_children: Vec<(Entity, usize)> = Vec::new(); // (entity, tri_count)

        while let Some(current) = stack.pop() {
            // Cherche enfants.
            if let Ok(children) = q_children.get(current) {
                for child in children.iter() {
                    stack.push(child);

                    // Verifie si cet enfant a un mesh (pour fallback tri count).
                    if let Ok((mesh3d, _mat)) = q_mesh_mat.get(child) {
                        let tri_count = meshes
                            .get(&mesh3d.0)
                            .map(|m| m.indices().map_or(0, |idx| idx.len() / 3))
                            .unwrap_or(0);
                        mesh_children.push((child, tri_count));

                        // Verifie si le nom matche.
                        if name_match.is_none() {
                            if let Ok(name) = q_name.get(child) {
                                if is_trunk_primitive_name(name.as_str()) {
                                    name_match = Some(child);
                                }
                            }
                        }
                    }
                }
            }
        }

        let double_timeout = cfg.frames_polled_max.saturating_mul(2);

        if let Some(trunk_child) = name_match {
            // Match par nom : override immediat.
            commands
                .entity(trunk_child)
                .insert(MeshMaterial3d(bark_mat_handle.clone()));
            commands.entity(entity).remove::<NeedsTrunkOverride>();
            cfg.overridden += 1;
        } else if marker.frames_polled >= cfg.frames_polled_max && !mesh_children.is_empty() {
            // Fallback : enfant avec le plus grand nombre de triangles.
            mesh_children.sort_unstable_by_key(|&(_, tri)| tri);
            let (fallback_entity, _) = *mesh_children.last().unwrap();
            commands
                .entity(fallback_entity)
                .insert(MeshMaterial3d(bark_mat_handle.clone()));
            commands.entity(entity).remove::<NeedsTrunkOverride>();
            cfg.fallback += 1;
        } else if marker.frames_polled >= double_timeout {
            // Abandon : aucune primitive apres double timeout.
            commands.entity(entity).remove::<NeedsTrunkOverride>();
            cfg.not_found += 1;
        } else {
            marker.frames_polled += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests purs (sans App Bevy)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_canonical_variants() {
        assert!(is_trunk_primitive_name("bark"));
        assert!(is_trunk_primitive_name("BARK"));
        assert!(is_trunk_primitive_name("Bark_LOD0"));
        assert!(is_trunk_primitive_name("tree_bark"));
        assert!(is_trunk_primitive_name("wood_01"));
        assert!(is_trunk_primitive_name("stem_main"));
        assert!(is_trunk_primitive_name("Trunk"));
        assert!(is_trunk_primitive_name("trunk_low"));
    }

    #[test]
    fn rejects_canopy_leaves() {
        assert!(!is_trunk_primitive_name("leaves"));
        assert!(!is_trunk_primitive_name("canopy"));
        assert!(!is_trunk_primitive_name("foliage_high"));
        assert!(!is_trunk_primitive_name("Sphere001"));
        assert!(!is_trunk_primitive_name("Empty"));
        assert!(!is_trunk_primitive_name(""));
    }

    #[test]
    fn case_insensitive() {
        assert!(is_trunk_primitive_name("WOOD_01"));
        assert!(is_trunk_primitive_name("Stem_Main"));
        assert!(!is_trunk_primitive_name("LEAVES"));
    }

    #[test]
    fn substring_match() {
        // Patterns peuvent etre au milieu ou a la fin du nom.
        assert!(is_trunk_primitive_name("oak_bark_lod0"));
        assert!(is_trunk_primitive_name("mesh_trunk_001"));
        assert!(is_trunk_primitive_name("polyhaven_wood"));
        assert!(is_trunk_primitive_name("root_stem_a"));
    }
}
