# Story-549 — HDR + Bloom + TonyMcMapface (Tier 2 quick win)

> **Status** : IN PROGRESS
> **Scale BMAD** : Quick (≤3 fichiers)
> **Effort estimé** : ~half-day
> **Roadmap ref** : [ROADMAP_ROGUELITE Tier 2 #4](../ROADMAP_ROGUELITE.md#tier-2--polish-visuel-quick-wins-parallèles)
> **Parent gap** : Gap #3 industry research — 0 emissive, 0 bloom → signature Cult of the Lamb absente

## Pourquoi

Tier 2 quick-win identifié dans ROADMAP_ROGUELITE.md : **Camera3d hdr=true + Bloom 0.25 + TonyMcMapface**. Coût ~1j, impact ★★★★. Foundation requise pour story-538 (emissive Brazier + mushroom emissive cyan) — sans HDR pipeline, les materials emissive saturent à RGB(1,1,1) au lieu de bloomer.

## Scope minimal (Quick)

Ce ticket ne couvre **que** la FpsCamera. RPG OrbitCamera + materials emissive Brazier = follow-ups (550, 551).

## Acceptance Criteria

- [ ] AC1 — `FpsCamera` spawn inclut `Camera { hdr: true, ..default() }`
- [ ] AC2 — `Tonemapping::TonyMcMapface` attaché à FpsCamera
- [ ] AC3 — `Bloom::NATURAL` (ou settings explicites intensity ~0.15) attaché à FpsCamera
- [ ] AC4 — `cargo check -p forgia-player` 0 erreur
- [ ] AC5 — `cargo clippy -p forgia-player --no-deps` 0 warning sur fichiers touchés

## Files

- `crates/forgia-player/src/lib.rs` (FpsCamera spawn, lignes ~167-179)

## Test in-game (post-merge other terminal)

1. **Action** : `cargo run --profile release-fast -p forgia-game` puis spawn jeu
2. **Redémarrage requis** (changement Component camera)
3. **Effet attendu** : highlights (lampes, ciel, skybox soleil) blooment visuellement (halo doux), couleurs plus contrastées via TonyMcMapface (vs AcesFitted default)
4. **Sensor** : visuel uniquement (pas de sensor dédié). Comparaison screenshot avant/après.
5. **Variantes si KO** :
   - Bloom invisible → augmenter `intensity` 0.15 → 0.3
   - Tout cramé → revert hdr=true ou réduire prefilter threshold
   - Crash render graph → vérifier feature `bevy/tonemapping_luts` activée

## Follow-ups identifiés

- **story-550** : Étendre HDR+Bloom+Tonemapping à RPG OrbitCamera (camera spawn site distinct, à localiser)
- **story-551** : Material emissive Brazier (observer SceneInstance ready → StandardMaterial.emissive = LinearRgba(orange × 5.0)) + champignons cyan emissive cluster
- **story-552** : Skybox HDR PolyHaven volcanique (remplace `sky_129_stacked.png` actuel)

## Cross-refs

- [ROADMAP_ROGUELITE.md Tier 2 #4](../ROADMAP_ROGUELITE.md)
- [reference_industry_3_gaps_forgia_roguelite.md](../../../memory/reference_industry_3_gaps_forgia_roguelite.md) (Cult of the Lamb signature visuelle)
