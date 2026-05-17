# Forgia Rewrite — Session State 2026-05-15

> Snapshot fin de journée pour reprise demain.

## ⏩ Suite session marathon soir 2026-05-14 (mon territoire FPS)

**5 commits master V2 livrés** depuis snapshot du matin :

```
a0b1c70  feat(sky): port V1 Skybox cubemap (sky_129_stacked.png day variant)
0adaf82  feat(sky): nuages orbit circulaire continu (jamais wrap brusque)
ea5b6c7  feat(map): vraie arène FPS 44×44m multi-zones + 25 assets KayKit + 3 weapons
c83ad7a  feat(phase-2.3): muzzle flash + impact VFX + tracer fix + hitmarker
32e73f4  feat: V2 bootstrap + Phase 1 Hello World + Phase 2.1 hitscan minimum
```

**Phase 2 V2 finalisée côté FPS** :
- Arena KayKit 44×44m (121 floor + 44 walls + 12 piliers + 14 covers + 5 banners + 8 torches + 5 cubes)
- Skybox cubemap V1 (sky_129) wired via SkyboxPending + reinterpret_stacked_2d_as_array(6)
- 18 nuages popcorn orbital (CloudOrbit { angle, radius, height }) — jamais wrap brusque
- Hitscan complet : muzzle 4-layer + tracer 5cm + impact VFX + hitmarker + flash + hit-stop + cooldown
- Camera recoil DÉSACTIVÉ (design choice Antoine, style Valorant)
- 3 cubes target → 5 cubes dispersés map entière

**Atmosphere Hillaire 2020 backlog Phase 5** : tentative wired = écran noir (manque Bloom + Exposure + AutoExposure + AtmosphereSettings + ordre init). Fallback Skybox cubemap V1 stable. Voir `feedback_v2_atmosphere_complex_setup_backlog` mémoire.

**Anti-traps Bevy 0.18 documentés** (mémoire `reference_bevy_018_renames_v2`) : Hdr Component marker, CursorOptions séparé, MessageReader<MouseButtonInput> (pas Res<ButtonInput> consumed by egui), ClearColorConfig::None sur MenuCamera2d, exclude Player du raycast Rapier (FpsCamera child).

**Bug majeur fix** : ForgiaDialoguePlugin orphan oublié dans forgia-game/lib.rs → panic Resource manquante. **Wired ligne 41**.

## Suite suggérée demain (FPS Phase 2.5+)

| Option | Effort |
|---|---|
| Damage numbers flottants V1 V5-F | 30 min |
| Viewmodel 1P (ak47.glb dispo dans assets) | 45 min |
| Multi-armes Digit1/2/3 (3 weapons assets prêts) | 1h |
| Bot IA arena (1 ennemi qui tire) | 2h |
| Sensor stack V2 (forgia2_health/perf/lifecycle) | 2h |
| Atmosphere debug profond (Bloom + Exposure setup complet) | 1-2h |

## Status quo (ne change pas)

## Ce qui marche

- ✅ Workspace **237 crates**, `cargo check --workspace` 0 erreurs
- ✅ `cargo run` (depuis racine) lance le jeu, Pattern A Renzora-style
- ✅ Arena V1 FPS jouable end-to-end (partenaire) — boucle Menu→Combat→Death→Cleanup
- ✅ RPG world spawné OnEnter(GameMode::Rpg) : sol procédural multi-octave + biome colors + texture grass V1 réelle, 2 buildings + 5 NPCs typés
- ✅ Interaction touche E : raycast → InteractablePoint → StartDialogue
- ✅ `forgia-ui-dialogue` egui modal (lit DialogueSession, render speaker/line/choix, MessageWriter ChooseDialogueOption)
- ✅ 2 sample DialogueTrees registered (Aldric quête mines + Lyra commerce)
- ✅ 10/10 tests RPG passent

## Crates implémentées cette session (~32)

Mon territoire (n'overlap pas avec partenaire) :

**RPG core** :
- forgia-rpg (multi-octave heightmap + biome colors + V1 texture + interactions E)
- forgia-inventory (LOCK-INV-1 80 slots, 4 tests)
- forgia-quests (QuestDef/State/Catalogue, 2 tests)
- forgia-dialogue (DialogueTree + sessions + effects, 1 test)
- forgia-xp-curves (Linear/Exponential curves, 3 tests)
- forgia-ui-dialogue (egui panel)

**Combat atomic** :
- forgia-damage (Health + DamageEvent + DeathEvent)
- forgia-weapon-hitscan (rapier raycast + falloff + cooldown)
- forgia-ai-arena-bot (Idle/Chase/Attack + respawn queue)
- forgia-juice-camera-shake (trauma-based + hash_noise)
- forgia-vfx-tracers (cached mesh anti-freeze)
- forgia-damage-numbers (Text2d billboards)

**44 post-process** : forgia-pp-toon, forgia-pp-outline (impl complet FullscreenMaterialPlugin), 42 autres wirés template

**6 ports Renzora** : forgia-mod-outline, forgia-gauge, forgia-silk (stub - bevy_silk 0.10=Bevy 0.16), forgia-navigator, forgia-oxr, forgia-websocket (tungstenite-based custom)

**5 fondations** : forgia-app-state (alias forgia-core), forgia-system-sets (alias), forgia-time, forgia-genome-core, forgia-manifest

Territoire partenaire (n'a pas overlap) : forgia-core, forgia-input, forgia-player, forgia-fps, forgia-ui, forgia-game/lib.rs.

## En cours / à reprendre demain

### 1. Port forgia-terrain (CRITIQUE pour M2 streaming)

État actuel :
- **9 fichiers principaux + 8 sub-files portés** sur disque (~2600 LOC / 10000 V1)
- lib.rs `pub mod` commentés → plugin no-op
- forgia-rpg fournit heightmap inline en attendant

**Manquants pour activer compile graph** :
- `meshing.rs` (43K, surface-nets — CRITIQUE)
- `terrain_material.rs` (17K, shader splat)
- `grass_material.rs` (9K)
- `modular.rs` (24K)

**Stubs à compléter** (types référencés mais champs manquants) :
- WorldMapIntent.seed/landmarks
- VillageNetwork.villages
- ChunkPipelineDiag.chunk/detail_level
- CaveNetworkTopology + carve_network_tunnels

### 2. Wire forgia-ui-dialogue + forgia-dialogue dans forgia-game/lib.rs

**Territoire partenaire** — proposer plutôt que faire directement. Ajout requis ligne ~38 :
```rust
forgia_dialogue::ForgiaDialoguePlugin,
forgia_ui_dialogue::prelude::ForgiaUiDialoguePlugin,
```

### 3. Wire les 6 crates combat atomiques

forgia-damage / forgia-weapon-hitscan / forgia-ai-arena-bot / forgia-juice-camera-shake / forgia-vfx-tracers / forgia-damage-numbers ne sont pas dans main.rs. Le partenaire a sa propre logique inline dans forgia-fps. À discuter : remplacer ou cohabiter.

### 4. UI inventory (forgia-ui-inventory)

Crate scaffoldée stub. Implémentation à faire : egui grid 80 slots paginé.

## Décision à prendre

Port forgia-terrain à 100% (90+ min meshing.rs + terrain_material.rs) OU rester sur heightmap inline forgia-rpg jusqu'à M2 ?

**Ma reco** : rester sur heightmap inline (déjà visuellement correct + texture PBR + biome colors). Porter forgia-terrain quand on aura besoin du streaming chunks.

## Fichiers clés à lire en reprise

- `crates/forgia-rpg/src/lib.rs` — état RPG playable
- `crates/forgia-ui-dialogue/src/lib.rs` — panel egui
- `crates/forgia-terrain/src/lib.rs` — état port (commentaires détaillés sur manquants)
- `Cargo.toml` racine — Pattern A workspace
- `MEMORY.md` (memory dir) — session_2026_05_14_forgia_rewrite_marathon

## Sensors / logs

- `forgia2_run.log` : dernière session 18:52-18:53 — RPG pas testé encore avec terrain texturé V1
- `forgia_arena_feedback.json` : 0 sounds (audio non wiré, normal)

## Commandes utiles

```bash
cargo run                              # lance le jeu (Pattern A)
cargo run -p forgia-game               # backward compat partenaire
run_debug.bat                          # Windows + log capture + backtrace
cargo check --workspace                # verify
cargo test -p forgia-rpg --tests       # 3 tests passent
```
