# Story-491 — Workspace Re-compile : API bridge voicelines/loot/music/waves

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_health.json`, fichier `character.rs`, symbole `sys_apply_stage_toggles`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> ✅ **RÉSOLU 2026-05-21 plus simplement que prévu** : `cargo check --workspace` était déjà vert (les stubs no-op des fonctions `parse_music_state` / `sys_apply_stage_toggles` / `draw_portal_overlay` / `draw_bark_bubble` / `draw_stage_notification` sont définis IN-CRATE, pas références d'APIs cassées). L'audit avait extrapolé "workspace cassé" depuis les commentaires TODO, conclusion erronée.
>
> Seuls **5 warnings clippy `-D warnings`** bloquaient `cargo clippy --workspace` :
> - `forgia-rpg/character.rs:55` : empty_line_after_doc_comments (orphan doc après refacto story-482 P1)
> - `forgia-inventory/lib.rs:82,94` : explicit_iter_loop (2× `for x in iter_mut()`)
> - `forgia-weapon-hitscan/lib.rs:104` : useless_conversion (`(*xf.forward()).into()`)
> - `forgia-websocket/lib.rs:150` : useless_conversion (`out.into()`)
> - `forgia-asset-cdn/lib.rs:140` : derivable_impls (manual Default vs derive)
> - `xtask/main.rs:366,378` : manual_range_contains (2× `n >= 1 && n <= 500`)
>
> Tous fixés en BMAD-Quick. Workspace clippy clean. Reste pour vraie validation runtime :
> - Story-490 hits_with_damage > 0 in-game
> - Story-485 AC6/AC7 sensor `forgia2_stage_layout.json` runtime
>
> Voir commit `<a-venir>`.

**Status:** DRAFT
**Scale:** BMAD Standard (4 crates touchées, story requise, checklist post-impl)
**Created:** 2026-05-21
**Blocks:** AC6/AC7 story-485 runtime validation · story-490 sensor validation · toute nouvelle feature Roguelite testable in-game
**Related:** audit `docs/audit/audit-rpg-roguelite-2026-05-21.md` §6 action #1

---

## 1. Contexte

`cargo check -p forgia-mode-roguelite` échoue depuis commit `9e149ca` (refacto autre terminal en cours). Plusieurs APIs ont été supprimées/migrées sans réimplémentation, et `forgia-mode-roguelite` les référence toujours :

**Missing crates APIs** :
- `forgia_audio_voicelines::{BarkEvent, ActiveBark, BarkKind}` — crate wipée à 16 LOC
- `forgia_loot_tables::{LootTable, DropPool}` — refacto autre terminal
- `forgia_audio_music_state::MusicState` — supprimé
- `forgia_mode_roguelite::waves::{current_stage_node, composition_for_stage}` — supprimés intra-crate

**Conséquence** : `cargo run` lance silencieusement le binaire stale (cf `.claude/rules/multi-terminal-coordination.md` règle 5), AC runtime de toutes les stories récentes invalidés.

## 2. Goals

1. Remettre `cargo check -p forgia-mode-roguelite` au vert sans casser ce que l'autre terminal écrit
2. Choisir, par API, **stub no-op temporaire** vs **implémentation minimale** vs **suppression du call-site dormant**
3. Documenter chaque décision dans le code (commentaire `// story-491:` + ref story future si re-impl prévue)
4. Préserver le path validation runtime des stories 485/490

## 3. Non-Goals

- Réimplémenter complètement voicelines tier 1.5 → **story-481-reopen** ou nouvelle story dédiée
- Refacto V7 damage pipeline unifié → **story future**
- Toucher au code en cours d'écriture par l'autre terminal (claim avant Edit, cf règle 3)

## 4. Acceptance Criteria

- [ ] AC1 — Coordination explicite avec autre terminal : claim qui touche quoi (Resource/Event/system) avant 1er Edit
- [ ] AC2 — `forgia_audio_voicelines` exporte stubs publics `BarkEvent`, `ActiveBark`, `BarkKind` avec `Default` impl no-op (compile, 0 runtime effect)
- [ ] AC3 — `forgia_loot_tables` exporte stubs `LootTable`, `DropPool` no-op (le path drop pickup reste hardcodé dans `obs_roguelite_enemy_death`)
- [ ] AC4 — `forgia_audio_music_state` exporte stub `MusicState` (parse retourne `None`, état actuel)
- [ ] AC5 — `forgia-mode-roguelite::waves` re-implémente `current_stage_node(depth, variant)` et `composition_for_stage(stage_id)` OU le code dormant est supprimé proprement avec ref story future
- [ ] AC6 — `cargo check -p forgia-mode-roguelite -p forgia-audio-voicelines -p forgia-loot-tables -p forgia-audio-music-state` clean
- [ ] AC7 — `cargo check --workspace` clean
- [ ] AC8 — `cargo clippy -p forgia-mode-roguelite --no-deps -- -D warnings` 0 warning
- [ ] AC9 — `cargo run --profile release-fast` lance et entre en mode Roguelite sans crash
- [ ] AC10 — `forgia2_health.json` ne dégrade pas (reste `ok` ou améliore)

## 5. Architecture & Patterns

**Pattern stub no-op** :
```rust
// story-491: stub temporaire, ré-impl prévue story-XXX
#[derive(Event, Default)]
pub struct BarkEvent;

#[derive(Resource, Default)]
pub struct ActiveBark;
```

**Coordination multi-terminal** : appliquer strictement `.claude/rules/multi-terminal-coordination.md` règles 1-5. Vérifier `mtime(bin) > mtime(source) > mtime(sensor)` après chaque relance.

## 6. Files Touchés (estim)

- `crates/forgia-audio-voicelines/src/lib.rs` (stub re-add)
- `crates/forgia-loot-tables/src/lib.rs` (stub re-add ou peuple minimal)
- `crates/forgia-audio-music-state/src/lib.rs` (stub re-add)
- `crates/forgia-mode-roguelite/src/{waves.rs, run.rs, hud.rs, lib.rs}` (re-impl ou nettoie call-sites dormants)

## 7. Risques

- ⚠️ Conflit Edit garanti si autre terminal touche les mêmes fichiers — coordination obligatoire
- Stubs no-op risquent de masquer la régression UX (pas de bark audio, pas de music transition) — accepter pour débloquer ship V7, capitaliser en stories follow-up
