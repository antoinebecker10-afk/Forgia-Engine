# GDD — Structure de run & progression d'arme in-run (« La Trempe »)

> Patch scopé du [GDD v1](gdd-roguelite-v1.md) — sections §6 structure / §7 progression /
> §8 level design. Rédigé 2026-07-02 sur demande Antoine : « mécaniques de level design +
> intérêt de monter les stats des armes comme dans Gunfire Reborn ».
> Sources : rapport gunfire-like 2026-07-02, code réel (`forgia-stage/graph.rs`,
> `meta_shop.rs`, `merchant.rs`), direction verrouillée (LITE : 1 arme/run, 4 armes).

## Conflits avec GDD v1 (signalés avant patch — mode update)

| GDD v1 (2026-05-27) | Réalité livrée / ce patch |
|---|---|
| Structure fixe Vestibule→Cœur→Sanctuaire→Trône | Remplacée par **RunGraph** StS (7 `StageKind`, branché, déjà codé + 50 tests) |
| Monnaie unique « éclats d'âme » | **Double monnaie livrée** : Or (in-run, perdu) / Âmes (persistantes) |
| Ennemis « Cages » (6) | Squelettes KayKit 4 archétypes livrés (Boomer/Mage manquants) |
| Aucune progression d'arme in-run | **Ajoutée par ce patch** (La Trempe) |

---

## 1. Le problème design

Gunfire Reborn rend « monter son arme » désirable par une boucle **pression → réponse → choix** :
les ennemis scalent par stage (pression), l'arme doit suivre via loot d'armes de niveau
supérieur + Craftsman payant + inscriptions (réponse), et chaque dépense est un arbitrage
(choix). **Forgia n'a aujourd'hui aucun des trois** : `difficulty_budget` est généré mais
jamais consommé (zéro pression), les dégâts d'arme sont fixes toute la run (zéro réponse),
et l'Or n'a presque pas de puits (zéro arbitrage).

## 2. Le twist identitaire Forgia (USP vs Gunfire)

Dans Gunfire on **jette** ses armes (loot d'une meilleure). Chez Forgia l'arme est une
**amie vivante** — on ne remplace pas une amie, on la **forge**. La progression in-run =
approfondir le lien, pas swapper. Cohérent avec la direction LITE (1 arme choisie au wizard,
story-612) et l'identité n°1 (« armes qui parlent »).

**Pilier (falsifiable)** : *« Mon arme au boss est ~2× plus forte qu'au départ, et je l'ai
sentie grandir »* — DPS effectif depth 4 / depth 0 ≈ 2.0 (sensor), ennemis scalés pour que
le TTK reste stable si le joueur trempe (et punisse s'il ne trempe pas).

---

## 3. Les 5 mécaniques (ordre = dépendances)

### M1 — Pression : scaling ennemi par profondeur *(prérequis de tout le reste)*

Consommer `stages[depth].difficulty_budget` (déjà généré, pondération StS) dans le spawner :

- PV : `hp_mul = 1 + depth × <gene: enemy_hp_scale_per_depth>` (cible ~+35 %/salle)
- Dégâts : `dmg_mul = 1 + depth × <gene: enemy_dmg_scale_per_depth>` (cible ~+15 %/salle)
- Le budget du node module le NOMBRE d'ennemis par vague (enfin consommé).

### M2 — Structure : consommer le RunGraph + portails de choix

- Run = `total_stages = <gene: run_total_stages>` (cible 5 : 3 Combat/Elite → 1 Rest/Shop
  → Boss), `branching = 2`.
- Après clear : **2 portes typées** (icône + couleur du `StageKind` de chaque variant du
  depth suivant) — `draw_portal_overlay` existe en dead_code, le modal porte existe
  (story-646). LE moment de décision roguelite : « Trésor risqué ou Repos ? ».
- Dispatch arène : `stage_id_for_depth` lit ENFIN `graph.stages[depth][variant].kind`
  (Combat/Elite → arène combat ; Rest/Shop → salle Forge ; Treasure → salle coffre ;
  Boss → Crypts boss).

### M3 — Réponse : « La Trempe » (niveau d'arme in-run, 1→5)

- **Où** : la **Forge du Maître** — station Enclume dans les salles Rest/Shop (et une
  gratuite après le mid-run). Interaction E, UI 1 bouton (créateur 14 ans).
- **Quoi** : payer de l'**Or** → Trempe +1 : `+<gene: trempe_damage_per_level>` (cible
  +15 %/niveau, multiplicatif avec boons) + VFX arme qui rougeoie 2 s + **bark de l'arme**
  (déclencheur `upgrade` — les pools du moteur de barks existent).
- **Coût croissant** : `<gene: trempe_cost_base>` (cible ~80 Or) ×
  `<gene: trempe_cost_growth>` (cible ×1.6/niveau) → 5 trempes ≈ 850 Or, à calibrer sur le
  revenu Or réel mesuré (sensor, cf §6).
- **Pourquoi l'Or** : il est perdu à la mort → puits parfait, zéro inflation méta ; et ça
  crée l'arbitrage Trempe ↔ marchand (heal/boon) — le « choix » de la boucle Gunfire.

### M4 — Choix : « Gravures » aux Trempes 3 et 5 *(inscriptions-light)*

À la Trempe 3 puis 5 : choisir **1 gravure parmi 2**, spécifiques à l'arme (2×2×4 armes =
16 gravures, réutilisent la plomberie boons/tags et une partie des boons d'arme du GDD v1) :

| Arme | Trempe 3 (ex.) | Trempe 5 (ex.) |
|---|---|---|
| Pépin | « Canon rayé » +30 % dégâts tête | « Chargeur profond » +50 % chargeur |
| Bourrasque | « Plombs rebondissants » ricochet ×1 | « Souffle large » +2 pellets |
| Lenoir | « Monocle poli » +1 zoom, +20 % HS | « Patience de dame » no-spread instantané |
| Boucherie | « Poudre généreuse » +1 m de rayon | « Double étage » 20 % double explosion |

Data-driven `<gene: gravures TOML par arme>`, hot-reload. Trade-offs légers autorisés
(pattern R3.4 du rapport), vocabulaire CE2, anti-canon respecté.

### M5 — Méta : la Maîtrise (lien affectif, `weapon_levels` déjà en save)

- +1 niveau par run **terminée** avec l'arme (Victory ; défaite au boss = +1 aussi —
  récompenser l'essai, cible kids/casual). Cap `<gene: mastery_level_cap>` (cible 10).
- Effet : `+<gene: mastery_damage_per_level>` (cible +2 %/niveau — sensible sans écraser
  la Trempe) + **déblocage de répliques** aux niveaux 3/6/9 (l'arme te connaît de mieux en
  mieux — le hook collectionneur Hadès du GDD v1, gratuit en contenu : les pools existent).
- Affichage : carte du wizard (story-612) + onglet Enclume. Chaque run fait progresser
  quelque chose de visible **même en défaite** (rétention douce, cohérent Âmes).

---

## 4. Level design — la run qui porte l'économie

- **Parcours = couloirs entre salles** (R2.3) : 60-90 s de platforming entre deux portes,
  collectibles **Or en hauteur / Âmes au bout des défis de marteaux**. Effet système : le
  platforming **finance la Trempe** — mieux tu traverses, plus tu forges. C'est la soudure
  entre les deux identités (parcours unique + armes-amies) et le différenciateur vs les
  couloirs morts de Gunfire.
- **Salle Rest = Forge du Maître** : Enclume (Trempe) + soin léger
  `<gene: rest_heal_fraction>` (cible 30 %) + 1 réplique Maître Forgeron.
- **Salle Treasure** : coffre d'Or + 1 gravure gratuite si défi optionnel réussi
  (marteau-pendule au-dessus du coffre — « oser les obstacles DANS les salles », R2.4).
- **Elite** : vague unique avec 1 archétype à affixe simple (+50 % PV, aura visible),
  récompense Or ×2.5. LITE : pas de nouveau modèle, un tint + scale suffit.

## 5. Hors-scope (discipline LITE, direction verrouillée)

- ❌ Loot d'armes en run / 2ᵉ arme swappable (Phase 3 post-playtest).
- ❌ Inscriptions aléatoires roll/reroll façon Gunfire (les gravures sont des CHOIX, pas du RNG).
- ❌ Ascension multi-paliers, élites à mécaniques complexes, Boomer/Mage (post-playtest).

## 6. Success metrics (→ sensors, observability-required)

| Métrique | Cible design | Sensor / champ |
|---|---|---|
| DPS effectif depth4 / depth0 | ≈ 2.0 | `forgia2_trempe.json` (niveau, dmg_mul) × `forgia2_combat.json` |
| TTK par salle (joueur qui trempe) | stable ±20 % | `forgia2_combat.json` + depth du `forgia2_run_graph.json` |
| Or gagné vs coût 5 trempes | revenu ≈ 110 % du coût | `forgia2_trempe.json` (or_spent) + sensor économie |
| % runs avec ≥1 choix de porte non-Combat | > 60 % | `forgia2_run_graph.json` (kinds choisis) |
| Maîtrise consommée | bonus visible wizard | `forgia2_meta_shop.json` (weapon_levels) |

## 7. Epics → stories

| Epic | Contenu | Story | Dépend de |
|---|---|---|---|
| E1 Pression | Consommer `difficulty_budget` (PV/dégâts/count par depth) | à créer | — |
| E2 Structure | RunGraph consommé + portails de choix typés (R2.1/R2.2) | à créer | — |
| E3 Trempe | Station Forge + économie Or + VFX + bark upgrade | à créer | E1 (sinon inutile) |
| E4 Gravures | Choix ×2 aux trempes 3/5, TOML par arme | à créer | E3 |
| E5 Maîtrise | Consumer `weapon_levels` (+2 %/niv + répliques 3/6/9 + wizard) | à créer (R3.2, ~1 j) | — |
| E6 Parcours intégré | Couloirs platformer entre salles + collectibles Or/Âmes (R2.3) | à créer | E2 |

Ordre recommandé : **E5 (quick win 1 j) → E1+E2 (le squelette) → E3 → E4 → E6**.

---

*Chaque valeur chiffrée ci-dessus est une CIBLE DESIGN — la valeur vit dans
`assets/genomes/roguelite/*.toml` (couche definition, hot-reload), jamais en dur (no-hardcode).*
