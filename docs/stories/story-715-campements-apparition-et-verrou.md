# Story 715 — Les campements : apparition à l'approche, et verrou de progression

**Statut** : DRAFT
**Niveau BMAD** : Standard
**Dépend de** : 712, 713

---

## La donnée existe déjà, personne ne la lit

`expedition_vallon.json` porte **trois campements entièrement spécifiés**, et
**aucun consommateur Rust** — exactement le motif des 16 braseros, résolu le
2026-08-14 :

```
camp_1 | rayon 12 m | ligne_max 24 m | vision 20 m | 7 apparitions | 4 abris
camp_2 | rayon 12 m | ligne_max 24 m | vision 20 m | 7 apparitions | 5 abris
camp_3 | rayon 12 m | ligne_max 24 m | vision 20 m | 7 apparitions | 6 abris
```

Chaque campement porte aussi `verrou_xyz` et `verrou_cap_rad` : **le blocage du
chemin est déjà autoré**, il attend son code.

## Ce que la story livre

1. **Apparition à l'approche**, pas au chargement. Peupler les 3 campements dès
   l'entrée dans la carte, c'est 21 ennemis qui pensent en permanence sur un
   chemin de 359 m. L'apparition se déclenche à l'entrée du rayon.
2. **Le verrou** : le chemin reste bloqué tant que le campement n'est pas nettoyé.
   C'est ce que vous aviez demandé — *« des campements d'ennemis sur la route
   qu'on devra tuer pour avancer »*.
3. **Moins d'ennemis que l'arène, mais chacun compte** (cf. le dosage ci-dessous).

## Le dosage — dérivé, pas choisi

**Mesures actuelles.** Joueur **100 PV**. Pépin **168 dps** (28 dmg × 6/s,
×2 à la tête = 336). Bourrasque **121 dps**. Ennemi **200 PV**, tir **12 dmg /
1,5 s = 8 dps**, portée 35 m.

```
temps pour tuer un ennemi      200 / 168 = 1,19 s   (0,60 s à la tête)
temps pour qu'IL vous tue      100 /   8 = 12,5 s
```

**Voilà le « trop facile » chiffré** : vous le tuez 10 fois plus vite qu'il ne
vous tue.

Densité arène de référence : **5 / 3 / 4 / 5 mobs**, puis 1 boss à `hp_mult 5`.

### Le piège à éviter

Pour porter un engagement à 8 s en n'augmentant que les PV, il faudrait
**1 344 PV** — soit **6,7×**. Ce n'est pas de la difficulté, c'est une éponge à
balles : le joueur ne joue pas mieux, il tient la gâchette plus longtemps.

### La proposition

**3 ennemis par campement** (contre 4-5 en arène), mais l'engagement dure
**8-12 s** parce que le joueur doit *faire* des choses, pas parce que la barre
est longue :

| levier | effet sur la durée | ce que ça demande au joueur |
|---|---|---|
| PV 200 → **420** | ×2,1 | rien — le socle, pas la difficulté |
| l'ennemi **utilise les abris** (4-6 par camp, déjà bâtis) | ×1,3-1,6 | se déplacer pour le déloger |
| **un porteur de totem** (story 716) réduit les dégâts subis par le groupe | ×1,4 tant qu'il vit | **choisir sa cible** |
| **une mêlée télégraphiée** (story 714) | — | esquiver, gérer sa distance |

Produit : ~8-11 s d'engagement pour **420 PV seulement**. La longueur vient des
décisions, pas du réservoir.

### Composition proposée, croissante

| | composition | ce qu'on enseigne |
|---|---|---|
| **camp_1** | 2 tireurs + 1 brute | la mêlée existe, et elle se télégraphie |
| **camp_2** | 2 tireurs + 1 porteur de totem | tuer dans le bon ordre |
| **camp_3** | 2 tireurs + 1 brute + 1 totem | les deux à la fois |

## Critères d'acceptation

- [ ] Les 3 campements se peuplent depuis `apparitions_xyz` du manifeste, jamais
      depuis des coordonnées écrites en Rust
- [ ] L'apparition se déclenche à l'approche, pas au chargement de la carte
- [ ] `verrou_xyz` bloque la progression, et se lève **à la mort du dernier**
- [ ] La composition vient d'un génome, pas du code
- [ ] **Aucun ennemi n'apparaît dans le décor** — `spawn-clearance.md` est bloquante :
      le spawn cherche une place libre, il ne pousse jamais le décor
- [ ] Un campement nettoyé **le reste** (pas de re-peuplement au retour)
- [ ] Le capteur de 712 expose l'état de chaque camp (`endormi` / `engagé` / `nettoyé`)

## Le défaut connu que cette story va révéler

`ligne_max_m = 24` contre `vision = 20` : **4 m de tir gratuit** — le joueur peut
tirer sur un ennemi qui ne peut ni voir ni répondre (`map-design-intention.md`
§2.2). C'est mesuré et documenté depuis la construction de la carte. Deux issues :
porter la vision des tireurs à 24 m, ou casser la ligne avec de l'occultation.
**À trancher manette en main.**

## Fichiers

- `crates/forgia-mode-expedition/src/campements.rs` (nouveau)
- `crates/forgia-mode-expedition/src/plugin.rs` (câblage)
- `assets/genomes/expedition_campements.toml` (nouveau — compositions)

## Risque

**Moyen.** Nouveau module isolé, mais dépend du spawn de 713.

## Cross-refs

- `.claude/rules/spawn-clearance.md` — **bloquante**
- `.claude/rules/map-design-intention.md` §2.1, §2.2, §2.4
- `[[reference_weapon_stats_real_source_viewmodel_arena]]`
