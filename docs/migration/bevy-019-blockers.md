# Gate board — Bloqueurs migration Bevy 0.19

> **À relire ~mensuellement.** La migration Bevy 0.18.1 → 0.19 ne peut PAS démarrer tant que les 3 cases ci-dessous ne sont pas toutes ✅. C'est l'Étape 0 du runbook [`bevy-0.19-rust-migration-prep.md`](bevy-0.19-rust-migration-prep.md#6-process-de-debugpatch-ordonné-le-runbook).
> Dernière vérif : **2026-08-05**.

## Pourquoi c'est bloquant

Ces 3 crates sont des dépendances *dures* du graphe (`bevy_rapier3d` à elle seule = 17 crates consommatrices). Leur dernière release crates.io dépend encore de Bevy 0.18 → `cargo` refusera tout bump Bevy 0.19 tant qu'elles n'ont pas publié (ou été forkées localement).

## Les 3 gates

- [x] **`bevy_rapier3d`** → release ciblant Bevy 0.19 publiée sur crates.io — **✅ AVEC ASTÉRISQUE**
  - État 2026-08-05 : ✅ **0.35.0 publiée le 2026-07-12**, `bevy = "^0.19.0"` (vérifié crates.io). ⚠️ **MAIS** elle épingle `rapier3d = "=0.33.0-alpha"` — le cœur physique est en **alpha**. Rapier = dépendance dure de 17 crates + invariant déterminisme FixedUpdate 64 Hz → **le signal de départ propre est une 0.35.x avec rapier STABLE**, pas cette release.
  - À surveiller : release `bevy_rapier3d` ≥ 0.35.x dont le `rapier3d` épinglé n'est plus `-alpha`.

- [x] **`bevy_hanabi`** → release ciblant Bevy 0.19 publiée
  - État 2026-08-05 : ✅ **0.19.0 publiée le 2026-06-27** (vérifié crates.io). Note : MSRV Rust 1.95+.

- [ ] **`bevy_water`** → release 0.19 publiée **OU** fork/patch local validé
  - État 2026-08-05 : 🔴 Toujours dormant — dernière release **0.18.1 (février 2026)**, rien depuis 6 mois. **Dernier bloqueur écosystème.** Track RPG/FORGE (pas SHIP) → la décision est « fork + `[patch.crates-io]` » ou « sortir l'eau du workspace / feature-gate », pas « attendre ».
  - À surveiller : [branches](https://github.com/Neopallium/bevy_water/branches/all). Plan B : fork + `[patch.crates-io]`.

### Hors gates — vérifiés compatibles 0.19 au 2026-08-05

`leafwing-input-manager` 0.21.0 (22 juin, `^0.19` vérifié) · `bevy_mod_scripting` 0.21.0 (30 juillet, `^0.19` vérifié) · `bevy_egui` / `lightyear` / `bevy_kira_audio` ✅ (cochés au masterplan 2026-07-01). Bevy lui-même : **0.19.0 du 19 juin, aucun patch 0.19.x depuis** — un 0.19.1 serait un plus avant de migrer.

## Quand les 3 sont ✅

→ Passer à l'Étape 1 du runbook (worktree isolée). Avant l'Étape 4, **relancer une cartographie d'impact exhaustive** (cf §9 du dossier : la carte initiale sous-compte events/observers, scheduling, input, cursor, shaders WGSL).

## Décision de fenêtre (2026-08-05)

La migration est devenue **techniquement possible** mais reste **volontairement différée**. Conditions de départ (toutes) :

1. Le cycle en cours est fermé : validation manette en main + courbe de puissance réparée + playtest externe #1.
2. `bevy_rapier3d` publie une release dont le cœur `rapier3d` n'est plus en alpha.
3. La décision `bevy_water` est prise (fork local ou sortie du workspace).

Fenêtre visée : **avant la phase contenu P4** (on ne migre pas un moteur pendant la production d'armes/boss). Si rapier traîne en alpha : **alternative assumée = shipper sur 0.18.1** et migrer post-launch. Rien dans 0.19 n'est un débloqueur de ship (contact shadows, GPU light clustering, culling skinné = confort ; le coût perf mesuré est la scène statique, pas le skinning).

## Journal de vérif

| Date | rapier | hanabi | water | Note |
| --- | --- | --- | --- | --- |
| 2026-06-22 | 🟠 PR #694 WIP | 🔴 rien | 🔴 dormant | Création du board. |
| 2026-08-05 | ✅* 0.35.0 (^0.19) mais rapier3d **=0.33.0-alpha** | ✅ 0.19.0 | 🔴 dormant (0.18.1, 6 mois) | Mur principal tombé le 12/07. leafwing 0.21 + bevy_mod_scripting 0.21 vérifiés ^0.19. Décision : différé — cf § Décision de fenêtre. |
