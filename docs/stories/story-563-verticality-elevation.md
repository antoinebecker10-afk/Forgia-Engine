# Story-563 — Verticalité : étages, plateformes, multi-niveaux

> **Status** : DRAFT (2026-05-29)
> **Scale** : **Enterprise** (10+ fichiers, plan mode requis, IA navigation)
> **Owner** : Claude Opus 4.8 (1M)
> **Bible** : v1 cartoon family-friendly (cf [[reference_bible_forgia_roguelite_v1]])
> **Roadmap** : Phase 4 "le contenu" — voir [ROADMAP_ROGUELITE.md](../ROADMAP_ROGUELITE.md)
> **Série décor** : 561 (POI) → 562 (structures) → **563 (verticalité)**
> **Dépend de** : 561 + 562 livrées et stables (pipeline anchor + structures plain-pied validé)

---

## 1. Contexte

Demande user : *« y'a pas d'étage »*. L'arène est strictement plate (un seul
plan de sol, modules de couvert 1-1.75m max).

**⚠️ Cette story est la plus risquée des 3 — Enterprise, à faire en DERNIER,
isolée.** Raison : la verticalité casse la navigation IA au sol.

### Le problème de fond — IA navigation verticale

Le `ArenaBot` actuel : déplacement au sol + LOS. Il **ne sait pas** :
- monter une rampe / un escalier vers un étage
- pathfinder en 3D (multi-niveaux)
- gérer le joueur "au-dessus de lui" (LOS vertical, tir vers le haut/bas)

Sans navmesh ou flow-field 3D, ajouter des étages = des bots qui restent en bas,
tournent en rond, ou se coincent. **C'est le cœur de l'effort, pas la géométrie.**

Référents : la verticalité Resistance/Insomniac (3m/6m) est déjà citée dans
`level_modules.toml` comme inspiration — mais jamais implémentée niveau nav.

---

## 2. Vision

Un combat qui utilise la **3e dimension** : prendre la hauteur pour l'avantage,
les ennemis qui menacent depuis plusieurs niveaux, des rampes/plateformes qui
récompensent le mouvement (lien story-560 slide/sprint pour atteindre des perchoirs).

Bible : verticalité **douce et lisible** (rampes larges, pas de saut de précision
punitif — cible enfants). Pas de plateforming hardcore.

---

## 3. Acceptance Criteria (ébauche — à raffiner en plan mode)

### AC1 — Décision navigation IA ✅ **BLOQUANT — plan mode AVANT tout code**

Choisir l'approche nav (chacune un trade-off documenté) :
- **A. Navmesh** (ex: `oxidized_navigation` ou recast) — robuste, gros setup, dep externe
- **B. Flow-field / waypoint graph 3D** maison — contrôle total, effort dev élevé
- **C. Verticalité "fake"** : étages accessibles au joueur seulement, bots restent au sol mais peuvent **tirer en hauteur** (LOS 3D) — **le plus pragmatique pour V1**

**Recommandation forte V1 : Option C** (verticalité = avantage joueur + menace
ranged ennemie, sans nav 3D complète). A/B = post-V1.

### AC2 — Géométrie multi-niveaux data-driven ✅

- Plateformes / rampes / étages définis en genome (hauteur, prefab, anchor)
- Rampes ≥ pente douce franchissable KCC (autostep limité — tester)
- Garde-corps / bords lisibles (pas de chute accidentelle frustrante)

### AC3 — Combat vertical fonctionnel ✅

- LOS 3D : ennemis au sol peuvent voir/tirer le joueur en hauteur et vice-versa
- Le joueur en hauteur a un avantage de placement (récompense la prise de hauteur)
- (Option C) bots ranged (Sniper) deviennent pertinents contre joueur perché

### AC4 — Pas de stuck / pas de chute infinie ✅

- `forgia2_physics.json::kcc_stuck=0` sur rampes/plateformes
- Floor safety net (story-453, player/lib.rs:528) étendu aux niveaux
- Bots ne tombent pas en boucle / ne se bloquent pas au pied des rampes

### AC5 — Observability ✅ **OBLIGATOIRE**

- `forgia2_verticality.json` : `player_elevation_m`, `levels_count`, `kcc_stuck`, `bot_pathing_failures`
- Health check : `bot_pathing_failures > seuil` → alerte (régression nav)

---

## 4. Hot path check (Enterprise — détailler en plan)

- [ ] LOS 3D = toujours 1 raycast/bot (pas N) — respecter Lock L4 EditorRaycast budget
- [ ] Pas de pathfinding par frame — recompute throttlé / event-driven
- [ ] `par_iter` bots si N > 32
- [ ] Draw calls multi-niveaux surveillés

---

## 5. Fichiers candidats (Enterprise 10+)

À cartographier en **plan mode** (/plan). Touche probablement :
`forgia-stage`, `forgia-mode-roguelite`, `forgia-ai-arena-bot` (nav — ⚠️ autre
terminal), `forgia-player` (KCC rampes), `forgia-observability`, genomes, +
éventuelle dep navmesh.

⚠️ **Coordination CRITIQUE** : touche `forgia-ai-arena-bot` (autre terminal actif).
Cette story ne démarre PAS tant que l'IA n'est pas coordonnée / l'autre terminal
mergé. Baseline complète + claim check obligatoires.

---

## 6. Test in-game (récap — à compléter)

1. **Action** : monter une rampe vers un étage, tirer d'en haut, observer les bots.
2. **Redémarrage** : `cargo run`.
3. **Effet attendu** : on monte sans se coincer ; en hauteur on a l'avantage ; les bots ranged nous menacent ; aucun bot bloqué au pied de la rampe.
4. **Sensor** : `forgia2_verticality.json::player_elevation_m > 0` en hauteur, `bot_pathing_failures=0`, `kcc_stuck=0`.
5. **Variantes si KO** :
   - Coincé sur rampe → adoucir pente / autostep
   - Bots stupides → confirmer Option C (pas de nav 3D promise), ou escalader nav navmesh
   - Chute infinie → floor safety net par niveau

---

## 7. Definition of Done

- [ ] **Plan mode** complété (/plan) AVANT tout Edit — décision nav AC1 actée
- [ ] AC2-AC5 livrés
- [ ] `cargo check` + clippy 0 warning
- [ ] `kcc_stuck=0` + `bot_pathing_failures=0` runtime
- [ ] Sub-agents verifier + qa-lead + edge-case-hunter (Enterprise)
- [ ] Récap in-game fourni
- [ ] Story DONE + ROADMAP mise à jour

---

## 8. Coupes assumées (V1)

- ❌ Navmesh complet (Option A/B) → post-V1 sauf si playtest le réclame
- ❌ Plateforming de précision (anti-bible enfants)
- ❌ Bots qui pathfindent en 3D (Option C = bots sol + LOS 3D suffit V1)
- ❌ Ascenseurs / téléporteurs verticaux (anchor `Teleporter` existe mais hors scope ici)
