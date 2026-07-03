# Audit Performance — Forgia V2 Roguelite vs Industrie

> **Date** : 2026-07-01 · **Scope** : perfs du jeu (mode Roguelite shippé) + comparaison état de l'art industrie.
> **Méthode** : 2 agents d'audit interne (télémétrie runtime + anti-patterns Bevy 0.18) + veille web sourcée.
> **Données** : sensors runtime frais (session en cours, mtime 22:55-22:58). Lecture seule, 0 fichier modifié.

---

## 0. TL;DR (verdict en 5 lignes)

- **Le jeu NE tient PAS 60 fps stable** : moyenne 16,57 ms (= plafond budget), `fps_smoothed 55,2` **même à vide** (0 ennemi), et un **cluster de freeze 186-250 ms** (5 frames droppées d'affilée) **non corrélé à la charge combat**.
- **Cause n°1 la plus probable** : stall bloquant du main thread = **chunk streaming synchrone** (confirmé par le sensor : `async pool not implemented`) et/ou upload GPU de texture. Trou d'observabilité : aucun sensor ne timestampe le moment exact d'un chunk-load vs le frame lag.
- **Levier structurel n°1** : les **collision groups G1-G5 sont écrits mais jamais câblés** → tout collide avec tout, chaque raycast (tir + LOS bot 8 Hz + avoidance + collide-and-slide) teste la BVH complète et filtre après coup via `HashSet`/walk d'arbre alloués par appel.
- **Dette scaling** : `bot_separation` en O(n²) + 2 allocations/frame **viole la règle projet `scalability.md`** — négligeable à N=3 bots actuels, dangereux quand les vagues densifieront (Gunfire vise 15-20 ennemis à l'écran).
- **Bonne nouvelle** : la physique est quasi-gratuite (1486/1487 corps `Fixed`, 0 `Dynamic`), la génération de chunk est sous budget (1 ms), l'infra d'observabilité est riche.

---

## 1. État runtime mesuré (sensors frais 2026-07-01 22:58)

| Sensor | Contenu clé | Lecture |
|---|---|---|
| `forgia2_perf.json` | frame_avg **16,57 ms**, min 2,76, **max 31,6 ms**, fps_smoothed **55,2** | 🟡 à vide, sous 60 fps |
| `forgia2_perf_diag.json` | **8 freezes** retenus, max **120,2 ms**, à `enemies: 0-2` | 🔴 pas la charge combat |
| `forgia2_lag_events.json` | 209 events, **21/30 s** ; cluster t=131,17-131,92 s : dt 55/204/**250/250**/186 ms consécutifs | 🔴 stall bloquant |
| `forgia2_physics.json` | 1487 rigid bodies (**1486 Fixed, 0 Dynamic**), 1492 colliders, 1 KCC | 🟢 physique gratuite |
| `forgia2_entities.json` | **24 930 entités**, 1 player, 0 bots (creux entre vagues) | — |
| `forgia2_render.json` | **7 693 mesh3d (7 594 visibles)**, 3 caméras (1 active) | 🟠 draw calls non mesurés |
| `forgia2_vram.json` | ~**968 MB** (954 images + 14 meshes) ; top texture 10,64 MB ×3 (oak_bark 2k) | 🟡 textures = 98,5 % |
| `forgia2_memory.json` | RAM **2 511 MB**, VRAM adapter "N/A" (non instrumenté) | 🟢 RAM saine |
| `forgia_chunk_stream.json` | 49 chunks, gen_ms mean **1,00** / p99 1,00 — **mode: synchronous, async pool not implemented** | 🟠 sous budget mais non capé |
| `forgia2_sensor_health.json` | **2 stale** : `forgia2_arena.json`, `forgia2_combat.json` — producteurs bloqués ? | 🟠 trou de visibilité |
| `forgia2_toon.json` | **warn** : outline_enabled=true mais **0 caméra attachée** (config morte) | 🟡 connu (crash wgpu) |

---

## 2. Findings par sévérité (avec fichier:ligne)

### 🔴 Critique

**F1 — Cluster de freeze 186-250 ms non expliqué par la charge**
`forgia2_lag_events.json` : 5 events consécutifs (55/204/250/250/186 ms) en 1 s réelle, à `enemies: 0-2`, `particle_effects: 0-3`, `point_lights: 40`. Le clamp exact à **250,00 ms deux fois** sent le **blocking I/O ou stall GPU/driver**, pas un pic CPU progressif.
→ **Candidats** : (a) chunk streaming synchrone déclenché en déplacement joueur ; (b) polls hot-reload TOML 1 Hz (mushrooms/decor/enemies/combat) tombant sur le même tick avec `fs::read_to_string`+`fs::metadata` synchrones ; (c) upload GPU de texture au chargement d'un chunk à décor lourd.
→ **Bloqueur** : aucun sensor ne capture le timestamp exact d'un chunk-load/texture-upload → hypothèse non confirmable à 100 %.

**F2 — Chunk streaming synchrone sur le main thread**
`crates/forgia-terrain/src/chunk.rs:233` · sensor : `"mode":"synchronous","_note":"async pool not implemented -> current_depth always 0 (placeholder)"`.
→ gen actuelle 1 ms (négligeable) mais **budget non garanti** : un chunk à forte densité vegetation/SDF peut dépasser 8 ms sans throttle au-delà de `chunks_per_frame: 2`. Dette structurelle la plus probable derrière les stutters en mouvement.

**F3 — `CollisionGroups` G1-G5 jamais câblés → tout collide avec tout**
Définis avec tests verts dans `crates/forgia-terrain/src/collision.rs:9-23`, mais **0 site d'appel gameplay** :
- Colliders arène spawnés sans `CollisionGroups` : `forgia-mode-fps-arena/src/lib.rs:497-902`, `forgia-stage/src/lib.rs:911-955`.
- Raycasts `QueryFilter::default()` (jamais `.groups()`) : `forgia-fps/src/lib.rs:931` (tir joueur), `forgia-ai-arena-bot/src/tactical.rs:142,267,330,358` (LOS 8 Hz + avoidance + collide-and-slide ×2).
→ **Chaque raycast teste la BVH complète** (terrain + décor + tous bots + tous colliders) et filtre a posteriori via `predicate` + `HashSet<Entity>`/walk `ChildOf` **alloués par tir** (`forgia-fps/src/lib.rs:888-901`). G1-G5 = code mort runtime.

**F4 — Allocation `meshes.add()`/`materials.add()` par tir de bot**
`crates/forgia-ai-arena-bot/src/lib.rs:412,419` (`spawn_tracer`, appelé à **chaque tir de chaque bot**) alloue 2 entrées Assets par coup. Le pré-warm correct existe **juste au-dessus** (`BotFireballAssets`, l.142-163) — le tracer a été oublié. Bonus : matériaux dupliqués = **casse le batching auto** de Bevy.

### 🟠 Majeur

**F5 — `bot_separation` : O(n²) bots×bots + 2 allocations/frame, sans throttle**
`crates/forgia-ai-arena-bot/src/tactical.rs:444-494` : double boucle pairwise (l.457-483, commentaire admet *"O(N²) acceptable jusqu'à ~50 bots"*), + `Vec<(Entity,Vec3)>` (l.452) + `HashMap<Entity,Vec2>` (l.456) **alloués chaque frame**, aucun `run_if`.
→ **Viole directement `scalability.md`** (« pas de `Vec::new()`/`HashMap::new()` en hot path ; utiliser `Local<>`+`clear()` ; pas de brute-force O(n²), utiliser une grille spatiale »).

**F6 — Chaînes IA & nameplate sans `run_if(in_state)`**
`forgia-ai-arena-bot/src/lib.rs:216-240` (11 systèmes) et `forgia-enemy-nameplate/src/lib.rs:81-93` (7 systèmes) tournent en `Update` **même en Menu/Pause/RPG** — coût `ReadRapierContext`+query+`Res<Time>` payé chaque frame. Le pattern correct est juste à côté (`forgia-fps/src/lib.rs:499-541`). **Non conforme L7 (GameSet).**

**F7 — `sys_unstick_bots_from_decor` : O(bots×obstacles) chaque frame, sans throttle**
`crates/forgia-mode-roguelite/src/decor.rs:781-807` : double boucle, seul guard `is_empty()`. Correctif de position → un throttle 4-8 Hz suffit.

**F8 — `asset_server.load()` runtime dans le spawn d'arène (viole L1)**
`crates/forgia-stage/src/lib.rs:870-874` charge les scenes floor/dirt/rocks à la volée au lieu de `GameAssets` préchargé. Pas per-frame (idempotent) mais chaque nouveau stage re-déclenche un load disque non batché — candidat du stall F1.

**F9 — `tick_respawns` alloue un `Vec` même queue vide** · `forgia-ai-arena-bot/src/lib.rs:543` → `Local<Vec>` + `clear()` ou early-return.

**F10 — 2 sensors stale (`combat`, `arena`)** · `forgia2_sensor_health.json`. Si leur producteur a freeze pendant le cluster t=131 s, ça confirmerait un blocage système large. À investiguer.

### 🟡 Mineur

- **F11** — 3 textures dupliquées à 10,64 MB (oak_bark 2k, diff/nor/arm) = ~32 MB pour une écorce de fond ; textures = 98,5 % des 968 MB VRAM. Mip cap 1k sur assets d'ambiance → gain ~15-20 MB/asset. (`forgia2_vram.json`)
- **F12** — Outline toon configuré mais 0 caméra attachée = config morte (déjà documenté, crash wgpu si réactivé — cf memory `reference_toon_outline_dual_pass_crash`). (`forgia2_toon.json`)
- **F13** — `billboard_to_camera` sans LOD distance (`forgia-enemy-nameplate/src/lib.rs:291-309`) — Lock L5 prévu Phase 3, à activer avant scale-up bots.
- **F14** — `find_health_ancestor` walk `ChildOf` (`forgia-fps/src/lib.rs:562-582`) — OK à faible volume, à surveiller si genome `pellets > 16`.

### 🟢 OK (à préserver / répliquer)

- Physique quasi-gratuite : 1486/1487 corps `Fixed`, 0 `Dynamic`.
- Chunk gen 1,00 ms mean / 1,64 max — largement sous le budget 8 ms (malgré le mode sync).
- `bot_los_check` throttlé **8 Hz** proprement (`tactical.rs:84`) → **le bon pattern à répliquer sur `bot_separation`**.
- `sys_perf_diag` : queries filtrées `With<T>`, `count()` 1×/s — négligeable même à 25k entités.
- `mushrooms.rs` : spawn one-shot idempotent, PointLight par cluster (pas par champignon).

---

## 3. Budget frame — verdict chiffré

| Catégorie | Budget | Mesuré | Statut |
|---|---|---|---|
| Frame moyen | < 16,6 ms | 16,57 ms (fps 55,2) | 🟡 au plafond, **0 réserve** |
| Frame max session | < 16,6 ms | **250 ms** (cluster) / 120 / 31,6 | 🔴 ×15 le budget |
| Chunk gen | < 8 ms | 1,00 ms | 🟢 sous budget (mais non capé) |
| Draw calls | < ~2500 | **non mesuré** (proxy 7 594 meshes visibles) | 🟠 trou d'instrumentation |
| VRAM | preset | 968 MB (98,5 % textures) | 🟢 raisonnable PBR 2K |
| RAM | — | 2 511 MB | 🟢 sain |

**Point aveugle clé** : à vide (0 ennemi), le jeu est déjà à 55 fps. Où partent les 16 ms ? 7 594 meshes visibles + PBR 2K → probablement **GPU-bound sur la scène statique**, OU coût CPU des systèmes non gatés (F6). **Diagnostic manquant : CPU-bound vs GPU-bound** (étape 1 de la méthode industrie ci-dessous).

---

## 4. Ce qui se fait dans l'industrie (veille sourcée)

**Budget & mesure**
- Budget frame = **16,6 ms @ 60 fps / 8,3 ms @ 120 fps**, réparti input(1-8) → logique(2-10) → culling(1-5) → draw calls(1-3) → GPU(5-15) → présentation. Garder une **réserve** pour la variation inter-niveaux. Forgia est **à 16,57 ms sans réserve**.
- **La constance du frame-time prime sur la moyenne FPS** : un 30 fps stable est plus fluide qu'un 60 fps qui oscille 10-30 ms. → Le cluster 250 ms de Forgia est *exactement* le failure mode qui compte, pas la moyenne.
- Méthode de profiling pro : **mesurer en ms, pas en FPS** (non linéaire) ; benchmarks **déterministes** (caméra sur spline, moment de jeu fixe) ; profiler **CPU et GPU séparément** ; profiler le **jeu complet** (save/cheat), pas le 1er niveau ; stocker l'historique pour comparer avant/après. *La réduction de draw calls ne garantit pas un gain frame-time — valider au profiler GPU.*

**Bevy / moteur**
- **Batching/instancing automatique** : jusqu'à **×2 fps** (160k cubes, 11,7k visibles en 1 draw instancé) — **à condition même mesh + même material**. → F4 (matériaux dupliqués par tir) et la vérif du batching sur les 7 594 meshes sont directement concernés.
- **`par_iter`** : utile seulement sur **gros N à travail uniforme** ; en dessous, l'overhead dépasse le gain (Bevy retombe en single-thread tout seul). → Pour `bot_separation`, throttle + grille spatiale avant `par_iter`.
- L'executor parallèle par défaut maximise le multi-thread ; les **systèmes exclusifs** (accès `&mut World`) cassent le parallélisme — à éviter en hot path.
- **Tracy** : `cargo run --features bevy/trace_tracy,bevy/debug --release`, UI Tracy lancée avant le jeu ; les spans GPU apparaissent en ligne `RenderQueue`. `FrameTimeDiagnosticsPlugin` pour FPS/frame-time in-app. → **workflow de profiling non documenté dans Forgia** ; l'infra sensor est riche mais il manque Tracy + le timestamp chunk-load.

**Cas genre (Gunfire Reborn)**
- Chute **165 → 40 fps** quand densité ennemis/alliés + clutter de particules à l'écran. → Confirme que le **risque #1 de Forgia** est le scaling de la densité de vagues (F5) + le coût des VFX simultanés, pas l'état à vide.

**hanabi** : batch les effets pour réduire les draw calls, mais coût GPU réel sur **overdraw** quand beaucoup d'effets simultanés — surveiller quand les vagues densifieront.

---

## 5. Plan priorisé (ROI décroissant)

| # | Prio | Action | Fichier(s) | Effort | Gain |
|---|---|---|---|---|---|
| 1 | **P0** | Instrumenter le **timestamp exact chunk-gen / texture-upload** vs `lag_events` | `forgia-terrain/src/chunk.rs` | ~1-2 h | Débloque la root cause du cluster 250 ms |
| 2 | **P0** | Diagnostiquer **CPU-bound vs GPU-bound** (Tracy + désactiver systèmes gatés) + investiguer les 2 sensors stale | workflow + `combat`/`arena` producers | ~1 h | Oriente tous les fixes suivants |
| 3 | **P1** | **Câbler `CollisionGroups` G1-G5** sur colliders arène/bots + `.groups()` sur tous les raycasts | `forgia-mode-fps-arena`, `forgia-stage`, `forgia-ai-arena-bot/tactical.rs`, `forgia-fps/src/lib.rs:931` | Moyen (tests déjà là) | **Broad-phase réduite sur CHAQUE raycast** (tir + LOS + avoidance + slide) — plus gros levier, 4 systèmes d'un coup |
| 4 | **P1** | Rendre le **chunk streaming asynchrone** (`AsyncComputeTaskPool`) | `forgia-terrain/src/chunk.rs` | Moyen | Supprime le stall main-thread non capé |
| 5 | **P1** | `bot_separation` → throttle 15-20 Hz + `Local<Vec>/Local<HashMap>` scratch ; pré-warm `BotTracerAssets` ; `run_if(in_state)` sur chaînes IA/nameplate | `forgia-ai-arena-bot/src/{lib,tactical}.rs`, `forgia-enemy-nameplate/src/lib.rs` | Faible | 0 alloc/frame, coût 0 hors combat, protège le scaling vagues |
| 6 | **P2** | Throttle `sys_unstick_bots_from_decor` 4-8 Hz ; mip cap 1k textures d'ambiance ; précharger stage scenes dans `GameAssets` | `decor.rs:781`, assets, `forgia-stage/src/lib.rs:870` | Faible | Micro-gains cumulés + conformité L1 |

**Vérifs post-fix** : re-lire `lag_events`+`perf_diag` (le cluster t=131 s doit disparaître ou pointer ailleurs) ; `sensor_health.stale` → 0 ; bench criterion chunk-gen après passage async ; test de charge `bot_separation` à N=30-50 bots simulés avant que le contenu ne scale.

---

## Sources (veille industrie)

- [Unofficial Bevy Cheat Book — Performance Tunables](https://bevy-cheatbook.github.io/setup/perf.html)
- [Unofficial Bevy Cheat Book — Internal Parallelism (par_iter)](https://bevy-cheatbook.github.io/programming/par-iter.html)
- [Bevy 0.12 — Automatic batching/instancing](https://bevy.org/news/bevy-0-12/)
- [Bevy — docs/profiling.md (Tracy)](https://github.com/bevyengine/bevy/blob/main/docs/profiling.md)
- [How to properly profile your game — Procedural Pixels](https://www.proceduralpixels.com/blog/how-to-properly-profile-your-game)
- [Game Performance Optimization Checklist — PulseGeek](https://pulsegeek.com/articles/game-performance-optimization-a-complete-checklist/)
- [Gunfire Reborn — Low FPS Fixes (Steam)](https://steamcommunity.com/sharedfiles/filedetails/?id=3595621371)
- [bevy_hanabi — GitHub](https://github.com/djeedai/bevy_hanabi)

*Audit lecture seule — 0 fichier de code modifié. Fixes à déléguer à `implementer` après validation du plan.*
