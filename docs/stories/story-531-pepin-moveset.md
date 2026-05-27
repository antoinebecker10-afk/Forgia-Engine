# Story-531 — 🔫 Pépin Moveset Distinctive (Mission 3 GDD)

> **Status** : DRAFT
> **Scale BMAD** : Standard
> **Effort estimé** : ~4 jours
> **GDD ref** : [Mission 3 - Pépin](../design/gdd-roguelite-v1.md#-pépin--larme-accessible)
> **Prérequis** : story-528 (FPS feel)

## Pourquoi

Gap #1 du roadmap : 4 armes définies TOML mais joueur tire AK générique. Pépin = arme accessible (1ère rencontrée), simple à prendre en main mais profondeur cachée (jauge confiance).

## Acceptance Criteria

- [ ] AC1 — LMB hitscan instant, 15 dmg, mag 12, 4/s, tracer cyan visible 100ms
- [ ] AC2 — RMB ADS zoom 1.5×, accuracy ×2 quand static, voiceline *"Concentre-toi..."*
- [ ] AC3 — Spé (Shift) "Petit cri" burst 3 tirs rapides, consomme 3 ammo, cooldown 6s, voiceline *"AAA-IIIE !"*
- [ ] AC4 — Animation FPS : tremblote idle 0.5Hz, recoil 2° pitch, fume cyan canon post-tir, mag drop reload
- [ ] AC5 — Voicelines tir pool : *"PIOU !"*, *"Tac !"*, *"J'ai eu !"* — random per shot
- [ ] AC6 — Voicelines kill pool : *"OH ! J'ai réussi !"*, *"Maître serait fier !"*
- [ ] AC7 — Voicelines low ammo : *"Heu... j'ai presque plus..."*
- [ ] AC8 — Couleur dominante bleu cyan respectée (material + tracer + muzzle flash)
- [ ] AC9 — Mécanique jauge confiance impl : Resource `PepinConfidence(u8)` 0-10, +1/hit, -1/miss, UI cœur clignote
- [ ] AC10 — Sensor `forgia2_pepin.json` : shots/hits/HS ratio, confidence current, peak confidence run

## Files
- `crates/forgia-weapon-hitscan/src/pepin.rs` NEW ou extend
- `crates/forgia-viewmodel/src/pepin_anim.rs` NEW
- `assets/genomes/roguelite/weapons/pepin.toml` (extend)
- Voicelines : déjà dans `assets/genomes/roguelite/roguelite_dialogue.toml`

## Anti-canon
- "Tir" pas "fire/shoot"
- "Coup" pas "damage" en UI

## Cross-refs
- GDD V1 Mission 3 Pépin
- Bible v1 persona Pépin
- story-530 (boons Pépin réutilisent confidence gauge)
