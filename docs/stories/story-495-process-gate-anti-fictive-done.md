# Story-495 — Process gate anti-fictive-DONE

**Status:** DRAFT
**Scale:** BMAD Standard (~5-6 fichiers : 1 script gate + 1 rule + 1 hook + .bmad config + checklist update)
**Created:** 2026-05-21
**Blocks:** Confiance dans le statut des stories · validité des memories capitalization · audits utiles
**Related:** audit `docs/audit/audit-rpg-roguelite-2026-05-21.md` §9 R10 · memory `reference_v7_p0_session_2026_05_20.md` (fictif)

---

## 1. Contexte — la découverte

Audit 2026-05-21 a révélé que **9 stories (471-479) étaient marquées DONE** avec claims précis (tests verts, LOC livrées, clippy clean) **alors que la réalité du code était** :

| Story | Statut affiché | Crate code réel | Branche |
|---|---|---|---|
| 471 analytics-sentry | DONE 8/8 + 12 tests | 16 LOC scaffold inchangé depuis V2 bootstrap | story.md untracked |
| 472 voicelines-tier1 | DONE 22 tests | 16 LOC scaffold | story.md untracked |
| 473 stage-graph | DONE 10/10 + 24 tests | 875 LOC WIP **mais crate untracked** | tout `??` |
| 475 equipment | DONE 8/8 + 12 tests | 16 LOC scaffold | story.md untracked |
| 476 status-effects | DONE | 16 LOC scaffold | story.md untracked |
| 477 audio-music-state | DONE 12/12 | 16 LOC scaffold | story.md untracked |
| 478 audio-ducking | DONE | 16 LOC scaffold | story.md untracked |
| 479 scene-saves | DONE | crate sans src/ | story.md untracked |

Le memory `reference_v7_p0_session_2026_05_20.md` capitalisait "**8 P0 livrées, 148 tests, ~3700 LOC, 0 clippy**" comme intelligence cumulative — **fiction quasi-totale**.

## 2. Cause racine

Le rituel post-impl (`.bmad/checklists/post-implementation.md`) valide :
- `cargo check` ✅ (mais une crate scaffold 16 LOC compile trivialement)
- `cargo clippy` ✅ (16 LOC = 0 warning facilement)
- Story status mis à DONE manuellement

**Aucun gate ne vérifie** :
- (a) que la crate fait > N LOC réelles (au-delà du scaffold V2 bootstrap)
- (b) que les tests prétendus existent (`#[test]` count)
- (c) que les fichiers story sont sur une branche **commitée** (vs untracked `??`)
- (d) que le code prétendu livré est **commité** (vs WIP perdu si autre terminal écrase)
- (e) que la memory capitalization a un cross-check vs git log

Conséquence : **un agent peut self-report DONE en toute bonne foi**, et le système n'a aucun moyen de détecter la divergence.

## 3. Goals

1. **Script de validation `xtask story-gate`** : pour une story-NNN, vérifie cohérence claim vs code/git
2. **Hook PreToolUse** : bloque tout passage `Status: DONE` si gate échoue
3. **Rule `.claude/rules/story-done-gate.md`** : règle bloquante avant marquer DONE
4. **Audit rétroactif** : passe le gate sur toutes les stories DONE actuelles, génère rapport
5. **Mise à jour `.bmad/checklists/post-implementation.md`** : ajoute item gate obligatoire

## 4. Non-Goals

- Re-coder les 7 stories fictives — ce sera fait via leurs propres stories (cf #491+)
- Vérifier la qualité runtime des stories (ça reste hors scope automatable)
- Auditer toutes les memories (focus sur stories d'abord, memories phase 2)

## 5. Acceptance Criteria

- [ ] AC1 — `xtask/src/bin/story_gate.rs` créé : commande `cargo xtask story-gate <story-id>` ou `--all-done`
- [ ] AC2 — Gate vérifie pour chaque story DONE :
  - **G1 git-tracked** : fichier `docs/stories/story-NNN-*.md` est dans `git ls-files` (pas `??`)
  - **G2 git-committed** : dernier commit touchant story < 90j ET au moins 1 commit dans `HEAD`
  - **G3 crate-loc** : si story mentionne `crate forgia-X`, alors `wc -l crates/forgia-X/src/**/*.rs` > 50 LOC (scaffold V2 = 16)
  - **G4 tests-count** : si story claim "N tests verts", alors `grep -r "#\[test\]" crates/forgia-X/src/` retourne ≥ N hits
  - **G5 ac-checkboxes** : tous les `- [x]` de la section "Acceptance Criteria" cochés
  - **G6 cross-check memory** : si une memory `reference_*` mentionne cette story et claim "DONE", elle pointe vers fichier code réel (pas scaffold)
- [ ] AC3 — `cargo xtask story-gate --all-done` génère `docs/audit/story-gate-YYYY-MM-DD.md` avec status par story
- [ ] AC4 — Hook PreToolUse `validate-story-done.sh` lit toute Edit/Write sur `docs/stories/*.md` — si diff contient `Status: DONE` mais le gate échoue, bloque
- [ ] AC5 — Rule `.claude/rules/story-done-gate.md` créée : règle bloquante (~80 lignes), origine documentée
- [ ] AC6 — Passe gate sur stories 441/447/452/453/483/485/486 (RPG/Roguelite DONE) → toutes vertes, sinon ajuster gate
- [ ] AC7 — Rapport rétroactif livré : table de toutes les stories DONE actuelles + résultat gate + classification (FICTIVE / WIP / OK)
- [ ] AC8 — `.bmad/checklists/post-implementation.md` mis à jour : item "Story-gate passe (`cargo xtask story-gate <id>` vert)"
- [ ] AC9 — Test sur stories 471-479 → toutes FAIL G1 (untracked) ou G3 (scaffold) — confirme détection
- [ ] AC10 — `cargo check + clippy` du xtask propre

## 6. Architecture & Patterns

```
xtask/src/bin/story_gate.rs
├── parse story.md (status, AC, crate mentioned, tests claimed)
├── G1-G6 checks (git ls-files, wc -l, grep, etc.)
└── output: pass/fail per gate + reason

.claude/hooks/validate-story-done.sh
├── trigger PreToolUse Edit/Write docs/stories/*.md
├── if new content contains "Status: DONE"
└── runs cargo xtask story-gate <id> → exit 1 if fail
```

**Pattern source de vérité** : git tracking + crate LOC + test count sont **mécaniquement vérifiables**. Pas de subjectivité. Le gate ne juge pas la qualité — juste l'existence.

## 7. Files Touchés (estim)

- `xtask/src/bin/story_gate.rs` (nouveau, ~250 LOC)
- `xtask/Cargo.toml` (déjà existe ? sinon créer)
- `.claude/hooks/validate-story-done.sh` (nouveau)
- `.claude/rules/story-done-gate.md` (nouveau)
- `.bmad/checklists/post-implementation.md` (1 item ajouté)
- `docs/audit/story-gate-2026-05-21.md` (rapport rétroactif)

## 8. Risques

- Faux positifs G3 sur crates volontairement minces (utility/proxy crates) → seuil paramétrable + allowlist `.claude/config/story-gate-skip.txt`
- Hook peut être contourné — règle complémentaire dans CLAUDE.md "ne jamais skip hook story-done-gate"
- Couts CI léger (gate sur all-done = O(stories)) — < 30s estimé

## 9. Suite (après merge)

- Story-496 : audit memories capitalization — passes gate similaire mais sur memory files cross-check vs code claim
- Story-497 : intégration GitHub Actions — gate sur PR avant merge (pré-merge CI vs hook local)
