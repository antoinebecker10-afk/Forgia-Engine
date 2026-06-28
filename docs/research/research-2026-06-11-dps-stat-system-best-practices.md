# Research — Best practices : système de calcul/affichage de DPS et stats modifiables

> Recherche directe 2026-06-11 (suite des deux rapports FTUE du même jour). 5 WebSearch + 3 WebFetch vérifiés.
> Contexte : afficher un « DPS effectif » par arme dans le Roguelite (carte d'arme, delta boon, avant/après Enclume) pour rendre la progression de puissance lisible.

---

## TL;DR

1. **Architecture standard** : base value + liste de modificateurs typés (flat / pourcentage), application flat d'abord puis multiplicatifs, **lazy recompute sur changement** (jamais par frame) — maps 1:1 sur `Changed<T>` Bevy.
2. **Formule à afficher** : le **sustained DPS** (`dégâts_chargeur / (temps_pour_vider + reload)`) est le seul chiffre honnête pour comparer des armes aux profils différents — le burst DPS surévalue de 20-50 % les armes à petit chargeur/long reload.
3. **Le piège canonique** : le tooltip DPS de Path of Exile — un chiffre « précis » mais conditionnel auquel les joueurs ne font plus confiance (exode vers Path of Building). Un chiffre simple, stable et **calculé par le même code que le dégât réel** vaut mieux qu'un chiffre exhaustif qui fluctue.
4. **Ce que le DPS n'inclut pas, par design** : élément/matchup, crits, recul, précision, distance, DoT — à afficher À CÔTÉ (icônes), pas plié dans le nombre.

---

## 1. Architecture du système de stats (pattern vérifié)

Source principale : [RefresherTowel Games — How to Comfortably Deal with Modifiable Stats](https://refreshertowelgames.wordpress.com/2024/02/17/how-to-comfortably-deal-with-modifiable-stats/) (fetch vérifié).

- **Structure 3 niveaux** : `base_value` + tableau `modifiers[]` + `current_value` recalculée.
- **Ordre d'application** : additifs (flat) d'abord, puis multiplicatifs : `_value += mod.value` puis `_value *= 1 + mod.value`.
- **Cycle de vie propre** : `AddModifier()` / `RemoveModifierById()` — chaque buff temporaire est retirable par référence (time source globale pour l'expiration).
- **Lazy evaluation vérifiée** : « If we haven't added any modifiers since the value was last retrieved, we can simply return the current value... Otherwise, we have to recalculate » — flag `altered`, zéro recalcul par frame. **Équivalent Bevy : recalcul sur event (boon ramassé, achat Enclume, changement d'arme), jamais dans Update.**
- Distinction additif/multiplicatif ([NeoGAF discussion](https://www.neogaf.com/threads/gaf-i-need-your-help-understanding-multiplicative-vs-additive.1262391/)) : 50%+50% additif = +100 % ; multiplicatif = +125 % — le multiplicatif entre catégories est la source classique de power creep exponentiel. Pratique courante (confiance moyenne, pas de source canonique unique) : **additif à l'intérieur d'une catégorie, multiplicatif entre catégories** (méta × boons × gimmick) — borne la croissance par catégorie.
- Couches de puissance ([Achterman, The Craft of Game Systems, Game Developer](https://www.gamedeveloper.com/design/the-craft-of-game-systems-practical-examples), fetch vérifié) : « This base power level would later be increased by additional factors like skills, powers, and random loot modifiers » — base curve (arme/genome) × couches (méta, boons).
- **Psychologie d'affichage** (Achterman, vérifié) : « a weapon [...] one or two points better than his previous item just didn't feel very good » — calibrer l'échelle des nombres pour que les upgrades se *sentent* (deltas à 2 chiffres).

## 2. Quelle formule afficher

Sources : [Kordu DPS calculator](https://kordu.tools/tools/gaming/dps-calculator/), [axiscalc](https://axiscalc.com/damage-per-second-calculator/), calculateurs Warframe (formules convergentes).

- **Burst DPS** = dégâts par tir × tirs/seconde (ignore le rechargement).
- **Sustained DPS** = dégâts du chargeur / (temps pour vider le chargeur + temps de reload).
- Écart typique : **sustained 20-50 % plus bas** ; l'écart grandit avec un petit chargeur et un long reload → le burst DPS *ment* précisément sur les armes que le Roguelite veut différencier (Lenoir one-shot vs SMG spam).
- **Reco : sustained DPS comme chiffre principal** — il intègre dégâts + cadence + chargeur + reload en un seul nombre comparatif, arrondi à l'entier.

## 3. Les deux cas d'école du chiffre qui ment

- **Path of Exile tooltip DPS** ([forum officiel](https://www.pathofexile.com/forum/view-thread/2242989/page/2), [nerdburglars](https://nerdburglars.net/question/how-accurate-are-dps-numbers-from-path-of-building-compared-to-actual-gameplay/)) : le tooltip in-game est si peu fiable que la communauté entière utilise un outil externe (Path of Building). Même PoB affiche un « instantaneous peak DPS under perfect conditions » (buffs conditionnels supposés actifs à 100 %). Leçons : (a) **ne jamais inclure de bonus conditionnels** dans le chiffre affiché ; (b) le tooltip reste utile pour les **changements relatifs** (« upgrading gear affects your DPS in a general sense ») — c'est exactement notre cas d'usage (suivre sa progression).
- **Borderlands** ([wiki DPS via recherche](https://borderlands.fandom.com/wiki/DPS), fandom 403) : le DPS « eliminates factors like enemy size and distance, weapon recoil, accuracy, critical hit zones, elemental effectiveness, and damage over time » — liste exacte de ce qu'il faut afficher À CÔTÉ et non dans le nombre. BL4 a caché des stats dans des sous-menus → grogne communautaire ([TheGamer](https://www.thegamer.com/borderlands-4-weapon-hidden-stats-in-one-place/)) : les infos de comparaison doivent être visibles au moment du choix, pas enfouies.

## 4. Spécification recommandée pour Forgia

1. **`effective_dps()` = fonction pure dans `forgia-combat`**, qui compose les MÊMES facteurs dans le MÊME ordre que la chaîne de dégât réelle (`effective_dmg` : base genome × PermanentPlayerMods × boons × gimmick). Single source of truth — l'anti-PoE.
2. **Formule affichée** : sustained DPS neutre (sans élément, sans crit, cible standard), arrondi entier. Élément = icône à côté.
3. **Recalcul sur événement** (boon, achat, équipement, hot-reload genome), résultat caché dans un Component/Resource — zéro travail par frame (hot path : rien dans Update sans `Changed`).
4. **Modificateurs typés et retirables** : catégories méta / boons / gimmick, additif intra-catégorie, multiplicatif inter-catégories (borne le power creep et rend la décomposition lisible).
5. **Sensor `forgia2_power.json`** : par arme — base, somme flat, % par catégorie, multiplicateurs, DPS final + décomposition. Permet de vérifier affiché == réel.
6. **Golden tests** : tests unitaires verrouillant la formule + un test d'égalité « DPS affiché == dégâts effectivement appliqués par la chaîne hitscan sur cible neutre pendant 1 cycle chargeur+reload » (anti-divergence UI/sim).
7. **Échelle des nombres** : calibrer pour que chaque boon/achat produise un delta visible ≥ 2 chiffres (psychologie Achterman) — ajustable genome.

## Limites

- Pas de source AAA primaire sur l'architecture interne d'un calcul de DPS affiché (les studios ne publient pas ça) ; le pattern stats vient d'un tutoriel de qualité (RefresherTowel) corroboré par la pratique standard (même structure que les systèmes Unity/Godot populaires).
- « Additif intra-catégorie, multiplicatif inter-catégories » = pratique répandue mais sans source canonique unique — confiance moyenne, à valider contre la chaîne `effective_dmg` existante.
- Borderlands wiki et gamesfuze inaccessibles en fetch (403) — claims issus des snippets de recherche.
