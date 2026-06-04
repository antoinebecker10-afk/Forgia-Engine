# Story-561 — Points d'Intérêt (POI) : peupler les anchors de gameplay

> **Status** : DONE (part libre livrée 2026-06-03 — `forgia-stage` jamais touché, voie restée gelée). Suivi optionnel `forgia-stage` (cf §9) différé.
> **Scale** : Standard (~5 fichiers, story requise, checklist post-impl)
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Roadmap** : Phase 4 "le contenu" — voir [ROADMAP_ROGUELITE.md](../ROADMAP_ROGUELITE.md)
> **Série décor** : 561 (POI) → 562 (structures) → 563 (verticalité)

---

## 1. Contexte

Demande user (2026-05-29) : *« comment on peut améliorer le décor ? C'est assez
basic, y'a pas d'étage, de bâtiment dans lesquels on peut entrer, des zones
d'intérêts etc. »*

### État vérifié

- Arène = **hexagone plat unique** (90m, roguelite_stages.toml `crypts_of_anvil`),
  ceinturé de murs KayKit (ramparts), couverts dispersés par solveur sight-line
  (`forgia-stage/src/layout.rs`).
- ✅ **Bonne nouvelle** : `AnchorKind` expose déjà `PoiSlot`, `Landmark`,
  `LootZone`, `Teleporter`, `SniperPerch`, `MeleePit`, `FlankRoute`
  (`forgia-anchor/src/lib.rs:43`). `anchor_slots = 6` configuré.
- ⚠️ Mais ces anchors servent aujourd'hui de **points d'ancrage de modules de
  couvert**, PAS de zones à intérêt gameplay. Aucun contenu (loot vault, hazard,
  vantage, forge physique).

### Précision code 2026-06-02 (lecture `forgia-stage`)

Le pipeline POI **existe déjà à moitié** — 561 n'est PAS un pipeline neuf, c'est
une **couche gameplay par-dessus un spawn déjà en place** :

- `poi_anchor_positions(extent, slots, seed)` (`forgia-stage/src/lib.rs:593`) —
  positions POI déterministes dans l'hex (testé, seed-stable).
- `pick_poi_weighted()` (`forgia-stage/src/lib.rs:719`) — choisit un prefab POI
  pondéré depuis un pool.
- spawn `PrefabSpawn::new(&poi.prefab, pos).with_name("POI_{slot}")` +
  `AnchorPoint::new(AnchorKind::PoiSlot, slot)` (`forgia-stage/src/lib.rs:942-969`).

⇒ Des prefabs POI **sont déjà posés et visibles**, mais purement **cosmétiques**
(aucune interaction, aucun hazard, aucun flag run). Le delta 561 = brancher
loot / lave / forge + types data-driven + sensor **SUR ces spawns existants**.
Conséquence scope : moins de travail dans `forgia-stage` (juste typer l'anchor),
plus de travail dans `forgia-mode-roguelite` (la mécanique).

Cette story = la **moins risquée** des 3 (pas de navigation verticale, reste au sol).

---

## 2. Vision

Donner une **raison d'explorer** l'arène au lieu de tourner en rond. Chaque POI
= une micro-décision risque/récompense, lisible pour un enfant :

- **Vault de loot** (`LootZone`) : un coffre/tas de Souls bonus, mais gardé /
  exposé (zone ouverte sans couvert → risque).
- **Zone de lave** (hazard, biome Volcanic) : dégâts au contact, mais raccourci
  ou zone de kill environnemental (pousser ennemis dedans = synergie knockback).
- **Perchoir vantage** (`SniperPerch`) : position haute avec ligne de vue, prime
  le placement (sans verticalité complexe : une plateforme basse accessible).
- **Forge physique** (`Landmark`) : le Coffre du Forgeron devient un **lieu** dans
  l'arène (story-558 l'a en overlay UI ; ici on lui donne une présence physique +
  emissive + le maître forgeron).

Bible : tout est **mignon et lisible** — la lave est orange cartoon, le vault
brille, pas de piège sournois invisible (Perfect Information).

---

## 3. Acceptance Criteria

### AC1 — POI data-driven dans le genome ✅ **OBLIGATOIRE**

- Schema POI dans `roguelite_stages.toml` (ou nouveau `roguelite_poi.toml`) :
  type (`loot_vault` | `lava_hazard` | `vantage` | `forge_landmark`), prefab GLB,
  reward/danger params, biomes autorisés
- Le solveur de layout assigne les types POI aux `anchor_slots` (pas de hardcode position)
- Hot-reload Shift+F12

### AC2 — Vault de loot fonctionnel ✅

- `LootZone` anchor → spawn un prefab visuel (coffre KayKit) + un gain Souls bonus à l'interaction (touche Interact, déjà mappée `PlayerAction::Interact`)
- Exposé : placé hors des clusters de couvert (le solveur le sait déjà via spacing)
- 1 seul loot par run par vault (pas de farm infini)

### AC3 — Hazard de lave (biome Volcanic) ✅

- Zone trigger (sensor Rapier) qui inflige des dégâts périodiques au contact (joueur ET ennemis)
- Ennemi poussé dedans (knockback boon / Bourrasque) = mort environnementale → synergie satisfaisante
- Visuel : emissive orange cartoon + (optionnel) particules ashfall déjà prévues
- ⚠️ Lisibilité : bord clairement marqué (pas de dégât invisible) — anti-frustration enfants

### AC4 — Forge physique (Landmark = le Coffre) ✅

- Le Coffre du Forgeron (story-558, overlay UI) gagne une **présence physique** : un prefab forge à un `Landmark` anchor, emissive, vers lequel le joueur se dirige pendant le break
- (Optionnel) ouvrir l'overlay Coffre par proximité + Interact plutôt qu'auto, pour un sens du lieu

### AC5 — Observability ✅ **OBLIGATOIRE**

- `forgia2_stage_poi.json` (ou étendre `forgia2_roguelite_state.json`) :
  liste des POI placés (type, position), `loot_vaults_looted`, `lava_kills_total`,
  `forge_visited`
- Permet de vérifier "les POI spawnent-ils ?" sans relancer

---

## 4. Hot path check

- [ ] Hazard lava : trigger Rapier sensor, pas de scan distance par frame sur tous les ennemis (utiliser collision events)
- [ ] Dégât périodique : `Timer` par entité dans la zone, pas d'alloc
- [ ] Spawn POI = OnEnter stage, pas par frame
- [ ] Systèmes `.in_set(GameSet::*)` + `run_if(in_state(GameMode::Roguelite))`

---

## 5. Fichiers candidats (Standard ~5)

| Fichier | Rôle | Isolation (2026-06-02) |
|---|---|---|
| `assets/genomes/roguelite/roguelite_poi.toml` (NEW) | définitions POI data-driven (type + prefab + reward/danger + biomes) | ✅ libre |
| `crates/forgia-mode-roguelite/src/poi.rs` (NEW) | **gros du travail** : loot vault interactable + lava hazard (Sensor Rapier) + forge landmark | ✅ libre (crate non touchée) |
| `crates/forgia-mode-roguelite/src/lib.rs` | wire systems (`run_if(in_state(GameMode::Roguelite))`) | ✅ libre |
| sensor `forgia2_stage_poi.json` (writer AC5) | observability | ✅ libre |
| `crates/forgia-stage/src/{layout.rs,lib.rs}` | typer l'anchor POI (pondération par biome) — petit delta vu §1 | 🔴 **BLOQUÉ** |

⚠️ **Coordination — confirmé 2026-06-02** : `forgia-stage/src/lib.rs` EST en cours
d'édition par un autre terminal (18 fichiers non-commités : `forgia-stage`,
`forgia-foliage`, `forgia-rpg`, `forgia-anim-locomotion`). Règle multi-terminal
§3.3 : **ne pas éditer `forgia-stage` tant qu'il n'est pas pushé** (merge conflict
garanti). Séquencement recommandé :

1. Livrer d'abord la **part libre** (`poi.rs` + `roguelite_poi.toml` + sensor) en
   se branchant sur le spawn POI existant (`POI_{slot}` / `AnchorKind::PoiSlot`).
2. Faire l'**assignation type→anchor** dans `forgia-stage` **en dernier**, une
   fois la crate libérée (`git diff HEAD --name-only` ne montre plus `forgia-stage`).

Baseline avant 1er Edit : `cargo check -p forgia-stage` **et**
`cargo check -p forgia-mode-roguelite` (build-baseline sain, règle §3.2).

---

## 6. Test in-game (récap obligatoire)

1. **Action** : lancer Roguelite. Explorer l'arène : trouver le vault, marcher près de la lave, aller à la forge pendant le break.
2. **Redémarrage** : `cargo run`. POI params → Shift+F12.
3. **Effet attendu** :
   - Vault visible (coffre brillant) → Interact (E) → gain Souls + "ouvert" une fois
   - Marcher dans la lave → dégâts + écran feedback ; pousser un ennemi dedans → il meurt
   - Pendant le break, la forge est un lieu physique emissive
4. **Sensor** : `forgia2_stage_poi.json` → POI list non vide, `lava_kills_total` incrémente quand on push un ennemi dedans
5. **Variantes si KO** :
   - POI pas visibles → vérifier assignation anchor + prefab path
   - Lava ne fait rien → vérifier sensor Rapier `Sensor` + collision events (pas raycast)
   - Vault farmable → vérifier flag `looted` par run

---

## 7. Definition of Done

- [x] AC1-AC5 livrés (AC4 = présence physique forge ; proximité-open Coffre différée, cf §9)
- [x] `cargo check` + clippy 0 warning (`forgia-mode-roguelite`)
- [x] Sub-agents verifier + qa-lead — 2 "Majeur" relevés (561-01/04) **vérifiés faux positifs** (cf §10)
- [x] Sensor `forgia2_stage_poi.json` + registre mis à jour (SENSOR_REGISTRY.md)
- [x] Récap in-game fourni (cf réponse session 2026-06-03)
- [x] 0 hardcode position (anchors solveur) / reward (genome `roguelite_poi_gameplay.toml`)
- [x] Story DONE
- [ ] Commit (en attente — non commité, cf coordination autre terminal)

---

## 8. Coupes assumées

- ❌ Pas de verticalité (story-563)
- ❌ Pas d'intérieurs praticables (story-562)
- ❌ Pas de POI à objectif scénarisé (one-shot story-driven hors scope V1)
- ❌ Vault = **walk-over** (pas Interact) — pattern `stations.rs` éprouvé, 0 dep `forgia-input`. Interact-to-open = polish différé.
- ❌ Lava **non biome-gated** (apparaît selon poids genome, pas seulement Volcanic). Le gating biome-aware appartient au solveur de layout = `forgia-stage` (bloqué). Différé au suivi §9.

---

## 9. Suivi `forgia-stage` (différé, crate gelée autre terminal)

Quand `forgia-stage` est libéré (`git diff HEAD --name-only` ne le montre plus) :

1. **Cosmétique** : `forgia-stage` spawne un prefab POI générique + glow à CHAQUE
   `PoiSlot` (lib.rs:954-985). Mes POI gameplay se posent par-dessus. Overlap
   inoffensif (glow sous le coffre = joli), mais on peut supprimer le prefab
   générique là où un type gameplay est assigné, OU faire correspondre le prefab.
2. **Biome-aware** : typer l'anchor (`AnchorKind::LootZone`/`Landmark` au lieu de
   `PoiSlot` générique) + pondérer lava sur biome Volcanic dans le solveur →
   mon consumer keyerait sur le kind précis au lieu de tirer lui-même.

Tant que ce n'est pas fait, le découplage actuel (consumer tire le type via
`RunSeed`+slot) est complet et fonctionnel — pas un blocage.

---

## 10. Note auto-QA (2026-06-03)

Le sub-agent qa-lead a relevé **BUG-561-01 (Majeur — "bot zombie HP=0 non
despawné")** et son corollaire **BUG-561-04 (heal_on_kill cassé)**. Les deux sont
des **FAUX POSITIFS**, vérifiés adversarialement avant tout patch :

- `despawn_dead_cubes` (`forgia-fps/lib.rs:421`) filtre `With<TargetCube>` ET
  trigger `DeathEvent` avant despawn (lib.rs:430).
- Les `ArenaBot` portent **TargetCube + forgia_combat::Health** (`waves.rs:120,132`).
- Donc un bot tué par la lave (HP combat=0) est despawné par `despawn_dead_cubes`
  qui émet `DeathEvent` → loot + heal_on_kill + defeat fonctionnent.
- Mon `sys_lava_tick` est **identique** au pattern shippé `shockwave.rs`
  (story-572/573). Le fix proposé par qa-lead (`commands.trigger(DeathEvent)`
  dans sys_lava_tick) **recréerait** le double-DeathEvent que le code interdit
  explicitement (shockwave.rs:13-14) → double loot/heal.

Leçon : vérifier la composition réelle des entités avant d'agir sur un finding
"haute confiance". Cf `feedback_adversarial_verify_before_runtime_ship`.
