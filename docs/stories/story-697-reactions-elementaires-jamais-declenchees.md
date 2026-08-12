# story-697 — Les éléments s'appliquent, mais ne réagissent JAMAIS

**Statut** : DRAFT — diagnostic FAIT 2026-08-12, cause tranchée ; la suite est une décision de design, pas un correctif
**Créée** : 2026-08-12
**Niveau BMAD** : Standard (moteur de réactions + VFX + audio, ≥ 2 crates)
**Origine** : run de validation du 2026-08-12, lecture des capteurs `elements` et
`element_vfx`.
**Bloque** : [story-611](story-611-reaction-combustion.md) (Combustion), et par
extension tout le pilier « réactions élémentaires ».

---

## Symptôme, mesuré

Sur une run complète, `forgia2_elements.json` :

| Ce qui marche | Ce qui ne marche pas |
|---|---|
| `burns_applied: 176` | `reactions.combustions: **0**` |
| `shocks_applied: 102` | `reactions.miasmas: **0**` |
| `poisons_applied: 1` | `reactions.surcharges: **0**` |
| `hits: {fire:192, shock:133, armor_pierce:13, poison:1}` | |
| `elem_absorbed: 3503` · `aoe_hits: 23` · `executes: 3` | |

Corroboré par `forgia2_element_vfx.json` : `sparks_spawned: 339`,
`dot_pulses: 66`, mais **`reaction_bursts: 0`**.

**Les quatre éléments sont débloqués** (`unlocked_count: 4`) et **s'appliquent
massivement**. C'est l'étage au-dessus — la table de réactions — qui ne produit
jamais rien.

## Pourquoi c'est grave au-delà du bug

Le GDD *The Spared* fait des **réactions inter-acteurs le cœur du coop** : *« l'un
applique, l'autre détone »*, et les ultimes y sont décrits comme « les applicateurs
d'élément de masse — le moment de spectacle ». Un moteur de réactions qui ne
déclenche jamais vide cette promesse.

C'est aussi la deuxième moitié d'un constat déjà connu : `forgia2_power.json`
donne `boon_damage_count: 0` (courbe de dégâts plate). Deux systèmes de montée en
puissance inertes en même temps.

## DIAGNOSTIC — 2026-08-12. Deux causes indépendantes, aucune n'est un bug du moteur

**Le moteur de réactions est correct.** `ReactionTable::triggered` est testée
(`reaction_table_all_statuses_*`), `had_burn`/`had_poison`/`had_shock` lisent bien
les statuts **pré-hit** (`elements.rs:1188-1194`), les durées tiennent (burn 3 s,
shock 4 s, poison 4 s) et `[surcharge] enabled = true` dans le génome. Rien de tout
cela n'est en cause.

Les trois paires exigent chacune **deux éléments co-présents sur la même cible** :

| Réaction | Paire | Armes porteuses (nom en jeu) |
|---|---|---|
| Combustion | Feu + **Poison** | **Bourrasque** + **Boucherie** |
| Miasma | Choc + **Poison** | **Pépin** + **Boucherie** |
| **Surcharge** | Feu + Choc | **Bourrasque + Pépin** — les deux premières touches |

> ⚠️ **Trois vocabulaires pour les mêmes quatre armes**, et aucun ne ressemble aux
> autres. C'est le piège de lecture le plus coûteux du sujet :
>
> | Nom en jeu | Enum Rust | Clé du capteur `elements.mapping` | Touche |
> |---|---|---|---|
> | **Pépin** (revolver) | `WeaponType::ModernAR` | `pistol` | 1 |
> | **Bourrasque** (SMG) | `WeaponType::AssaultRifle` | `smg` | 2 |
> | **Madame Lenoir** (sniper) | `WeaponType::Shotgun` | `sniper` | 3 |
> | **Boucherie** (lance-roquettes) | `WeaponType::RocketLauncher` | `pompe` | 4 |
>
> Deux clés sont carrément trompeuses : `WeaponType::Shotgun` **est le sniper**, et
> `pompe` (= fusil à pompe) **est le lance-roquettes**. Toujours vérifier le mapping
> dans `roguelite_elements.toml` avant de raisonner sur une arme par son nom.
> Source de vérité des stats : `viewmodel_arena.toml`, jamais `roguelite_weapons.toml`.

### Fausse piste écartée — le garde `ev.damage > 0.0`

> Consignée parce qu'elle était **convaincante et fausse**, et que la prochaine
> lecture la retrouvera.
>
> Le génome porte `[weapons.boucherie] damage = 0.0` (*« projectile : dégâts portés
> par l'explosion AOE »*), et `elements.rs:1285` garde tout sur `ev.damage > 0.0`
> avec un commentaire qui cite ce cas. Tout concordait : la Boucherie est **l'unique
> source de poison** (`roguelite_elements.toml:26`), donc le garde semblait la
> débrancher et tuer deux réactions sur trois.
>
> **Vérifié dans le code de l'explosion, c'est faux** : `boucherie_rocket.rs:290`
> émet un `CombatHitEvent { damage: EXPLOSION_DAMAGE, weapon: Some(RocketLauncher) }`.
> Le poison a donc bien un chemin. Lire la ligne au lieu de raisonner sur le
> commentaire aurait évité l'aller-retour.
>
> Si `poisons_applied: 1`, c'est simplement que **la Boucherie n'a tiré que 3 fois
> en 400 s** (log : 3 × `[boucherie] BOUM ! roquette → 1 touchés`).

### La cause, unique — une arme = un élément, et l'ennemi meurt avant le changement

Surcharge (Feu + Choc) est le cas qui tranche : **192 hits feu, 133 hits choc**,
aucun problème d'application, et pourtant **zéro réaction**. Il faut toucher **la
même cible** avec deux armes différentes en moins de 3-4 s. Or le TTK est de
**0,18 s** sur un grunt (30 pv / 168 dps) et le log montre **3 changements d'arme
sur 400 s**.

Les trois réactions échouent donc pour la **même** raison, et le poison rare n'en
est qu'un symptôme aggravant.

Ce n'est pas un défaut d'implémentation : c'est ce que le GDD prévoit.

> *« Chaque acteur porte **un** élément à la fois. Les grosses réactions exigent
> deux poseurs : l'un applique, l'autre détone. **En solo, le compagnon est le
> second élément.** »* — GDD §4

## Conséquence sur l'ordre des phases — à trancher, c'est une décision de design

`REFONTE_GDD.md` place ce ticket en **Phase 0**, avec pour jalon *« une réaction se
déclenche, se voit et s'entend »*. Or **rien dans `elements.rs` ne peut satisfaire
ce jalon** : le code y est juste. Le remède nommé par le GDD est le **compagnon
comme second poseur** — c'est-à-dire la **Phase 1**.

**Phase 0 dépend donc de Phase 1**, ce que le document ne prévoit pas.

Trois issues possibles, toutes des décisions de game design, aucune n'est un
correctif :

1. **Déplacer ce ticket en Phase 1** et laisser Phase 0 à 696/698 (hitstop, kill).
   Le plus honnête : on ne prétend pas réparer ce qui n'est pas cassé.
2. **Rendre les réactions atteignables en solo mono-arme** — par exemple une arme
   qui pose deux éléments, ou un élément qui persiste au changement d'arme. C'est
   un changement de design, contraire au *« chaque acteur porte un élément »* du GDD.
3. **Un ennemi assez robuste pour survivre à un changement d'arme.** Le TTK de
   0,18 s sur un grunt est le vrai verrou ; un élite (120 pv, 0,71 s) laisse déjà
   plus de marge, un boss encore plus. Le jalon pourrait se mesurer **sur un boss**
   plutôt qu'en combat courant.

**Recommandation** : l'option 3 pour le jalon (elle ne change aucun design et se
teste tout de suite), puis l'option 1 pour la suite du ticket.

### Le protocole de run qui prouve ou infirme, en une fois

1. **Rebuild** (`cargo run -p forgia`) — sans ça, les capteurs décrivent l'ancien code.
2. Aller sur un **boss**, ou au minimum un **élite** (120 pv → TTK 0,71 s, contre
   0,18 s sur un grunt : c'est ce délai qui rend le second élément possible).
3. **Toucher LA MÊME cible avec les deux premières touches** :
   **Pépin (1, choc)** puis **Bourrasque (2, feu)** — c'est la paire **Surcharge**.
   L'ordre n'importe pas ; ce qui compte est que le second tir arrive **avant que le
   premier statut expire** (choc 4 s, brûlure 3 s) et **avant que la cible meure**.
4. Retour au menu, puis `python tools/ai/phase0_check.py`.

**Ce qu'on apprend dans les deux cas** — c'est ce qui rend ce test utile :

- `reactions.surcharges > 0` → le moteur est **innocenté en jeu**, et la cause est
  bien « le combat courant ne produit jamais la situation ». Phase 0 franchie sur
  cette base, la suite passe en Phase 1 avec le compagnon.
- Toujours **0** avec deux éléments posés sur une cible vivante → le diagnostic
  ci-dessus est **faux** et il faut rouvrir le moteur. Ce serait une surprise :
  `ReactionTable` est testée. Mais c'est exactement pour ça qu'on mesure.

## Ce qu'il ne faut PAS faire

- **Ne pas toucher `elements.rs`.** `ReactionTable` est testée, `had_*` lit les
  statuts pré-hit, les durées tiennent, `[surcharge] enabled = true`. Un correctif
  ici chercherait un bug qui n'existe pas.
- **Ne pas retoucher les dégâts des DoT** : le `next_step` du capteur le dit déjà.
- **Ne pas supprimer le garde `ev.damage > 0.0`** : la fausse piste ci-dessus a
  montré qu'il ne bloque rien, et il protège d'un vrai défaut.

## Pistes initiales, désormais tranchées

1. **Un seul élément par acteur à la fois ?** Le GDD dit qu'une grosse réaction
   « exige deux poseurs ». Si un ennemi ne peut porter qu'un statut, la condition
   à deux n'est jamais réunie en solo. Vérifier `active_burns` /`active_shocks` :
   ils sont à **0** au moment de l'écriture, ce qui n'exclut ni l'un ni l'autre.
2. **Le mapping arme→élément est-il trop uniforme ?** `pistol:shock, smg:fire,
   sniper:armor_pierce, pompe:poison` — avec une seule arme en main la plupart du
   temps, deux éléments distincts coexistent-ils jamais sur la même cible ?
3. **La `ReactionTable` est-elle consommée ?** Chercher son point d'appel réel,
   pas sa définition. **Grep le CONCEPT** (« reaction, combustion, detonate »),
   pas le nom du type — leçon de story-626 ce même jour.

## Critères d'acceptation

- [x] La cause est **nommée et prouvée** par une mesure — une arme = un élément,
      TTK 0,18 s, 3 changements d'arme en 400 s. Le moteur, lui, est juste.
      Une fausse piste (le garde `ev.damage > 0`) est consignée comme telle.
- [ ] `reactions.combustions` > 0 après une run où feu et choc sont posés
- [ ] `element_vfx.reaction_bursts` > 0 en même temps (le visuel suit la logique)
- [ ] Un test couvre la condition de déclenchement, pas seulement l'application
- [ ] story-611 peut être validée ou infirmée sur pièces

## Cross-refs

- `reference_elemental_reaction_engine_and_shock` (mémoire) — ReactionTable/Shock/Miasma
- GDD *The Spared* §4 « Réactions inter-acteurs (le cœur coop) »
- `.claude/rules/no-speculative-fix.md` — trois pistes, aucune n'est encore une cause
