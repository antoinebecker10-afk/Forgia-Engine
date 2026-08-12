# Story-562 — Structures praticables (intérieurs plain-pied)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_entities.json`, fichier `layout.rs`, symbole `ArenaBot`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **Status** : DRAFT (2026-05-29)
> **Scale** : Standard→Enterprise (~6-9 fichiers — dépend du coût IA, voir §1)
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Roadmap** : Phase 4 "le contenu" — voir [ROADMAP_ROGUELITE.md](../ROADMAP_ROGUELITE.md)
> **Série décor** : 561 (POI) → **562 (structures)** → 563 (verticalité)
> **Dépend de** : story-561 recommandée d'abord (POI valide le pipeline anchor→prefab)

---

## 1. Contexte

Demande user : *« des bâtiments dans lesquels on peut entrer »*. Aujourd'hui les
murs KayKit sont un **périmètre** (ramparts), il n'y a aucun intérieur.

Cette story ajoute des **structures où le joueur ENTRE** mais reste **au sol**
(plain-pied, pas d'étage — la verticalité est story-563). KayKit Dungeon Pack a
déjà les pièces (rooms, couloirs, portes) — assets disponibles (cf
[[reference_v2_kaykit_assets_minimal_copy]]).

### ⚠️ Le vrai coût = l'IA (décision de scope)

Le `ArenaBot` se déplace au sol avec du LOS. Des intérieurs créent :
- des **murs intérieurs** qui cassent le LOS (le bot peut "voir" à travers ?)
- des **goulots** (portes) où le KCC peut se coincer (déjà vécu story-540)
- des **embuscades** où le bot doit savoir entrer/sortir

**Décision à trancher en début de story** :
- **Option A (Standard)** : structures = couvert lourd contournable, bots **restent dehors / ne rentrent pas** (LOS gère naturellement, le joueur s'en sert comme refuge). Faible risque.
- **Option B (Enterprise)** : bots **poursuivent dans les intérieurs** (waypoints/flow-field aux portes). Gros risque nav, à isoler.

**Recommandation** : livrer A d'abord (refuges tactiques), B en story suivante si le playtest le réclame.

---

## 2. Vision

Des **ruines/bâtiments KayKit** où le joueur entre pour :
- se mettre à l'abri (regen, recharger, respirer pendant une vague)
- tendre une embuscade (sortir, tirer, rentrer)
- trouver un POI caché (loot vault intérieur — lien story-561)

Bible : ruines mignonnes de la forge (enclumes géantes, fours éteints), pas de
donjon glauque. Lisibilité : les entrées sont **évidentes** (arches larges,
emissive au seuil).

---

## 3. Acceptance Criteria

### AC1 — Décision de scope IA actée ✅ **BLOQUANT EN PREMIER**

- Choisir Option A (bots dehors) ou B (bots dedans) — documenter dans la story + sensor
- Si A : valider que LOS exclut correctement (mur intérieur bloque la vue bot→joueur)
- Si B : créer story Enterprise dédiée nav (NE PAS bricoler ici)

### AC2 — Structures data-driven ✅

- Schema structure dans genome : prefab GLB KayKit (room/corridor), position (anchor), rotation, biomes
- Placées sur anchors `PoiSlot`/`Landmark` par le solveur (réutilise pipeline story-561)
- Colliders cohérents : murs solides, sol intérieur navigable, **seuil de porte franchissable** par capsule joueur (radius 0.3, marge ≥1.2m — cf CORRIDOR_KEEPOUT story-540)

### AC3 — Le joueur peut entrer/sortir sans se coincer ✅

- Portes/arches ≥ 2m de large (anti-stuck KCC)
- Sol intérieur plat continu avec le sol arène (pas de marche qui bloque le KCC — autostep ou ramp)
- Test stuck : sensor `forgia2_physics.json` (story-549) `kcc_stuck` reste 0 dans/autour des structures

### AC4 — Couvert / refuge fonctionnel ✅

- Intérieur casse le LOS ennemi (le joueur est en sécurité relative)
- (Option A) bots attendent/patrouillent les sorties au lieu de rentrer
- Pas de cheese total : structure traversante (2 sorties) pour éviter le camp invincible

### AC5 — Observability ✅ **OBLIGATOIRE**

- Étendre `forgia2_stage_poi.json` : `structures_placed`, `player_inside_structure` (bool), réutiliser `kcc_stuck` de `forgia2_physics.json`
- Health check : si `kcc_stuck > 0` près d'une structure → alerte (régression story-540)

---

## 4. Hot path check

- [ ] LOS bot : déjà géré (`exclude_sensors` story-2026-05-22), vérifier que les murs intérieurs sont raycast-bloquants
- [ ] `player_inside_structure` : trigger Rapier sensor, pas de scan par frame
- [ ] Spawn structures = OnEnter stage
- [ ] Draw calls : les rooms KayKit ajoutent des GLB — surveiller `forgia2_entities.json` (budget, cf story-540 540 walls = pression draw-call)

---

## 5. Fichiers candidats (~6-9)

| Fichier | Rôle |
|---|---|
| `assets/genomes/roguelite/roguelite_structures.toml` (NEW) | définitions structures + prefabs KayKit |
| `crates/forgia-stage/src/layout.rs` | placement structures sur anchors + keepout portes |
| `crates/forgia-mode-roguelite/src/structures.rs` (NEW) | spawn rooms + colliders + triggers inside |
| `crates/forgia-mode-roguelite/src/lib.rs` | wire |
| `crates/forgia-ai-arena-bot/...` | (Option B uniquement) traversal portes — SINON ne pas toucher |
| `crates/forgia-observability/...` | sensor AC5 |
| `assets/...kaykit dungeon rooms` | prefabs (déjà copiés V1→V2) |

⚠️ **Coordination CRITIQUE** : `forgia-ai-arena-bot` est touché par l'autre
terminal (standup 2026-05-29). **NE PAS éditer cette crate sans coordination.**
Préférer Option A qui n'y touche PAS. Baseline `cargo check -p forgia-stage`.

---

## 6. Test in-game (récap obligatoire)

1. **Action** : entrer dans une structure, recharger à l'abri, ressortir tirer.
2. **Redémarrage** : `cargo run`. Positions → Shift+F12.
3. **Effet attendu** :
   - Bâtiment visible avec entrée évidente (arche emissive)
   - On entre **sans se coincer**, on est à l'abri du tir ennemi
   - (Option A) les bots attendent dehors
4. **Sensor** : `forgia2_stage_poi.json::player_inside_structure=true` quand dedans ; `forgia2_physics.json::kcc_stuck=0`
5. **Variantes si KO** :
   - Coincé à la porte → élargir arche / vérifier keepout / autostep
   - LOS traverse les murs → vérifier collider mur = solide raycast
   - Draw calls explosent → réduire nombre de structures / LOD

---

## 7. Definition of Done

- [ ] AC1 (décision scope) actée AVANT impl
- [ ] AC2-AC5 livrés
- [ ] `cargo check` + clippy 0 warning
- [ ] `kcc_stuck=0` confirmé runtime (pas de régression story-540)
- [ ] Sub-agents verifier + qa-lead
- [ ] Récap in-game fourni
- [ ] Story DONE + ROADMAP mise à jour

---

## 8. Coupes assumées

- ❌ Étages / verticalité (story-563)
- ❌ Option B nav intérieure SI risque trop élevé → story dédiée
- ❌ Destruction de structures (hors scope V1)
