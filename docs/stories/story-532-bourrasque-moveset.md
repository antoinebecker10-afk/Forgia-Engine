# Story-532 — 💨 Bourrasque Moveset Distinctive (Mission 3 GDD)

> **Status** : EN COURS — incrément 1 livré 2026-06-11 (AC1+AC9, AC6 via moteur barks)
> **Scale BMAD** : Standard
> **Effort estimé** : ~4 jours
> **GDD ref** : [Mission 3 - Bourrasque](../design/gdd-roguelite-v1.md#-bourrasque--larme-chaos-proche)
> **Prérequis** : story-528 (FPS feel)

## Incrément 1 (2026-06-11) — identité de tir chaos proche

- **AC1 ✅ v2 VALIDÉE 2026-06-11 (« C'est mieux! »)** : `viewmodel_arena.toml
  [weapons.bourrasque]`. v1 = pump 1.5/s × 7 pellets cone 20° (lecture littérale
  GDD) → feedback user « **ça fait trop fusil à pompe** » (redondant Boucherie).
  **v2 = LANCE-RAFALES de vent** : `auto` 4/s × **5 pellets cône serré 12°**,
  6 dmg/pellet (30/rafale, DPS 120 close), range 12 m (falloff 5→12, ×0.25),
  mag 20 (5 s de souffle continu) / réserve 100, reload 1.6 s, juice léger +
  wobble yaw 0.25° (pas de kick pump). Unique au roster : Pépin semi précis /
  Lenoir sniper / Boucherie pump / **Bourrasque souffle continu**. Itération
  full hot-reload (file_watcher), validée en live sans rebuild.
- **AC6 ✅ déjà couvert** : kill-barks Bourrasque (« WHOOSH ! Voilà ce que
  j'appelle un kill ! »…) actifs depuis le moteur barks (story-531 AC5-7).
- **AC9 ✅ code** : `forgia-fps/bourrasque.rs` — `BourrasqueStats` (observe
  `ShotResolved` + `CombatHitEvent` multicast, ZÉRO modification du fire path),
  reset OnEnter(Roguelite), sensor `forgia2_bourrasque.json` 1Hz
  (shots/accuracy/pellets ratio/kills) + registry. 2 tests headless — 27 verts.
- **Découverte design** : le knockback identité existe DÉJÀ partiellement —
  sort **F « Coup de Bourrasque »** (gust 12 m + pop, CD 7 s, story-572,
  `forgia-mode-roguelite/shockwave.rs`). AC2 « Souffle » RMB ferait doublon →
  **décision Antoine requise** : (a) garder F seul, (b) RMB = souffle court 0 dmg
  interrupt EN PLUS du F (remplace l'ADS de Bourrasque), (c) déplacer le gust F
  sur RMB. AC3 TORNADE = conflit Shift=Sprint (même décision keybind que Pépin
  « Petit cri »).

### Test in-game (incrément 1)

1. **Action** : run Roguelite → Digit2 (Bourrasque) → tirer à bout portant,
   à 5 m, à 15 m sur ennemis.
2. **Effet attendu** : pump 1,5 tir/s (plus d'auto 16/s), gerbe de 7 impacts
   en cône large, ennemis fondent à bout portant (~2 tirs), quasi-zéro dégâts
   au-delà de 10 m, mag de 5 + recharge rapide, kick visible.
3. **Sensor** : `forgia2_bourrasque.json` → shots_run/pellet_hit_ratio/kills_run.
4. **Variantes si KO** : trop faible à bout portant → `damage` 8→10 (hot-reload
   Shift+F12 inutile, file_watcher auto) ; cône trop large → `spread_deg` 20→14 ;
   trop puissant → `damage_falloff_start` 4→2.

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
