# GDD — Forgia: The Spared

> **Type** : looter-FPS d'exploration en duo, cœur FPS roguelite · **Plateforme** : PC 1920×1080
> **Statut** : v1 — gravé depuis la session design du 2026-08-09 · mis à jour 2026-08-12
> (HUD duo §4, arène 5v5 post-v1 §6, portes d'architecture §10)
> **Relation aux GDD existants** : [gdd-roguelite-v1.md](gdd-roguelite-v1.md) (missions FPS, boons par arme) et
> [gdd-run-structure-weapon-progression.md](gdd-run-structure-weapon-progression.md) restent valides pour le
> moment-à-moment combat — ils décrivent ce qui devient ici **l'Abîme**. Ce document les chapeaute :
> structure du jeu, méta-progression, narratif, DA. En cas de conflit, ce document gagne au niveau structure,
> les GDD antérieurs gagnent au niveau feel/combat.

---

## 1. Résumé exécutif

**Core concept** — *Deux amis explorent des mondes pour nourrir leur forge, et plongent dans l'Abîme pour
mériter le monde suivant.* Un looter-FPS d'exploration en duo (humain + compagnon-bot d'abord, humain +
humain ensuite), structuré en deux modes qui se nourrissent : les **Expéditions** (explorer, rapporter,
débloquer) et **l'Abîme** (roguelite de descente infinie, qui sert exclusivement à tremper les armes). Le
pivot entre les deux : **la Forge**. L'ascension d'univers en univers est gatée par la **puissance**
(joueur + armes + stuff).

**Public cible** — joueurs de coop FPS roguelite (Gunfire Reborn, Roboquest) + farmers long-terme
(WoW, Warframe, Destiny) + duos d'amis cherchant un jeu-rendez-vous.

**USP vs Gunfire Reborn / Roboquest** :

1. **Le coop se joue *ensemble*, pas côte à côte** — combos élémentaires inter-joueurs (je pose, tu
   détones), draft de boons négocié en équipe, décision d'extraction partagée. Le genre leader fait du
   jeu en parallèle ; nous faisons de l'interdépendance.
2. **Une structure looter à trois chasses** au-dessus du roguelite (puissance / reliques / ultra-rares) —
   la rétention de Warframe sur le moment-à-moment de Gunfire.
3. **Le compagnon-bot** — le duo existe dès le solo, et il est le contrat d'interface du futur coop humain.
4. **Une fiction mémorielle lisible** — l'Oubli contre les Épargnés, une DA où l'état du monde EST l'interface.

**Le nom** — *The Spared* (les Épargnés) nomme le camp du joueur : tout ce qui a échappé à l'Oubli —
vous, vos armes, le Hall, les créatures rares que vous recueillez. Vérifié sans collision Steam
(2026-08-09, seul « SPARED! », jeu de jam sans rapport).

---

## 2. Objectifs & contexte

- **Track SHIP** (CLAUDE.md §1) : ce GDD ne remplace pas le Roguelite en cours — il le **positionne**.
  L'arène actuelle (vagues, stages, boons) devient l'Abîme, cœur combat déjà largement construit.
- **Le track FORGE reflue dans le SHIP** : `forgia-terrain` (BiomeMap Voronoi, streaming chunks, SDF)
  était classé « RPG plus tard » — il devient le moteur des Expéditions. Conforme au modèle deux-tracks.
- **Rationale** : le « jouable sans fin » ne reposait sur rien de structurel (constat session 2026-08-09 :
  courbe de boons plate, pas de raison de revenir). Ce GDD installe l'ascension verticale (puissance),
  le cycle horizontal (Expéditions ↔ Abîme) et trois chasses à horizons étagés.
- **Décision de scope v1** : ship = Expédition solo+compagnon + Abîme existant + Forge + puissance +
  loot. Coop humain temps réel = post-validation (§13). **Arène 5v5 = endgame post-v1**
  (§6 Mode 3, E10, décidée 2026-08-12) : elle ne consomme aucun budget v1, mais fixe trois
  contraintes d'architecture à tenir dès maintenant (§10).

---

## 3. Core gameplay

### Piliers (falsifiables)

| # | Pilier | Test de falsification |
| --- | --- | --- |
| P1 | **Duo d'abord** — toute mécanique à deux = un verbe simple que le bot sait exécuter (tenir, activer, porter, se poster) | Chaque mécanique duo listée au §4 a son verbe bot implémenté ; aucune mécanique duo sans verbe |
| P2 | **Deux modes, une économie chacun** — Expéditions = s'équiper/débloquer ; l'Abîme = tremper les armes, point | Aucune récompense croisée : zéro pièce de stuff dans l'Abîme, zéro XP d'arme au-delà du cap |
| P3 | **La profondeur d'abord** — la base de puissance d'une pièce vient d'OÙ elle tombe ; la qualité est un bonus borné | Un légendaire du palier N < un commun du palier N+2 (borne : bonus qualité ≤ 1 palier) |
| P4 | **Le beau sans stats** — reliques et cosmétiques n'ont aucun effet gameplay | Grep des tables de reliques : zéro champ de stat |
| P5 | **Rien d'indispensable dans le RNG** — les ultra-rares sont des sidegrades exclusifs, jamais de la puissance brute | Aucun contenu gaté par un drop RNG ; DPS des sidegrades ≤ équivalents déterministes |

### Core loop (macro)

```
Expédition ──► stuff (boss/paliers) + matériaux + déblocage ──► Forge ──► armes améliorées
    ▲                                                                          │
    │                                                                          ▼
    └──── gate de puissance franchi ◄──── cap atteint ◄──── l'Abîme (trempe des armes)
```

### Loop d'une Expédition

1. Au Hall : choisir la destination (univers débloqué), partir à deux (joueur + compagnon).
2. Explorer : objectifs non-kill (atteindre, trouver, purger un foyer d'Oubli, rapporter),
   gardiens de palier, traces d'Épargnés à chasser.
3. La pression monte : l'Oubli a remarqué les intrus, son front avance visiblement (§9).
4. **LA décision** : extraire maintenant, ou pousser au palier suivant ?
5. Retour au Hall : la Forge, le journal de collection, les piédestaux.

### Loop de l'Abîme

Descente infinie par paliers de l'arène existante. L'XP d'arme **survit à la mort** (méta-progression) ;
la profondeur **multiplie le taux d'XP** (risque = vitesse de farm). S'éteint au cap d'arme de l'univers
courant, se rallume au déblocage du suivant.

### Victoire / défaite

- **Expédition** : extraction réussie = tout est gardé. Mort = le stuff équipé et l'XP restent ;
  les **matériaux non extraits sont perdus** (recommandation — décision ouverte D2, §14).
- **Abîme** : la mort est attendue (roguelite) ; l'XP d'arme banquée reste. Le rang de profondeur
  s'affiche au tableau (score de RANG existant), sans récompense.

---

## 4. Mécaniques

### Primaires (existant / neuf)

| Mécanique | État | Source |
| --- | --- | --- |
| Mouvement FPS, tir, feel | ✅ existant | `fps_tuning.toml`, `combat_default.toml`, GDD v1 Mission 1 |
| 4 armes à identité + boons par arme | ✅ existant | GDD v1 Mission 2, `roguelite_boons.toml` |
| Éléments & réactions (Shock, Miasma…) | ✅ existant | ReactionTable (forgia-combat) — devient inter-acteurs (neuf) |
| Ultimes à barre d'énergie | ⚙️ base existante | `forgia-combat/src/ultimate.rs` — charge à trancher (D1, §14) |
| Défense tri-couche Bouclier→Armure→Vie | ✅ existant | forgia-damage |
| Compagnon (suivre, verbes duo, combat d'appoint) | 🆕 | Épic E1 — base `forgia-ai-arena-bot` + navmesh |
| Extraction au choix | 🆕 | Épic E2 |
| Foyers d'Oubli à purger (verbe à deux) | 🆕 | Épic E5 |
| Chasse aux traces (Épargnés) | 🆕 | Épic E4 |
| Niveaux d'arme / trempe | 🆕 | Épic E6 |

### Réactions inter-acteurs (le cœur coop)

Chaque acteur (joueur, compagnon, futur binôme humain) porte **un** élément à la fois. Les grosses
réactions exigent deux poseurs : *l'un applique, l'autre détone*. En solo, le compagnon est le second
élément. En duo humain, le compagnon se retire du combo (il reste utilitaire). Les ultimes sont les
applicateurs d'élément de masse — le moment de spectacle.

### Contrôles

Existant : leafwing-input-manager, AZERTY (`KeybindRegistry`). Neuf : 1 touche « ordre au compagnon »
(contextuelle : le verbe dépend de la cible visée) — gene `companion_order_key`.

### HUD duo (décidé 2026-08-12)

Deux besoins distincts, deux surfaces distinctes — ne jamais les fusionner :

- **Les PV du compagnon** : bande permanente à l'écran (modèle Left 4 Dead), **jamais sur la carte**.
  Une carte, ça s'ouvre ; des PV, ça se lit sans rien ouvrir. À deux acteurs, une seule barre suffit.
- **La carte** : minimap permanente, qui sert autant à **donner des ordres** (« va tenir là »,
  « poste-toi ici ») qu'à situer. Elle rend *spatial* l'ordre aujourd'hui limité à la cible visée —
  c'est le seul emprunt au genre MOBA qui transfère réellement en vue FPS. Base : la minimap à
  révélation du V1, à porter selon la doctrine §11 (**porter = corriger**).
- ⚠ **Afficher l'IA monte la barre de qualité qu'on lui demande.** Un mob coincé à 40 m passe
  inaperçu ; un compagnon dont le point ne bouge plus sur la carte se voit toutes les trois secondes.
  La carte rend le navmesh (E1) et un chien de garde de désenlisement non négociables — cf.
  [spawn-clearance.md](../../.claude/rules/spawn-clearance.md) §5, qui documente déjà l'absence des deux.

Carte plein écran : décision ouverte **D5** (§14).

---

## 5. Armes & combat

**Existant, acté** (source réelle des stats : `viewmodel_arena.toml` — ⚠ piège documenté :
`WeaponType::Shotgun` = Lenoir) :

| Arme | Identité (GDD v1) | Gimmick |
| --- | --- | --- |
| Pépin | fusil, confiance | confiance gauge |
| Bourrasque | SMG, chaos joyeux | — |
| Madame Lenoir | précision/patience | — |
| Boucherie | chaos physique | — |

**Neuf — la trempe** : chaque arme a un **niveau**, plafonné par l'univers courant
(gene : `weapon_level_cap_universe_<n>`). L'XP vient de l'usage dans l'Abîme, multipliée par la
profondeur (gene : `weapon_xp_rate_by_depth`, cible design : ×1 au palier 1, croissance motivant la
descente sans rendre les premiers paliers inutiles). La montée de niveau consomme aussi des **matériaux
d'expédition** à la Forge (lien économique entre les deux modes).

**Fiction des armes** : âmes de maîtres-forgerons versées dans leurs œuvres — c'est pourquoi elles
parlent. Le cœur de braise de l'arme **rougeoie quand elle parle**. Design par époque/maître (variété
justifiée par le lore). Qualité = noblesse du matériau (le légendaire est forgé en verre-de-monde).

---

## 6. Structure

### L'écran d'accueil — un menu de préparation, pas un lieu (décidé 2026-08-13)

Le Hall comme **lieu** habité (piédestaux, refuge des créatures) part en **post-v1**, avec l'arène 5v5.
L'accueil de la v1 assume d'être un menu : on s'y prépare, on part vite.

```
   [ EXPÉDITIONS ]   [ ARÈNES ]   [ 5v5 · prochainement ]

   Inventaire · Armes · Talents · Codex · Forgeron
                                    [ Château de Forgia · prochainement ]
```

- **Trois modes visibles d'emblée**, dont un verrouillé. Le joueur voit la forme du jeu au premier
  écran, sans naviguer.
- **Le bouton « Château de Forgia »** est la porte du futur Hall. Verrouillé, mais présent : il dit
  qu'il existe un ailleurs.
- **Le mode choisi est celui qui se lance.** Aujourd'hui le stage est tiré au `run_seed` sans lire
  `SelectedChapter` — le menu promet une destination et en livre une autre (bug playtest 2026-08-09).
- **Le Sac** (page existante) accueille ce que les mobs lâchent, ressources d'arène comprises.
- **Missions = la boussole de puissance**, pas des défis quotidiens : elle dit où en est la puissance
  et ce qui manque pour le gate suivant. Elle **lit** le capteur `forgia2_power.json`, qui décompose
  déjà `boons / perm / mastery / trempe / equip` — elle ne calcule rien.
  ⚠ **Une mission ne donne aucune récompense** : sinon elle devient une troisième source de monnaie et
  perce l'étanchéité du §7. Sa récompense est la puissance gagnée et le gate ouvert.
  ⚠ **Au plus 3 objectifs affichés** — une liste de dix transforme un roguelite en corvée.
- **Retiré de la nav** : `Succès` (aucun haut fait suivi). Le code reste, seul `in_nav` passe à `false`,
  comme `ArenaTest` le 2026-07-30. **Marketplace** : laissé de côté, décision reportée.
- **Le compagnon n'a aucune surface d'accueil** — ni présence, ni état, ni sélection. Manque ouvert (E1).

⚠ **Ce que ces deux « prochainement » engagent** : afficher du contenu verrouillé sur l'écran
d'accueil est une promesse publique. Elle se tient, ou elle se retire — elle ne se laisse pas pourrir.

### Mode 1 — Les Expéditions

- **Mondes procéduraux thématiques** sur `forgia-terrain`. Les 6 ambiances existantes
  (`roguelite_ambiances.toml` : Forge ardente, Crypte suintante, Halles de bois, Nécropole glacée,
  Gorges d'ocre, Cime de pierre) = les 6 premiers univers. Candidats suivants (archétypes plébiscités) :
  abysses océaniques, cité mécanique, monde fongique/féérique, céleste/néant (endgame).
- **Paliers de profondeur** internes : chaque palier = un front d'Oubli plus avancé, clos par un
  **gardien** (boss). Le fond de l'univers N est calibré au niveau d'entrée de N+1.
- **Objectifs non-kill** : atteindre, trouver, purger un foyer, rapporter. Chaque univers introduit
  une mécanique environnementale à deux (obscurité portée, surfaces conductrices d'élément, flore
  sonore…) qu'on **apprend** en la rencontrant — pas de tutoriel.
- **Extraction** : au choix du joueur, à des points dédiés. Rester = plus de butin, front d'Oubli
  qui avance.

### Mode 2 — L'Abîme

Le mode arène actuel (stages, vagues, boons) **reprofilé en descente infinie**. Fiction : le creuset
primordial sous la Forge, où les premiers mondes ont été fondus — on n'y gagne rien, on s'y trempe.
Récompense unique : XP d'arme. Tableau de rang de profondeur sans loot pour les joueurs maîtrise.

### Mode 3 — L'Arène 5v5 (post-v1, E10)

Débloquée au niveau joueur (gene `arena_5v5_unlock_player_level`, cible indicative : 30) — donc
nourrie par les deux modes v1, jamais séparée d'eux. Un MOBA **en vue FPS** : 3 lanes, jungle,
sbires, tours, boutique en match — construit sur le contenu déjà payé (armes, éléments, ultimes,
économie), pas sur du contenu neuf.

**Principe fondateur : 10 *slots*, pas 10 humains.** Tout slot vide est tenu par un compagnon, via
l'interface du §10. Le bot n'est pas un mode dégradé, c'est l'architecture : 1 humain + 9 bots,
3v3 + 4 bots, ou 10 humains — toutes les combinaisons sont valides et jouables. Sans ça le mode
meurt en file d'attente, ce qui a tué Battleborn et Gigantic bien avant leur design.

**Adaptations non triviales du modèle MOBA à la vue FPS** :

| Élément | Adaptation |
| --- | --- |
| **Last-hit** | Impossible au réticule. L'or **jaillit** du sbire mourant : à récupérer — ou à refuser à l'adversaire — en tirant dessus dans une fenêtre courte |
| **Brouillard de guerre** | Redondant en FPS (on ne voit déjà que devant soi). Ce qui survit : minimap limitée à la vision de l'équipe + wards = capteurs déployables |
| **Lanes** | **Verticalité** : lanes étagées, jungle en dessous. L'axe que le genre top-down ne peut pas exploiter |
| **Composition d'équipe** | Les **réactions élémentaires** (§4) : « je pose, tu détones » à deux devient une méta de draft à cinq. Aucun MOBA-FPS n'a cet axe — c'est notre différenciant, et il est déjà à moitié construit |
| Sbires, tours, niveaux, boutique en match | Transfèrent tels quels |

**Dépendances** : navmesh (livré par E1 — les sbires en sont les premiers consommateurs), notion
d'équipe (§10), netcode (E9). **Rien de tout cela n'est codé en v1.**

⚠ Point de funnel à trancher le moment venu : gater le seul mode PvP derrière 30 niveaux de PvE
fait rebondir les joueurs venus pour le PvP. La version à bots permet de les laisser entrer plus tôt.

### Génération & seed

Expéditions : `forgia-terrain` + placement objectifs/foyers/gardiens par seed de run. Abîme :
générateurs d'arènes existants (`roguelite_stages.toml`, `stage_seed(depth)`). Une seed par sortie,
affichée (partage, reproduction de bug — capteur crash existant croise déjà run/wave/seed).

### Persistance (ce qui survit à quoi)

| Élément | Mort en run | Fin de session |
| --- | --- | --- |
| XP / niveau d'arme | ✅ garde | ✅ compte |
| Stuff équipé | ✅ garde | ✅ compte |
| Matériaux non extraits | ❌ perdus (D2) | — |
| Reliques, ultra-rares, journal | ✅ compte | ✅ compte |
| Build de run Abîme (boons) | ❌ reset (roguelite) | ❌ reset |

### Le loot — trois chasses à horizons étagés

| Chasse | Obtention | Règles |
| --- | --- | --- |
| **Équipement** (puissance) | Gardien de palier = pièce **garantie** | Base = univers+palier (gene `gear_base_power_<universe>_<tier>`) · Qualité commun→légendaire = bonus borné (gene `gear_quality_bonus_pct`, **contrainte P3 : ≤ 1 palier**) · v1 : qualité = points seulement, zéro perk |
| **Reliques** (beauté) | Conditions liées au lieu/circonstance de la trouvaille | Zéro stat (P4) · exposées au Hall · le stuff raconte tes voyages |
| **Ultra-rares** (obsession) | Mobs rares — les **Épargnés** — qui se **chassent** : traces, cris, silhouettes semées par la génération | Sidegrades exclusifs jamais indispensables (P5) : compagnons rares, variantes d'ultime, confort · RNG pur pour cosmétiques, **pity** pour tout avantage (gene `rare_pity_ramp`) · liés au compte, jamais au Marketplace · loot personnel en duo |

**Organes de la chasse** : journal de collection (silhouettes grisées + source), piédestaux vides au
Hall. Un Épargné-créature recueilli ne « drop » pas : il **se sauve** et rejoint le Hall — refuge des
Épargnés, ce que le titre du jeu promet.

---

## 7. Progression & balance

> ⚠ Règle Forgia : aucune valeur en dur ici — chaque grandeur nomme son **gene** (couche definition,
> `assets/genomes/`), la cible design est indicative et se règle au playtest.

### La puissance

**Puissance = niveau du joueur + niveau des armes équipées + score du stuff porté.**

Chaque composante vient d'une activité différente (aucun mode skippable) :
stuff ← Expéditions · niveau d'armes ← Forge + Abîme · niveau joueur ← tout.

- Gate dur par univers : gene `universe_power_gate_<n>` (cible : franchi naturellement en ayant
  poussé au fond de l'univers précédent, PAS en refarmant l'entrée — conséquence de P3).
- **Fin de l'expédition N ≈ entrée de N+1** : les tables de puissance des paliers du fond de N
  s'alignent sur `universe_power_gate_<n+1>`.
- **Scaling en bande** : les ennemis d'un univers suivent la puissance de l'escouade entre un plancher
  et un plafond (genes `enemy_scaling_floor_pct` / `enemy_scaling_ceiling_pct`) — on sent sa force,
  on ne roule jamais sur le contenu. Jamais de scaling total (leçon Oblivion).
- ⚠ Un modèle de puissance existe déjà côté Abîme (`power_gain_per_round`) et son capteur est
  **en alerte** (« la puissance réelle dépasse le modèle du mur », 2026-08-09) — la refonte E3
  absorbe ce recalibrage au lieu de le patcher isolément.

### L'arbre de talents (remplace la trempe — décidé 2026-08-13)

> **La trempe devient un arbre de talents.** Même rôle — spécialiser une arme — mais on n'achète plus
> un *niveau*, on achète un **choix**.

**Règle dure : un nœud ne vend jamais un pourcentage.** Chaque nœud est une technique ou un
comportement (« la foudre chaîne sur 3 cibles », « le rechargement actif accélère le tir suivant »),
jamais un `+12 % dégâts`.

*Pourquoi cette règle existe.* L'Enclume des Âmes vendait exactement quatre stats — `max_hp`,
`damage`, `armor`, `gold` — et le capteur `power` est **en alerte** : « la puissance réelle dépasse
largement le modèle du mur ». C'est le motif que le consensus 2026 désigne comme ce qui tue le genre :
la réussite devient dépendante du farm, pas de l'habileté. Un arbre chiffré reproduirait le défaut
en le renommant.

**L'arbre REMPLACE l'Enclume**, il ne s'y ajoute pas : sinon Forgia porte quatre systèmes de
progression (boons, Enclume, arbre, équipement) dont deux font le même travail.

⚠ **Le piège à éviter dans l'arbre lui-même** : si les avantages *joueur* et les techniques *d'arme*
partagent la même monnaie, le joueur prendra toujours les avantages joueur d'abord — ils s'appliquent
partout, tout le temps. Soit deux monnaies, soit les avantages joueur coûtent une ressource
**spécifique à l'arme**, ce qui force à choisir sa spécialisation pour y accéder.

**Socle déjà construit** : `meta_shop.rs` porte un niveau de maîtrise **par arme** (`+1` par run),
son plafond, et le clamp de « niveau effectif » qui évite d'afficher « Niveau 13/6 » sur une vieille
save. L'arbre convertit cet acquis, il ne repart pas de zéro.

### Le cap par univers (cyclique, pas terminal)

`weapon_level_cap_universe_<n>` : l'univers courant borne la spécialisation d'arme. Débloquer
l'univers suivant relève le cap → l'Abîme s'éteint et se **rallume** à chaque étage de l'ascension.

**C'est ce cap qui empêche l'Abîme de devenir la ferme optimale.** Sans lui, le couloir donnerait la
même chose que l'Expédition en plus court et plus sûr — c'est le risque « les modes se cannibalisent »
nommé au §13. Au cap, l'Abîme cesse de lâcher la ressource concernée : *falsifiable au grep des tables*.

### Rattrapage duo (pour le futur coop humain)

Décision ouverte D3 (§14) — options : l'hôte porte son binôme (loot réduit pour le porté), ou drop
boosté dans les univers sous sa puissance. À trancher avant E9 (netcode), pas avant.

### Économie — deux monnaies étanches (décidé 2026-08-13)

| Monnaie | Tombe où | Achète quoi |
| --- | --- | --- |
| **Ressources d'arène** (gene `abyss_resource_drop_<universe>`) | mobs de l'Abîme | **la spécialisation d'arme** — les nœuds de l'arbre, et rien d'autre |
| **Matériaux d'expédition** (gene `forge_material_drop_<universe>`) | Expéditions | **les gates d'univers et l'équipement** |

**Étanches par construction** : aucune ressource d'arène ne peut franchir un gate, aucun matériau
d'expédition ne peut acheter un nœud d'arbre. C'est ce qui fait tenir le pilier P2 — chaque mode
garde une raison d'exister, et le jalon se vérifie au grep des tables.

*Précédent maison* : la séparation Âmes / **Éclats**, décidée le 2026-08-06 — « deux monnaies = deux
lectures qui ne se brouillent pas ». Les Éclats (cosmétique) restent une troisième monnaie.

⚠ **Sa justification écrite meurt avec l'Enclume.** Le code dit aujourd'hui « un cosmétique payé en
Âmes est un rang d'Enclume non acheté » — faux dès que l'Enclume n'existe plus. La séparation reste
juste, pour une **autre** raison, à réécrire : cosmétique et puissance ne partagent jamais un
porte-monnaie, sinon acheter un chapeau coûte une technique.

**Retiré** : « L'Enclume des Âmes » et ses quatre stats — remplacée par l'arbre (§ ci-dessus).
Les ultra-rares et reliques restent hors économie marchande (P4/P5).

**Décision ouverte D6** — les avantages *joueur* de l'arbre : monnaie séparée, ou ressource
spécifique à l'arme ? Cf. le piège signalé plus haut. À trancher avant d'écrire les nœuds.

---

## 8. Level / arena design

- **Expéditions** : les règles existantes s'appliquent intégralement —
  [map-design-intention.md](../../.claude/rules/map-design-intention.md) (spec de combat AVANT la
  géométrie, archétypes ennemis, arrivées déclarées) et
  [map-design-patterns.md](../../.claude/rules/map-design-patterns.md) (14 patterns, mesures dérivées).
  S'y ajoute le **gradient d'Oubli comme langage de navigation** : la désaturation croissante EST la
  carte du danger, aucune UI nécessaire.
- **Abîme** : arènes existantes. ⚠ Capteur `stage_layout` en **erreur** au 2026-08-09 (0 abris pour
  191 attendus) — dette connue, à résoudre dans le cadre arène existant, hors scope de ce GDD.
- **Hazards thématiques** : 1 mécanique environnementale par univers (§6), qui ressert dans les
  profondeurs de l'Abîme une fois apprise.

---

## 9. Art & audio

### DA — toon, verre + braise, et l'Oubli

- **Base** : toon/cel stylisé (pipeline `toon.wgsl` existant, `roguelite_toon.toml`). Argument marché :
  le stylisé vieillit mieux, stream mieux, coûte moins — et tolère l'hétérogénéité d'assets, vital pour
  un jeu construit par IA. Héritage DA **verre + braise** : la braise = la mémoire vivante (âmes,
  forge) ; le verre = la mémoire figée (ce que l'Oubli laisse).
- **L'Oubli — une corrosion de la mémoire** : il se propage comme la rouille (foyers, veines, plaques)
  mais c'est de l'effacement. **4 stades lisibles** :
  1. **Terni** — couleurs voilées, sons assourdis ;
  2. **Pâli** — matières translucides, contours tremblants, les Oubliés rôdent ;
  3. **Vitrifié** — êtres et arbres figés en verre ;
  4. **Effacé** — le vide.
  Purger un foyer fait **remonter la couleur le long des veines** (l'effet Okami — LA récompense
  visuelle du jeu). Genes : `oubli_spread_rate`, `oubli_stage_thresholds`.
- **1 couleur dominante par univers** (`roguelite_palettes.toml` existant). Les couleurs d'éléments
  restent **sacrées**, réservées à la lisibilité gameplay FPS.
- **Ennemis** : les **Oubliés** — pâles, translucides, à moitié effacés. Les abattre est une
  délivrance (lisible, tragique, grand public).

### Audio

Existant : barks d'armes, musique par ambiance (pistes Suno, vertical audio), audio biome. Neuf :
l'Oubli **assourdit** (stade Terni = filtre passe-bas progressif — l'oreille lit le danger comme
l'œil), thème du Hall = le foyer.

---

## 10. Specs techniques (Bevy/Rust)

- **Navigation compagnon** : le bot actuel (`forgia-ai-arena-bot`) avance en ligne droite + LOS —
  insuffisant en terrain d'expédition. Déclencheur du chantier **vleue_navigator 0.15** (navmesh
  polyanya, compatible bevy ^0.18 — veille 2026-08-06, sans migration). Navmesh généré depuis le
  terrain d'expédition ; régénération par chunk streamé à évaluer.
- **Le compagnon = contrat d'interface d'un *slot*** : ses verbes (tenir, activer, porter, se
  poster) définissent l'API des interactions duo. Le futur joueur humain (E9) consomme la même
  interface — et l'arène 5v5 (E10) la consomme **N fois**. À spécifier « occupe un slot », jamais
  « l'allié du joueur » : la nuance est gratuite maintenant, structurelle plus tard.
- **Notion d'équipe / faction sur les entités** : un codebase qui suppose « le joueur contre les
  ennemis » ne se rétrofite pas en 5v5. Un composant faction posé tôt ne coûte presque rien ; posé
  après, c'est une refonte transverse. Porte à ne pas fermer, même si E10 est lointain.
- **Netcode** : hors v1 **en code**, mais l'architecture est **décidée** (2026-08-12) :
  **P2P / listen-server** — une machine joueur héberge, aucun serveur à payer. État réel vérifié le
  2026-08-12 : V2 ne contient **aucune dépendance réseau** (lightyear 0.26.4 n'a jamais quitté V1,
  cf. `.claude/rules/build-stack.md` qui décrit encore le stack V1) — le choix est donc ouvert et
  sans coût de sortie.
  - **La condition qui rend le choix réversible** : la simulation est **autoritative d'un seul
    côté, les clients envoient des inputs**. « Le serveur » devient alors un choix de
    *déploiement*, pas d'architecture : même code en listen-server chez un joueur (E9, gratuit) ou
    en binaire dédié (E10, si la population le justifie). Une autorité **partagée/confiante** entre
    pairs interdit définitivement ce passage.
  - **L'avantage de l'hôte se traite, il ne se subit pas** — trois techniques éprouvées :
    (a) **délai d'input local** sur l'hôte, aligné sur le RTT moyen des autres ; (b) **lag
    compensation** au tir — le serveur rembobine à la vue du tireur, donc chacun touche ce qu'il
    voit ; (c) **même buffer d'interpolation pour tout le monde, hôte compris** — sans quoi l'hôte
    voit l'état du monde avant les autres, un avantage d'*information* que le délai d'input ne
    corrige pas.
  - ⚠ Ces techniques règlent l'**équité**, jamais l'**intégrité** : l'hôte fait autorité, donc reste
    capable de tricher. Sans conséquence en PvE (E9) ; à re-trancher pour le PvP de E10.
  - Transport : **Steam Datagram Relay** (gratuit avec Steamworks) — NAT punch-through et IP des
    joueurs masquées (le P2P brut les expose).
  - Toute sim de gameplay reste en **FixedUpdate déterministe** (fondation existante).
- **GameSet** : les systèmes compagnon en `GameSet::AI`, extraction/objectifs en gameplay standard,
  capteurs en `GameSet::Sensors` (chaîne L7 respectée).
- **Budget frame** : la propagation d'Oubli (front visuel) doit être chunk-locale et dirty-flagged —
  jamais un repaint global par frame (hot path check §3.4 concept-first).

> **Les quatre portes à ne pas fermer** (décidé 2026-08-12, en vue de E9/E10) : le **contrat de
> slot**, la **notion de faction**, la **sim en FixedUpdate déterministe**, et l'**autorité d'un
> seul côté** (jamais d'autorité partagée entre pairs). Aucune ligne de code réseau ni MOBA n'est
> écrite en v1 — seules ces quatre contraintes sont tenues, et elles ne coûtent rien aujourd'hui.

---

## 11. Epics → stories

> Mapping seulement — le détail vit dans `docs/stories/`. Numéros à réserver à la création
> (dernier connu : story-694). Ordre = ordre de validation recommandé.

| Épic | Contenu | Stories | Statut |
| --- | --- | --- | --- |
| **E1 Compagnon** | navmesh (vleue_navigator), suivre, verbes duo, combat d'appoint | à créer | 🔜 premier |
| **E2 Mode Expédition** | terrain→mode jouable, objectifs, paliers, gardiens, extraction | à créer | après E1 |
| **E3 Puissance & gates** | formule, gates univers, scaling en bande, recalibrage `power_gain_per_round` | à créer | |
| **E4 Loot & chasses** | tables qualité, Épargnés + traces, reliques, journal de collection | à créer | |
| **E5 L'Oubli** | stades visuels, foyers, purge, propagation | à créer | |
| **E6 Arbre de talents** | *(ex « Forge & trempe », reprofilé 2026-08-13)* nœuds = techniques jamais chiffrées, remplace l'Enclume, cap par univers, ressources d'arène | socle partiel (`meta_shop.rs` : maîtrise par arme + plafond) | v1 |
| **E7 Hall des Épargnés** | piédestaux, refuge des créatures, trophées (éditeur existant story-665) | à créer | **post-v1** — différé avec E10 le 2026-08-13 ; l'accueil v1 est un menu (§6) |
| **E8 Narratif** | Livre (10 chapitres existants) recâblé sur l'arc l'Oubli, barks contextuels | à créer | |
| **E9 Coop humain** | netcode duo, rattrapage (D3) | à créer | post-validation |
| **E10 Arène 5v5** | MOBA en vue FPS (lanes verticales, jungle, sbires, tours, boutique), 10 slots humains/bots, déblocage niveau joueur | à créer | post-v1 — endgame (§6 Mode 3) |

Existant réutilisé : story-597 (FTUE), story-665 (éditeur Hall), story-690 (ArenaGeometry),
GDD v1 missions 1-2 (combat core).

### Portage V1 → V2 (doctrine, actée 2026-08-09)

L'inventaire du legacy `D:\Forgia` (session 2026-08-09) a identifié ~10 systèmes directement
utiles aux Expéditions, jamais portés : DiscoveryMap (brouillard d'exploration), minimap à
révélation, spawn d'ennemis en monde ouvert par biome, objectifs dynamiques, TriggerZones,
portails+intérieurs seamless, donjon BSP, générateur de château/POI, kit modulaire
pièces/sockets, entrées de grottes de surface. Le V1 contient des bugs connus (confirmé
Antoine) — d'où la règle :

> **Porter = corriger.** On ne répare jamais dans D:\Forgia. Chaque système arrive en V2 par
> une story dédiée qui (a) **mine d'abord les memories V1** (`feedback_*`, `reference_*` via
> `docs/AI_MEMORY_MAP.md`) pour lister les défauts connus du système, (b) réécrit propre aux
> normes V2 (genome, observabilité, tests, 0 hardcode), (c) ne copie-colle jamais. Le V1 est
> une référence d'algorithme, pas une base de code.

Lacunes confirmées côté V1 (à créer en V2, ne pas chercher à porter) : générateur de POI
dispersés (ruines/camps/tours), placement procédural de loot/coffres, quêtes procédurales.

## 12. Success metrics (→ sensors)

> Règle : pas de métrique sans capteur. Les capteurs neufs suivent `observability-required.md`
> (export JSON + health check + seuils en genome).

| Métrique | Capteur | Champ / seuil |
| --- | --- | --- |
| Perf (FPS, stutter) | `forgia2_perf.json` / `forgia2_lag_events.json` (existants) | budget frame tenu en expédition streamée |
| Durée d'expédition, taux extraction vs mort | `forgia2_expedition.json` 🆕 | cible : décision d'extraction réellement disputée (~60/40) |
| Progression de puissance | capteur `power` existant, à étendre 🆕 | vitesse de franchissement des gates ; alerte si un gate exige du re-farm d'entrée (violation P3) |
| Drops par qualité, pity | `forgia2_loot.json` 🆕 | distribution réelle vs genome |
| Compagnon | `forgia2_companion.json` 🆕 | verbes exécutés, temps bloqué/coincé (chien de garde navmesh) |
| Journal de collection | `forgia2_collection.json` 🆕 | % remplissage par univers — la santé du long terme |
| Économie des modes (P2) | `forgia2_economy.json` (existant à étendre) | zéro récompense croisée détectée |

## 13. Hors-scope v1

- **Coop humain temps réel** (E9 — post-validation de la boucle solo+compagnon)
- **Montures** (traversée + trophée — « à terme », système entier)
- **Perks sur épique/légendaire** (la qualité v1 = points seulement)
- **Marketplace pour ultra-rares** (jamais — un trophée acheté ne vaut rien au Hall)
- **PvP en v1** — l'arène 5v5 est **décidée** (§6 Mode 3, E10) mais post-v1 : zéro ligne de code en
  v1, seules les trois portes d'architecture (§10) sont tenues
- Mode spectateur, cross-save
- Récompenses dans l'Abîme au-delà de la trempe (le tableau de rang est un affichage, pas un système)

## 14. Hypothèses, dépendances & décisions ouvertes

**Hypothèses** :
- H1 — `forgia-terrain` est réutilisable pour les Expéditions sans refonte (à valider par E2 ;
  le capteur `rpg_player` note « BiomeMap absent » côté roguelite : le branchement est le chantier).
- H2 — vleue_navigator 0.15 tient ses promesses sur bevy 0.18 (veille OK, prototype E1 = preuve).
- H3 — le mode arène actuel se reprofile en Abîme sans réécriture (stages/vagues/boons conservés).

**Décisions ouvertes** :
- **D1** — Ce qui charge la barre d'ultime (dégâts ? kills ? découverte ?) — chaque réponse tire le
  rythme ; à trancher au prototype E2.
- **D2** — Coût de la mort en expédition : recommandation = matériaux non extraits perdus, stuff et
  XP gardés. À valider manette en main.
- **D3** — Mécanisme de rattrapage duo — à trancher avant E9 seulement.
- **D4** — Noms définitifs FR/EN des univers 7+ et de leurs archétypes.
- **D6** — Les avantages *joueur* de l'arbre de talents : monnaie séparée, ou ressource spécifique à
  l'arme ? Sans réponse, ils écrasent les branches d'arme (cf. §7). À trancher avant d'écrire les nœuds.
- **D5** — Carte plein écran en plus de la minimap permanente (§4 HUD duo) ? Se tranche manette en
  main, jamais depuis un tableau (`map-design-intention.md` §5.3).

---

## Premier pas de validation

**Une Expédition solo + compagnon sur le terrain existant** — teste d'un coup les trois plus grosses
inconnues : la navigation du bot (H2), le branchement terrain (H1), et le fun de l'exploration duo
dans notre moteur (le concept lui-même).

---

*Session design source : 2026-08-09. Rédigé via `/gdd`. Toute valeur chiffrée de balance vit en
genome (couche definition) — ce document nomme les genes, le playtest les règle.*
