# Story-581 — Monitor perf/mémoire enrichi + VRAM (port V1)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_lag_events.json`, fichier `bindings.rs`, symbole `ForgiaDebugPlugin`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : EN COURS (Phase A du chantier "loading/memory best-practices").
**Scale** : Standard (forgia-observability + forgia-debug, ~6 fichiers).
**Date** : 2026-06-07
**Lignée** : User — *"vérifie les logs des chargements de la map, la gestion mémoire. Rust est censé être optimisé pour ça, regarde les best-practices industrie. Affiche-moi un moniteur perf pour contrôler."* Puis : *"regarde dans D:\Forgia (V1) y'a peut-être une base à reprendre."*

## Diagnostic live (jeu en cours, RPG, t=141s)

- Steady-state **244 FPS** (frame 3.23ms), **RAM 2,17 GB** → OK.
- 🔴 **Stutter au chargement** : cluster ~30 frames à 55-103ms (10-18 FPS) t=103-106s + spikes sporadiques. `forgia2_lag_events.json` warn (13/30s, 360 total).
- 🔴 **Cause racine** : `forgia_chunk_stream.json` → `"mode":"synchronous"` — génération chunk + spawn foliage **sur le thread principal** (async pipeline conçu mais jamais câblé, "P2").
- ⚠️ **VRAM = "N/A"** dans `forgia2_memory.json` (memory_sensor déclare wgpu incapable).
- 🐛 **Monitor F3 partiellement cassé** : `forgia-debug/snapshot.rs` lit `forgia2_perf.json::fps`/`frame_ms` alors que le sensor écrit `fps_smoothed`/`frame_time_avg_ms` → FPS/frame_ms = `n/a`. Idem chunk_stream (`chunks_loading` vs `counts.loaded`) + watchdog (`seconds_in_emergency` inexistant).

## Base V1 réutilisable (D:\Forgia — même Bevy 0.18.1)

- `forgia-game/src/debug/sensors/vram_breakdown.rs` — **VRAM estimée CPU-side** (itère `Assets<Image>`/`Assets<Mesh>`, `texture_descriptor` + `count_vertices`), **ne requête pas le driver** → portable verbatim, contourne le "wgpu N/A". Nomme les top offenders.
- `ship_overlay.rs` / `perf_history.rs` / `memory_budgets.rs` — références d'affichage.
- (Phase B) `terrain/streaming.rs` + `forgia-terrain/meshing.rs` + `config/genomes/async_pipeline_default.toml` — pipeline async complet AsyncComputeTaskPool data-driven.

## Phase A — Livrables (cette story)

1. **Port VRAM** : `forgia-observability/src/vram_sensor.rs` → `forgia2_vram.json` (total estimé MB + top10 textures/meshes + shares + severity). Adapté V2 (Local timer 5s, serde derive).
2. **Fix mappings cassés** `forgia-debug/snapshot.rs` : perf (fps_smoothed/frame_time_avg_ms + frame_time_max_ms), chunk_stream (counts.loaded/pending + mode + gen_ms.p99 + foliage_coverage), watchdog (total/consecutive_lag_frames), vegetation (forgia_vegetation total_trees), lag (worst dt_ms + total).
3. **Enrichir catégorie System** (F3 → 1) : VRAM estimée + top offender, pire spike ms, lag total.
4. **Enrichir catégorie Terrain** (F3 → 4) : **mode streaming (⚠️ synchronous)**, gen_ms p99, foliage coverage (without_veg).

## Phase B — Plan (story suivante, à valider)

Câbler l'async streaming (AsyncComputeTaskPool) sur la génération chunk + spawn foliage time-budgété, base = V1 streaming.rs/meshing.rs + genome async_pipeline. Hot path, touche forgia-rpg/lib.rs + forgia-terrain. Enterprise.

## 🚨 Découverte runtime (user : "je vois rien que des gizmos")

Le monitor F3 **n'était JAMAIS branché dans le jeu** : `ForgiaDebugPlugin` (crate forgia-debug, story-547) n'était ajouté nulle part — aucun crate n'en dépendait → code mort. Mes enrichissements catégories étaient invisibles. Pire, en RPG **F3 = `toggle_streaming_overlay`** (gizmos grille chunks de forgia-rpg) → l'user appuyant F3 ne voyait QUE ces gizmos.

**Fix (3 étapes)** :
1. **Branché** `ForgiaDebugPlugin` : dep `forgia-game/Cargo.toml` + `app.add_plugins(...)` dans `forgia-game/src/lib.rs`.
2. **Rebind** master toggle **F3 → F2** (`bindings.rs`) → plus de collision avec les gizmos chunks RPG (qui gardent F3).
3. **Bug egui latent** : `draw_overlay_system`/`draw_console_system` tournaient en `Update` ; bevy_egui 0.39 exige `EguiPrimaryContextPass` (sinon ctx inactif → rien rendu). Déplacés. C'est pourquoi le monitor n'avait jamais marché même branché.

→ **Monitor = F2** désormais. Sensors JSON (forgia-observability) toujours OK indépendamment.

## Auto-QA (post-impl, story Standard)

- **qa-lead** : WARN → 2 défauts mineurs corrigés.
  - BUG-581-01 (timer `elapsed_secs==0` edge-case headless) → **fixé** : pattern accumulateur delta (aligné memory/perf_sensor).
  - BUG-581-02 (conformité observability) → **fixé partiel** : `forgia2_vram.json` ajouté à `rpg_monitor.toml::expected_sensors` (CHK-5 alerte si absent). Dette assumée : pas de gènes genome pour seuils 2048/4096 MB — **cohérent** avec memory_sensor/perf_sensor (même vague story-467, mêmes consts Rust).
- **verifier** : checks mécaniques verts côté implémenteur (clippy 0, 6 tests vram, check -p forgia OK).

## AC Phase A

- [x] cargo check + clippy 0 warning + 6 tests vram verts + check -p forgia OK
- [x] Auto-QA sous-agents (qa-lead WARN → corrigé)
- [ ] `forgia2_vram.json` écrit avec total + top offenders (jeu lancé) — **runtime, rebuild requis**
- [ ] F3 → System affiche FPS/frame_ms réels (plus de n/a) + VRAM + pire spike — **runtime**
- [ ] F3 → Terrain affiche mode "synchronous" + gen p99 — **runtime**
- [ ] Runtime : F3 au spawn → métriques cohérentes vs sensors JSON — **runtime**
