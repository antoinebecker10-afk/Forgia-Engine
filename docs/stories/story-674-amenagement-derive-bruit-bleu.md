# story-674 — L'aménagement se DÉRIVE : bruit bleu + compte depuis l'aire

**Statut** : DONE (2026-08-12 — runtime validé, 9/9 AC)
**Date** : 2026-07-31
**Niveau BMAD** : Standard
**Related** : story-672 (dégagement des spawns), story-673 (registre d'assets mesuré)

---

## Le problème, en une phrase

Le décor d'une arène roguelite était **89 props posés à des angles aléatoires** —
un compte littéral qui ne savait rien de la taille de la salle, et un tirage
polaire qui produit des **amas et des clairières**.

C'est ce qui se lisait en jeu comme *« les salles sont assez vides »* : le compte
n'était pas absurde, c'est la **répartition** qui laissait de grandes zones nues
pendant que d'autres s'entassaient.

Deux défauts distincts, tous les deux de la même famille que
`feedback_derive_ne_patche_pas_la_geometrie` :

| | Avant | Pourquoi c'est faux |
|---|---|---|
| **Le compte** | `decor_perimeter_count = 34`, `decor_scatter_count = 55` | Un littéral ne peut pas savoir si l'anneau fait 15 m ou 32 m de large. La même valeur donne une salle dense et une salle nue. |
| **Les positions** | angle aléatoire + rayon aléatoire | Le tirage uniforme **n'a aucune notion de distance minimale**. Les amas sont la norme, pas l'exception. |

---

## Ce qui est livré

### 1. `forgia-core::layout` — les primitives partagées

Deux fonctions qui existaient déjà, mais au mauvais endroit et sans consommateur
utile :

- **`poisson_disk_sample`** (Bridson) vivait dans `forgia-terrain::sampling`, pour
  la végétation. L'aménagement des arènes en avait besoin — et une crate gameplay
  n'a pas à dépendre du terrain pour des maths pures. Déplacée, **chemin d'origine
  conservé par re-export** : la végétation et ses 5 proptests n'ont pas bougé.
- **`covers_expected(aire, espacement)`** vivait dans le banc Arena Test, où elle
  **mesurait sans jamais s'appliquer**. C'est le cas d'école du capteur qui
  constate un défaut que rien ne corrige.

Ajouté : `poisson_disk_annulus` (les arènes sont des disques, pas des rectangles)
et `disc_area`.

### 2. Le compte se dérive

`covers_expected(aire_de_l_anneau, espacement)` remplace les deux littéraux, qui
sont **supprimés du génome** — pas laissés en place à ne rien piloter.

### 3. Le plafond de budget le DIT quand il mord

`decor_max_props = 420` est un garde-fou de **budget de frame**, pas de goût. Deux
choix explicites :

- quand il mord, un `warn!` nomme le nombre de props **non posés** — jamais de
  troncature silencieuse (`map-design-patterns.md` §13) ;
- il **sous-échantillonne à pas régulier**, il ne tronque pas. L'ordre d'insertion
  de Bridson est une frontière qui progresse : prendre le préfixe remplirait un
  secteur et laisserait le reste nu. Un test vérifie que les 4 quadrants restent
  habités.

---

## Mesuré (test `the_delivered_defaults_are_measured_not_claimed`)

| | Avant | Après | |
|---|---|---|---|
| Périmètre (solides) | 34 | **94** | dérivé, non plafonné |
| Semis (au sol) | 55 | **326** | dérivé à 633, **plafonné** |
| **Total** | **89** | **420** | ×4,7 |

Le plafond mord sur le semis : la dérivation demande 633 props, le budget en
autorise 326. **307 props ne sont pas posés**, et le log le dit à chaque
génération. C'est un arbitrage assumé, pas un défaut caché.

Coût d'instanciation : 420 props à `decor_spawn_budget_per_frame = 12` →
**35 frames**, ~0,6 s à 60 fps, étalées par la file existante.

---

## Acceptance criteria

- [x] `covers_expected` a un consommateur qui **agit**, pas seulement qui mesure
- [x] Aucune position de prop ne vient plus d'un tirage polaire uniforme
- [x] Les deux gènes de compte littéral sont **supprimés** (pas laissés morts)
- [x] Le plafond est observable quand il mord (`warn!` avec le compte manquant)
- [x] Test multi-graines de l'espacement minimal (5 graines)
- [x] Test que le plafond garde la répartition (4 quadrants)
- [x] Test que le compte suit la taille de la salle
- [x] Le chemin public de `poisson_disk_sample` est conservé pour la végétation
- [x] **Validé en jeu** — 2026-08-12. Preuve dans `forgia2_run.log` : la dérivation
      tourne bien à partir de l'aire, pas d'un compte écrit à la main —
      `[decor] semis : 404 positions dérivées à 5.0 m (aire/espacement² = 633)`,
      puis `[decor] planned 564 GLB props` sur `forge_sanctum`. Jugement de
      répartition rendu manette en main (« ça semble bon »).
      ⚠️ **Réserve consignée, hors scope de cette story** : le plafond mord —
      `decor_max_props = 330 → 74 props NON posés` (le message le dit
      « volontairement, budget de frame »). La dérivation est correcte ; c'est le
      plafond qui décide du résultat final.

---

## Ce que ça ne règle PAS, et qu'il faut dire

- **Le couvert est toujours dans l'anneau extérieur.** Les props solides sont sur
  `ring_radius 42→74 m`. L'aire de combat centrale (< 42 m) ne reçoit que du semis
  **sans collider**. La salle est plus dense, mais le joueur n'a toujours pas
  d'abri là où il se bat. C'est un défaut de **composition**, pas de densité —
  levier 2 (composer par RÔLE mesuré), non fait.
- **Rien ne compose par rôle.** Le registre mesure `role_from_height`
  (traversée / inutile / casse-vue / repère) depuis story-673, mais le générateur
  tire toujours dans des pools par NOM de pool, pas par rôle. C'est ce qui a
  produit « 16 blocs `cover` étaient des murs de 3-6 m ».
- **Aucun capteur d'aménagement.** On ne saura pas en jeu quelle fraction de
  l'aire est réellement occupée, ni le profil de portées — levier 4, non fait.

---

## Cross-refs

- `.claude/rules/map-design-patterns.md` §11 (espacement 3-10 m), §13 (pas de
  capteur aveugle, pas de plafond muet)
- `feedback_derive_ne_patche_pas_la_geometrie` — dériver au lieu de déclarer 2×
- story-673 — le registre d'assets mesuré, dont le rôle attend un consommateur
