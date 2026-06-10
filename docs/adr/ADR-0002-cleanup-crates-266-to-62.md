---
id: ADR-0002
title: Cleanup workspace — 266 crates scaffold → 62 crates réelles
status: ACCEPTED
date: 2026-05-26 (décision/exécution) — formalisé 2026-06-10 (story-593 M1.7)
authors: [Antoine]
supersedes: [décision "237 crates fine-grained" du 2026-05-14]
related:
  - docs/audit/crates-maturity-audit-2026-05-19.md (audit forensic du drift)
  - docs/audit/scaffolds-audit-2026-05-23.md
  - .claude/rules/fine-grained-crates.md (règle réécrite en conséquence, 2026-06-10)
---

# ADR-0002 — Cleanup crates 266 → 62

## Contexte

Le bootstrap V2 (2026-05-14) visait 13 crates. Les sessions 2026-05-15→18 ont ajouté
~250 crates scaffold (<50 LOC) pour « réserver les namespaces » des phases futures
(post-process ×45, editor ×16, net ×7, script ×6, qa ×8…). L'audit forensic du
2026-05-19 mesurait : 258 crates dont **220 scaffolds (85 %)**, 41 PARTIAL, ratio
wired 21,6 %.

Coûts constatés : bruit de navigation (humain et IA), temps de compile workspace,
docs structurellement fausses, gouvernance impossible (« quel scaffold est vivant ? »).

## Décision

Supprimer les scaffolds vides : **266 → 167 crates (2026-05-23, PR #1) puis → 62
(2026-05-26)**. Une crate n'existe que si elle porte du code réel avec consommateur.
Protection anti-récidive : ratchet `cargo xtask no-scaffold` (bloque toute crate
<50 LOC non justifiée).

## Alternatives rejetées

- **Garder les scaffolds avec deadline de peuplement** : la deadline n'a pas de
  mécanisme d'enforcement, et 85 % de vide reste 85 % de bruit en attendant.
- **Feature-gater les scaffolds hors du build** : réduit le coût compile mais pas le
  bruit de navigation ni la dette de gouvernance ; complexité features inutile.

## Conséquences

### Positives
- Workspace navigable (62 crates toutes réelles), compile plus rapide, docs tenables.
- Le pattern « crate fine à la demande » reste vivant (voir fine-grained-crates.md
  révisée) — c'est la réservation préventive qui meurt, pas la granularité.

### Négatives / dettes induites
- ARCHITECTURE.md, README et `.claude/rules/fine-grained-crates.md` sont restés à
  l'état pré-cleanup pendant 2 semaines (audit 2026-06-10 : « doc fausse ×4 ») —
  corrigés par story-593 ; gardés désormais par `cargo xtask arch-drift`.
- Les ambitions différées (editor, net, scripting) n'ont plus de placeholder visible :
  elles vivent dans la roadmap (M5), pas dans le workspace.

## Leçon capitalisée

La réservation de namespaces par scaffolds est un anti-pattern en solo-dev : la
structure doit suivre le code, pas le précéder. Toute déclaration structurelle
(crate, doc, sensor) non adossée à du code réel devient un mensonge en quelques jours.
