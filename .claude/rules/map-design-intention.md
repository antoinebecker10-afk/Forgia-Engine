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

Les archétypes existent déjà, en couche definition : `assets/genomes/enemies/`.
Valeurs `default` (chaque gène est borné min/max et hot-reloadable) :

| Archétype | pv | vitesse | vision | portée | comportement |
|---|---|---|---|---|---|
| **grunt** `enemy_grunt` | 30 | **9,0 m/s** | **20 m** | mêlée 3,0 m | essaim, rush en groupe |
| **archer** `enemy_archer` | 45 | 5,5 m/s (repli ×0,6) | **35 m** | tir **15 m** | kite, garde ses distances |
| **elite** `enemy_elite` | 120 | 5,0 m/s (**charge ×2,5 = 12,5**) | **25 m** | mêlée 3,5 m | charge |
| **boss** `boss_default` | tuné | — | — | — | phases à seuils de % pv |

### 2.1 Une salle doit laisser l'archétype ARRIVER

C'est la relation qui décide de la taille d'une salle de mêlée. Le joueur tue en
séquence ; pendant qu'il descend la file, les survivants avancent.

```
portée_d'engagement_max  =  vitesse × (N × pv / dps) + portée_d'attaque
```

Avec le fusil (168 dps) :

| | TTK unitaire | avance pendant son propre TTK | essaim de 8 |
|---|---|---|---|
| grunt | 30/168 = **0,18 s** | 1,6 m | avance **12,9 m** → arrive si l'engagement démarre sous **~16 m** |
| archer | 45/168 = **0,27 s** | 1,5 m | tire dès 15 m, n'a pas besoin d'arriver |
| elite | 120/168 = **0,71 s** | **8,9 m** en charge | arrive de ~12 m en solo |

**Verdict sur la cour** (ligne max mesurée **24,2 m**, médiane 10,8 m) : un essaim
de 8 grunts lâché à la ligne max **n'atteint jamais le joueur**. La salle est trop
grande pour son archétype de mêlée. À la médiane, il arrive. Donc soit les grunts
apparaissent près, soit la salle change de rôle.

> Une salle trop grande ne rend pas le combat « plus aéré » : elle **supprime**
> l'archétype de mêlée. C'est un choix de design, pas un réglage de confort.

### 2.2 La plus longue ligne ne doit pas dépasser la vision des ennemis

Un ennemi dont la portée de vision est inférieure à la plus longue ligne de sa
salle se fait tirer **sans pouvoir répondre ni réagir**.

- grunt vision **20 m** vs cour ligne max **24,2 m** → **4 m de tir gratuit**
- elite vision 25 m → couvert ✓ · archer 35 m → couvert ✓

Règle : `ligne_max(salle) ≤ min(vision des ennemis qui y apparaissent)`, ou bien
casser la ligne avec de l'occultation.

### 2.3 Chaque archétype impose sa géométrie

- **grunt** — lignes courtes et **plusieurs approches** : un essaim qui arrive par
  un seul goulot se fait faucher en file indienne, ce n'est plus un essaim.
- **archer** — de la **profondeur de repli derrière lui** (il recule à 3,3 m/s) et
  de la couverture à sa hauteur. Un archer dos au mur ne kite pas, il meurt.
- **elite** — une **ligne de charge dégagée** de l'ordre de 10 m. Les couvertures
  qui cassent cette ligne annulent son archétype.
- **boss** — de l'espace pour esquiver, une arène **lisible depuis le seuil**,
  aucune couverture totale qui permette d'attendre la fin des phases à l'abri.

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
- `assets/genomes/enemies/` — les archétypes · `assets/genomes/arena_waves.toml` — les vagues
- `no-hardcode.md` · `genome-code.md` — les valeurs vivent en couche definition
