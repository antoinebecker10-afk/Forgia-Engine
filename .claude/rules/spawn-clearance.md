# Dégagement des points d'apparition (Forgia) — RÈGLE BLOQUANTE

> **Personne n'apparaît jamais dans un décor.** Ni le joueur, ni un ennemi, ni un
> PNJ, ni un objet ramassable.
>
> Origine : 2026-07-31, rapporté en jeu — « je spawn dans un asset et parfois les
> mobs sont bloqués dans le décor ». Diagnostic : les anneaux d'apparition ennemis
> (12 / 25 / 50 m) tombaient en plein dans les anneaux de décor (semis 12→72 m,
> périmètre 42→74 m). Personne n'avait écrit que ces deux familles de rayons
> devaient s'éviter, donc elles ne s'évitaient pas.

---

## 1. Pourquoi c'est bloquant, et pas cosmétique

Un prop mal placé ne produit pas un défaut visuel, il produit un défaut de
**jeu** :

- **Le joueur qui apparaît dans un rocher** est dans un état dont il ne peut pas
  sortir par le jeu normal : la caméra est dans la géométrie, il ne voit pas la
  salle, et le contrôleur peut le pousser n'importe où en se dépénétrant.
- **L'ennemi né contre un pilier ne se débloque jamais.** Les bots de Forgia
  n'ont **pas de navmesh** : ils avancent vers leur cible en ligne droite
  (`bot_tactical_movement`). Face à un collider, ils poussent dedans
  indéfiniment. Ce n'est pas un ralentissement, c'est un ennemi retiré du combat
  — et une vague qui ne se nettoie jamais si le joueur ne va pas le chercher.
- Le symptôme est **intermittent** (« parfois ») parce que le placement est
  seedé : il passe les tests manuels et sort en playtest.

---

## 2. L'invariant, et le SENS dans lequel on le tient

> **Personne n'apparaît dans un solide.**

La question n'est pas *quoi* garantir, c'est **qui cède**. Et la réponse n'est pas
la même selon l'acteur :

| Acteur | Qui cède | Pourquoi |
|---|---|---|
| **Joueur** | **le décor** — petit disque interdit autour de sa position réelle | Sa position est imposée (spawn du contrôleur). On dégage juste ce qu'il faut pour qu'il ouvre les yeux au clair. |
| **Ennemis** | **le spawn** — il cherche une place libre dans le décor | Leur position est LIBRE sur un anneau. C'est à eux de s'adapter, pas au décor de s'effacer. |

### La leçon payée cher

La première version réservait aussi les **anneaux d'apparition ennemis** au décor.
Mesuré : cela interdisait **54 % du rayon utile** aux props solides. Résultat en
jeu — *« les salles sont assez vides »* — et le bug n'était **même pas corrigé**,
parce que l'emprise des props était sous-estimée par ailleurs.

> **Un invariant qui vide la scène n'est pas un invariant, c'est une régression.**
> Avant de réserver une zone, MESURER la fraction de l'espace qu'elle retire. Si
> c'est plus de quelques pour cent, c'est le mauvais acteur qui cède.

---

## 3. Le décor d'abord, les spawns ensuite

L'ordre correct :

1. **Le décor se pose**, dense et cohérent, sans se soucier des ennemis.
2. **Il publie ses emprises solides** (centre, rayon) — au moment du **plan**, pas
   après instanciation : les props sont instanciés étalés sur plusieurs frames
   alors qu'une vague apparaît d'un coup. Interroger les entités déjà spawnées
   donnerait une liste incomplète et un résultat dépendant du timing.
3. **Chaque spawn cherche une place libre** dedans : on balaie l'anneau par pas
   réguliers, on prend le premier point libre, et **si tout est encombré on prend
   le plus dégagé** — jamais rien, jamais le point voulu par défaut.

```rust
// ✅ le spawn s'adapte au décor
let angle = obstacles.clear_angle_on_ring(radius, wanted, body_radius, TRIES);

// ❌ le décor s'interdit la moitié de l'arène
if keepout.blocks_ring(pos) { reject_prop(); }
```

Corollaire : le filtre côté décor reste **en sortie de planification**, une seule
fois — le décor est produit par une demi-douzaine de générateurs (périmètre,
semis, gravats, salles en L, anneau de forge, bâtiments), et corriger « le rayon
du périmètre » ne corrigerait que celui-là.

---

## 4. L'emprise, c'est l'emprise — pas le rayon du collider

Le défaut le plus vicieux de la première version : le rayon d'emprise était
calculé en multipliant la taille cible par `col_radius_factor`, un coefficient qui
**rétrécit le collider** pour le feel de tir. Un bâtiment de 12 m se déclarait
**1,92 m** de rayon. Trois fois trop petit : il passait tous les tests, et le mob
naissait dedans.

> **Une valeur de tuning n'est pas une mesure.** L'emprise au sol se dérive de la
> taille de l'objet, pas d'un coefficient de gameplay qui se trouve être à portée
> de main.

Et l'emprise doit être **généreuse** : un prop rejeté à tort coûte un trou dans le
décor, un prop accepté à tort coûte un joueur qui apparaît dedans. Les deux
erreurs n'ont pas le même prix.

---

## 4 bis. Les positions se LISENT, elles ne se supposent pas

La position du joueur **n'est pas l'origine** : il est spawné par une autre crate.
La supposer protégeait le mauvais endroit — c'est exactement le
« j'ai respawn en plein sur un asset » rapporté.

```rust
// ✅ on lit où il est vraiment
let p = q_player.iter().next().map(|t| t.translation.xz())...

// ❌ on suppose
SpawnKeepout { player: (Vec2::ZERO, r) }
```

Même principe pour les rayons d'anneaux : les **lire** sur leur config
(`WaveCompConfig.ring`), jamais les recopier. Si un miroir est inévitable (crates
séparées), il faut un **test qui compare les deux ensembles**.

---

## 5. Ce qui n'est PAS couvert, et qu'il faut dire

Cette règle traite l'apparition. Elle **ne traite pas** le blocage en cours de
déplacement :

- un mob qui poursuit le joueur et se coince contre un pilier **en chemin** ;
- un mob poussé dans le décor par un knockback ;
- un joueur qui se coince en sautant entre deux props.

Ces cas relèvent du **pathfinding** (absent : pas de navmesh) et d'un
**chien de garde de désenlisement** (un bot qui n'a pas bougé de plus de X m en
Y secondes alors qu'il est en poursuite est repositionné). Tant que ces deux
pièces n'existent pas, **ne pas prétendre que « ça n'arrive jamais »** — dire que
l'apparition est garantie et que le reste est ouvert.

---

## 6. Checklist avant de livrer un système qui place des objets

- [ ] **Qui cède ?** Celui dont la position est LIBRE s'adapte ; celui dont la
      position est imposée fait céder l'autre. Se tromper de sens vide la scène.
- [ ] **Quelle fraction de l'espace la zone réservée retire-t-elle ?** Le
      **mesurer**, pas l'estimer. Plus de quelques pour cent = mauvais sens.
- [ ] Le **rayon de l'objet** entre-t-il dans le test, ou seulement son centre ?
- [ ] Ce rayon est-il une **emprise mesurée**, ou un coefficient de tuning attrapé
      au passage ?
- [ ] Les positions et rayons sont-ils **lus** depuis leur source, ou supposés /
      recopiés ? Si recopiés : y a-t-il un test qui compare ?
- [ ] Les emprises sont-elles publiées **au plan**, ou après instanciation ? Un
      décor étalé sur N frames n'est pas interrogeable par un spawn instantané.
- [ ] La recherche de place libre a-t-elle un **repli déterministe** (« le moins
      mauvais ») ? Elle ne doit jamais rendre « rien », ni le point voulu par défaut.
- [ ] Y a-t-il un **test multi-graines** ? Un placement seedé qui passe une fois
      ne prouve rien.
- [ ] Y a-t-il un test que la scène **n'est pas vidée** ? Un invariant qui
      supprime tout est un autre bug — et c'est celui qu'on a livré au 1er essai.
- [ ] Le rejet est-il **observable** (log/compteur) ? Sinon une zone trop large
      vide la salle en silence.

---

## 7. Cross-refs

- `on-demand/map-design-intention.md` §2.4 — les arrivées ennemies « ni dans le dos au
  contact, ni hors de vue, **atteignables** »
- `on-demand/map-design-patterns.md` §1 — le personnage est un **disque** : les
  dégagements se mesurent en distance, jamais en AABB gonflée
- `feedback_derive_ne_patche_pas_la_geometrie` — traiter la classe, pas le symptôme
- `observability-required.md` — le nombre de props rejetés doit se voir

---

*Adoptée 2026-07-31 (story-672). Implémentation : `SpawnKeepout` dans
`crates/forgia-mode-roguelite/src/decor.rs`, génome
`roguelite_decor.toml` → `decor_keepout_player_m` / `decor_keepout_spawn_margin_m`.*
