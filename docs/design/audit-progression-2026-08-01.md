# Audit de la progression — pourquoi c'est « trop simple », et quoi faire

**Date** : 2026-08-01
**Méthode** : lecture du code et des génomes réels + comparaison avec Gunfire Reborn,
Hades, Risk of Rain 2 et la littérature de design roguelite.
**Cross-refs** : story-677 (la boucle et son mur), `docs/design/boucles-roguelite-etat-et-benchmarks-2026-07-31.md`

---

## Le verdict en une phrase

Il y a **cinq systèmes de progression**, et ils font tous **exactement la même
chose** : multiplier un nombre. Il n'y a donc aucune décision à prendre — et
c'est ça, la sensation de « trop simple ». Ce n'est pas un manque de contenu,
c'est un manque de **choix**.

Et à côté de ça, le système qui s'appelle littéralement « niveau » ne fait
**rien du tout**.

---

## A. Le « leveling » au sens strict est INERTE

[`progress.rs`](../../crates/forgia-mode-roguelite/src/progress.rs) tient un
niveau, de l'XP et des points de talent. Voici ce qu'ils valent :

| Élément | Réalité mesurée dans le code |
|---|---|
| **Gain d'XP** | `40 + durée de la run en secondes` (`sys_award_run_xp`) |
| **Performance** | **aucun terme.** Mourir au round 1 ou clear 10 rounds ne change rien d'autre que le chrono |
| **Défaite / Victoire** | **identiques** — le même système tire sur les deux |
| **Courbe** | `xp_to_next = 80 + (niveau − 1) × 40`, linéaire |
| **Points de talent** | +1 par niveau, **jamais dépensés** |

Le seul consommateur de `talent_points` dans tout le workspace est
[`forgia-ui/src/lib.rs:452`](../../crates/forgia-ui/src/lib.rs#L452) — qui les
**affiche**. Rien ne les dépense.

### Le pire : l'écran promet quelque chose qui n'existe pas

La page Talents du Hall affiche, mot pour mot :

> « Choisis ton style — Feu · Givre · Éclair · Poison — et débloque des combos
> signature en jouant. »
> « N point(s) de talent en attente »

Il n'y a **ni arbre, ni style, ni combo signature**. Le joueur accumule des
points en lisant une promesse. C'est le défaut le plus grave de cet audit :
un système qui ne fait rien serait neutre, un système qui **annonce** ce qu'il
ne fait pas est une déception programmée.

### Et l'XP récompense la LENTEUR

`40 + secondes` : un joueur qui traîne gagne strictement plus qu'un joueur qui
nettoie vite. C'est l'inverse exact de ce que la boucle de rounds demande
(story-677 : nettoyer dans le budget). Les deux systèmes se contredisent.

---

## B. La vraie progression : cinq systèmes, un seul verbe

| Système | Ce qu'il donne | Plafond |
|---|---|---|
| **Méta-boutique** | +15 PV ×5 · +8 % dégâts ×5 · +5 % réduction ×4 · +50 Or ×3 | stats |
| **Maîtrise d'arme** | +4 % dégâts / niveau, cap 6 | +20 % |
| **Trempe** (in-run) | +15 % dégâts / palier, cap 5 | ×2,01 |
| **Équipement** (story-675) | rareté = multiplicateur ×1 à ×5 du gain de slot | stat |
| **Boons** (18) | `damage_mul` ×4, `fire_rate_mul` ×3, `flat_bonus` ×3, `heal_on_kill` ×2, `damage_reduction` ×2, `knockback` ×2, `chain_targets` ×2 | stat |

**Cinq systèmes, cinq façons d'écrire « +X % ».** Prendre +8 % de dégâts n'est
pas un choix : c'est une formalité. On clique, on ne décide pas.

C'est précisément le piège que la littérature de design nomme : les upgrades de
stats permanents sont décrits par les joueurs comme *« an extremely unsatisfying
kind of progression »*, et les roguelites qui durent penchent lourdement vers la
croissance **horizontale** — nouvelles armes, mécaniques, modificateurs de règles
— parce qu'elle nourrit l'expérimentation au lieu du power creep.

### Le pool est trop petit pour créer de la variété

**18 boons**, dont 8 communs. Le tirage est pondéré (common 100 / uncommon 45 /
rare 18 / legendary 6), donc en pratique on revoit les mêmes 8 en boucle.

Pour comparaison : Hades compte des centaines de boons ; Gunfire Reborn a un
arbre de talents réparti en **six catégories** (Expédition, Combat, Compétence,
Survie, Arme, Héros) plus les Ascensions. Avec 18 entrées, tout est vu en deux ou
trois runs.

---

## C. Le seul levier horizontal existe déjà — et il dort

Deux choses sont déjà dans le code et sous-exploitées :

**Les tags.** `roguelite_boons.toml` déclare : *« 3 tags identiques pendant la run
→ légendaire correspondant unlock »*, sur 6 tags (`fire`, `ricochet`,
`knockback`, `chain`, `precision`, `chaos`). **C'est un vrai système de
synergie** — le seul endroit du jeu où un choix en conditionne un autre. Mais
avec 18 boons répartis sur 6 tags, l'archétype ne se construit presque jamais.

**Les déblocages.** `weapon_unlocks` (3 armes) et `boon_tier_unlocks` (3 paliers)
sont de la progression horizontale correcte — ils **élargissent le pool** au lieu
de gonfler les nombres. Il y en a six en tout.

---

## D. Ce que font les références

**Gunfire Reborn — le niveau EST la somme de tes choix.** Il n'y a pas d'XP du
tout : *« le système de niveau tourne autour de la dépense d'une monnaie en
talents plutôt que de la collecte d'expérience »*. Ton niveau **égale le nombre
de talents que tu possèdes. Un talent 2/5 te donne 2 niveaux.** C'est élégant :
le niveau ne peut pas être vide, par construction.

**Hades — le choix est exclusif.** Le Miroir de Nuit propose **deux versions de
chaque capacité** (rouge et verte) et tu ne peux en activer **qu'une**. Prendre
A **interdit** B. C'est ce qui transforme un achat en décision.

**Hades — les rendements décroissent, avec un plancher.** Les Poms of Power ont
un effet fort sur les 2-3 premiers niveaux puis décroissant, *« mais la chute a
un plancher »* : même très haut, on gagne encore un peu. Résultat : un arbitrage
permanent entre « approfondir » et « ouvrir autre chose ».

**Risk of Rain 2 — les objets changent le comportement, pas les nombres.** Les
développeurs disent explicitement qu'au lieu d'implémenter les objets comme de
simples gains de stats, ils laissent le joueur **choisir quand les utiliser** —
*« donner un choix au joueur est toujours quelque chose qu'ils veulent tisser
dans le design des objets »*.

**Risk of Rain 2 — l'identité de build se VOIT.** Montrer les objets collectés
sur le personnage a été *« l'une des forces motrices du passage à la 3D »*. Le
joueur doit voir sa build sur lui.

---

## E. Ce que je recommande, par valeur

### 1. Le niveau doit acheter quelque chose — ou disparaître

C'est le point le plus urgent, parce que l'écran ment aujourd'hui. Deux issues
honnêtes :

- **(a) Modèle Gunfire — supprimer l'XP.** Le niveau devient le **nombre de rangs
  achetés** dans la méta-boutique. Il ne peut plus être creux : il *est* la somme
  des choix. Bonus : ça supprime un système inerte et une deuxième monnaie
  redondante avec les Âmes.
- **(b) Livrer l'arbre promis** — Feu / Givre / Éclair / Poison, comme annoncé.

**Je recommande (a) maintenant, (b) plus tard.** (a) coûte une soirée et supprime
le mensonge ; (b) est une vraie feature qui mérite son propre cadrage. Ce qui
n'est pas défendable, c'est de laisser la page en l'état.

### 2. Rendre les choix EXCLUSIFS (modèle Miroir de Nuit)

Un choix n'existe que si prendre A interdit B. Concrètement : la méta-boutique
propose **deux variantes par ligne**, une seule active, permutable au Hall.
Exemple : *Vitalité* (+15 PV) **ou** *Sursis* (une seconde chance par run). Même
coût, effets incomparables — donc une vraie décision, et deux styles de jeu.

C'est le changement au meilleur rapport valeur/effort de toute la liste : il ne
demande **aucun contenu nouveau**, seulement une variante par ligne existante.

### 3. Des rendements décroissants avec plancher (modèle Pom)

+8 % cinq fois d'affilée est plat. Le rang 1 doit être fort, le rang 5 marginal —
ça crée l'arbitrage « j'approfondis ou j'ouvre autre chose ». Les coûts croissent
déjà (`[25, 50, 85, 130, 190]`) ; le **gain**, lui, est constant. C'est le
contraire de ce qu'il faut.

### 4. Réveiller les tags — le seul levier horizontal déjà présent

Le mécanisme « 3 tags → légendaire » existe et ne se déclenche presque jamais.
Trois pistes, par coût croissant : afficher la progression des tags à l'écran
(sinon le joueur ne peut pas jouer avec) · abaisser le seuil ou donner 2 tags à
certains boons · faire des tags de vrais **archétypes** avec un bonus de palier
à 2 et 4.

### 5. Grossir le pool — mais PAS avec des stats

18 boons, c'est trop peu. Mais ajouter 20 boons « +X % dégâts » n'apporterait
rien : ça multiplierait le même non-choix. Ce qu'il faut, ce sont des boons qui
**changent le comportement** : ricochet, chaîne, explosion à la mort, tir chargé,
munitions qui se rechargent sur kill. Le catalogue a déjà `chain_targets` et
`knockback` — ce sont les seuls du lot qui modifient la façon de jouer, et il y
en a deux de chaque.

### 6. Faire VOIR la build

Story-675 a livré un personnage découpé en pièces d'équipement, avec rareté =
couleur. C'est **exactement** la fondation de ce que RoR2 a cherché en passant en
3D. Rendre les pièces visibles sur le viewmodel / à l'écran de fin de run coûte
peu et donne au joueur l'image de sa progression.

### 7. Faire converger l'XP avec la boucle

Si on garde une XP (option (b)), elle doit récompenser **ce que la boucle
demande** : le round atteint, la marge de temps, les paliers franchis — pas la
durée. Aujourd'hui les deux systèmes se contredisent : la boucle demande de
nettoyer vite, l'XP paie la lenteur.

---

## F. Ce que cet audit ne couvre pas

- **Les éléments** (`elements.rs`, 87 Ko) — le plus gros module du mode, non
  audité ici. Il porte probablement une part de la profondeur réelle du jeu, et
  mérite son propre passage.
- **Aucun réglage n'a été mesuré en jeu.** Les plafonds cités (+20 % maîtrise,
  ×2,01 Trempe) sont lus dans les génomes, pas observés sur une run.
- **La rétention** — combien de runs avant que le joueur ait tout vu — n'est pas
  mesurable sans télémétrie de session.

---

## Sources

- [Gunfire Reborn — Talents (Fandom)](https://gunfirereborn.fandom.com/wiki/Talents)
- [Gunfire Reborn — Ascensions (Fandom)](https://gunfirereborn.fandom.com/wiki/Ascensions)
- [Gunfire Reborn — Commonly Asked Questions (Slyther Games)](https://www.slythergames.com/2020/11/02/gunfire-reborn-commonly-asked-questions/)
- [Hades — Mirror of Night (Fandom)](https://hades.fandom.com/wiki/Mirror_of_Night)
- [Hades — Pom of Power (Fandom)](https://hades.fandom.com/wiki/Pom_of_Power)
- [Pom Power: How Poms and Boon Levels work (Steam Guide)](https://steamcommunity.com/sharedfiles/filedetails/?id=2658113414)
- [How to Design a Roguelite Meta-Progression (Bugnet)](https://bugnet.io/blog/how-to-design-a-roguelite-meta-progression)
- [Stat-based meta-progression debate (ResetEra)](https://www.resetera.com/threads/im-starting-to-feel-that-stat-based-meta-progression-is-starting-to-ruin-roguelites-generally-speaking.1509337/page-2)
- [How moving from 2D to 3D shaped the design of Risk of Rain 2 (Game Developer)](https://www.gamedeveloper.com/design/how-moving-from-2d-to-3d-shaped-the-design-of-i-risk-of-rain-2-i-)
- [Vertical vs horizontal progression (Cyberly)](https://www.cyberly.org/en/how-does-vertical-progression-differ-from-horizontal-in-video-games/index.html)
