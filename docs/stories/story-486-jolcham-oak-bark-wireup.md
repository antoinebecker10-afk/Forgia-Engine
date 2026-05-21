# Story-486 — Jolcham Oak Bark Wire-up (material_override trunk)

**Status** : IN PROGRESS
**Type** : Standard
**Crate** : `forgia-foliage`
**Date** : 2026-05-21

---

## Intent

Wire-up la texture Jolcham Oak Bark (CC0 Poly Haven, déposée en reserve
`assets/textures/pbr/jolcham_oak_bark_01/`) comme override material sur les
primitives trunk de tous les arbres (`AssetCategory::Tree`) spawnes dans
le monde RPG.

Origine : MEMORY.md `reference_jolcham_oak_bark_asset_reserve.md`.

---

## Scope

- **Crate cible** : `forgia-foliage` (NEW file `material_override.rs`)
- **Activation** : boot ON via `BarkOverrideConfig::default()` (custom impl)
- **Heuristique trunk** : nom primitive regex `bark|trunk|wood|stem` case-insensitive
- **Fallback** : dernier enfant par triangle count apres N frames polling
- **ARM mapping** : meme Handle sur `occlusion_texture` + `metallic_roughness_texture`
- **Genome TOML** : hors scope phase 1
- **Health alert** : hors scope phase 1

**Hors scope** :
- Split ARM diff/roughness en passes separees
- Toggle UI in-game
- Validation runtime sensor

---

## Acceptance Criteria

- [x] `BarkOverrideConfig`, `BarkTextures`, `NeedsTrunkOverride` publics depuis crate
- [x] `is_trunk_primitive_name` correctement filtre bark/trunk/wood/stem, rejette canopy/leaves
- [x] Tests purs : >= 4 tests `is_trunk_primitive_name` sans App Bevy
- [x] `cargo check -p forgia-foliage` : 0 erreurs
- [x] `cargo clippy -p forgia-foliage --no-deps` : 0 warnings
- [x] Sensor `forgia_vegetation.json` enrichi : `trunk_override.{enabled, overridden, fallback, not_found}`
- [x] Boot ON automatique (Default impl custom `enabled: true`)

---

## Risques

- BFS multi-frame : les enfants GLB peuvent ne pas etre disponibles au frame 0
  (async scene loading). Le polling NeedsTrunkOverride.frames_polled mitigue ca.
- Nommage primitives KayKit : inconnu sans runtime test. Fallback triangle count
  assure la couverture si nom non-match.
- Lock L1 : aucun nouveau Handle<Image> ajoute dans assets.rs — OK car BarkTextures
  est insere au Startup via system dedie (pattern hors whitelist acceptable :
  textures PBR vegetation, pas des assets game critiques).

---

## Plan execute

Phase A : Squelette `material_override.rs` + types publics
Phase B : Logique systemes (preload + marker + BFS override)
Phase C : Extension sensor + tests + boot ON

---

## Validation runtime

Apres commit : lancer RPG, lire `forgia_vegetation.json` -> voir
`trunk_override.overridden > 0` apres ~30 frames.
