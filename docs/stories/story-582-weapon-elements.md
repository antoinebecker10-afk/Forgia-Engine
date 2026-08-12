# Story-582 — Système d'éléments par-arme (Roguelite, Phase A)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_elements.json`, fichier `elements.rs`, symbole `Health`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : EN COURS (2026-06-07)
> **Scope BMAD** : Enterprise (multi-crate, system gameplay cœur)
> **Track** : SHIP Roguelite (vision pivot 2026-06-04 — Gunfire Reborn-like)

## Contexte

Refonte du loop d'améliorations Roguelite vers un **système d'éléments par-arme**
type Borderlands × Gunfire Reborn (décision user 2026-06-07). Chaque arme a un
élément signature avec une efficacité différente par type d'ennemi (matchups) +
des status effects (burn/poison DoT) + AOE / exécution.

Phase A (cette story) = le **cœur gameplay**, testable dans l'arène sans toucher
au parcours. Phases suivantes (B parcours gauntlet, C couronne/rétrécissement)
= stories séparées.

## Mapping arme → élément (identité fixe)

| Slot | Arme | WeaponType | Élément | Fort contre |
|------|------|-----------|---------|-------------|
| 1 | Pépin (pistolet) | ModernAR | 💥 Explosif (AOE) | groupes Runners |
| 2 | Bourrasque (SMG) | AssaultRifle | 🔥 Feu (burn DoT) | Runners |
| 3 | Madame Lenoir (sniper) | Shotgun | 🎯 Perforant (exécution) | Tanks / Boss |
| 4 | Boucherie (pompe) | RocketLauncher | 🟣 Poison (DoT + shred) | Tanks |

## Décision d'architecture (critique)

- **Deux types `Health`** : ennemis = `forgia_combat::Health`, joueur =
  `forgia_damage::Health`. `forgia_damage::DamageEvent` → `apply_damage` ne mute
  QUE `forgia_damage::Health` → **n'affecte pas les ennemis** (le chain boon
  existant est un no-op silencieux sur eux).
- **Donc** : le système d'éléments mute **`forgia_combat::Health` directement**.
  `despawn_dead_cubes` (forgia-fps:423) fait le pont : `current ≤ 0` → trigger
  `DeathEvent` → observers loot/heal/defeat. Kill credit = source=None (cohérent
  avec tous les kills enemy existants).
- **Aucune édition `forgia-fps` / `forgia-damage`** : tout vit dans
  `forgia-mode-roguelite/elements.rs`, lit `CombatHitEvent` (pattern ChainTargets).

## Critères d'acceptation

- AC1 — Mapping data-driven : chaque arme applique son élément (sensor le montre).
- AC2 — Matchups : Lenoir one-shot un Tank (×2.0 + exécution), pas un Runner via SMG.
- AC3 — Burn (Feu) : un ennemi touché à la SMG perd des PV ~3 s après le dernier tir.
- AC4 — Poison (pompe) : stacks (max 5), DoT + shred (+dmg reçu/stack).
- AC5 — AOE (pistolet) : un tir touche les ennemis dans 3.5 m du point d'impact.
- AC6 — Exécution (sniper) : instakill si cible < 25 % PV après le hit.
- AC7 — 100 % data-driven : `assets/genomes/roguelite/roguelite_elements.toml`
  hot-reload (mtime, Shift+F12-like). Default Rust = miroir exact (zéro régression).
- AC8 — Observable : `forgia2_elements.json` (mapping + hits/élément + DoT actifs +
  executes), severity `warn` si `always_on=0`.
- AC9 — `cargo check -p forgia` + clippy 0 warning + tests unitaires verts.

## Phase A — incréments

1. A1 — Element enum + ElementConfig (genome mtime) + ElementStats.
2. A2 — `sys_apply_elements_on_hit` : matchup bonus sur `forgia_combat::Health`.
3. A3 — StatusBurn/StatusPoison + `sys_tick_element_status` (DoT groupé 0.5 s).
4. A4 — AOE explosif + exécution perforante.
5. A5 — Sensor `forgia2_elements.json` + health check `always_on`.

## Suite (hors Phase A)

- Phase B — parcours gauntlet (retirer plateforme par défaut, départ→portail fin→
  niveau suivant, checkpoints, choix 1-parmi-3 qui débloque/tier les éléments).
- Phase C — couronne / rétrécissement cat-size (player scale system).
- VFX colorés (balle de feu / violette / impact explosif) = micro-incrément A5-bis.
