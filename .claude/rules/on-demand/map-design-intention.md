# Concevoir une carte (Forgia) — CE QU'ON BÂTIT, avant COMMENT

> Compagnon amont de [`map-design-patterns.md`](map-design-patterns.md), qui traite
> la construction géométrique. Celui-ci traite ce qui vient **avant** : l'intention,
> les ennemis, la composition d'une salle, le rythme d'une run, et la porte de sortie.
>
> **Pourquoi il existe.** Le banc Arena Test a produit 35 défauts et une boucle de
> correction qui ne se fermait pas. Cause profonde : la carte n'avait **aucune
> spécification de combat**, donc aucun critère disait quand s'arrêter. On mesurait
> « 76 % de lignes moyennes dans la cour » sans pouvoir dire si c'était bien —
> parce qu'il n'y a **aucun ennemi dans cette carte**. Un blockout est une
> hypothèse sur un combat ; sans le combat, rien n'est falsifiable.

---

## 0. Procédure — quand on demande une carte

**Dans cet ordre. La §1 est bloquante : sans elle, la géométrie n'a pas de juge.**

1. **Écrire la spec de combat** (§1). Si les entrées manquent, les demander — ce
   sont des décisions de game design, pas des valeurs à inventer.
2. **Confronter aux archétypes ennemis** (§2). C'est là que la taille d'une salle
   cesse d'être une question de confort.
3. **Dériver les dimensions** de la spec. Aucun nombre choisi.
4. **Composer chaque salle** (§3).
5. **Poser le rythme** de la séquence (§4).
6. **Construire** — passer aux 14 patterns de `map-design-patterns.md`.
7. **Passer la porte de sortie** (§5) avant d'annoncer quoi que ce soit.

---

## 1. La spec de combat — le document qui rend tout falsifiable

Une salle de combat se décrit AVANT d'être dessinée :

```
SALLE <id>            rôle : <combat | repos | récompense | boss | transit>
  ennemis            : <N> × <grunt | archer | elite | boss>, arrivant de <points>
  arsenal joueur     : <armes attendues à ce moment de la run>
  durée d'engagement : <t> s visées
  condition de sortie: <tous morts | objectif | survivre t s>
  ce qu'on veut voir : <une phrase — la scène, pas la géométrie>
```

### Ce qui se DÉRIVE de la spec (et ne se choisit donc jamais)

| Grandeur | Formule | Vérification sur la carte actuelle |
|---|---|---|
| **Durée d'engagement** | `Σ pv / dps_arme` | fusil **168 dps**, smg 121 |
| **Traversée de salle** | `diagonale / vitesse` | cour diag 31,1 m → **4,8 s** en marche, **3,2 s** en sprint |
| **Nombre de couvertures** | `aire / espacement²`, espacement **3–10 m** (Watch Dogs, Gears) | cour 484 m² à 6 m → **≈ 13 attendues**, on en a **4**. Sous-couverte d'un facteur 3 |
| **Portée dominante** | l'arsenal à ce moment de la run | pleine puissance ≤ **30 m**, −40 % au-delà |
| **Désengagement** | `distance_rupture / sprint` | sortir de la bande pleine puissance ≈ **3 s** à 9,75 m/s |
| **Entrées joueur** | **≥ 2** par salle de combat | une salle de combat à une entrée est un cul-de-sac |

**La spec est la seule chose qui transforme une mesure en verdict.** « 0 % de
lignes au-delà de 30 m » n'est un défaut que parce que l'arsenal contient un
sniper 300 m — sans spec d'arsenal, c'est juste un nombre.

---

## 2. Les ennemis — la contrainte géométrique la plus dure

> ⚠️ **Corrigé le 2026-08-18.** Cette section citait `assets/genomes/enemies/`
> (grunt / archer / elite). **Ces quatre fichiers n'ont aucun consommateur
> Rust** — vérifié par `python tools/ai/strates.py` (C2 : 61 génomes morts sur
> 141). La §2.1 dimensionnait donc des salles contre des ennemis qui n'existent
> pas, et le résultat avait l'air sourcé. Les valeurs ci-dessous sont celles que
> le jeu lit vraiment. Détail : `docs/audit/audit-2026-08-18-strates-the-spared.md` §4.
>
> **Avant de citer un génome dans un raisonnement de design** :
> `grep -rl "$(basename FICHIER .toml)" --include=*.rs crates/ src/` — zéro
> résultat = donnée morte, quelle que soit sa cohérence interne.

Source unique et **vivante** : `assets/genomes/roguelite/roguelite_enemies.toml`
(hot-reload par poll mtime 1 Hz ; miroir exact des `Default` Rust de
`forgia-mode-roguelite/src/enemies.rs` — si le fichier disparaît, le jeu retombe
dessus). L'énumération est `EnemyArchetype { Tank, Runner, Sniper, Boss }`.

| Archétype | pv | vitesse | détection | portée de tir | dégâts | comportement réel |
|---|---:|---:|---:|---:|---:|---|
| **tank** | 120 | **2,8 m/s** | 22 m | 5 m (`stop` 3,0) | 25 | avance lentement, frappe au contact |
| **runner** | 35 | **7,0 m/s** | 40 m | 8 m (`stop` 6,0) | 8 | se rapproche vite, harcèle **à courte portée** |
| **sniper** | 45 | 3,2 m/s | **55 m** | **28 m** (`stop` 22,0) | 18 | tient ses distances, `jitter` 1,5° |
| **boss** | 800 | 3,5 m/s | **80 m** | 32 m | 22 | unique, 2 phases (enrage à 50 % pv) |

**Trois écarts au modèle « essaim » que la version précédente supposait, et qui
changent le dimensionnement** :

1. **Aucun archétype n'est en mêlée pure.** Les quatre tirent
   (`shoot_damage`/`shoot_range`). Le tank tire à 5 m — c'est du contact, mais
   c'est un tir : il n'a pas besoin de *toucher* le joueur.
2. **Le plus lent est le plus résistant** (tank 120 pv à 2,8 m/s), l'inverse du
   grunt rapide et fragile. Une grande salle ne *supprime* donc pas un
   archétype ; elle allonge son approche pendant qu'il tire déjà.
3. **La détection dépasse partout la portée d'attaque** (sniper : 55 vs 28). Le
   §2.2 « ligne_max ≤ vision » est donc **beaucoup moins contraignant** qu'écrit :
   le vrai plafond est la portée de tir, pas la vision.

Les raisonnements des §2.1 à §2.6 restent valides **comme méthode** ; leurs
chiffres d'exemple sont à re-dériver de ce tableau.

### 2.1 Une salle doit laisser l'archétype ARRIVER

C'est la relation qui décide de la taille d'une salle de mêlée. Le joueur tue en
séquence ; pendant qu'il descend la file, les survivants avancent.

```
portée_d'engagement_max  =  vitesse × (N × pv / dps) + portée_d'attaque
```

Avec le fusil (168 dps), sur les archétypes **réels** du §2 :

| | TTK unitaire | avance pendant son propre TTK | groupe de 8 |
|---|---|---|---|
| runner | 35/168 = **0,21 s** | 1,5 m | avance **11,7 m** → sous sa portée de tir (8 m) dès ~20 m |
| tank | 120/168 = **0,71 s** | 2,0 m | avance **16,0 m** → il tire à 5 m, donc il lui faut ~21 m pour entrer en jeu |
| sniper | 45/168 = **0,27 s** | 0,9 m | **n'a pas à avancer** : il tire à 28 m |

**Ce que ça change par rapport au modèle « essaim de mêlée »** : la question
n'est plus *« la salle est-elle assez petite pour qu'ils arrivent ? »* mais
*« la salle place-t-elle le joueur dans la bande de tir de qui ? »*. Un tank
lâché à 25 m tire quand même — il arrive juste tard. Le seul archétype qu'une
grande salle neutralise vraiment est le **runner**, dont la portée est de 8 m.

> Une salle ne « supprime » pas un archétype par sa taille seule : elle décide
> **lequel des quatre tire en premier**. C'est un choix de design, pas un réglage
> de confort.

### 2.2 La plus longue ligne se compare à la portée de TIR, pas à la vision

Un ennemi dont la portée d'attaque est inférieure à la plus longue ligne de sa
salle se fait tirer **sans pouvoir répondre**. Sur les archétypes réels, la
détection dépasse partout l'attaque (sniper 55 vs 28) — **c'est donc la portée de
tir qui plafonne**, jamais la vision.

- runner tir **8 m** → au-delà, il court sous le feu sans riposter
- tank tir **5 m** → même chose, en plus lent
- sniper tir **28 m** · boss **32 m** → seuls à tenir une ligne longue

Règle : `ligne_max(salle) ≤ max(portée de tir des ennemis qui y apparaissent)`,
ou bien casser la ligne avec de l'occultation. Corollaire : **une salle de plus
de ~28 m sans sniper ni boss est un stand de tir.**

### 2.3 Chaque archétype impose sa géométrie

- **runner** (7,0 m/s, tir 8 m) — lignes courtes et **plusieurs approches** : un
  groupe qui arrive par un seul goulot se fait faucher en file indienne. C'est le
  seul archétype qu'une grande salle neutralise vraiment.
- **tank** (2,8 m/s, 120 pv, tir 5 m) — il lui faut du **temps sous le feu** :
  une couverture intermédiaire à mi-parcours, sinon il meurt avant d'entrer dans
  sa portée et n'a servi qu'à absorber des balles.
- **sniper** (tir 28 m, `stop_distance` 22 m) — une **ligne longue tenue depuis
  l'arrière** et de la couverture à sa hauteur. Enfermé dans une petite salle il
  perd son archétype ; il ne recule pas pour le retrouver.
- **boss** (800 pv, tir 32 m, détection 80 m) — de l'espace pour esquiver, une
  arène **lisible depuis le seuil**, aucune couverture totale qui permette
  d'attendre la fin des phases à l'abri.

> **Ce que l'IA sait faire, et rien de plus.** `BotState` n'a que quatre
> valeurs — `Idle · Chase · Attack · Dead`. Le mouvement (`tactical.rs :
> bot_tactical_movement`) est une poursuite avec **strafe sinusoïdal**
> (amplitude 1,8 m, 0,9 Hz, 35 % de bruit — « Doom imp dodge »), évitement
> d'obstacle, séparation entre bots, et un dés-enlisement. Il n'existe **ni
> repli, ni kite, ni prise de couverture, ni charge**. Ne pas concevoir une salle
> autour d'un comportement qu'aucun système ne produit — les différences entre
> archétypes ci-dessus sont **entièrement portées par les stats**.

### 2.4 Les arrivées

- Elles se **déclarent dans la spec**, jamais après coup.
- **Distinctes des entrées joueur** (§3.2) : sinon le combat commence au seuil,
  avant qu'on ait lu la salle.
- **Ni dans le dos du joueur au contact, ni hors de sa vue** : une arrivée
  invisible est perçue comme une triche, une arrivée dans la ligne de tir meurt
  avant d'agir.
- **Atteignables** : une arrivée d'où l'ennemi ne peut pas rejoindre le joueur est
  un ennemi décoratif — cf. §2.5.

### 2.5 L'IA n'a pas les jambes du joueur

Le joueur saute **1,174 m** et n'a ni grimpe ni dash. **L'IA ne saute pas.** Donc :

- tout ressaut supérieur au pas de l'IA est une **barrière pour elle** et un
  **exploit pour le joueur** — on monte quelque part où rien ne suit ;
- les **61 m² de bords sans garde-corps** de la carte actuelle sont exactement ça :
  le joueur descend d'un niveau, les poursuivants restent en haut ;
- toute liaison verticale d'une route doit être une **rampe** franchissable par
  l'IA, pas une marche. C'est déjà l'invariant du pattern 7 côté géométrie ; ici
  c'est sa raison **de gameplay**.

### 2.6 La couverture est bidirectionnelle

Une couverture ne sert pas que le joueur. Si toutes les couvertures d'une salle ne
protègent que lui, la salle est un stand de tir. Vérifier qu'une position d'abri
existe **du côté des arrivées ennemies** aussi.

---

## 3. Composer une salle

**3.1 Le battement d'une salle.** Quatre temps : **entrée** (on lit la salle avant
d'y être engagé) → **engagement** → **repli / repositionnement** → **résolution**.
Chaque temps doit avoir son support géométrique. Une salle sans repli est un
couloir avec des ennemis dedans.

**3.2 Les arrivées ennemies sont distinctes des entrées joueur.** Voir §2.4.

**3.3 On lit la salle depuis son seuil.** Depuis chaque entrée, on doit voir les
sorties et la forme générale. Une salle qu'on ne lit qu'en la traversant force
l'exploration à l'aveugle sous le feu.

**3.4 Pas de couverture collée à une entrée.** Un abri au seuil transforme
l'entrée en position tenable et bloque la circulation.

**3.5 La densité est inverse à la portée de la voie.** Une voie longue encombrée
n'est plus une voie longue — c'est ce qui a fait échouer Berlin (CoD Vanguard).

---

## 4. Le rythme d'une run

**4.1 Alterner tension et relâche.** Combat → transit ou récompense → combat. Deux
salles de combat consécutives sans respiration se lisent comme une seule salle
trop longue.

**4.2 Une branche facultative doit coûter.** Un détour de récompense qui ne coûte
ni temps ni risque n'est pas un choix, c'est un couloir que tout le monde prend.
*Notre branche `ruines` coûte +2 m de détour : elle est gratuite.*

**4.3 Budget de durée de run.** `Σ engagements + Σ transits`, le transit étant
`longueur de trajet / vitesse`. Sans budget, la longueur d'une run est un accident.

**4.4 La courbe de difficulté est portée par les archétypes ET la géométrie.**
Le même essaim est trivial dans une grande salle et mortel dans une petite (§2.1).
Une montée en difficulté qui ne joue que sur les multiplicateurs de pv
(cf. `arena_waves.toml`) laisse la moitié du levier inutilisée.

**4.5 Roguelite : déclarer CE QUI VARIE.** Squelette fixe et contenu variable, ou
squelette variable ? Une carte à disposition unique n'est pas une carte de
roguelite — c'est un niveau. *Notre carte a une seule disposition, et aucun
document ne dit si c'est voulu.*

---

## 5. Le nom est un contrat, et la porte de sortie

### 5.1 Le nom est un contrat

**Un élément déclaré DOIT être ce que son nom dit.** Trois défauts du registre
n'ont pas d'autre cause :

- une route déclarée **large de 14 m** qui occupait toute la section de la salle ;
- un `role = "bridge"` posé sur un **massif plein**, dont les piles sont enterrées ;
- un contrôle de marches au vert alors que la carte contient **zéro bloc
  `traversal`** — il ne mesurait rien.

Corollaire : **un rôle sans instance est une alerte, pas un silence.**

### 5.2 Porte de sortie — ce qui doit être vrai avant d'annoncer « fini »

- [ ] Chaque salle de combat a sa **spec** (§1), et ses dimensions en **dérivent**
- [ ] Chaque salle laisse ses archétypes **arriver** (§2.1) et **voir** (§2.2)
- [ ] Les **arrivées ennemies** sont déclarées, distinctes, atteignables (§2.4)
- [ ] Aucun endroit accessible au joueur et **pas à l'IA** (§2.5)
- [ ] Nombre de couvertures conforme à la dérivation, et **bidirectionnelles** (§2.6)
- [ ] Chaque salle de combat a **≥ 2 entrées joueur**
- [ ] Le **profil de portées mesuré** correspond à l'arsenal déclaré
- [ ] **Aucun rôle déclaré sans instance** (§5.1)
- [ ] Les 14 patterns de construction passent — voir leur **tableau d'état**
- [ ] Budget de durée de run posé et mesuré (§4.3)
- [ ] Ce qui varie entre deux runs est **écrit** (§4.5)
- [ ] Les décisions réservées au playtest sont **listées comme telles** (§5.3)

Tant qu'une case manque, on dit **où on en est**, pas « fini ».

### 5.3 Ce que les chiffres ne tranchent pas

Les barèmes proposent, le playtest dispose. Se décident **manette en main**, jamais
depuis un tableau : la taille ressentie d'une salle, la lisibilité d'un repère, le
plaisir d'une rampe, le rythme perçu, la lisibilité d'un essaim. *La seule
validation qui ait tenu sur cette carte — « la taille c'est bon » — a été rendue en
jeu.* Attention : §2.1 montre qu'une taille validée **au ressenti, sans ennemis**,
peut condamner un archétype. Les deux jugements sont nécessaires.

### 5.4 Un changement de géométrie périme des mesures

Déplacer une salle invalide en silence tout ce qui a été mesuré autour. Après tout
changement d'emprise ou de niveau : **re-dériver** le dossier de mesures avant de
le citer. *Un audit de 78 constats a été produit sur une carte qui n'existait
plus, et cinq chiffres « défaut connu » codés en dur étaient tous faux.*

---

## Cross-refs

- [`map-design-patterns.md`](map-design-patterns.md) — les 14 patterns de construction + tableau d'état
- [Registre des défauts](../../docs/audits/arena-test-registre-defauts.md) — la matière première
- `assets/genomes/roguelite/roguelite_enemies.toml` — **les archétypes vivants**
  (`assets/genomes/enemies/` est mort : 0 consommateur Rust, cf. §2)
- `assets/genomes/arena_waves.toml` — les vagues
- `tools/ai/strates.py` — **avant de citer un génome ici**, vérifier qu'il est lu
  (C2 : 61 fichiers morts sur 141 au 2026-08-18)
- [`docs/audit/audit-2026-08-18-strates-the-spared.md`](../../../docs/audit/audit-2026-08-18-strates-the-spared.md) §4 — pourquoi cette section a été corrigée
- `no-hardcode.md` · `genome-code.md` — les valeurs vivent en couche definition
