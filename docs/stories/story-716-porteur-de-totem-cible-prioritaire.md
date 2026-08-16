# Story 716 — Le porteur de totem : une cible prioritaire

**Statut** : DRAFT
**Niveau BMAD** : Standard
**Dépend de** : 713, 715

---

## L'idée en une phrase

Un ennemi qui **protège les autres**, et qu'il faut donc tuer en premier.

C'est la mécanique de trash pack la plus universelle de WoW, et la moins chère à
écrire : elle ne demande **aucune IA nouvelle**. Le porteur se comporte comme un
tireur ordinaire ; ce qui change, c'est ce qu'il fait aux *autres* tant qu'il vit.

## Pourquoi elle vaut plus que d'augmenter les PV

Vous avez demandé « plus compliqué et long à tuer ». Il y a deux façons
d'allonger un combat :

| | ce que le joueur vit |
|---|---|
| tripler les PV | il tient la gâchette plus longtemps |
| **une cible prioritaire** | il **regarde le groupe**, décide, et se déplace |

La seconde crée une **erreur possible** — tirer sur le mauvais ennemi coûte du
temps — donc un apprentissage, donc de la difficulté au sens propre. La première
ne crée rien : on ne peut pas mal jouer une éponge.

C'est aussi ce qui tient la consigne « **mécaniques simples** » : une règle, un
indice visuel, aucune barre de cast à lire.

## La mécanique

```
        porteur vivant                        porteur mort
   ┌──────────────────────┐            ┌──────────────────────┐
   │  alliés dans 10 m :  │            │  alliés dans 10 m :  │
   │  dégâts subis ×0,55  │  ────►     │  dégâts subis ×1,0   │
   │  un lien visible     │   il meurt │  le lien s'éteint    │
   └──────────────────────┘            └──────────────────────┘
```

**Le lien visible est la moitié de la mécanique.** Sans lui, le joueur ne
comprend pas pourquoi ses balles font moins d'effet — il conclut « éponge » et
la mécanique se retourne contre elle-même. Un trait lumineux du totem vers
chaque protégé, dans la teinte des braises déjà utilisée par l'Expédition.

**Il ne se soigne pas, il ne ressuscite pas.** Une seule règle.

## Critères d'acceptation

- [ ] Un porteur réduit les dégâts subis par ses alliés dans un rayon, tant qu'il vit
- [ ] Le facteur, le rayon et la portée viennent d'un génome
- [ ] **Un lien visuel** relie le porteur à chaque allié protégé, et disparaît à sa mort
- [ ] La réduction cesse **à la frame de sa mort**, pas au tick suivant
- [ ] Un porteur isolé (aucun allié à portée) ne protège que lui-même — et le
      capteur le dit, pour qu'un placement raté se voie
- [ ] Le porteur est **visuellement distinct** à 24 m, la ligne max d'un campement :
      une mécanique qu'on ne peut pas identifier de loin n'en est pas une
- [ ] Aucune allocation par frame dans la boucle de liens (`scalability.md`)

## Ce qu'il faudra trancher en jouant

- **0,55 est un point de départ, pas une mesure.** Il porte l'engagement de
  ~5 s à ~8 s sur trois ennemis à 420 PV. Trop bas, le joueur croit son arme
  cassée ; trop haut, la mécanique est ignorable.
- **Le rayon de 10 m** dans un campement de 12 m de rayon signifie « presque tout
  le camp ». À vérifier : c'est peut-être ce qu'on veut (une seule décision) ou
  trop généreux (le porteur devrait avoir à se placer).

## Extensions volontairement écartées

Interruption de sort, soin, invocation, marquage au sol, enrage. Toutes crédibles,
toutes plus coûteuses, et **aucune n'est nécessaire pour que le combat cesse
d'être un stand de tir**. À rouvrir après playtest, si la file de campements
devient monotone.

## Fichiers

- `crates/forgia-combat/` (l'aura et son application)
- `crates/forgia-effects/` (le lien visuel)
- `assets/genomes/enemies/enemy_totem.toml` (nouveau)

## Risque

**Bas-moyen.** Additif : sans porteur dans une composition, rien ne change.

## Cross-refs

- `.claude/rules/creator-simplicity.md` — une règle compréhensible en 3 secondes
- `[[reference_two_health_types_combat_vs_damage]]` — les ennemis portent
  `forgia_combat::Health`, mutation directe ; ne pas passer par `DamageEvent`
