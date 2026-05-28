# Story-550 — HDR + Bloom + Tonemapping sur RPG OrbitCamera

> **Status** : DONE
> **Scale BMAD** : Quick (≤3 fichiers)
> **Effort estimé** : ~10 minutes
> **Prérequis** : story-549 (FPS camera HDR foundation)

## Pourquoi

Story-549 a wiré HDR+Bloom+TonyMcMapface sur FpsCamera. Sans le même pipeline
sur RpgOrbitCamera (mode 3P Rex), passer Roguelite → RPG perd la cohérence
visuelle : highlights écrasés, pas de bloom, tonemapping default (AcesFitted).

## Acceptance Criteria

- [x] AC1 — `RpgOrbitCamera` spawn inclut `Bloom::NATURAL`
- [x] AC2 — `Tonemapping::TonyMcMapface` attaché à RpgOrbitCamera
- [x] AC3 — `cargo check -p forgia-rpg` 0 erreur (43 crates compiled)
- [x] AC4 — `cargo clippy -p forgia-rpg --no-deps` 0 warning

## Files

- `crates/forgia-rpg/src/character.rs` — imports + spawn tuple (lignes ~26-28 + ~174-186)

## Test in-game

1. **Action** : `cargo run --profile release-fast -p forgia-game` puis bascule Roguelite → RPG (selon binding actuel)
2. **Redémarrage requis** (changement Component camera)
3. **Effet attendu** : en mode RPG (caméra 3P sur Rex), highlights (skybox, lampes) émettent halo doux. Bloom + emissive brazier (story-551) visibles si arena Crypts chargée.
4. **Sensor** : visuel uniquement
5. **Variantes si KO** : identiques story-549 (`Bloom::NATURAL` → `OLD_SCHOOL`, prefilter threshold)

## Cross-refs

- Story-549 (FPS HDR foundation)
- Story-551 (Brazier emissive — payoff visible aussi en mode RPG)
