# story-690 — Le capteur d'arène mesure enfin la géométrie posée

**Statut** : IN_PROGRESS (livré, runtime en cours de validation)
**Date** : 2026-08-06
**Niveau BMAD** : Standard (5 fichiers)
**Related** : story-485 (solveur de layout), story-625 (coquilles autorées), story-674
(aménagement dérivé), story-683 (pièces et couloirs), story-688 (couvert de l'aire de combat)

---

## Le défaut

`forgia2_stage_layout.json`, tel qu'il sortait avant cette story :

```json
{ "severity": "ok", "stage_id": "forge_sanctum",
  "modules_placed": 0, "cover_low_count": 0, "cover_high_count": 0,
  "longest_sightline_m": 0.0, "min_cover_spacing_m": -1.0,
  "layout_source": "authored", "authored_pieces": 13 }
```

**Tout est à zéro, et il se déclare vert.** C'est exactement ce que
`map-design-patterns.md` §13 interdit : *« zéro mesuré n'est pas vert, c'est
aveugle »* — et le §14 explique pourquoi ça coûte cher : un contrôle qui passe à
vide **cache** tous les défauts qu'il devait attraper, et fait consigner de faux
verts comme des preuves.

### Pourquoi il était aveugle

Il ne lisait que `LayoutResult.placements`, rempli par le seul
`layout::place_modules`. Or ce solveur **ne produit rien sur aucune des quatre
cartes** :

| Carte | Pourquoi zéro module |
|---|---|
| `crypts_of_anvil` | `suppress_procedural_modules = true` |
| `forge_sanctum` | `suppress_procedural_modules = true` |
| `donjon_oublie` | aucun `module_palette` dans `roguelite_stages.toml` |
| `hauts_paturages` | aucun `module_palette` |

Les deux branches de severity (`authored` → « ok » sur `authored_pieces > 0`,
procédural → « info » à 0 module) couvrent donc **100 % des cas réels**, et
aucune des deux ne mesure un mètre. Les invariants sourcés de story-485
(sight-line < 40 m, espacement d'abris, inégalité de crête) ne s'appliquaient
nulle part et personne ne pouvait le voir.

---

## Ce qui est livré

### 1. La géométrie posée devient une donnée — `ArenaGeometry`

Une ressource unique où **quatre producteurs** déposent, sans se connaître :

| Producteur | Quand | Ce qu'il dépose |
|---|---|---|
| murs de pièces (`rooms`) | au bâti | tronçons (`SolidSeg`) |
| modules (`place_modules`) | au bâti | disques |
| pièces autorées | à l'arrivée du GLB | disques **mesurés** |
| décor (mode roguelite) | après planification | disques |

`reset()` au bâti, puis chacun ajoute. Le mode roguelite dépose depuis une autre
crate — même sens de dépendance que `ExtraFloorPreloads` : `forgia-stage` n'a pas
à connaître le décor du roguelite.

**Un mur reste un segment.** Le réduire à un disque le rendrait soit troué
(rayon = demi-épaisseur), soit démesuré (rayon = demi-longueur).

### 2. L'emprise d'une pièce autorée se MESURE, elle ne se déclare pas

`arena_layouts.toml` ne porte aucune taille — et bien lui en prend : la
redéclarer serait « une grandeur écrite deux fois », et attraper un coefficient
de tuning au passage sous-estimerait l'emprise d'un facteur trois
(`spawn-clearance.md` §4). La seule vérité est le mesh.

`sys_collide_authored_pieces` calculait déjà les meshes pour poser le collider :
il publie maintenant l'AABB monde au même endroit. **Le collider et la mesure
décrivent le même solide, par construction.**

Les GLB arrivent en différé → `authored_pending` compte ce qu'on attend. Tant
qu'il n'est pas nul, l'échantillon est incomplet et le capteur **refuse de
conclure** : une mesure prise à mi-chargement dirait « aucun couvert » sur une
carte qui en a.

### 3. Le rôle se dérive de la hauteur, jamais du nom

`SIGHT_BREAK_H_M = 1,80 m` remonte dans `forgia-core::layout` (il vivait dans
`decor.rs`, hors de portée du capteur). L'œil est à 1,70 m et il n'y a pas
d'accroupissement : en dessous, un obstacle masque le corps sans masquer la vue.

Les modules réutilisent `HeightClass::height_m()` — le même barème que le
solveur, pas un second.

### 4. Un PROFIL de portées, plus une seule ligne

`map-design-intention.md` §1 : *« une carte n'a pas une taille, elle a un profil
de portées »*. `longest_unbroken_sightline_m` ne mesurait que l'axe
joueur↔boss — une ligne sur une carte, et `0.0` quand il n'y a pas de boss.

64 rayons depuis le point d'apparition → médiane, maximum, et part au-delà de
30 m (le falloff de l'arsenal : −40 % de dégâts au-delà). 64 = un rayon tous les
5,6°, plus fin que la largeur angulaire d'un abri de 2 m vu à 20 m (5,7°) :
aucun abri ne peut se glisser entre deux rayons. C'est une **résolution de
mesure**, pas un réglage de gameplay.

### 5. Une severity qui ne peut plus mentir

Dans l'ordre, et **aucun seuil n'est choisi ici** :

| Condition | Verdict | Source du seuil |
|---|---|---|
| `solids == 0` | `info` + « AVEUGLE » | §13 |
| `authored_pending > 0` | `info` + « INCOMPLET » | — |
| aucun abri à < 10 m du spawn | `error` « STAND DE TIR » | `COVER_SPACING_MAX_M` (Watch Dogs, Gears) |
| moins de la moitié du couvert attendu | `error` + **le facteur** | `covers_expected()` dérivé de l'aire |
| deux abris à < 3 m | `error` | `COVER_SPACING_MIN_M` |
| ligne > 40 m | `warn` | `SIGHTLINE_MAX_M` (COD WW2 / TF2) |
| > 50 % des lignes hors pleine puissance | `warn` | falloff de l'arsenal (30 m) |

Le compte attendu se dérive à l'espacement le plus **lâche** de la bande sourcée
(10 m) : c'est la borne la plus conservatrice, donc la moins contestable. Si on
est court à 10 m, on l'est à 6 m aussi.

`INFINITY` se sérialise en `null`, plus en `-1` : `-1` se lit comme une mesure,
`null` se lit comme une absence.

Le diagnostic de palette (story-485) **descend au rang de note** (`palette_note`)
— il reste utile quand le solveur tourne, il ne décide plus de la santé d'une
arène.

---

## Critères d'acceptation

- [x] Le capteur mesure la géométrie posée, quel que soit le générateur
- [x] `solids_measured == 0` → `info` + « AVEUGLE », jamais `ok`
- [x] Échantillon incomplet (GLB en vol) → `info`, pas un faux rouge
- [x] Emprise autorée **mesurée** depuis le mesh, aucune redéclaration TOML
- [x] Profil de portées à 64 rayons, pas une ligne unique
- [x] Tous les seuils dérivés de sources existantes — **aucun genome nouveau**
- [x] `INFINITY` → `null`, plus de sentinelle `-1`
- [x] 13 tests purs (`forgia-core::layout`) + 12 (`layout_sensor`), 0 warning clippy
- [x] **Lecture runtime** : `forge_sanctum` mesuré (ci-dessous)
- [x] La géométrie meurt avec l'arène (défaut trouvé à cette lecture)
- [ ] Lecture des 3 autres arènes (un chapitre = un seul `stage_id`)
- [ ] Les constats qu'il révèle sont consignés (pistes 1 à 6 de l'audit)

---

## La première lecture runtime — 2026-08-06, `forge_sanctum`

```json
{ "severity": "error", "stage_id": "forge_sanctum",
  "solids_measured": 166, "authored_pending": 0, "rays": 64,
  "playable_radius_m": 69.28, "covers_count": 13, "covers_expected": 150.8,
  "cover_deficit_factor": 11.6, "cover_min_spacing_m": 7.74,
  "open_radius_at_spawn_m": 7.72, "sightline_median_m": 12.02,
  "sightline_max_m": 69.28, "sightline_frac_over_falloff": 0.28 }
```

**166 solides mesurés là où le capteur en voyait 0**, et `error` là où il rendait
`ok`. Ce que la lecture apprend, et qu'aucun chiffre ne disait avant :

- **13 abris pour 151 attendus** — sous-couverte d'un facteur 11,6. Même en
  restreignant le dénominateur au seul anneau de couvert (20→42 m, 4 285 m²),
  on en attendrait 43 : le déficit tient quel que soit le découpage.
- **`sightline_max_m` = le rayon jouable exact** (69,28 m) : au moins un rayon ne
  rencontre **rien** entre l'apparition et le rempart.
- **Médiane 12 m** : le combat typique est court, mais 28 % des lignes sont
  au-delà de 30 m, donc jouées à −40 % de dégâts.
- **Repli à 7,7 m** : sous les 10 m de la bande sourcée — c'est le seul des
  quatre critères qui passe.

⚠️ **Ce que cette lecture ne tranche PAS** : 13 abris sur 166 solides peut venir
d'un vrai manque de couvert *ou* d'un pool calibré trop plat (`height_at()` rend
`native_height × target / native_max_dim` — un rocher large et bas calibré à 7 m
reste sous 1,80 m). Les deux hypothèses sont maintenant **mesurables**, ce qui
était tout l'objet de la story. Ne rien corriger avant de les avoir départagées.

### Défaut trouvé par cette lecture, et corrigé

`forgia2_stage.json` annonçait `state: "idle"` pendant que le capteur de
géométrie affirmait encore mesurer `forge_sanctum` : la ressource survivait à la
démolition du stage. Un capteur qui décrit une arène qui n'existe plus est
précisément ce que cette story corrige — `despawn_stage_entities` remet
maintenant la géométrie à zéro.

## Ce que cette story NE fait PAS

Elle **mesure**, elle ne corrige rien. Les défauts que l'audit du 2026-08-06 a
listés — murs de pièces absents de la carte d'obstacles de spawn, décor figé sur
la graine de run, disque central sans couvert, rayons de décor globaux — restent
entiers. C'est délibéré : sans capteur honnête, aucune de ces corrections
n'aurait été falsifiable, et on aurait mesuré le succès au ressenti.

**Ne jamais regagner le vert en abaissant un seuil** (§13). Si une carte passe au
rouge, c'est la carte qu'on corrige.

---

## Suite — 690b : la clé de cache de fusion dérive du contenu

**Signalé en jeu au round 2** : des colliders de mur sans mur visible.

`floor_merge.rs:196` construisait `cache_key = "{label}:{extent_dm}"`, sur
l'hypothèse écrite juste au-dessus : *« le plan est déterministe par extent »*.
Vraie pour les deux labels d'origine (`floor`, remparts), **fausse** depuis :

| Label | Ce dont le plan dépend vraiment | Symptôme |
|---|---|---|
| `rooms` (story-683) | `plan_rooms(extent, **graine**, cfg)` | round 2 : colliders du nouveau labyrinthe, **murs dessinés de l'ancien** |
| `floor` (story-676) | les tuiles de l'**ambiance du round** | le ciel changeait d'univers, le sol rejouait le précédent |
| `walls` | le **kit** du stage | `forge_sanctum` et `donjon_oublie` (extent 80) partageaient `walls:800` |

`MergedStaticCache` est une Resource que ni `despawn_stage_entities` ni
`cleanup_stage_arena` ne vident : la collision traversait tous les rounds.

**Correctif** : la clé se dérive des poses et des chemins de scène du lot
(`content_cache_key`, FNV-1a sur les bits des `f32`). Ajouter la graine aurait
remis la clé à jour de ce qu'on sait *aujourd'hui* ; le prochain label qui
dépendra d'autre chose l'aurait repérimée en silence. Dérivée du contenu, elle
est juste par construction et il n'y a plus de liste de dépendances à tenir.

`extent` disparaît de la signature de `spawn_static_merge` : un paramètre qui ne
détermine plus rien ne se passe pas.

**Coût assumé** : quand le contenu change vraiment (nouvel univers, nouveau
labyrinthe), il y a rebuild — c'est exactement ce qu'on veut, et c'est ce que
l'ancienne clé économisait à tort. Une ré-entrée dans une arène identique
continue de faire cache HIT. Les deux chemins sont désormais **loggés**.

**3ᵉ occurrence de la même classe en trois jours** (identité d'arène 04/08 ·
décor sur `run_seed` · cette clé) → cf. `feedback_derive_ne_patche_pas_la_geometrie`.

Tests : 8 nouveaux dans `floor_merge` — dont
`two_different_layouts_never_share_a_key`, celui qui aurait attrapé le bug, et
`the_same_layout_still_hits_the_cache`, qui garantit qu'on n'a pas tué le cache.

**Non validé runtime** : le round 1 se bâtit normalement (`floor` 1257→91,
`walls` 66→20, `rampart_props` 15→13, clés de contenu distinctes) ; atteindre le
round 2 demande de jouer.

## Fichiers

| Fichier | Rôle |
|---|---|
| `crates/forgia-core/src/layout.rs` | primitives pures : `SolidDisc`/`SolidSeg`, profil, repli, espacement |
| `crates/forgia-stage/src/lib.rs` | `ArenaGeometry` + dépôt (murs, modules, spawn) |
| `crates/forgia-stage/src/authored.rs` | mesure de l'emprise autorée depuis le mesh |
| `crates/forgia-stage/src/layout_sensor.rs` | verdict, next-step, sérialisation |
| `crates/forgia-mode-roguelite/src/decor.rs` | `height_m` sur les solides + dépôt |
