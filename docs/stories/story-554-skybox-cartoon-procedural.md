# Story-554 — Skybox cartoon procedural (Phase 1 : Rust generated cubemap)

> **Status** : IN PROGRESS
> **Scale BMAD** : Quick (≤3 fichiers)
> **Effort estimé** : ~1h Phase 1 (hardcoded colors), ~30min Phase 2 (TOML data-driven)
> **Roadmap ref** : C1 — exploration cartoon HDR skybox (vs Cult of the Lamb / Death's Door)

## Pourquoi

Story-553 a wiré sky_skybox.ktx2 HDR mais le style trop réaliste perd l'identité cartoon Forgia Roguelite (bible v1 Cult of the Lamb canon réf). PolyHaven/HDR photographés ne matchent pas, Blockade Labs payant. Le pattern AAA cartoon (Cult of the Lamb, Death's Door, Hadès) = gradient 2-3 couleurs flat → trivial à générer en code.

## Architecture (Phase 1)

- Au Startup : `generate_cartoon_skybox()` crée `Image` 256×256×6 (RGBA8 sRGB), peint gradient zénith→horizon par face en code Rust
- Insère directement dans `Assets<Image>`, stocke handle dans `SkyboxPending`
- `attach_skybox_to_camera` (inchangé story-553) attache comme Skybox normal
- **HDR-compat** : sRGB sampling auto-linear par GPU → Bloom/HDR camera fonctionne

## Acceptance Criteria Phase 1

- [ ] AC1 — Fonction pure `generate_cartoon_skybox() -> Image` dans forgia-player (couleurs hardcoded Crypts palette)
- [ ] AC2 — `load_skybox` n'utilise plus `asset_server.load(KTX2)` mais `Assets<Image>::add(generate_cartoon_skybox())`
- [ ] AC3 — Couleurs Crypts cartoon : zénith deep violet `#3D2466`, horizon warm orange `#FF6B35`, ground dark mauve `#2A1B3D`
- [ ] AC4 — `cargo check -p forgia-player` 0 erreur
- [ ] AC5 — `cargo clippy -p forgia-player --no-deps` 0 warning

## Phase 2 (follow-up)

- TOML genome `config/genomes/biome_sky.toml` : palette per-biome (Crypts/Forge/...)
- Hot-reload Shift+F12
- Optionnel : sun disk, cloud noise overlay procédural

## Files Phase 1

- `crates/forgia-player/src/lib.rs` — `generate_cartoon_skybox` + swap dans `load_skybox`

## Test in-game

1. **Action** : `cargo run --profile release-fast -p forgia-game` puis spawn jeu
2. **Redémarrage requis** (changement asset)
3. **Effet attendu** : ciel cartoon gradient violet (haut) → orange (horizon), face sol mauve sombre. Look "Cult of the Lamb" simple flat
4. **Sensor** : visuel uniquement + log `[forgia-player] Skybox HDR attached`
5. **Variantes si KO** :
   - Cubemap inversé (gradient haut→bas inversé) → inverser t = 1.0 - y/h dans la lerp
   - Couleurs ternes → palette à durcir (saturation +)
   - Aspect "tilted" → tweak orientation des faces side via face indexing

## Cross-refs

- Story-553 (skybox HDR KTX2 — remplacé par procedural)
- [feedback_hdr_pipeline_needs_hdr_skybox_first.md](../../../memory/feedback_hdr_pipeline_needs_hdr_skybox_first.md)
- Bible v1 Crypts of Anvil palette (violet/orange/mauve)
- [Bevy 0.18 release notes](https://bevy.org/news/bevy-0-18/)
