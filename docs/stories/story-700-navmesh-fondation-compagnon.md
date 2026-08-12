# story-700 — E1 inc.1 : le navmesh existe, compile et répond

**Statut** : IN_PROGRESS — ✅ **incréments 1 à 3 VALIDÉS EN JEU le 2026-08-13 00:03.**
Reste l'incrément 4 (le compagnon, sous-découpé 4a-4d ; **4a livré**).
**Créée** : 2026-08-12
**Niveau BMAD** : Standard (crate neuve + génome + câblage workspace)
**Origine** : [REFONTE_GDD.md](../REFONTE_GDD.md) Phase 1 — épic **E1 Compagnon**.
**Prouve** : hypothèse **H2** du [GDD](../design/gdd-forgia-the-spared.md) §14.
**Débloque** : le compagnon (E1), le remède structurel aux mobs coincés
([spawn-clearance.md](../../.claude/rules/spawn-clearance.md) §5), et à terme les sbires de E10.

---

## Le problème

`forgia-ai-arena-bot` avance en ligne droite vers sa cible et pousse dans les colliders. Un mob
né derrière un pilier ne se débloque **jamais** : ce n'est pas un ralentissement, c'est un ennemi
retiré du combat, et une vague qui ne se nettoie pas si le joueur ne va pas le chercher.

Le compagnon de E1 aggrave le défaut au lieu de le subir : un mob coincé à 40 m passe inaperçu,
un compagnon dont le point ne bouge plus sur la minimap se voit **toutes les trois secondes**.

## H2 — prouvée, à deux niveaux

Le GDD listait H2 comme *« veille OK, prototype E1 = preuve »*. La preuve est faite :

| Niveau | Résultat |
| --- | --- |
| **Résolution** | `vleue_navigator 0.15.0` + `polyanya 0.16.1` contre **un seul** `bevy_ecs 0.18.1` — aucun doublon de version, 564 paquets |
| **Compilation** | `cargo check -p forgia-navmesh` — 58,7 s, propre |
| **Exécution** | 11 tests headless verts, dont deux qui interrogent un vrai maillage |

**Sans migrer vers bevy 0.19** — le report de migration reste intact (revérifié : `bevy_rapier3d`
sans release depuis 0.35.0, `rapier3d` en 0.35.0-**beta**, donc toujours pré-release).

## Ce qui est livré

- **`crates/forgia-navmesh/`** — convertit des solides (`forgia_core::layout`) en maillage polyanya
  et répond « chemin de A à B ». Pas de système Bevy, pas de composant de suivi.
- **`assets/genomes/navmesh.toml`** — rayon d'agent, ressaut franchissable, qualité d'approximation.
- **Câblage workspace** — member + `[workspace.dependencies]`, `default-features = false`
  (une lib n'impose pas `bevy_gizmos`).

### Trois décisions de conception

1. **Le seuil de navigation n'est PAS celui du couvert.** Réutiliser `SolidDisc::breaks_sight()`
   (1,80 m) était le réflexe — c'était sous la main, et c'est faux. Un muret d'un mètre ne masque
   personne et arrête pourtant un agent **qui ne saute pas**
   ([map-design-intention.md](../../.claude/rules/map-design-intention.md) §2.5). La moitié des
   obstacles serait devenue invisible au maillage. Prédicat retenu : `h > step_height_m` (0,45 m,
   `MaxStepHeight` d'Unreal). Un test dédié interdit la confusion des deux seuils.
2. **On sur-approxime toujours.** Les disques deviennent des polygones **circonscrits**, jamais
   inscrits : un polygone inscrit laisserait des chemins mordre dans l'obstacle. Un agent qui
   contourne large coûte un détour ; un agent qui frotte coûte un blocage.
3. **L'API prend un bord quelconque, pas un `ArenaGeometry`.** L'Expédition (Phase 2) n'a pas
   d'arène hexagonale et doit réutiliser la fonction telle quelle.

Plus : `BuildReport::is_blind()` — **zéro obstacle mesuré n'est pas un succès**
([map-design-patterns.md](../../.claude/rules/map-design-patterns.md) §13). Le rapport le dit au
lieu de laisser croire à une arène dégagée.

## Critères d'acceptation

- [x] `vleue_navigator` résout et compile contre bevy 0.18.1, sans doublon dans le lock
- [x] Crate `forgia-navmesh` créée, membre du workspace, `cargo check` propre
- [x] Zéro littéral numérique dans le code — tout vient de `assets/genomes/navmesh.toml`
- [x] Lecture du génome via `forgia_core::def_io` (jamais `std::fs` — échec silencieux sur wasm)
- [x] Un test compare le TOML au repli Rust : s'ils divergent, il casse
- [x] Le seuil de navigation est testé distinct du seuil de couvert
- [x] Un test prouve que la dilatation **ferme** un passage plus étroit que l'agent
- [x] `cargo test -p forgia-navmesh` — **11 passés, 0 échec**
- [x] `cargo clippy -p forgia-navmesh --all-targets` — **0 warning** (vrai cargo, pas RTK)
- [ ] **Validation en jeu** — aucun consommateur n'est encore câblé (cf. inc. suivants)

## Ce que cette story ne fait PAS

Volontairement hors scope, chacun étant un incrément à part :

- **Personne ne consomme le maillage.** `forgia-ai-arena-bot` avance toujours en ligne droite.
- **Le désenlisement** d'un agent déjà coincé — chien de garde séparé, à livrer **avec** le
  compagnon et non après, puisque la carte le rendra visible.
- **La régénération par chunk** d'un terrain streamé — Phase 2.
- **L'évitement dynamique** entre agents — le maillage est statique.

## Suite

| Inc. | Contenu |
| --- | --- |
| ~~2~~ | ✅ **LIVRÉ 2026-08-12** — cf. section ci-dessous |
| ~~3~~ | ✅ **LIVRÉ 2026-08-12** — cf. section ci-dessous |
| **4** | Le compagnon — **découpé en 4 sous-incréments**, cf. ci-dessous. **4a livré.** |
| **5** | Le compagnon porte le **second élément** → jalon hérité de [story-697](story-697-reactions-elementaires-jamais-declenchees.md) : les réactions partent en combat ordinaire, plus seulement sur boss |

---

## Incrément 2 — le maillage se bâtit tout seul depuis l'arène (2026-08-12)

`forgia-stage` appelle désormais `forgia-navmesh`. **Le sens de la dépendance est
délibéré** : la crate de navigation ne connaît ni les arènes ni leur cycle de vie, ce qui
lui permettra de servir le terrain d'expédition (Phase 2) sans être réécrite.

### Les deux gardes, et pourquoi aucune n'est optionnelle

1. **Debounce d'une frame.** Les solides sont déposés par une demi-douzaine de producteurs
   (murs de pièces, modules, pièces autorées, décor du mode roguelite). Rebâtir à chaque
   dépôt triangulerait N fois la même arène pour un seul résultat utile. On attend donc la
   première frame **stable** après la dernière modification.
2. **`authored_pending == 0`.** Les pièces autorées arrivent avec leur GLB, en asynchrone.
   Un maillage bâti pendant leur chargement ignorerait leurs emprises et laisserait des
   agents traverser des murs bien réels — **et le défaut serait invisible, puisque la
   construction réussit**. `ArenaGeometry` documente exactement ce piège pour son propre
   capteur ; il vaut ici à l'identique.

Troisième garde, moins évidente : une arène dont le rayon jouable ne dépasse pas l'emprise
de l'agent (le cas au `reset()`, entre deux stages) fait **oublier** le maillage au lieu
d'en bâtir un vide. Router sur une géométrie démontée enverrait les agents à travers des
murs qui n'existent plus.

### Le capteur `forgia2_navmesh.json`

Il expose la **provenance** (`source` + `seed`), pas seulement le résultat : un maillage
qui survit à un changement de stage est un bug silencieux, et c'est ce champ qui le rend
visible. Plus `discs_seen` / `segs_seen` / `obstacles_kept` / `blind` / `build_ms`.

**Un maillage bâti n'est jamais `ok` par défaut.** Deux façons de « réussir » à vide, et
chacune a son alerte distincte :

| Cas | Severity | Ce que le `next_step` pointe |
| --- | --- | --- |
| Aucune zone bâtie | `info` | Normal hors arène |
| Bâti sur **zéro solide soumis** | `warn` | La géométrie était vide/incomplète (`authored_pending`) |
| Solides soumis, **zéro retenu** | `warn` | Le gène `agent.step_height_m` rejette tout |

### Vérifications

- `cargo test -p forgia-navmesh -p forgia-stage` — **verts** (exit 0)
- `cargo clippy --all-targets` sur les deux crates — **0 warning** (vrai cargo, pas RTK)
- `xtask sensor-audit` — **OK**, 132 déclarés = 132 produits, 0 orphelin, 0 manquant
- ⚠️ `xtask verify-sensors-format` échoue sur `forgia2_arena.json` — **pré-existant et sans
  rapport** : c'est un artefact runtime non suivi par git, absent parce que le jeu n'a pas
  tourné en mode arène.

### Toujours pas de consommateur

`forgia-ai-arena-bot` avance encore en ligne droite. Le maillage existe et se construit
maintenant tout seul, mais **personne ne le suit** — c'est l'inc. 3.

---

## Incrément 3 — le bot suit enfin le chemin (2026-08-12)

### Deux découvertes qui ont réduit le scope

**1. Le chien de garde de désenlisement existait déjà.** L'incrément prévoyait de le
construire ; `unstick_step` le fait depuis longtemps, et bien : `stuck_secs` s'accumule sur
un seuil **relatif à la vitesse** (un bot qui rabote un mur avance encore un peu — tester
« déplacement nul » l'aurait raté), déclenche `unstick_left`, fait longer l'obstacle au
lieu de foncer, remet l'ardoise à zéro en sortie pour éviter la sortie perpétuelle, et
compte ses déclenchements dans le capteur.

> J'allais en écrire un second. C'est exactement le piège que `concept-first` §4 nomme :
> chercher le nom d'un type au lieu du **mot-concept**. La règle
> [`spawn-clearance.md`](../../.claude/rules/spawn-clearance.md) §5 dit encore que cette
> pièce n'existe pas — **elle est périmée sur ce point**.

**Conséquence heureuse** : ce mécanisme devient le **filet** sous le suivi de chemin, et
son compteur `unstick_triggered_session` offre une **mesure de succès gratuite** — il doit
**chuter**. S'il ne bouge pas après cet incrément, le chemin n'est pas suivi.

**2. Une grandeur écrite deux fois, et c'était ma faute.** `TacticalTuning::max_step_up_m`
vaut **0,45 m** ; le `step_height_m` que j'avais écrit à l'inc. 1 dans `navmesh.toml` vaut
**0,45 m**. Même grandeur, deux fichiers, deux crates — la classe de défaut n°1 du projet,
reproduite par moi douze heures plus tôt.

Le miroir est inévitable (crates séparées), donc `spawn-clearance.md` §4bis impose un test
qui compare les deux ensembles. Il existe :
`le_ressaut_du_navmesh_et_celui_du_bot_sont_la_meme_grandeur`. **S'ils divergent, le
maillage promet des chemins que le bot ne peut pas emprunter — et rien ne lève d'erreur :
le bot suit un trajet valide et se bloque contre une marche.**

### Le changement, et sa petitesse est le point

`bot_tactical_movement` empile strafe, évitement local, glissement contre les murs et
suivi de sol sur une direction de base. **Une seule ligne portait « fonce vers la cible ».**
C'est elle, et elle seule, qui change :

```rust
// avant
let fwd_dir = (to_target / dist).with_y(0.0).normalize_or_zero();
// après — le reste de la pile tactique est intact
let fwd_dir = nav_path.and_then(BotPath::current).map(…).unwrap_or(straight);
```

**Le repli en ligne droite est explicite et non négociable.** Hors arène, pendant le
chargement, ou si la cible est hors du maillage, le bot se comporte *exactement* comme
avant. On n'échange pas un défaut connu contre un bot immobile.

### Ce qui est neuf

- **`BotPath`** — composant : points, curseur, délai, cible planifiée. Le curseur avance
  **en boucle** : un grunt à 9 m/s franchit deux points serrés dans une frame, et
  s'arrêter au premier le ferait reculer.
- **`sys_bot_navpath`** — entretient le chemin avant le mouvement. Recalcul sur **trois
  raisons seulement** : plus de chemin, cible déplacée de plus de 2 m, ou délai de 0,5 s
  écoulé. Le chemin est du travail de *planification*, pas de frame.
- **`sys_attach_bot_path`** — attache le composant aux bots existants, plutôt que d'aller
  modifier chaque crate de mode qui les spawne.
- Trois gènes dans `TacticalTuning` : `waypoint_arrive_m`, `repath_period_secs`,
  `target_moved_repath_m`.
- Sans maillage, `sys_bot_navpath` **vide** les chemins au lieu de les laisser périmer.

### Vérifications

- `cargo test -p forgia-ai-arena-bot` — **38 passés, 0 échec** (26 + 12)
- `cargo clippy -p forgia-ai-arena-bot --all-targets` — **0 warning** (vrai cargo)
- Le test miroir du ressaut **passe** : 0,45 = 0,45

### Ce qui reste à prouver, et seul le jeu peut le faire

Aucun test headless ne dit qu'un bot *se sent* mieux. Le juge est le capteur :
**`unstick_triggered_session` doit chuter** entre une run d'avant et une run d'après, et
`forgia2_navmesh.json` doit montrer `obstacles_kept > 0` sur la même run. Les deux chiffres
existent déjà — il ne manque que la run.

---

## ✅ VALIDATION EN JEU — 2026-08-13, 00:03

La run que quatre incréments attendaient. **Trois résultats, trois natures de preuve.**

### 1. Le maillage se construit — capteur

```json
"severity": "ok", "built": true, "source": "forge_sanctum",
"discs_seen": 166, "obstacles_kept": 13, "blind": false,
"build_ms": 0.52, "builds_session": 1
```

**0,52 ms** de construction, et `builds_session` — le compteur ajouté le soir même — répond
enfin à la question pour laquelle le capteur existait.

### 2. Les bots suivent le chemin — mesure normalisée

| | Run 21:56 (avant) | **Run 00:03 (après)** |
| --- | --- | --- |
| Checks LOS — *l'activité des bots* | 695 | **1 248** |
| **Désenlisements déclenchés** | 18 | **2** |
| Un désenlisement tous les… | 39 checks | **624 checks** |

L'activité a **doublé** pendant que les blocages étaient divisés par 9 : normalisé,
**16× moins de blocages**. Comparer les compteurs bruts aurait été trompeur — c'est le
ratio qui prouve, parce qu'il est insensible à la durée de la run.

### 3. Les ennemis ne naissent plus dans les bâtiments — l'œil

> *« Non, ils n'étaient plus dans un mur ou bâtiment. »* — Antoine, 2026-08-13

**Aucun capteur ne pouvait rendre ce verdict.** C'est précisément ce que
[`REFONTE_GDD.md`](../REFONTE_GDD.md) §3 dit du palier de validation : *la seule chose
qu'aucun test ne peut faire, confronter le code à la réalité.*

### La réserve honnête

`obstacles_kept: 13` **sur 166 solides soumis** — les 153 autres passent sous le ressaut de
0,45 m. Et ça recoupe l'alerte `stage_layout` : *« sous-couverte d'un facteur 11,6 : 13
abris pour 151 attendus »*. Le maillage fonctionne, mais **l'arène ne lui donne pas
grand-chose à contourner**. Défaut de contenu connu et antérieur — la preuve est réelle,
l'épreuve est plus douce qu'elle ne le sera dans une vraie salle.

---

## Incrément 4 — le compagnon, découpé (2026-08-12)

Tel qu'écrit — *suivre, se poster, barre PV* — l'incrément touchait **trois zones à la
fois** : une crate neuve, le spawn dans une crate de mode, et `hud.rs` (2 671 lignes,
god-file, zone possible de l'autre terminal). C'est le format d'incrément qui part en
vrille et qu'on n'ose plus committer. Découpé :

| Sous-inc. | Contenu | Zone | État |
| --- | --- | --- | --- |
| **4a** | `Faction` | `forgia-core` | ✅ **livré** |
| **4b** | Crate `forgia-companion` : suivre, se poster, capteur `forgia2_companion.json` | Neuve, isolée | ⬜ |
| **4c** | Le spawn — le compagnon devient **visible** | Crate de mode | ⬜ |
| **4d** | Barre PV permanente (GDD §4 HUD duo) | `hud.rs` — **à coordonner** | ⬜ |

### 4a — `Faction`, la porte qu'il fallait ouvrir maintenant

Quatre camps : `Player`, `Allied`, `Hostile`, `Neutral`. Dans `forgia-core` parce qu'il a
**zéro dépendance workspace** : n'importe quelle crate peut en parler sans créer de cycle.

C'est l'une des **quatre portes à ne pas fermer** du [GDD §10](../design/gdd-forgia-the-spared.md),
et trois chantiers en dépendent — le compagnon (E1), le coéquipier humain (E9), les équipes
du 5v5 (E10). *Le même camp, trois sources.*

**Deux décisions :**

- **`Player` et `Allied` sont distincts mais du même bord.** Un compagnon n'est pas le
  joueur : le loot personnel, la caméra et surtout les **combos élémentaires** du GDD §4
  doivent pouvoir les séparer. Mais ils ne se tirent pas dessus.
- **`Neutral` n'est ni allié ni cible de personne.** *« Pas mon allié » n'est pas « ma
  cible »* — les confondre ferait tirer les bots sur la faune. Un test l'exige.

Le défaut est `Player` : **un composant oublié produit l'entité la plus inoffensive du
jeu**, pas un hostile qui attaquerait tout le monde.

Le type **nomme les camps, il n'arbitre rien** — ni dégâts, ni ciblage, ni aggro. Ces
règles restent aux systèmes qui les appliquent, pour qu'une table centrale ne devienne pas
le passage obligé de tout le gameplay.

**Vérifications** : 5 tests verts · clippy 0 warning.

> ⚠️ **Leçon de méthode.** `cargo check -p forgia-core` est passé au vert alors que le
> module de tests ne compilait pas — il ne construit pas la cible test. C'est
> `--all-targets` qui l'a attrapé. Un check qui ne compile pas ce qu'on prétend vérifier
> est le même défaut qu'un capteur à zéro qui dit « ok » ([story-699](story-699-un-capteur-a-zero-ne-doit-pas-dire-ok.md)).

### La dette d'observabilité de cette story

**Trois incréments de suite sans rien de visible en jeu** (1, 2, 3), et le 4a n'y change
rien. C'est assumé et écrit à chaque fois — mais ça ne peut pas durer. **Le 4c est celui
qui doit tomber** pour qu'un joueur voie enfin quelque chose bouger.

---

## Test runtime

Sans objet à cet incrément — **aucun effet observable en jeu**, par construction : rien ne consomme
encore le maillage. La preuve de cet incrément est mécanique (build + 11 tests + clippy), et c'est
exactement ce qu'elle prétend être. Le premier récap de test runtime viendra à l'inc. 3, quand un
bot suivra réellement un chemin.
