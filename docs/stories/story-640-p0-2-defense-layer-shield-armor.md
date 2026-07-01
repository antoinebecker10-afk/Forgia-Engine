# Story-640 — P0-2 : défense tri-couche Vie / Bouclier / Armure

> **Source** : plan de production `docs/audit/forgia-gunfire-masterplan-2026-07-01.md` §5,
> Phase P0, item **P0-2**. Direction : `docs/design/direction-forgia-gunfire.md` §5.
> **Note ID** : demandée « story-639 » mais cet ID est déjà pris (ultimate-techniques-vfx,
> renommée de 596). Prochain libre = **640**.
> **Pourquoi** : c'est LE pilier structurant qui rend Forgia reconnaissable comme Gunfire —
> barres colorées Vie(rouge)/Bouclier(bleu)/Armure(jaune), le bouclier régénère hors combat.
> Débloque le HUD segmenté (P1) et le couplage élémentaire (P0-3 Shock / P0-4 matchup→couche).
> **Scale BMAD** : Standard (≥2 crates : `forgia-damage` + `forgia-mode-roguelite` + `forgia-fps`).
> **Date** : 2026-07-01. **Statut** : ✅ DONE 2026-07-01 — commit `fdaec7d`, validé user (capteur
> `forgia2_shield.json`). Compile + clippy 0-warn + tests verts `forgia-damage` 18/18,
> `forgia-mode-roguelite` 228/228 ; auto-QA verifier + qa-lead passée ; story-gate --story 640 PASS.
> En attente **commit** (branche partagée) + **validation runtime** avant `DONE`
> (story-done-gate G1 exige le git-track). Défauts QA non bloquants tracés en §"Hors scope".

## Objectif
Introduire une couche défensive **au-dessus** des deux `Health` existantes (le joueur
porte `forgia_damage::Health`, les ennemis `forgia_combat::Health` — PIÈGE dual-health).
Un composant `DefenseLayer{shield, armor, ...}` absorbe les dégâts **avant** la Vie
(ordre Bouclier → Armure → Vie), le Bouclier **régénère** après `regen_delay` s. sans
coup. 100 % data-driven (`roguelite_defense.toml`, hot-reload) + sensor `forgia2_shield.json`.
Consomme `EnemyStatsConfig` **live** au spawn → flip `spawn_live:false → true` (P0-1).

## Architecture (décisions critiques)
- **Mécanisme dans `forgia-damage`** (crate atomique neutre, déjà dep de fps + roguelite) :
  `DefenseLayer` (composant générique, agnostique du type `Health`) + `DamageChannel` +
  `absorb()`/`note_hit()`/`regen()` **purs et testables**. Aucun cycle de dep.
- **Joueur** : `apply_damage` (forgia-damage, choke point unique) absorbe via `DefenseLayer`
  s'il est présent sur la cible, AVANT de muter `forgia_damage::Health` (après `HealthGuard`).
- **Ennemis** : le **hit de base** (forgia-fps hitscan `lib.rs:1048`, canal `Physical`)
  passe par `DefenseLayer` avant de muter `forgia_combat::Health`. **JAMAIS de `DamageEvent`
  sur un ennemi** (piège dual-health).
- **Données par archétype + joueur** (shield_max / armor_max / regen) dans
  `roguelite_defense.toml` → `DefenseConfig` Resource (roguelite). Mécanisme = forgia-damage,
  données = mode roguelite (même séparation que `elements.rs`).
- **Couplage élémentaire** : Fire (bonus/DoT) et Poison (DoT pur) frappent **déjà** la Vie
  en direct (canal `TrueHealth` implicite, bypass bouclier) — ✅ « Feu→Vie », « Poison DoT pur »
  émergent sans toucher `elements.rs`. **Électrique→Bouclier** (canal Shield) arrive en
  **P0-3** (Shock n'existe pas encore). **Perforant→Armure** (drain armure + ×fort) = **P0-4**
  (re-route matchup→couche). Cette story livre la STRUCTURE + le canal `Physical` du hit de base.

## Critères d'acceptance
| # | AC | Preuve |
|---|---|---|
| AC1 | `DefenseLayer` (shield/shield_max/armor/armor_max/regen_rate/regen_delay/since_hit) + `DamageChannel{Physical,TrueHealth}` + `absorb`/`note_hit`/`regen` purs dans `forgia-damage` | `defense.rs` |
| AC2 | `apply_damage` absorbe le bouclier/armure du joueur avant la Vie (après `HealthGuard`) | `forgia-damage/lib.rs` |
| AC3 | `roguelite_defense.toml` (shield/armor par archétype + joueur + regen global) + `DefenseConfig` (Default miroir, hot-reload mtime 1Hz) | genome + `defense.rs` roguelite |
| AC4 | Spawn ennemis consomme `Res<EnemyStatsConfig>` **live** + attache `DefenseLayer` par archétype → `forgia2_enemies.json` passe `spawn_live:true` | `waves.rs` + `run.rs` |
| AC5 | Hit de base joueur→ennemi (forgia-fps) draine le bouclier puis l'armure puis la Vie | `forgia-fps/lib.rs` |
| AC6 | Le bouclier régénère à `regen_rate`/s après `regen_delay` s. sans coup (ennemis + joueur), gaté Roguelite | `defense.rs` roguelite |
| AC7 | Sensor `forgia2_shield.json` (bouclier/armure agrégés ennemis + joueur, regen actif) + health alert si config>0 mais 0 porteur (wiring cassé) | `defense.rs` + registre |
| AC8 | Le joueur reçoit son `DefenseLayer` (attach idempotent Roguelite, retiré OnExit) | `defense.rs` roguelite |
| AC9 | 0 warning clippy, no-hardcode (valeurs → genome/Default), tests purs verts | `cargo clippy` + `cargo test` |

## Critère de fin (masterplan)
Un ennemi porte une couche rouge + bleue ; tirer draine d'abord le bleu (bouclier) puis
le rouge (vie) ; le bouclier se régénère `regen_delay`(=3 s) après le dernier coup ;
`forgia2_shield.json` écrit l'état ; les tests éléments passent toujours.

## Hors scope (séquencé)
- **Électrique→Bouclier** (canal Shield, ×fort vs bouclier) → **P0-3** (avec `Element::Shock`).
- **Perforant→Armure** (bypass bouclier, drain armure) + re-route matchup→couche → **P0-4**.
- **HUD barres segmentées** (rouge/bleu/jaune) → **P1-5** (bloqué par cette méca).
- **Netcode / réplication** `DefenseLayer` → post-ship (solo au launch, champs f32 replication-ready).

### Chemins de dégâts qui NE passent PAS encore par `DefenseLayer` (audit QA #640-A)

Seul le **hit de base** (forgia-fps) + le hit **joueur** (`apply_damage`) sont routés en P0-2.
Les chemins dérivés mutent encore `combat::Health` en direct (2nd pipeline) — à router en **P0-4** :

- **Matchup bonus élémentaire** (`elements.rs:704-718`) — Feu/Poison→Vie = voulu ; Perforant→Armure = P0-4.
- **Combustion + AOE explosif** (`elements.rs:766-808`) — dégâts physiques dérivés, devraient drainer bouclier/armure (P0-4). *(trou de scope relevé par QA, désormais tracé ici)*.
- **Burn/Poison DoT tick** (`elements.rs:816-861`) — bypass **voulu** (Poison DoT pur / Feu→Vie), n'appelle pas `note_hit` par design (le DoT ne gèle pas la régén du bouclier).
- **`CombatHitEvent.damage` = montant PRÉ-absorption** (`forgia-fps:1083`) : depuis P0-2, il diffère du dégât réellement porté à la Vie (`to_health`). Conséquence : le floating number affiche le dégât brut du tir et les effets dérivés (AOE/combustion/chaîne/ultime) se dimensionnent sur le brut. Décision : **inchangé en P0-2** (changer la sémantique de l'event touche 6+ consommateurs et n'a de sens qu'avec le couplage P0-4 + le HUD P1-5). À traiter **avec P0-3/P0-4** — éventuel champ `raw_damage` séparé pour dissocier « dégât du tir » (VFX) et « dégât porté à la Vie » (balance).

## Suite
P0-3 = moteur de réactions générique (`HashSet<Status>` + `ReactionTable`) + `Element::Shock`
(canal Shield → couplage Électrique→Bouclier).
