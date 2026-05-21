# Story-Gate audit rétroactif — 2026-05-21

> Premier passage du gate `xtask story-gate --all-done` sur toutes les stories Forgia V2.
> Livré par story-495.

## Résumé (révision 2 — 2026-05-21 soir)

**12 stories DONE échouent au moins un gate après ajustements (heuristique tests + skip-list multi-crate).**

| Catégorie | Count | Stories |
|---|---|---|
| 🚨 Fictives — code n'existe pas (G1 FAIL + G3 FAIL + G4 FAIL) | 7 | 471, 472, 475, 476, 477, 478, 479 |
| 🟠 WIP non commit (G1 FAIL mais G3/G4 PASS) | 1 | 473 stage-graph (875 LOC, 35 tests) |
| 🟡 Tracked mais dep crate vide (G3 FAIL via dépendance) | 1 | 481 (forgia-audio-voicelines vide → cascade-invalidée) |
| ⚠️ Untracked + claims fictifs (pattern 471-479) | 1 | 482 (cascade-invalidée) |
| 🟠 Untracked nouveau (créées ce jour, OK après commit) | 1 | 495 |

### Corrections rev 2

- **story-466** n'est plus flaggée — heuristique `claimed_tests` resserrée (require "N/N tests" ou "N tests verts|passing|livrés" explicite, plus de match sur nombre isolé proche du mot "test")
- **story-483** ajoutée à `STORY_GATE_SKIP_LIST` (multi-crate : forgia-mode-roguelite 38 tests + forgia-stage-arena 58 tests = 96 ≥ 88 claim, gate single-crate sous-comptait)
- **stories 481 + 482** cascade-invalidées avec banner DRAFT
- Audit `[[reference-v7-p0-session-2026-05-20]]` invalidé en mémoire

---

## Détail — par catégorie

### 🚨 Fictives — code intact depuis V2 bootstrap

Ces 7 stories ont été marquées DONE 2026-05-20 mais les crates n'ont jamais été peuplées :

| Story | Crate | LOC | Tests claim | Tests actual |
|---|---|---|---|---|
| 471 analytics-sentry | forgia-analytics | 16 | 12 | 0 |
| 472 voicelines-tier1 | forgia-audio-voicelines | 16 | 22 | 0 |
| 475 equipment | forgia-equipment | 16 | 12 | 0 |
| 476 status-effects | forgia-status-effects | 16 | 19 | 0 |
| 477 audio-music-state | forgia-audio-music-state | 16 | 12 | 0 |
| 478 audio-ducking | forgia-audio-ducking | 16 | 17 | 0 |
| 479 scene-saves | forgia-scene | 16 | 17 | 0 |

**Status corrigé 2026-05-21** : DRAFT + banner invalidation appliqué sur chaque .md.

### 🟠 WIP non commit

| Story | État | Action |
|---|---|---|
| 473 stage-graph | Crate 875 LOC + 35 tests RÉELLES mais **dossier crate `??` entièrement untracked** | `git add crates/forgia-stage-graph/` + commit dès que coordination autre terminal OK |
| 495 process-gate (nouveau) | Story créée 2026-05-21, en cours d'add | Auto-résolu par commit final session |

### 🟡 Tests count inflated (code OK)

Ces stories ont du vrai code, mais les claims de tests sont supérieurs à la réalité grep `#[test]` :

| Story | Crate | LOC | Tests claim | Tests actual | Hypothèses |
|---|---|---|---|---|---|
| 466 death-event-observer-migration | forgia-damage | 282 | 12 | 3 | Migration partielle ? Tests promis mais retirés ? |
| 483 roguelite-stage-arena-foundations | forgia-mode-roguelite | 2212 | 88 | 38 | Le gate compte uniquement `#[test]` literal — peut-être 50 tests via `#[rstest]` / macros / `#[cfg(test)] mod tests` ? À investiguer. |

**Action recommandée** : audit individuel pour confirmer si claim inflated ou si le gate sous-compte (macros de test custom).

### 🟡 Story tracked mais dépendance crate scaffold

| Story | Issue |
|---|---|
| 481 audio-voicelines-tier1.5-wireup | Story.md tracked, mais `forgia-audio-voicelines` reste 16 LOC. La story décrit un wireup d'une crate non-peuplée. À cascade-invalider quand 472 sera vraiment livrée. |

### 🟠 Untracked + claims précis

| Story | Issue |
|---|---|
| 482 audio-voicelines-tier1.6-bark-text-overlay | Untracked + claim 30 tests sur crate scaffold. Symptôme identique au batch 471-479. À invalider. |

---

## Conséquences cumulées

1. **Memory invalidé** : `reference_v7_p0_session_2026_05_20.md` (claims 8 P0 / 148 tests / 3700 LOC quasi-fictifs)
2. **8 stories invalidées** avec banner DRAFT explicite (471, 472, 475, 476, 477, 478, 479 + 482)
3. **Trou process identifié** : aucun gate mécanique post-impl ne vérifiait l'existence du code derrière le statut DONE
4. **Outil livré** : `xtask story-gate` opérationnel, 3 gates (G1/G3/G4), CI-ready (exit 1 sur fail)

## Suite

- **Story-495 AC8** : ajouter item "Story-gate passe" dans `.bmad/checklists/post-implementation.md`
- **Story-495 AC4** : hook PreToolUse `validate-story-done.sh` — reporté (V2 workspace n'a pas de hooks dir, à wirer côté D:/Forgia ou via settings.local.json)
- **Story-496 candidate** : audit memories `reference_*_session_*.md` similaire (chiffres claim vs git)
- **Investiguer 466/483 G4 FAIL** : confirmer si tests claim inflated ou gate sous-compte (macros)

---

*Rapport généré 2026-05-21 par `cargo run -p xtask -- story-gate --all-done`. Reproductible à chaque modif.*
