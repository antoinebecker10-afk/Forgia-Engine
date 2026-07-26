# Handoff — fondations PCG et kit château

Date : 2026-07-22

## But de cette session

Transformer la méthode PCG Forgia en contrats et outils exécutables, sans
remplacer le château actuel ni inventer un kit d'assets inexistant. Le résultat
est une fondation data-driven, déterministe et offline-first : les GLB sont
cuisinés dans Blender, le runtime Bevy ne fait qu'instancier/streamer un plan
déjà validé.

## État honnête

Les fondations sont implémentées et vérifiées. **Aucun kit de production n'est
encore publié** sous `assets/pcg/`, et le château reste le GLB monolithique
`assets/models/environment/castle/castle_highlands.glb`. Ne pas prétendre que
le château est déjà généré procéduralement.

Le prochain vertical slice est : découper/cuisiner un petit kit du château
(Grande Salle landmark + entrée + mur droit + angle + tour), lui écrire un
manifest et une spec, puis le charger via le runtime/streaming existant.

## Décisions d'architecture à préserver

- **Data-driven** : `content-spec` exprime l'intention ; `kit-manifest` est la
  seule couche qui connaît les GLB ; le solveur produit `SpatialPlan` avant
  Blender/Bevy.
- **Déterminisme** : seed explicite, sous-seeds dérivées par chemin stable,
  ordre stable ; aucun choix fondamental dépendant de `HashMap`.
- **Offline-first** : Blender cuit GLB, LOD, collision proxy, navmesh/HLOD
  futurs. Bevy sélectionne, instancie et stream ; pas de compilation GLB ni
  milliers de colliders générés en frame.
- **Physique** : jamais de `AsyncSceneCollider`/TriMesh par mesh sur un gros
  décor. Proxies simples par bloc/cellule uniquement. Le château a déjà montré
  pourquoi : 8 052 colliders pouvaient provoquer crash/hitches.
- **Repères prouvés** : Unity→Blender par réflexion, rotations Unity ZXY si
  nécessaires, offset ancré sur le bâtiment, `export_apply=False`, Cycles et
  non EEVEE en headless.
- **Limite actuelle volontaire** : le solveur constructif ne gère que les
  sockets horizontaux et le yaw Y. Ne pas contourner la garde pour les
  escaliers/plafonds ; concevoir une policy quaternion + clearance dédiée.

## Fichiers ajoutés/modifiés par Codex

### Documentation / bible

- `docs/architecture/pcg-methodology.md`
  - Méthode 8 couches / 3 moteurs complétée : IR, hard/soft constraints,
    graph grammar, validation, streaming, apprentissage, lockfile, roadmap,
    fondations existantes Forgia et changelog.
- `docs/architecture/schemas/content-spec.md`
  - Schéma et exemples château, donjon, véhicule.
- `docs/architecture/schemas/kit-manifest.md`
  - Sockets formels, exemples, convention Blender, export GLB/LOD/proxy.

### Crate pur PCG

- `crates/forgia-pcg-core/`
  - `src/lib.rs` : `ContentSpec`, `GenerationContext`, `LogicalPlan`,
    `SpatialPlan`, `StableId` ; parse TOML et seed déterministe.
  - `src/kit.rs` : `KitManifest`, pièces, sockets et compatibilité formelle
    (famille, genre, acceptation, aperture, orientation).
  - `src/solver.rs` : `assemble_socket_chain`, solveur constructif d'une
    recette ordonnée ; place une pièce sur un socket, calcule translation/yaw,
    échoue sur zones/pièces/instances/sockets invalides.
  - `src/registry.rs` : `PcgRegistryLock` ; hashes SHA-256 déclarés,
    dépendances résolues et graphes acycliques.
- `crates/forgia-procgen-graph/`
  - Dépend maintenant de `forgia-pcg-core` et réexporte les contrats PCG
    génériques ; son `VillageGraph` historique est conservé.
- `Cargo.toml`, `Cargo.lock`
  - Workspace raccordé à `forgia-pcg-core`.

### Blender et QA

- `tools/blender/catalog_pcg_kit.py`
  - Lit les collections `PCG_PIECE__<id>`, `PCG_ROOT__<id>`, sockets
    `PCG_SOCKET__*` et métadonnées ; émet un `kit.toml`.
- `tools/blender/export_pcg_kit.py`
  - Export non destructif de modules statiques :
    `render/<piece>.glb`, `render/<piece>_lod<n>.glb`,
    `collision/<piece>_proxy.glb`.
  - Testé avec Blender 5.0.1 sur fixture temporaire : trois GLB produits,
    GLB de rendu inspecté avec `gltf-transform`, tangentes présentes.
- `xtask/src/main.rs`, `xtask/Cargo.toml`
  - Nouvelle gate : `cargo xtask validate-pcg [--root <dir>] [--require-any]`.
  - Valide `content-spec`, `kit-manifest` et `pcg-registry-lock` selon
    `schema_version`. Par défaut elle fait `SKIP` si `assets/pcg/` n'existe pas
    — comportement voulu tant qu'aucun contrat de production n'est publié.

## Conventions de source Blender

```text
PCG_PIECE__wall_straight_4m   collection d'une pièce
PCG_ROOT__wall_straight_4m    Empty unique : repère local stable
PCG_SOCKET__west              Empty : interface typée
PCG_COLLISION__wall           mesh proxy physique séparé
PCG_LOD1__wall                mesh LOD séparé
```

Les sockets doivent porter `pcg_family`, `pcg_role`, `pcg_gender`,
`pcg_aperture_shape`, les dimensions et `pcg_accepts`. Voir le schéma pour les
types exacts. La première version de `catalog_pcg_kit.py` exige au moins une
socket par pièce : une Grande Salle non connectable devra soit recevoir une
socket de transition explicite, soit faire évoluer le contrat avec un tag
`landmark` testé plutôt que contourner silencieusement la validation.

## Commandes utiles

```powershell
# Contrats Rust
cargo test -j 1 -p forgia-pcg-core -p forgia-procgen-graph
cargo clippy -j 1 -p forgia-pcg-core -p xtask --all-targets -- -D warnings

# Gate des contrats de production (à activer dès assets/pcg créé)
cargo xtask validate-pcg --require-any

# Cataloguer un .blend modulaire
& "C:\Program Files\Blender Foundation\Blender 5.0\blender.exe" `
  --background assets/source/castle_stone.blend `
  --python tools/blender/catalog_pcg_kit.py -- `
  --kit-id forgia.castle.stone@1.0.0 `
  --asset-root assets/pcg/kits/castle_stone/1.0.0 `
  --output assets/pcg/kits/castle_stone/1.0.0/kit.toml

# Cuire les GLB statiques, LOD et proxy
& "C:\Program Files\Blender Foundation\Blender 5.0\blender.exe" `
  --background assets/source/castle_stone.blend `
  --python tools/blender/export_pcg_kit.py -- `
  --asset-root assets/pcg/kits/castle_stone/1.0.0
```

## Validations réellement exécutées

- `cargo test -j 1 -p forgia-pcg-core -p forgia-procgen-graph` : vert
  (12 tests lors de la dernière passe croisée ; 9 tests directs PCG après
  l'ajout du lockfile).
- `cargo clippy -j 1 -p forgia-pcg-core -p xtask --all-targets -- -D warnings`
  : vert.
- `cargo xtask check-orphans --strict` : vert (62 crates, 0 orpheline
  inattendue ; 2 outils standalone attendus).
- `cargo xtask validate-pcg` : `SKIP` attendu car `assets/pcg/` n'existe pas.
- Fixture de `validate-pcg --require-any` : 1 content-spec + 1 kit-manifest,
  vert ; fixture retirée ensuite.
- `git diff --check` ciblé : vert ; warnings CRLF de Git attendus.

## Prochain plan concret — ne pas tout faire en même temps

1. **Créer un source Blender modulaire** à partir du château visuellement validé.
   Ne pas modifier le GLB monolithique runtime en place. Conserver ce dernier
   comme référence/fallback.
2. **Premier kit minuscule** : Grande Salle (landmark), entrée, mur droit,
   angle, tour ; proxies box et LOD si nécessaires. Mesurer triangles, meshes,
   taille GLB et collision proxy pour chaque bloc.
3. Publier sous `assets/pcg/` : `kit.toml`, `content-spec`,
   `registry.lock.toml`, exports. Activer `validate-pcg --require-any` dans la
   CI pertinente.
4. Ajouter un adapter runtime explicite `SpatialPlan -> forgia-streaming` ;
   chargement préventif de la cellule destination, activation collision/navmesh
   avant rendu, ordre inverse au déchargement.
5. Ajouter les validateurs hard avant le WFC : AABB/clearance, bounds monde,
   budgets, reachability/navmesh. Une porte intégrée dans un mur est un
   chevauchement intentionnel : ne pas écrire une règle AABB globale naïve.
6. Seulement ensuite : graph grammar de donjon, WFC sur sous-problèmes locaux,
   recherche offline, CI multi-seeds.

## Discipline de collaboration

Le worktree est déjà très sale et partagé avec l'utilisateur/Claude. Préserver
les changements non reliés ; inspecter `git diff` avant toute modification ; ne
pas faire de `git reset --hard` ou checkout destructif. Si Cargo indique
`Blocking waiting for file lock`, un autre terminal compile : attendre, ne pas
le tuer. Après toute modification, mettre ce handoff à jour avec preuve et
commande de validation réellement exécutée.

---

## Session 2026-07-22 (suite Claude) — audit, vertical slice grey-box, adapter runtime

### 1. Audit des fondations (aucun bug confirmé)

Lecture intégrale de `forgia-pcg-core` (lib/kit/solver/registry), des 2 scripts
Blender et de `validate-pcg`. Vérification manuelle : placement `place_on_socket`
correct (translation/yaw re-dérivés), compat sockets correcte, détection de cycle
registry correcte. **Aucun bug confirmé** — je n'ai rien « corrigé » à l'aveugle
(règle no-speculative-fix). Le « 1 error » de clippy était le **fantôme RTK**
documenté (RTK wrappe `cargo` et fausse la sortie clippy) ; le vrai clippy via
`$(rustup which cargo)` sort exit 0. Gaps MVP relevés puis comblés ci-dessous.

### 2. Schéma kit-manifest aligné sur la doc + fix G1

`forgia-pcg-core/src/kit.rs` : `PieceSpec` porte désormais `bounds_m`,
`collision`, `lods` ; `SocketSpec` porte `clearance` (types `Bounds`,
`CollisionProxy`, `Lod`, `Clearance`) — tous optionnels (fixtures existantes
inchangées). `lib.rs` : `ContentSpec` parse `budgets` (BTreeMap déterministe),
`constraints.hard`, `streaming`. **Fix G1** : `ContentSpec::validate` rejette
maintenant un `zone.parent` inconnu (`PcgError::UnknownParent`).

### 3. Validateurs hard — `forgia-pcg-core/src/validate.rs`

`validate_spatial_plan(spec, manifest, plan) -> ValidationReport` (data-driven,
déterministe) : **bounds monde** (extent union ≤ `max_extent_m`), **budgets**
(stream_cells / visible_meshes / collision_proxies, + contraintes `kind="budget"`),
**clearance contextuelle** (le volume libre d'un socket ne doit pas être intrudé
par le *solide* d'une autre instance — **sauf paire liée par un socket** : une
porte dans son mur est un chevauchement intentionnel, jamais une interdiction AABB
globale naïve), **reachability** (connectivité structurelle zone→zone sur les
bindings). Ce qui n'est pas prouvable au niveau plan (marchabilité capsule /
navmesh, dominance lock-and-key, triangles/VRAM) est **déféré** explicitement à
son exécuteur (`DeferredCheck`), jamais passé en silence.

### 4. Cœur streaming pur — `forgia-pcg-core/src/stream.rs`

`compute_stream_cells(plan, layout)` partitionne en grille 3D déterministe
(BTreeMap, dépendances = voisins Chebyshev ≤ `preload_neighbors`). Machine à états
`CellPhase` (Unloaded→Preloaded→Physics→Rendered) : `next_up` (charge) garantit
collision/navmesh **avant** rendu, `next_down` (décharge) l'inverse.
`activate_order_sound` / `deactivate_order_sound` valident que l'ordre déclaré
dans le TOML respecte la ladder. `cell_of(pos, layout)` mappe une position → cellule.

### 5. Premier vertical slice grey-box (kit `castle.stone@1.0.0`)

Décision produit (validée user) : **grey-box baké headless**, jamais toucher au
château GLB monolithique. Nouveau `tools/blender/author_castle_stone_kit.py`
construit 5 pièces en géométrie primitive (grande salle landmark, entrée, mur
droit, angle, tour), conventions `PCG_*`, proxies **boîtes simples** (jamais
per-mesh). `catalog_pcg_kit.py` **étendu** : émet `bounds_m` (AABB rendu),
`clearance` (props socket) et chemins d'asset **relatifs à `assets/`**
(Bevy-ready, ferme G4) ; socket-id via prop `pcg_socket_id` (évite les
`west.001` de la dé-duplication Blender).

Artefacts produits (`assets/`) :

- `source/castle_stone_greybox.blend` (source reproductible)
- `pcg/kits/castle_stone/1.0.0/kit.toml` + **15 GLB** (5 render, 5 lod1, 5 proxy)
- `pcg/specs/hall_highlands.content-spec.toml`
- `pcg/registry.lock.toml` (SHA-256 réel du kit.toml)

Repères prouvés respectés : `export_apply=False`, Cycles-non-EEVEE (rendu de
contrôle non lancé cette passe), conversion Blender→Forgia `(x,z,-y)`, offset
ancré ROOT, proxies simples.

### 6. Preuve end-to-end sur assets shippés

`crates/forgia-pcg-core/tests/castle_stone_slice.rs` (`include_str!` des contrats
réels) : parse spec+kit → `assemble_socket_chain` (5 pièces, 4 bindings, tous
raw-opposés — contrainte V1) → `compute_stream_cells` → `validate_spatial_plan`
**is_valid** (5 collision proxies ≤ 32, clearance capsule déférée). Prouve
spec→kit→solveur→SpatialPlan→cellules→validateurs sur les vraies données.

### 7. Adapter runtime Bevy — `crates/forgia-pcg-runtime` (isolée, PAS encore câblée)

`ForgiaPcgStreamPlugin` + `PcgStreamPlan::build(plan, manifest, streaming)`
(résout render/collision assets par instance) + driver `drive_pcg_stream` :
préchargement de la cellule destination + voisins, une marche de ladder par frame
(collision→rendu, inverse au déchargement). Surface Bevy minimale (Resources +
1 système), **3 tests headless** (`App::new()`) prouvent l'ordonnancement réel.
Non enregistrée dans le plugin-set de l'app live (worktree en churn 230 fichiers) :
la couche jeu consomme `PcgStreamState::transitions` pour le spawn/collider/visibilité
concret — **c'est le prochain jalon**.

### 8. Validations réellement exécutées (vrai cargo, bypass RTK)

- `cargo test -j 1 -p forgia-pcg-core -p forgia-procgen-graph -p forgia-pcg-runtime`
  : **vert** — 33 tests (core 22 + slice e2e 1 + runtime 3 + procgen-graph 7), 0 échec.
- `cargo clippy -j 1 -p forgia-pcg-core -p forgia-pcg-runtime -p xtask --all-targets -- -D warnings`
  : **vert** (0 warning).
- `cargo xtask validate-pcg --require-any` : **vert** (content-specs=1,
  kit-manifests=1, registry-locks=1, failures=0).
- `cargo xtask arch-drift` : **vert** (63 crates ; j'ai ajouté `forgia-pcg-core`
  — que Codex avait oublié — et `forgia-pcg-runtime` à ARCHITECTURE.md).
- `cargo xtask no-scaffold` : **vert** (0 violation).

### 8b. Reprise (même session) — jalons 2 et 3 livrés

**Publisher hash (jalon 2)** : `validate-pcg` recalcule désormais le SHA-256 de
chaque manifest référencé par `registry.lock.toml` et refuse divergence, fichier
manquant ou chemin hors racine (`xtask::verify_lock_hashes`, dep `sha2` locale à
xtask). Preuves : vert sur les vrais assets (failures=0) ; **FAIL prouvé** sur
copie sabotée (hash déclaré vs réel affichés, exit 1) ; clippy xtask 0 warning.

**Rendu Cycles de contrôle (jalon 3)** : transforms du solveur figés en **golden
test** (`castle_stone_slice.rs`) → `previews/slice_layout.json` →
`tools/blender/render_pcg_kit_preview.py` → `previews/slice_34.png` +
`slice_top.png` (l'assemblage 5 pièces est visible et conforme au plan). Le
rendu a attrapé **2 vrais défauts**, corrigés :

- **Normales inversées** dans les boîtes authored (`from_pydata` brut) — fatal en
  jeu (StandardMaterial Bevy single-sided). Fix : bmesh +
  `recalc_face_normals` dans l'author ; GLB re-cuits (kit.toml/hash inchangés,
  les normales ne sont pas dans le manifest).
- **Originaux non cachés au rendu** : les pièces sources sont toutes à l'origine
  → elles rendaient empilées au centre du layout. Fix : `hide_render` sur tout
  objet non-PREVIEW.

Pièges renderer en plus : `render.filepath` **relatif** = écriture silencieusement
perdue en headless (Blender affiche pourtant « Saved ») → chemins `resolve()`
obligatoires ; l'API World de Blender 5 (`use_nodes` déprécié) rend le réglage
d'ambiance non fiable → 2 suns (clé + fill opposé) pour ne jamais avoir de face
noire.

### 9. Prochain jalon concret

1. **Câbler `ForgiaPcgStreamPlugin` dans l'app live** (quand le worktree se calme) :
   systèmes jeu qui consomment `PcgStreamState::transitions` → charge GLB +
   spawn caché (Preloaded), insère proxy collision box + marqueur navmesh
   (Physics), `Visibility::Visible` (Rendered) ; inverse au déchargement. Ajouter
   le sensor `forgia2_pcg_stream.json` (observabilité) à ce moment.
2. ~~Publisher offline SHA-256~~ — **FAIT** (§8b, `verify_lock_hashes`).
3. ~~Rendu Cycles de contrôle~~ — **FAIT** (§8b) ; reste `gltf-transform` pour
   les budgets triangles/VRAM offline (peu urgent sur du grey-box).
4. Étendre le solveur V1 au-delà des sockets raw-opposés (policy quaternion +
   clearance) pour les vrais coins/escaliers — garder la garde `NonPlanarSocket`.
