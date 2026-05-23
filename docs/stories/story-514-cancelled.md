# Story-514 — INVALIDATED (forgia-core split god-object)

**Status** : INVALIDATED
**Origin** : story-512 §10 follow-up
**Invalidated** : 2026-05-23
**Reason** : premisse fausse — `forgia-core` n'est pas un god-object

---

## Plan initial (rejette)

Story-512 §10 listait :
> **story-514** : Vague 3 — split `forgia-core` god-object 239 incoming → `forgia-prelude` + `forgia-core` (BMAD Enterprise, session dédiée, plan mode)

Le terme "god-object 239 incoming" venait de l'audit workspace 2026-05-23 (`docs/audit/crates-maturity-audit-2026-05-19.md` + audit Explore sub-agent).

## Verite terrain (mesuree 2026-05-23 PM)

| Metrique | Audit predit | Verite mesuree | Source |
|---|---|---|---|
| LOC totales `forgia-core` | "god-object" | **121 LOC** dans `lib.rs` unique | `wc -l crates/forgia-core/src/lib.rs` |
| Fichiers | "complexe" | **1 seul fichier** | `ls crates/forgia-core/src/` |
| Incoming (consumers) | 239 | **189** (` grep -rl forgia.core crates/`) | encore eleve mais pas god-object |
| Contenu | "monolithe" | 3 States + 1 SystemSet + 1 Plugin | `lib.rs:8-121` |
| Pattern prelude | absent | DEJA present `pub mod prelude` | `lib.rs:12-16` |

## Analyse

Le terme "god-object" caracterise un objet **avec beaucoup de responsabilites disparates**. `forgia-core` a UNE responsabilite : foundation canonique du workspace (States + SystemSet + Plugin agregateur). C'est la definition meme d'une **prelude crate saine** (cf Bevy `bevy_app`, Tokio `tokio-core`).

Les 189 incoming reflectent **"tout le monde depend de la foundation"** — c'est sain, pas pathologique. Splitter une fondation de 121 LOC ajouterait :

- 1 nouvelle crate (vs -99 cumulees story-512+513)
- Complexite path (`use forgia_core::prelude::GameSet` -> `use forgia_prelude::GameSet`)
- 0 gain mesure (la "douleur" theorique n'existe pas)

Violations directes de :
- `CLAUDE.md §6` "INTERDIT sur-ingenierer"
- `.claude/rules/no-speculative-fix.md` "ne touche pas a ce qui fonctionne"
- `MEMORY.md feedback_streaming_already_mature_dont_recreate.md` (analogie : crate mature, ne pas recreer)

## Lesson learned (capitalisable)

**Failure mode audit** : "high incoming count" != "god-object". L'audit doit distinguer :

| Pattern | Signal | Action |
|---|---|---|
| Foundation crate (States/SystemSet) | high incoming, low LOC, peu de responsabilites | sain — ne pas toucher |
| God-object reel | high incoming, **haut LOC** (>1000), responsabilites disparates | candidate split |
| Hub fragile | incoming + outgoing eleves, churn frequent | candidate stabilisation |

Le seul indicateur "incoming > 150" n'est PAS suffisant pour declencher un split. Croiser avec LOC + nombre de responsabilites distinctes.

## Decision

**Story-514 ANNULEE.** `forgia-core` reste tel quel (121 LOC, 3 States + GameSet + Plugin + prelude).

Future story candidate **uniquement si** :
- LOC `forgia-core` depasse 500 (signal complexite)
- OU des consumers se plaignent d'un import couteux (re-build cycle, type bloat)
- OU une responsabilite nouvelle apparait (audio events ? input events ?) qui justifie un module separe

## Suite

- **story-515** : xtask `story-gate` ratchet anti-stub (preserver gains story-512+513)
- Audit reviewer (futur) : mecanisme distinguer foundation crate vs god-object reel
