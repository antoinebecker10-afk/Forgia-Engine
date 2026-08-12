# Refonte GDD — le chemin de l'état actuel vers *Forgia: The Spared*

> **Ce document est FINI.** Il décrit le trajet entre le jeu d'aujourd'hui et la v1 du
> [GDD](design/gdd-forgia-the-spared.md). Quand la Phase 5 est franchie, il est archivé — il ne
> devient pas une 6ᵉ roadmap permanente.
>
> **Contrat avec [`ROADMAP.md`](ROADMAP.md), à ne jamais enfreindre** :
> ce fichier détient **le chemin** (phases, ordre, dépendances, jalons de falsification) ;
> `ROADMAP.md` détient **le maintenant** (Now/Next/Later). Le NOW de la roadmap est toujours un
> **sous-ensemble de la phase courante**, et il **ne recopie rien** d'ici — il pointe.
> *Une grandeur écrite deux fois finit toujours par divorcer : c'est la classe de défaut n°1 du projet.*
>
> Créé : **2026-08-12**, pendant la purge des stories. Les stories des phases se créent au fur et à
> mesure (§7) — aucune n'est présumée exister à l'écriture de ce document.

---

## 1. D'où on part (vérifié le 2026-08-12, pas supposé)

| Fait | État |
| --- | --- |
| Cœur combat FPS (mouvement, tir, 4 armes, boons, défense tri-couche) | ✅ construit et jouable |
| Réactions élémentaires (`ReactionTable`) | ⚠️ **le code existe, les réactions ne partent jamais** (story-697) |
| `forgia-terrain` (BiomeMap Voronoi, streaming, SDF) | ✅ existe — ❌ **aucune référence à `BiomeMap` dans `forgia-mode-roguelite`** : non branché |
| Prototype forêt (90 % de couverture, 240 FPS) | ✅ **commité** (`vegetation_density.toml` suivi) |
| `forgia-ai-arena-bot` | ⚠️ ligne droite + LOS, **pas de navmesh**, pas de chien de garde de désenlisement |
| `ArenaGeometry` (`forgia-stage`) | ✅ existe — source du futur navmesh d'arène |
| Netcode | ❌ **aucune dépendance réseau en V2** |
| Hub / Hall, meta shop, Éclats, Livre, cosmétiques, FTUE MVP | ✅ construits |

---

## 2. La nature du travail — et c'est la bonne nouvelle

La refonte **n'est pas** une réécriture. Trois natures de travail, de la moins chère à la plus chère :

### 🔄 CONVERSION — ça existe, ça change de *sens*

| Aujourd'hui | Devient | Ce que ça coûte |
| --- | --- | --- |
| L'arène roguelite (stages, vagues, boons) | **L'Abîme** — descente infinie | Reprofilage + **retirer** toute récompense sauf l'XP d'arme (P2) |
| Meta shop / L'Enclume des Âmes | **La Forge** | Ajouter la trempe et les matériaux |
| Hub menu / castle | **Le Hall des Épargnés** | Piédestaux + refuge des créatures |
| Livre (10 chapitres) | Arc de **l'Oubli** | Recâblage narratif |
| Score de RANG | Tableau de **profondeur** | Affichage, zéro système |

### 🔌 BRANCHEMENT — ça existe, ce n'est pas câblé

- `forgia-terrain` → **les Expéditions** (le chantier central : `BiomeMap` absent côté mode)
- Prototype forêt → **le premier univers**
- `roguelite_ambiances.toml` (6 ambiances) → **les 6 premiers univers**
- `ReactionTable` → **inter-acteurs** (dès que 697 est réparée)
- Portage V1 (**porter = corriger**, GDD §11) : DiscoveryMap, minimap à révélation, spawn par biome,
  objectifs dynamiques, TriggerZones, donjon BSP, générateur de POI

### 🆕 CONSTRUCTION — vraiment neuf

Compagnon + navmesh · extraction au choix · l'Oubli (4 stades, propagation) · les 3 chasses
(Épargnés, traces, reliques, journal) · la trempe · la puissance & ses gates.

> **Lecture** : l'essentiel de la structure du GDD est de la **conversion** et du **branchement**.
> Le neuf se concentre sur le compagnon, l'Oubli et les chasses. La refonte est plus courte
> qu'elle n'en a l'air — à condition de ne pas reconstruire ce qui existe.

---

## 3. Les phases

> Chaque phase se termine par un **jalon falsifiable**. Tant qu'il n'est pas franchi, on n'ouvre pas
> la suivante. Un jalon qui échoue **change le plan** — il ne se contourne pas.

### Le rythme — deux natures d'incrément, et une seule bloque

> **Adopté le 2026-08-12, après s'être fait avoir le jour même.** Quatre incréments livrés,
> testés, poussés — tests headless verts, clippy propre, gates au vert — sans qu'**aucun**
> ne tourne une seule fois en jeu. Personne ne savait si un bot suivait réellement un
> chemin. Le défaut ne levait aucune erreur : c'est exactement un capteur à zéro qui
> rapporte « ok » ([story-699](stories/story-699-un-capteur-a-zero-ne-doit-pas-dire-ok.md)),
> appliqué au processus au lieu du code.

| Nature | Définition | Règle |
| --- | --- | --- |
| 🧱 **FONDATION** | Rien ne le consomme encore — une crate neuve, un type partagé, un capteur sans producteur | **Peut s'empiler.** Le jeu ne peut pas en montrer l'effet, exiger une run serait du théâtre |
| 👁️ **OBSERVABLE** | Il change ce qui se passe à l'écran ou dans un capteur | **Bloque le suivant** tant qu'il n'a pas tourné |

**Chaque incrément déclare sa nature dans sa story.** Ce n'est pas un détail de forme :
c'est ce qui rend la règle applicable au lieu d'être une bonne intention.

*Vérification rétroactive du 2026-08-12* — inc.1 (crate navmesh), inc.2 (aucun consommateur)
et inc.4a (`Faction`) étaient bien des **fondations** : les empiler était légitime.
**L'inc.3 était OBSERVABLE** — il change le déplacement des bots — et il aurait dû bloquer.
La règle aurait attrapé exactement la faute commise.

### Le palier de validation

**À chaque fin de phase, et après tout incrément OBSERVABLE** : une run, quatre commandes.

```bash
python tools/ai/validation_debt.py     # 0. combien de travail n'a jamais tourne ?
cargo build -p forgia --profile release-fast   # jamais -p forgia-game : exe perime EN SILENCE
#   … jouer la situation que l'increment est cense changer …
python tools/ai/forgia_digest.py all   # 97 capteurs + log -> ~2,5 Ko
python tools/ai/phase0_check.py        # un verdict par story ouverte
```

`validation_debt.py` est le garde : il compte les crates plus récentes que le binaire, les
capteurs antérieurs au code, et les stories livrées non validées. **Il rend la dette
refusable au lieu de la laisser invisible.** Son seuil de stories en REVIEW est **3** —
au-delà, on ne sait plus laquelle a cassé quoi.

### Ce que le palier n'est pas

Ce n'est **pas** un test de non-régression, ni un remplacement des tests headless — ceux-là
restent le filet mécanique. C'est la seule chose qu'aucun test ne peut faire : **confronter
le code à la réalité**. Un bot peut suivre un chemin géométriquement parfait et avoir l'air
absurde à l'écran ; aucun `assert` ne le dira jamais.

### Phase 0 — Réparer le socle (prérequis, pas de la refonte)

État **mesuré** le 2026-08-12 par `tools/ai/phase0_check.py` (un verdict par story ouverte, tiré des
capteurs — pas du ressenti) :

| Story | Verdict | Ce qu'il reste |
| --- | --- | --- |
| **696** hitstop | ✅ **CLOSE** | Rien. Le hitstop est **retiré définitivement** (décision Antoine). Il avait été supprimé en mai par ADR-0002 — seuls les commentaires prétendaient encore qu'il existait |
| **697** réactions | ✅ **PASS** | Rien à corriger : surcharges **16**, miasmas **4**, bursts VFX **16**. Le moteur est correct — voir l'encadré |
| **698** kill | ❌ **ÉCHEC partiel** | Burst **70/77 (90 %)** ✅ · son **8/77 (10 %)** ❌ — **un seul canal à corriger, le son de mort** |
| **699** capteurs menteurs | 🚧 IN_PROGRESS | Helper livré, **1 capteur sur 3 converti**. C'est ce défaut qui a rendu tout le reste invisible |

**Reste réellement en Phase 0** : le **son de mort** (698) et la **conversion des capteurs** (699).
Puis l'assemblage du « kill satisfaisant » — désormais en 3 temps, le hitstop n'existant plus.

> ### 🔑 Ce que 697 prouve — et c'est le meilleur argument du GDD
>
> Une arme porte **un** élément. Une réaction exige **deux** éléments sur la même cible en 3-4 s.
> Or le TTK d'un grunt est de **0,18 s**, et le log montre **3 changements d'arme en 400 s**. En
> solo, la fenêtre n'existe donc que sur un **élite** (0,71 s) ou un **boss**.
>
> Le GDD §4 avait déjà tranché : *« Chaque acteur porte un élément à la fois […] en solo, le
> compagnon est le second élément. »* Le diagnostic **ne bloque pas la refonte — il la valide
> empiriquement**. La « décision de design » qu'attend story-697 s'appelle **E1**.
>
> ➡️ 697 devient donc un **critère d'acceptation de la Phase 1**, pas un prérequis de la Phase 0.

**🚩 Jalon** : `phase0_check.py` rend **0 ÉCHEC** sur un binaire **fraîchement rebuildé**
(l'outil refuse de conclure sur un exe périmé — respecter son avertissement).

### Phase 1 — E1 · Le compagnon (la fondation)

> ✅ **H2 est PROUVÉE — 2026-08-12, [story-700](stories/story-700-navmesh-fondation-compagnon.md).**
> `vleue_navigator 0.15.0` + `polyanya 0.16.1` résolvent **et compilent** contre un seul
> `bevy_ecs 0.18.1` (58,7 s), 11 tests headless verts, clippy 0 — **sans migrer vers bevy 0.19**.
> Le plus gros risque de la refonte est retiré. Reste à câbler des consommateurs.

- **Navmesh `vleue_navigator` 0.15 depuis `ArenaGeometry`** — dans l'**arène d'abord** : bornée,
  existante, déjà instrumentée. On ne débogue pas le navmesh et le streaming en même temps.
  ✅ inc.1 livré (crate `forgia-navmesh` + génome) · ⬜ inc.2 branchement `ArenaGeometry`
- Suivre · se poster · chien de garde de désenlisement
- Barre PV compagnon permanente (GDD §4)
- Capteur `forgia2_companion.json` : verbes exécutés, **temps bloqué**

**🚩 Jalon — H2 prouvée** : le compagnon traverse une arène complète sans se coincer, temps bloqué ≈ 0
sur N graines. *Si le navmesh ne tient pas sur bevy 0.18, tout le duo est à repenser — d'où sa place ici.*

**🚩 Jalon hérité de la Phase 0 — story-697** : le compagnon porte le **second élément**, et les
réactions partent en **combat ordinaire**, plus seulement sur élite ou boss. C'est la mesure qui
transforme l'USP n°1 du GDD d'intention en mécanique jouée.

### Phase 2 — E2 · L'Expédition minimale (LE go/no-go)

C'est le **premier pas de validation** défini par le GDD. Volontairement pauvre : on teste le concept,
pas le contenu.

- Brancher `forgia-terrain` en mode jouable (**H1**)
- **Un seul** type d'objectif (« atteindre »), **un seul** point d'extraction
- Le compagnon sur terrain **streamé** — navmesh régénéré par chunk : le vrai test
- Minimap portée du V1, en surface d'ordre

**🚩 Jalon — H1 prouvée + le concept jugé fun manette en main.**
*C'est le jalon le plus important du document. Si explorer à deux n'est pas fun ici, ce n'est pas le
contenu qui manque — c'est le GDD qui change.*

### Phase 3 — Le cycle économique (ce qui fait tenir les deux modes ensemble)

Sans cette phase, on a deux jeux côte à côte au lieu d'une boucle.

- **E6 trempe** : niveaux d'arme, XP par profondeur, cap par univers
- **L'Abîme reprofilé** : descente infinie, récompense **unique** = XP d'arme
- Matériaux d'expédition consommés à la Forge

**🚩 Jalon — P2 falsifiable** : zéro pièce de stuff lâchée dans l'Abîme, zéro XP d'arme au-delà du cap.

### Phase 4 — La progression verticale

- **E3 puissance** : joueur + armes + stuff · gates d'univers · scaling en bande ·
  recalibrage de `power_gain_per_round` (son capteur est **en alerte** — la refonte l'absorbe,
  on ne le patche pas isolément)
- **E4 loot (socle)** : gardien de palier = pièce **garantie**, qualité en bonus **borné**

**🚩 Jalon — P3 falsifiable** : un légendaire du palier N vaut moins qu'un commun du palier N+2.
Et aucun gate ne se franchit en refarmant l'entrée d'un univers.

### Phase 5 — L'identité (ce qui fait que c'est *ce* jeu)

- **E5 l'Oubli** : 4 stades lisibles, foyers, purge, propagation chunk-locale et dirty-flaggée.
  La remontée de couleur le long des veines est **LA** récompense visuelle du jeu.
- **E4 chasses** : Épargnés, traces, reliques, journal de collection
- **E7 Hall des Épargnés** : piédestaux, refuge des créatures
- **E8 narratif** : Livre recâblé, barks contextuels

**🚩 Jalon — P4 / P5 falsifiables** : grep des tables de reliques = zéro champ de stat ; aucun contenu
gaté par un drop RNG.

**➡️ Fin de la refonte. Ce document est archivé ici.**

### Phase 6 — Post-v1 (hors refonte, pour mémoire)

**E9** coop humain (listen-server, architecture tranchée au GDD §10) · **E10** arène 5v5.
Ne s'ouvre qu'après validation de la boucle solo+compagnon.

---

## 4. Qui prouve quoi

| Hypothèse / Pilier | Prouvé en | Si ça casse |
| --- | --- | --- |
| **H2** — vleue_navigator tient sur bevy 0.18 | ✅ **PROUVÉE** 2026-08-12 (story-700) | — |
| **H1** — `forgia-terrain` réutilisable sans refonte | Phase 2 | Les Expéditions coûtent 3× plus cher |
| **Le concept** — explorer à deux est fun | Phase 2 | **Le GDD change**, pas le contenu |
| **H3** — l'arène se reprofile en Abîme sans réécriture | Phase 3 | L'Abîme devient de la construction |
| **P2** — deux modes, une économie chacun | Phase 3 | Les modes se cannibalisent |
| **P3** — la profondeur d'abord | Phase 4 | La courbe redevient plate (défaut déjà connu) |
| **P1** — duo d'abord (chaque mécanique a son verbe bot) | Continu | Une mécanique duo sans verbe = dette |
| **P4 / P5** — beau sans stats, rien d'indispensable dans le RNG | Phase 5 | Le loot devient pay-to-win par le hasard |

---

## 5. Ce qui ne bouge pas (protection de scope)

Le GDD est explicite : *les GDD antérieurs gagnent au niveau feel/combat*. Donc **on ne touche pas** :

le feel FPS (mouvement, tir, `fps_tuning.toml`) · les 4 armes et leurs identités · la défense
tri-couche · les boons (ils restent le build de run de l'Abîme) · toute l'infra (capteurs, genomes,
`GameSet`, gates xtask, hooks).

Toute proposition de « pendant qu'on y est » sur ces éléments est **hors scope** par défaut.

---

## 6. Les risques nommés

| Risque | Pourquoi il est réel | Parade |
| --- | --- | --- |
| **Reconstruire ce qui existe** | 143 stories ouvertes viennent d'être purgées ; le réflexe « je réécris propre » coûte des mois | Le tableau §2 : conversion et branchement d'abord, construction en dernier |
| **Le navmesh ne tient pas** | H2 n'est qu'une veille, pas un prototype | Phase 1 dans l'arène bornée, avant le terrain streamé |
| **Afficher l'IA expose l'IA** | Un compagnon coincé se voit toutes les 3 s sur la minimap | Chien de garde de désenlisement livré **avec** le compagnon, pas après |
| **Deux modes qui ne se parlent pas** | C'est le défaut par défaut d'une structure à deux boucles | Phase 3 **avant** tout contenu — l'économie croisée est la couture |
| **La courbe reste plate** | Défaut déjà mesuré (`power_gain_per_round` en alerte, boons plats) | P3 falsifiable en Phase 4, capteur `power` étendu |
| **Le scope enfle par le contenu** | 6 univers annoncés, 10 systèmes V1 à porter | Phase 2 délibérément pauvre : 1 objectif, 1 extraction |
| **Un système inerte ne lève aucune erreur** | 29 capteurs disaient « ok » avec **tous** leurs compteurs à zéro — c'est ce qui a masqué les réactions pendant des semaines | story-699 + `tools/ai/sensor_honesty.py`. Tout nouveau capteur de feature doit distinguer « ok » de « rien mesuré » |
| **Trois vocabulaires pour les mêmes 4 armes** | `WeaponType::Shotgun` **est le sniper**, la clé capteur `pompe` **est le lance-roquettes** — un raisonnement par le nom se trompe de cible | Toujours vérifier `roguelite_elements.toml` et `viewmodel_arena.toml` avant de conclure sur une arme. Table complète : sortie de `phase0_check.py` |

---

## 7. Comment les stories se créent

- **Une story par incrément livrable**, jamais par épic — un épic est un dossier, pas une unité de travail.
- **Prendre l'ID via `cargo run -p xtask -- story-ids`** avant d'écrire : deux collisions déjà subies.
- **DONE mécanique** : `cargo run -p xtask -- story-gate --story <id>`. Jamais de DONE auto-déclaré.
- **Limite WIP 3** (`ROADMAP.md`) — elle vient d'être reconquise par la purge, ne pas la reperdre.
- Chaque nouveau système naît avec son capteur (`observability-required.md`) : le GDD §12 liste déjà
  ceux qui manquent (`forgia2_expedition`, `forgia2_loot`, `forgia2_companion`, `forgia2_collection`).

---

*Le GDD dit **où on va**. Ce fichier dit **par quel chemin, dans quel ordre, et à quoi on saura qu'on
y est**. `ROADMAP.md` dit **ce qu'on fait cette semaine**. Les stories disent **le détail**.*
