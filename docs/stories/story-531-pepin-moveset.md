# Story-531 — 🔫 Pépin Moveset Distinctive (Mission 3 GDD)

> **Status** : EN COURS — incrément 1 livré 2026-06-10 + **VALIDÉ RUNTIME 2026-06-11**
> (user « ok parfait » : cœurs visibles, peak_run=10 atteint, dmg 37.0→44.4 = ×1,20 confirmé log+sensor)
> **Scale BMAD** : Standard
> **Effort estimé** : ~4 jours
> **GDD ref** : [Mission 3 - Pépin](../design/gdd-roguelite-v1.md#-pépin--larme-accessible)
> **Prérequis** : story-528 (FPS feel)

## Incrément 1 (2026-06-10) — jauge de confiance complète

- **AC9 ✅ code** : `forgia_combat::confidence` (ShotResolved par-tir + PepinConfidence
  + `apply_shot` pur testé) ; émission dans fire_weapon_minimal (hit = ≥1 pellet sur
  ennemi, mur/vide = miss, GDD) ; reset OnEnter(Roguelite) ; **payoff base
  genome-driven** : +2 %/stack de dégâts (= +20 % à 10), neutre hors Pépin —
  `assets/genomes/roguelite/pepin_confidence.toml` hot-reload, défauts = miroir.
- **AC9 HUD ✅ code** : `forgia-ui-lib/hud/confidence.rs` — 10 cœurs cyan (AC8) au-dessus
  de l'énergie, visibles UNIQUEMENT Pépin en main, cœur de tête clignote 0,5 s au changement.
  - **Fix 2026-06-11 (jauge invisible in-game)** : les glyphes texte ♥/♡ (U+2665/U+2661)
    ne sont pas couverts par les polices egui → rien à l'écran alors que le payoff tournait
    (log : dmg 37.0→37.7→38.4→39.2 par hit consécutif = +2 %/stack actif). Cœurs désormais
    dessinés au painter via `draw_cartoon_heart` (energy.rs, passé `pub(crate)`) — la même
    primitive que le HUD énergie qui, lui, était visible.
- **AC10 ✅ code** : sensor `forgia2_pepin.json` 1Hz (stacks/peak_run/accuracy/damage_mul)
  + entrée SENSOR_REGISTRY.
- Tests : 4 purs (apply_shot/saturations) + 4 fps (tuning miroir, mul par-arme,
  disabled, App headless multi-armes) — 69 verts sur les 3 crates, clippy 0.
- **AC5-7 incrément kill ✅ code (2026-06-11)** : `forgia-ui-lib/hud/barks.rs` — le
  genome `roguelite_dialogue.toml` (dormant depuis l'abandon 471-479) est ENFIN
  consommé. Trigger = `CombatHitEvent{is_kill, weapon}` (multicast, killfeed non
  affecté) → l'arme qui tue parle pour les **4 personas** (pools kill existants).
  Pattern Hadès : P(bark)=0.30, tirage pondéré, cooldown par ligne, lock global
  2,5 s anti-overlap, plafond 12/min anti-fatigue — tout genome hot-reload. Bulle
  cartoon bas-droite, liseré couleur persona, fade-out. Sensor `forgia2_barks.json`
  + registry. 5 tests (parse TOML réel, tirage déterministe, gate, mapping, App
  headless kill→bark+lock) — 12/12 verts, clippy 0.
- **Restent** (incréments suivants) : AC2 ADS accuracy ×2 static, AC3 « Petit cri »
  (⚠ Shift = déjà Sprint, keybind à trancher), AC4 anims viewmodel, AC5-7 events
  restants (fire « PIOU ! » = pas de pool fire dans le TOML actuellement, lowhp,
  idle, reload, swap, pickup — le moteur barks les accepte déjà, manque les triggers),
  AC1 audit stats genome vs GDD (15 dmg/mag 12/4 par s), tracer cyan 100 ms (AC8 partiel).

### Test in-game (incrément 1)

1. **Action** : rebuild → run Roguelite → garder Pépin (slot 1) → enchaîner des tirs
   sur ennemis puis rater exprès.
2. **Effet attendu** : rangée de cœurs cyan discrets au-dessus d'ÉNERGIE qui se
   remplit à chaque hit (le dernier clignote), se vide d'un cœur par tir raté ;
   à pleine jauge les ennemis tombent sensiblement plus vite (+20 %) ; en changeant
   d'arme (2/3/4) les cœurs disparaissent et la jauge ne bouge plus.
3. **Sensor** : `forgia2_pepin.json` → stacks suit les cœurs, `damage_mul` 1.00→1.20,
   accuracy = hits/(hits+misses).
4. **Variantes si KO** : cœurs absents → vérifier arme = slot 1 (ModernAR) + mode
   Roguelite ; jauge ne monte pas → vérifier log `[fire] pellet … HIT` ; payoff
   imperceptible → monter `per_stack_damage` à 0.04 dans le TOML (hot-reload).

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
