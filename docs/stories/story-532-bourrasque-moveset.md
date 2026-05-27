# Story-532 — 💨 Bourrasque Moveset Distinctive (Mission 3 GDD)

> **Status** : DRAFT
> **Scale BMAD** : Standard
> **Effort estimé** : ~4 jours
> **GDD ref** : [Mission 3 - Bourrasque](../design/gdd-roguelite-v1.md#-bourrasque--larme-chaos-proche)
> **Prérequis** : story-528 (FPS feel)

## Pourquoi

Bourrasque = arme du chaos proche, court-range explosif joyeux. Pattern de jeu distinctif : "danse autour des ennemis avec knockback". Idéale joueurs aimant être au cœur de l'action.

## Acceptance Criteria

- [ ] AC1 — LMB shotgun 7 pellets cone 20°, 8 dmg/pellet, range 10m max, mag 5, fire rate 1.5/s
- [ ] AC2 — RMB "Souffle" cone 4m, knockback 4m, 0 damage, déstabilise/interrupt ennemis 1s. Voiceline *"Pousse-toi !"*
- [ ] AC3 — Spé (Shift) "TORNADE !" vortex stationnaire 3m radius 2s, pull ennemis vers centre. Bourrasque tourne IRL visible viewmodel. Cooldown 12s. Voiceline *"WIIIII !"*
- [ ] AC4 — Animation FPS : sautille léger idle 1Hz, pump-action très visible reload (clic-clac théâtral), muzzle smoke jaune
- [ ] AC5 — Voicelines tir : *"PROUT !"*, *"BAAM !"*, *"YAA !"*
- [ ] AC6 — Voicelines kill : *"WHOUUUU !"*, *"Sayonara !"*, *"Ça fait du bien !"*
- [ ] AC7 — Voicelines low ammo : *"Saperlipopette, faut recharger !"*
- [ ] AC8 — Couleur dominante jaune chaud (material + smoke + tornado VFX)
- [ ] AC9 — Sensor `forgia2_bourrasque.json` : pellets fired/hit ratio, souffle uses/s, tornado pulls count, knockback distance moyenne

## Files
- `crates/forgia-weapon-hitscan/src/bourrasque.rs` NEW
- `crates/forgia-viewmodel/src/bourrasque_anim.rs` NEW
- `crates/forgia-effects/src/tornado_vfx.rs` NEW
- `assets/genomes/roguelite/weapons/bourrasque.toml`

## Anti-canon
- "Pellets" pas "shotgun shrapnel"
- "Souffle" pas "blast"

## Cross-refs
- GDD V1 Mission 3 Bourrasque
- Bible v1 persona Bourrasque
- story-530 boons chaos/knockback Bourrasque
