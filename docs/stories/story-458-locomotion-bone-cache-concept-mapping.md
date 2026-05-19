# Story-458 — Concept mapping `locomotion-bone-cache`

**Statut** : DONE 2026-05-19 (hygiène doc).
**BMAD scale** : Quick (1 fichier doc).
**Vague** : V2 — Discipline & traçabilité.

---

## Contexte

V2 reprend le protocole **concept-first.md §6** depuis V1 (`D:/Forgia/.claude/rules/concept-first.md`). Le bug runtime du 2026-05-18 ("38 erreurs LocomotionBoneCache" + race `proc_walk_cycle` rigless) a montré qu'il manquait une entrée concept dédiée à la cache BFS bones partagée par 4 systèmes de `forgia-rpg::character.rs`.

V2 n'a pas (encore) son propre `concept-first.md`. Cette story documente la cartographie inline en attendant la création éventuelle d'un `concept-map.md` côté V2.

---

## Cartographie — `locomotion-bone-cache`

| Champ | Valeur |
|---|---|
| **Mots à grep** | `LocomotionBoneCache`, `proc_walk_cycle`, `proc_body_anim`, `init_locomotion_bone_cache`, `cache_ready` |
| **Layer** | `fw` (framework Rust V2-only, pas de genome) |
| **Producteur** | `crates/forgia-rpg/src/character.rs:758` `init_locomotion_bone_cache` — BFS one-shot sur skeleton GLTF post-spawn Rex (frame jusqu'à `cache.ready = true`) |
| **Consommateurs** | `character.rs:1243` `proc_walk_cycle` (frame, hot) ; `character.rs:902` debug telemetry (1Hz) ; `character.rs:932` debug overlay (1Hz) ; `character.rs:212` + `:662` insert sur spawn Rex |
| **Sensor** | `forgia_anim_layer.json` (champ `cache_ready` exposé) |
| **Hot** | `*` — `proc_walk_cycle` itère bones par frame |
| **Net** | `L` (local, anim cosmétique) |
| **Script** | `int` |

### Invariants

1. `init_locomotion_bone_cache` est idempotent : la BFS s'arrête après population (`cache.ready = true`).
2. Tout consumer DOIT gate sur `cache.ready == true` ou skip silencieusement. Sinon : Transform écrit à `(0,0,0)` → Rex T-pose statique (race observée 2026-05-16 NUIT).
3. `LocomotionBoneCache` est inséré SUR `RexCharacter` post-spawn ; si l'entité est absente quand `proc_walk_cycle` tourne → query vide, no-op (pas de panic).

### Failure mode canonique

> Symptom : Rex statique malgré bones rotated. Cause : `cache.ready = false` (BFS pas encore complète) OU bones names absents du skeleton.
> Diagnostic : lire `forgia_anim_layer.json` champ `cache_ready` AVANT d'éditer character.rs.

---

## Liens

- Memory pattern : `reference_forgia_rig_topology_3d_classification.md`
- Memory pattern : `reference_rigless_proc_anim_pattern.md`
- Memory pattern : `feedback_cross_plugin_onenter_race_pattern.md`
- Règle origine : `D:/Forgia/.claude/rules/concept-first.md` §6 (V1)

## Acceptance

- [x] Mapping producteur/consommateurs/timing/sensor/hot/net/script écrit
- [x] Invariants nommés (3 items)
- [x] Failure mode + diagnostic décrits
- [x] Story marquée DONE dans ROADMAP V2
