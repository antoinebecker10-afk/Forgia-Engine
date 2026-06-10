# Story-594 — M2 session 1 : feel data-driven, anti-hitch, tests combat, KTX2 armes

> **Source** : [roadmap post-audit](../ROADMAP_POST_AUDIT_2026-06-10.md) jalon M2,
> items B3/B4/B5/B7 (piste B — UX & tech plancher Steam).
> **Scale BMAD** : Standard. **Date** : 2026-06-10. **Statut** : CODE-COMPLETE — validation runtime requise.

## Critères d'acceptance

| # | AC (item roadmap) | Statut | Preuve |
|---|---|---|---|
| AC1 (B5) | 8 dummies hanabi pré-spawnés au boot (anti-freeze 1er tir) | ✅ code | `prespawn_hanabi_dummies` réel (PostStartup, Visibility::Hidden, Y=-10000, despawn 5 s via lifetime_tick). Était « TODO Phase 2 » depuis Phase 0 |
| AC2 (B4) | speed/jump/gravity/turn en genome hot-reload | ✅ code | `PlayerMovementTuning` (pattern HitFeedbackTuning) + `assets/genomes/player_movement.toml` ; défauts = miroir exact des littéraux (test `movement_tuning_defaults_mirror_pre_genome_literals`) |
| AC3 (B4) | Chaîne player dans GameSet::Movement (Lock L7) | ✅ code | `.in_set(GameSet::Movement)` sur la chaîne mouse_look→movement→dash ; ordre Movement→Camera garantit enfin l'hypothèse de l'aim assist |
| AC4 (B7) | apply_damage couvert | ✅ | 5 tests headless (soustraction, guard clampé 0..1, kill→DeathEvent unique, cible morte ignorée, cumul même frame) — 8/8 verts |
| AC5 (B7) | genome-core couvert | ✅ | `parse_genome` extrait pur + 6 tests (TOML invalide = Err pas panic, défauts serde, champ requis manquant, mauvais type) |
| AC6 (B3) | 4 armes en KTX2/UASTC | ✅ code / ⏳ runtime | gltf-transform uastc --zstd 0 (recette barks story-588 : scheme=0 vérifié par lecture header). gpuSize/texture : 21,28→5,59 MB (÷3,8). VRAM armes attendue ~340→~89 MB |

## Reports documentés

- **B7 loot_room** : extraction des fonctions pures de loot_room.rs (868 LOC, 0 test)
  différée — refactor substantiel dans une crate partiellement claimée multi-terminal.
- **A4 économie** : volontairement non fait à l'aveugle (no-speculative-fix) — le
  recalibrage boons/souls se fait EN JOUANT avec forgia2_boons/roguelite_state.json.
- **B1/B2/B6/B8, A1-A3/A5-A8** : sessions M2 suivantes.

## Notes techniques

- **🚨 Cause racine de l'instabilité cargo test ENFIN attrapée** : `Allocation failed`
  + rustc tué (`STATUS_STACK_BUFFER_OVERRUN`) en compilant bevy_asset pendant que
  l'autre terminal buildait = **OOM machine** → artefacts corrompus → cascades E0463
  aléatoires. Parade : `-j 4` + retry (110/110 puis 7/7 verts au 2e essai).
- **KTX2 armes** : textures EMBARQUÉES dans les .glb (≠ barks standalone) →
  `gltf-transform uastc` (npm, présent) qui exige le CLI `ktx` — **KTX-Software 4.4.2
  installé silencieusement dans `C:\Users\Antoi\Tools\KTX`** (pas de winget, NSIS /S,
  sans admin). Disque +29 % (UASTC sans zstd = seule recette validée bevy runtime) —
  le gain est en VRAM, pas sur disque. `extensionsRequired: KHR_texture_basisu` :
  si bevy refuse au chargement → armes invisibles → rollback `git checkout HEAD~ --
  assets/models/weapons/forgia/` + forgia2_assets.json le détecterait (scene_failed).
- asset-load rebaseliné : +1 call-site légitime (player_movement.toml).

## Test in-game (récap obligatoire)

1. **Action** : rebuild (`cargo run --profile release-fast`) → lancer une run Roguelite,
   tirer immédiatement au spawn, cycler les 4 armes.
2. **Effets attendus** : (a) AUCUN hitch au premier tir (vs micro-freeze avant) ;
   (b) les 4 armes ont leurs textures normales (pas de magenta/blanc = échec KTX2) ;
   (c) le feel de déplacement est INCHANGÉ (défauts = miroir).
3. **Sensors** : `forgia2_vram.json` → top_images : les textures d'armes passent de
   21,28 MB à ~5,6 MB pièce, total images ~1210→~960 MB ; `forgia2_lag_events.json` →
   pas de spike au 1er tir ; `forgia2_assets.json` → scene_failed=0.
4. **Hot-reload feel** : éditer `assets/genomes/player_movement.toml` (ex. speed 5→7),
   sauvegarder → le changement s'applique EN JEU sans rebuild (file_watcher).
5. **Variantes si KO** : armes magenta → rollback git des 4 GLB + signaler (feature
   basisu) ; feel changé → vérifier que le TOML n'a pas été édité (défauts=miroir) ;
   hitch persiste au 1er tir → vérifier log `[forgia-effects] prespawn 8 hanabi dummies`.

## Fichiers touchés

crates/forgia-effects/src/lib.rs · crates/forgia-player/src/lib.rs ·
assets/genomes/player_movement.toml · crates/forgia-damage/src/lib.rs ·
crates/forgia-genome-core/src/lib.rs · assets/models/weapons/forgia/*.glb (4) ·
xtask/asset-load-allowlist.toml
