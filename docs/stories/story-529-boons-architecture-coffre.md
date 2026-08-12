# Story-529 — Boons Architecture + Coffre UI + 5 boons neutres (Mission 2 GDD)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_boons.json`, fichier `boons_apply.rs`, symbole `ActiveBoons`).
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
> **GDD ref** : [Mission 2.1](../design/gdd-roguelite-v1.md#21-architecture) + [Mission 2.3](../design/gdd-roguelite-v1.md#23-boons-neutres-5)
> **Prérequis** : story-528 (FPS feel) — pas bloquant mais préférable

## Pourquoi

Gap #2 du roadmap : 0 boons mécaniques actuellement. Cible Hadès doctrine = 4 boons par run, 1 boon après chaque wave clear. Sans système boons : aucune raison de refaire un run. **Tier 1 critique commercial.**

## Acceptance Criteria

- [ ] AC1 — Format Boon TOML data-driven : `assets/genomes/roguelite_boons.toml` schema (id, name, voiceline_preview, effect_kind, tags[], rarity, weapon_filter)
- [ ] AC2 — Choix 1 boon après chaque wave clear (3 stages × 3 waves = 9 occasions) + 1 récompense mid-boss
- [ ] AC3 — UI "Coffre du Forgeron" 3 cartes cartoon flip, sprite Maître Forgeron *« Choisis bien ! »*, hover zoom + voiceline preview
- [ ] AC4 — 5 boons neutres impl : "Éclat d'âme nourrissant", "Métal chaud", "Bénédiction de l'Enclume", "Souffle du Maître", "Petit Champignon Lumineux"
- [ ] AC5 — Système Tags simple (`fire`, `ricochet`, `knockback`, `chain`, `precision`, `chaos`) + 3+ tags identiques = légendaire caché unlock prochain coffre
- [ ] AC6 — Boons appliqués via Resource `ActiveBoons` + Event `BoonAppliedEvent` consommé par systèmes affectés
- [ ] AC7 — Sensor `forgia2_boons.json` : count active, tags actifs, légendaires débloqués, choices total session

## Files
- `crates/forgia-roguelite-data` NEW ou extend `forgia-rpg-data` (boons module)
- `assets/genomes/roguelite_boons.toml` NEW
- `crates/forgia-ui-lib/src/coffre_forgeron.rs` NEW
- `crates/forgia-mode-roguelite/src/boons_apply.rs` NEW

## Anti-canon
- Voicelines preview <5s, vocab CE2
- "Coffre" pas "loot box"
- Maître Forgeron NPC narrative

## Cross-refs
- GDD V1 Mission 2.1 + 2.3
- story-530 (catalogue 24 boons + anti-boons)
