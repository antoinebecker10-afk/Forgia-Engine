# Story-493 — DamageEvent multi-observer ordering fix (BufferedEvent + .chain())

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `lib.rs`, symbole `DeathEvent`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

**Status:** DRAFT
**Scale:** BMAD Standard (~4-5 fichiers, story requise, checklist post-impl)
**Created:** 2026-05-21
**Blocks:** Reliabilité Roguelite coop (story-468 § BLOQUANT A2) · loot attribution correct
**Related:** memory `[[feedback-bevy-018-observer-ordering-not-guaranteed]]` · audit §6 action #7

---

## 1. Contexte

Bevy 0.18 documentation officielle : **l'ordre des Observers `On<Event>` n'est PAS garanti** quand plusieurs consumers écoutent le même Event (cf PR #19596).

Forgia a actuellement plusieurs consumers sur `DeathEvent` :
- `obs_roguelite_enemy_death` (loot pickup spawn) — forgia-mode-roguelite
- `obs_roguelite_player_death` (defeat trigger) — forgia-mode-roguelite
- `enemy_nameplate_cleanup` (UI removal) — forgia-fps ou forgia-ui
- Hit feedback (story-456 nameplate billboard floater)
- Killfeed counter
- Souls drop attribution (story future coop)

**Risque actuel** : ordre Observer non-déterministe → cas reproductible où nameplate cleanup spawn AVANT loot pickup, défi UX cohérent, et en coop la `source: None` du DeathEvent rendra l'attribution arbitraire entre clients.

**Décision audit story-468** : migrer multi-consumer `DamageEvent` / `DeathEvent` vers `BufferedEvent` (event reader-driven) + 3-5 systems `.chain()` ordonnés dans `GameSet::Effects`.

## 2. Goals

1. Identifier tous les consumers actuels de `DamageEvent` et `DeathEvent`
2. Choisir par event : garder `EntityEvent` (single-consumer ChildOf propagation type) vs migrer `BufferedEvent` (multi-consumer ordonné)
3. Migrer multi-consumer events vers `BufferedEvent` + systems `.chain()`
4. Documenter le contrat d'ordre dans le code (`// chain order: 1.dmg apply → 2.feedback → 3.killfeed → 4.cleanup`)
5. Préserver le path Observer pour les events vraiment single-consumer

## 3. Non-Goals

- Refactor complet vers `MessageReader` partout → out-of-scope, scope multi-consumer uniquement
- Networking coop attribution (sera consumed by story future)

## 4. Acceptance Criteria

- [ ] AC1 — Inventaire complet `DamageEvent`/`DeathEvent` consumers (grep + table dans story)
- [ ] AC2 — `DeathEvent` migré `EntityEvent` → `BufferedEvent` (decision si multi-consumer confirmé)
- [ ] AC3 — N systems `.chain()` dans `GameSet::Effects` : ordre documenté
- [ ] AC4 — Test pur : émettre 3 `DeathEvent` simultanés, vérifier ordre traitement (souls spawn AVANT nameplate cleanup AVANT killfeed update)
- [ ] AC5 — `cargo check + clippy` clean
- [ ] AC6 — Tests existants verts (89 stage-arena + tests fps + tests damage)
- [ ] AC7 — Runtime : tuer 3 ennemis avec hit chain → 3 pickups Souls visibles + 3 entries killfeed dans l'ordre
- [ ] AC8 — Sensor `forgia2_damage.json` ajoute `events_processed_this_tick` pour observabilité chain

## 5. Architecture & Patterns

```rust
#[derive(BufferedEvent)]
pub struct DeathEvent { pub target: Entity, pub source: Option<Entity> }

// GameSet::Effects chain (forgia-damage::plugin)
.add_systems(Update, (
    apply_damage,
    on_death_spawn_loot,    // story-490
    on_death_feedback_vfx,  // story-456
    on_death_killfeed,      // forgia-fps
    on_death_nameplate_cleanup,
).chain().in_set(GameSet::Effects))
```

## 6. Files Touchés (estim)

- `crates/forgia-damage/src/{lib.rs, plugin.rs}` (event type migration)
- `crates/forgia-mode-roguelite/src/run.rs` (Observer → MessageReader)
- `crates/forgia-fps/src/lib.rs` (death systems chain)
- `crates/forgia-ui/src/nameplate.rs` (cleanup system)

## 7. Risques

- `BufferedEvent` perd la propagation ChildOf automatique → si certains use-cases en dépendent, garder dual path
- Ordre `.chain()` rigide — toute story future ajoutant un consumer devra updater l'ordre explicite (documenter dans `.claude/rules/`)
