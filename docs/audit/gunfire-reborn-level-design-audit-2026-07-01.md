# Audit de Parité Level Design — Gunfire Reborn ↔ Forgia Roguelite

> **Date** : 2026-07-01
> **Auteur** : Level Designer / Architecte Forgia
> **Périmètre** : 6 axes de level design (structure macro, types de salles & flux, encounters combat, procédural vs authored, interactables & jalons, boucle de niveau cible)
> **Statut du code Forgia** : branche `feat/arena-authored-shell-625`, V7 M3 (jusqu'à story-636)
> **Nature** : audit lecture-seule + roadmap de parité. Aucun code modifié.

---

## 1. Résumé exécutif

### 1.1 Constat central — arène-vagues, PAS traversée de salles

Forgia Roguelite est aujourd'hui structuré comme une **arène-vagues bouclée** (Hadès-like chamber), pas comme la **traversée de salles clear-to-progress** de Gunfire Reborn. Concrètement :

- Le joueur reste dans **UNE arène** (`crypts_of_anvil` ou `forge_sanctum`) où s'enchaînent **3 vagues** (`waves.rs:WAVES_TOTAL=3`), la 3ᵉ contenant le boss.
- Après le boss, il passe dans un **parcours platformer bonus** (`loot_room.rs`, kit ithappy ~1200 pièces), puis **re-boucle l'arène**.
- Il n'y a **ni actes, ni biomes enchaînés, ni salles connectées, ni choix de chemin**. La progression multi-stage du stage-graph est **générée mais jetée** (`run.rs:736-742`, TODO story-471..479).

Gunfire, à l'inverse, est un **room-graph** : 4 zones linéaires × (1 safe zone + 3 field battles + 1 boss), chaque salle se clear en tuant tous les ennemis, un waypoint guide vers la suivante, et le chemin à travers un pool de salles authored varie run-to-run.

> **La bonne nouvelle** : ~50 % des briques nécessaires existent déjà dans le repo mais sont **dormantes ou débranchées** — le stage-graph complet (`forgia-stage/graph.rs`), le portail de choix (`hud.rs:618 draw_portal_overlay` stub), le pattern portail→sous-niveau→retour (`loot_room.rs`), la coquille authored data-driven (`arena_layouts.toml`). **L'écart est un écart d'assemblage et de câblage, pas de fondation.**

### 1.2 Scorecard de parité

| # | Axe | Parité | Verdict | Priorité de comblement |
|---|-----|:------:|---------|:----------------------:|
| 1 | Structure macro (actes / biomes / stages / boss) | **38 %** | Run linéaire configurable OK, mais **0 concept d'acte/biome enchaîné**, 1 seul boss terminal | **P0** |
| 2 | Types de salles & flux de traversée | **32 %** | Gate-par-clear OK, mais **stage-graph non consommé**, portail de choix **dormant** | **P0** |
| 3 | Design de salle & encounters (combat) | **32 %** | Vagues + boss + cover/verticalité OK, mais **1 arène qui boucle**, **0 élite/challenge-room/vault** | **P0** |
| 4 | Assemblage procédural vs authored | **38 %** | Coquille authored + overlay procédural OK, mais **1 layout figé par stage**, **pas de pool de salles** | **P1** |
| 5 | Interactables & jalons dans les niveaux | **52 %** | Pickups/boons/marchand/parcours OK, mais **squelette mono-arène**, **pas de Statue de Bénédiction/pré-boss** | **P1** |
| 6 | Boucle de niveau cible | *(synthèse)* | — | **P0** |

**Parité globale pondérée : ~38 %.** Les 3 axes les plus bas (structure, salles, encounters) convergent tous vers **le même verrou** : la boucle de run est mono-arène au lieu d'être une traversée multi-salles.

### 1.3 Thèse de comblement

Un **seul chantier structurel** débloque la majorité des écarts des 6 axes : **dé-stubber le stage-graph pour transformer l'arène-vagues bouclée en run multi-salles clear-to-progress**. Tout le reste (élites en dernière vague, safe-zone pré-boss, biomes distincts, Statue de Bénédiction, vaults, portes multiples) se greffe dessus. Il ne faut **PAS** commencer par l'art des biomes (effort L, bloquant contenu) ni par un générateur de géométrie procédurale (Gunfire prouve que l'assemblage d'un pool authored suffit).

---

## 2. Axe 1 — Structure macro (actes / biomes / stages / boss)

### 2.1 (a) Spec Gunfire Reborn — chiffrée

| Élément | Valeur Gunfire |
|---|---|
| Actes/biomes de base | **4** en séquence linéaire : Longling Tomb → Anxi Desert → Duo Fjord → Hyperborean Jokul |
| Niveaux par acte | **6 / 5 / 5 / 4** (~20 niveaux par run complète) |
| Structure d'un niveau | 1er = safe zone, dernier = boss ; entre = **3 field battles + 1 boss battle** |
| Boss | **4 principaux** (1/acte) + **3 alternatifs** débloquables = **7 boss** (Golem après 5× Lu Wu, Wind God après 5× Ichthyosaurus, Abyssal Serpent après 3× Yoruhime-Maru) |
| Mini-boss/élites | Pas systématiques — dans les **Vaults** optionnels |
| Ce qui change entre actes | Palette + thème visuel, roster d'ennemis dédié, faiblesse élémentaire du boss, difficulté croissante |
| Longueur d'une run | **40–90 min** (les « 30h+ » = maîtrise cumulée, pas une run) |
| Post-boss final | Mode **Endless Journey** débloqué (Réincarnation niv. 9) |

> **Piège à éviter** : les DLC PAYANTS ajoutent héros + armes, **pas** d'actes. Les actes viennent des updates gratuites. « Vault of the Sea » / « The Peak » ne sont **pas** des noms d'actes officiels.

### 2.2 (b) État Forgia — fichiers

- **Longueur de run** : `forgia-stage/graph.rs:238` (default 5, clamp 1-12) + `roguelite_run.toml:17` (gene `roguelite_stage_count`, default 4.0).
- **Distribution stage types** : `graph.rs:58-97` (StageKind : Combat 53 %, Event 22 %, Rest 12 %, Elite 8 %, Shop 5 %, + Boss forcé). Pattern Slay-the-Spire (floors forcés, anti-consécutif, diversité intra-depth).
- **Boss** : `graph.rs:185-186` (`boss_depth() = total_stages - 1`) → 1 boss terminal unique, `EnemyArchetype::Boss` scalable + enrage 50 % HP (`waves.rs:311-357`).
- **Biomes** : `run.rs:109-118` (`stage_id_for_depth` : depth pair → crypts volcanique, impair → forge plaines — **alternance hardcodée**).
- **Data-driven partiel réel** : `roguelite_stages.toml` définit par-stage biome (Volcanic/Plains), arena_extent, ramparts_kit, music_state, boss_pad_required, weather_override (**2 stages seulement**).
- **Director budget** : `graph.rs:280-285` (`base * mult^depth`) + `roguelite_run.toml:43-60` (base 2.0 credits/s, mult 1.25/stage).

### 2.3 (c) Écart

| Manque | Effort | Risque |
|---|:---:|---|
| **Concept d'ACTE** (Act I..IV regroupant N stages + 1 boss + 1 thème) | M | Refonte `generate_run_graph` + `RunState`, risque sensor `forgia_stage_graph.json` |
| **Boss par acte** (4 principaux) au lieu d'1 terminal | M | Registre de boss data-driven, impact `combat_running`/`boss_portal` (supposent 1 boss) |
| **Boss alternatifs** débloquables (compteur kills persisté) | M | Nouvelle surface `MetaShopSave` à versionner |
| **Roster + faiblesse élémentaire par biome** | M | Archétypes génériques réutilisés partout ; croise story-636 anim + budget draw-calls |
| **≥4 biomes art-complets** (palette/kit/ennemis) | **L** | Coût art majeur — pipeline data prêt, **contenu manque** (plus gros écart de contenu) |
| **Vaults spatialisés** (fissure → défi → coffre) | M | Généraliser le portail `loot_room.rs` en événement optionnel intra-stage |
| **Difficulté par paliers d'acte** | S | `director_budget` déjà data-driven, mais dépend du concept d'acte |
| **Endless Journey** post-boss-final | M | Nécessite condition de fin de run formelle (débat design tranché 2026-06-17 : plus d'écran Victoire auto) |

### 2.4 (d) Plan

**Premier pas concret** : introduire le concept d'ACTE **dans le genome**.
1. Ajouter dans `roguelite_run.toml` une section `[acts]` (ex. 3 actes, chacun avec `stage_count` + `biome_pool`).
2. Ajouter un champ `act` / `boss_archetype` dans `roguelite_stages.toml`.
3. Modifier `generate_run_graph` (`graph.rs:298`) pour **segmenter le RunGraph par acte** et faire `forced_kind_for_depth` (`graph.rs:142`) forcer un Boss à la fin de **CHAQUE acte** (pas seulement `total-1`).

Réutilise 100 % de la fondation (RunGraph, `roguelite_stages.toml`, director budget, boucle boss→portail→Return de `loot_room.rs`). Débloque boss-par-acte, difficulté par palier, transition inter-acte.

---

## 3. Axe 2 — Types de salles & flux de traversée

### 3.1 (a) Spec Gunfire Reborn — chiffrée

Gunfire n'utilise **PAS** de node-map explicite. Structure : **segments → petites salles reliées**, la randomisation n'OUVRE QUE certaines portes d'un pool de salles connectées (les hors-chemin sont **murées**). Flux :

1. On entre → **porte se ferme derrière** → tuer tous les ennemis (souvent en vagues) → portes s'ouvrent **avant ET arrière** → **waypoint jaune** vers la salle suivante.
2. **Choix in-world** via icônes de salle : **2 épées croisées = mobs**, **tête de cheval = mini-boss/élite** (→ green chest = 1/3 scrolls), **coffre = trap room**.
3. **Vaults** : cracked wall (couleur par stage) → tirer dessus → portail vers side dungeon → retour au point d'origine. Types : Non-Fighting / Fighting / Elite / Special.
4. **Intermission stages** (safe zones) entre niveaux : **Peddler + Craftsman**. Avant chaque boss, les deux apparaissent **TOUJOURS ensemble**.
5. **Portail à tête d'ours** après le boss de Zone 3 → débloque Zone 4.

### 3.2 (b) État Forgia — fichiers

- **Gate-par-clear** : `waves.rs:sys_wave_orchestrator` (poll `bots_alive==0` → break 15s → next wave) + `boss_portal.rs:sys_reconcile_boss_gate` (porte fermée jusqu'à `boss_defeated`).
- **Portails** : `loot_room.rs:89-96` (PortalKind : Enter/Next/Return, anneaux émissifs vert/orange).
- **Parcours 3-zones** : `loot_room.rs:1-335` (kit ithappy, chest-to-ground colliders incrémentaux, fall-to-checkpoint, ZoneReward story-589).
- **Stage-graph** : `graph.rs:54-97` (7 StageKind, distribution StS, déterminisme SplitMix64).
- **Portail de choix (DORMANT)** : `hud.rs:562-626` (`draw_portal_overlay()` **no-op** ligne 618, `PORTAL_KEYS` + `stage_kind_display()` **#[allow(dead_code)]** — champs `pending_portal_choices`/`chosen_variant_idx` retirés au refactor 471-479).
- **ZoneReward** : `loot_room.rs:156-196` (1-parmi-3, touches 1/2/3).
- **Dispatch stage** : `run.rs:99-118` (`stage_id_for_depth` : alternance biome hardcodée).

### 3.3 (c) Écart

| Manque | Effort | Risque |
|---|:---:|---|
| **Salles typées avec icône preview in-world** (2 épées/tête de cheval/coffre) | M | Ossature écrite (`stage_kind_display` icônes/couleurs), surtout câblage UI+spawn |
| **Portes murées hors-chemin** (illusion d'embranchement) | **L** | Vrai layout multi-salles, impact géométrie + pathfinding IA |
| **Vaults gated** (cracked wall → tir → side-dungeon → retour) | **L** | Concept neuf, touche combat/spawn/persistence/netcode/cleanup OnExit |
| **Peddler + Craftsman pré-boss** (toujours ensemble) | M | Marchand existe (`merchant.rs`), upgrade existe (`meta_shop.rs`) — orchestration + Craftsman in-run |
| **Récompense typée par salle** (blue box combat/green élite) | S | Généraliser ZoneReward hors du parcours |
| **Structure multi-zones à niveaux variables** (6/5/5/4) | **L** | Refactor structure de run (persistence, save between levels) |
| **Composition ennemis par TYPE de salle** | S | `wave_composition()` global → fonction du kind, data-driven |

### 3.4 (d) Plan

**Premier pas concret** : câbler le stage-graph au flux de salles.
1. Remplacer `stage_id_for_depth(depth)` (`run.rs:109`) par une lecture de `graph.stages[depth].kind` (le RunGraph est déjà généré dans `sys_start_run:678` mais **jeté après** `total_stages`/`boss_depth`).
2. Ré-activer `draw_portal_overlay` (`hud.rs:618`) pour afficher 2-3 salles-cibles avec leur icône via `stage_kind_display()` (déjà écrit, dead_code).
3. Restaurer les champs `pending_portal_choices`/`chosen_variant_idx` sur `RogueliteWave` (ou une Resource dédiée).

**Débloque en un seul chantier la preview de salle typée + le choix de chemin** — le cœur du level design Gunfire — en réutilisant du code dormant, **sans refonte géométrique**.

---

## 4. Axe 3 — Design de salle & encounters (combat)

### 4.1 (a) Spec Gunfire Reborn — chiffrée

- **Clear = tuer TOUS les ennemis** ; spawns **scriptés et mémorisables** (ordre fixe par salle, PAS aléatoire).
- **Late-spawns** à déclencheurs : (a) positionnel (franchir une zone), OU (b) progression (kill de X mobs / toute la vague en cours). **Mélange vagues + triggers, pas un dump instantané.**
- Difficulté croissante par vague, **élites en dernière vague**.
- **2 archétypes de layout** : couloirs serrés + salles-hub (Longling Tomb) ; niveaux ouverts + cover rochers + rues far-west (Anxi Desert).
- **Cover + verticalité + headshots = compétences cœur.**
- **Élites** : versions uniques de mobs, AoE punitif en petite salle (spin de l'Elite Spearman « couvrait toute la salle »).
- **Salles-défi** : cube rouge (1-2 vagues, protéger l'objet → green box), variante treasure dragon (temps limité).
- **Arène défense** : 5 vagues + boîte rouge à bouclier (échec si bouclier cassé).
- **Vaults** : 4 types (Non-Fighting / Fighting / Elite / Special), fissures colorées par stage.

### 4.2 (b) État Forgia — fichiers

- **Spawn vagues 3 phases** : `waves.rs:1-403` (W1=8 ennemis 3T/3R/2S, W2=12 4T/4R/4S, W3=1 Boss+4R), break 15s + HP restore + Coffre Forgeron.
- **Archétypes différenciés** : `enemies.rs:1-200` (Tank HP120/Runner HP35/Sniper HP45/Boss HP800 enrage).
- **Sight-line solver** : `layout.rs:1-1383` (6 invariants story-485 : sight-line brisée CoverHigh <40m, cover spacing ≥3m, SniperPerch edge, MeleePit central, anti-spawn-camp 8m, hex inscrit 0.866×).
- **Corridor circulation** : `layout.rs:73-74` (CORRIDOR_KEEPOUT 1.5m).
- **Arène authored** : `authored.rs` + `arena_layouts.toml:31-221` (24 pièces Crypts : fosse mêlée r9.6m, perchoir tour 47m, cover haut/bas).
- **Bible narrative** : `docs/lore/locations/crypts_of_anvil.md:39-77` (6 sections, palette rouge braise + rose pastel + bleu champignon).
- **Détection stratifiée** : `enemies.rs:59-116` (Tank 22m/Runner 40m/Sniper 55m/Boss 80m).

### 4.3 (c) Écart

| Manque | Effort | Risque |
|---|:---:|---|
| **Late-spawns scriptés à déclencheurs** (zone + kill) | M | Système triggers + sous-vagues, risque de casser gate `seen_alive`/`bots_alive` |
| **Élites** (EnemyArchetype::Elite + AoE signature) | M | `elite_pad`/`wave_elite` en data sans consumer ; équilibrage AoE petite salle |
| **Salles-défi cube rouge / treasure dragon** | **L** | Encounter system événementiel complet (défends l'objet, timer, fail-state) |
| **Arène défense 5 vagues + bouclier** | M | Fail-state par destruction d'objet (n'existe pas — seul player death = Defeat) |
| **Vaults** (4 types dont puzzle/trap-maze) | **L** | Cross-crate, sous-systèmes d'épreuve + puzzle inexistant |
| **Run linéaire multi-zones** (6/5/5/4) | **L** | Refactor structurel majeur (dé-stubber stage-graph) |
| **Archétype couloir serré** | M | Nouveau générateur/preset, interférence corridor-keepout + invariants sight-line |

**Densité de cover** : ~0.0014/m² vs cible Hadès ~0.05/m² = **35× moins dense** (audit 2026-06-26 §5.3). Tier 3 story-626 propose de densifier **autour des points focaux**, pas uniformément.

### 4.4 (d) Plan

**Premier pas concret** : dé-stubber le stage-graph pour transformer le single-arène-3-vagues en run multi-salles clear-to-progress.
- Dans `run.rs:~736-742`, remplacer le fallback `spawn_wave_enemies(1)` dans une arène unique par une boucle `RunState::InRun{stage}` qui, au `boss_defeated` (`waves.rs:257`), fait `set_next_state(InRun{stage+1})` + re-déclenche `forgia-stage::spawn_stage_arena_on_request` avec `RunSeed::stage_seed(stage)` (déjà présent).
- **1er incrément minimal** : 3 stages combat + 1 stage boss enchaînés, chacun clear-to-progress, **avant** de toucher aux élites/challenge-rooms/vaults.

C'est le **prérequis structurel** de tous les autres écarts.

---

## 5. Axe 4 — Assemblage procédural vs authored

### 5.1 (a) Spec Gunfire Reborn — chiffrée

Modèle **HYBRIDE room-graph** (ni full-procedural ni full-authored) :

- Chaque stage possède un **POOL fixe de salles handcrafted** (« bucket of unique rooms »), assemblées (« stitches together ») en chemin semi-aléatoire.
- **AUTHORED (fixe)** : nombre de salles, biome, boss, ordre des acts, pool de salles, thème visuel.
- **PROCÉDURAL (varie)** : (1) sélection/connexion des salles, (2) **choix de 2 portes parmi plusieurs** dans une salle, (3) spawn ennemis, (4) loot/coffres/vaults.
- **Salles FIXES et réutilisées** — le CHEMIN change. « The maps aren't generated inch-by-inch. »
- **Verdict** : ~80 % de la variété = **assemblage + spawns/loot**, PAS génération géométrique. **Investir dans un pool authored + graphe de connexion + variantes de portes, pas dans un générateur de géométrie.**

### 5.2 (b) État Forgia — fichiers

- **Coquille authored** : `arena_layouts.toml:1-221` + `authored.rs:1-221` (ArenaPiece : prefab/pos/rot/scale/role/walkable/blocker/section).
- **Policy** : `authored.rs:56-65` (`suppress_procedural_modules:bool`).
- **Overlay procédural** : `layout.rs:95-224` (`place_modules`, dart-throw + invariants) + `level_modules.toml` (4 kinds : CoverCluster/CoverWall/SniperPerch/MeleePit).
- **Équilibre par stage** : Crypts = 100 % authored (`suppress=true`, 24 pièces) ; Forge = 100 % procédural.
- **Pattern room-graph EXISTE côté villages** : `forgia-procgen-graph` (nodes+edges) + `forgia-village-generator` (Poisson+Voronoi+routes) + worldgen recipes (grammaire base/body/cap).
- **Déterminisme** : `lib.rs:57-68` (splitmix64), `RunSeed.stage_seed(depth)`.

### 5.3 (c) Écart

| Manque | Effort | Risque |
|---|:---:|---|
| **Pool de salles authored par stage** (primitive centrale) | **L** | Investissement art + refonte schema TOML + loader ; si pool trop petit → répétition (feedback Gunfire) |
| **Room-graph d'assemblage intra-stage** (K salles → chemin seedé) | **L** | Placement/orientation salles, raccords portes, navmesh transition ; pattern nodes/edges existe (villages) mais adaptation 3D jointive non triviale |
| **Portes multiples** (2 tirées sur N) | M | Concept 'door slot' dans ArenaPiece, dépend du pool + room-graph |
| **Vaults** (mur fissuré → side-dungeon → 4 types) | M | Brique `loot_room.rs`/`poi.rs` présente, manque mur destructible + tirage d'épreuve |
| **Lookup data-driven stage_id** (remplace hardcode) | **S** | Déjà TODO V2 `run.rs:107`, petit refactor, faible risque |

### 5.4 (d) Plan

**Premier pas concret** : transformer le schema de `arena_layouts.toml` d'« un layout figé par stage » vers « un POOL de salles authored par stage ».
1. Ajouter `[layouts.<stage>.rooms.<room_id>]` dans le TOML.
2. Étendre `authored.rs` (`ArenaLayoutsGenome` → `HashMap<stage, Vec<Room>>`).
3. Faire tirer par `spawn_stage_arena_on_request` une salle du pool via `req.seed` (splitmix64 déjà en place).

Prérequis de tout le reste (room-graph, portes, vaults). Réutiliser le pattern nodes/edges de `forgia-procgen-graph` pour la phase de connexion. **NE PAS créer de générateur de géométrie voxel.**

---

## 6. Axe 5 — Interactables & jalons dans les niveaux

### 6.1 (a) Spec Gunfire Reborn — chiffrée

3 piliers déterministes + jalons probabilistes :

| Jalon | Fréquence | Effet |
|---|---|---|
| **Goblet → Ascension** | 1 garanti / niveau (coffre bleu près du portail) | Choix 1/3 Ascensions (buff run) |
| **Peddler + Craftsman** | Systématique salle pré-boss | Peddler = or (buns/munitions/armes/scrolls) ; Craftsman = level-up arme ×2 |
| **Statue de Bénédiction (Spiritual Remnant)** | Début de chaque acte (Réincarnation) | 3 blessings en Soul Essence, refresh payant cap 60 |
| Vaults | Très haute chance / niveau, non garanti | Occult scrolls |
| Salles-défi | Parkour (timer 2 min → green box), trap (blue box), cube rouge (green box) | Coffres bleu/vert |
| Drops Elite | Par élite | Bun + Arme + Scroll + Golden Goblet (roll indépendant) + poignée Essence |
| Pickups | En jeu | Or (Peddler) + Soul Essence (méta, ×1.25 R1 → ×1.55 R8) + magnétisation |

### 6.2 (b) État Forgia — fichiers

- **Anchors** : `forgia-anchor/lib.rs:43-101` (11 types : PlayerSpawn/PoiSlot/BossPad/Teleporter/LootZone/cover/lane).
- **POIs data-driven** : `poi.rs` (Loot Vaults 50 Âmes fixes, Lava Hazard, Forge/Coffre) + beacons lumineux + sensor `forgia2_stage_poi.json`.
- **Stations** : `stations.rs` (4× Health +30 HP, 4× Ammo, reset par stage).
- **Pickups** : `run.rs:328-598` (cœurs HP, Or Coin GLB, Wisps d'Âmes 8 % normaux/4 au boss) + magnétisation `run.rs:487-519` (38 m/s).
- **Boons** : `forgia-rpg-data::boons::CoffreSession` (Coffre Forgeron, 1-parmi-3 au break) + ZoneReward parcours (`loot_room.rs:156-196`).
- **Parcours items** : couronne (rétrécissement 0.4×), cœur (+20 HP max), pièces/étoiles.
- **Marchand** : `merchant.rs:59-139` (position FIXE, ammo/heal/revive) + L'Enclume méta (`meta_shop.rs`).

### 6.3 (c) Écart

| Manque | Effort | Risque |
|---|:---:|---|
| **Squelette multi-chapitres à biomes distincts** | **L** | Refacto run-loop (cross-crate), casse cycle wave/parcours/persistence |
| **Statue de Bénédiction** (blessings en Âmes début d'acte) | M | Réutilise panneau merchant/Coffre + catalogue TOML |
| **Peddler+Craftsman pré-boss** | M | Gate merchant sur RunState pré-boss + second étal Craftsman |
| **Loot Vault probabiliste** (scrolls, variantes rares) | M | `RoguelitePoiConfig` data-driven existe, enrichir contenu |
| **Drops Elite multi-récompense** | M | Flag Elite + table de loot dédiée |
| **Trap/Challenge rooms + treasure dragon** | M | Réutilise `loot_room.rs`, logique défense-objet + timer |
| **Multiplicateurs méta-monnaie par difficulté** (R1→R8) | M | Resource DifficultyTier, data-driven |
| **Explosive barrels destructibles** | M | Destruction physics + loot, budget frame |
| **Jump pads** | S | Walk-over + impulsion verticale, isolé |

### 6.4 (d) Plan

**Premier pas concret** : implémenter le squelette multi-chapitres (débloque presque tous les autres jalons).
1. Ajouter `ChapterProgress { index: u8, count: 3 }` dans `run.rs` (Resource, reset au start).
2. Dans `sys_reconcile_boss_gate` (`boss_portal.rs`), au lieu de re-boucler l'arène, incrémenter `chapter.index`, sélectionner un preset `arena_layouts.toml` + biome (BiomeMap) et rescaler `wave_composition` (`waves.rs`).
3. Gater une 4e zone optionnelle derrière le boss du chapitre 3.
4. Exposer `forgia2_run_progress.json` (chapter index, biome, wave).

**Effort L, risque High → passer en BMAD Standard avec story dédiée avant tout Edit (≥2 crates touchées).**

---

## 7. Boucle de niveau cible

### 7.1 La boucle à répliquer (modèle Gunfire)

```
[ACTE N — biome distinct]
   │
   ├─ SAFE ZONE (niveau 1)          → 0 ennemi, Peddler/Statue de Bénédiction, prep
   │
   ├─ FIELD BATTLE 1 (salle typée)  → entrée → porte ferme → clear vagues → porte ouvre → waypoint
   │     └─ CHOIX DE CHEMIN : icône preview (2 épées / tête de cheval / coffre)
   │        └─ [optionnel] VAULT : cracked wall → tir → side-dungeon → coffre → retour
   │
   ├─ FIELD BATTLE 2 (salle typée)  → idem, layout alterné (couloir vs hub)
   │
   ├─ FIELD BATTLE 3 (salle typée)  → élite en dernière vague
   │
   ├─ SALLE PRÉ-BOSS                → Peddler + Craftsman TOUJOURS ensemble
   │
   └─ BOSS (niveau final)           → boss d'acte + faiblesse élémentaire → portail acte N+1
                                         │
                                         └─ Goblet → Ascension (1/3)
[boucle vers ACTE N+1 — biome suivant] … puis Endless Journey post-boss-final
```

### 7.2 Mapping vers les briques Forgia existantes

| Beat de la boucle cible | Brique Forgia existante | État |
|---|---|---|
| **Chaîne d'actes/stages** | `forgia-stage/graph.rs` (RunGraph complet, 7 StageKind, StS) | ✅ **généré mais jeté** (`run.rs:736`) → **à consommer** |
| **Salle typée + choix de chemin** | `hud.rs:618 draw_portal_overlay` + `stage_kind_display()` (icônes/couleurs) | ⏸️ **dormant, dead_code** → **à réactiver** |
| **Gate-par-clear** | `waves.rs:sys_wave_orchestrator` + `boss_portal.rs:sys_reconcile_boss_gate` | ✅ **fonctionnel** |
| **Transition inter-salle (portail → sous-espace → retour)** | `loot_room.rs` (Portal Enter/Next/Return, checkpoints, touch-gate z) | ✅ **existe** (usage unique parcours) → **à généraliser** |
| **Coquille authored par biome** | `arena_layouts.toml` + `authored.rs` (24 pièces Crypts) | 🟡 **1 layout/stage** → **passer au pool** |
| **Vagues + boss + enrage** | `waves.rs` (3 vagues, W3=boss) + `sys_boss_enrage` | ✅ **fonctionnel** |
| **Cover/verticalité/sight-line** | `layout.rs` (6 invariants) + sniper perch 47m + melee pit | ✅ **fonctionnel** |
| **Safe zone / marchand** | Lobby + `merchant.rs` + `meta_shop.rs` (L'Enclume) | 🟡 **hors flux** → **placer aux stages Shop + pré-boss** |
| **Récompense fin de segment (1/3)** | `loot_room.rs:ZoneReward` + `boons::CoffreSession` | ✅ **existe** → **généraliser hors parcours** |
| **Pickups + magnétisation** | `run.rs` (cœurs/or/wisps + `sys_magnetize_pickups_on_break`) | ✅ **fonctionnel** |
| **Déterminisme seedé** | `splitmix64` + `RunSeed.stage_seed(depth)` | ✅ **fonctionnel** |

### 7.3 Ce qu'il faut assembler / réactiver (par ordre de dépendance)

1. **Consommer le RunGraph** : `run.rs:736` lit `graph.stages[depth].kind` au lieu de `stage_id_for_depth`.
2. **Réactiver le portail de choix** : `hud.rs:618` + restaurer `pending_portal_choices`/`chosen_variant_idx`.
3. **Généraliser le portail sous-espace** : réutiliser `loot_room.rs` (Portal/Checkpoint) pour chaîner N salles au lieu d'1 arène monolithique.
4. **Passer au pool de salles authored** : `arena_layouts.toml` → `[rooms.<id>]` tiré par seed.
5. **Segmenter par acte** : `ChapterProgress` + boss-par-acte + biome par chapitre.

---

## 8. Roadmap de parité priorisée

> **Principe directeur** : commencer par **réactiver/assembler la boucle de traversée** (code dormant + câblage), pas par le contenu art (effort L bloquant) ni un générateur de géométrie.

### P0 — Boucle de traversée (débloque 3 axes ≤ 38 %)

| Story candidate | Effort | Crates touchées | Risque |
|---|:---:|---|:---:|
| **story-637 : Consommer le RunGraph — dé-stubber la boucle multi-stages** (remplacer `stage_id_for_depth` par `graph.stages[depth].kind`, boucle `InRun{stage}` sur boss_defeated) | M | `forgia-mode-roguelite` (run, waves, boss_portal), `forgia-stage` | **High** (refacto run-loop, sensors) |
| **story-638 : Réactiver le portail de choix** (`draw_portal_overlay` + `stage_kind_display` + restaurer champs `pending_portal_choices`) | S | `forgia-mode-roguelite` (hud, waves) | Medium |
| **story-639 : Récompense typée par StageKind** (généraliser ZoneReward hors parcours : blue=combat, green=élite) | S | `forgia-mode-roguelite` (loot_room, poi) | Low |

> **Note BMAD** : story-637 touche ≥2 crates + refacto architectural → **BMAD Standard obligatoire** (story + checklist), et **story-done-gate** avant tout `Status: DONE`.

### P1 — Contenu de salles & jalons (débloque axes 4-5)

| Story candidate | Effort | Crates touchées | Risque |
|---|:---:|---|:---:|
| **story-640 : Pool de salles authored par stage** (`[layouts.<stage>.rooms.<id>]` + tirage seedé) | L | `forgia-stage` (authored, layout), assets genomes | Medium |
| **story-641 : Concept d'ACTE + boss par acte** (`[acts]` genome, `forced_kind_for_depth` boss fin d'acte, `ChapterProgress`) | M | `forgia-stage` (graph), `forgia-mode-roguelite` (run) | High |
| **story-642 : EnemyArchetype::Elite + late-spawns à déclencheurs** (AoE signature, triggers zone/kill) | M | `forgia-mode-roguelite` (enemies, waves) | Medium |
| **story-643 : Peddler+Craftsman pré-boss** (gate merchant sur RunState pré-boss + Craftsman in-run) | M | `forgia-mode-roguelite` (merchant), `forgia-rpg-data` | Medium |
| **story-644 : Statue de Bénédiction** (PoiKind autel, 3 blessings en Âmes début de chapitre, refresh cap) | M | `forgia-mode-roguelite` (poi, run), `forgia-rpg-data` (boons) | Low |

### P2 — Enrichissement & polish (contenu signature)

| Story candidate | Effort | Crates touchées | Risque |
|---|:---:|---|:---:|
| **story-645 : Vaults spatialisés** (cracked wall → tir → side-dungeon → 4 types → coffre) | L | `forgia-stage`, `forgia-mode-roguelite` (combat, loot) | High |
| **story-646 : Salles-défi cube rouge / treasure dragon** (défends-l'objet + timer + fail-state) | L | `forgia-mode-roguelite` (nouveau encounter system) | Medium |
| **story-647 : Portes multiples** (2 tirées sur N, 'door slot' ArenaPiece) | M | `forgia-stage` (authored) | Medium |
| **story-648 : Densité de cover focale (Tier 3)** (~0.05/m² autour des points focaux) | S | `forgia-stage` (layout), assets | Low |
| **story-649 : Difficulté Réincarnation + multiplicateurs Âmes** (DifficultyTier R1→R8) | M | `forgia-mode-roguelite` (waves), `forgia-rpg-data` | Medium |
| **story-650 : Biomes art-complets ≥4** (palette/kit/ennemis/VFX météo par biome) | L | assets, `forgia-stage`, anim (croise story-636) | **Contenu majeur** |

---

## 9. Décisions ouvertes (à trancher par le game-maker)

Ces choix conditionnent la roadmap. **À trancher avant story-637** car ils changent la cible.

### D1 — 🔴 BLOQUANT : Garder l'arène-vagues OU passer à la traversée de salles ?

C'est **la** décision fondatrice. Deux options :
- **(A) Traversée de salles** (parité Gunfire) : dé-stubber le stage-graph, enchaîner N salles clear-to-progress. **Coûteux mais aligné sur la référence.** Débloque 5 des 6 axes.
- **(B) Rester arène-vagues bouclée** (modèle Hadès-chamber actuel) : renoncer à la parité macro/salles, se concentrer sur la profondeur d'UNE arène (élites, vagues plus riches, boss variés). **Moins de refacto, mais parité plafonnée ~40 %.**

> **Ma recommandation** : (A) avec incrément minimal (3 stages combat + 1 boss) — c'est le seul chemin vers la sensation Gunfire, et 50 % des briques sont déjà là (dormantes). Mais **c'est un choix de vision**, pas technique.

### D2 — Nombre d'actes / biomes ?

Gunfire = 4 actes × 6/5/5/4 niveaux. Forgia n'a que 2 stages (Crypts/Forge). Options :
- **3 actes × ~4 stages** (MVP réaliste, tenable côté art).
- **4 actes** (parité stricte, coût art L majeur — plus gros écart de contenu du projet).

> Trancher le **nombre** avant de peupler `[acts]` dans `roguelite_run.toml`.

### D3 — Pool de salles authored vs procédural par biome ?

Gunfire = pool authored + assemblage (80 % variété), **pas** de géométrie procédurale. Forgia a les 2 modèles (Crypts authored, Forge procédural). Options :
- **(A) Tout-authored par pool** (fidèle Gunfire, coût art par salle).
- **(B) Coquille authored + overlay procédural** (modèle Returnal actuel, moins de salles à créer).
- **(C) Hybride par biome** (biomes signature authored, biomes de remplissage procéduraux).

> Impacte directement l'ampleur de story-640.

### D4 — Réactiver le portail de choix maintenant ?

Le code est **dormant** (`hud.rs:618`, dead_code prêt). Le réactiver (story-638, effort S) donne immédiatement la preview de salle typée + le choix de chemin — **cœur du LD Gunfire** — pour un coût faible. **Y a-t-il une raison design de garder un run linéaire sans choix ?** Si non, c'est un quick-win P0.

### D5 — Endless Journey / condition de fin de run ?

Décision user 2026-06-17 : **plus d'écran Victoire auto**, pas de fin de run explicite. Gunfire a une fin (boss final) + Endless Journey. **Faut-il rouvrir ce débat** pour introduire une condition de fin d'acte final ? Sinon, le squelette multi-chapitres boucle indéfiniment sans climax.

### D6 — Élites : spawn dédié ou variante d'archétype ?

`elite_pad`/`wave_elite` existent **en data sans consumer**. Options : (A) nouvel `EnemyArchetype::Elite` avec AoE signature, (B) simple scaling stats d'un archétype existant. Gunfire = **versions uniques de mobs** (design + attaques propres) → penche (A), mais coût anim/VFX.

---

*Document d'audit — lecture seule, aucun code modifié. Les stories candidates (637-650) sont des propositions non créées. Toute story touchant ≥2 crates (637, 640, 641, 645, 646) requiert BMAD Standard + story-done-gate avant `DONE`.*

**Fichiers clés référencés** (chemins absolus workspace `c:\Users\Antoi\Desktop\Forgia Rewrite\`) :
- `crates\forgia-stage\src\graph.rs` — RunGraph / StageKind / director budget
- `crates\forgia-stage\src\layout.rs` — sight-line solver, 6 invariants
- `crates\forgia-stage\src\authored.rs` + `assets\genomes\arena_layouts.toml` — coquille authored
- `crates\forgia-mode-roguelite\src\run.rs` — RunState, `stage_id_for_depth` (à dé-stubber ~736-742)
- `crates\forgia-mode-roguelite\src\waves.rs` — vagues + boss + enrage
- `crates\forgia-mode-roguelite\src\hud.rs:618` — `draw_portal_overlay` (dormant)
- `crates\forgia-mode-roguelite\src\loot_room.rs` — portail sous-espace + ZoneReward
- `crates\forgia-mode-roguelite\src\boss_portal.rs` — porte boss-gated
- `crates\forgia-mode-roguelite\src\poi.rs` / `merchant.rs` / `stations.rs` — interactables
- `assets\genomes\roguelite_run.toml` / `roguelite_stages.toml` — genomes structure de run
