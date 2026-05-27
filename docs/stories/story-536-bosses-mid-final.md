# Story-536 — Mid-boss + Boss Final (Mission 4.4 GDD)

> **Status** : DRAFT
> **Scale BMAD** : Enterprise (plan mode)
> **Effort estimé** : ~7 jours (le plus gros morceau V1)
> **GDD ref** : [Mission 4.4](../design/gdd-roguelite-v1.md#44-boss)
> **Prérequis** : story-535 (ennemis FSM), 531-534 (armes pour différenciation patterns)

## Pourquoi

2 bosses V1 = climax narratif. Mid-boss "Grand Forgeron Cage" stage 2 récompense boon légendaire. Final "Forgeron Noir" en pyjama = pivot narratif principal + unlock all armes permanently.

## Acceptance Criteria

### Mid-boss "Le Grand Forgeron Cage" 🟡

- [ ] AC1 — Géant cartoon, marteau visible, voix grave-comique
- [ ] AC2 — Phase 1 (100→50%) : marteau swing AOE télégraphe 2s + shrapnel cartoon
- [ ] AC3 — Phase 2 (<50%) : colère, tire 3× plus + summon 3 Cages standard per cycle
- [ ] AC4 — Dialogues phase 1 : *"JE SUIS LE PLUS GROS !"*, *"TU ES MINUSCULE !"*
- [ ] AC5 — Dialogues phase 2 : *"AÏE MES MARTEAUX !"*, *"PFFF JE SUIS FATIGUÉ"*
- [ ] AC6 — Victoire : brise en éclats lumineux dorés + âme arme bonus → choix 1 boon entre 3 légendaires
- [ ] AC7 — Différenciation 4 armes vs Grand Forgeron : weakpoint joyau torse (Pépin/Lenoir), stun openings (Bourrasque), chain summons (Boucherie)

### Boss final "Le Forgeron Noir" 🔴

- [ ] AC8 — Petit perso ridicule (Bowser-energy), chapeau ridicule trop grand, monté sur Machine de Forge géante
- [ ] AC9 — Phase 1 : machine tire boulets télégraphe 2s, Forgeron Noir sur toit rit *"AHAHA !"*. Summon 2 Cages periodically
- [ ] AC10 — Phase 2 (<50%) : machine se brise progressivement, Forgeron Noir descend **en pyjama** (révélation comique). Court autour salle en couinant *"OUILLE !"*
- [ ] AC11 — Dialogues phase 1 : *"TU N'AURAS JAMAIS LES ÂMES !"*, *"JE VAIS TE TRANSFORMER EN MARTEAU !"*
- [ ] AC12 — Dialogues phase 2 : *"AÏE MA SOUPE !"*, *"POURQUOI MOIIII ?"*, *"MAMAN !"*
- [ ] AC13 — Victoire cinematic 30s : toutes âmes-armes apparaissent autour Apprenti, Forgeron Noir s'endort dans pyjama (ronfle ZzzZ), Maître Forgeron apparaît *« Tu l'as fait mon petit. »*
- [ ] AC14 — Retour hub avec **toutes 4 armes unlock permanently** (state persist via éclats save)
- [ ] AC15 — Différenciation 4 armes vs Forgeron Noir : snipe machine (Lenoir), démolition (Boucherie), tornade machine + Forgeron (Bourrasque), face-à-face P2 (Pépin)

### Sensors

- [ ] AC16 — `forgia2_bosses.json` : encounter starts, phase transitions, death cause per arme, run completion rate
- [ ] AC17 — Boss difficulty curve : anti-frustration fail 2× → +1 cœur cosmetic (recycle pattern story-528 AC anti-frustration)

## Files
- `crates/forgia-mode-roguelite/src/boss_mid.rs` NEW
- `crates/forgia-mode-roguelite/src/boss_final.rs` NEW
- `crates/forgia-mode-roguelite/src/cinematic_victory.rs` NEW
- `crates/forgia-mode-roguelite/src/boss_phases_fsm.rs` NEW
- `assets/genomes/roguelite/bosses/grand_forgeron_cage.toml` NEW
- `assets/genomes/roguelite/bosses/forgeron_noir.toml` NEW
- `assets/genomes/roguelite/bosses/dialogues.toml` NEW (~50 voicelines bosses)

## Anti-canon
- "S'endort" boss final dans pyjama (pas "die")
- Forgeron Noir ridicule jamais menaçant (style Bowser)
- "Éclats lumineux dorés" = libération âme, pas explosion violente

## Cross-refs
- GDD V1 Mission 4.4
- Bible v1 personas Forgeron Noir + Maître Forgeron
- story-535 (Cages summons réutilisent FSM)
- story-537 (méta-progression toutes armes unlock après boss final)
