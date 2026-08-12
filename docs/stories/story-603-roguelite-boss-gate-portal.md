# Story-603 — Roguelite : porte du socle (boss-gated) → parcours

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_roguelite_state.json`, fichier `boss_portal.rs`, symbole `Enter`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : EN COURS (2026-06-17)
> **Niveau BMAD** : Standard (5 fichiers : assets + loot_room + waves + sensor + lib)
> **Demande user** : « ajoute des colliders au socle du milieu de la map, pose le
> portail fermé (avec collider) sur le socle ; quand le boss final est vaincu, le
> portail ouvert remplace le fermé ; toucher le portail ouvert téléporte dans le
> parcours. »

## Décisions (AskUserQuestion 2026-06-17)

1. **Boss → boucle** : tuer le boss (vague 3) n'émet PLUS `EndRunEvent(Victory)`.
   À la place, la porte du socle s'ouvre. Flux : boss → porte → parcours →
   (portail Retour) → arène. Condition de fin de run = à brancher plus tard.
2. **Remplacer l'ancien portail** : l'anneau torus `Enter` (z=-34, toujours
   ouvert) est supprimé. Le parcours est accessible UNIQUEMENT via la porte du
   socle, après le boss.

## Architecture (concept-first)

- Concept = `spawn`/`portal`/`teleport` + `state machine` (boss-clear). Layer =
  framework (Rust) + asset (GLB). Net = local. Script = interne.
- **Producteur (vérité)** : `waves.rs:sys_wave_orchestrator` — sur clear de la
  vague finale, met `RogueliteWave.boss_defeated = true` (au lieu d'émettre
  Victory). Timing = frame.
- **Consommateurs** :
  - `loot_room::sys_reconcile_boss_gate` (frame) — swap porte fermée ↔ ouverte
    selon `wave.boss_defeated` (gère aussi le reset à `false` au prochain run).
  - `loot_room::sys_portal_walkover` (frame, EXISTANT) — branche `Enter`
    téléporte dans le parcours (réutilisé tel quel).
  - `loot_room::sys_calibrate_portal` (frame) — scale GLB via AABB (target hauteur).
- **Sensor** : `forgia2_roguelite_state.json` étendu — `boss_defeated` +
  `boss_gate_open`.
- **Socle** : construit côté roguelite (mesh primitif étagé + `RigidBody::Fixed`
  + `Collider`), au centre-avant (`SOCLE_POS`), 0 touche `forgia-stage`.

## Critères d'acceptation

- [x] Socle visible au centre-avant de l'arène, solide (joueur ne le traverse pas). *(code-complete, valid. runtime)*
- [x] Portail FERMÉ (GLB `portal_closed.glb`) posé sur le socle, avec collider (bloque). *(code-complete)*
- [x] Boss (vague 3) vaincu → portail fermé remplacé par portail OUVERT (`portal_open.glb`). *(code-complete)*
- [x] Toucher le portail ouvert → téléportation dans le parcours (zone 1). *(réutilise `PortalKind::Enter`)*
- [x] Plus d'écran VICTOIRE auto au boss (boucle ouverte assumée). *(EndRunEvent(Victory) retiré)*
- [x] Ancien anneau Enter (z=-34) supprimé.
- [x] Nouveau run (depuis Lobby) → porte re-fermée (reset `boss_defeated` + `sys_reconcile_boss_gate`).
- [x] `cargo check -p forgia-mode-roguelite` + clippy 0 warning.

## Auto-QA (post-impl, 2026-06-17)

- **verifier** : VALIDÉ — Locks L1-L8/LOCK-INV-1 intacts, GameSet OK, DespawnOnExit
  sur toutes les entités, `EndRunEvent`/`RunResult` retirés proprement, sensor
  format cohérent.
- **qa-lead** : 5 findings. **3 corrigés** : BUG-603-01 (reset `LootRoomState`+
  `ShrinkBuff` sur `OnExit(RunState::Lobby)` — anti soft-lock si abandon depuis le
  parcours), BUG-603-03 (garde explicite `wave.boss_defeated` sur walk-over Enter),
  BUG-603-05 (doc waves.rs). **2 réfutés (faux positifs)** : BUG-603-02 (`sys_setup`
  ne re-tourne pas par run — 1×/entrée GameMode ; reconcile garde 1 seule porte),
  BUG-603-04 (conflit `&Transform`/`&mut Transform` CROSS-système = sérialisé par
  le scheduler Bevy, pas de panic ; seuls les conflits intra-système paniquent).

> Statut : **code-complete + QA OK**. Reste : validation runtime in-game.

## Itération 2 (2026-06-17 — retour runtime « mécanique parfaite »)

User : la mécanique fonctionne. 3 ajustements :
1. **Portail enfoncé dans le socle** → `sys_ground_portal` : après calibration scale
   (GlobalTransform propagé), mesure le min Y monde de la géométrie (8 coins AABB
   par mesh) et décale le SceneRoot pour que la base repose sur `base_world_y`
   (sommet socle). Robuste au pivot GLB inconnu (≠ supposer pivot au pied).
   Portails restructurés : parent (pos/Portal/collider) + enfant visuel calibré+groundé.
2. **Flammes dans les yeux des crânes** → `spawn_eye_flames` : 4 sphères émissives
   orange + PointLight scintillant (`sys_flicker_portal_flames`, phase/œil) aux
   `SKULL_EYE_OFFSETS` (tunable). Sur porte fermée ET ouverte.
3. **Compteur `WAVE x/4` → `x/3`** : `hud.rs::draw_wave_counter` utilisait
   `RunGraph.total_stages` (4 nœuds) au lieu de `WAVES_TOTAL` (3 vagues, 3e = boss).

## Itération 3 (2026-06-17 — « pose-le sur CE socle »)

User pointe le **dais central existant** (≠ mon socle cylindre). C'est le module
`melee_pit` de forgia-stage (prefab `CirclePlatformSmall`, ancre `AnchorKind::MeleePit`),
**placé par le seed (bouge chaque run)** et **sans collider**. Donc :
- Socle custom (3 cylindres) **supprimé**.
- `sys_reconcile_boss_gate` **lit l'ancre `MeleePit`** (repli central `DAIS_FALLBACK_POS`
  si absente), **ajoute un collider** cylindre solide sur le dais (`spawn_dais_collider`,
  `DAIS_TOP_Y`/`DAIS_COLLIDER_RADIUS` tunables) et **pose la porte au sommet**.
- **Re-pose si le dais bouge** (nouveau run/seed → ancre déplacée > 0.5 m) :
  despawn socle+porte, re-spawn à la nouvelle position. Suit toujours le dais.
- 0 touche forgia-stage (lecture d'ancre seulement, comme `decor.rs`).

## Itération 4 (2026-06-17 — « bon socle » mais 3 défauts runtime)

Log confirme placement correct sur le dais (`@ (6.9,0,15.8)` = ancre MeleePit,
*derrière* le spawn → d'où l'importance du facing). 3 corrections :
1. **Porte face au joueur** : `yaw = atan2(dais.x, dais.z) + PORTAL_YAW_OFFSET` →
   la porte regarde le spawn (origine). Offset tunable selon l'axe avant du GLB.
2. **Collider du dais absent** : collider cylindre déplacé du *child* → **sur
   l'entité même** (RigidBody::Fixed + Collider + Transform) = enregistrement
   rapier garanti + visible F10.
3. **Porte surélevée** : fin de la hauteur devinée (`DAIS_TOP_Y` supprimé) →
   `measure_dais_top` mesure le sommet RÉEL du mesh dais (AABB monde dans un rayon,
   seuil `DAIS_MEASURE_MIN_Y` pour attendre le chargement) → porte + collider posés
   pile dessus. `BossGate.dais_top_y: Option<f32>`.

> ⚠️ Dette : `loot_room.rs` édité 30×, hotspot. Candidat extraction `boss_portal.rs`
> une fois validé runtime (story suiveuse).

## Itération 5 (2026-06-17 — collider absent + porte trop haute + 180°)

Log : `dais mesuré top=3.67 m` → la **mesure par rayon (4.5 m) captait un prop
voisin**, pas le dais → porte placée trop haut. Le collider cylindre ne se voyait
pas non plus. Bascule sur l'approche **mesh exact** (comme `decor.rs` pour le parcours) :
1. **Dais trouvé par NOM** (`Module_melee_pit*`, `find_dais_root`), plus de rayon.
2. **`solidify_and_measure_dais`** : collider **TriMesh sur le mesh même du dais**
   (épouse le visuel, marchable, visible F10, enregistrement rapier garanti) +
   mesure du **vrai sommet** (AABB monde du sous-arbre du dais, attend chargement complet).
3. **Yaw 180°** : `PORTAL_YAW_OFFSET = PI` (avant natif GLB = +Z).
- Supprimés : `BossSocle`, `spawn_dais_collider` (cylindre), `measure_dais_top` (rayon),
  consts `DAIS_COLLIDER_RADIUS`/`DAIS_MEASURE_MIN_Y`.

## Itération 6 (2026-06-17 — colliders OK, mais porte en lévitation)

User : colliders parfaits, mais porte flotte. Log : `top=3.67 m` AUSSI en mesh-AABB →
le **mesh du dais a une AABB haute** (déco totem/braséro) ≠ son **sol marchable** (~1 m).
L'AABB max est le mauvais signal. Fix :
1. **Surface par RAYCAST** : `solidify_dais` (collider TriMesh seul) puis, 1 frame
   après (le temps que rapier enregistre), `raycast_dais_surface` tire un rayon
   vertical au CENTRE du dais → vrai sol. `BossGate.dais_ready` séquence les 2 phases.
2. **Garde anti-sol** : hit ≤ 0.5 m (sol du stage à y≈0) = collider pas encore
   enregistré → retry (évite porte sous terre).

## Assets

- `assets/models/environment/portal/portal_closed.glb` (16.7 MB) ← « Portail Forgia Fermé.glb »
- `assets/models/environment/portal/portal_open.glb` (42.4 MB) ← « Portail Forgia Actif.glb »
- Sources hors git (`D:/ressources externes/Batiments/Portail/`).
