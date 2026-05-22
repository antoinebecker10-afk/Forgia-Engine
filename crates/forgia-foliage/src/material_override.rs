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
use std::collections::VecDeque;

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

/// Diagnostic des fallback events pour pinpoint le bug
/// "invisible+collider" 2026-05-22 : sensor `forgia_foliage_fallback.json`.
///
/// Chaque event capture l'arbre concerné + la liste de ses mesh_children
/// (nom + tri_count + flag picked). Permet de voir si un mesh dégénéré
/// (tri=0) ou un Empty rafle le slot via `first()` ascending.
///
/// Buffer FIFO bounded (50 events) pour ne pas explorer l'allocation
/// au runtime. Hot path : 1 push par arbre fallback, jamais récurrent.
#[derive(Resource)]
pub struct FoliageFallbackDiagnostic {
    pub events: VecDeque<FallbackEvent>,
    pub max: usize,
    pub total_recorded: u64,
}

impl Default for FoliageFallbackDiagnostic {
    fn default() -> Self {
        Self {
            events: VecDeque::with_capacity(50),
            max: 50,
            total_recorded: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FallbackEvent {
    pub timestamp_secs: f32,
    pub tree_name: String,
    pub picked_name: String,
    pub picked_tri: usize,
    pub children: Vec<(String, usize)>,
}

/// Startup system : charge les 3 textures PBR et insere `BarkTextures`.
///
/// Sampler en mode `Repeat` obligatoire : `uv_transform` du material scale les
/// UVs > 1.0, et le default Bevy 0.18 (Clamp) renverrait le bord de l'image
/// partout = aplat uniforme. Voir memory `reference_v2_terrain_uv_tiling_repeat_sampler`.
pub fn preload_bark_textures(mut commands: Commands, asset_server: Res<AssetServer>) {
    use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
    let repeat = |s: &mut ImageLoaderSettings| {
        let mut desc = ImageSamplerDescriptor::linear();
        desc.address_mode_u = ImageAddressMode::Repeat;
        desc.address_mode_v = ImageAddressMode::Repeat;
        desc.address_mode_w = ImageAddressMode::Repeat;
        s.sampler = ImageSampler::Descriptor(desc);
    };
    commands.insert_resource(BarkTextures {
        diff: asset_server.load_with_settings(BARK_DIFF_PATH, repeat),
        normal: asset_server.load_with_settings(BARK_NOR_PATH, repeat),
        arm: asset_server.load_with_settings(BARK_ARM_PATH, repeat),
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
    mut diag: ResMut<FoliageFallbackDiagnostic>,
    time: Res<Time>,
    mut q_targets: Query<(Entity, &mut NeedsTrunkOverride)>,
    q_children: Query<&Children>,
    q_name: Query<&Name>,
    q_mesh_mat: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>)>,
) {
    // Lazy-build du material bark (une seule fois, premiere entite traitee).
    let bark_mat_handle = match &bark.material {
        Some(h) => h.clone(),
        None => {
            // Bark PBR Jolcham : diff + normal + ARM packed (occlusion + MR meme handle).
            // uv_transform identite (1.0) : pas de tiling additionnel, on respecte
            // le UV mapping natif du GLB Quaternius. Si trop etire, augmenter scale.
            let mat = StandardMaterial {
                base_color_texture: Some(bark.diff.clone()),
                normal_map_texture: Some(bark.normal.clone()),
                occlusion_texture: Some(bark.arm.clone()),
                metallic_roughness_texture: Some(bark.arm.clone()),
                perceptual_roughness: 0.85,
                metallic: 0.0,
                uv_transform: bevy::math::Affine2::from_scale(Vec2::splat(1.0)),
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
            // Bevy 0.18 quirk : `insert(MeshMaterial3d)` ne remplace pas
            // toujours le component existant via Commands. `remove + insert`
            // force le swap atomique.
            commands
                .entity(trunk_child)
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert(MeshMaterial3d(bark_mat_handle.clone()));
            commands.entity(entity).remove::<NeedsTrunkOverride>();
            cfg.overridden += 1;
        } else if marker.frames_polled >= cfg.frames_polled_max && !mesh_children.is_empty() {
            // Fallback : enfant avec le PLUS PETIT nombre de triangles (canopy
            // typique = sphere/cluster avec plus de tris, trunk = cylindre simple).
            // sort_unstable_by_key ascending + .first() = lowest tri count = trunk.
            mesh_children.sort_unstable_by_key(|&(_, tri)| tri);
            let (fallback_entity, picked_tri) = *mesh_children.first().unwrap();

            // Diagnostic 2026-05-22 : capture l'arbre + tous ses mesh_children avec
            // noms et tri_count. Permet de pinpoint le bug "invisible+collider"
            // (hypothèse : un Empty/mesh dégénéré tri=0 rafle le slot via first()).
            let tree_name = q_name
                .get(entity)
                .map(|n| n.as_str().to_string())
                .unwrap_or_default();
            let picked_name = q_name
                .get(fallback_entity)
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|_| "<unnamed>".to_string());
            let children_named: Vec<(String, usize)> = mesh_children
                .iter()
                .map(|&(e, tri)| {
                    let n = q_name
                        .get(e)
                        .map(|n| n.as_str().to_string())
                        .unwrap_or_else(|_| "<unnamed>".to_string());
                    (n, tri)
                })
                .collect();
            if diag.events.len() >= diag.max {
                diag.events.pop_front();
            }
            diag.events.push_back(FallbackEvent {
                timestamp_secs: time.elapsed_secs(),
                tree_name,
                picked_name,
                picked_tri,
                children: children_named,
            });
            diag.total_recorded = diag.total_recorded.saturating_add(1);

            commands
                .entity(fallback_entity)
                .remove::<MeshMaterial3d<StandardMaterial>>()
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

/// Écrit `forgia_foliage_fallback.json` toutes les ~1s. Dump les events
/// fallback récents (buffer FIFO 50) pour diag bug "invisible+collider".
pub fn write_foliage_fallback_sensor(
    time: Res<Time>,
    diag: Res<FoliageFallbackDiagnostic>,
    mut last_write: Local<f32>,
) {
    let now = time.elapsed_secs();
    if now - *last_write < 1.0 {
        return;
    }
    *last_write = now;

    let events_json: String = diag
        .events
        .iter()
        .map(|e| {
            let children_json: String = e
                .children
                .iter()
                .map(|(n, tri)| {
                    let picked = n == &e.picked_name;
                    format!(
                        "{{\"name\":\"{}\",\"tri\":{},\"picked\":{}}}",
                        n.replace('"', "'"),
                        tri,
                        picked
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"t\":{:.1},\"tree\":\"{}\",\"picked_name\":\"{}\",\"picked_tri\":{},\"children\":[{}]}}",
                e.timestamp_secs,
                e.tree_name.replace('"', "'"),
                e.picked_name.replace('"', "'"),
                e.picked_tri,
                children_json
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let json = format!(
        "{{\"timestamp_secs\":{:.1},\"total_recorded\":{},\"buffer_len\":{},\"events\":[{}]}}",
        now,
        diag.total_recorded,
        diag.events.len(),
        events_json
    );
    let _ = std::fs::write("forgia_foliage_fallback.json", json);
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
