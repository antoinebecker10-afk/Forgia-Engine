# Story-583 — Budget par frame du spawn de foliage (anti-stutter chargement)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : trace partielle** (fichier `lib.rs`) — une partie de ce
> qu'elle décrit existe, le reste n'a pas été retrouvé.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> (Renuméroté 582→583 : 582 pris par story-582-weapon-elements de l'autre terminal.)
> **État d'origine (périmé, cf bandeau)** : EN COURS. **Scale** : Quick (forgia-foliage/src/lib.rs, 1 fichier).
**Date** : 2026-06-07
**Lignée** : Phase B reciblée du chantier loading. Le monitor story-581 a prouvé que le stutter (cluster 55-103ms) n'est PAS la génération de chunks (`gen_ms p99=1.0ms`) mais le **spawn de foliage non budgété**.

## Cause racine (prouvée, pas spéculée)

`populate_new_chunks` (forgia-foliage/lib.rs:198) itère **TOUS** les chunks non-peuplés (`for ... in &q_chunks`, ligne 223) **dans une seule frame**, spawn jusqu'à `target` arbres/chunk (SceneRoot GLB + Collider compound + matériau + bark override). Quand plusieurs chunks deviennent prêts simultanément (spawn monde, traversée région) → **rafale de spawn d'entités sur 1 frame** = spike 55-103ms.

Preuves : code (boucle sans cap) + sensors (gen 1ms ≠ cause ; 14 200 entités ; `forgia2_lag_events.next_step` pointe foliage ; cluster t≈103 = près du spawn = chunks peuplés en masse au boot).

## Fix

Time-budget : ne peupler que `chunks_per_frame` chunks par frame, le reste repasse à la frame suivante (les chunks non-peuplés restent dans `q_chunks`, re-query chaque frame). Pattern = colliders incrémentaux ithappy (story session 2026-06-06).

**Budget configurable (pas de hardcode, rule scalability)** : réutilise `StreamingConfig.async_pipeline.chunks_per_frame` (déjà dans `config/genomes/streaming.toml`, genome-driven + hot-reload Shift+F12, défaut 2) — knob conçu pour ça mais jamais consommé. forgia-streaming est déjà une dép de forgia-foliage.

## Implémentation

`populate_new_chunks` : + param `streaming_cfg: Option<Res<forgia_streaming::StreamingConfig>>`, compteur `populated_this_frame`, gate `if populated_this_frame >= budget { break; }` après le skip `contains_key` (gratuit) et avant le travail coûteux ; `+= 1` après `veg.chunk_entities.insert`.

## Validation (before/after via monitor story-581)

- AVANT : cluster lag 55-103ms au spawn/mouvement (vu t=103-106).
- APRÈS : `forgia2_lag_events.events_last_30s` doit chuter, `frame_time_max_ms` rester < ~16ms en traversée. Pop-in foliage progressif acceptable (si visible, bump `chunks_per_frame` en hot-reload).

## AC

- [ ] cargo check + clippy 0 warning
- [ ] Runtime : traversée rapide → plus de spike > 30ms corrélé chunk load ; foliage se remplit en ~quelques frames
- [ ] Budget hot-reloadable (Shift+F12 streaming.toml)
