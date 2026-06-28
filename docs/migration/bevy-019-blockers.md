# Gate board — Bloqueurs migration Bevy 0.19

> **À relire ~mensuellement.** La migration Bevy 0.18.1 → 0.19 ne peut PAS démarrer tant que les 3 cases ci-dessous ne sont pas toutes ✅. C'est l'Étape 0 du runbook [`bevy-0.19-rust-migration-prep.md`](bevy-0.19-rust-migration-prep.md#6-process-de-debugpatch-ordonné-le-runbook).
> Dernière vérif : **2026-06-22**.

## Pourquoi c'est bloquant

Ces 3 crates sont des dépendances *dures* du graphe (`bevy_rapier3d` à elle seule = 17 crates consommatrices). Leur dernière release crates.io dépend encore de Bevy 0.18 → `cargo` refusera tout bump Bevy 0.19 tant qu'elles n'ont pas publié (ou été forkées localement).

## Les 3 gates

- [ ] **`bevy_rapier3d`** → release ciblant Bevy 0.19 publiée sur crates.io
  - État 2026-06-22 : 🟠 PR draft [#694](https://github.com/dimforge/bevy_rapier/pull/694) « Update to Bevy 0.19.0 (WIP) » (Buncys). Bloquée sur la chaîne glam/glamx : Bevy 0.19 veut glam 0.32.1, rapier 0.32/parry 0.26.1 épinglent glamx 0.1.3 → glam 0.30.10. **dimforge doit release Rapier/parry (glamx 0.2+/0.3) d'abord.** Pas d'ETA.
  - À surveiller : merge de #694 + nouvelle release `bevy_rapier3d` (> 0.34.0) avec `bevy = "0.19"`.

- [ ] **`bevy_hanabi`** → release ciblant Bevy 0.19 publiée
  - État 2026-06-22 : 🔴 Rien d'amorcé (main sur 0.18, 0 PR/issue/commit 0.19). Repo très actif (djeedai). Historique : upgrade ~1-4 mois après chaque release Bevy.
  - À surveiller : [CHANGELOG](https://github.com/djeedai/bevy_hanabi/blob/main/CHANGELOG.md) + nouvelle release 0.19.

- [ ] **`bevy_water`** → release 0.19 publiée **OU** fork/patch local validé
  - État 2026-06-22 : 🔴 Dormant depuis fév. 2026 (dernière branche `bevy_0.18`, pas de `bevy_0.19`). **Le plus risqué.** Track RPG (pas SHIP) → contournable par fork local si reste dormant.
  - À surveiller : [branches](https://github.com/Neopallium/bevy_water/branches/all). Plan B : fork + `[patch.crates-io]`.

## Quand les 3 sont ✅

→ Passer à l'Étape 1 du runbook (worktree isolée). Avant l'Étape 4, **relancer une cartographie d'impact exhaustive** (cf §9 du dossier : la carte initiale sous-compte events/observers, scheduling, input, cursor, shaders WGSL).

## Journal de vérif

| Date | rapier | hanabi | water | Note |
|---|---|---|---|---|
| 2026-06-22 | 🟠 PR #694 WIP | 🔴 rien | 🔴 dormant | Création du board. |
