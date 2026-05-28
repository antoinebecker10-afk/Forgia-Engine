# Story-553 — Skybox HDR KTX2 (foundation re-add HDR pipeline)

> **Status** : IN PROGRESS
> **Scale BMAD** : Quick (≤3 fichiers)
> **Effort estimé** : ~30 min
> **Roadmap ref** : Tier 2 #5 + unblocker pour re-add stories 549/550/551 (revert'd 2026-05-28)

## Pourquoi

Les commits `1001ee5`/`994f588`/`a3bb409` (HDR+Bloom+Brazier) reverté'd `be25bbc`/`6cb1530`/`7d8d1dc` car skybox LDR `sky_129_stacked.png` rendait l'écran noir total sous pipeline HDR.

Asset `assets/hdri/env-maps-v1/sky_skybox.ktx2` (HDR cubemap Bevy-native, 512KB) déjà déposé V1 mais **jamais wiré V2**. Le swap unlock le re-add propre de 549/550/551.

## Acceptance Criteria

- [ ] AC1 — `SKYBOX_PATH` swap `"hdri/sky_129_stacked.png"` → `"hdri/env-maps-v1/sky_skybox.ktx2"`
- [ ] AC2 — `attach_skybox_to_camera` : retirer `reinterpret_stacked_2d_as_array` + `TextureViewDescriptor` (KTX2 cubemap déjà natif)
- [ ] AC3 — `SKYBOX_BRIGHTNESS` ajusté pour HDR (500.0 au lieu de 1000.0 LDR — tunable)
- [ ] AC4 — `ImageLoaderSettings` : retirer `sampler = ImageSampler::linear()` (KTX2 sampler par défaut OK)
- [ ] AC5 — `cargo check -p forgia-player` 0 erreur
- [ ] AC6 — `cargo clippy -p forgia-player --no-deps` 0 warning
- [ ] AC7 — Test runtime : ciel visible, pas d'écran noir (sans HDR/Bloom activé — baseline pre-HDR)

## Files

- `crates/forgia-player/src/lib.rs` — SKYBOX_PATH + attach_skybox_to_camera (~ligne 55 + 197-242)

## Test in-game

1. **Action** : `cargo run --profile release-fast -p forgia-game` puis spawn jeu
2. **Redémarrage requis** (changement asset)
3. **Effet attendu** : ciel visible avec apparence HDR (gradient ou paysage), PAS noir, PAS cramé blanc
4. **Sensor** : visuel uniquement. Si log `[forgia-player] Skybox attached to N Camera3d(s)` apparaît → wire OK
5. **Variantes si KO** :
   - Écran noir → KTX2 mal formé, fallback PNG : revert + chercher autre KTX2
   - Ciel cramé blanc → bump down brightness 500 → 100
   - Crash load → vérifier feature `ktx2` Bevy (confirmé activée Cargo.toml workspace)
   - Cubemap inversé → ajouter `Skybox::rotation` Quat

## Follow-ups (re-add HDR pipeline)

Si AC7 PASS → ré-attaquer dans l'ordre :
- **story-549 v2** : HDR + Bloom + TonyMcMapface FpsCamera
- **story-550 v2** : same pour RpgOrbitCamera
- **story-551 v2** : Brazier emissive auto-detect

## Cross-refs

- [feedback_hdr_pipeline_needs_hdr_skybox_first.md](../../../memory/feedback_hdr_pipeline_needs_hdr_skybox_first.md)
- Commits revert : `be25bbc`, `6cb1530`, `7d8d1dc`
