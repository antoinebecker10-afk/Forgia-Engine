//! Asset registry — per-module metadata for procedural worldgen (story-578 P0).
//!
//! Pure data, no Bevy. The registry catalogs the reusable modules of an asset kit
//! (e.g. ithappy `platformer_underworld`) together with the geometry needed to **place**
//! them correctly: the local-space AABB and `ground_offset` (the y to add so the module's
//! bottom rests on the placement height — the fix for the recurring "sunken / floating
//! asset" bug, cf the loot-room saga).
//!
//! Geometry fields are produced **exactly** from the kit GLB (accessor min/max). `role`
//! and `collider` are a geometric first pass to be refined by hand in the RON — it is data,
//! not logic, so no rebuild is needed to retune them.

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A 3-component vector as stored in RON: `(x, y, z)`. Kept as a plain tuple to avoid a
/// math dependency in this pure-data crate (Bevy/glam arrive in P1).
pub type V3 = (f32, f32, f32);

/// What a module is *for* — drives layout (where it can go) and collision.
///
/// This is a deliberately small, gameplay-agnostic vocabulary. It is a *first geometric
/// pass*; refine entries directly in `asset_meta.ron`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssetRole {
    /// Walkable surface (block, bridge, disc, floor).
    Platform,
    /// Tall vertical element (column, spire, tower).
    Pillar,
    /// Long thin vertical element (fence, beam, wall segment).
    Wall,
    /// Sloped walkable connector.
    Ramp,
    /// Solid decorative object (rock, crate, lamp).
    Prop,
    /// Flat zero-height card (crack, foliage decal, shadow). No collision.
    Decal,
    /// Large distant scenery (cliff, mountain). Visual only.
    Backdrop,
    /// Pickup / treasure anchor.
    Loot,
    /// Damaging volume (lava, spikes).
    Hazard,
    /// Unclassified — needs manual review.
    Unknown,
}

/// How to build the physics collider for a module.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColliderKind {
    /// Axis-aligned box from the AABB — cheapest, for blocky walkables.
    Cuboid,
    /// Vertical cylinder from the AABB — columns / posts.
    Cylinder,
    /// Convex hull of the mesh — solid props (fills concavities).
    ConvexHull,
    /// Exact triangle mesh — preserves openings (arches, passages stay traversable).
    TriMesh,
    /// No collider — decals, backdrops, visual-only scenery.
    NoCollider,
}

/// Metadata for a single reusable module of an asset kit.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssetMeta {
    /// glTF mesh name — the stable spawn id (`gltf.named_meshes[id]`).
    pub id: String,
    /// Local-space AABB minimum (mesh coordinates, before scaling).
    pub aabb_min: V3,
    /// Local-space AABB maximum.
    pub aabb_max: V3,
    /// Y to add to the placement height so the module's bottom rests on the ground.
    /// Equals `-aabb_min.y`. This is the anti-sink/float pivot.
    pub ground_offset: f32,
    /// What the module is for.
    pub role: AssetRole,
    /// How to build its collider.
    pub collider: ColliderKind,
}

impl AssetMeta {
    /// Size of the AABB: `(width_x, height_y, depth_z)`.
    #[inline]
    pub fn size(&self) -> V3 {
        (
            self.aabb_max.0 - self.aabb_min.0,
            self.aabb_max.1 - self.aabb_min.1,
            self.aabb_max.2 - self.aabb_min.2,
        )
    }

    /// Height of the module (local AABB extent on Y).
    #[inline]
    pub fn height(&self) -> f32 {
        self.aabb_max.1 - self.aabb_min.1
    }

    /// World Y at which to place the module's origin so its **bottom rests on `ground_y`**,
    /// for a uniform `scale`. This is the grounding formula:
    /// `placement_y + aabb_min.y * scale == ground_y` for all modules.
    #[inline]
    pub fn placement_y(&self, ground_y: f32, scale: f32) -> f32 {
        ground_y + self.ground_offset * scale
    }
}

/// A versioned RON document: one asset kit's module catalog.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AssetRegistryFile {
    /// Schema version (bump on breaking field changes).
    pub version: u32,
    /// Kit name (e.g. `"platformer_underworld"`).
    pub kit: String,
    /// Asset path of the GLB the modules are spawned from.
    pub source_glb: String,
    /// All modules in the kit.
    pub entries: Vec<AssetMeta>,
}

/// Runtime lookup: module id -> metadata, with kit / source provenance.
#[derive(Resource, Clone, Debug, Default)]
pub struct AssetRegistry {
    /// Kit name this registry was built from.
    pub kit: String,
    /// GLB asset path modules are spawned from.
    pub source_glb: String,
    by_id: HashMap<String, AssetMeta>,
}

impl AssetRegistry {
    /// Parse a registry from a RON document string.
    pub fn from_ron(ron_str: &str) -> Result<Self, ron::error::SpannedError> {
        let file: AssetRegistryFile = ron::from_str(ron_str)?;
        Ok(Self::from_file(file))
    }

    /// Build the runtime registry from a parsed document.
    ///
    /// Intentionally **infallible**: duplicate ids collapse silently (last wins). If you
    /// need to detect collisions, compare `len()` against `file.entries.len()` before/after
    /// (this is what the `all_ids_are_unique` test does). A future faillible loader can be
    /// added when a second consumer needs hard validation (story-578 P1).
    pub fn from_file(file: AssetRegistryFile) -> Self {
        let mut by_id = HashMap::with_capacity(file.entries.len());
        for entry in file.entries {
            by_id.insert(entry.id.clone(), entry);
        }
        Self {
            kit: file.kit,
            source_glb: file.source_glb,
            by_id,
        }
    }

    /// Look up a module by its glTF mesh id.
    #[inline]
    pub fn get(&self, id: &str) -> Option<&AssetMeta> {
        self.by_id.get(id)
    }

    /// Number of unique modules.
    #[inline]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether the registry has no modules.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Iterate over all modules.
    pub fn iter(&self) -> impl Iterator<Item = &AssetMeta> {
        self.by_id.values()
    }

    /// Iterate over modules with a given role.
    pub fn by_role(&self, role: AssetRole) -> impl Iterator<Item = &AssetMeta> {
        self.by_id.values().filter(move |m| m.role == role)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real generated registry of the ithappy `platformer_underworld` kit.
    ///
    /// NOTE: `include_str!` here is **test-only** (embed the data so tests are hermetic and
    /// cwd-independent). The P1 runtime path loads the RON via the Bevy asset server, not this.
    const ASSET_META_RON: &str = include_str!("../../../assets/registry/asset_meta.ron");

    fn registry() -> AssetRegistry {
        AssetRegistry::from_ron(ASSET_META_RON).expect("asset_meta.ron must parse")
    }

    #[test]
    fn ron_parses_and_has_expected_shape() {
        let file: AssetRegistryFile =
            ron::from_str(ASSET_META_RON).expect("asset_meta.ron must parse");
        assert_eq!(file.version, 1);
        assert_eq!(file.kit, "platformer_underworld");
        assert!(file.source_glb.ends_with(".glb"));
        assert_eq!(file.entries.len(), 107, "all ithappy modules catalogued");
    }

    #[test]
    fn all_ids_are_unique() {
        let file: AssetRegistryFile = ron::from_str(ASSET_META_RON).unwrap();
        let raw = file.entries.len();
        let reg = AssetRegistry::from_file(file);
        assert_eq!(reg.len(), raw, "no duplicate module ids (HashMap would collapse them)");
    }

    #[test]
    fn lookup_by_known_id_works() {
        let reg = registry();
        // Cube.017 is one of the 10x10 walkable blocks.
        let m = reg.get("Cube.017").expect("Cube.017 present");
        assert_eq!(m.role, AssetRole::Platform);
    }

    #[test]
    fn aabb_is_well_formed() {
        let reg = registry();
        for m in reg.iter() {
            assert!(m.aabb_min.0 <= m.aabb_max.0, "{}: min.x > max.x", m.id);
            assert!(m.aabb_min.1 <= m.aabb_max.1, "{}: min.y > max.y", m.id);
            assert!(m.aabb_min.2 <= m.aabb_max.2, "{}: min.z > max.z", m.id);
            for c in [
                m.aabb_min.0, m.aabb_min.1, m.aabb_min.2, m.aabb_max.0, m.aabb_max.1,
                m.aabb_max.2, m.ground_offset,
            ] {
                assert!(c.is_finite(), "{}: non-finite geometry", m.id);
            }
        }
    }

    #[test]
    fn ground_offset_matches_pivot() {
        // ground_offset must be exactly -aabb_min.y (RON rounded to 4 decimals).
        let reg = registry();
        for m in reg.iter() {
            let expected = -m.aabb_min.1;
            assert!(
                (m.ground_offset - expected).abs() < 1e-3,
                "{}: ground_offset {} != -aabb_min.y {}",
                m.id,
                m.ground_offset,
                expected
            );
        }
    }

    /// THE P0 PROOF: every module, at any scale, at any ground height, lands with its
    /// bottom exactly on the ground — no sinking, no floating. This is the invariant the
    /// loot-room saga (gate enfoncée -16.9, passages flottants) violated for lack of a pivot.
    #[test]
    fn every_module_grounds_correctly() {
        let reg = registry();
        for &ground in &[0.0_f32, 5.0, -3.2, 41.7] {
            for &scale in &[1.0_f32, 0.8, 0.25, 2.0] {
                for m in reg.iter() {
                    let placed_origin_y = m.placement_y(ground, scale);
                    let bottom = placed_origin_y + m.aabb_min.1 * scale;
                    assert!(
                        (bottom - ground).abs() < 1e-3,
                        "{}: bottom {} != ground {} (scale {})",
                        m.id,
                        bottom,
                        ground,
                        scale
                    );
                }
            }
        }
    }

    #[test]
    fn decals_have_no_collider_and_zero_height() {
        let reg = registry();
        for m in reg.by_role(AssetRole::Decal) {
            assert_eq!(m.collider, ColliderKind::NoCollider, "{} decal must not collide", m.id);
            assert!(m.height() < 0.06, "{} decal should be flat", m.id);
        }
    }

    /// Invariant guarding the QA-03 bug class: only purely-visual roles (Decal, Backdrop)
    /// may have NoCollider. Anything you can stand on or bump into MUST be solid — a
    /// walkable accidentally classified as collider-less would drop the player through it.
    #[test]
    fn solid_roles_have_colliders() {
        let reg = registry();
        for m in reg.iter() {
            let visual_only = matches!(m.role, AssetRole::Decal | AssetRole::Backdrop);
            let no_collider = m.collider == ColliderKind::NoCollider;
            assert_eq!(
                visual_only, no_collider,
                "{}: role {:?} vs collider {:?} — solid roles must collide, visual roles must not",
                m.id, m.role, m.collider
            );
        }
    }

    #[test]
    fn has_walkable_and_vertical_modules() {
        // Sanity: a usable kit has both surfaces to stand on and verticality.
        let reg = registry();
        assert!(reg.by_role(AssetRole::Platform).count() >= 5, "kit needs walkables");
        assert!(reg.by_role(AssetRole::Pillar).count() >= 1, "kit needs verticality");
    }
}
