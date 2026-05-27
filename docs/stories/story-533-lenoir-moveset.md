# Story-533 — 🎩 Madame Lenoir Moveset Distinctive (Mission 3 GDD)

> **Status** : DRAFT
> **Scale BMAD** : Standard
> **Effort estimé** : ~4 jours
> **GDD ref** : [Mission 3 - Lenoir](../design/gdd-roguelite-v1.md#-madame-lenoir--larme-précision)
> **Prérequis** : story-528 (FPS feel)

## Pourquoi

Lenoir = arme précision/patience. Pattern de jeu distinctif : long-range, attend l'ouverture parfaite, juge le joueur quand il rate. Pour joueurs patients qui aiment prendre leur temps.

## Acceptance Criteria

- [ ] AC1 — LMB hitscan instant, 80 HS / 40 body, mag 4, reload long 3s, tracer fin blanc 200ms
- [ ] AC2 — RMB scope 4×, no breath sway si static >0.5s, crosshair = monocle élégant
- [ ] AC3 — Spé (Shift) "Coup d'œil" silhouettes outline tous ennemis 5s à travers murs (through-walls shader). Cooldown 15s. Voiceline *"Une dame voit tout."*
- [ ] AC4 — Animation FPS : Lenoir parfaitement immobile idle, recoil 0° (impeccable), reload manipule cartouche dorée avec deux doigts comme un mouchoir
- [ ] AC5 — Voicelines tir : *"Tsk."*, *"Acceptable."*, *"Élégant."*
- [ ] AC6 — Voicelines HS (rare = mémorable) : *"Précis."*, *"Mes félicitations."* (5% chance, ne se déclenche pas si <2 HS d'affilée)
- [ ] AC7 — Voicelines miss : *"Lamentable."*, *"On se ressaisit, voulez-vous ?"*
- [ ] AC8 — Voicelines reload : *"Patientez..."*
- [ ] AC9 — Couleur dominante noir + blanc (smoking style), accent doré cartouche
- [ ] AC10 — Sensor `forgia2_lenoir.json` : HS ratio (cible >40% gamers, <20% débutants), scope time avg, coup d'œil uses

## Files
- `crates/forgia-weapon-hitscan/src/lenoir.rs` NEW
- `crates/forgia-viewmodel/src/lenoir_anim.rs` NEW
- `crates/forgia-effects/src/coup_doeil_shader.rs` NEW (outline through-walls)
- `assets/genomes/roguelite/weapons/lenoir.toml`

## Anti-canon
- "Adversaire" pas "target"
- "Élégance" terme récurrent voicelines

## Cross-refs
- GDD V1 Mission 3 Madame Lenoir
- Bible v1 persona Lenoir
- story-530 boons precision Lenoir
