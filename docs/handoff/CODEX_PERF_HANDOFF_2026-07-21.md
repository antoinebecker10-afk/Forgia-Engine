# Handoff — performance, freezes et Tracy

Date : 2026-07-21

## Objectif

Rendre le Roguelite stable à 60 FPS sans freezes perceptibles. Le workspace est
sale : conserver les modifications utilisateur non liées, surtout les assets et
le viewmodel des bras.

## Résultats runtime confirmés

- La moyenne de frame est bonne (environ 4 à 6 ms, souvent 135 à 240 FPS), mais
  des hitches de 46 à 196 ms ont été reproduits en Roguelite.
- Le warm-up PBR fonctionne désormais : `forgia2_pipeline_warmup.json` indique
  `done=true`, `classes_spawned=52`, `scenes_ready=52`, `frames_to_ready=90`.
- Les hitches ne sont pas des compilations de pipelines : sur la dernière passe,
  `forgia2_load_timing.json` indique `pipeline_waiting=0` et
  `pipeline_waiting_delta=0` pour les freezes non liés au spawn.
- Un hitch de spawn GLTF a été confirmé (+60 entités, +24 colliders, 85 ms).
- Cause majeure confirmée : fuite mémoire du fond vidéo du menu. Windows a
  mesuré environ 16 Go de working set/private bytes alors que le menu ne
  comportait que 150 entités. `forgia2_memory.json` a confirmé 15,2 Go.

## Cause de la fuite mémoire menu

`crates/forgia-ui/src/menu_video.rs` est un player de 361 frames WebP.
Le HashMap LRU retirait les handles hors fenêtre, mais chaque appel à
`EguiContexts::add_image(EguiTextureHandle::Strong(...))` conservait un handle
fort dans le registre egui. Les frames évincées restaient donc décodées en
RAM/VRAM pendant tout le cycle vidéo.

Correction appliquée dans `ensure_menu_video_frame` : avant le `retain`, les
AssetId des frames évincées sont collectés puis passés à
`contexts.remove_image(image_id)`. Après cela, les handles sortent réellement
du cache Bevy.

Validation attendue : lancer le jeu, rester 60 s au menu (plus d'un cycle vidéo),
puis relire `forgia2_memory.json` et la mémoire Windows. La RAM ne doit plus
croître continuellement ni dépasser le budget normal du jeu.

## Modifications Codex appliquées

### Robustesse et assets

- `crates/forgia-mode-roguelite/src/persist.rs`
  - Une sauvegarde TOML corrompue est renommée en
    `<nom>.corrupt-<timestamp>` avant fallback ; évite la perte silencieuse de
    progression.
- `crates/forgia-assets/src/lib.rs`
  - `GameAssets` précharge les deux bras viewmodel et les 8 scènes de warm-up
    (squelettes, sols, portails).
- `crates/forgia-viewmodel/src/arms.rs`
  - Utilise les handles `GameAssets`, plus de chargement GLTF au premier spawn.
- `crates/forgia-mode-roguelite/src/pipeline_warmup.rs`
  - Utilise les handles préchargés.
  - Le spawn du diorama de warm-up est réessayé en `Update` tant qu'une caméra
    3D active n'existe pas. Avant, l'ordre de transition pouvait laisser
    `classes_spawned=0` définitivement.
- `crates/forgia-mode-roguelite/src/hub.rs` et `meta_shop.rs`
  - Le bouton/touche Entrée de lancement attendent `WarmupState.done` et
    affichent « Préparation de la Forge… ». Le joueur ne peut plus entrer en
    combat avant le warm-up.
- `crates/forgia-mode-roguelite/src/decor.rs`
  - Ajoute `Visibility::Visible` aux parents de props, pour corriger les
    warnings Bevy B0004 de hiérarchie de visibilité.

### Observabilité des freezes

- `crates/forgia-effects/src/pipeline_ready.rs`
  - Ajoute `PipelineCacheStats { waiting }`, copié du `RenderApp` vers le monde
    principal chaque frame.
- `crates/forgia-mode-roguelite/src/load_timing.rs`
  - Les freezes reportent désormais `pipeline_waiting` et
    `pipeline_waiting_delta`.
  - Attribution : `scene_spawn_gltf`, `colliders_rapier`,
    `pipeline_cache_pending`, ou `unattributed_cpu_or_gpu`.
  - Ne plus conclure « shader compile » quand le cache est vide.
- `docs/observability/SENSOR_REGISTRY.md` et
  `xtask/asset-load-allowlist.toml` ont été mis à jour afin que les gates passent.

### Profiling Tracy et build dev

- `Cargo.toml` (racine) : feature `profile-tracy` qui propage
  `forgia-game/profile-tracy`.
- `crates/forgia-game/Cargo.toml` : `profile-tracy = ["bevy/trace_tracy"]`.
- `.cargo/config.toml` :
  - alias `cargo forgia-dev` et `cargo forgia-fast` avec Tracy ;
  - `[build] jobs = 2`, indispensable pour éviter l'OOM de Bevy + wgpu + Tracy.
- `run_debug.ps1` : build automatiquement `release-fast` avec Tracy et `-j 2`.
- `README.md` : commandes de dev/profiling documentées.
- Feature mémoire ajoutée après les mesures : `profile-tracy-memory` propage
  `bevy/trace_tracy_memory`. Commande : `cargo forgia-memory`. Elle est réservée
  à l'enquête allocation/rétention (pas aux mesures FPS, qu'elle perturbe) et
  compile avec `cargo check -j 2 -p forgia --profile release-fast --features
  profile-tracy-memory`.

Important : ne pas utiliser `cargo forgia-fast --help` pour tester l'alias : les
arguments après `--` sont passés au jeu et la commande déclenche réellement un
build. Un ancien build Codex ainsi lancé a retenu le lock Cargo ; il a été arrêté.

## Commandes de reproduction

Fermer le jeu et tous les builds Cargo concurrents, puis :

```powershell
cargo build -j 2 -p forgia --profile release-fast --features profile-tracy
.\target\release-fast\forgia.exe
```

Ou :

```powershell
.\run_debug.ps1
```

Lancer Tracy Profiler avant le jeu. Tracy ne crée pas automatiquement un fichier
local : il transmet sa capture au viewer. Faire une run de 60 à 90 s, puis
sauvegarder la capture Tracy et inspecter les systèmes / threads avec les pics
de 45 ms ou plus.

## Gates déjà validées après les modifications

- `cargo check` ciblé sur les crates modifiées.
- `cargo clippy -- -D warnings` ciblé sur les crates modifiées.
- `cargo test -p forgia-mode-roguelite --lib` : 271 tests verts.
- `cargo test -p forgia-effects --lib` : 12 tests verts.
- `cargo test -p forgia-ui --lib` : 3 tests verts.
- `cargo xtask asset-load` : vert.
- `cargo xtask sensor-audit` : vert.
- `git diff --check` : vert (des warnings CRLF Git sont attendus).

## Travail restant prioritaire

1. Refaire la mesure mémoire après le correctif `remove_image` et confirmer que
   le menu ne fuit plus.
2. Refaire une capture Tracy, isoler les hitches `unattributed_cpu_or_gpu`.
3. Corriger uniquement les systèmes qui apparaissent effectivement chauds dans
   Tracy. Ne pas ajouter de warm-up spéculatif : les pipelines ont été innocentés
   par le compteur `PipelineCacheStats`.
4. Examiner le spawn GLTF/colliders confirmé et l'étaler davantage si le hitch
   apparaît encore durant les vagues.

## Mesure complémentaire du 21 juillet (instance propre)

- Une instance `release-fast + profile-tracy` a été lancée sans interaction et
  arrêtée après environ 45 s. À 37 s, le capteur mesurait **2,0 Go RAM**
  (`Private Bytes` Windows : **3,9 Go**) ; le pic précédent à 17,7 Go n'a donc
  pas été reproduit sur ce run. Il reste un risque à mesurer sur une session
  longue/jouée, et ne doit pas encore être déclaré corrigé.
- Le cache menu reste borné : `cache_size=8`, `frames_loaded=8`.
- Les hitches persistent à vide : 55–68 ms à environ 32–36 s, avec zéro ennemi,
  zéro delta d'entités/colliders et `pipeline_waiting=0`. Les deux hitches de
  chargement scène, eux, sont confirmés : 46 ms (+81 entités) et 56 ms (+53
  entités, +20 colliders).
- Hypothèse prioritaire à vérifier dans Tracy : surcharge périodique des
  nombreux capteurs synchrones qui font des `std::fs::write` sur le thread de
  jeu. Ce n'est pas encore une cause confirmée ; profiler avant correction.

### Test A/B du capteur vidéo menu

- `crates/forgia-ui/src/menu_video.rs` écrit désormais à 2 s seulement lorsque
  le menu est actif, puis à 30 s hors menu (avec export immédiat à chaque
  transition d'état). Le changement est couvert par un test ; `cargo test -p
  forgia-ui --lib` et Clippy ciblé sont verts.
- L'A/B invalide ce capteur comme cause principale : avec le nouvel intervalle,
  le dernier export était à 39 s, mais des pics de 64–70 ms sont restés présents
  de 42 à 45 s, sans delta d'entités/colliders ni pipeline en attente.
- Cette réduction d'I/O reste saine, mais ne pas la présenter comme le fix des
  freezes. La capture Tracy est désormais nécessaire pour attribuer le hot path.

## Capture Tracy `C:\Users\Antoi\Desktop\Tracy\1.tracy` (2026-07-21)

La capture a été exportée avec `tracy-csvexport`. Le mode
`profile-tracy-memory` dégrade volontairement les FPS et ne sert pas de benchmark,
mais il a révélé une cause de hitch de chargement **confirmée** : Bevy 0.18
exécute `Mesh::generate_tangents()` (Mikktspace) pour des GLB avec normal maps
mais sans attribut `TANGENT`.

Pires appels uniques :

- `portal_open.glb` : **1 317 ms** ; `portal_closed.glb` : **1 204 ms** ;
- `madame_lenoir.glb` : **1 164 ms** ;
- `boucherie.glb` : **656 ms** ; `bourrasque.glb` : **639 ms** ;
  `pepin.glb` : **532 ms** ;
- `Gobli.glb` : **636 ms**.

Preuve code Bevy : `bevy_gltf-0.18.1/src/loader/mod.rs:793-809` déclenche
Mikktspace lorsque UV + normales sont présents, que le matériau en a besoin, et
que les tangentes manquent. Correction durable recommandée : réexporter ces GLB
avec tangentes calculées dans Blender/DCC, puis valider visuellement les normal
maps et vérifier que les zones `generate_tangents` disparaissent d'une capture
CPU Tracy normale. Ne pas désactiver la génération runtime sans tangentes : cela
dégrade le rendu des normal maps.

### Correction appliquée et validée

- Blender 4.5 LTS a été installé via Winget. Le script reproductible
  `tools/blender/reexport_glb_with_tangents.py` réimporte un GLB et le réexporte
  avec `export_tangents=True`.
- Assets réexportés : les deux portails, les quatre armes Forgia, `Gobli.glb`,
  `one_file_assets.glb`, `platformer_underworld.glb`, `ChestBig_001.glb`,
  `Coin_001.glb` et `GoldPileSmall_001.glb` (12 GLB).
- Chaque export a été validé avant remplacement : mêmes primitives, matériaux
  et animations ; `TANGENT` présent pour toutes les primitives. Les sauvegardes
  originales sont hors workspace dans
  `C:\Users\Antoi\Desktop\Forgia Asset Backups\tangents-2026-07-21_1614`
  (112,56 Mo), donc le retour arrière est une restauration de ces fichiers.
- Validation runtime : lancement avec `RUST_LOG=warn,bevy_gltf=debug`, puis
  analyse de `Missing vertex tangents`. **Zéro** signalement pour les 12 assets
  convertis et zéro calcul de tangentes durant la session chargée.

## Capture Tracy `C:\Users\Antoi\Desktop\Tracy\2.tracy` (2026-07-21)

Capture CPU normale `release-fast + profile-tracy`, analysée avec
`tracy-csvexport` :

- la moyenne est saine (~241 FPS, ~4,75 ms) et le GPU a beaucoup de marge
  (~0,52 ms mesurés par les pass GPU) ;
- les hitches restent CPU : le pire `PostUpdate` atteint **96,8 ms** ; sa cause
  directe est `bevy_transform::systems::parallel::propagate_parent_transforms`
  à **95,2 ms** ;
- le capteur confirme les mêmes pics (jusqu'à 100 ms) et environ 4 700–5 500
  entités pendant la run. Ce n'est donc ni une saturation GPU ni le calcul de
  tangentes désormais supprimé ;
- les chargeurs GLTF restent coûteux sur workers (jusqu'à 516 ms pour
  `boucherie.glb`, 493 ms pour `pepin.glb`) mais ne sont pas, à eux seuls, la
  durée du frame hitch. Leur instanciation doit rester étalée.

### Correction : warmup GLTF étalé

`crates/forgia-mode-roguelite/src/pipeline_warmup.rs` ne spawn plus les 52
`SceneRoot` de warmup sur une seule frame. Il prépare une file dédupliquée et
instancie **2 scènes par frame**. Le gate attend explicitement que toute la
file soit drainée et que toutes les scènes aient envoyé `SceneInstanceReady`
avant de pouvoir déclarer le warmup prêt. Ainsi, le warmup conserve sa fonction
(éviter les compilations de pipelines en combat) sans créer une très grande
hiérarchie de transforms d'un coup au Lobby.

Validation : `cargo check -j 1 -p forgia-mode-roguelite` vert et
`cargo test -j 1 -p forgia-mode-roguelite pipeline_warmup` : 4 tests verts.
Une tentative de test `release-fast -j 2` a échoué dans `bevy_audio` par manque
de mémoire LLVM ; ce n'est pas une erreur de code. Ne lancer qu'un seul build
Cargo à la fois (`-j 1` ou attendre la fin) : deux builds concurrents avaient
créé un verrou `target` et ont été arrêtés, sans toucher aux processus utilisateur.

## Capture Tracy `C:\Users\Antoi\Desktop\Tracy\4.tracy` (2026-07-21)

La correction I/O est validée par mesure : 17 capteurs de
`forgia-observability` utilisent désormais `forgia_core::sensor_io`, une file
bornée écrite par un unique thread de fond. Dans la capture précédente, une
rafale synchronisée d'environ 45 `std::fs::write` avait fait prendre 2–8 ms à
chaque capteur. Après migration, les capteurs concernés sont sous **0,1 ms**
(rendu 0,093 ms, health 0,092 ms, lag-events 0,086 ms, etc.).

Le problème de frame restant est donc confirmé indépendant de ces écritures :
`bevy_transform::systems::parallel::propagate_parent_transforms` reste le
premier hot path à **64,1 ms** (contre 74,6 ms dans `3.tracy`). Le prochain
chantier doit mesurer puis réduire/flatten les hiérarchies `SceneRoot` statiques
qui alimentent ces 368 860 jobs de propagation ; ne pas accuser le game-feel,
le GPU, ni les capteurs migrés sans nouvelle preuve.

## État du worktree

Le worktree contenait déjà des modifications utilisateur importantes : assets
bras, previews Blender, `style.rs`, `arms.rs`, `calibration.rs`, `genome.rs` et
documents Story-661. Ne pas les annuler ni les reformater globalement. Aucune
modification n'a été commitée par Codex.

## Suite de session — crash natif et protocole mémoire (22:48 locale)

### Faits confirmés

- Une run `release-fast` a terminé par `0xc0000005` (`STATUS_ACCESS_VIOLATION`)
  après environ 44 s. C'est une exception native Windows, pas un `panic` Rust.
  La dernière ligne Rust (`forgia_viewmodel::attach::auto_scale_viewmodel`) est
  un point de synchronisation de log, **pas** une causalité établie ; ce système
  ne contient ni `unsafe` ni pointeur natif.
- Forcer `WGPU_BACKEND=dx12` n'a pas éliminé le crash : ne pas l'attribuer à
  Vulkan ni à Tracy sans capture/mini-dump qui nomme un module fautif.
- Juste avant l'arrêt, `forgia2_memory.json` rapportait 14 656 MiB de mémoire
  de processus pour environ 2 138 entités. C'est un P0 à investiguer. Cette
  mesure vient de `sysinfo::Process::memory()` lu dans un worker dédié ; elle
  n'est pas une estimation par comptage ECS.
- Les freezes restants (50 à 88 ms) sont toujours présents avec
  `pipeline_waiting=0`. Le spawn de scène initial est, lui, séparément
  attribué à `scene_spawn_gltf` (+104 entités, ~50 ms).

### Correctifs appliqués pendant cette suite

- `crates/forgia-mode-roguelite/src/waves.rs` : plafond de quatre rigs animés
  complets par vague ; les ennemis suivants conservent toutes leurs composants
  de gameplay mais utilisent un proxy visuel statique. C'est une réduction
  mesurée de la propagation de transforms, pas un LOD dynamique final.
  Validation : `cargo check -p forgia-mode-roguelite -p forgia`, six tests de
  vagues et l'audit `sensor-sync-writes` étaient verts à l'application.
- `crates/forgia-observability/src/lib.rs` : le module existant
  `vram_sensor` est désormais enregistré dans `ForgiaObservabilityPlugin`.
  Il écrit toutes les cinq secondes `forgia2_vram.json` : total estimé,
  nombre d'images/meshes, et top-10 de chaque type. Le capteur n'effectue pas
  d'appel driver et n'est pas dans le hot path. `cargo check -j 1 -p
  forgia-observability -p forgia` est vert après ce branchement.

### Protocole réutilisable — avant toute nouvelle optimisation

1. Lancer une seule instance sans Tracy pendant 60 s, puis relever
   `forgia2_memory.json`, `forgia2_vram.json`, `forgia2_load_timing.json` et
   `forgia2_perf_diag.json`. Ne pas lancer deux builds Cargo simultanés.
2. Si RAM ou VRAM croît continûment, noter les top assets de
   `forgia2_vram.json`, puis comparer avec une run menu immobile et une run
   combat. Ne pas supprimer/compresser des assets au hasard.
3. Si un freeze dépasse 30 ms et `pipeline_waiting=0`, faire une capture Tracy
   normale. Chercher le système dominant sur la frame, pas seulement la
   dernière ligne de console.
4. Si un crash `0xc0000005` revient, conserver l'heure exacte, le backend et
   les quatre JSON ci-dessus ; chercher un mini-dump récent avant de conclure
   sur le driver ou un système Rust.

### Collaboration Codex / Claude

Ce fichier est la mémoire partagée : chaque agent doit y ajouter les
modifications réellement appliquées, les commandes de validation et le niveau
de preuve (confirmé / hypothèse). Il ne faut pas s'appuyer sur la mémoire
conversationnelle d'un agent. Avant d'éditer, lire ce handoff et `git diff`,
préserver le worktree sale, puis compléter la section de suite après toute
mesure ou correction importante.

## Session Claude — enquête crash OOM + capteur VRAM (2026-07-21 ~23:20 locale)

Aucune modification de code appliquée cette session (diagnostic seul). Le build
`release-fast` a seulement été relinké pour embarquer le capteur VRAM déjà
enregistré (`cargo build -j 1 -p forgia --profile release-fast`, EXIT 0 ; vérif
`cargo build` brut « Finished in 3.74s »). Captures brutes conservées dans
`docs/handoff/captures/` (menu_idle, crash_repro, mem_trajectory).

### Faits CONFIRMÉS (mesurés)

1. **Le crash `0xC0000409` est un abort OOM Rust, PAS un fault driver/Vulkan/DX12
   ni un panic.** stderr capturé (`captures/crash_repro/stderr.log`) :
   `memory allocation of 4194304 bytes failed` →
   `std::alloc::rust_oom` → `handle_alloc_error` → `alloc::raw_vec::handle_error`
   → `<bevy_ecs::FunctionSystem>::run_unsafe` (worker thread multi_threaded).
   L'alloc qui échoue ne fait que **4 Mo** : la limite de commit est atteinte, le
   4 Mo est la goutte d'eau. `forgia2_crash.json` absent → panic hook jamais
   déclenché → ce n'est pas un panic qui unwind. `release-fast` = `panic=unwind`
   (hérite de `release`, aucun `panic=abort`).
2. **La limite est le COMMIT système, pas la RAM physique.** Machine : 32,5 Go RAM
   (18 Go libres au moment du test), **commit limit 46 Go** (pagefile 13,5 Go),
   **déjà ~30 Go committés au repos hors forgia** → ~15,7 Go de marge seulement.
   Une alloc de 4 Mo peut donc échouer avec 18 Go physiques libres.
3. **`sysinfo::Process::memory()` = WORKING SET (RAM résidente).** Calibré :
   `ram_bytes=2142457856` du capteur == `WorkingSet64` mesuré par
   `Get-Process`. Donc le **14,7 Go de `forgia2_memory.json` était 14,7 Go de RAM
   résidente réelle** (run combat 4736 entités), pas un artefact de mesure.
4. **À l'idle/vagues légères (~2200 entités), forgia est SAIN et stable** :
   WorkingSet ~2,0–2,1 Go, Private/commit ~3,3 Go, plat sur 80 s (run
   `mem_trajectory`, aucun crash). Le « 14 656 Mo » lu au début d'une run était le
   fichier stale de la run précédente (le nouveau process ne l'avait pas encore
   réécrit).
5. **VRAM/assets estimés SAINS : ~1,03 Go** (`forgia2_vram.json`, 148 images /
   522 meshes). Top offenders = textures **`portal_open.glb` 2×4096² = 85 Mo
   chacune**, plusieurs 2048² à 21 Mo (portal_closed, Gobli). Pas la cause des
   14 Go. `top1_share=0.08`, aucune texture ne domine.
6. **Freezes `unattributed_cpu_or_gpu` (47–95 ms, `pipeline_waiting=0`)
   reconfirmés.** Le nouveau capteur `forgia2_transform_lag.json` attribue les
   racines des transforms modifiés pendant le hitch aux **enemies tank**
   (`RogueliteEnemy_W2_tank_*`, 45 transforms changés chacun) + Player →
   cohérent avec `propagate_parent_transforms` de Tracy 2/4.tracy.

### Cause la plus probable (P0 mémoire/crash)

**Bloat/fuite mémoire proportionnel au combat lourd** : de ~2 Go (idle, 2200
entités) à 14,7 Go (combat, 4736 entités) ≈ **~3 Mo/entité**, anormal. Ce n'est
PAS les assets de base (~1 Go, stables) ni un `Vec` à capacité corrompue (l'alloc
fatale ne fait que 4 Mo). Combiné à la marge de commit machine faible (~15 Go),
ce bloat fait dépasser la limite de commit → abort OOM. Les crashs idle #1/#2
(28 s / 5,7 s) se sont produits quand la machine était déjà proche de la limite
(résidus de commit des runs précédentes + autres process) ; la run #3, plus tard,
avec plus de marge, n'a pas crashé en 80 s.

Suspects du bloat combat (NON tranchés — requièrent la mesure combat) :
- **Colliders Rapier trimesh non partagés** : 1830 colliders dans la run 4736 ;
  un trimesh par ennemi copie les vertices du mesh (Gobli 28 852 verts ≈ ~1 Mo)
  → centaines de Mo à Go. À mesurer.
- Duplication mesh/skin par instance d'ennemi (le cap 4 rigs limite l'anim, pas
  forcément la mémoire des proxies).
- Accumulation VFX/particules ou assets runtime par salle non libérés.

### Prochain test NÉCESSAIRE (bloquant pour trancher la source)

Run **combat multi-salles** (~2–3 min, franchir portails jusqu'à sévérité
`critical` du capteur mémoire, ~4000+ entités) avec échantillonnage
WorkingSet/Private/`forgia2_vram.json`/entités/colliders. Décision :
- si `forgia2_vram.json total_estimated_mb` grimpe avec `images_count`/
  `meshes_count` → duplication d'assets (trouver le site) ;
- si VRAM reste ~1 Go alors que WorkingSet grimpe → coût hors-assets (Rapier
  colliders / structures CPU / VFX). Corréler `colliders_now` (load_timing) au
  WorkingSet.
Script prêt : `docs/handoff/captures/` (voir session). Ne rien « fixer » en
mémoire avant cette attribution (règle no-speculative-fix).

### Capture Tracy `14.tracy` (2026-07-21 23:33, run combat ~130 s)

Exportée avec `tracy-csvexport.exe`
(`...WinGet\Packages\wolfpld.tracy_*\tracy-csvexport.exe`) →
`docs/handoff/captures/tracy14_zones.csv` (3538 zones).

**Mémoire de ce run : SAINE, pas de crash.** `forgia2_memory.json` = 3,2 Go,
`forgia2_vram.json` = 916 Mo (128 img / 565 meshes). Run resté ~1857–2187
entités → le pic 14,7 Go n'a PAS été reproduit (multi-salles lourd non atteint).
Le P0 mémoire reste ouvert : refaire une run poussée pour l'attribuer.

**Freeze CPU confirmé (zones, colonnes ...,total_ns,total_perc,counts,mean_ns,
min_ns,max_ns,std_ns) :**

| Zone | %CPU | appels | moy | **max/frame** |
|---|---|---|---|---|
| `bevy_transform::systems::parallel::propagate_parent_transforms` | 3,6 % | 35005 | 0,13 ms | **80,0 ms** |
| `sync_simple_transforms` | 1,0 % | 35005 | 0,04 ms | 4,5 ms |
| `mark_dirty_trees` | 0,4 % | 27080 | 0,02 ms | 1,64 ms |
| `update` (schedule complet) | 94 % | 27078 | 4,5 ms | **255 ms** (frames de spawn) |
| `par_for_each(&Mesh3d,&mut Aabb) Changed/AssetChanged` (recalc AABB) | 6,9 % | 1,63 M | — | 0,36 ms |
| `par_for_each &mut ViewVisibility` | 1,45 % | 6,52 M | — | 0,46 ms |

Deux familles de hitches : (1) **spawn/room-load** → `update` ≤ 255 ms
(instanciation SceneRoot GLTF + colliders Rapier + 1ère propagation) ; (2)
**combat sans spawn** → `propagate_parent_transforms` ≤ 80 ms (moyenne 0,13 ms,
bimodal). Le coût *steady* le plus lourd = recalcul d'AABB (6,9 %) — pas un
freeze mais candidat d'optimisation (meshes marqués `Changed` en boucle ?).

**Correction au plan Tracy 4** : `forgia2_transform_lag.json` (au hitch t=119,82,
dt 62,8 ms) attribue les racines *dirty* aux **ennemis animés**
(`RogueliteEnemy_W1_runner_0`/`_tank_1`, 45 transforms changés chacun) + Player,
PAS aux décors statiques. Aplatir les hiérarchies statiques de warmup
n'adressera donc pas les spikes de combat ; la piste réelle = les squelettes
d'ennemis animés (root qui bouge chaque frame → re-propagation du sous-arbre).
Enquête Bevy 0.18 idiomatique en cours (comportement exact de
`propagate_parent_transforms` O(dirty) vs O(taille sous-arbre)).

## Session Claude — « le jeu se ferme » RÉSOLU : build Tracy sans profiler = OOM (2026-07-23 ~04:45 locale)

### Cause racine CONFIRMÉE (niveau de preuve : confirmé, mesuré)

**L'exe quotidien `target\release-fast\forgia.exe` (rebuild 2026-07-23 03:12:28)
était compilé avec la feature `profile-tracy`.** Preuves directes : le process
**écoute sur le port TCP 8086** (port Tracy) et le binaire contient 38 strings
`tracy`. Sans profiler connecté, tracy-client (mode par défaut, pas d'on-demand)
**accumule tout l'historique de zones en RAM** en attendant une connexion →
fuite linéaire mesurée **~220-260 Mo/s dans TOUS les états** (menu, hub, in-game)
→ la marge de commit machine (~15 Go, cf §2 du 21/07) s'épuise → abort OOM
`0xC0000409` (`memory allocation of 8388608 bytes failed`) en 20-90 s selon la
pression ambiante. C'est le « jeu se ferme » signalé par l'utilisateur.

### Chaîne d'élimination (mesures, aucune spéculation)

1. Trace de mort : alloc 8 Mio refusée dans le loader async ; RAM capteur
   17 841 Mo à t=88 s au menu (`forgia2_memory.json` severity critical).
2. Vidéo menu HORS DE CAUSE : pipeline rendu inerte (dossier frames vide →
   fallback 361, aucun décodage possible) → fuite inchangée (~270 Mo/s).
   NB : le « fallback dossier vide » documenté est inatteignable — un dossier
   vide retombe sur la const 361, jamais sur frame_count=0.
3. Cellules château HORS DE CAUSE : `cells=1/46`, desc=428, meshes=226
   constants pendant que Priv grimpe linéairement (run instrumenté 5 s).
4. Assets HORS DE CAUSE : `forgia2_vram.json` stable ~1 Go (134-165 images,
   283-337 meshes) pendant la montée.
5. I/O disque HORS DE CAUSE : `ReadTransferCount` plat (0,0 Mo/s après t=10 s)
   pendant +222 Mo/s de Priv → allocation interne pure.
6. Port 8086 + strings binaire → Tracy compilé. Les runs du 21 étaient plates
   avec Tracy CONNECTÉ (il streame au lieu d'accumuler) ; les « crashs idle »
   #1/#2 (28 s / 5,7 s) du 21 s'expliquent pareil : runs Tracy sans GUI attaché.

### Correctif appliqué

Rebuild du binaire quotidien SANS la feature :
`cargo build -j 1 -p forgia --profile release-fast` (cargo réel, pas le wrapper
rtk). Validation ci-dessous après rebuild : port 8086 absent + RAM plate 60 s au
menu.

### Règles pour éviter la récidive

- **Un build `--features profile-tracy` n'est JAMAIS le binaire quotidien.**
  Après toute session Tracy, rebuilder sans features avant de rendre la main.
- Lancer une session Tracy = GUI Tracy connecté AVANT d'être en jeu longtemps ;
  sans GUI, l'accumulation mémoire fausse toute mesure et tue le process.
- Suivi souhaitable (non appliqué, zone WIP autre terminal) : activer l'option
  `on-demand` de tracy-client dans le wiring `profile-tracy` pour que les runs
  non connectés n'accumulent rien.

## Session Claude (suite) — hub château : rayon, double-binding F10, sondes (2026-07-23 soir)

Trois correctifs appliqués + build vert (`cargo build -j 1 -p forgia --profile
release-fast`, exe 18:27) :

1. **« Je ne vois pas tout le château »** — `castle_hub.rs` :
   `CASTLE_STREAM_RENDER_RADIUS_M` 48→**240** / `UNLOAD` 64→**280**. À 48 m,
   seules 24/46 cellules étaient éligibles depuis le spawn (cellule la plus
   lointaine à 139,6 m ; ~230 m depuis un bord opposé — distribution calculée
   depuis le manifest). Le découpage et la cadence 1 spawn/3 frames restent ;
   si la perf l'exige un jour → HLOD distant, pas de retour au château tronqué.
2. **Double-binding F10 (CONFIRMÉ, gel apparent + tunneling)** —
   `enter_castle_hub_hotkey` (F10, Menu) ET `sys_toggle_collider_debug`
   (F10, **sans garde**, mode-roguelite/lib.rs) : un F10 au menu entrait dans
   le Hall ET allumait le wireframe Rapier du TriMesh château (55 632 tris
   re-poly-lineé chaque frame) → frames de plusieurs secondes (mesuré : logs
   stoppés 0,5 s après l'entrée, CPU actif, `Responding=false`, capteurs figés)
   → et dt physique géant = joueur qui passe à travers les sols (« y'a plus les
   colliders » ressenti, alors que `colliders_ready=true` à 3 s). Fix : toggle
   gardé `run_if(in_state(AppMode::InGame))` — anti-piège documenté « 1 KeyCode
   = 1 handler avec gardes ».
3. **Sondes streaming permanentes** — `stream_castle_visual_cells` logge
   désormais chaque spawn (`spawn cellule X (N déjà chargées / plan 46)`),
   chaque unload (id + distance + player_pos) et un tick d'état ~4 s (chargées /
   dans-le-rayon / cooldown / position). Diagnostic « bloqué à N » lisible dans
   n'importe quel stderr.

Leçon de méthode payée cash : un build background a échoué (E0502) et le run
suivant est parti sur l'exe **stale** — TOUJOURS vérifier `BUILD_EXIT` + mtime
exe + strings des nouveaux logs dans le binaire avant de lancer (règle
« artefact = preuve »).

**Validation restante (user, à son rythme)** : entrer dans le Hall, château
complet attendu en ~1 s ; vérifier ensuite `forgia2_castle_hub.json` →
`streamed_cells` doit atteindre 46 (persiste après la session, pas besoin de
capture stderr). Restent ouverts : calage hauteur herbe (`castle_ground_tune.json`
live, autre terminal) et trous de couverture walkable (chantier calage).

## Session Claude (suite) — VRAI bug streaming trouvé : château tronqué à 2/46 (2026-07-23 nuit)

**Symptôme persistant** : même après rayon 240 + build propre (port 8086 absent,
RAM plate 3 Go), le château restait figé à **2/46 cellules** pendant 60 s+
(mesuré : `forgia2_castle_hub.json streamed_cells=2` stable, `spawn cellule`
loggé 2 fois puis plus rien, `stream tick` JAMAIS loggé). Donc ni Tracy, ni
volume mémoire (les 46 cellules = 113 Mo total sur disque) : **bug logique.**

**Cause racine (CONFIRMÉE, one-liner)** : `place_player_in_castle` faisait
`commands.remove_resource::<CastleVisualStreaming>()` au moment du placement du
joueur (à côté du retrait légitime de `CastleSpawnPending`, la garde one-shot de
placement). Le placement arrive après ~4 frames → la ressource de streaming
disparaît → `stream_castle_visual_cells` sort en early-return à jamais → le
château reste figé aux ~2 cellules chargées dans ces 4 frames. Le log « Joueur
posé après 4 frame(s) » = l'instant exact de l'arrêt. Copier-coller erroné du
terminal château.

**Fix** : retrait de la ligne fautive dans `place_player_in_castle` ; le nettoyage
de `CastleVisualStreaming` déplacé dans `cleanup_castle_hub` (OnExit, symétrique).

**Validé (build 19:53, F10 autonome, 60 s)** : `streamed_cells` 2→**46/46** en
~1 s, 46 spawns, `meshes=8560` (château entier), RAM plate **3,5 Go**,
`frame_avg=4,7 ms` (213 FPS) — **identique à 2 cellules**, `frame_max=16 ms`.

**Anneaux LOD / HLOD : PAS nécessaires ici (décision data-driven).** Le château
entier (8560 meshes, 46 cellules) tient à 4,7 ms sans surcoût mesurable vs 2
cellules (statique + frustum culling). La demande user « chargement par zones
proche/moyen/lointain » était une bonne intuition d'archi mais le vrai problème
était ce bug de ressource, pas le volume. Les anneaux (near=full / medium=HLOD
décimé cuit offline / far=impostor, façon UE5 World Partition) restent le chemin
de **scaling pour des mondes plus grands** — c'est exactement l'archi
`StreamCell` de la méthodologie PCG — mais les construire pour ce château de
113 Mo serait de la sur-ingénierie. À réévaluer par la mesure quand un monde
dépassera le budget frame, pas avant.

**Set complet des fixes hub cette session** : (1) OOM = build sans profile-tracy ;
(2) rayon streaming 48→240 (voir tout le château) ; (3) F10 double-binding gardé
InGame (anti-gel/tunneling) ; (4) **bug ressource streaming** (le principal, château
complet) ; (5) sondes streaming permanentes (`spawn cellule`/`unload`/`stream tick`).

## Session Claude (suite) — collision par-cellule : « je tombe hors dalle de spawn » (2026-07-23)

**Symptôme** : château complet mais on tombe dès qu'on quitte la dalle de spawn.

**Cause racine (CONFIRMÉE, mesurée)** : les GLB de collision offline
(`castle_highlands_collision_runtime.glb` structural + `..._walkable_runtime.glb`)
sont dans un **repère incompatible** avec les cellules visuelles découpées. Bornes
mesurées (parse accessors GLB) : cellules visuelles Y[-107,181] Z[-147,215] ; mais
collision **walkable** Y[-55,**23**] (le sol jouable est à Y≈36,5 → la collision est
~13 m SOUS les pieds), et les deux collisions s'arrêtent à Z≈45 sur un château de
215 m. Seule la boîte manuelle sous le spawn portait → on tombe hors d'elle. Même
piège « frames incompatibles » que l'herbe.

**Fix** : générer la collision **depuis les cellules visuelles elles-mêmes**.
Nouveau `build_streamed_cell_colliders` (castle_hub.rs) : dès qu'une cellule est
instanciée, BFS de ses meshes en **transform local accumulé** (pas GlobalTransform,
non propagé la frame du spawn de scène), fusion positions+indices → UN
`Collider::from_bevy_mesh(TriMesh)` par cellule (≤ 46 colliders — jamais un par mesh
= crash 8052). Aligné **par construction** avec le visuel, quel que soit le repère
d'origine. Additif : les anciens GLB de collision restent (filet), les cellules
ajoutent la couverture précise. 1 build/frame pour lisser le hitch.

**Validé (build 20:48, sonde raycast anneau 24 pts autour du spawn)** :
`walkable_probe_hits` **23/24** dès les 46 cellules chargées (vs ~0 avant hors
spawn), stable 44 s en hub, **0 crash**, RAM/perf saines. Le seul point manquant =
un vrai vide/cour. Nouveau champ capteur `walkable_probe_hits`/`_total` dans
`forgia2_castle_hub.json` pour re-vérifier sans jouer.

**Piège méthodo confirmé** : les « crashs à l'entrée hub » observés pendant le debug
étaient du **bruit de test-harness** (`WScript.AppActivate` répétés envoyant des
inputs parasites `ESC`/clic « Quitter vers le menu ») — PAS un crash du jeu. Une
mesure propre (F10 une seule fois, puis zéro interaction fenêtre, poll du fichier
capteur) a tourné 44 s sans incident. Ne pas conclure « crash » sur un exit sans
signature (0xc0000409/panic/RAM) quand le harness manipule le focus.

Reste ouvert (autre terminal) : calage hauteur herbe + éventuels rares points de
chute (rebords/vides réels, rattrapés par le filet `recover_fallen_castle_player`).
