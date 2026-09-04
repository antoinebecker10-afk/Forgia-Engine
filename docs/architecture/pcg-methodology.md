# Méthodologie PCG — Générer n'importe quoi depuis un « plan »

> **Document vivant.** Architecture générale de génération de contenu procédural (PCG)
> pour Forgia. Origine : reconstruction du château « FANTASTIC Highlands Castle »
> (pack Unity → Blender → GLB → jeu, 2026-07-22). On a généralisé le cas particulier
> en méthode. Ce doc grandit à chaque nouveau kit / domaine / piège.
>
> Cross-refs : `ARCHITECTURE.md` · `docs/explainer-genome-system.md`.

---

## 0. Pourquoi ce document

Forgia = « décris ton idée + apporte tes assets, l'IA construit le jeu ». Ce doc est
**le moteur de cette promesse** pour le contenu 3D : comment passer d'une *intention*
(« un château fort avec une cour et 6 tours ») à un *asset jouable*, de façon
**générale** (pas juste des châteaux) et **cumulative** (le système gagne en capacité
à chaque asset).

---

## 1. Principe fondateur — séparer l'INTENTION de la RÉALISATION

Tout système génératif est une **échelle d'abstraction** :

```
INTENTION      « un château fort avec une grande cour et 6 tours »
   ↓
STRUCTURE      le PLAN — graphe de salles + murs + tours, en DONNÉE   ← un « plan » vit ICI
   ↓
ARRANGEMENT    chaque pièce à sa transform (mur, tour, porte…)
   ↓
GÉOMÉTRIE      le GLB / les assets
```

Un **plan** = une **spécification formelle à un niveau plus haut que la géométrie**.
Le générateur *descend l'échelle*. Pourquoi ça marche — 4 principes :

1. **Compositionnalité** — le complexe = parts simples + règles de combinaison
   (Lego, langage, chimie). Loi universelle du génératif.
2. **Une grammaire définit un LANGAGE** — vocabulaire fini + règles finies → infinité
   de structures valides ; le plan est **une phrase**. (Chomsky ; grammaire de formes
   CGA de Müller pour les bâtiments : raffiner une masse en étages→façades→fenêtres.)
3. **Les contraintes bornent un ESPACE de possibles** — contrôlable au lieu du chaos ;
   la variation = une *seed* (un point dans l'espace). (WFC : « problème de contraintes
   à milliers de solutions ; choisir au hasard **dans** les contraintes = un générateur ».)
4. **Interfaces stables entre niveaux** — changer le *réaliseur* (Blender→Unreal) sans
   toucher au plan ; changer le *vocabulaire* (kit pierre→glace) sans toucher à la
   grammaire. **C'est ce qui généralise à n'importe quoi.**

---

## 2. Architecture universelle — 8 couches

Adossée à la taxonomie PCG (grammaire / contrainte / recherche / constructif / sélection).

| # | Couche | Rôle | Méthode établie | État Forgia |
|---|---|---|---|---|
| 1 | **Vocabulaire** | catalogue de pièces + **interfaces typées** (sockets, dims, pivot, tags, UV?, collider?) | modules / kit-bashing | kit mesuré à la main |
| 2 | **Grammaire / Contraintes** | combinaisons **légales** | adjacence (WFC) · split (CGA) · connexion (graph-grammar) | règles de cellules (proto) |
| 3 | **Plan / Spec** | l'**intention** en donnée : GRAPHE/ARBRE paramétrique | représentation PCG | « genome » château (proto) |
| 4 | **Solveur** | plan+grammaire → **arrangement** | constructif · contrainte/WFC · recherche | constructif (cell-grid) |
| 5 | **Réaliseur** | arrangement → assets (place, repère, textures, export) | pipeline DCC | Blender headless → GLB ✅ |
| 6 | **Validateur** | respecte l'intention ? (WYSIWYG + métriques) | génère-teste | rendus Cycles + capteurs |
| 7 | **Intégrateur** | assets → runtime (colliders, spawn, mode, streaming) | engine integration | `castle_hub` ✅ |
| 8 | **Base de connaissance** | fait **grandir** le système (registres cumulatifs) | — | mémoire + refs |

### Couche 4 — les 3 moteurs de solveur (interchangeables)
- **Constructif** : pipeline déterministe, rapide, contrôle direct (nos cellules). *Pour aller vite.*
- **Contrainte (WFC)** : remplir une grille de tuiles compatibles (sudoku/Carcassonne). *Pour la variété locale cohérente.*
- **Recherche** : faire évoluer des candidats sous une fitness (jouable ? beau ? équilibré ?). *Pour optimiser vers un objectif.*

### Couche 8 — la base de connaissance (le moat)
- **Registre de kits** — chaque vocabulaire catalogué (manifests).
- **Bibliothèque de grammaires** — un jeu de règles **par domaine** (bâtiments, donjons, véhicules, terrains, créatures…).
- **Bibliothèque de patterns** — plans/recettes éprouvés.
- **Registre de pièges** — gotchas résolus (voir §5). ← déjà la mémoire.

---

## 3. Généralisation — on ne change que les couches 1–3

| Domaine | Vocabulaire | Grammaire |
|---|---|---|
| Château | kit bâtiment | split (masse→étages) + graphe de salles |
| Donjon | kit salles | graph-grammar (salles reliées par portes) |
| Véhicule | kit pièces méca | grammaire de connexion (sockets) |
| Terrain | tuiles biomes | WFC + bruit |
| Créature | membres/os | grammaire squelettique (L-system) |

Backbone **partagé** : solveur → réaliseur → validateur → intégrateur → connaissance.

---

## 4. Plan de construction Forgia (concret)

1. **Schéma `content-spec`** — format de plan/genome (TOML typé, hiérarchique) = interface créateur.
2. **Format `kit-manifest`** + auto-catalogueur (Blender headless *mesure* un kit → manifest).
3. **Solveur** à 3 moteurs (démarrer constructif ; brancher WFC/recherche ensuite).
4. **Réaliseur** = pipeline Blender→GLB généralisé (repère ancré, instanciation, textures).
5. **Registres** (kits / grammaires / patterns / pièges) qui grossissent à chaque asset.

---

## 5. Pièges capitalisés (registre — à compléter en continu)

- Repère Unity(gaucher,Y-haut)→Blender(droitier,Z-haut) : `Fx·(Rx90·Mu·Rx⁻90)·Fx` (réflexion obligatoire, sinon pièces inclinées de travers).
- Rotations Unity : quaternions **incomplets** dans les mods → basculer sur **euler ZXY**.
- Textures packées à chemins cassés → **re-charger depuis les PNG source** et réassigner dans les nœuds.
- **EEVEE headless crashe** (pas de GPU) → **Cycles** pour un rendu de contrôle texturé.
- Colliders : **jamais per-mesh** sur un gros GLB (8052 TriMesh = crash) → **boîtes manuelles** sur zones walkables.
- Export : **offset ancré sur le bâtiment** → les variants gardent un repère stable (colliders/spawn réutilisables) ; instanciation (`export_apply=False`) ; textures 512.
- Multi-terminal : **1 fichier = 1 terminal** ; séparer asset (GLB) et code.
- Blender **dé-duplique les noms d'objets globalement** : deux pièces avec un socket `west` → le 2e devient `west.001`. Porter l'ID réel dans une prop `pcg_socket_id` (le catalogueur la préfère au nom).
- Solveur constructif **V1** : la compat = forwards **bruts opposés** (`dot ≤ -0.999`), la pose ne fait qu'un yaw d'alignement APRÈS. Un socket ne se lie donc qu'à son opposé exact → concevoir les sockets du kit en conséquence (mur : est `+X` / ouest `−X` ; une extrémité `−X` libre veut une pièce à socket `+X`). Coins/escaliers réels = future policy quaternion.
- Chemins d'asset du `kit.toml` **relatifs à `assets/`** (racine du AssetServer Bevy), pas depuis la racine repo, sinon double préfixe `assets/assets/`.

---

## 6. GAPS / questions ouvertes (à compléter — cf prompt Codex)

- [x] Design du schéma **`content-spec`** concret (types, hiérarchie, exemples multi-domaines) — [`schemas/content-spec.md`](schemas/content-spec.md). Implémentation Rust à faire en MVP-0.
- [x] Design du **`kit-manifest`** + interfaces/sockets typés — [`schemas/kit-manifest.md`](schemas/kit-manifest.md). Parseur/validation de base dans `forgia-pcg-core`; catalogueur et export Blender statique sont disponibles ; clearance/rotation policy complète reste à faire en MVP-1.
- [x] Design du solveur : 3 moteurs, critère de choix, hard/soft et reproductibilité — §6A. Implémentation progressive à faire.
- [x] **Graph-grammar** de salles (donjon) : formalisme et exemple — §6C. À exécuter en MVP-2.
- [x] Stratégie de **validation** automatisable — §6D. Les gates runtime Hall existent partiellement ; les gates PCG restent à implémenter.
- [x] Structure des **registres** composables — §6E. Lockfile et résolveur à implémenter en MVP-0.
- [x] Frontière **offline** (Blender/Python) vs **runtime** (Bevy/Rust), streaming inclus — §6F. Le Hall valide le proxy, pas encore le découpage en cellules.
- [x] Boucle **learning** post-mortem → pattern/piège — §6G. La taxonomie est spécifiée, l'outillage reste à créer.
- [x] Références complémentaires — §8 (Merrell, ASP/clingo, Marahel, WFC, PCGML, Townscaper, glTF Transform).

---

## 6A. Audit 2026-07-22 — ce qui manque au modèle initial

Le modèle à 8 couches est une bonne **carte de responsabilités**, mais ne doit pas
devenir huit silos. Son défaut initial est de faire croire qu'un `plan` est envoyé
directement au Blender puis au jeu. En pratique, il faut des contrats vérifiables
et deux représentations intermédiaires : une logique et une spatiale.

### Les contrats transversaux obligatoires

| Contrat | Problème évité | Règle Forgia implémentable |
|---|---|---|
| **Sémantique / ontologie** | « grande salle », « porte », « route » n'ont pas le même sens selon le kit | Toute pièce, socket, nœud de graphe et règle déclare des `capabilities` et `requires` stables (`space.hall`, `portal.door`, `vehicle.power_bus`). |
| **IR logique** | une grammaire ne doit jamais dépendre d'un nom de GLB | `content-spec` → `LogicalPlan` (graphe typé, missions, zones, contraintes) ; aucun chemin d'asset à ce stade. |
| **IR spatial** | rendre/valider une composition sans réexécuter la recherche | `LogicalPlan` → `SpatialPlan` (instances de kit, socket↔socket, transform, zone, LOD, collider proxy), sérialisé en TOML/JSON. |
| **Provenance** | une seed seule ne reproduit pas un monde si les kits changent | chaque build archive `spec_hash`, versions/hashes de manifests et grammaires, version du solveur, seed racine et seeds dérivées, plateforme Blender. |
| **Unités / repères** | erreurs Unity→Blender→Bevy et colliders décalés | mètres SI, Y-up Bevy dans toutes les specs/IR ; la conversion est un adaptateur d'import/export unique, jamais une règle du solveur. |
| **Budget** | une sortie « correcte » qui fait freezer le jeu | le plan porte des budgets hard : draw calls, triangles, textures, RAM/VRAM, entités, colliders, temps de génération et de chargement. |
| **Évidence de validation** | « ça a l'air bon » non reproductible | chaque artefact a un `validation-report` versionné : métriques, captures Cycles, résultats de bots, profil runtime et verdict. |

Ces contrats sont des **aspects transversaux** aux 8 couches, pas une neuvième
couche : ils gardent la composition des couches possible dans le temps.

### Critique des trois moteurs

Les trois moteurs sont complémentaires, mais « WFC » n'est pas une catégorie
équivalente à « recherche » : WFC est un solveur de CSP local avec heuristiques
d'entropie. Il ne garantit ni les objectifs globaux (clé avant porte, nombre de
tours), ni une solution sans backtracking. La recherche ne doit jamais choisir
parmi des candidats invalides ; elle optimise **après** des contraintes hard.

La règle de décision est donc :

1. **Constructif** si la topologie est connue et les décisions sont des formules ou
   des choix locaux sans retour arrière : extrusion d'un bâtiment, placement de
   sockets déjà résolus, terrain déterministe, véhicule à squelette imposé.
2. **CSP / WFC généralisé** si l'inconnue est l'étiquette d'une cellule, face ou
   arête, et si la validité se déduit principalement d'adjacences locales : murs,
   façades, décor de sol, ville sur maillage irrégulier. Utiliser propagation,
   ordre MRV/entropie, backtracking borné et diagnostics de contradiction ; pas
   une boucle de relance aveugle.
3. **Recherche / ASP / CP-SAT** si des dépendances globales, une mission, des
   ressources ou plusieurs objectifs importent : graphe de donjon lock-and-key,
   progression roguelite, allocation de kits rares, optimisation perf/variété.
   L'ASP est le meilleur premier choix déclaratif pour des graphes de taille
   modérée et des règles explicables ; une recherche évolutionnaire/novelty est
   réservée aux compromis où la fonction de score est mesurable mais difficile à
   formaliser.

La norme est le **pipeline hybride**, pas l'exclusivité : ASP/CSP pour le graphe
macro → constructif pour le layout des salles → WFC sur une grille ou un graphe
irrégulier pour le détail → recherche hors ligne pour sélectionner les meilleurs
candidats. Model Synthesis est un quatrième adaptateur utile : il apprend les
adjacences d'un exemple *annoté* et produit un manifest/pattern candidat ; il ne
remplace pas les contraintes de jouabilité ou de budget.

### Déterminisme, hard/soft et reproductibilité

- Toutes les sources de hasard passent par `GenerationContext { root_seed,
  derivation_path, generator_version }`. Une sous-seed est dérivée par hash stable
  de `root_seed / spec-id / zone-id / pass-id`, jamais par l'ordre d'itération
  d'une `HashMap`.
- Le rapport expose la chaîne complète de seeds, les choix du solveur et les
  backtracks. Les itérations doivent être triées par ID stable.
- **Hard** = une violation rend le candidat non exportable : reachability,
  sockets, non-intersection, collider budget, licences, navmesh et invariants de
  mission. **Soft** = score normalisé et expliqué : variété, vues, densité,
  symétrie, coût GPU. Un soft ne contourne jamais un hard.
- Le replay CI reconstruit l'IR à partir de la provenance et compare son hash ;
  les images de contrôle sont comparées avec un seuil perceptuel, pas au pixel.

---

## 6B. Schémas exécutables : PLAN et KIT

Les deux contrats sont spécifiés dans :

- [`schemas/content-spec.md`](schemas/content-spec.md) : intention hiérarchique,
  paramètres, contraintes, budget, seed et trois exemples complets.
- [`schemas/kit-manifest.md`](schemas/kit-manifest.md) : assets, sockets typés,
  compatibilité formelle, proxies de collision/LOD et exemple de mur.

**Règle d'architecture :** le `content-spec` ne référence que des capacités et des
IDs de registre ; le `kit-manifest` est la seule couche qui connaît les fichiers
GLB. Le solveur produit `SpatialPlan` avant d'appeler Blender ou Bevy.

---

## 6C. Grammaire de graphe pour donjon — formalisme concret

Un donjon est un graphe attribué orienté `G = (V, E, A)`. Un nœud a un type
(`entrance`, `combat`, `key`, `lock`, `boss`, `treasure`) et des attributs
(`tier`, `biome`, `footprint`, `capabilities`). Une arête est un portail avec
(`door_type`, `one_way`, `requires`, `grants`). Une production est :

```
P = (LHS, guard, RHS, embedding, cost_delta)
```

Elle remplace un sous-graphe `LHS` par `RHS`, reconnecte les frontières avec
`embedding`, et n'est applicable que si `guard` et les budgets restent vrais.
Exemple de grammaire de progression :

```toml
[production.expand_gate]
lhs = { node = "frontier", tags = ["needs_progress"] }
guard = ["depth < max_depth", "remaining_rooms >= 3"]
rhs.nodes = [
  { id = "fight", kind = "combat", grants = ["sigil.azur"] },
  { id = "gate",  kind = "lock", requires = ["sigil.azur"] },
  { id = "next",  kind = "frontier", tags = ["needs_progress"] },
]
rhs.edges = [
  { from = "$boundary", to = "fight", kind = "door" },
  { from = "fight", to = "gate", kind = "door" },
  { from = "gate", to = "next", kind = "door" },
]
embedding = "boundary_in -> fight; next -> former_outgoing"
```

Les invariants hard sont évalués après chaque production : une entrée existe,
un boss est atteignable, chaque `requires(x)` a un `grants(x)` sur tous les chemins
admissibles, et aucun lock ne domine sa propre clé. Un solveur ASP/CP-SAT choisit
les productions à l'échelle macro ; le layout spatial ne commence qu'après cette
preuve. Cette séparation est directement motivée par les donjons action-aventure
à contraintes lock-and-key étudiés par Smith, Padget et Vidler.

---

## 6D. Validation : générer → prouver → mesurer → sélectionner

### Matrice minimale de gates automatisables

| Domaine | Gate hard | Mesures soft / diagnostic | Exécuteur |
|---|---|---|---|
| Structure | tous les sockets sont appariés ou explicitement bouchés ; pas d'overlap d'AABB | connectivité, cyclomaticité, profondeur, variété inter-seed | solveur Rust |
| Jouabilité | spawn→objectif atteignable sur NavMesh ; clé avant serrure ; clearance capsule | longueur de chemin, temps bot, couverture des affordances, difficulté | bot headless + Rapier/NavMesh |
| Géométrie | pas de faces inversées/NaN ; transforms et unités valides | ratio d'occlusion, surface répétée, silhouette | Blender + glTF validator |
| Performance offline | budgets meshes/materials/images/triangles/LOD/proxies respectés | taille GLB/KTX2, densité texel, temps de cook | Blender + `gltf-transform` |
| Performance runtime | pas de collider per-mesh ; streaming sous budget ; aucune frame > seuil pendant load | P50/P95/P99 frame-time, draw calls, VRAM/RAM, temps d'activation de zone | capteurs Forgia + Tracy |
| Esthétique | palettes et tags de biome compatibles | diversité, symétrie voulue, entropie visuelle, capture Cycles validée | règles + revue humaine/ML optionnel |

Une sortie est classée `rejected`, `valid_unranked`, `selected` ou `shipped`.
Le validateur ne « répare » pas silencieusement : toute réparation devient un
nouveau pass versionné dans la provenance. Les métriques sont conservées aussi
pour l'**expressive range** : si 1 000 seeds donnent la même profondeur et le même
nombre de salles, le générateur est techniquement valide mais créativement pauvre.

### Budgets initiaux pour une zone Forgia

Les chiffres finaux dépendent de la cible matérielle ; ce sont donc des budgets de
spec à rendre configurables, non des constantes universelles. Pour le Hall actuel,
le gate prouvé est `scene_colliders = 0`, `collision_proxies <= 32` (5 en V1) ;
les 8 052 TriMesh du GLB sont une régression bloquante. Ajouter par tier cible :
`frame_p99_ms`, `load_p99_ms`, `visible_meshes`, `visible_triangles`, `materials`,
`texture_vram_mb`, `streaming_peak_mb` et `despawn_p99_ms`.

---

## 6E. Registres composables, versionnés et auditables

Un registre n'est pas un dossier de fichiers : c'est un package immuable avec un
manifest, un ID canonique et des dépendances résolues dans un lockfile.

```
pcg-registry/
  kits/<namespace>/<kit>/<semver>/kit.toml
  grammars/<namespace>/<grammar>/<semver>/grammar.toml
  patterns/<namespace>/<pattern>/<semver>/pattern.toml
  pitfalls/<domain>/<id>.md
  registry.lock.toml
```

- IDs : `forgia.castle.stone@1.2.0`, `forgia.dungeon.lock_key@1.0.0`.
- Dépendances avec bornes semver et hash de contenu ; un `registry.lock.toml`
  fige les versions réellement utilisées par une génération.
- L'héritage est **explicite et fini** : `extends = "forgia.castle.base@1.1.0"`,
  puis `override` par clé ; une résolution détecte les cycles et matérialise un
  manifest final avant solveur. Aucun héritage implicite par chemin de dossier.
- Les compatibilités de sockets appartiennent au kit, les règles de forme à la
  grammaire, les objectifs à la spec ; les patterns ne font qu'assembler des IDs.
- Un piège relie `symptom`, `scope`, `root_cause`, `fix`, `regression_test`,
  `evidence` et versions affectées. Il devient une règle de validation quand
  possible, pas seulement une note humaine.

Le lockfile a son propre schéma et ne résout aucune plage semver au runtime :

```toml
schema_version = "forgia.pcg-registry-lock/v1"

[[entries]]
id = "forgia.castle.stone@1.0.0"
kind = "kit" # kit | grammar | pattern
manifest = "kits/forgia/castle/stone/1.0.0/kit.toml"
content_hash = "sha256:<64-hex-lowercase>"
dependencies = []
```

`forgia-pcg-core::PcgRegistryLock` rejette les IDs, hashes, dépendances ou
cycles invalides. La gate `cargo xtask validate-pcg` le reconnaît comme un
contrat PCG ; le publisher offline aura ensuite la responsabilité de recalculer
le SHA-256 du fichier référencé et de refuser toute divergence.

---

## 6F. Frontière offline / runtime et streaming

| Cuire offline (Blender/Python → GLB/KTX2/proxies) | Générer/activer runtime (Bevy/Rust) |
|---|---|
| import Unity, conversion de repère, meshes, UV/tangentes, atlas/KTX2, LOD/HLOD, colliders **proxy**, NavMesh par zone, probes et captures Cycles | sélection seedée de variantes déjà cuites, instanciation d'IR, spawn gameplay, état destructible, décor léger, streaming de cellules, LOD/visibility, persistence |
| solveurs coûteux, recherche de milliers de candidats, bake de terrain statique | solveur borné seulement si le jeu exige un monde inédit ; ne jamais compiler un GLB ni générer des milliers de colliders en frame de jeu |

Le `SpatialPlan` est découpé en `StreamCell` avec : AABB, dépendances voisines,
bundle d'assets, proxy physique, points de spawn et budget. Le runtime précharge
la cellule destination et ses voisines avant un portail ; il active d'abord proxy
collision/navmesh, puis rendu ; il désactive rendu puis physique au déchargement.
L'état de jeu est séparé de l'asset cuit par IDs stables de cellule/instance.

Le château Highlands est le contre-exemple à ne pas répéter : un GLB monolithique
de ~48–52 MB et 8 052 meshes visibles a été intégré visuellement, puis son
`AsyncSceneCollider` a produit 8 052 colliders et des hitches mesurés. Le correctif
est un proxy de Grande Salle à 5 boîtes ; la solution durable est le découpage
offline en cellules + HLOD + proxies par zone, pas davantage de logique runtime.

---

## 6G. Boucle d'apprentissage et IA : apprendre sans polluer la vérité

Chaque build produit un `generation-case` : spec verrouillée, provenance,
`SpatialPlan`, rapport de validation, captures, télémétrie et décision humaine.
Un échec suit ce flux :

```
symptôme → reproduction minimale → cause → correctif → test de non-régression
          → pitfall/pattern/manifest/grammar versionné → revalidation des seeds
```

La base de connaissance doit distinguer **fait mesuré**, **règle de production** et
**hypothèse**. Une IA peut proposer une spec, une grammaire ou une réparation ;
elle ne peut publier dans le registre qu'après les gates automatiques et une revue.
À moyen terme, PCGML sert d'abord à classer/ranker/réparer à partir des
`generation-case` (petit jeu de données, explication obligatoire), pas à produire
des GLB opaques. C'est cohérent avec la littérature PCGML : l'analyse et la
réparation de contenu sont des usages aussi importants que la génération.

---

## 6H. Plan d'implémentation — MVP vers plateforme

| Phase | Livrable et jalon | Couches principalement couvertes |
|---|---|---|
| **MVP-0 : contrat** | `forgia-pcg-core` : parseurs/validateurs initiaux `content-spec` + `kit-manifest`, `GenerationContext`, `LogicalPlan` et `SpatialPlan` sérialisables, test de seed et socket matching ; restent lockfile, clearance/rotation policy complète, diagnostics de références et golden tests | 1, 3, 4, 8 + contrats |
| **MVP-1 : un kit / un domaine** | auto-catalogueur Blender, exporteur statique GLB + proxy + LOD, kit `castle.stone`, solveur constructif de murs/sockets, rapport de validation | 1, 2, 4, 5, 6 |
| **MVP-2 : donjon jouable** | graph grammar + ASP/CP-SAT macro, layout constructif, bot reachability et lock-key gates ; 1 000 seeds expressive-range CI | 2, 3, 4, 6, 7 |
| **MVP-3 : runtime robuste** | bundles de `StreamCell`, préchargement/déchargement, capteurs budgets et QA headless ; régression Hall supprimée par gate | 5, 6, 7 |
| **Complet : composition** | WFC généralisé sur graphe irrégulier, héritage de registres, recherche multiobjectif hors ligne, patterns multi-domaines | 1–8 |
| **Apprentissage assisté** | corpus de `generation-case`, ranking/repair PCGML et assistant de spec avec preuves ; jamais source de vérité non validée | 6, 8 |

**Premier vertical slice recommandé :** un mini-donjon de 6–12 salles avec trois
kits de salle, portes/sockets, une clé et une serrure, exporté en 2–4 cellules.
Il force toutes les interfaces importantes sans se cacher derrière la complexité
du château monolithique.

---

## 6I. Audit des fondations Forgia déjà présentes — réutiliser, ne pas dupliquer

| Existant | Ce qu'il apporte réellement | Raccord PCG recommandé | Manque à combler |
|---|---|---|---|
| `forgia-anchor` | `AnchorPoint` typé, slots stables, layouts déterministes, capteur | compiler les `spawn.player`, `poi.*`, `landmark`, `boss` du `SpatialPlan` vers ces composants | étendre par IDs/capabilities de spec, sans renuméroter `AnchorKind` |
| `forgia-stage::graph` | graphe de run seedé et testé, choix branchés | le conserver comme **méta-graphe de run** ; faire référencer par chaque `StageNode` un `content-spec` de niveau | provenance complète/seed dérivée, graphe de salles et solveur de mission intra-stage |
| `forgia-asset-registry` / assets bundles | manifest de packs, versions, licence, état installé | le prendre comme couche de distribution de `kit-manifest`; le lock PCG résout les versions de packs en hashes | manifeste de pièces/sockets, dépendances de grammaire et résolution d'héritage |
| `forgia-streaming` | génome de rayons, hystérésis, budget, queue async et télémétrie | les `StreamCell` du `SpatialPlan` alimentent ce système, non un second manager | enforcers de cellule, bundles par zone, activation proxy→nav→rendu |
| `forgia-game::castle_hub` + capteurs | cas réel de gros GLB, cleanup et budget physique désormais observable | conserver comme fixture de régression pour le cook/streaming | découpage offline, HLOD/LOD et proxies par cellule |
| `tools/blender/inspect_glb.py`, rendu headless, réexport tangentes | inspection et contrôle d'assets déjà scriptables | les appeler depuis le catalogueur et les gates CI | convention d'empties sockets, génération de manifest, rapports machine-readable |
| `xtask validate-pcg` | gate pure et reproductible des contrats publiables | l'exécuter avant tout cook/export : `cargo xtask validate-pcg --require-any` une fois `assets/pcg/` introduit | vérification de hashes GLB, clearance, budgets et lockfile |

Le choix structurant est donc : **ne pas créer un “PCG runtime” monolithique**.
`forgia-pcg-core` existe désormais comme crate pur (schéma initial, IR, seed et
tests, sans Bevy) ; la suite est `forgia-pcg-cook` offline (Blender/outils), puis
des adaptateurs minces vers `anchor`, `stage`, `asset-registry` et `streaming`.
Cela préserve les tests headless et évite les cycles de dépendances ECS.

---

## 7. Changelog

- **2026-07-22** — Création. Généralisé depuis la reconstruction du château Highlands.
  8 couches, 3 moteurs, principes de fond, registre de pièges initial.
- **2026-07-22** — Audit/complément : contrats IR/provenance/budgets, règle de
  décision des solveurs, grammaire de donjon, validation automatisable,
  registres versionnés, streaming offline/runtime, boucle d'apprentissage et
  feuille de route, audit des fondations Forgia. Schémas liés :
  `schemas/content-spec.md`, `schemas/kit-manifest.md`.
- **2026-07-22** — MVP-0 démarré : ajout de `forgia-pcg-core`, crate headless
  pour parser/valider le `content-spec` initial, dériver des seeds stables et
  compiler le `LogicalPlan`; `SpatialPlan` est le contrat prêt pour les prochains
  solveurs/cookers.
- **2026-07-22** — MVP-0 étendu : `kit-manifest` et compatibilité formelle des
  sockets (famille, genre, acceptation, aperture, orientation) implémentés et
  testés dans `forgia-pcg-core`; le crate est raccordé à `forgia-procgen-graph`.
- **2026-07-22** — MVP-1 amorcé : `tools/blender/catalog_pcg_kit.py` catalogue
  une scène Blender conventionnée en `kit.toml` et convertit explicitement le
  repère Blender vers Forgia. Testé headless sur un fixture temporaire.
- **2026-07-22** — MVP-1 étendu : `tools/blender/export_pcg_kit.py` cuit
  non destructivement les pièces statiques conventionnées en GLB de rendu, LOD
  et proxy de collision distincts. Le round-trip headless a produit et inspecté
  les trois GLB d'un fixture ; aucun `.blend` source du château n'est encore
  annoté comme kit publiable.
- **2026-07-22** — Gate ajoutée : `cargo xtask validate-pcg` valide les
  `content-spec` et `kit-manifest` par leur `schema_version`, en ordre stable,
  avant publication. Elle est volontairement en `SKIP` tant qu'aucun contrat
  n'existe sous `assets/pcg/`; la CI de publication utilisera `--require-any`.
- **2026-07-22** — Solveur constructif initial : `forgia-pcg-core` assemble une
  recette ordonnée de sockets en `SpatialPlan`, calcule position et yaw du
  module attaché, et refuse zones, pièces, instances ou interfaces invalides.
  La V1 est sciemment limitée aux sockets horizontaux et à la rotation Y ; les
  cages d'escalier, plafonds et pièces inclinées devront introduire une policy
  quaternion/clearance explicite, jamais contourner cette garde.
- **2026-07-22** — Provenance figée : `PcgRegistryLock` valide le lockfile des
  kits/grammaires/patterns (hash SHA-256 déclaré, dépendances résolues et DAG
  acyclique). `validate-pcg` couvre désormais également ce contrat.
- **2026-07-22 (suite)** — MVP-1/3 avancés : schéma kit aligné (bounds_m,
  collision, lods, clearance), **validateurs hard** (`validate_spatial_plan` :
  bounds monde, budgets, clearance contextuelle porte-dans-mur, reachability ;
  reste déféré = navmesh/dominance), **cellules de streaming** + ladder
  d'activation collision/nav→rendu (`stream.rs`), **premier vertical slice
  grey-box** `castle.stone@1.0.0` (5 pièces bakées, contrats `assets/pcg/`
  validés), preuve end-to-end sur assets shippés, et adapter Bevy
  `forgia-pcg-runtime` (SpatialPlan→cellules, plugin d'ordonnancement testé
  headless, pas encore câblé dans l'app live). Détail + preuves de validation :
  `docs/handoff/CODEX_PCG_HANDOFF_2026-07-22.md` (section « suite Claude »).

---

## 8. Références

- CGA shape grammar — Müller et al., *Procedural Modeling of Buildings*, ACM ToG 2006.
- *Wave Function Collapse Explained* — boristhebrave.com ; *WFC* — gridbugs.org.
- *Search-Based Procedural Content Generation: A Taxonomy and Survey* — Togelius et al.
- Paul Merrell, *Example-Based Model Synthesis* (2007) — <https://graphics.stanford.edu/~pmerrell/model_synthesis.pdf>. Bon adaptateur pour inférer des adjacences depuis un exemple, sous contraintes Forgia.
- Adam M. Smith & Michael Mateas, *Answer Set Programming for PCG: A Design Space Approach* (2011) — <https://adamsmith.as/papers/tciaig-asp4pcg.pdf>. Base du modèle déclaratif hard constraints + solveur.
- Thomas Smith, Julian Padget & Andrew Vidler, *Graph-based Generation of Action-Adventure Dungeon Levels using ASP* (FDG 2018) — <https://doi.org/10.1145/3235765.3235817>. Graphe de mission lock-and-key contrôlable.
- Adam Summerville et al., *Procedural Content Generation via Machine Learning* (2018) — <https://arxiv.org/abs/1702.00539>. PCGML pour analyse, réparation et co-création, avec vigilance sur les données.
- Oskar Stålberg, *Organic Towns from Square Tiles* (IndieCade Europe 2019) — <https://www.youtube.com/watch?v=Vrg6Gxeu4Ps>. Référence pratique pour topologie irrégulière et règles locales ; ne pas l'assimiler à un WFC carré standard.
- Maxim Gumin, *WaveFunctionCollapse* — <https://github.com/mxgmn/WaveFunctionCollapse>. Référence d'implémentation : propagation, entropie minimale, contradictions et limites NP-hard ; inclut l'explication du lien à Model Synthesis.
- Khalifa et al., *Marahel: A Language for Constructive Level Generation* (AIIDE 2016) — <https://ojs.aaai.org/index.php/AIIDE/article/download/12970/12818/16487>. Inspirant pour décrire des passes constructifs data-driven ; Forgia doit conserver son IR typé et ses gates plutôt que réimplémenter un DSL opaque.
- Potassco, *clingo / ASP* — <https://potassco.org/guide/> et <https://github.com/potassco/clingo>. Candidat pragmatique hors ligne pour graphes de mission et contraintes globales, sous licence MIT.
- glTF Transform — <https://gltf-transform.dev/>. Outil de cook/inspection pour LOD, meshopt et KTX2 ; à intégrer aux gates offline, non au runtime.
