# Story-535 — 6 Ennemis V1 FSM + Contre-strats (Mission 4.3 GDD)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_enemies.json`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT
> **Scale BMAD** : Standard
> **Effort estimé** : ~5 jours
> **GDD ref** : [Mission 4.3](../design/gdd-roguelite-v1.md#43-catalogue-ennemis-v1-6)
> **Prérequis** : story-528 (FPS feel), prefer 531-534 (armes pour test contre-strats)

## Pourquoi

Variation gameplay : actuellement 1-2 ennemis types. Cible 6 archetypes V1 distincts avec contre-stratégies par arme = force rotation armes + build variety boons (chaque ennemi favorise certaines armes).

## Acceptance Criteria

### 6 ennemis FSM

- [ ] AC1 — 🤖 **Cage Marchante** : FSM walk→stop→tire(0.5s télégraphe)→walk. Hitscan 10 dmg. SFX *"GRRR !"*
- [ ] AC2 — 🏃 **Cage Rapide** : chase + melee swing close. 15 dmg. SFX *"Clink clink !"*. Outline jaune
- [ ] AC3 — 🛡️ **Cage Tank** : slow walk + AOE ground pound 3m radius. 25 dmg, télégraphe 1.5s. SFX *"BOUM BOUM !"*. Outline rouge
- [ ] AC4 — 🎯 **Cage Sniper** : stationary perché, charge laser visible 1.5s line gizmo, tire. 30 dmg. SFX *"Tch-tch !"*. Outline violet
- [ ] AC5 — 💣 **Cage Boomer** : court vers joueur, explose on death/proximity. AOE 40 dmg radius 4m. SFX *"Bip-bip-BIP !"* accélère. Outline orange clignote
- [ ] AC6 — 🔮 **Cage Mage** : stationary, lance projectile homing slow 8 m/s. 20 dmg. SFX *"Hmmmmm !"*. Outline cyan + glow

### Outlines + SFX

- [ ] AC7 — Outline shader couleur par archetype (forgia-effects ou material wrapping)
- [ ] AC8 — Sound cue 0.5s pre-attack distinct par archetype (audio_biome map étendue)
- [ ] AC9 — Anti-canon : ennemi à 0 HP = "ZzzZ" floating text + ☁️ smoke poof + ragdoll soft. Jamais "die"

### Sensors

- [ ] AC10 — `forgia2_enemies.json` : count par archetype alive, kills/run par archetype, contre-strat efficiency par arme

## Files
- `crates/forgia-ai-arena-bot/src/archetypes/` NEW : marchante.rs, rapide.rs, tank.rs, sniper.rs, boomer.rs, mage.rs
- `crates/forgia-effects/src/enemy_outline_shader.rs` NEW
- `assets/genomes/roguelite_enemies.toml` (extend with 6 archetypes)

## Anti-canon
- "S'endorment" sur kill
- Noms français cartoon ("Cage Marchante", pas "Walker Cage")

## Cross-refs
- GDD V1 Mission 4.3
- Bible v1 lore "Cage" = arme volée par Forgeron Noir
- story-536 (boss réutilisent FSM patterns)
