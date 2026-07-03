# Story-643 — Perf pass : allocations hot-path (Phase A) + backlog Phase B runtime-gated

**Statut** : 🚧 IN_PROGRESS (Phase A livrée & compilée ; Phase B différée jusqu'au push du terminal 2)
**Créée** : 2026-07-01
**Source** : audit perf `docs/audit/audit-2026-07-01-perfs-jeu-vs-industrie.md`
**BMAD** : Standard (cross-crate à terme) — Phase A livrée = 1 crate / 2 fichiers (Quick)
**Contexte multi-terminal** : terminal 2 actif sur `forgia-mode-roguelite/src/elements.rs` (story-642). Phase A limitée aux crates **orthogonales**, validée `cargo check -p` par crate (jamais le binaire complet → risque artefact stale, règle 5).

---

## Objectif

Éliminer les allocations hot-path identifiées par l'audit du 2026-07-01, sans toucher à la logique gameplay/collision (zéro risque behavior, validable par compilation seule). Les fixes behavior-sensitive et le diagnostic runtime sont différés en Phase B (exigent de faire tourner le jeu, actuellement bloqué par le terminal 2).

---

## Phase A — LIVRÉE ✅ (crate `forgia-ai-arena-bot`)

| # | Fix | Fichier:ligne | Finding audit | Compile |
|---|---|---|---|---|
| A1 | Pré-warm tracer bot : mesh constant + cache matériaux par couleur (`BotTracerAssets`) → 0 alloc `meshes.add`/`materials.add` par tir après le 1er de chaque teinte + batching préservé | `crates/forgia-ai-arena-bot/src/lib.rs` (`BotTracerAssets`, `setup_bot_fireball_assets`, `spawn_tracer`, `bot_shoot_at_target`) | 🔴 F4 | ✅ |
| A2 | `bot_separation` : buffers `Local<Vec>`/`Local<HashMap>` réutilisés (`.clear()`) au lieu de `Vec::new()`/`HashMap::new()` par frame → 2 allocs/frame supprimées | `crates/forgia-ai-arena-bot/src/tactical.rs:444` | 🟠 F5 (part alloc) | ✅ |
| A3 | `tick_respawns` : early-return si `queue.is_empty()` avant l'alloc `Vec::new()` (cas quasi-permanent) | `crates/forgia-ai-arena-bot/src/lib.rs` (`tick_respawns`) | 🟠 F9 | ✅ |

**Notes de conception**
- A1 : `tracer_emissive` **varie par archétype** (`forgia-mode-roguelite/src/enemies.rs:151`) → un matériau unique aurait changé les couleurs. Cache `HashMap<[u32;3], Handle>` clé = bits des composantes émissives → couleurs identiques, 0 alloc après warmup, mesh+matériau partagés (batching auto Bevy).
- A2 : **seule la part allocation** est faite. Le throttle 15-20 Hz + grille spatiale (sortie du O(n²)) est un **changement de feel** → Phase B (validation runtime requise).
- Validation : `cargo check -p forgia-ai-arena-bot` ✅ (99 crates) + `--tests` ✅. **Pas** de build binaire (terminal 2 actif).

---

## Phase B — DIFFÉRÉE (runtime-gated : à faire après push terminal 2)

Ces items exigent de **faire tourner le jeu** pour valider (behavior-sensitive ou observabilité à confirmer). Interdits maintenant : binaire stale (règle 5) + `no-speculative-fix`.

| # | Fix | Cible | Pourquoi runtime-gated | Sévérité |
|---|---|---|---|---|
| B1 | **Câbler `CollisionGroups` G1-G5** (membership sur colliders + `.groups()` sur raycasts) | `forgia-mode-fps-arena`, `forgia-stage`, `forgia-ai-arena-bot/tactical.rs`, `forgia-fps/src/lib.rs:931` | All-or-nothing : rater un collider = traverse sol / LOS cassée. Doit être validé en jeu. **Plus gros levier** (broad-phase sur chaque raycast). | 🔴 F3 |
| B2 | `run_if(in_state(Fps).or(Roguelite)).in_set(Combat)` sur chaînes IA + nameplate | `forgia-ai-arena-bot/src/lib.rs:216`, `forgia-enemy-nameplate/src/lib.rs:81` | Vérifier en jeu que les bots/nameplates tournent bien dans les bons modes. | 🟠 F6 |
| B3 | `bot_separation` : throttle 15-20 Hz + grille spatiale (sortie O(n²)) | `forgia-ai-arena-bot/src/tactical.rs` | Changement de feel (séparation moins fréquente) → valider visuellement. | 🟠 F5 (part throttle) |
| B4 | Chunk streaming réellement async (`AsyncComputeTaskPool`) | `forgia-streaming` + `forgia-rpg/src/lib.rs:872` | ⚠️ **Lead à confirmer d'abord** : `record_gen_ms` n'est appelé QUE depuis `forgia-rpg` (path RPG). Le Roguelite (arène authored) ne stream peut-être pas → le freeze 250 ms aurait une autre cause. À trancher runtime avant de coder. | 🔴 F2 |
| B5 | Throttle `sys_unstick_bots_from_decor` 4-8 Hz | `forgia-mode-roguelite/src/decor.rs:781` | ⚠️ **Même crate que terminal 2** (`forgia-mode-roguelite`) → attendre son push pour éviter le conflit. | 🟠 F7 |
| B6 | **Diagnostic CPU-bound vs GPU-bound** (Tracy `--features bevy/trace_tracy,bevy/debug`) + instrumentation timestamp du vrai coût du freeze 250 ms | workflow + crate à identifier | Exige de faire tourner le jeu. **Prérequis** de B1/B4 (oriente la vraie cause). À vide 55 fps : est-ce GPU (7594 meshes PBR 2K) ou CPU (systèmes non gatés) ? | 🔴 F1 |
| B7 | Mip cap 1k textures d'ambiance + précharger stage scenes dans `GameAssets` | assets, `forgia-stage/src/lib.rs:870` | Valider visuellement le mip cap ; L1. | 🟡 F8/F11 |

---

## Acceptance Criteria

Phase A :
- [x] A1 pré-warm tracer (mesh + cache mat par couleur), couleurs préservées
- [x] A2 `bot_separation` buffers `Local<>` (part alloc)
- [x] A3 `tick_respawns` early-return
- [x] `cargo check -p forgia-ai-arena-bot` + `--tests` verts
- [x] 0 build binaire (coordination terminal 2)

Phase B (à cocher post-push terminal 2) :
- [ ] B6 diagnostic CPU/GPU-bound (prérequis)
- [ ] B4 lead chunk streaming tranché runtime
- [ ] B1 CollisionGroups câblés + validés en jeu
- [ ] B2/B3/B5/B7

---

## Cross-refs
- Audit : `docs/audit/audit-2026-07-01-perfs-jeu-vs-industrie.md`
- Règles : `scalability.md` (buffers Local<>), `no-speculative-fix.md` (B différé), `multi-terminal-coordination.md` (règle 5 artefact stale)
- Story voisine active : `story-642` (terminal 2, `forgia-mode-roguelite`)
