# story-703 — L'arène en trois parts : le chronomètre devient l'épreuve

**Statut** : DRAFT — design arrêté, rien d'implémenté
**Créée** : 2026-08-13
**Niveau BMAD** : Enterprise (géométrie + IA + vagues + capteur, ≥ 4 crates)
**Origine** : session de design du 2026-08-13 avec Antoine, après la run où **6 mobs sur 7
étaient aveugles** ([commit `0cd2899`](../../)) — c'est en regardant des ennemis plantés
sur place que la question « et s'ils tenaient un poste ? » est venue.
**Rend diégétique** : `universe_power_gate_<n>` du [GDD](../design/gdd-forgia-the-spared.md) §7.

---

## 1. La mécanique

L'arène est divisée en **trois parts égales, physiquement cloisonnées**, chacune fermée par
une **porte**. Elles remplacent les trois vagues.

1. Les mobs entrent par leur porte et **marchent jusqu'à leur poste** dans leur part.
2. Ils y tiennent. Ils n'attaquent que si le joueur **s'approche trop**.
3. **Toutes les X secondes, une porte s'ouvre** — chronomètre fixe, quoi qu'il arrive.
4. Il faut nettoyer un pack avant l'ouverture suivante, sinon on cumule.

Le joueur choisit donc **quand** et **combien** il engage. C'est le *pull* de donjon, pas
une vague qui déferle.

## 2. Ce que le chronomètre est vraiment : un DPS check

C'est le cœur de la story, et ce n'est pas un réglage de difficulté parmi d'autres.

Le GDD porte déjà une porte de puissance chiffrée par univers
(`universe_power_gate_<n>`). Un seuil qu'on **lit**. Le chronomètre en est la version
qu'on **sent** : le pack 2 arrive avant que le pack 1 soit tombé, et le verdict est rendu
sans qu'aucun chiffre ne s'affiche.

Et il mesure **les trois économies du GDD à la fois** — celui-ci exige qu'aucune ne soit
skippable (§7) :

| L'épreuve échoue → le joueur va… | Ce que ça monte | Épic |
| --- | --- | --- |
| Farmer les ressources lâchées par les morts | Le **niveau d'arme** (trempe, Forge) | E6 |
| Partir en **Expédition** | Le **stuff** et le niveau joueur | E2 · E3 |
| Rejouer | Son **skill** | — |

Un seul instrument, trois axes. C'est plus honnête que trois jauges séparées, et ça donne
à l'échec une **direction** au lieu d'un mur.

## 3. L'aggro angulaire — et la poche qui rétrécit

Rayon d'aggro d'un pack = **sa part + un quart de chaque part voisine**, soit **180°** sur
des parts de 120°.

Conséquence, vérifiée :

| Portes ouvertes | Zone sûre restante |
| --- | --- |
| 1 | Une part entière — **120°** |
| 2 | Le cœur de la dernière part — **60°** |
| 3 | **Aucune** |

**L'espace sûr est divisé par deux à chaque porte, puis disparaît.** C'est une escalade que
le joueur voit et sent, sans une ligne d'interface. Un joueur assez rapide n'en a jamais
besoin : c'est celui qui traîne qu'on accule, progressivement et lisiblement.

> ⚠️ **Notion ANGULAIRE, pas un rayon.** L'aggro est aujourd'hui une distance
> (`detect_range`, 50 m — plus large qu'une part, donc entrer dans un secteur réveillerait
> tout). Il faut la coder telle qu'elle est pensée : part d'origine du mob, angle du
> joueur, débordement en fraction de part. Un `atan2` par mob à basse fréquence.
>
> ```toml
> aggro_sector_spill_frac = 0.25   # débordement sur les parts voisines
> ```
>
> Ce gène devient un **curseur de difficulté à part entière** : plus large, plus dur.

### Pourquoi la poche n'est pas un exploit

Une zone sûre serait cassée si on pouvait *engager → reculer → réinitialiser → recommencer*.

Elle ne l'est pas, grâce à ce qui a été livré la veille
([story-700](story-700-navmesh-fondation-compagnon.md) inc.3c) : un mob accroché **suit**,
6 s de persistance et 50 m de laisse. **La poche protège des packs non réveillés, jamais du
combat qu'on a engagé.** Reculer ne remet rien à zéro — ça gagne du temps sur les autres, et
en perd sur le chronomètre. C'est exactement le bon arbitrage.

## 4. Les cloisons doivent être PHYSIQUES

C'est la condition sans laquelle tout s'effondre, et elle règle trois autres problèmes.

`sightline_max_m` vaut **69 m** — toute l'arène. Si les parts ne sont que des zones
logiques, un sniper de la part 1 tire dans la part 3, et « éviter en restant à l'écart » ne
veut plus rien dire.

De vrais murs apportent **quatre choses d'un coup** :

1. La **coupure de ligne de vue** qui rend l'évitement réel
2. Le **couvert qui manque** — capteur `stage_layout` en **erreur** : 13 abris pour 151
   attendus, facteur **11,6**. Cette story le résout au lieu de le contourner.
3. Des **portes**, donc des passages étroits — que le navmesh sait franchir depuis
   [story-700 inc.3b](story-700-navmesh-fondation-compagnon.md)
4. Une **lisibilité immédiate** : on voit d'où viendra le prochain pack

## 5. « Se poster » — un comportement neuf, et partagé

Aujourd'hui un mob qui ne voit pas le joueur est `Idle` : il ne bouge **pas du tout**. Cette
story demande autre chose — il entre, **marche jusqu'à son poste**, et tient.

> **Le même verbe sert le compagnon.** Le GDD §10 liste ses quatre verbes : tenir, activer,
> porter, **se poster**. Ce qu'on écrit pour les mobs sert E1, et inversement. À concevoir
> comme une primitive partagée, pas comme deux implémentations.

## 6. Les packs : moins nombreux, plus lourds

Cette story suppose la refonte de composition discutée le même jour, et les deux ne se
livrent pas séparément — un pack de 8 mobs à 30 PV meurt en **1,4 s** de tir soutenu, il
n'y a aucune place pour une stratégie dedans.

**Et la durée de vie se dérive, elle ne se choisit pas** : la fenêtre de réaction
élémentaire est de 3-4 s (brûlure 3 s, choc 4 s). Pour qu'un mob soit *réactible*, il doit
tenir **2 à 4 s** — soit **350 à 650 PV** à 168 dps, contre 30 aujourd'hui.

> C'est aussi ce qui débloque l'**USP n°1** du GDD. [story-697](story-697-reactions-elementaires-jamais-declenchees.md)
> a établi que les réactions ne partent jamais parce qu'un grunt meurt en **0,18 s**.
> « Moins de mobs, plus de PV » n'est pas une préférence de goût : c'est la **précondition
> mécanique** de « je pose, tu détones ».

## 7. Le capteur — sans quoi le chronomètre se règle à l'aveugle

`forgia2_arena_packs.json` 🆕, par pack : **temps de nettoyage**, temps restant au
chronomètre à l'ouverture suivante, nombre de packs simultanément actifs, et si le joueur a
utilisé la poche sûre.

C'est ce qui dira, **chiffres à l'appui**, si le chronomètre est juste. Sans lui on le
devine run après run.

Ordre de grandeur de départ, à jouer et non à calculer : 4 mobs × ~400 PV à 168 dps = ~10 s
de tir pur, soit **20-25 s réels** avec déplacements et rechargements. Un chronomètre à
**30 s est clément, 20 s tendu**.

## 8. Critères d'acceptation

- [ ] Trois parts **physiquement cloisonnées**, portes incluses
- [ ] `stage_layout` **n'est plus en erreur** sur le déficit de couvert
- [ ] Aucune ligne de vue entre deux parts non adjacentes, **mesuré**
- [ ] Aggro **angulaire** (part + fraction de voisine), gène `aggro_sector_spill_frac`
- [ ] Comportement « se poster » — entrer, rejoindre, tenir
- [ ] Chronomètre fixe, en gène, hot-reloadable
- [ ] La poche sûre rétrécit bien 120° → 60° → 0, **vérifié en jeu**
- [ ] Un mob accroché **suit hors de sa part** (pas de reset par recul)
- [ ] `forgia2_arena_packs.json` publie le temps de nettoyage par pack
- [ ] PV de mob dans la bande **350-650**, dérivés de la fenêtre de réaction
- [ ] Une réaction élémentaire part **en combat ordinaire**, plus seulement sur boss

## 8 bis. Deux prérequis hérités du chantier navmesh (2026-08-13)

Ils ne sont pas facultatifs : cette story **multiplie** les passages étroits, donc
elle amplifie les deux défauts restants au lieu de les contourner.

### P1 — Les couloirs du maillage sont plus étroits que la moitié des ennemis

Le maillage dilate chaque obstacle du rayon d'agent (**0,30 m**), il garantit donc
des couloirs de **0,60 m**. Or :

| archétype | rayon | couloir requis | verdict |
|---|---|---|---|
| sniper | 0,30 m | 0,60 m | ✅ |
| runner | 0,32 m | 0,64 m | limite |
| **tank** | 0,55 m | **1,10 m** | **×1,8** |
| **boss** | 1,40 m | **2,80 m** | **×4,7** |

Symptôme correspondant, rapporté en jeu : *« les dps ça a l'air bien mais les
autres ça marche moins bien »*.

**Décision à prendre** : un maillage par classe de gabarit (la solution standard,
coût de bâti négligeable — 0,3 ms), ou sortir le boss du pathfinding. Tant qu'elle
n'est pas prise, **toute porte dessinée pour cette story doit faire ≥ 2,80 m de
passage utile** si un boss doit la franchir.

Garde en place : test `le_maillage_ne_promet_pas_des_couloirs_ou_le_bot_ne_passe_pas`.

### P2 — La hauteur déclarée d'un prop ne vaut pas son collider

Mesuré : un bot bloqué par une **paroi de 0,60 m** alors que le maillage n'avait
retenu que **13 obstacles sur 166 disques soumis** — donc les 153 autres se
déclaraient `h ≤ 0,45 m`. Le maillage trace droit à travers un solide que la
physique arrête.

Même classe de défaut que le rayon d'emprise, sur l'autre axe. **Non tranché.**
Il devient bloquant ici : les cloisons de §4 sont précisément des solides dont la
hauteur décide s'ils coupent la ligne de vue *et* s'ils bloquent le pas.

## 9. Décisions ouvertes

- **D1** — La valeur du chronomètre. Se tranche manette en main, avec le capteur.
- **D2** — Les compétences propres par archétype. *Le vrai morceau de design* : aujourd'hui
  runner / tank / sniper diffèrent par PV, vitesse et portée — c'est une différence de
  **statistique**, pas de **menace**. Pour qu'un ordre de kill existe, ignorer un runner
  doit coûter autre chose qu'ignorer un sniper.
- **D3** — Les élites esquivables (référence donjon WoW) : dans une part, ou entre les parts ?
- **D4** — Le chronomètre se remet-il à zéro entre deux salles, ou se durcit-il avec la
  profondeur ?

## 10. Ce que cette story ne fait PAS

- **Toucher les Expéditions** — c'est l'Abîme et l'arène, mode 2
- **Le multi-salles** — RunGraph et portails existent déjà (story-646)
- **Trancher D2**, qui mérite sa propre passe de design

## Cross-refs

- [GDD §7](../design/gdd-forgia-the-spared.md) — les gates de puissance que ce chronomètre rend diégétiques
- [story-700](story-700-navmesh-fondation-compagnon.md) — navmesh, portes franchissables, persistance de poursuite
- [story-697](story-697-reactions-elementaires-jamais-declenchees.md) — pourquoi la durée de vie des mobs est bloquante
- `map-design-intention.md` §1 — les grandeurs se **dérivent**, elles ne se choisissent pas
