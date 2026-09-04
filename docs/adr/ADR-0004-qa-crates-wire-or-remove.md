---
id: ADR-0004
title: Crates QA (qa-core/replay/harness/autopilot) — brancher pour de vrai ou sortir du workspace
status: PROPOSED (décision du mainteneur requise)
date: 2026-06-10 (story-593 M1.6)
authors: [Claude (proposition), mainteneur (décision)]
supersedes: []
related:
  - docs/audit/audit-2026-06-10-full-codebase.md §4 thème C (findings vérifiés adversarialement)
  - docs/ROADMAP_POST_AUDIT_2026-06-10.md M1.6
---

# ADR-0004 — Le sort des 4 crates QA

## Contexte (état vérifié, audit 2026-06-10)

4 crates (~4 300 LOC) : forgia-qa-core, forgia-qa-replay, forgia-qa-harness,
forgia-qa-autopilot. État mesuré et contre-vérifié :

- **0 producteur** de `BugReport` dans le gameplay ; le drain est compilé **no-op**
  (feature `qa-runtime` jamais activée par forgia-game).
- **Replay inutilisable pour un FPS** : capture clavier seul (pas de souris), jamais
  déclenché (aucun binding), aucun consommateur d'injection, hash KeyCode
  **irréversible** (DefaultHasher — instable entre versions Rust), binaire
  `forgia_repro` documenté mais inexistant.
- harness/autopilot : **0 dépendant inverse** ; golden frames sur sensors mock.
- MAIS : types bien conçus (BugReport ULID+dédup, sessions RON versionnées), 155 tests
  internes verts, et les plugins sont DÉJÀ branchés (no-op) dans le binaire.

Coût actuel : compile + entretien de la fiction « QA automatisée » (docs qui
sur-vendent), pendant que la vraie QA (les 1 066 tests + sensors) vit ailleurs.

## Options

### Option A — Brancher pour de vrai (~1-2 jours)
Activer `qa-runtime` dans forgia-game ; émettre des BugReport depuis 3-4 points chauds
Roguelite (panic [déjà couvert par forgia2_crash.json story-592], HP incohérent,
NaN transform, wave stuck) ; 1 SmokeBot en CI. Replay : descoper officiellement
(retirer la promesse des docs) ou le réparer (souris + table KeyCode stable + binding
— chantier M, pas S).
**Pour** : le moat « QA pilotable par IA » devient réel ; infra déjà écrite.
**Contre** : 1-2 jours pris sur M2 ; touche forgia-game (claimé multi-terminal au
moment de l'ADR).

### Option B — Sortir du workspace (réactivable post-ship) (~1 h)
Retirer les 4 crates de members + les add_plugins ; archiver (git garde tout).
**Pour** : -4 300 LOC de fiction, compile plus rapide, honnêteté immédiate.
**Contre** : perd l'élan (re-brancher post-ship = friction) ; le SmokeBot CI serait
pourtant le filet anti-régression le moins cher pour M2-M3.

### Option C — Statu quo
**Rejetée d'office** : c'est l'état que l'audit qualifie de pire des trois (fiction
maintenue, coût payé, valeur nulle).

## Recommandation (Claude)

**Option A minimale, séquencée après levée du claim forgia-game** : (1) activer
qa-runtime + 3 émetteurs BugReport (S) ; (2) 1 scénario SmokeBot « boot → menu →
run wave 1 → quit » dans le job CI per-crate (S-M) ; (3) **descoper officiellement le
replay** (retirer binaire fantôme et promesses des docs, story de réparation en M4 si
besoin réel). Le SmokeBot CI est le seul élément qui protège directement le ship —
c'est lui qui justifie l'option A.

## Décision

_À trancher par le mainteneur. Statut passera ACCEPTED-A / ACCEPTED-B avec date._
