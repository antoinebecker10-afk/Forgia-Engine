# Story-556 — Bug "joueur bloqué" investigation + fix KCC

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_player_state.json`, fichier `lib.rs`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT (à investiguer en runtime, attendre push autre terminal)
> **Scale BMAD** : Standard
> **Effort estimé** : ~half-day investigation + ~1h fix
> **Prérequis** : autre terminal push forgia-player WIP (15 fichiers modifiés)

## Symptôme

Mode Roguelite Crypts of Anvil, joueur se bloque régulièrement (`stuck_events_session: 21` en 70s session). Sensor `forgia2_player_state.json` :

```json
{
  "position": [45.738, 1.198, 17.941],
  "velocity_planar_m_s": 0.0,
  "velocity_y_m_s": 0.0,
  "grounded": true,
  "kcc_collisions": 0,
  "stuck_frames_consecutive": 44,
  "stuck_events_session": 21,
  "frames_observed": 21993
}
```

**Indice clé** : `kcc_collisions: 0` malgré `stuck_frames_consecutive: 44` → **collision silencieuse non-rapportée**, pattern "penetration creep" (cf story-540 fix offset 0.05).

## Hypothèses (par diag agent 2026-05-28)

| # | Cause | Confiance | Évidence |
|---|---|---|---|
| H1 | Penetration creep KCC vs prop trimesh GLB | 75% | offset 0.05 + 164 props Crypts + Y=1.198 = enfoui |
| H2 | snap_to_ground 0.4m tire dans sol incliné | 60% | snap > offset → recover trop fort |
| H3 | Autostep bloqué sur prop dynamic (exclude_dynamic_bodies=false) | 50% | Anvil/Column = static donc OK normalement |
| H4 | Code récent broken (autre terminal 15 files WIP) | 15% | aucune évidence diff |

## Acceptance Criteria

- [ ] AC1 — Investigation runtime : reproduire stuck en RPG vs Roguelite, comparer events sensor
- [ ] AC2 — Identifier prop coupable : ajouter sensor temporaire qui log nearest_prop_distance quand stuck event fire
- [ ] AC3 — Tester fix candidates (bisection) : offset 0.05→0.1 OU snap 0.4→0.15 OU trimesh→cuboid pour 1 prop type
- [ ] AC4 — Mesure : stuck_events_session/min divisé par >5 vs baseline
- [ ] AC5 — Story-540 reference memory mise à jour avec nouveau finding

## Files candidats (à confirmer pendant impl)

- `crates/forgia-player/src/lib.rs` — KCC config (~ligne 144 selon agent diag)
- `crates/forgia-stage/src/lib.rs` — prop collider config (trimesh vs cuboid)
- Sensor temporaire `forgia2_stuck_diag.json` (nearest prop, position trace)

## Non-goals

- Fix de masse colliders props (story dédiée si décidé)
- Modifier autre terminal WIP

## Cross-refs

- Memory : [story-540 KCC fix](C:/Users/Antoi/.claude/projects/d--Forgia/memory/) (offset 0.05m)
- Sensor : `forgia2_player_state.json` (stuck counters)
- Diag agent run : conversation session 2026-05-28 PM
