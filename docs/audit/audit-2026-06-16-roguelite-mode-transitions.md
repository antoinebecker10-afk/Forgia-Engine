# Audit — Transitions de mode Roguelite : map invisible, joueur coincé, accumulation d'entités, freezes

> **Date** : 2026-06-16
> **Contexte** : investigation déclenchée par « écran marron » / « map Roguelite invisible » / blocage
> au changement de mode. Diagnostic sensor-first (forgia2_*.json + forgia2_run.log + diagnostic
> `forgia2_load_timing.json` ajouté pour l'occasion).
> **Destinataire** : terminal qui possède le contenu Roguelite/RPG (forgia-mode-roguelite::decor /
> loot_room, forgia-stage, forgia-rpg::character) — la plupart des causes y vivent.
> **Périmètre** : ces bugs sont **indépendants** des réglages graphiques (story-599) et du fix
> d'isolation stage (story-600), qui sont sains et committés.

---

## Symptômes observés (utilisateur)

1. Écran marron / on ne voit que le skybox en entrant en Roguelite (ou Cyber).
2. « Map Roguelite invisible » — HUD présent, ennemis trackés (blips minimap), mais aucun sol/mur.
3. Blocage au changement de mode (freezes).
4. Éléments Roguelite visibles dans CyberCity (déjà corrigé : story-600).

---

## Cause 1 — Décor invisible (CONFIRMÉ + CORRIGÉ ce jour)

**Preuve** : 160× `warning[B0004]: The DecorVisual entity with InheritedVisibility has a parent
(Decor_*) without InheritedVisibility` dans forgia2_run.log.

**Mécanisme** : `forgia-mode-roguelite/src/decor.rs::decor_markers()` spawnait le **parent** décor
sans composant `Visibility` ; l'enfant `DecorVisual` (`SceneRoot`) a `InheritedVisibility` (composant
requis), mais sans `Visibility` sur le parent la propagation casse → enfant **cullé** → mesh invisible
(le collider, lui, fonctionne → le joueur peut buter dessus sans le voir).

**Fix appliqué (2026-06-16)** : `Visibility::default()` ajouté au tuple `decor_markers`. Décor de
nouveau visible. ✅

---

## Cause 2 — Joueur coincé + map (sol) invisible (NON RÉSOLU — racine = Cause 3)

**Preuve** : `forgia2_player_state.json` → `severity:"critical"`, `position:[0,1.097,0]`,
`stuck_frames_consecutive:2825` (~47 s), `grounded:true`, `kcc_collisions:0`. Le sol du stage
(`forgia-stage`) a pourtant `Visibility::default()` (lib.rs:863) et est centré à l'origine
(base Y=-0.5, lib.rs:873) — il **devrait** être visible. Or l'écran est tout skybox.

**Interprétation** : le joueur spawn à l'origine **encastré dans de la géométrie qui se chevauche**
(grounded + stuck + caméra à l'intérieur d'un mesh → faces arrière cullées → skybox visible). Ce
n'est pas un problème de la FpsCamera (HUD + skybox rendent ; `FpsCamera` réactivée par garde-fou
story-aud — voir plus bas). C'est un problème de **ce qui est posé à l'origine**.

---

## Cause 3 — Accumulation d'entités / fuite d'isolation de mode (RACINE, NON RÉSOLU)

**Preuve** (diagnostic `forgia2_load_timing.json` + log) :
- `entities_now:24903` (un niveau Roguelite propre ≈ 4000 ; menu ≈ 0).
- Saut de **+13 737 entités en une frame**.
- Au quit Roguelite : `[forgia-rpg] World cleaned : 68 entités + 943 trees + LOD2 tiles despawned`,
  `Rex despawned, FpsCamera re-enabled`, `Lineup cleaned : 4 characters` — **du contenu RPG était
  présent dans la session Roguelite**.

**Mécanisme** : passer d'un mode à l'autre ne **purge pas** le contenu du mode précédent. Le monde
RPG (943 arbres + terrain LOD2 + Rex + lineup + PNJ), la scène CyberCity (1815 meshes), et plusieurs
contenus Roguelite **s'empilent** à l'origine. Conséquences en cascade :
- Le joueur spawn dans ce tas → encastré → bloqué + ne voit que le skybox (Cause 2).
- En RPG/Cyber, `spawn_rex_character` désactive la `FpsCamera` ; si on enchaîne vers Roguelite sans
  cleanup, la map (rendue par FpsCamera) est invisible (garde-fou ajouté ce jour, voir §Fixes).

**C'est la même classe que story-600** (StageLoadRequest non nettoyée), mais **généralisée** à tout
le contenu de mode. La discipline d'isolation (Bevy : `run_if(in_state)` + `StateScoped`/`DespawnOnExit`
+ cleanup `OnExit`) n'est pas appliquée uniformément : certains contenus ont `DespawnOnExit<GameMode>`
(ex. décor), d'autres reposent sur un cleanup manuel `OnExit(mode)` qui ne couvre pas tous les
chemins de transition.

**Piste de fix (à la charge du propriétaire du contenu)** : garantir qu'**entrer dans un mode purge
les autres** — soit tagger TOUT le contenu de mode en `DespawnOnExit<GameMode>` (Bevy nettoie
automatiquement), soit un système `OnEnter(mode)` qui despawn tout marqueur d'un autre mode. Vérifier
en particulier : monde RPG (`forgia-rpg` cleanup_world/cleanup_rex), scène CyberCity, loot_room, stage.

---

## Cause 4 — Freeze GPU ~7 s au chargement (NON RÉSOLU)

**Preuve** (`forgia2_load_timing.json`) : `worst_dt_ms:7173` avec `worst_entity_delta:0` +
`worst_collider_delta:0` — une frame de **7,2 s SANS spawn d'entité ni de collider**. Donc ni le
spawn (rapide, <100 ms d'après les timestamps log), ni les colliders (batchés ~20/frame, doc
loot_room) ne sont le pic. C'est le **traitement render** de la scène fraîchement instanciée :
compilation des pipelines shaders + upload GPU des meshes/matériaux des grosses scènes GLTF
(loot-room ~1200 pièces, cyber-city 1815 meshes).

**Pistes de fix** : pré-warm des pipelines au boot (rendu hors-écran), écran de chargement explicite,
ou découpe/allègement des scènes lourdes. Amplifié par l'accumulation (Cause 3) : plus il y a
d'entités/meshes empilés, plus le coût render explose.

---

## Fixes appliqués cette session (côté réglages/UI — sains)

- **story-600** (committé) : `cleanup_stage_arena` retire `StageLoadRequest` + `sys_stage_dispatch`
  auto-réparant → corrige la fuite du **stage** (écran marron initial + arène dans CyberCity).
- **Garde-fou FpsCamera** (forgia-mode-roguelite/run.rs `sys_ensure_fps_camera_active`,
  `run_if(Roguelite)`) : réactive la FpsCamera si un mode 3P l'a laissée éteinte → corrige la
  **vue** quand on arrive en Roguelite depuis RPG/Cyber.
- **Décor Visibility** (decor.rs `decor_markers`) : §Cause 1, décor de nouveau visible.
- **Diagnostic** `load_timing.rs` (sensor `forgia2_load_timing.json`) : temporaire, à retirer une
  fois Causes 3+4 traitées.

## Ce qui reste (racine, à traiter par le propriétaire du contenu)

| # | Problème | Sévérité | Fichiers probables |
|---|---|---|---|
| Cause 3 | Accumulation d'entités inter-mode (24 903) → joueur coincé + map invisible | **Bloquant** | forgia-rpg (cleanup_world/rex), forgia-stage, decor, loot_room, transitions GameMode |
| Cause 4 | Freeze GPU ~7 s au chargement de scène lourde | Majeur (UX/ship) | loot_room, cyber_city, pipeline pré-warm |
| Cause 2 | Joueur spawn coincé à l'origine | **Bloquant** (conséquence de Cause 3) | spawn joueur Roguelite + géométrie origine |

> Recommandation : traiter **Cause 3 en premier** (la racine). Les Causes 2 et 4 en découlent largement.
