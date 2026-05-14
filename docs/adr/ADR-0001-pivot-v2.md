---
id: ADR-0001
title: Pivot V2 — workspace propre + portage incrémental
status: ACCEPTED
date: 2026-05-14
authors: [Antoine]
supersedes: []
related:
  - state-of-forgia-2026-05-14 (audit V1 préalable)
  - PLAN_V2_FOUNDATIONS_2026-05-14 (plan d'exécution)
---

# ADR-0001 — Pivot V2 Forgia

## Contexte

Le 2026-05-14, audit complet V1 par 7 sub-agents Claude (Explore + bevy-specialist + game-maker + planner + performance-analyst + economy-designer + terrain-specialist + qa-lead) :

- 22 crates actives, ~205 000 LOC, ~95 sensors JSON
- Ratio shippable / total = **12 %**
- 4 P0 ouverts, 5 hotfixes menu V1 cumulés, 16 fichiers `#[allow(dead_code)]`
- Ship M1 Arena pas atteint (audit honnête : 60 % confiance Q1-Q2 2027)

## Décision

Créer `C:/Users/Antoi/Desktop/Forgia Rewrite/` (V2) workspace propre :
- 13 crates DAG-libre dual-mode FPS Arena + RPG OpenWorld
- Portage chirurgical (60 % du V1 portable verbatim)
- Refait propre : UI/menu/sensors/plugin wiring (zones pourries V1)
- V1 reste vivant en mode **bug-fix only** (`D:/Forgia/`)

## Alternatives considérées et rejetées

### A. Rewrite from scratch pur
**Rejeté** : aucun cas solo dev rewrite-then-shipped documenté. Spolsky 2000 + Brooks 1975 + Borland + Netscape unanimes. Pattern "infra-first trap" (memory `feedback_infra_first_trap_solo_dev.md`).

### B. Strangler Fig in-place sur V1
**Considéré sérieusement** mais rejeté car :
- Antoine veut workspace neuf pour appliquer rigueur dès jour 1
- L'objectif "team-ready pour étudiants Technofutur" justifie la repensée structurelle
- Décision Antoine prise après débat 5 tours avec IA — pas un rewrite, un fork structurel

### C. Continuer V1 sans changement
**Rejeté** : audit montre dette accumulée bloque progression. 4 P0 ouverts simultanément = signal saturation cognitive.

## Conséquences

### Positives
- Workspace neuf = 0 dette technique day 1
- Architecture explicite par crate dès le départ (étudiants Technofutur futurs)
- Patterns terrain V1 (DAG-libre, BiomeGenomeOverrides, async, LRU) appliqués partout
- Mode dual FPS/RPG architecturé natif (pas bolted-on)
- Decision GO/NO-GO Phase 2 protège du piège "trop investi pour reculer"

### Négatives
- Décale ship Q4 2026 → Q1-Q2 2027 (~3 mois)
- Risque Second System Effect (Brooks) — mitigé par discipline `no-speculative-fix.md`
- 4 P0 V1 doivent être fixés avant Phase 0 V2 (~3-4 jours cette semaine)

### Risques + mitigations
- **Re-tuning gunfeel Phase 2** : décision GO/NO-GO stricte, retour V1 Strangler si échec
- **Burnout solo dev** : ship Q1 2027 cible, sprints 1 sem max, vendredi off
- **Pièges Bevy 0.18 oubliés** : 9 anti-traps documentés CLAUDE.md §6 dès Phase 0

## Plan d'exécution

Voir `D:/Forgia/RUST/Forgia/Forgia/docs/audits/PLAN_V2_FOUNDATIONS_2026-05-14.md` (plan complet 7 phases).

Phase 0 livrée 2026-05-14 : workspace 13 crates + Cargo.toml + lib.rs squelettes + CONTRIBUTING.md + ARCHITECTURE.md + ce ADR.

## Suivi

- **Indicateurs de dérive** :
  - Apparition d'un 14e crate non prévu sans ADR justifiant
  - Nombre de sensors > 12 (limite dure CI)
  - `#[allow(dead_code)]` réintroduit
  - Hardcode gameplay détecté CI

- **Revue trimestrielle** : auditer respect plan + éventuels écarts Phase

---

*Décision actée 2026-05-14 par Antoine après débat 5 tours avec IA. Validation : structure créée immédiatement après cette ADR.*
