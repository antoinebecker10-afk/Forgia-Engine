# Dossier de préparation — Migration Forgia V2 : Bevy 0.18.1 → 0.19 + Rust 1.94 → 1.96

> **Statut : PRÉPARATION uniquement. Aucun code n'est modifié maintenant.** La migration est *gated* sur des releases upstream non publiées. Ce dossier est le runbook reprenable à exécuter le jour où les bloqueurs lèvent.
> Cible : le workspace V2. Date : 2026-06-22.
> Généré par workflow multi-agents (`bevy-019-rust-migration-prep`, 20 agents). 96 breaking changes recensés, 68 sites localisés (carte d'impact = **plancher**, voir §9).

---

## 1. TL;DR + verdict

**Verdict : migration IMPOSSIBLE aujourd'hui.** Elle est bloquée par **3 releases upstream** qui n'existent pas encore en version compatible Bevy 0.19 :

1. **`bevy_rapier3d`** (physique/collisions — cœur gameplay, 17 crates consommatrices) → PR draft #694 *en cours mais bloquée* sur la chaîne `glam`/`glamx` de Rapier/parry.
2. **`bevy_hanabi`** (particules GPU/VFX) → *aucun travail 0.19 visible*, repo actif mais rien d'amorcé.
3. **`bevy_water`** (eau procédurale, track RPG) → *dormant* depuis fév. 2026, le plus risqué (probable fork/patch local nécessaire).

Tant que ces 3 crates n'ont pas publié une version ciblant Bevy 0.19 (ou ne sont pas forkées localement), **le workspace ne compilera pas** : ce sont des dépendances *dures* du graphe. La politique projet (`Cargo.toml:97` : *« Pin Bevy 0.18.1 stable until V1 ship. Migration 0.19+ post-ship V1.1 »*) **converge avec cette contrainte technique** — on ship le Roguelite d'abord, on migre après.

**Côté coût-bénéfice quand ce sera débloqué** : la surface réelle de patch dans *notre* code est **modérée mais sous-estimée par la carte d'impact fournie** (voir §9). Les points chauds confirmés :
- **`forgia-postprocess`** (render-graph → systèmes ECS) = risque rendu n°1, ~45 `impl FullscreenMaterial` à migrer.
- **`bevy_scene` → `bevy_world_serialization`** = mass-rename mécanique (~38 sites, `SceneRoot`/`Handle<Scene>`).
- **`TextFont.font_size: f32 → FontSize::Px`** = 2 sites triviaux.
- **`Skybox.image → Option<Handle<Image>>`** = 2 sites.
- **`Query<Entity>` non filtrés** = 2 sensors faussés (à filtrer `Without<IsResource>`).
- **GLTF `Handle<StandardMaterial>` → `Handle<GltfMaterial>`** = 1 site high (`forgia-worldgen/spawn.rs:151`).

**Rust 1.96** : risque principal = **clippy** (5 nouveaux lints `complexity` warn-by-default) sur une politique 0-warning. À neutraliser avec `cargo clippy --fix` avant le bump.

---

## 2. Bloqueurs upstream (HARD)

> **La migration ne peut pas démarrer tant que ces 3 lignes ne sont pas toutes ✅.** Suivre ce tableau comme un *gate board*.

| Crate | Version utilisée | Dernière publiée | Statut 0.19 | Tracking | Ce qu'on attend |
|---|---|---|---|---|---|
| **bevy_rapier3d** | `0.33` (`Cargo.toml:99`) | `0.34.0` (14 mai 2026) | 🟠 **PR DRAFT en cours, bloquée** | [PR #694](https://github.com/dimforge/bevy_rapier/pull/694) | PR « Update to Bevy 0.19.0 (WIP) » par Buncys (contributeur récurrent, a fait le 0.18). Bevy bumpé 0.19 + glam 0.32.1 *mais* conflit transitif : rapier 0.32/parry 0.26.1 épinglent `glamx 0.1.3 → glam 0.30.10`. Il faut que **dimforge release Rapier/parry avec glamx 0.2+/0.3** d'abord. Aucun ETA ferme. |
| **bevy_hanabi** | `0.18` (`Cargo.toml:101`) | `0.18.0` | 🔴 **Rien d'amorcé** | [CHANGELOG](https://github.com/djeedai/bevy_hanabi/blob/main/CHANGELOG.md) | main encore sur Bevy 0.18, zéro signal 0.19 (ni PR ni issue ni commit). Repo très actif (maintainer djeedai, ~10 commits juin 2026). Historique : upgrade ~1-4 mois après chaque release Bevy → plausible « semaines/mois » mais non commencé. |
| **bevy_water** | `0.18` (en dur dans 2 crates) | `0.18.1` (2 fév. 2026) | 🔴 **Dormant — le plus risqué** | [Branches](https://github.com/Neopallium/bevy_water/branches/all) | Aucune activité depuis fév. 2026. Dernière branche = `bevy_0.18`, pas de `bevy_0.19`/`dev`/`next`. **Si reste dormant → fork/patch local ou nudge maintainer.** Track RPG (pas SHIP) donc moins critique pour le 1er jeu. |

**Conséquence opérationnelle** : ne pas même tenter `cargo update` vers Bevy 0.19 avant que **rapier + hanabi** soient ✅ (water peut être contourné par fork local car track RPG). Tant que rapier est rouge, le build est mort pour 17 crates dont `forgia-game`, `forgia-player`, `forgia-combat`, `forgia-fps`.

---

## 3. Matrice de compat écosystème + drift

### 3.1 — Ready vs Blocked

| Crate | Utilisée | Dernière | Req Bevy de la dernière | Statut migration |
|---|---|---|---|---|
| bevy_rapier3d | 0.33 | 0.34.0 | `^0.18.1` | 🔴 **BLOCKED** (PR #694 WIP) |
| bevy_hanabi | 0.18 | 0.18.0 | `^0.18` | 🔴 **BLOCKED** (rien amorcé) |
| bevy_water | 0.18 | 0.18.1 | `^0.18` | 🔴 **BLOCKED** (dormant) |
| bevy_egui | 0.39.1 | **0.40.0** | `^0.19.0` | 🟢 **READY** |
| bevy_kira_audio | 0.25 | **0.26.0** | `^0.19.0` | 🟢 **READY** |
| leafwing-input-manager | 0.20 | **0.21.0** | `^0.19` | 🟢 **READY** |

Les 3 READY ne peuvent pas être bumpés isolément : ils requièrent **Bevy 0.19**, lui-même gated sur les 3 BLOCKED. Tout migre ensemble ou rien.

### 3.2 — DRIFT workspace-dep (À CORRIGER — pré-travail safe, cf §7)

Certaines crates épinglent une version **en dur** au lieu de `{ workspace = true }`. C'est une dette qui *multiplie* le risque de migration (chaque bump devra toucher N fichiers au lieu d'1 ligne dans `Cargo.toml` racine) :

| Crate | Fichier | Pins en dur | Problème |
|---|---|---|---|
| forgia-observability | `crates/forgia-observability/Cargo.toml` | `bevy_kira_audio = "0.25"` (L25), `bevy_water = "0.18"` (L26), `leafwing-input-manager = "0.20"` (L27) | versions dupliquées hors `[workspace.dependencies]` (confirmé sur fichier) |
| forgia-debug | `crates/forgia-debug/Cargo.toml` | `bevy_egui = "0.39.1"` (L12) | version en dur (confirmé) |
| forgia-water | `crates/forgia-water/Cargo.toml` | `bevy_water = "0.18"` (L13) | **`bevy_water` est totalement absent de `[workspace.dependencies]`** → déclaré en dur ici ET dans observability (confirmé : `Cargo.toml` racine L96-124 n'a aucune ligne `bevy_water`) |

**Action recommandée (safe maintenant)** : ajouter `bevy_egui`, `bevy_water`, `bevy_kira_audio`, `leafwing-input-manager` (déjà L100/102/103 pour 3 d'entre eux) dans `[workspace.dependencies]`, ajouter la ligne **`bevy_water`** manquante, puis remplacer les pins en dur par `{ workspace = true }`. Ça ne change *aucune* version (toujours 0.18/0.39.1/0.25/0.20) → 0 risque ship, et le jour J le bump se fait en **6 lignes** dans un seul fichier.

### 3.3 — Doc stale `build-stack.md` (À CORRIGER)

`.claude/rules/build-stack.md` cite comme stack :
- **`lightyear 0.26.4`** (networking) et **`bevy_mod_scripting 0.19`** (scripting Luau).

**Aucun des deux n'est une dépendance réelle du workspace V2** (grep `Cargo.toml` = 0 hit ; absents de `[workspace.dependencies]` L96-124 et des `Cargo.lock` consommés par le code). La table concept-first les mentionne aussi indirectement (colonne Script « Lua »). **Doc à corriger** : retirer ou marquer « non utilisé V2 / différé » ces deux lignes pour ne pas induire un futur agent en erreur lors de la migration (il chercherait à bumper des crates inexistantes).

---

## 4. Breaking changes Bevy 0.19 → carte par area

> Légende risque : 🔴 high · 🟠 medium · 🟢 low. Compte = sites réels trouvés dans la carte d'impact (à recouper avec §9, plusieurs comptes sont sous-estimés).

### 4.1 — `ecs-components` (Resources = Components) — **2 sites high + comportementaux**

Changement de fond : les Resources sont désormais stockées comme composants sur des entités-singleton portant `IsResource`. **Les `Query<Entity>` larges itèrent maintenant aussi les ressources.**

| Site | Risque | Patch |
|---|---|---|
| `crates/forgia-observability/src/migration_baseline.rs:82` — `all_entities: Query<Entity>` | 🔴 | Ajouter `Without<IsResource>`. Sinon `boot_entity_count` (L93) gonfle du nb de ressources → fausse alerte `entity_delta_pct` (L127-129). **Ironie : le sensor censé détecter les régressions de migration est lui-même faussé par elle.** |
| `crates/forgia-mode-roguelite/src/load_timing.rs:36` — `q_all: Query<Entity>` | 🔴 | Ajouter `Without<IsResource>`. Sinon le compteur `entities` du log FREEZE (L40) est décalé d'un offset constant (le delta reste valide, la valeur absolue trompe). |
| `crates/forgia-qa-autopilot/src/soak.rs:160` — `entities().len()` | 🟢 | Aucun fix compile. Comportement : inclut les ressources, mais le test compare un delta → OK. Note seulement. |

**Stratégie** : factoriser `type AllEntities<'w,'s> = Query<'w,'s, Entity, Without<IsResource>>`. Vérifier le chemin d'import exact (`bevy::ecs::resource::IsResource` probable). Les ~55 autres `Query<Entity, With<X>>` sont **immunisés** (le filtre `With<X>` exclut naturellement les entités-ressources).
**Non impacté** : aucun `derive(Component, Resource)` doublé, aucune ressource non-send, aucun appel bas-niveau World/Access/`resource_id`/`clear_entities`.

### 4.2 — `assets` — **~38 sites (mass-rename + 1 high)**

Deux gros chapitres + un point dur :

**BC3 — `bevy_scene` → `bevy_world_serialization` (mass-rename `Scene`→`WorldAsset`, `SceneRoot`→`WorldAssetRoot`)** — risque 🟢 par site mais **volume élevé**. Hotspots :
- `forgia-prefab/src/lib.rs:25,112,117` (crate centrale, consommée par village-loader + stage)
- `forgia-mode-fps-arena/src/lib.rs:430+` (~25 `Handle<Scene>` + ~10 `SceneRoot`)
- `forgia-mode-roguelite/src/decor.rs:33` (hotspot fichier : Vec<Handle<Scene>> L356-363, SceneRoot L732/769/1004/1036/1070/1113)
- `forgia-rpg/src/{lib.rs:1910,2418, character.rs:28, worldgen_village.rs:18}`, `forgia-terrain/src/lod.rs:187`, `forgia-viewmodel/src/attach.rs:32`, `forgia-stage/src/lib.rs:829`, `forgia-foliage/src/lib.rs:337`, `forgia-observability/src/assets_load_sensor.rs:16`, et les `SceneRoot` de `boss_portal.rs:13`, `merchant.rs:34`, `loot_room.rs:22`, `run.rs:14`, `waves.rs:110`, `cyber_city.rs:89`, `wave.rs:418`.
- ⚠️ **Pièges à NE PAS toucher** : `GltfAssetLabel::Scene(0)` et les strings `#Scene0` sont des **labels glTF** (bevy_gltf), pas le type bevy_scene → inchangés. `forgia-rig-topology/src/lib.rs:404` `"SceneRoot"` = **nom de bone string**, pas le type. `cyber_city.rs:69` `CyberCitySceneRoot` = marker maison.
- **Stratégie** : sed workspace piloté par `cargo check`, *après* avoir exclu les faux positifs glTF.

**BC2 — GLTF material `Handle<StandardMaterial>` → `Handle<GltfMaterial>`** — 🔴 **1 site dur** :
- `forgia-worldgen/src/spawn.rs:151` — `prim.material.clone()...` puis `MeshMaterial3d(material)` (L154) exige `StandardMaterial`. Résoudre via le label `/std` du `GltfMaterial` (ou `Assets<GltfMaterial>`). **Seul vrai blocage de type de l'area assets.**

**BC4 — `Assets::get_mut() → Option<AssetMut<A>>`** — 🟢 **source-compatible** (Deref vers `&mut A`, `Some(mat)` matche toujours). Sites : `forgia-effects/src/{lib.rs:108, mesh_fader.rs:112}`, `mode-roguelite/src/{element_vfx.rs:114, shockwave.rs:454}`, `enemy-nameplate/src/lib.rs:331`, `audio/src/biome.rs:106`. **Aucun fix requis** (bénéfice : events `Modified` plus précis). Surveiller un éventuel clippy `needless_borrow`.

**Non impacté** : `get_pixel`/`set_pixel` (0), `AssetPath::resolve` (0), `get_full_extension` (0), `DynamicSceneBuilder::from_world` (0 — tous les `from_world` sont des méthodes maison `Hex`/`ChunkCoord`).

### 4.3 — `rendering-materials` — **14 sites, mais RISQUE n°1 du projet (`forgia-postprocess`)**

**HOTSPOT unique = `forgia-postprocess`** (seul endroit touchant le pipeline rendu bas-niveau). Tout passe par `FullscreenMaterial::node_edges() -> Vec<InternedRenderLabel>`, directement cassé par **BC#1 (render-graph → systèmes ECS, `RenderLabel`/`Node` trait supprimés)** + **BC#15 (`FullscreenMaterial` → `schedule_configs()`)** :

| Site | Risque | Patch |
|---|---|---|
| `forgia-postprocess/src/lib.rs:48` — import `render_graph::{InternedRenderLabel, RenderLabel}` (macro `define_simple_pp_effect!` ×43) | 🔴 | Supprimer import render_graph + override `node_edges()`. Ré-ordonner via `schedule_configs()` dans Core3d. **Point de migration central : 1 fix dans la macro propage à 43 effets.** |
| `forgia-postprocess/src/lib.rs:73` — `fn node_edges() -> Vec<InternedRenderLabel>` (×43) | 🔴 | API d'ordering 0.19. Un effet mal ordonné devient **silencieusement passthrough**. |
| `forgia-postprocess/src/lib.rs:43` — import `fullscreen_material::{FullscreenMaterial, ...}` | 🟠 | Vérifier que le chemin reste valide en 0.19 (peut migrer vers `bevy_material`/`bevy::post_process`). |
| `forgia-postprocess/src/toon.rs:40,12` — `node_edges()` hand-written (**WIRED en Roguelite**, `mode-roguelite/lib.rs:138`) | 🔴 | Migrer identique. Tester visuellement le cel-shading (sensor `toon_config.rs` surveille `toon_attached`). |
| `forgia-postprocess/src/outline.rs:43,13` — `node_edges()` (plugin désactivé crash wgpu, **mais code compile**) | 🔴 | Doit migrer pour que la crate compile. Note : le crash venait justement de l'ordering `node_edges` → la migration est l'occasion de le fixer. |

Autres sites rendu (mécaniques, low/medium) :
- `forgia-player/src/{skybox_genome.rs:212, lib.rs:366}` — **BC#23 `Skybox.image → Option<Handle<Image>>`** 🟠 : wrapper en `Some(...)`. Erreur de compile mécanique.
- `forgia-player/src/lib.rs:6` — import `Skybox` (surveiller chemin) 🟢.
- `forgia-observability/src/render_sensor.rs:17` — `Bloom` (read-only via `Has<Bloom>`) 🟢 : aucun re-tuning, juste surveiller le chemin `bevy::post_process::bloom`.
- `forgia-effects/src/weapon_vfx/tracer.rs:146` + ~12 sites `AlphaMode::Blend/Add` (`run.rs:420/550`, `poi.rs:432`, `shockwave.rs:387/418`, `character.rs:634`, etc.) — **BC#19 `AlphaMode` → `bevy_material`** 🟢 : via prelude → quasi sans changement. Si erreur `unresolved import` après bump, ajouter `use bevy::pbr::AlphaMode;`.
- `forgia-combat/src/combat_juice.rs:11` — `ChromaticAberration` déjà commenté 🟢.

**Non impacté (faux positifs écartés)** : Atmosphere (= `DistanceFog`+`AmbientLight` thématique, pas le composant Bevy), `bevy_default()` (0), aucun `Material`/`AsBindGroup`/`SpecializedMeshPipeline` custom.

### 4.4 — `ui-text` — **2 sites trivial**

Forgia UI = **quasi 100% egui** (bevy_egui), pas bevy_text natif. Seuls 2 sites Text2d :
- `forgia-fps/src/score.rs:62` — `TextFont { font_size: 36.0 }` → 🟠 `font_size: FontSize::Px(36.0)`. **BC#4** (compile fail sinon).
- `forgia-effects/src/damage_numbers.rs:52` — `font_size` (local f32 du genome `HitFeedback`) → 🟠 `font_size: FontSize::Px(font_size)`. Les champs genome `head/body/limb_font_size: f32` (`forgia-damage`) **restent f32** — seul le pont vers `TextFont` change.

Ni l'un ni l'autre ne fixe le champ `font` → **BC#3 (`Handle<Font>` → `FontSource`) ne s'applique pas**. ⚠️ Tous les `font:`/`TextFont` de `forgia-ui-lib`/`mode-roguelite`/`killfeed` sont **egui** (intouchés). Tous les `Node {` sont des types domaine (GraphNode/StageNode…). **⚠️ Voir §9 : ce compte de 2 est contesté par le completeness critic** (124 occurrences UI à re-vérifier).

### 4.5 — `ecs-scheduling` — **7 sites `SystemParam` (low) — mais compte sous-estimé ×100 (§9)**

**BC3 — `SystemParam` validation différée au fetch-time** (au lieu de la registration). 7 `#[derive(SystemParam)]` : `forgia-fps/src/lib.rs:{221,230,263,277}`, `forgia-fps/src/ammo_systems.rs:28`, `forgia-viewmodel/src/genome.rs:253`, `forgia-stage/src/lib.rs:277`. **Aucun rename, aucun fix compile.** Risque comportemental : un panic « Res manquante » se déplace de la registration au 1er run.
**Action** : auditer l'ordre d'init des `Res` non-optionnelles dans `HitscanCtx` (L277 : HitscanSensorState, HitFeedback, PlayerCombatMods, PepinConfidence, PepinTuning), `AmmoCtx` (EquippedWeapons), `LayoutParams` (LevelModulesHandles, LayoutResult) — vérifier qu'elles sont init au plugin build, pas lazy `on_enter`.
**Non impacté** : `type_id→system_type` (0), `ExecutorKind` (0), `DefaultErrorHandler` (0), `SystemBuffer::queue` (0), Task drop WASM (0 — pipeline async documenté mais inexistant en code, `forgia-streaming/lib.rs:308`).

### 4.6 — Areas à **0 site** dans la carte (à re-vérifier — §9)

| Area | Carte | Note |
|---|---|---|
| `reflection` | 0 | Forgia n'enregistre pas de resources réflectives. Seuls `TypePath`/`FromReflect`/`Reflect` (genome assets) restent re-exportés. **⚠️ §9 conteste** (`#[derive(Reflect)]` sur `Actionlike` `forgia-input/lib.rs:39`). |
| `ecs-queries` | 0 | Aucun `Ref<T>` Bevy (tous les `Ref<` sont `AsRef<Path>`), aucun WorldQuery custom. Les `QueryFilter` sont **rapier**, pas Bevy. ✅ crédible. |
| `ecs-events-observers` | 0 | Observers `On<Add/Insert>` jettent le trigger via `_:` (`lifecycle_sensor.rs:46-65`). **⚠️ §9 conteste violemment** (242 occurrences Event/EventReader). |
| `math-transform` | 0 | Aucun `Affine3`/`Frustum`/`ViewFrustum`. Les `primitives::Aabb` sont une autre area. ✅ crédible. |
| `input` | 0 | Tout passe par leafwing. Aucun `InputFocus`/`InputDispatchPlugin`/picking. **⚠️ §9 conteste** (196 occurrences ButtonInput/KeyCode natifs). |
| `window-app` | 1 (informatif) | `forgia-game/src/lib.rs:32` configure `WindowPlugin` (title/resolution only). Pas d'exit system custom. **⚠️ §9 conteste** (132 occurrences cursor/PresentMode). |
| `animation` | 0 | Animation 100% procédurale (`forgia-anim-locomotion`, spring bones). Zéro `AnimationPlayer`/`AnimationClip`/`AnimationTargetId`. ✅ crédible (confirmé audit interne). |
| `audio` | 2 (Cargo.toml) | Tout l'audio = bevy_kira_audio (kira/Symphonia), aucun `bevy_audio` natif. Voir §4.7. ✅ crédible. |
| `misc` | 1 (Cargo.toml) | Aucun flag feathers/ui_widgets/multi_threaded utilisé. ✅ crédible. |

### 4.7 — `audio` + features Cargo — **1 site clé : `Cargo.toml:98`**

**BC#1 (audio devient default-feature explicite) + BC#4 (flags rodio restructurés, défaut = vorbis only)**. Tout l'audio runtime = **bevy_kira_audio** ; `bevy_audio` (rodio) n'est consommé par **aucune ligne de code** mais tiré par `wav`/`mp3`/`vorbis` (L98).
- **Option recommandée (B)** : `default-features = false` + ré-énumérer les features réellement utilisées (render/winit/ktx2/basis-universal/webp/jpeg/file_watcher/bevy_dev_tools) + **retirer wav/mp3/vorbis** → élimine bevy_audio + rodio 0.22. ⚠️ Sans `default-features = false`, l'audio revient silencieusement en 0.19.
- Vérif post : `cargo tree -p bevy -e features | grep -i audio` = vide.
- Le seul `.mp3` du jeu (`forgia-audio/src/biome.rs:63`, Jungle ambience) passe par **kira/Symphonia**, indépendant des flags rodio → vérifier juste que kira garde le support mp3.

---

## 5. Rust 1.94 → 1.96 — ce qui casse un build clippy-0-warning

> **Risque principal = clippy**, pas rustc. La politique projet (`Cargo.toml:72-94`, plusieurs lints en `warn`) + l'objectif 0-warning rendent les **lints `complexity` warn-by-default** bloquants.

### 5.1 — Nouveaux lints clippy `complexity` (warn-by-default → BLOQUANTS)

| Lint | Version | Patterns présents dans le repo | Grep |
|---|---|---|---|
| `manual_checked_ops` | 1.95 | arithmétique + test overflow (locomotion, character, asset-cdn) | `checked_add`, `checked_sub`, `checked_mul`, `if .* > .* { .* -` |
| `manual_take` | 1.95 | `mem::replace(&mut x, Default::default())` → `mem::take` | `mem::take`, `mem::replace`, `Default::default()` |
| `manual_option_zip` | 1.96 | `a.and_then(|x| b.map(|y| (x,y)))` → `a.zip(b)` (foliage, viewmodel, observability) | `.and_then(`, `.map(` |
| `manual_pop_if` | 1.96 | test sur `last()`/`back()` + `pop()` → `pop_if` (loot_room, console) | `.pop()`, `.last()` |
| `manual_noop_waker` | 1.96 | construction manuelle `Waker` no-op (peu probable, jeu Bevy) | `RawWaker`, `Waker::from_raw` |

26 occurrences candidates sur 18 fichiers détectées au recon. **Mitigation** : `cargo clippy --workspace --fix --allow-dirty` puis re-vérifier manuellement les 5 lints.

**Inertes ici** : `disallowed_fields` (warn mais nécessite `clippy.toml` configuré — absent), `unnecessary_trailing_comma` + `duration_suboptimal_units` (pedantic → non actifs, le workspace n'active pas `clippy::pedantic`).

### 5.2 — Changements rustc (peuvent émettre warnings/erreurs)

| Changement | Version | Risque Forgia |
|---|---|---|
| **`uninhabited_static` deny-by-default + reporté dans les deps** | 1.96 | 🟠 Le plus susceptible de casser un build vert **sans action de notre code** : si une dep (Bevy/transitive) a un static d'un type non-habité → ERREUR. Surveiller au 1er build après `rustup update`. |
| `ambiguous_glob_imported_traits` (future-incompat) | 1.95 | 🟠 Bevy = `use bevy::prelude::*` massif. Nouveau warning si trait importé ambigu via glob → casse 0-warning. |
| Déprécation `Eq::assert_receiver_is_total_eq` | 1.95 | 🟢 Rare (impls `Eq` manuels). |
| Conflit attribut derive-helper vs built-in (future-compat) | 1.95 | 🟢 Possible avec macros Bevy/serde. |
| Imports `self` plus stricts (`use $crate::{self}`, `use S::{self}`) | 1.95/1.96 | 🟢 Vérifier code macro/réexport. |
| RPITIT type trop privé = erreur | 1.96 | 🟢 Si trait avec `-> impl Trait` exposant type privé. |
| Précédence `export_name`/`link_name` | 1.96 | 🟢 Improbable (jeu pur Rust, pas de FFI). |

**Non breaking maintenant** : Range types Copy (RFC3550) — `0..1` produit toujours les types legacy en edition 2021. Aucun impact.

---

## 6. PROCESS DE DEBUG/PATCH ordonné (LE RUNBOOK)

> Reprenable, sur **worktree dédiée**, ordonné par dépendance. **Ne pas démarrer avant §2 toutes vertes** (rapier + hanabi publiés 0.19 ; water forké si encore dormant). Gate par crate = `cargo check -p <crate>`. Ordre : **feuilles → orchestrateurs**.

### Étape 0 — Pré-conditions (gate d'entrée)
- [ ] `bevy_rapier3d` 0.34+ ciblant Bevy 0.19 publié (PR #694 mergée + release) — **OU** fork local validé
- [ ] `bevy_hanabi` 0.19 publié — **OU** fork local
- [ ] `bevy_water` 0.19 publié — **OU** fork local (le plus probable, cf §2)
- [ ] Drift workspace-dep corrigé (§7.1) déjà mergé sur `main`
- [ ] `build-stack.md` corrigé (§7.5)
- [ ] Baseline sensors capturée (§7.4) commitée comme référence runtime

### Étape 1 — Worktree + branche isolée
```bash
cd <racine-du-workspace>
git worktree add ../forgia-migrate-019 -b migrate/bevy-019-rust-196
cd ../forgia-migrate-019
```
- [ ] Worktree créée (isole la migration du ship-track `main`)
- [ ] Tag de rollback : `git tag pre-migrate-019` sur le commit de départ

### Étape 2 — Toolchain Rust
```bash
rustup update stable          # 1.94 → 1.96
rustc --version               # confirmer 1.96.0
rtk cargo clippy --workspace --fix --allow-dirty   # neutralise les 5 lints complexity AVANT le bump Bevy
rtk cargo clippy --workspace -- -W warnings        # doit être 0 warning sur Bevy 0.18 encore
```
- [ ] Rust 1.96 actif
- [ ] clippy 0-warning sur Bevy 0.18 (isole les régressions Rust des régressions Bevy)
- [ ] Si `uninhabited_static`/`ambiguous_glob` rouge → traiter ici, séparément du bump Bevy

### Étape 3 — Bump des versions (1 seul fichier si §7.1 fait)
Éditer `Cargo.toml` `[workspace.dependencies]` (L96-103) :
```toml
bevy = { version = "0.19", default-features = false, features = [/* §4.7 option B, sans wav/mp3/vorbis */] }
bevy_rapier3d = { version = "0.3X", features = ["debug-render-3d"] }   # version 0.19-compatible
bevy_egui = "0.40.0"
bevy_hanabi = { version = "0.19", default-features = false, features = ["3d"] }
bevy_kira_audio = "0.26.0"
leafwing-input-manager = "0.21.0"
bevy_water = "0.19"   # ou patch.crates-io vers le fork local
```
- [ ] `cargo update` (vérifier `Cargo.lock` cohérent, glam unifié)
- [ ] **NE PAS** lancer `cargo build` workspace entier ici (bruit illisible). Passer crate par crate (Étape 4).

### Étape 4 — Fix compile crate par crate (FEUILLES d'abord)
Ordre par profondeur du graphe de dépendances. Gate = `rtk cargo check -p <crate>` vert avant de passer à la suivante.

**Tier A — feuilles / fondations (peu/pas de dep interne)** :
- [ ] `forgia-core`, `forgia-rng`, `forgia-genome-core`, `forgia-input` (← `Reflect` sur Actionlike, §4.6)
- [ ] `forgia-damage`, `forgia-combat`

**Tier B — feuilles rendu/assets (les patchs durs vivent ici)** :
- [ ] **`forgia-postprocess`** ← #1 priorité (§4.3, macro `node_edges` ×43 + toon + outline)
- [ ] `forgia-prefab` (← BC3 Scene rename, crate centrale)
- [ ] `forgia-water` (← bevy_water 0.19/fork)
- [ ] `forgia-effects` (← BC4 get_mut, AlphaMode), `forgia-foliage`, `forgia-terrain`
- [ ] `forgia-player` (← Skybox `Some()`, §4.3), `forgia-viewmodel`, `forgia-enemy-nameplate`
- [ ] `forgia-worldgen` (← **BC2 GltfMaterial `/std`, spawn.rs:151 high**)
- [ ] `forgia-audio`

**Tier C — orchestrateurs / modes (consomment Tier A+B)** :
- [ ] `forgia-fps` (← SystemParam audit + `score.rs:62` FontSize), `forgia-crosshair`, `forgia-stage`
- [ ] `forgia-rpg`, `forgia-rpg-data`, `forgia-mode-fps-arena` (← gros volume Scene rename)
- [ ] `forgia-mode-roguelite` (← decor.rs Scene rename, load_timing.rs `Without<IsResource>`)
- [ ] `forgia-ui`, `forgia-ui-lib`, `forgia-killfeed`, `forgia-debug`
- [ ] **`forgia-observability`** (← `migration_baseline.rs:82` + `assets_load_sensor.rs:16` + render_sensor)

**Tier D — top / binaires + QA** :
- [ ] `forgia-game` (← `WindowPlugin`, cyber_city Scene), `src/main.rs`
- [ ] `forgia-qa-core`, `forgia-qa-replay`, `forgia-qa-harness`, `forgia-qa-autopilot` (← soak.rs note), `xtask`

### Étape 5 — Build complet + clippy
```bash
rtk cargo build --workspace
rtk cargo clippy --workspace -- -W warnings    # objectif 0 warning (politique projet)
```
- [ ] `cargo build` vert sur tout le workspace
- [ ] clippy 0 warning
- [ ] `cargo xtask no-scaffold` + `arch-drift` + `story-gate` passent

### Étape 6 — Shaders WGSL (angle mort .rs, cf §9)
- [ ] Auditer `assets/shaders/**` (65 fichiers .wgsl) — **toon.wgsl + outline.wgsl sont RÉELS et wirés** (43/45 stubs passthrough). Vérifier `#import bevy_pbr::...`, bindings `@group/@binding`, view uniforms vs conventions 0.19.
- [ ] Lancer le jeu : un shader cassé compile-OK mais rend noir/passthrough au runtime.

### Étape 7 — Validation runtime via sensors `forgia2_*.json`
> Le binaire réel = **`forgia.exe`** (pkg `forgia`, `cargo build -p forgia`), PAS `forgia-game` (exe stale silencieux — cf MEMORY). Vérifier `mtime(artefact) > mtime(source)` avant tout diagnostic.

Sensors à comparer contre la **baseline pré-migration** (§7.4) :
- [ ] `forgia2_render.json` — `mesh3d_visible` vs `total` (écran brun = régression rendu, le plus probable après migration postprocess/skybox)
- [ ] `forgia2_combat.json` (damage_dir, hitscan, hud_ammo, killfeed, screen_flash) — gameplay FPS intact
- [ ] `forgia2_physics.json` — gravité/vélocité (régression rapier)
- [ ] `forgia2_rpg_player.json` (swim/depth) + `forgia_water.json` — eau (régression bevy_water)
- [ ] `forgia_chunks_snapshot.json` / `forgia_terrain.json` — streaming
- [ ] sensor `toon_config.rs::toon_attached` — cel-shading wired (régression FullscreenMaterial)
- [ ] `migration_baseline.rs` → `entity_delta_pct` cohérent (vérifie que le filtre `Without<IsResource>` est bien appliqué)
- [ ] `forgia2_inventory/quests/npcs.json` — systèmes RPG

**Test runtime structuré** :
1. **Action** : `cargo run -p forgia` → lancer Roguelite, jouer 1 run complet (spawn → combat → boss portal)
2. **Rechargement** : rebuild complet (pas hot-reload — c'est un bump de moteur)
3. **Effet attendu** : jeu identique pré-migration (rendu, tirs, particules, audio)
4. **Où observer** : les `forgia2_*.json` ci-dessus = égaux à la baseline
5. **Variantes si KO** : écran brun → `forgia2_render.json::mesh3d_visible==0` → postprocess/skybox ; pas de particules → hanabi ; pas de collisions → rapier ; exe stale → `mtime` check.

### Étape 8 — Merge ou rollback
- [ ] Tout vert + runtime conforme → `git checkout main && git merge migrate/bevy-019-rust-196`
- [ ] **Rollback** : `git worktree remove ../forgia-migrate-019` (la branche est isolée, `main` jamais touché) ; ou `git reset --hard pre-migrate-019` ; restaurer toolchain `rustup default 1.94` si besoin.

---

## 7. Pré-travaux possibles MAINTENANT (sans casser le pin ship-first)

> Tous **0 risque ship** (aucun changement de version Bevy/rapier) et réduisent fortement le coût/risque du jour J.

### 7.1 — Corriger le drift workspace-dep 🟢 (le plus rentable)
- Ajouter dans `Cargo.toml` `[workspace.dependencies]` : la ligne **`bevy_water = "0.18"`** (manquante), et s'assurer que `bevy_egui`/`bevy_kira_audio`/`leafwing` y sont (déjà L100/102/103).
- Remplacer les pins en dur par `{ workspace = true }` : `forgia-observability/Cargo.toml` L25-27, `forgia-debug/Cargo.toml` L12, `forgia-water/Cargo.toml` L13.
- **Versions inchangées** → le jour J, le bump = 6 lignes dans 1 seul fichier au lieu de 5 fichiers.

### 7.2 — Doc de tracking des bloqueurs 🟢
- Créer `docs/migration/bevy-019-blockers.md` avec le tableau §2 + cases à cocher, à relire mensuellement (rapier #694, hanabi CHANGELOG, bevy_water branches). C'est le déclencheur de l'Étape 0.

### 7.3 — Wrappers fins sur les APIs volatiles 🟠 (optionnel, à évaluer)
- **Scene** : si `forgia-prefab` exposait déjà un type-alias `pub type GameScene = bevy::scene::Scene;` consommé par les ~38 sites, le rename BC3 deviendrait **1 ligne**. (Coût : refactor préparatoire ; bénéfice : énorme sur le mass-rename.)
- **FullscreenMaterial** : difficile à wrapper (l'API change de paradigme). Ne pas sur-investir.

### 7.4 — Capturer une baseline sensors pré-migration 🟢
- Lancer `forgia.exe`, jouer 1 run Roguelite + 1 session RPG, archiver tous les `forgia2_*.json` / `forgia_*.json` dans `docs/migration/baseline-sensors-pre-019/`. C'est la **référence de non-régression runtime** de l'Étape 7.

### 7.5 — Corriger `build-stack.md` 🟢
- Retirer/marquer « non utilisé V2 » les lignes `lightyear 0.26.4` et `bevy_mod_scripting 0.19` (cf §3.3 : 0 hit Cargo.toml). Évite qu'un futur agent cherche à bumper des crates inexistantes.

### 7.6 — Pré-fix clippy 1.96 sur Bevy 0.18 🟢
- Une fois Rust 1.96 dispo (déjà le cas), faire tourner `cargo clippy --fix` sur les 5 lints complexity **dès maintenant sur main** (sans toucher Bevy). Sépare le risque Rust du risque Bevy le jour J.

---

## 8. Estimation effort + risque par crate

| Crate | Sites | Risque | Effort | Cause principale |
|---|---|---|---|---|
| **forgia-postprocess** | 7 (×43 expansions) | 🔴 **High** | ~1-2 j | render-graph→ECS, `node_edges`, toon wired + outline |
| forgia-worldgen | 1 | 🔴 High | ~2-4 h | GLTF `GltfMaterial`/`/std` (spawn.rs:151) |
| forgia-mode-fps-arena | ~35 | 🟠 Medium | ~3-4 h | mass-rename Scene (volume) |
| forgia-mode-roguelite | ~20 + 1 sensor | 🟠 Medium | ~3 h | Scene rename + `load_timing` IsResource |
| forgia-rpg | ~15 | 🟠 Medium | ~2-3 h | Scene rename + test plugin |
| forgia-observability | 3 | 🟠 Medium | ~2 h | `migration_baseline` IsResource + assets_load_sensor + render_sensor |
| forgia-prefab | 3 | 🟢 Low | ~1 h | Scene rename (crate centrale, à faire tôt) |
| forgia-terrain / lod | ~8 | 🟢 Low | ~1 h | Scene rename + WGSL terrain à recouper |
| forgia-player | 3 | 🟢 Low | ~1 h | Skybox `Some()` |
| forgia-viewmodel / attach | ~6 | 🟢 Low | ~1 h | Scene rename |
| forgia-stage | 2 + SystemParam | 🟢 Low | ~1 h | Scene rename + LayoutParams init order |
| forgia-effects | 4 | 🟢 Low | ~30 min | get_mut (no-op) + AlphaMode |
| forgia-fps | 4 SystemParam + 1 | 🟢 Low | ~1 h | FontSize + audit init Res |
| forgia-foliage | 1 | 🟢 Low | ~30 min | Scene rename + material_override WGSL |
| forgia-audio | 1 | 🟢 Low | ~30 min | get_mut (no-op) + features Cargo |
| forgia-enemy-nameplate | 1 + WGSL | 🟢 Low | ~30 min | get_mut + nameplate_hp.wgsl |
| forgia-game | 2 | 🟢 Low | ~30 min | WindowPlugin (info) + cyber_city Scene |
| Cargo.toml (features) | 1 | 🟠 Medium | ~1 h | audio default-features=false (§4.7) |
| **Shaders WGSL** | 65 fichiers | 🟠 **Medium-High** | ~1 j | angle mort (toon/outline réels, §9) |
| Bumps tiers + Cargo.lock | — | 🔴 **High (gating)** | — | gated upstream (§2) |

**Effort total estimé (post-déblocage)** : ~5-8 jours-homme, dont **~50% concentré sur forgia-postprocess + shaders WGSL + le mass-rename Scene**. Le reste est mécanique.

---

## 9. Trous connus / à re-vérifier (completeness critic)

> La carte d'impact fournie **sous-estime massivement** plusieurs areas. Le critic est *faible confiance dans la carte, élevée dans son diagnostic*. Contexte : le workspace étant pinné 0.18.1, la carte semble générée par scan partiel. **À re-faire exhaustivement le jour J, avant l'Étape 4.**

**Gaps les plus graves (à ne PAS croire « 0 site ») :**
- 🔴 **`ecs-events-observers` = 0 → FAUX.** 242 occurrences `#[derive(Event)]` + EventReader/Writer/add_event/add_message sur 75 fichiers (forgia-fps/lib.rs 14, rpg-data/dialogue.rs 12, quests.rs 10, shop.rs 12…). Le rename **Event→Message** est un des plus gros chapitres récents. **Gap n°1.**
- 🔴 **`ecs-scheduling` = 7 → sous-compté ×100.** 731 occurrences OnEnter/OnExit/run_if/in_state/in_set/before/after/chain/add_systems sur 125 fichiers (mode-roguelite/lib.rs 123 à lui seul). Tout changement schedule/state-scoped touche des centaines de sites.
- 🟠 **`input` = 0 → FAUX.** 196 occurrences ButtonInput/KeyCode/MouseButton natifs sur 34 fichiers (au-delà de leafwing). + leafwing 0.20→0.21 à bumper.
- 🟠 **`ui-text` = 2 → sous-compté.** 124 occurrences Text2d/Node/Val/TextFont/UiRect sur 19 fichiers (à re-vérifier même si majorité egui).
- 🟠 **`window-app` = 1 → sous-compté.** 132 occurrences PrimaryWindow/Window/cursor/CursorGrabMode/PresentMode. Le **cursor grab a migré (CursorOptions component)** — chaque `set_cursor` suspect (forgia-ui/lib.rs 34, pause_menu.rs 22, camera-orbit 14).
- 🟠 **`rendering-materials` = 14 → sous-estime le risque.** Les 45 `impl FullscreenMaterial` câblés render-graph = l'API la plus volatile entre versions Bevy. Compté comme une poignée.
- 🟡 **`reflection` = 0 → douteux.** Au moins `#[derive(Reflect)]` sur `Actionlike` (forgia-input/lib.rs:39). register_type/ReflectComponent à auditer.

**Catégories totalement absentes de la carte (angles morts) :**
- 🔴 **Dépendances tierces (le vrai bloqueur)** — déjà couvert §2/§3, mais c'est *hors* du scan .rs.
- 🔴 **65 shaders `.wgsl`** (`assets/shaders/**`, post_process ×~50, layers, v1-port ×7) — **invisibles au scan .rs**. Conventions WGSL Bevy (imports, bindings, view uniforms) changent souvent. toon.wgsl + outline.wgsl RÉELS. **Couvert Étape 6.**
- 🟠 **Bevy feature flags** (ktx2/basis-universal/webp/dev_tools/file_watcher, `Cargo.toml:98`) — disponibilité/noms changent entre versions, à valider 1 par 1.
- 🟠 **`load_internal_asset`/`embedded_asset`/`weak_handle`** (~30 fichiers) — chargement interne de shaders a changé ; vérifier que les 38 sites assets couvrent bien la branche shader-handles, pas que `asset_server.load` gameplay.
- 🟠 **Custom `SystemParam`** (forgia-stage, forgia-fps, forgia-viewmodel) — cassent souvent sur les changements de lifetimes/world-access entre versions.
- 🟡 **Plugin `build()` × ~18+** — signatures et ordres d'`add_plugins`/`init_resource`/`add_event` = points de rupture classiques.

**Recommandation** : avant l'Étape 4 du runbook, **relancer une cartographie exhaustive** (ripgrep et recherche sémantique sur Event/Message, schedule, cursor, WGSL imports) sur le code *réel à jour*, et **ne pas se fier aux comptes de cette carte**. Le jour J, le compilateur (`cargo check -p <crate>` crate par crate) est la source de vérité finale — il listera tout ce que la carte a raté.

---

*Dossier de préparation — Forgia V2. Migration GATED sur bevy_rapier3d + bevy_hanabi + bevy_water (0.19). Aucun code modifié. À exécuter via le runbook §6 le jour où §2 est tout vert. Pré-travaux §7 exécutables dès maintenant sans risque ship.*
