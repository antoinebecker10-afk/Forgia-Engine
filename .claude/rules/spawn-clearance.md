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

## 2. L'invariant

> **Aucun élément de décor ne doit atterrir dans une zone d'apparition.**

Deux régimes, et la distinction n'est pas un détail :

| Zone | Ce qui est refusé | Pourquoi |
|---|---|---|
| **Disque d'apparition du joueur** | **TOUT** — collider ou non | C'est là qu'il ouvre les yeux. Apparaître le nez dans un tonneau traversable est exactement le symptôme rapporté. |
| **Anneaux / points d'apparition ennemis** | **le SOLIDE seulement** | Un mob ne se coince que dans un collider. Interdire aussi le semis décoratif condamnerait ~40 % de l'aire jouable et viderait la salle. |

Le test d'intersection est **disque contre disque** : `distance < rayon_zone +
rayon_prop`. Le rayon du prop compte — un gros prop tangent à un anneau bloque
quand même. C'est précisément le cas qui produisait « mob né dans le décor ».

---

## 3. Où l'appliquer : EN SORTIE, jamais dans chaque générateur

**Le filtre se pose une fois, sur la liste finale de props.**

C'est le point le plus important de cette règle. Le décor de Forgia est produit
par une demi-douzaine de générateurs (périmètre, semis, gravats, salles en L,
anneau de forge, ceinture de bâtiments, silhouettes de fond), chacun avec ses
propres rayons. Corriger « le rayon du périmètre » ne corrige que le périmètre :
au générateur suivant, le défaut revient sous un autre nom.

```rust
// ✅ un seul point de vérité — tout générateur futur en hérite sans rien faire
specs.retain(|s| s.is_background() || !keepout.blocks(s.pos(), s.radius(), s.is_solid()));

// ❌ N corrections de rayons qui divergeront
if ring_radius_min < enemy_ring { ring_radius_min = enemy_ring + 4.0; }
```

C'est l'application directe de `feedback_derive_ne_patche_pas_la_geometrie` : au
3ᵉ défaut semblable, on traite **la classe**, pas le symptôme.

---

## 4. Les zones se LISENT, elles ne se recopient pas

Les rayons d'apparition ennemis vivent dans `roguelite_waves.toml` (`[ring]`).
La zone interdite doit les **lire**, pas les dupliquer :

```rust
// ✅ changer ring.tank déplace automatiquement la zone interdite
SpawnKeepout::from_configs(&decor_cfg, &comp_cfg.ring)

// ❌ un miroir qui divergera au premier réglage de balance
const ENEMY_RINGS: &[f32] = &[12.0, 25.0, 50.0];
```

Si un miroir est inévitable (crates séparées), il faut un **test qui compare les
deux ensembles** — sinon la dérive est silencieuse et le défaut revient.

Ne pas oublier les variantes : un anneau élargi en vague 2 (`wave2_bonus_m`) est
un anneau de plus à protéger.

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

- [ ] Les points/zones d'apparition sont-ils **connus** du placeur ? (sinon il ne
      peut pas les éviter — c'est la cause racine du défaut d'origine)
- [ ] Le filtre est-il posé **en sortie**, une seule fois, et pas dans chaque
      générateur ?
- [ ] Le **rayon de l'objet** entre-t-il dans le test, ou seulement son centre ?
- [ ] Les zones sont-elles **lues** depuis leur source, ou recopiées ? Si
      recopiées : y a-t-il un test qui compare ?
- [ ] Le filtre a-t-il un **test multi-graines** ? Un placement seedé qui passe
      une fois ne prouve rien.
- [ ] Le filtre peut-il **tout supprimer** ? Un test doit vérifier qu'il reste du
      décor — un invariant qui vide la salle est un autre bug.
- [ ] Le rejet est-il **observable** (log/compteur) ? Sinon une zone trop large
      vide la salle en silence.

---

## 7. Cross-refs

- `map-design-intention.md` §2.4 — les arrivées ennemies « ni dans le dos au
  contact, ni hors de vue, **atteignables** »
- `map-design-patterns.md` §1 — le personnage est un **disque** : les
  dégagements se mesurent en distance, jamais en AABB gonflée
- `feedback_derive_ne_patche_pas_la_geometrie` — traiter la classe, pas le symptôme
- `observability-required.md` — le nombre de props rejetés doit se voir

---

*Adoptée 2026-07-31 (story-672). Implémentation : `SpawnKeepout` dans
`crates/forgia-mode-roguelite/src/decor.rs`, génome
`roguelite_decor.toml` → `decor_keepout_player_m` / `decor_keepout_spawn_margin_m`.*
