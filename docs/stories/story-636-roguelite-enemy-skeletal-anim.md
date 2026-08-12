# Story-636 — Animation squelettique + rig de contrôle sur les ennemis Roguelite

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_enemy_anim.json`, fichier `enemy_anim.rs`, symbole `SceneRoot`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : 🟡 IN_PROGRESS (créée 2026-06-29).
> **Niveau BMAD** : Standard/Enterprise (6 fichiers). **Crate** : `forgia-mode-roguelite`.

## Contexte
Les 4 archétypes Roguelite (Tank/Runner/Sniper/Boss) sont spawnés dans
`crates/forgia-mode-roguelite/src/waves.rs` avec un visuel `SceneRoot` KayKit
(`Skeleton_Warrior/Minion/Mage.glb`). Inspection des GLB : chaque squelette est **déjà
rigué** (skin `Rig`, 41 bones, `SkinnedMesh` auto-chargé) **et contient 95 clips bakés**
(`Idle`, `Walking_A`, `Running_A`, attaques, `Death_A`, `Hit_A`…). Mais **aucune ne joue** :
les ennemis glissent figés en pose de repos pendant que l'IA (`forgia-ai-arena-bot`) les
déplace par mutation de `Transform`.

## Objectif
Jouer les clips bakés via Bevy `AnimationPlayer`/`AnimationGraph`, pilotés par l'état IA
(`ArenaBot.state`) + vitesse mesurée → locomotion par défaut (idle/marche/course + idle de
combat). Ajouter une **viz de contrôle du rig** (mesh translucide + gizmos de bones) en
**toggle hot-reload**. Le toolbench procédural (`forgia-auto-rig`/`forgia-anim-locomotion`)
est écarté : conçu pour meshes NON animés, bloqué multi-humanoïde ; le rig + anims existent
déjà dans la donnée (concept-first étape 0 : couche `definition`).

## Implémentation
- `enemy_anim.rs` (nouveau) : `EnemyAnimConfig` (genome `roguelite_enemy_anim.toml`,
  hot-reload mtime) + cache `EnemyAnimGraphs` (1 `AnimationGraph` par GLB unique, clips via
  `Gltf.named_animations`) + binding robuste `Without<EnemyAnimBound>` (remonte `ChildOf` →
  `EnemyArchetype`) + sélection `desired_clip(state, speed)` avec crossfade + sensor
  `forgia2_enemy_anim.json`.
- `enemy_rig_debug.rs` (nouveau) : transparence via observer `On<SceneInstanceReady>`
  (clone de matériau **dédupliqué** par `AssetId` → toggle global) + gizmos de bones depuis
  `SkinnedMesh.joints`.
- `waves.rs` : additifs (parent `+ EnemyLocoSample`, visuel `.observe(on_enemy_scene_ready)`).
- `lib.rs` : `+ mod` + `.add_plugins(ForgiaRogueliteEnemyAnimPlugin)`.
- `assets/genomes/roguelite/roguelite_enemy_anim.toml` (nouveau, no-hardcode).

## Acceptance criteria
- [ ] Les ennemis Roguelite jouent idle/marche/course selon `ArenaBot.state` + vitesse.
- [ ] `forgia2_enemy_anim.json` : `players_bound == enemies_total`, `severity:"ok"`.
- [ ] Mesh translucide + gizmos de bones visibles quand `rig_visible/gizmo_enabled = true`.
- [ ] `rig_visible=false` (hot-reload) → ennemis opaques, sans rebuild.
- [ ] 6 tests purs verts (`parse_toml`, `desired_clip`, `severity`).
- [ ] `cargo clippy -p forgia-mode-roguelite` : 0 warning.

## Notes / dette
- 1 nouveau call-site `asset_server.load::<Gltf>()` (build du graphe d'anim — légitime
  à la volée). À surveiller vs Lock L1 (ratchet asset-load) au push.
- Clip-names changent au build du graphe (pas hot-reload) ; seuils/debug = hot-reload live.
- Fast-follow (hors story) : swing d'attaque réel one-shot, réaction `Hit_A`, death + délai
  de despawn, sync footsteps.

## QA auto (2026-06-29)
- **verifier** : ✅ VALIDÉ (0 warning clippy, 0 erreur build, GameSet/no-hardcode/observabilité OK, 10 tests).
- **qa-lead** : 2 fix appliqués — BUG-01 (vitesse mesurée déplacée en `PostUpdate`, lecture post-mouvement IA déterministe) + BUG-04 (purge `EnemyMatRegistry` `OnExit(Roguelite)`). Justifiés/skip : BUG-02 (délai 1-frame gardé, AutoInsertApplyDeferred), BUG-03 (O(41²)×16 ≈ 10µs gaté gizmo ; HashSet = alloc/frame → pire), sensor (`EnemyAnimBound` exclusif ennemis par construction), BUG-05 death anim (fast-follow documenté), BUG-06 (aucun bug réel).
- **Reste avant DONE** : validation runtime en jeu (effet observable) + commit (G1 git-tracked).

## Cross-refs
- `concept-first.md` étape 0 (data vs code) ; `no-hardcode.md` ; `observability-required.md`.
- `reference_roguelite_enemy_skeleton_mapping.md` (mapping archétype → GLB).
