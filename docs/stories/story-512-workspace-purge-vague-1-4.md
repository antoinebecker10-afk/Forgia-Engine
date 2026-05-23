# Story-512 — Workspace Purge Vagues 1 & 4 (cleanup stubs + modes inutilisés)

**Status** : DRAFT
**BMAD Scale** : Standard (>3 fichiers mais bounded scope, suppression sèche)
**Created** : 2026-05-23
**Owner** : Antoine + Claude
**Branch** : `cleanup/workspace-purge-vague-1-4`

---

## 1. Contexte & motivation

Audit workspace V2 du 2026-05-23 :

- **264 crates pour 57 735 LOC** (ratio 218 LOC/crate vs industrie 1500–5000)
- **145 scaffolds <50 LOC (54 %)**, dont **41 crates 100 % stubs** sans aucune implémentation
- **6 modes scaffolds** jamais implémentés (platformer, puzzle, race, rpg-openworld, rts, survival)
- Sources : [matklad large workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html), [Bevy crates/](https://github.com/bevyengine/bevy/tree/main/crates), [users.rust-lang.org](https://users.rust-lang.org/t/why-are-most-crates-1000-loc/83380)

**Problème mesurable** : chaque stub est un piège à hallucination IA (cf. `MEMORY.md feedback_fictive_done_status_2026_05_21.md`) — l'agent voit `forgia-render-bloom` dans le workspace, suppose qu'un système existe, génère du code qui appelle des APIs fantômes, marque des stories "DONE" sur 16 LOC.

**Cette story = suppression sèche.** Aucune ligne de code utile n'est perdue : les crates concernées contiennent au mieux un `Plugin` vide avec `// TODO`.

---

## 2. Scope exact

### Vague 1 — Stubs sans implémentation (49 crates)

| Cluster | Préfixe | Count | LOC total | Justification suppression |
|---|---|---|---|---|
| Editor | `forgia-editor-*` | 16 | 256 | Pas d'éditeur V1, Bevy egui suffit |
| Render | `forgia-render-*` | 12 | 192 | `bevy::core_pipeline` fournit déjà bloom/ssao/skybox |
| QA | `forgia-qa-*` | 8 | 128 | V1 a déjà QA mature (`d:\Forgia`), V2 stubs jamais branchés |
| Net | `forgia-net-*` | 7 | 112 | `lightyear` workspace dep, crate `forgia-net` à recréer quand multi démarre |
| Script | `forgia-script-*` | 4 | 64 | Scripting Luau pas dans scope V1 ship |
| Import | `forgia-import-*` | 2 | 32 | Pas d'imports actifs |

**Sous-total Vague 1 : 49 crates, 784 LOC de TODO**

### Vague 4 — Modes inutilisés (6 crates)

| Crate | LOC | Statut |
|---|---|---|
| `forgia-mode-platformer` | <50 | stub |
| `forgia-mode-puzzle` | <50 | stub |
| `forgia-mode-race` | <50 | stub |
| `forgia-mode-rpg-openworld` | <50 | stub (RPG vit dans `forgia-rpg`) |
| `forgia-mode-rts` | <50 | stub |
| `forgia-mode-survival` | <50 | stub |

**Sous-total Vague 4 : 6 crates**

### Total story-512

**55 crates supprimées · 264 → 209 crates · ratio 218 → 276 LOC/crate** (mécaniquement, comme on supprime des crates à 16 LOC).

---

## 3. Out of scope (autres vagues)

- **Vague 2** (fusion 45 `forgia-pp-*` en 1 crate `forgia-postprocess`) → story-513, après lecture des fichiers
- **Vague 3** (split `forgia-core` 239 incoming en `forgia-prelude` + `forgia-core`) → story-514 Enterprise dédiée
- Renames de crates, refactor de modules, ajout de tests → hors scope

---

## 4. Pré-requis bloquants

- [ ] **WIP autre terminal stashé** : `git stash push -u -m "wip-abandoned-terminal-2026-05-23"` (33 fichiers, incl. forgia-mode-roguelite + forgia-foliage + forgia-rpg)
- [ ] **Branche dédiée créée** : `git checkout -b cleanup/workspace-purge-vague-1-4`
- [ ] **Baseline compile time mesurée** : `cargo build --workspace --timings` → snapshot `target/cargo-timings/cargo-timing-baseline-2026-05-23.html`
- [ ] **Sensor baseline workspace** : créer/écrire `forgia2_workspace_health.json` avec `{crates_count: 264, total_loc: 57735, scaffolds_pct: 54, baseline_date: "2026-05-23"}`

---

## 5. Plan d'exécution (commits atomiques par cluster)

### Commit 1 — Editor stubs
```bash
git rm -rf crates/forgia-editor-animation crates/forgia-editor-asset-browser \
  crates/forgia-editor-blueprint crates/forgia-editor-command-palette \
  crates/forgia-editor-core crates/forgia-editor-foliage \
  crates/forgia-editor-gizmo crates/forgia-editor-hierarchy \
  crates/forgia-editor-inspector crates/forgia-editor-kit-macros \
  crates/forgia-editor-material crates/forgia-editor-particle \
  crates/forgia-editor-terrain crates/forgia-editor-theme \
  crates/forgia-editor-undo-redo crates/forgia-editor-viewport
# Editer Cargo.toml workspace : retirer les 16 lignes "forgia-editor-*"
cargo check --workspace
git add -A && git commit -m "cleanup(story-512): purge 16 forgia-editor-* stubs (256 LOC TODO)"
```

### Commit 2 — Render stubs
```bash
git rm -rf crates/forgia-render-atmosphere crates/forgia-render-clouds \
  crates/forgia-render-dof crates/forgia-render-env-map \
  crates/forgia-render-forward-decal crates/forgia-render-god-rays \
  crates/forgia-render-oit crates/forgia-render-pcss \
  crates/forgia-render-rt crates/forgia-render-skybox \
  crates/forgia-render-ssao crates/forgia-render-ssr
# Editer Cargo.toml : retirer 12 lignes "forgia-render-*"
cargo check --workspace
git commit -am "cleanup(story-512): purge 12 forgia-render-* stubs (192 LOC TODO)"
```

### Commit 3 — QA stubs V2
```bash
git rm -rf crates/forgia-qa-autopilot crates/forgia-qa-core \
  crates/forgia-qa-harness crates/forgia-qa-invariants \
  crates/forgia-qa-kg crates/forgia-qa-replay \
  crates/forgia-qa-telemetry crates/forgia-qa-vlm
# Editer Cargo.toml : retirer 8 lignes "forgia-qa-*"
cargo check --workspace
git commit -am "cleanup(story-512): purge 8 forgia-qa-* V2 stubs (128 LOC, V1 QA suffit)"
```

### Commit 4 — Net stubs
```bash
git rm -rf crates/forgia-net-chat crates/forgia-net-lightyear \
  crates/forgia-net-lobby crates/forgia-net-matchmaking \
  crates/forgia-net-replication-genome crates/forgia-net-rollback \
  crates/forgia-net-voice
# Editer Cargo.toml : retirer 7 lignes "forgia-net-*"
cargo check --workspace
git commit -am "cleanup(story-512): purge 7 forgia-net-* stubs (112 LOC, lightyear dep workspace suffit)"
```

### Commit 5 — Script + Import stubs
```bash
git rm -rf crates/forgia-script-api-bindings crates/forgia-script-hot-reload \
  crates/forgia-script-sandbox crates/forgia-script-variables \
  crates/forgia-import-glb-validated crates/forgia-import-usd
# Editer Cargo.toml : retirer 6 lignes
cargo check --workspace
git commit -am "cleanup(story-512): purge 4 script + 2 import stubs (96 LOC)"
```

### Commit 6 — Modes inutilisés (Vague 4)
```bash
git rm -rf crates/forgia-mode-platformer crates/forgia-mode-puzzle \
  crates/forgia-mode-race crates/forgia-mode-rpg-openworld \
  crates/forgia-mode-rts crates/forgia-mode-survival
# Editer Cargo.toml : retirer 6 lignes "forgia-mode-*"
cargo check --workspace
git commit -am "cleanup(story-512): purge 6 forgia-mode-* unimplemented (RPG vit dans forgia-rpg)"
```

### Commit 7 — Final
```bash
# Mesure post-cleanup
cargo build --workspace --timings
# Mettre à jour forgia2_workspace_health.json :
# {crates_count: 209, total_loc: ~56950, scaffolds_pct: ~45, baseline_date: "2026-05-23"}
git commit -am "cleanup(story-512): update workspace health sensor — 264→209 crates"
```

---

## 6. Critères d'acceptation

- [ ] AC1 : 55 crates supprimées, Cargo.toml workspace mis à jour, aucun member fantôme
- [ ] AC2 : `cargo check --workspace` PASS (0 erreur) sur la branche finale
- [ ] AC3 : `cargo clippy --workspace -- -W warnings` PASS (0 warning nouveau)
- [ ] AC4 : `cargo build --workspace` PASS (binaires `forgia-game` etc. compilent)
- [ ] AC5 : Compile time mesuré avant/après dans `cargo-timing-*.html`, ratio reporté dans la story
- [ ] AC6 : Sensor `forgia2_workspace_health.json` reflète l'état final
- [ ] AC7 : Aucun code "utile" supprimé (validation : `cargo build` réussit sans avoir à réimporter quoi que ce soit)

---

## 7. Critère de rollback

Si à n'importe quel commit :
- `cargo check --workspace` reste rouge >15 min après début du commit
- Un import inattendu casse (`use forgia-X::Y` dans une crate active)

**Action** : `git reset --hard HEAD~1`, documenter le cluster fautif dans la story, escalader à user pour décider (implémenter inline ou retirer la dépendance).

**Garde-fou** : chaque commit est atomique sur un cluster. Revert d'un commit ne casse pas les précédents.

---

## 8. Post-impl (checklist post-implementation Forgia)

- [ ] Story status DONE
- [ ] `docs/stories/_index.md` mis à jour (si existe en V2)
- [ ] `MEMORY.md` (auto-memory) : ajouter `reference_workspace_purge_vague_1_4_2026_05_23.md` avec patterns retenus
- [ ] Push branche, créer PR (ou merge direct sur master si solo)
- [ ] Sub-agents `verifier` + `qa-lead` lancés en parallèle (post-impl auto-QA)

---

## 9. Métriques attendues

| Métrique | Avant | Après | Delta |
|---|---|---|---|
| Crates total | 264 | 209 | -55 (-21 %) |
| Scaffolds <50 LOC | 145 | ~96 | -49 |
| Stubs 100 % vides | 41 | 0 | -41 |
| Total LOC | 57 735 | ~56 950 | -785 (~1.3 %) |
| Cargo.toml members lignes | 264 | 209 | -55 |
| `cargo check` cold workspace | TBD (baseline) | TBD | Mesure en C7 |
| Hallucinations potentielles (crates fantômes) | 41 | 0 | -41 |

---

## 10. Suite (stories follow-up)

- **story-513** : Vague 2 — fusion 45 `forgia-pp-*` → 1 `forgia-postprocess` (BMAD Standard, après lecture de 3–4 `pp-*/src/lib.rs`)
- **story-514** : Vague 3 — split `forgia-core` god-object 239 incoming → `forgia-prelude` + `forgia-core` (BMAD Enterprise, session dédiée, plan mode)
- **story-515** : Ratchet `xtask` "no-scaffold" — fail CI si crate <50 LOC ou >80 % TODO comments

---

## Sources

- Audit du 2026-05-23 (sub-agent Explore + general-purpose web research)
- [matklad — Large Rust Workspaces (2021)](https://matklad.github.io/2021/08/22/large-rust-workspaces.html)
- [Bevy crates/ workspace](https://github.com/bevyengine/bevy/tree/main/crates)
- [Bevy discussion #2187 — Split philosophy](https://github.com/bevyengine/bevy/discussions/2187)
- [corrode.dev — Faster Rust Compile Times](https://corrode.dev/blog/tips-for-faster-rust-compile-times/)
- [users.rust-lang.org — Why most crates >1000 LOC](https://users.rust-lang.org/t/why-are-most-crates-1000-loc/83380)
- [Joel Spolsky — Things You Should Never Do, Part I](https://www.joelonsoftware.com/2000/04/06/things-you-should-never-do-part-i/) (justification "no rewrite")
- `MEMORY.md feedback_fictive_done_status_2026_05_21.md` (failure mode canonique des stubs)
