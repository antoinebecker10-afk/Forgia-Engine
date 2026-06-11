# Story-534 — 🪓 Boucherie Moveset Distinctive (Mission 3 GDD)

> **Status** : EN COURS — incrément 1 livré 2026-06-11 (AC1 roquette+knockback, AC10, AC6 via barks)
> **Scale BMAD** : Standard
> **Effort estimé** : ~4 jours
> **GDD ref** : [Mission 3 - Boucherie](../design/gdd-roguelite-v1.md#-boucherie--larme-chaos-pur)
> **Prérequis** : story-528 (FPS feel)

## Incrément 1 (2026-06-11) — la roquette parabolique (1ère arme projectile du moteur)

- **AC1 ✅ code** : architecture **hitscan-plombier / projectile-porteur** —
  `viewmodel_arena.toml [weapons.boucherie]` : `damage = 0` + mag **3** +
  reload roquette-par-roquette (1.33 s ×3 = 4 s GDD) + gros kick ; nouveau
  guard `projectile_weapon` dans forgia-fps (damage≤0 ⇒ le ray ne porte ni
  dégâts ni tracer/impact — ammo/cooldown/recoil/muzzle/SFX restent).
  `forgia-mode-roguelite/boucherie_rocket.rs` : spawn sur `WeaponFiredEvent`
  (multicast, audio intact), **12 m/s + lob, gravité projectile dédiée**,
  segment-raycast rapier anti-tunneling (prédicat exclut joueur+viewmodel),
  explosion **4 m / 70 dmg + Knockback pop** (réutilise le composant shockwave,
  « ils volent »), VFX disque orange + CameraTrauma, failsafe 6 s.
- **Attribution d'arme dans les sorts F** (bonus transversal) :
  `shockwave::deal_damage/aoe_strike/line_strike` prennent désormais
  `Option<WeaponType>` → les kills aux sorts F déclenchent les kill-barks de la
  bonne persona + icône killfeed correcte (avant : `weapon: None`).
- **AC6 ✅ déjà couvert** : kill-barks Boucherie (*« AAAH ! Le bon morceau ! »*)
  via le moteur barks — maintenant aussi sur les kills d'explosion.
- **AC10 ✅ code** : `forgia2_boucherie.json` 1Hz (rockets_fired/explosions/
  enemies_hit/avg/kills) + registry. 141 tests verts (2 crates), clippy 0.
- **Note** : commit préalable de la story-591 (WIP Enclume des Âmes) pour
  déminer `lib.rs` avant branchement.
- **Restent** : AC2 « Roquette douce » RMB (même décision que RMB Bourrasque),
  AC3 « Salve festive » (⚠ Shift=Sprint), AC4 anims barillet, AC5/AC7 barks
  tir/lowammo (pools à écrire), AC8 couleurs/VFX dédiés (disque orange V1),
  AC9 vrai ragdoll (V1 = Knockback pop 0.32 s).

### Test in-game (incrément 1)

1. **Action** : run Roguelite → touche **4** (Boucherie) → tirer vers un groupe
   d'ennemis à 10-20 m, puis en l'air (cloche) pour voir la parabole.
2. **Effet attendu** : une **boule orange lumineuse** part lentement (12 m/s)
   en arc, explose au premier contact (mur/sol/ennemi) → disque orange,
   secousse caméra, ennemis dans 4 m **projetés en l'air** et repoussés ;
   ~2 roquettes tuent un ennemi plein PV (70 dmg). Mag 3, recharge roquette
   par roquette. PLUS de gerbe hitscan instantanée.
3. **Sensor** : `forgia2_boucherie.json` → rockets_fired/explosions_run/
   avg_hits_per_explosion/kills_run.
4. **Variantes si KO** : roquette invisible → log `[boucherie] BOUM !` présent ?
   (si oui = problème visuel mesh) ; explose au nez du joueur → augmenter
   `SPAWN_AHEAD` 1.8→2.5 (boucherie_rocket.rs, rebuild) ; arc trop tombant →
   `ROCKET_GRAVITY` 5→3.

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
