# Story-555 — Skybox cartoon palette TOML data-driven per-biome (Phase 2 de 554)

> **Status** : IN PROGRESS (impl, awaiting runtime test)
> **Scale BMAD** : Standard
> **Effort** : ~1h (impl) + test runtime
> **Plan** : `C:\Users\Antoi\.claude\plans\a-lovely-feigenbaum.md`
> **Prérequis** : story-554 Phase 1 (commit `c15fa6a`)

## Pourquoi

Phase 1 hardcodait 3 couleurs Crypts dans Rust. Phase 2 = pattern AAA (Hadès / Cult of the Lamb) : asset TOML dédié + auto-switch per biome via `StageLoadResult` + hot-reload Shift+F12. Pas d'extension de BiomeSpec terrain (anti-pattern coupling).

## Architecture

```
assets/genomes/biome_sky.toml
    ↓ GenomeLoader<SkyPaletteGenome> (forgia-genome-core pattern)
SkyPaletteGenome (HashMap<biome_id, SkyPalette>)
    ↓ AssetEvent::Added/Modified/Loaded
sync_palette_from_genome → CurrentSkyPalette
    ↑                              ↓ is_changed
track_stage_biome ←──── Res<StageLoadResult>.biome
                                   ↓
                       regen_skybox_image
                                   ↓
                       generate_cartoon_skybox(&palette)
                                   ↓
                       Assets<Image>::add(new_image)
                                   ↓
                       commands.entity(cam).insert(Skybox{new_handle})
```

## Acceptance Criteria

- [x] AC1 — `assets/genomes/biome_sky.toml` créé : 1 `[default]` + 10 biomes (Plains/Forest/Desert/Mountain/Swamp/Tundra/Savanna/Jungle/Volcanic/Canyon)
- [x] AC2 — `SkyPalette` + `SkyPaletteGenome` + `CurrentSkyPalette` + `SkyPaletteGenomeHandle` + `SkyboxGenomePlugin` dans `forgia-player::skybox_genome`
- [x] AC3 — `generate_cartoon_skybox()` refactoré → `(&SkyPalette)`
- [x] AC4 — Module wiré dans `ForgiaPlayerPlugin` via `add_plugins(SkyboxGenomePlugin)`
- [x] AC5 — `Cargo.toml` : ajout `forgia-genome-core`, `forgia-stage`, `serde`
- [ ] AC6 — `cargo check -p forgia-player` 0 erreur
- [ ] AC7 — `cargo clippy -p forgia-player --no-deps` 0 warning
- [ ] AC8 — Runtime : ciel identique à Phase 1 sans Roguelite (palette default)
- [ ] AC9 — Runtime : Shift+F12 sur edit TOML → palette change live <1s
- [ ] AC10 — Runtime : changement stage Roguelite → palette change auto

## Files

- NEW `assets/genomes/biome_sky.toml` (62 lignes)
- NEW `crates/forgia-player/src/skybox_genome.rs` (~180 lignes)
- MODIFY `crates/forgia-player/src/lib.rs` (refactor + wire ~12 lignes)
- MODIFY `crates/forgia-player/Cargo.toml` (+3 deps)

## Pattern réutilisé

- `forgia-killfeed::tuning::sync_killfeed_tuning` ([forgia-killfeed/src/tuning.rs:107-142](../../crates/forgia-killfeed/src/tuning.rs#L107-L142)) — MessageReader AssetEvent listener pattern canonique Forgia.
- `forgia-genome-core::GenomeLoader<T>` — TOML loader générique, registered via `app.register_genome::<T>()`.

## Crates safety (multi-terminal)

- ✅ forgia-player, forgia-stage : NOT locked
- ❌ forgia-mode-roguelite, forgia-asset-registry, forgia-core : LOCKED → PAS touchés

## Test in-game

1. **Action** : `rtk cargo run --profile release-fast -p forgia-game` puis spawn jeu
2. **Redémarrage requis**
3. **Effet attendu Test 1 (no regression)** : ciel violet→orange Crypts identique à Phase 1
4. **Effet attendu Test 2 (hot-reload)** : éditer `biome_sky.toml [default].horizon_rgb = [255,50,50]` + save + Shift+F12 in-game → horizon rouge en <1s
5. **Effet attendu Test 3 (per-stage)** : si stage change biome (e.g. crypts→ailleurs), palette switch auto
6. **Sensor** : logs `[forgia-player] Skybox palette loaded` / `Palette switched to biome '<X>'` / `Skybox regenerated`
7. **Variantes si KO** :
   - Pas de log "palette loaded" → asset path wrong ou loader pas registered
   - "applied 'default'" toujours → `StageLoadResult.biome` vide, vérifier sensor `forgia2_stage.json`
   - Crash deserialize → struct serde mismatch TOML (verify les 10 sections + types `[u8;3]`)
   - Cubemap reste l'ancien → vérifier `Camera3d` Query non vide quand regen tire

## Follow-ups potentiels (Phase 3+)

- Sun disk + cloud noise overlay procédural (étape 2 gradient)
- Per-biome ambient light color (sync palette → AmbientLight resource)
- Per-biome ClearColor pour bordures HDR
- Migration vers TOML par biome (un fichier par biome au lieu d'un global)

## Cross-refs

- Plan : `C:\Users\Antoi\.claude\plans\a-lovely-feigenbaum.md`
- Story-554 Phase 1 (`c15fa6a`)
- [feedback_hdr_pipeline_needs_hdr_skybox_first.md](../../../memory/feedback_hdr_pipeline_needs_hdr_skybox_first.md)
