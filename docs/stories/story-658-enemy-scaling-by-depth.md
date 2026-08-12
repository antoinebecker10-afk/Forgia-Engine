# Story-658 — Scaling ennemi par profondeur de salle (la pression)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_enemy_scaling.json`, fichier `enemy_scaling.rs`, symbole `Health`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS
**Niveau BMAD** : Standard (1 module neuf + 1 section TOML + plugin, 1 crate)
**Crate** : `forgia-mode-roguelite`
**GDD** : [gdd-run-structure-weapon-progression.md](../design/gdd-run-structure-weapon-progression.md) — mécanique M1
**Date** : 2026-07-02

## But

Donner son **intérêt plein** à « La Trempe » (story-657) : sans pression, monter son arme
rend le jeu trivial (arme qui monte, rien qui résiste). Ici les ennemis deviennent plus durs
à mesure que la run avance (`RogueliteWave.stage`) : vie + défenses + dégâts. Salle 0 = ×1.0
(référence), salle N = ×(1 + N × per_stage). C'est la boucle Gunfire **pression ↔ réponse**.

## Concept-first (mapping vérifié)

- **Source de profondeur** : `RogueliteWave.stage` (u8, salle courante 0-indexée) — avance
  déjà entre salles (story-646 R2 multi-stage). Pas besoin de consommer le RunGraph complet
  (E2) pour ce scaling.
- **Cibles** : la racine ennemi porte sur la MÊME entité `Health` + `DefenseLayer`
  (shield/armor) + `BotShootConfig` (`waves.rs` parent tuple, lignes 179-193) → tout scalable
  d'une query.

## Choix d'implémentation : POST-SPAWN (multi-terminal)

Le scaling s'applique via `Added<EnemyArchetype>` dans un **module neuf**
(`enemy_scaling.rs`), PAS dans `spawn_wave_enemies`. Raison : `waves.rs` est un fichier chaud
multi-terminal (VFX kill-burst de l'autre terminal + mon head-hitbox) → ce module n'y touche
pas (**zéro collision**). Effet identique : l'ennemi vit ~1 frame à ses stats de base avant
scaling (spawn pendant le break, sans combat → imperceptible). `Added<>` = idempotent (une
fois par entité, jamais de double-scaling).

## Design (cibles — genome, hot-reload)

- `hp_per_stage` = 0.35 (+35 %/salle : vie + bouclier + armure ensemble → EHP réel).
- `damage_per_stage` = 0.15 (+15 %/salle : `BotShootConfig.damage`).
- Salle 2 : ennemis ×1.70 EHP, ×1.30 dégâts. À calibrer sur playtest (croisé au TTK).

## Fichiers

- `crates/forgia-mode-roguelite/src/enemy_scaling.rs` (nouveau).
- `crates/forgia-mode-roguelite/src/lib.rs` : `pub mod` + plugin.
- `assets/genomes/roguelite/roguelite_progression.toml` : section `[scaling]`.

## Hors scope (notés)

- Consommer le `difficulty_budget` du RunGraph (module le NOMBRE d'ennemis) = M1 complet,
  dépend de E2 (RunGraph consommé). Ici on scale les STATS par stage, pas le nombre.
- Scaling intra-salle par vague (`current_wave`) : per-stage suffit pour l'incrément.

## Acceptance criteria

- [x] Test pur : `hp_mul_for_stage(0)` = 1.0 ; croît avec le stage ; parse `[scaling]` en
      ignorant `[trempe]` du même fichier ; fallback Default (6 tests).
- [ ] Runtime : salle 2, les ennemis encaissent nettement plus et tapent plus fort ;
      `forgia2_enemy_scaling.json` montre `stage`/`hp_mul`/`damage_mul`/`scaled_total`.
- [x] `cargo check` vert + clippy 0 warning fichiers touchés + 267 tests verts (+7 scaling).
