# story-664 — Warmup des pipelines PBR au Lobby (anti-freeze « tourner la caméra »)

**Statut** : 🟡 IN_PROGRESS (code livré + auto-QA + clippy/tests verts ; attend la **mesure runtime** user avant DONE)
**Type** : BMAD Standard (2 crates, 6 fichiers)
**Créée** : 2026-07-20
**Related** : story-627 (shader-prewarm viewmodel+éléments), story-663 (floor-merge), audit `docs/audit/audit-2026-07-20-fire-path-perf.md`

## Problème

Freezes runtime de **45-146 ms quand le joueur tourne la caméra** (confirmé par l'user + capteur `forgia2_load_timing.json` : freezes à charge plate `entity_delta:0`, cause fallback `gpu_or_shader_compile` ; profil burst-puis-calme). Diagnostic confirmé par un spécialiste Bevy : **compilation de pipeline de rendu (specialization) au 1er affichage** d'une variante mesh+`StandardMaterial` de décor/ennemi entrant dans le frustum. Le repo warme déjà les VFX Hanabi mais **aucun** des matériaux PBR.

## Piège Bevy 0.18.1 (central)

Le pattern de warmup « entité cachée à Y=-10000 + `Visibility::Hidden` » (dummies Hanabi) **NE compile PAS un pipeline PBR** : `queue_material_meshes` n'itère que sur les entités réellement dans le frustum (`ViewVisibility==true`). Il faut que l'objet soit **rendu** : soit dans le frustum réel, soit `NoFrustumCulling` + `Visibility` non-Hidden. Cf memory `reference_pbr_pipeline_warmup_frustum_trap`.

## Décision de design : warmup UNIQUE par session, au Lobby

`PipelineCache` est global/persistant + toutes les salles partagent le même catalogue (`DecorAssets` préchargé au Startup, 3 GLB squelettes). Compiler un représentant de chaque **une fois** au Lobby couvre toute la session. Le Lobby est le point le plus tôt avec une `Camera3d` réelle (elle n'existe qu'en jeu).

## Livraison

- **`crates/forgia-effects/src/pipeline_ready.rs`** (NOUVEAU) : `PipelinesReady` (resource) + `PipelinesReadyPlugin` — branche `ExtractSchedule` du `RenderApp` sur `PipelineCache::waiting_pipelines().count()==0`. Pattern exact de l'exemple officiel `loading_screen.rs`. Ajouté dans `ForgiaEffectsPlugin`.
- **`crates/forgia-mode-roguelite/src/pipeline_warmup.rs`** (NOUVEAU) : `PipelineWarmupPlugin`. `OnEnter(RunState::Lobby)` → spawne hors-champ (Y=-500), `Visible`, un `SceneRoot` par GLB de `DecorAssets` (8 groupes) + 3 squelettes distincts ; observer `on_warmup_scene_ready` pose `NoFrustumCulling` sur les meshes descendants (→ rendus/compilés sans être vus). `sys_tick_warmup` despawn quand `PipelinesReady && frames>=90` OU plafond 900 frames (anti-lock). `sys_clear_warmup` (OnExit) filet de sécurité. `WarmupState.done` session-once. Sensor `forgia2_pipeline_warmup.json`.
- Câblage : `lib.rs` (forgia-effects + forgia-mode-roguelite).

## Critères d'acceptation

- [x] `cargo check` + `clippy -D warnings` verts (forgia-effects, forgia-mode-roguelite)
- [x] Tests unitaires verts (dédup squelettes, min<max frames, PipelinesReady default false)
- [x] Auto-QA (verifier PASS + qa-lead : 2 Majeurs + 1 Mineur + 1 Cosmétique **corrigés**)
- [ ] **Mesure runtime user** : `forgia2_load_timing.json` — chute des freezes `gpu_or_shader_compile` en tournant la caméra en combat ; `forgia2_pipeline_warmup.json::done==true` après le Lobby
- [ ] Pas d'artefact visuel au Lobby (diorama hors-champ)

## Corrections auto-QA (post-livraison initiale)

- **#1 Majeur — sol + portails non warmés** : ajout du **sol** (`forgia_stage::MERGED_FLOOR_GLB`, exposé en source unique) + **portails de boss** (`boss_portal::PORTAL_*_GLB` en `pub(crate)`) au diorama. Le sol est vu à chaque combat dès la 1re frame → c'était le résidu le plus grave.
- **#2 Majeur — race de despawn prématuré** : `PipelinesReady` pouvait valoir `true` par *absence de travail* (scène lourde pas encore chargée). Ajout d'un compteur `scenes_ready` (incrémenté sur `SceneInstanceReady`) ; le despawn n'ouvre que quand `scenes_ready == classes_spawned` (toutes instanciées) + `PipelinesReady` + délai min. Test `despawn_gate_requires_all_scenes_ready`.
- **#3 Mineur** : `DespawnOnExit(GameMode::Roguelite)` ajouté au diorama (filet, cohérence `decor_markers`).
- **#4 Cosmétique** : commentaire corrigé (world-space + `NoFrustumCulling`, pas parenté caméra).

## Limites connues restantes (story de suivi si la mesure le montre)

- Sol **fusionné** (`floor_merge`) : on warme les GLB sources ; si le mesh fusionné a un layout de vertex différent, son pipeline pourrait rester non-couvert → à confirmer au runtime.
- `StandardMaterial` procéduraux des salles spéciales (mushrooms/stations) + VFX en `unlit+Blend` (status_vfx/shockwave, famille de pipeline disjointe) NON couverts. Les 4 mats d'élément le sont déjà via le prewarm `weapon_select`.
- Efficacité empirique : le gain réel se valide au runtime (mesure user).
