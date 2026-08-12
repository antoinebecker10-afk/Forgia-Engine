# Story-650 — Knockback par hit : les ennemis encaissent physiquement

> **Statut** : IN_PROGRESS (validation feel user en attente)
> **Niveau BMAD** : Standard (5 fichiers)
> **Origine** : audit VFX 2026-07-02 §P0-4 (trick Vlambeer : « chaque balle qui touche pousse physiquement l'ennemi dans la direction du tir ; la mort donne une impulse plus forte »).

## Contrainte découverte (concept-first)

Les ennemis sont **`RigidBody::KinematicPositionBased`** (`waves.rs:173`) → `ExternalImpulse` Rapier = **sans effet**. Le knockback passe par un composant `Knockback { vel }` à décroissance exponentielle (λ=8/s, ~63 % du déplacement en 125 ms — poussée sèche arcade) qui déplace le `Transform` chaque frame. **Additif** : compose avec le mouvement incrémental des bots (`forgia-ai-arena-bot`, non touché — chantier actif de l'autre terminal).

## Design

- Direction = attaquant→cible, **horizontale** (pas de lift sur simple hit).
- Kill = ×3 + **pop vertical** 0.25 m (le corps « part »).
- Par arme (genome, mapping WeaponType legacy → V2) : Pépin ×0.6 picote, Bourrasque ×0.45 micro-poussées, Lenoir ×1.6, **Boucherie ×2.2 projette**. Melee/world = ×1.0.
- Cumul borné `knockback_max_m` 2.5 m (anti-éjection hors arène en full-auto).
- `Res<Time>` virtuel → gèle en pause ET pendant le hitstop (le kill freeze PUIS le corps part — séquençage dramatique gratuit avec story-648).
- Déplacements exprimés en **mètres totaux** dans le genome (intuitif) ; λ = const de forme de courbe (creator-simplicity).

## Fichiers

- `crates/forgia-juice-lib/src/knockback.rs` (nouveau module : composant + tick + genome hot-reload 1Hz + capteur `forgia2_knockback.json` + 7 tests purs) + `lib.rs`
- `crates/forgia-combat/src/combat_juice.rs` — pont `CombatHitEvent → Knockback` (`sys_apply_hit_knockback`) + mapping armes ; `lib.rs` wiring (plugin idempotent + GameSet::Effects)
- `assets/genomes/roguelite/roguelite_gamefeel.toml` — 9 genes `knockback_*`

## Acceptance criteria

- [x] Hit = poussée horizontale ; kill = ×3 + pop vertical (tests purs)
- [x] Boucherie > 2× Pépin (test `weapon_mult_scales_push`)
- [x] Cumul borné par `knockback_max_m` (test `accumulation_is_capped`)
- [x] Genome hot-reload + capteur (pushes/kill_pushes/active_now/last_displacement_m)
- [x] **Le mécanisme tire** — run du 2026-08-12, `forgia2_knockback.json` :
      `pushes: 419`, `kill_pushes: 51`, `last_displacement_m: 0.60`
      (base 0,30 · kill ×3 · plafond 2,5 m). Recoupé par
      `forgia2_fps_feel.json::hit_feedbacks_total: 420` — deux compteurs
      indépendants qui concordent à 1 près.
- [ ] **Validation feel user** : les ennemis « encaissent », la Boucherie projette, pas d'éjection hors arène ni de clipping mur visible
      → *reste ouvert à dessein : un compteur ne dit pas si ça fait du bien.
      Le plafond à 2,5 m rend l'éjection hors arène improbable, mais ça se
      constate manette en main, pas dans un JSON.*
- [x] `cargo check` + clippy 0 warning introduit + tests verts

## Risques connus (à observer runtime)

1. **Clipping murs** : le Transform est déplacé sans query de collision — déplacements faibles (≤0.9 m hit Boucherie) → risque faible ; si observé, réduire `knockback_base_m` ou raycaster avant de déplacer (story suiveuse).
2. **Écrasement par l'IA** : si le mover des bots écrit une position absolue (pas incrémentale), la poussée serait partiellement mangée — le capteur `pushes` prouvera au moins que le système tire.

## Suite

Chime weakspot + thump kill (P0-5) · séquence de mort 4 temps (P1) — le knockback kill en est déjà le « pop » directionnel.
