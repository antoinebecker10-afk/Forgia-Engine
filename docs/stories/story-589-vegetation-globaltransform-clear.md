# Story-589 — Fix régression végétation RPG invisible (clear lit GlobalTransform non-propagé)

> **Statut** : DONE (2026-06-09), validé runtime. NON COMMITÉ.
> **Scope BMAD** : Standard (2 crates : forgia-rpg + forgia-foliage). Bug B-régression de l'audit végé.
> **Origine** : [audit-2026-06-09-vegetation-invisible-regression.md](../audit/audit-2026-06-09-vegetation-invisible-regression.md).

## Problème

En mode RPG, **aucune végétation visible** (arbres/biome) même loin du village. Les arbres spawnaient (compteur `total_trees` montait) mais `live_entities` (query live) = **0**.

## Cause racine

`sys_clear_village_foliage` (`forgia-rpg/src/worldgen_village.rs`, ajouté par commit 28df37d, **jamais exécuté avant le rebuild** = binaire stale) lisait le **`GlobalTransform`** des arbres pour les clear dans le disque village (50m). Les arbres sont spawnés dans le **même Update** ; la propagation `GlobalTransform` tourne en **PostUpdate** → la frame du spawn, `GlobalTransform` = identité `(0,0,0)`. Distance `(0,0)`→centre village `(16,16)` = **22.6m < 50.2m** → le clear croyait que **chaque arbre était dans le village** → **despawn de tous les arbres chaque frame avant rendu**.

## Fix

`sys_clear_village_foliage` : `Query<(Entity, &Transform), ...>` au lieu de `&GlobalTransform`. Les arbres sont des entités racine → `Transform.translation` = position monde correcte immédiatement (cohérent avec `spawn_village_paths_when_loaded` qui utilisait déjà `Transform`). 1 ligne de signature + 2 d'usage.

## Observabilité (ajoutée, permanente)

`forgia_vegetation.json → live_diag {live_entities, instantiated, min/max_dist_excl_m, inside_excl, excl_radius_m}` — vrai compte d'entités **vivantes** (query). Le `total_trees` historique est un compteur de spawn **cumulatif** (jamais décrémenté par le clear) qui a induit le diagnostic en erreur. `forgia-foliage/src/lib.rs::write_vegetation_sensor`.

## Critères d'acceptation

- AC1 — `live_diag.live_entities` > 0 en RPG. ✅ runtime (943).
- AC2 — Arbres visibles autour du village (hors 50m) + collines. ✅ runtime (user "c'est revenu").
- AC3 — `cleared_total` ne despawn que les vrais bords (≈35, pas ≈1358). ✅ runtime.
- AC4 — `cargo check -p forgia` + clippy 0 (forgia-rpg + forgia-foliage). ✅
- AC5 — Instrumentation de debug (spawn_rings/peak/sensor village_clear/compteurs despawn) retirée. ✅

## Leçon

Compteur cumulatif ≠ query live : ne jamais déduire la présence d'entités d'un compteur incrément-only. + [[feedback_unvalidated_wip_detonates_on_rebuild]] (WIP committé jamais run → explose au rebuild).
