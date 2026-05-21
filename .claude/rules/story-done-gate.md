# Story-Done Gate (Forgia) — RÈGLE BLOQUANTE

> **Avant de marquer une story `Status: DONE` ou `✅ DONE`, le gate `cargo run -p xtask -- story-gate --story <id>` DOIT passer vert.**

Origine : 2026-05-21, audit RPG/Roguelite a révélé que 7/9 stories du batch 471-479 étaient **DONE fictives** (claims "N tests verts, M LOC peuplé" mais crates restées scaffold 16 LOC, stories untracked). Memory `reference_v7_p0_session_2026_05_20.md` invalidé. Story-495 a livré l'outil mécanique.

---

## 1. Pourquoi cette règle

Le rituel post-impl mécanique (`cargo check` + `cargo clippy`) **passe vert trivialement sur un scaffold 16 LOC** : il n'y a rien à compiler. Aucun gate du process ne vérifiait :

- que la story était sur une branche commitée (`git ls-files`)
- que la crate prétendue livrée avait plus que le code scaffold V2 bootstrap
- que les tests prétendus existaient
- que le memory capitalization (`reference_*_session_*.md`) avait un cross-check vs git

**Failure mode canonique** : `self-reported DONE without mechanical gate`. Cohérent avec arXiv 2604.02547 (agents qui confirment leur succès au lieu de le falsifier).

---

## 2. Quand l'appliquer

**Obligatoire** dès qu'une story passe à :

- `Status: DONE`
- `✅ DONE YYYY-MM-DD`
- `**Statut** : ✅ DONE`
- ou équivalent dans le frontmatter / premières lignes

**Aussi obligatoire** avant :

- Écrire un memory `reference_*_session_*.md` qui capitalise des claims chiffrés (N tests / M LOC / 0 clippy)
- Citer la story dans un plan suiveur ("après story-NNN DONE...")
- Faire dépendre une story dans son champ `Related:` / `Blocks:` sans avertissement

**Skippable** seulement si la story est sur la `STORY_GATE_SKIP_LIST` de `xtask/src/main.rs` (orchestration / multi-crate / docs-only).

---

## 3. Les 3 gates appliqués automatiquement

| Gate | Vérifie | Source de vérité |
|---|---|---|
| **G1 git-tracked** | `git ls-files --error-unmatch docs/stories/story-NNN-*.md` retourne le fichier | git index |
| **G3 crate-LOC** | Si la story mentionne `forgia-X`, alors `crates/forgia-X/src/**/*.rs` totalise > 50 LOC (seuil = scaffold V2 = 16 + marge) | filesystem |
| **G4 tests-count** | Si la story claim "N tests verts" dans son header, alors `grep '#[test]'` retourne ≥ N hits | filesystem |

**Reportés (phase 2)** :
- G2 commit récent : `git log -1 -- crates/forgia-X/` ≤ 90j
- G5 AC cochés : tous les `- [x]` dans la section Acceptance Criteria
- G6 memory cross-check : si `reference_*` cite la story, l'API qu'elle pointe existe vraiment

---

## 4. Commandes

```bash
# Gate sur une story unique
cargo run -p xtask -- story-gate --story 471

# Audit rétroactif complet (toutes les DONE actuelles)
cargo run -p xtask -- story-gate --all-done

# Exit code : 0 = pass, 1 = au moins une FAIL
```

Le rapport rétroactif initial est dans `docs/audit/story-gate-2026-05-21.md`.

---

## 5. Procédure quand le gate FAIL

1. **Ne PAS marquer DONE** — repasser le status à `DRAFT` ou `IN_PROGRESS`
2. **Ajouter un banner d'invalidation** en tête du fichier story (cf format adopté 2026-05-21 sur les 8 stories invalidées)
3. **Si le code existe mais untracked** (cas story-473 stage-graph) : coordonner commit avec autre terminal si conflit, sinon `git add` + commit
4. **Si la crate est scaffold** : la story reste DRAFT, dépendance refaite via une vraie implémentation

---

## 6. Skip list — quand exempter G3/G4

Stories d'orchestration (pas de crate dédiée, multi-crate, docs-only) sont exemptées G3 et G4 mais doivent toujours passer G1.

Pour ajouter une story à la skip list : éditer `STORY_GATE_SKIP_LIST` dans `xtask/src/main.rs` avec une note justifiant pourquoi la story ne mappe pas 1:1 sur une crate.

---

## 7. Anti-patterns à bannir

- ❌ Marquer `Status: DONE` sans courir le gate au préalable
- ❌ Ajouter une story au skip list pour éviter G3 alors qu'elle a vraiment une crate dédiée
- ❌ Écrire un memory `reference_*_session_*.md` chiffré sans vérifier les chiffres via `xtask story-gate`
- ❌ Citer une story DONE dans un plan sans cross-check qu'elle l'est vraiment
- ❌ Désactiver le gate en CI parce qu'il fait du bruit
- ❌ Considérer G4 actual < claim comme un bug du gate plutôt qu'un signal sur la story

---

## 8. Sources externes

- **arXiv 2604.02547** — symptom fixation / self-confirmation failure mode
- **Trunk-Based Development (Paul Hammant)** — git index comme source de vérité opérationnelle (story files = source code de la planification)
- **Anthropic "Building Effective Agents"** — vérification mécanique > self-report
- **`.claude/rules/multi-terminal-coordination.md`** §5 — étendu : commit = preuve, comme binaire = preuve

---

## 9. Cross-refs

- `xtask/src/main.rs` — implémentation `story_gate` (~250 LOC)
- `docs/stories/story-495-process-gate-anti-fictive-done.md` — story qui a livré la règle
- `feedback_fictive_done_status_2026_05_21.md` (memory) — feedback origine
- `.bmad/checklists/post-implementation.md` — item "Story-gate passe" ajouté

---

*Adoptée 2026-05-21 par story-495. La règle est mécanique et non négociable : `xtask story-gate` est la seule source d'autorité pour "DONE means DONE".*