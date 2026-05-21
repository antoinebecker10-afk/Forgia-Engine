# Audit complet — Modes RPG & Roguelite — 2026-05-21

> Audit Enterprise read-only. Workspace V2 = `C:/Users/Antoi/Desktop/Forgia Rewrite/`.
> Lancé en parallèle 5 agents (Explore + sensors + stories + dette).
> ⚠️ État disque, 24 fichiers WIP non-commit par autre terminal (forgia-mode-roguelite, forgia-rpg, forgia-foliage, forgia-game).

---

## 0. TL;DR

| | RPG | Roguelite |
|---|---|---|
| **Maturité** | ✅ Stable, jouable | 🟡 Pipeline shippé, contenu en migration |
| **Stories DONE** | 6/6 RPG-spécifiques | 5/15 + 5 IN_PROGRESS + 1 SKIP + 4 unclear |
| **Sensors** | 🔴 CHK-4 critical (3 textures legacy 404) | ✅ OK (in_run snapshot stage 0/4 wave 2/3) |
| **Blocage majeur** | Aucun gameplay-blocking | **Workspace cargo run cassé** (story-471..479 APIs migrées) |
| **Risque ship V7** | Bas | Élevé — voicelines absent, damage routing fragile, runtime non-validé |

**Verdict global** : RPG est **bon état de marche** modulo dette texture legacy. Roguelite a son **squelette livré end-to-end** (run lifecycle + waves + loot + stage flow + spatial identity) mais **un refacto multi-crate en cours casse la compilation et masque la validation runtime des derniers livrables (485 AC6/AC7, 490 hits_with_damage)**.

---

## 1. Cartographie Mode RPG

### Entry & Lifecycle
- **Plugin** : `ForgiaRpgPlugin` ([crates/forgia-rpg/src/lib.rs:90-178](C:/Users/Antoi/Desktop/Forgia Rewrite/crates/forgia-rpg/src/lib.rs#L90-L178))
- **OnEnter(GameMode::Rpg)** : `spawn_world` (lib.rs:251-401) — 1 chunk HeightMap-grid 32×32m, Voronoi 10 biomes, sun + 18 cloud clusters, village genome request, `StreamingPause` auto
- **OnExit** : `cleanup_world` (lib.rs:1667-1699) — despawn `RpgWorldMarker` + LOD2 mega-tiles + reset resources

### Sous-systèmes opérationnels
| Système | Fichier:Ligne | Statut |
|---|---|---|
| Streaming chunks triple-radius UE5 | `lib.rs:431-663` | ✅ |
| Memory budget LRU eviction | `lib.rs:839-931` | ✅ |
| Debug overlay F3 (gizmos LOD) | `lib.rs:677-824` | ✅ |
| Village paths ribbon (Roman road) | `lib.rs:1023-1101` | ✅ |
| Rex character spawn + OrbitCamera | `character.rs:89-174` | ✅ |
| Character lineup (4 PNJ) | `character.rs:222-281` | ✅ |
| Procedural locomotion rigless | `character.rs:692-802` | ✅ (fallback) |
| Dialogue trees (Aldric, Lyra) | `lib.rs:1105-1189` | ✅ sample |
| Interaction E | `lib.rs:1703-1738` | ✅ |

### Sensors RPG
- `forgia_chunks_snapshot.json` — 1Hz, loaded chunks + biome distribution
- `forgia_walk_pose.json`, `forgia_foot_ik.json`, `forgia_rex_bones_live.json` — anim-locomotion
- `forgia2_rpg_health.json` — **CRITICAL** (cf §3)

### Trous / WIP
- `forgia-mode-rpg-openworld` : **stub 17 LOC** `// TODO: implement` (rôle wrapper incertain)
- `character.rs` 803 LOC : `#![allow(dead_code)]` — auto-rig Pinocchio skinning OFF, story-440 phase 2 (locomotion par-bone désactivé, fallback rigless actif)
- Bones state leak entry RPG → autre mode → RPG (story-482 P2c diag commit `11c1d1a`)
- `SeaLevel` Resource commenté dans forgia-rpg lib.rs:97 — fallback `forgia-water::SEA_LEVEL=4.0` constant

---

## 2. Cartographie Mode Roguelite

### Run Lifecycle ([crates/forgia-mode-roguelite/src/run.rs:25-470](C:/Users/Antoi/Desktop/Forgia Rewrite/crates/forgia-mode-roguelite/src/run.rs))
- States : `Lobby → InRun{stage} → Boss{stage} → Victory|Defeat` (SubState Bevy 0.18)
- Events : `StartRunEvent` (seed opt), `EndRunEvent(Victory|Defeat)`
- Seed déterministe **xoshiro256\*\*** (host-auth, pas float rapier)
- Defeat : Observer `On<DeathEvent>` filter `target==Player` + latch anti-double `victory_emitted`

### Stage Flow
- `sys_stage_dispatch` → `StageLoadRequest` → `spawn_stage_arena_on_request` (forgia-stage-arena)
- 2 stages TOML : `crypts_of_anvil`, `forge_sanctum` ; alternance + boss force crypts
- **Story-485 phase 5 câblée** : `layout::place_modules` (sight-line solver, cover/sniper-perch/melee-pit, dart-throw + seed splitmix64)

### Waves & Bots
| Wave | Composition | Total |
|---|---|---|
| 1 | 3 Tank + 3 Runner + 2 Sniper | 8 |
| 2 | 4 Tank + 4 Runner + 4 Sniper | 12 |
| 3 (boss) | 1 Boss + 4 Runner | 5 |

**Archetypes** (`stats_for()` pure) : Tank (HP120/range4m), Runner (35/7m), Sniper (45/24m), Boss (800/30m, enrage <50% HP).
Boss enrage : speed ×1.8, cooldown ×0.55.

### Loot & Rewards
- Pickup spawn sur `obs_roguelite_enemy_death` : Boss→Heart 40, low-HP→Heart 20, sinon Souls 5/3/2 selon archetype
- `Souls` Resource (current + total_collected, saturation u32)
- Walk-over collect radius 2.5m, lifetime 30s

### Sensors Roguelite
- `forgia2_roguelite_state.json` — **OK** (snapshot : in_run, stage 0/4, wave 2/3, bots_alive 20, souls 0)
- `forgia2_stage.json`, `forgia2_stage_layout.json` (story-485 ph5), `forgia_arena_waves.json`, `forgia_stage_graph.json`

### Modules dormants (compilent en no-op, APIs disparues)
- `draw_portal_overlay` — `RogueliteWave::pending_portal_choices` supprimé
- `draw_bark_bubble` — `ActiveBark` supprimé (voicelines wipe)
- `draw_stage_notification` — `wave.notification` supprimé
- `parse_music_state` — `MusicState` n'existe plus → retourne `None`
- `sys_apply_stage_toggles` — music_state toggle disabled, weather log-only
- `sys_unstick_bots` — supprimé, pas réimplémenté (risque bots stuck non-détecté)

---

## 3. Sensors — État actuel

| Sensor | Severity | Notes |
|---|---|---|
| `forgia2_health.json` | 🔴 **critical** | source=rpg_health |
| `forgia2_rpg_health.json` | 🔴 **critical** | CHK-4: 3 textures legacy 404 (`textures-v1/terrain/grass/{diff,normal,roughness}.jpg`) |
| `forgia2_sensor_health.json` | 🟡 warn | `rpg_health.json` stale (producer pas tick'd) |
| `forgia2_roguelite_state.json` | ✅ ok | run_state=in_run, stage 0/4, wave 2/3, souls=0, bots_alive=20 |
| `forgia2_stage*.json`, `forgia2_combat.json`, etc. | ✅ ok | nominal |
| CHK-1 (LOD2 desync), CHK-2 (biome lum), CHK-3 (sample asym), CHK-5 (liveness), CHK-6 (HP coherence) | ✅ ok | tous verts |

**Lecture importante** : le snapshot roguelite montre `souls_current=0` après wave 2 en cours (12 bots devraient avoir drop ≥ 20 souls). Cohérent avec bug story-490 damage routing fix livré mais non-validé runtime.

---

## 4. Stories — Statut détaillé

### RPG (6/6 ✅)
| ID | Titre | Statut |
|---|---|---|
| 441 | Spawn Village V1 | ✅ DONE 2026-05-17 (12/12 AC) |
| 442 | Procgen Village V1 | ⏳ IN_PROGRESS (0/8) — VillageDef reproducibility + procgen |
| 447 | Village Terrain Leveling | ✅ DONE 2026-05-18 (8/8) |
| 452 | RPG Health Monitor | ✅ DONE 2026-05-18 (8/8) |
| 453 | RPG Monitor Debt | ✅ DONE 2026-05-19 (5/5) |
| 486 | Jolcham Oak Bark Wireup | ✅ DONE 2026-05-21 (7/7) — 94% coverage trees |

### Roguelite (5 DONE / 5 IN_PROGRESS / 1 DRAFT / 1 SKIP / 4 unclear)
| ID | Titre | Statut | Blocage/Next |
|---|---|---|---|
| 418 | Arena StateScoped + health gating | ❓ unclear | À vérifier (cf _live_status) |
| 448 | Arena Precise Colliders | ✅ DONE (BUG-448-03 runtime pending) | |
| 449 | Bot Hitbox Auto-Calibrate | ✅ DONE (validation pending) | |
| 450 | Wave5 Phase3 Audit | ✅ DONE 8/8 | Manhattan→Euclidean fix |
| 453-arena | Combat Baseline Reset | ✅ DONE (5 AC pending) | |
| **455** | FPS UI Juice AAA | 📝 DRAFT — 7 phases | Ammo HUD, killfeed, DDI |
| **456** | Hit Feedback AAA | ⏳ IN_PROGRESS Vague 1 | Nameplate billboards + headshot popup |
| **464** | Bot LOS State Gating | ⏳ IN_PROGRESS 7/7 | `los_lost_grace_secs` genome |
| **468** | Roguelite MVP (Enterprise) | 📋 PLAN — 6 milestones | M1-M6 |
| 470 | V7 M1 Fondations | 📋 PLAN — scaffold M1 | RunState SubStates |
| 471 | Analytics Sentry | ✅ DONE 8/8 | Wiring forgia-game pending |
| 472 | Audio Voicelines Tier1 | ❓ unclear | scaffold wipé ? |
| 473 | Stage Graph | ✅ DONE 10/10 (24 tests) | |
| 474 | Loot Tables | ⏸️ SKIP — coordination dual-terminal | |
| 475 | Equipment | ✅ DONE 8/8 (12 tests) | |
| 476-479 | Status FX / MusicState / Ducking / Scene Saves | mostly DONE | wiring pending |
| 480/481/482 | Skeleton template + voicelines + anim | majoritairement ✅ | anim P2c WIP |
| 483 | Stage Arena Foundations | ✅ DONE 12/12 (88 tests) | |
| 485 | Arena Spatial Identity | ✅ CODE-COMPLETE 13/13 | **AC6/AC7 runtime deferred** |
| **490** | Damage Routing Bridge | ⏳ IN_PROGRESS 9/9 | Fix livré, validation runtime bloquée |

---

## 5. Bugs & Dette

### Bugs OPEN
| ID | Sév | Description |
|---|---|---|
| **BUG-490** | 🔴 Critique | Roguelite `hits_with_damage=0` — dual-Health type trap, fix livré commit `64bee3d`, validation pending |
| BUG-464-04 | 🟢 Cosmétique | `ArenaBot::default()` hardcode `los_lost_grace_left: 2.0` |
| BUG-483-03 | 🟡 Mineur | `pois_pool.collect()` alloc/stage (1×/load, toléré MVP) |
| **Workspace cargo run cassé** | 🔴 Critique | Depuis `9e149ca` — APIs `forgia_audio_voicelines`, `forgia_loot_tables`, `forgia_audio_music_state`, `waves::current_stage_node` référencées mais non-implémentées |

### Bugs récents fixés (last 30 commits)
`64bee3d` story-490 damage routing · `6868647` story-486 sensor runtime · `043746c` 4 dead-code warnings · `6f28a59` stage-arena qa-lead hardening · `11c1d1a` anim-locomotion hand bones · `73d8b41` shin/foot/forearm Pinocchio · `eaba1bb` ForgiaPrefabPlugin double-add guard · `3a2b2f6` extract forgia-anim-locomotion · `0226f8f` Kael.glb dupe removed.

### Dette RPG
1. **SeaLevel Resource hardcode** — fallback `forgia-water::SEA_LEVEL=4.0` const, pas de SoT
2. **Anim state leak entry/exit RPG** — patrouille `is_bone_valid()` dupliquée
3. **Pinocchio bone name lookup** — workaround `.get_bone_index_by_name()`, pas de Bevy native mapping
4. **CHK-4 textures legacy** — 3 paths `textures-v1/terrain/grass/*` orphelins, criticals depuis migration v2
5. **forgia-mode-rpg-openworld** : stub 17 LOC, rôle indéfini

### Dette Roguelite
1. **Dual-Health type trap endemic** — V7 migration unification (story-491 future)
2. **Wave orchestrator unstick_bots manquant** — risque bots stuck terrain non-détecté
3. **Voicelines crate wipé** — `BarkEvent`/`ActiveBark` absents, M1 audio wireup bloqué
4. **Loot pickup attacker unknown** — story-492 future pour coop attribution
5. **Stage Layout AC6/AC7 runtime validation impossible** — workspace cassé

### Dette transverse (impact ship V7)
1. **i18n strings FR inline** (story-468 § BLOQUANT B4) — 24 barks × 4 armes en FR, Steam EN requis → Fluent + `.ftl` (~1-2j)
2. **DamageEvent ordre observers non-garanti** (Bevy 0.18) — `BufferedEvent` + 3 systems `.chain()` requis
3. **Genome registry validator absent** — `forgia-genome-core` 94 LOC, validation cross-crate manquante
4. **Inventory 169 LOC sans Plugin** (LOCK-INV-1) — 80 slots hardcoded, bloque loot wiring story-475
5. **CHK-4 critical_assets** — 3 textures legacy 404, severity=critical permanent

---

## 6. Top 10 Plan de Remédiation (priorisé effort × impact)

| # | Action | Effort | Impact | Mode | Story candidate |
|---|---|---|---|---|---|
| **1** | 🔴 **Re-compiler workspace** — refacto APIs `voicelines/loot_tables/music_state/current_stage_node` ou stub temporaire | 1-2j | Critique | Roguelite | **story-491** (proposée) |
| **2** | 🔴 **Valider runtime story-490** (hits_with_damage>0, souls collect>0) après #1 | 0.5j | Critique | Roguelite | story-490 reopen |
| **3** | 🔴 **Valider runtime story-485 AC6/AC7** (`forgia2_stage_layout.json` cover_count/sightline_max in-app) | 0.5j | Élevé | Roguelite | story-485 close |
| **4** | 🟡 **Nettoyer CHK-4 critical_assets** — retirer 3 paths `textures-v1/` orphelins du config (5 min) ou re-pointer | 0.25j | Moyen (sensor health critical permanent) | RPG | quick-fix |
| **5** | 🟡 **i18n Fluent + `.ftl`** (24 barks × 4 armes EN/FR) — pré-ship Steam | 1-2j | Élevé (Steam EN obligatoire) | Roguelite | **story-492** (proposée) |
| **6** | 🟡 **Re-implémenter `sys_unstick_bots`** dans wave orchestrator | 0.5j | Moyen | Roguelite | quick-fix story-470 |
| **7** | 🟡 **DamageEvent `BufferedEvent` + `.chain()`** — fix observer ordering Bevy 0.18 | 1j | Élevé (multi-consumer fragile) | transverse | **story-493** (proposée) |
| **8** | 🟢 **forgia-mode-rpg-openworld** — soit peupler (vrai mode) soit supprimer (stub mort) | 0.5j | Bas | RPG | quick-fix |
| **9** | 🟢 **Voicelines crate re-implémentation Tier 1.5** — BarkEvent + ActiveBark + 1 wav stub | 1j | Moyen (débloque M1 audio) | Roguelite | story-481 reopen |
| **10** | 🟢 **Genome registry validator** — `forgia-genome-core` peuple : assert types cross-crate | 2j | Moyen long-terme | transverse | **story-494** (proposée) |

**Total** : ~8-12j cumulés pour remettre Roguelite en état "shippable runtime-validated".

---

## 7. Stories candidates proposées

| ID | Titre | Effort | Scope |
|---|---|---|---|
| story-491 | Workspace re-compile : refacto APIs voicelines/loot/music/waves | Standard | 4 crates touchées |
| story-492 | i18n Fluent + .ftl bilingue EN/FR (barks roguelite) | Standard | forgia-i18n + voicelines |
| story-493 | DamageEvent multi-observer ordering fix (BufferedEvent + chain) | Standard | forgia-damage + consumers |
| story-494 | Genome registry validator cross-crate | Enterprise | forgia-genome-core peuple |

À ouvrir en DRAFT — pas créées maintenant (audit profondeur 2 sur 3, pas 3). Si tu veux que je les crée, je peux enchaîner.

---

## 8. Recommandation immédiate

**Priorité absolue = #1** (re-compile workspace). Tant que `cargo run` ne marche pas :
- AC6/AC7 story-485 non-validés (livraison code-only)
- Story-490 fix damage routing non-validé (sensor `hits_with_damage` reste 0)
- Aucune nouvelle feature Roguelite testable in-game
- Story-468 M2+ enchainement bloqué

Une fois compile vert, ré-ouvrir 490 + 485 en 30 min de runtime check, puis enchaîner i18n (#5) avant tout polish UI/feedback (455/456).

---

*Audit généré 2026-05-21 par 5 agents Explore // + lecture directe sensors V2. État disque non-commit (24 fichiers WIP autre terminal). À ré-exécuter après prochain push pour ground-truth.*

---

## 9. Régressions identifiées (passe dédiée)

### 🔴 CONFIRMÉES (sévérité haute)

| # | Régression | Story origine | Fichier | Signal |
|---|---|---|---|---|
| R1 | **ForgiaAudioVoicelinesPlugin désactivé** — crate redevenue scaffold vide, plugin commenté | story-481/482 DONE 2026-05-20 (30/30 tests) | `crates/forgia-game/src/lib.rs:81-84` (WIP) | `// forgia_audio_voicelines::ForgiaAudioVoicelinesPlugin` + TODO(refactor-abandonné) |
| R2 | **sys_unstick_bots supprimé** — fonction inexistante, system commenté | story-470 V7 M1 fondations | `crates/forgia-mode-roguelite/src/lib.rs:119-120` (WIP) | TODO(story-471..479): supprimé de `crate::waves` |
| R3 | **forgia2_rpg_health.json STALE** — producteur ne tick plus | story-452 RPG Health Monitor DONE 2026-05-18 (8/8) | tick_count=1300, timestamp=1317s, sensor_health flag stale | `forgia2_sensor_health.json::stale_paths` |
| R4 | **SeaLevel Resource RPG commenté** — fallback const SEA_LEVEL=4.0 dans forgia-water | story-450 wave2 | `crates/forgia-rpg/src/lib.rs:100-105` (WIP) | TODO(refactor-abandonné) |

### 🟡 PROBABLES (sévérité moyenne, fonctions dormantes)

| # | Régression | Story origine | Fichier |
|---|---|---|---|
| R5 | `draw_portal_overlay` → no-op stub (UX sélection portail silencieuse) | story-470 | `forgia-mode-roguelite/src/hud.rs:366-374` |
| R6 | `draw_bark_bubble` → no-op stub (tier-1.6 UI désactivée) | story-482 (30/30 tests) | `forgia-mode-roguelite/src/hud.rs:406-412` |
| R7 | `draw_stage_notification` → no-op stub (toast système stage) | story-470 | `forgia-mode-roguelite/src/hud.rs:421-428` |
| R8 | `parse_music_state` → toujours `None` (consumer absent) | story-477 DONE 2026-05-20 (12/12) | `forgia-mode-roguelite/src/run.rs:167-169` |
| R9 | `sys_apply_stage_toggles` → music toggle off, weather log-only | story-477/483 | `forgia-mode-roguelite/src/run.rs:174-193` |

### 🟢 SUSPECTÉES (validation runtime requise post-#491)

| # | Régression | Story origine | Notes |
|---|---|---|---|
| R10 | `ForgiaAnalyticsPlugin` — scaffold 16 LOC `// TODO: implement` malgré story DONE 8/8 affirmée | story-471 | **Discordance story doc vs code** — audit story-471 nécessaire |
| R11 | CHK-5 sensor_liveness reporté ok mais sensor lui-même stale (22min) | story-452 | Cohérent avec R3 — sensor ne se valide pas lui-même quand le tick s'arrête |

### Quick-fix (< 30 min)

| Action | Effort |
|---|---|
| Re-activate `ForgiaAudioVoicelinesPlugin` ligne 81-84 forgia-game/lib.rs | 5 min (si crate reste accessible) |
| Re-add `SeaLevel` Resource forgia-rpg/lib.rs | 10 min |
| Re-impl `sys_unstick_bots` (copier signature, 20 LOC AI fallback) | 20 min |
| Audit story-471 (vérifier si code Analytics réel existe ou si scaffold = vraie état) | 15 min |

### Conséquence sur Top 10 §6

**Ces régressions sont massivement liées à #1 (workspace re-compile)**. La majorité (R1, R5-R9) sont des artefacts du refacto WIP autre terminal qui a wipé les APIs sans réimpl. Une fois #1 livré (story-491), R1/R2/R4/R5-R9 peuvent être tranchées en bloc : soit re-impl, soit suppression propre avec ref story future.

**R3 (rpg_health stale) est différent** : story-452 est en charge du sensor, son producer system tourne dans `forgia-observability` (à confirmer). Soit le system n'est plus dans la chaîne `Update`, soit `run_if(in_state(GameMode::Rpg))` mais le sensor a été lu hors RPG. À investiguer une fois compile rétabli.

**R10 (Analytics scaffold vs claim DONE)** est le plus inquiétant : **divergence entre statut story affiché et code réel**. Si confirmé, signale un trou dans le rituel checklist post-impl — à investiguer pour comprendre comment une story 8/8 peut laisser un crate 16-LOC scaffold.
