# Story-534 — 🪓 Boucherie Moveset Distinctive (Mission 3 GDD)

> **Status** : DRAFT
> **Scale BMAD** : Standard
> **Effort estimé** : ~4 jours
> **GDD ref** : [Mission 3 - Boucherie](../design/gdd-roguelite-v1.md#-boucherie--larme-chaos-pur)
> **Prérequis** : story-528 (FPS feel)

## Pourquoi

Boucherie = arme chaos pur. Pattern de jeu distinctif : AOE chaos, repousse ennemis, ennemis s'envolent ragdoll. Pour joueurs qui veulent voir le monde brûler (en cartoon).

## Acceptance Criteria

- [ ] AC1 — LMB roquette parabolique 12 m/s, explosion 4m radius, 70 dmg + knockback 8m. Mag 3, reload shell-per-shell 4s
- [ ] AC2 — RMB "Roquette douce" lobée fort knockback, 30 dmg, AOE 5m. Voiceline *"On va juste les faire voler."*
- [ ] AC3 — Spé (Shift) "Salve festive" 3 roquettes simultanées spread cone. Consomme 3 ammo. Cooldown 12s. Voiceline *"C'EST LA FÊTE !"*
- [ ] AC4 — Animation FPS : Boucherie bouge épaules respiration idle, recoil 8° pitch massive, barillet rotatif visible, smoke roux orangé post-tir
- [ ] AC5 — Voicelines tir : *"BOUM !"*, *"ÇA PÈTE !"*, *"AHAHA !"*
- [ ] AC6 — Voicelines kill : *"ENVOLE-TOI !"*, *"C'est la fête !"*, *"AHA-HA !"*
- [ ] AC7 — Voicelines low ammo : *"Plus de jouets ! Vite vite !"*
- [ ] AC8 — Couleur dominante rouge orangé (material + smoke + explosion VFX)
- [ ] AC9 — Ragdoll ennemis touchés ≥ 1s soft (rapier impulse Y + cooldown re-control AI)
- [ ] AC10 — Sensor `forgia2_boucherie.json` : roquettes fired, AOE hits avg per shot, knockback distance avg, ragdolls/run

## Files
- `crates/forgia-weapon-hitscan/src/boucherie.rs` NEW (projectile bevy_rapier)
- `crates/forgia-viewmodel/src/boucherie_anim.rs` NEW
- `crates/forgia-effects/src/explosion_vfx.rs` NEW
- `assets/genomes/roguelite/weapons/boucherie.toml`

## Anti-canon
- "S'envolent" pas "blown up"
- "Ragdoll" technique OK code, UI dit "tombent"

## Cross-refs
- GDD V1 Mission 3 Boucherie
- Bible v1 persona Boucherie
- story-530 boons chain/chaos Boucherie
