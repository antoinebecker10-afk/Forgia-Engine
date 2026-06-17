<!-- Audit 2026-06-17 — workflow audit-mode-coupling (6 vecteurs, 48 couplages, 7 agents). Branche pretoday-2026-06-16. -->

# AUDIT — Couplages inter-modes Forgia Rewrite (Bevy 0.18, World ECS unique)

## 1. TL;DR

Forgia tourne dans **un seul World ECS Bevy** où *tous* les plugins de mode (FPS, RPG, Roguelite, CyberCity) sont ajoutés en permanence (`forgia-game/src/lib.rs:91-120`). Bevy n'impose **aucune** isolation entre `GameMode` : la séparation est **100 % discipline** (`run_if(in_state)` + `OnEnter`/`OnExit` + markers). Modifier un mode impacte les autres dès qu'on touche une **Resource globale**, une **caméra partagée** (la FpsCamera/OrbitCamera servent plusieurs modes) ou un **système non-gaté** — car ces objets sont physiquement les mêmes entre modes. Chaque mutation non remise à zéro en `OnExit` = fuite directe dans le mode suivant. Trois fuites HIGH sont confirmées dans le code (lineup PNJ RPG→CyberCity, ClearColor non restauré, arène Roguelite→CyberCity).

## 2. Comment les modes sont reliés

**Machine à états** (`forgia-core/src/lib.rs:38-71`) :
- `AppMode` (Boot/Menu/InGame/Paused) — pilote la vie du **Player** (spawn `OnEnter(InGame)`, despawn `OnEnter(Menu)`, `forgia-player/src/lib.rs:247/252`).
- `GameMode` (None/Fps/Rpg/Roguelite/CyberCity) — pilote quel mode est actif.
- `WorldMode` (Game/Editor/Test) — **machine morte**, init mais zéro consommateur (`forgia-core/src/lib.rs:64-70`).
- `RunState` (SubState source `GameMode::Roguelite`, `forgia-mode-roguelite/src/run.rs:29-44`) — **seul** mécanisme d'isolation réellement imposé par le moteur (Bevy retire le SubState à la sortie du mode source).

**Schéma de transition** : tout switch passe par le menu — `Quit → GameMode::None + AppMode::Menu → despawn_player`, puis `sélection → GameMode::X + AppMode::InGame → respawn_player` (`forgia-ui/src/lib.rs:208-225`). Ce passage forcé par `None` fait *tirer* tous les `OnExit`, ce qui **neutralise par chance** beaucoup de fuites — mais rien dans le moteur ne le garantit, et le respawn du Player masque les fuites attachées à son sous-arbre (cascade despawn enfants).

**Le World, le render-graph, `RapierPhysicsPlugin`, la chaîne d'ordering `GameSet` (`forgia-core/src/lib.rs:79-91`), l'`AssetServer` et `ImagePlugin` sont tous uniques et partagés.** Il n'existe **aucun hub de cleanup central** : chaque crate enregistre indépendamment son `OnExit` (~15 fichiers).

## 3. Vecteurs de couplage

### 3.1 Resources globales mutées par du code mode-spécifique

| Couplage | Où (file:line) | Mécanisme | Isolation | Risque |
|---|---|---|---|---|
| **ClearColor** écrasé par biome Roguelite, jamais restauré | `forgia-stage/src/lib.rs:1161` (write) ; boot `forgia-game/src/lib.rs:132` | `spawn_stage_arena_on_request` fait `insert_resource(ClearColor(sky_color))` par biome ; `cleanup_stage_arena` ne réinsère JAMAIS le boot (0.35,0.22,0.18). MenuCamera2d = `ClearColorConfig::None` → le global transparaît | leaky | **HIGH** |
| **RpgVillageAnchor** jamais retiré → lineup PNJ fuite | `forgia-rpg/src/lib.rs:602` (insert) ; `cleanup_world:2125-2140` ne le liste pas (vérifié : retire ChunkManager/BiomeMap/… **pas** RpgVillageAnchor) | L'anchor stale + `LineupSpawned.done=false` (reset OnExit Rpg) → spawn des 4 villageois en CyberCity | leaky | **HIGH** |
| **MovementSpeedMultiplier / MouseSensitivityMultiplier / FOV ADS** | write `forgia-viewmodel/src/pose.rs:113-143` (run_if Fps\|Roguelite) ; read `forgia-player/src/lib.rs:486` (run_if AppMode::InGame) | Sortir en plein ADS (RMB tenu) fige le multiplicateur < 1.0 ; le writer cesse, aucun reset → mode suivant marche au ralenti | discipline-only | medium |
| **`loot_tables::Souls`** (monnaie partagée RPG ↔ Roguelite via `Souls as Gold`) | `forgia-rpg-data/src/loot_tables.rs:52-59` (collecte SANS run_if) ; alias `forgia-mode-roguelite/src/run.rs:24` | `sys_collect_pickups` mute `souls.current/total_collected` dans n'importe quel mode ; `total_collected` jamais reset entre modes | none | medium |
| **RogueliteToonConfig / ToonGenomeWatch** jamais `remove_resource` | `forgia-mode-roguelite/src/toon_config.rs` ; câblage `lib.rs:147-164` | Le component `ToonSettings` est bien détaché OnExit (sys_detach), mais les Resources résident en RAM après sortie | discipline-only | medium |
| **SeaLevel / WaterSettings.height** | `forgia-rpg/src/lib.rs:174` (insert boot) ; sync `forgia-water/src/lib.rs:66-79` | Pas retiré par `cleanup_world` ; fuite visuelle neutralisée par `hide_water` OnExit Rpg (Visibility) | discipline-only | low |
| **AutoRigGizmosConfig.enabled** | `forgia-rpg/src/character.rs:593/598` | Paire enable/disable OnEnter/OnExit CyberCity symétrique | discipline-only | low |
| **DebugRenderContext** (wireframe Rapier global, F10) | `forgia-mode-roguelite/src/lib.rs:470-474` | Toggle dev mute une Resource render globale ; pas de reset OnExit | discipline-only | low |
| Terrain globales (ChunkManager/BiomeMap/MapGenConfig…) | `forgia-rpg/src/lib.rs:607-642` ; cleanup `2125-2142` + `clear_world_biome_map()` | Cleanup exhaustif et discipliné OnExit(Rpg) | discipline-only | low |
| Msaa / ImagePlugin default_sampler / RapierConfiguration.gravity / UserSettings | `forgia-game/src/lib.rs:26-33` | Aucune mutation mode-spécifique (au défaut) | enforced | low |

### 3.2 Caméras — composants de rendu sur entités partagées

| Couplage | Où (file:line) | Mécanisme | Isolation | Risque |
|---|---|---|---|---|
| **Hdr + Bloom** posés post-hoc sur OrbitCamera = passe principale cassée | `forgia-game/src/cyber_city.rs:220-223` (retirés, commentaire) | Ajout post-hoc de config rendu HDR/Bloom sur la caméra orbitale partagée → "écran ClearColor nu : ni skybox ni géométrie". Contourné en n'attachant que fog+ambient | leaky | **HIGH** (latent) |
| **OrbitCamera + Rex + locomotion** partagés Rpg/CyberCity via 1 run-condition | `forgia-rpg/src/lib.rs:147` (`rex_third_person_active = Rpg\|CyberCity`) ; cleanup `197-200` | Même chaîne, deux OnExit distincts appelant le même `cleanup_rex_character` ; aucun marker ne distingue OrbitCamera RPG vs CyberCity. Risque 2 Camera3d actives si OnExit raté | discipline-only | high |
| **ToonSettings** inséré sur **toute** Camera3d (query globale) | `forgia-mode-roguelite/src/toon_config.rs:196/228/237` | Query `With<Camera3d>` sans filtre + `Added<Camera3d>` ; seul filet = `sys_detach_toon_from_cameras` OnExit Roguelite | discipline-only | high |
| **Skybox** attaché à toute Camera3d, **jamais retiré** | `forgia-player/src/lib.rs:246` (ungated) + `354-376` ; regen `skybox_genome.rs:202` | `Update` sans run_if, query `With<Camera3d> Without<Skybox>` ; 0 `remove::<Skybox>`. Palette per-biome pilotée par stage Roguelite repeint toutes les caméras | none / leaky | medium |
| **FpsCamera** non despawnée en 3P (seulement `is_active=false`) | `forgia-rpg/src/character.rs:111-114` | Reste une Camera3d ciblée par skybox/toon ; accumule des composants sans nettoyage hors Roguelite | discipline-only | medium |
| **DistanceFog + AmbientLight** (atmosphère Roguelite / cyber) | `forgia-mode-roguelite/src/atmosphere.rs:66-85` ; `cyber_city.rs:215-238` | Components per-caméra (pas la Resource globale) ; paire OnEnter/OnExit | discipline-only | low |
| **OutlineSettings** désactivé (crash wgpu, node_edges partagés Toon) | `forgia-mode-roguelite/src/lib.rs:137-145` ; `toon_config.rs:266` (OUTLINE_ATTACHED=false) | Deux post-process partageant `Tonemapping→X→EndMainPass` = crash surface. Démontre la fragilité "toucher la passe principale" | leaky | low |
| **MenuCamera2d** | `forgia-ui/src/lib.rs:110-127` | Camera2d → exclue par type des queries `With<Camera3d>` | **enforced (type-level)** | low |

### 3.3 Systèmes non-gatés tournant dans tous les modes

| Couplage | Où (file:line) | Mécanisme | Isolation | Risque |
|---|---|---|---|---|
| **`spawn_stage_arena_on_request`** ungated | `forgia-stage/src/lib.rs:327-331` (vérifié : `Update` + `GameSet::Movement`, **0 run_if**) | Gardé uniquement par `Option<Res<StageLoadRequest>>` (lib.rs:761) ; cleanup = responsabilité du caller | discipline-only | **HIGH** (cf §5) |
| **`attach_skybox_to_camera`** ungated | `forgia-player/src/lib.rs:246` (vérifié) | `Update` sans run_if, cible toute Camera3d, aucun detach | leaky | medium |
| **`apply_settings_to_window/volume`** ungated | `forgia-ui-lib/src/pause_menu.rs:574-585` | Ciblent un état global par nature (1 fenêtre/1 volume) ; risque = boucle resize Changed (déjà mitigé par `bypass_change_detection`) | none | low |
| **forgia-debug overlay** (F2/F3) | `forgia-debug/src/lib.rs:98-107` (ungated) | Guard interne `master_visible` ; ne mute pas d'état de jeu, capture input clavier global (collision F2 vs F3) | discipline-only | low |

> Correction factuelle : `apply_tonemapping_to_cameras` / `apply_msaa_to_cameras` **n'existent pas** (grep exhaustif). Tonemapping/MSAA/VSync sont déférés (story-599). Le pause_menu n'applique que sensibilité/FOV/volume/fenêtre.

### 3.4 Cleanup / isolation OnExit

| Couplage | Où (file:line) | Mécanisme | Isolation | Risque |
|---|---|---|---|---|
| **`cleanup_character_lineup` câblé OnExit(Rpg) seulement** | `forgia-rpg/src/lib.rs:284` (vérifié) ; spawn run_if(Rpg\|CyberCity) | OnExit(CyberCity) ne lance que `cleanup_rex_character` (lib.rs:199), pas le lineup → PNJ RPG fuités persistent | leaky | **HIGH** |
| **Viewmodel** despawn OnExit(Fps) seul, spawn Fps\|Roguelite | `forgia-viewmodel/src/attach.rs:260/269` | Sortie Roguelite ne tire pas OnExit(Fps) ; neutralisé par cascade despawn Player parent | leaky | low |
| **Bots IA** : ArenaMarker (FPS) vs DespawnOnExit (Roguelite) + purge manuelle | `forgia-mode-fps-arena/src/wave.rs:350` ; `forgia-mode-roguelite/src/run.rs:667-682` | DespawnOnExit ne tire pas en restart intra-mode → purge manuelle `sys_start_run` (leak historique bots_alive=51) | discipline-only | low |
| **Aucun hub cleanup central** | `forgia-core/src/lib.rs:96-123` (3× init_state, 0 OnExit) | Chaque crate gère son OnExit isolément ; exhaustivité dépend de la mémoire du dev | discipline-only | high |

### 3.5 Pipelines & assets partagés

| Couplage | Où (file:line) | Mécanisme | Isolation | Risque |
|---|---|---|---|---|
| **`procedural_locomotion` `.single()`** mono-perso | `forgia-anim-locomotion/src/locomotion.rs:644-686` | 2 `LocomotionTarget` simultanés → `.single()` Err → anim Rex MEURT en silence (failure mode V2 documenté) | discipline-only | medium |
| **Player + FpsCamera unique** | `forgia-player/src/lib.rs:247/252` | Spawné OnEnter(InGame), partagé 4 modes ; HP/ammo/position/composants mode-spécifiques persistent jusqu'au retour Menu | discipline-only | medium |
| **GameSet / schedule global unique** | `forgia-core/src/lib.rs:79-91/107-120` | Pas de schedule par mode ; un système `.in_set(Movement)` sans run_if tourne partout | discipline-only | low |
| **AssetServer cache cross-mode** | `forgia-assets/src/lib.rs:16-34` (GameAssets vide Phase 0) | Dédup par chemin ; cyberpunk_city.glb ~185 Mo reste résident VRAM après OnExit, aucun unload par mode | discipline-only | low |
| **ImagePlugin default_sampler global** | `forgia-game/src/lib.rs:26-33` | Aucun `.set(ImagePlugin)` ; passer en aniso 16× affecterait les 4 modes (pas de réglage per-mode) | none | low |

## 4. Pourquoi "indépendant" est une illusion

Il n'y a **qu'un seul `App` / un seul `World`**. Un `GameMode` n'est pas un sandbox : c'est juste une valeur de Resource `State<GameMode>` que les systèmes consultent volontairement via `run_if(in_state(...))`. Bevy **n'impose rien** :

- **Aucun plugin scoping** : `forgia_fps`, `forgia_rpg`, `forgia_mode_roguelite`, `CyberCityDemoPlugin` sont tous présents en mémoire en permanence (`forgia-game/src/lib.rs:91-120`). Leurs systèmes existent dans le schedule de tous les modes.
- **Toute Resource globale mutée sans reset OnExit fuite** : c'est physiquement la même valeur entre modes (ClearColor, MovementSpeedMultiplier, Souls, SeaLevel…).
- **Tout système sans `run_if(in_state(GameMode))` tourne partout** : son seul garde-fou possible est un guard interne (présence de Resource, marker, `master_visible`). Une seule omission de gate (cf StageLoadRequest) suffit à faire fuiter un mode entier.
- **Toute entité partagée (Player, FpsCamera, OrbitCamera) garde les composants qu'un mode lui a collés** tant qu'un `OnExit` explicite ne les retire pas. Un composant de rendu (ToonSettings, Skybox, DistanceFog) est persistant, pas recalculé par frame.
- **Le seul vrai mécanisme moteur** est le SubState `RunState` (auto-retiré à la sortie de Roguelite) et le typage `Camera2d` de la MenuCamera2d (immune aux queries `With<Camera3d>`). Tout le reste = convention.

La conséquence : élargir une `run_if` (ex. `Rpg → Rpg|CyberCity`, `forgia-rpg/src/lib.rs:147`) embarque **toute** la chaîne (Rex + locomotion + caméra orbitale + **lineup PNJ**), pas seulement l'animation voulue. Le couplage se propage par les bords du graphe de systèmes, invisible à la compilation.

## 5. Cas concrets récents

- **Fuite arène Roguelite → CyberCity (story-600)** : `StageLoadRequest` est inséré (`run.rs:158`) et **jamais** `remove_resource` (vérifié : 0 occurrence dans tout le codebase). `cleanup_stage_arena` despawn les `StageArenaMarker` et reset `StageLoadResult` à Idle, mais laisse `StageLoadRequest` vivant + `Local<String> last_processed_id` figé. Comme `spawn_stage_arena_on_request` est ungated (`forgia-stage/src/lib.rs:327`, guard = `Option<Res<StageLoadRequest>>` seul), au prochain InGame en CyberCity : `request=Some`, l'idempotent check `last_processed_id==stage_id && state==Ready` est FALSE (state=Idle) → re-spawn complet du sol KayKit + ramparts hex + POIs du stage Roguelite **dans** CyberCity.

- **Écran marron/void = MSAA/HDR/Bloom sur la caméra orbitale** : confirmé en commentaire `forgia-game/src/cyber_city.rs:220-223` — "Hdr + Bloom retirés — posés après-coup sur la caméra orbitale, ils cassaient la passe principale (écran ClearColor nu : ni skybox ni géométrie)". Même classe de bug que l'Outline désactivé (crash wgpu `SurfaceAcquireSemaphores`, node_edges partagés avec Toon, `lib.rs:137-145`). **Toucher la passe principale via un composant attaché à une caméra partagée casse le rendu global.** Toute réintroduction HDR/Bloom/post-process sur l'orbit cam re-casse.

- **Aniso 16× via ImagePlugin global** : il n'existe aucun `.set(ImagePlugin)` (`forgia-game/src/lib.rs:26-33`). Passer `default_sampler` en aniso 16× pour les armes FPS s'appliquerait simultanément à RPG/CyberCity/Roguelite — aucun point de configuration per-mode.

- **Réglages graphiques appliqués à toute Camera3d** : `attach_skybox_to_camera` ungated (`forgia-player/src/lib.rs:246`) et `sys_apply_toon_settings` (query globale `With<Camera3d>`) collent leurs composants sur n'importe quelle caméra, y compris une FpsCamera désactivée survivante d'une session précédente.

## 6. Recommandations classées

### Quick wins (correctifs ciblés, faible risque)
1. **Restaurer ClearColor boot OnExit(Roguelite)** : ajouter `commands.insert_resource(ClearColor(BOOT_SKY))` dans `cleanup_stage_arena` (ou un OnExit dédié). Définir `BOOT_SKY` const partagée avec `forgia-game/src/lib.rs:132`.
2. **Retirer `StageLoadRequest` OnExit(Roguelite)** : `commands.remove_resource::<StageLoadRequest>()` dans `cleanup_stage_arena`, ET gater `spawn_stage_arena_on_request` par `run_if(resource_exists::<StageLoadRequest>)` (suffit à supprimer le scan archetype ungated). Fixe story-600.
3. **Retirer `RpgVillageAnchor` dans `cleanup_world`** (`forgia-rpg/src/lib.rs:2125`) + câbler `cleanup_character_lineup` sur `OnExit(CyberCity)` aussi (ou mieux, le rendre symétrique au gating `rex_third_person_active`). Fixe la fuite lineup PNJ.
4. **Reset `MovementSpeedMultiplier`/`MouseSensitivityMultiplier`/FOV ADS à 1.0** dans un système `OnExit(Fps)` + `OnExit(Roguelite)` (`forgia-viewmodel`).
5. **Detach Skybox** : ajouter un `OnExit(GameMode)` générique qui retire `Skybox` des caméras du mode, OU gater `attach_skybox_to_camera` par run_if et le rendre per-mode.

### Structurel (rendre l'isolation RÉELLE)
6. **Hub de cleanup central + `StateScoped`** : adopter systématiquement `StateScoped(GameMode::X)` (Bevy 0.18) sur toutes les entités spawnées par un mode — l'engine despawn alors automatiquement à la sortie, supprimant la dépendance aux markers manuels et au cleanup discipline-only. Prioritaire pour Rex, OrbitCamera, StageArenaMarker, lineup PNJ.
7. **Resources mode-scoped** : encapsuler les Resources mode-spécifiques mutant du global (ToonConfig, StageLoadRequest) avec un pattern insert OnEnter / remove OnExit systématique, ou les déplacer dans des Resources non-globales scoped au SubState.
8. **Composants caméra par-mode garantis** : tout composant de rendu attaché à une caméra partagée DOIT avoir un détacheur OnExit du mode qui l'a posé. Interdire les queries globales `With<Camera3d>` au profit de markers de caméra explicites (`FpsCameraMarker`, `OrbitCameraMarker`).
9. **Ne jamais toucher la passe principale via composants post-hoc** : graver en règle que HDR/Bloom/Outline doivent être configurés à la création de la caméra (cf MEMORY `reference_cyber_city_render_camera_3p_reuse`), jamais ajoutés après-coup.
10. **Réveiller ou supprimer `WorldMode`** : actuellement machine morte (`forgia-core/src/lib.rs:64-70`) — soit la brancher (gating sim/editor pour Phase 2), soit la retirer pour ne pas suggérer une isolation inexistante.
11. **Audit ratchet xtask** : étendre `check-orphans` (story-528) avec un lint "tout système `.in_set(GameSet::*)` sans `run_if(in_state(GameMode))` doit être whitelisté explicitement" + "toute `insert_resource` d'une Resource globale doit avoir un `remove_resource`/reset OnExit correspondant". Transforme la discipline en barrière CI.

**Preuves de référence** : `forgia-stage/src/lib.rs:327/761/1161`, `forgia-rpg/src/lib.rs:147/199/284/602/2125`, `forgia-game/src/cyber_city.rs:220-223`, `forgia-player/src/lib.rs:246/247/252`, `forgia-mode-roguelite/src/run.rs:158`, `forgia-core/src/lib.rs:64-70/96-123`.