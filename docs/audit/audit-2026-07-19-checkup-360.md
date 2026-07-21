# RAPPORT AUDIT 360° FORGIA V2 — 2026-07-19

> Checkup complet avant la suite du ship Roguelite. 4 agents parallèles (Architect / Dev Senior / QA / Perf)
> + gates mécaniques xtask + contre-vérifications manuelles des findings clés.
> **Périmètre** : workspace V2 `crates/**` + `xtask/` (PAS le V1 `D:/Forgia`).
> **Caveats** : arbre chaud (autre terminal sur `forgia-mode-roguelite` — findings tagués [CHAUD] à re-vérifier post-merge) ;
> sensors runtime datés du 2026-07-03 (16 j, aucune conclusion « état actuel » sans relance) ; perf sections alloc/queries = couverture partielle.

---

## Résumé exécutif

- **Score global : 47/100** (formule skill : 100 − 5×Critique − 3×Haut − 1×Moyen, sur problèmes **consolidés**, pas par occurrence : 3 C, 8 H, 14 M)
- **0 bug bloquant / 0 crash prod trouvé.** 1 seul bug runtime confirmé (boon chaîne inerte). Le code est **shippable** — la dette est de la qualité/process, pas de la stabilité.
- **Top 3 actions** :
  1. **Vider l'arbre chaud** (16 jours de WIP non commité) — bloque Ship P0, la FSM R2 et la moitié des fixes de cet audit.
  2. **Fixer le boon chaîne** (`boons_apply.rs:189` — 2 boons du catalogue morts en silence).
  3. **Dé-hardcoder le combat récent** (capacités d'armes, roquette, ultimes → genome TOML, la règle no-hardcode est bloquante).

### Points forts (à ne pas casser)

- **0 bloc `unsafe`** réel ; `#![forbid(unsafe_code)]` sur les 4 crates QA.
- **0 cycle de dépendances** ; hubs sains (`forgia-core` : 42 IN / 0 OUT).
- **Invariants V2 tenus** : L7 GameSet chain ✅ (`forgia-core/src/lib.rs:206-235`), 1 seul handler ESC ✅, B0001 ✅ (vérifié : `color_grading.rs` mute via `Commands`), MenuCamera2d/FpsCamera séparés ✅.
- **Saves atomiques** (`persist.rs` tmp+rename, %APPDATA%\Forgia) — RAS.
- **Gates mécaniques tiennent** : no-scaffold 0 violation (58 crates), arch-drift exact (62 crates), verify-sensors-format 14/14.
- **0 système >12 params, 0 tuple add_systems >20** — anti-traps V1 respectés.
- Keystone FixedUpdate (latch clic gauche) : câblage propre et gaté (vérifié — les « doublons » signalés étaient des faux positifs de tests).

---

## Gates mécaniques (2026-07-19)

| Gate | Résultat | Détail |
|---|---|---|
| `wip-check` | ❌ **FAIL** | **44 stories IN_PROGRESS > limite 3** — « stop starting, start finishing » non tenu |
| `check-orphans` | ⚠️ placeholder | Le gate anti-plugin-orphelin promis par CLAUDE.md §6 n'est **pas implémenté** |
| `no-scaffold` | ✅ | 0 violation — mais allowlist périmée (3 entrées vers crates supprimées) |
| `arch-drift` | ✅ | ARCHITECTURE.md = exactement les 62 crates |
| `verify-sensors-format` | ✅ | 14/14 canonical |
| `git status` | ❌ | **Sale à l'identique depuis le 2026-07-03 (16 j)** : 19 M + 13 ?? dont 4 modules roguelite (`enemy_scaling`, `forge_shop`, `head_hitbox`, `trempe`) et 4 stories (656-659) jamais commités |

---

## Findings consolidés

### 🔴 CRITIQUES (3)

| # | Finding | Preuve | Action |
|---|---|---|---|
| C1 | **Boon chaîne inerte** (bug runtime confirmé manuellement) : `sys_apply_chain_targets` cible `With<EnemyArchetype>` mais émet `forgia_damage::DamageEvent` ; `apply_damage` ne query que `forgia_damage::Health`, absente des ennemis (leur camp = `forgia_combat::Health`). **0 dégât silencieux** + log `info!("chain hit ×N")` = faux positif de confiance. 2 boons du catalogue morts : `chaine_des_ames` (légendaire), `rebond_du_caillou` (rare). [CHAUD] | `crates/forgia-mode-roguelite/src/boons_apply.rs:189-227` ; `forgia-damage/src/lib.rs:247-258` ; catalogue `assets/genomes/roguelite_boons.toml:122-129,201-207` | Muter `forgia_combat::Health` + `CombatHitEvent` (pattern `elements.rs`/`shockwave.rs`) — ~10 lignes + test. **Après merge arbre chaud.** |
| C2 | **Couche definition violée sur le combat récent** (règle no-hardcode bloquante) : ~20 constantes des 4 capacités d'armes (`CALIN/GUST/PIERCE/BOUM` cd/dégâts/rayons), roquette Boucherie (speed/gravité/explosion), ultimes (`CHAIN_HOP_RADIUS`, `FREEZE_SECS`… alors que `roguelite_ultimate.toml` **existe déjà**). [CHAUD] | `shockwave.rs:33-61`, `boucherie_rocket.rs:38-53`, `ultimate_tech.rs:26-40`, `forgia-combat/src/ultimate.rs:18-20` | Migrer vers `roguelite_abilities.toml` (à créer) + `roguelite_ultimate.toml`. Data-only, hot-reload, zéro risque. |
| C3 | **God-files** : 10 fichiers > 1200 LOC (seuil de la règle fine-grained-crates) ; `forgia-mode-roguelite` = 25 210 LOC = **22,6 % du workspace** dans 1 crate (36 fichiers) ; `Plugin::build` de **601 LOC** (`mode-roguelite/lib.rs:87`) ; + `generate_chunk_lod` = 654 LOC/12 params (plus grosse fn du workspace). | Table complète §Architecture | Split **post-ship** (sauf opportunité au merge) : sub-plugins par domaine, `elements.rs` (69 pubs) en 4 modules, `xtask` en `cmd/*.rs`. |

### 🟠 HAUTS (8)

| # | Finding | Preuve | Action |
|---|---|---|---|
| H1 | **Inversion de couche** : `forgia-ui` (générique) dépend de `forgia-mode-roguelite` → `forgia-fps`/`forgia-viewmodel` tirent transitivement les 25k LOC du mode. Idem `forgia-ui-lib` → `forgia-mode-fps-arena`. | Cargo.toml des crates concernées | Extraire le contrat (events/resources) vers `forgia-ui-lib` ou une crate contrat. |
| H2 | **Process** : wip-check FAIL 44>3 ; arbre sale 16 j ; 4 modules + 4 stories jamais commités = risque de perte sèche. | Gates ci-dessus | Merge/commit coordonné + passe de reclassement des statuts stories. |
| H3 | **Boilerplate genome hot-reload dupliqué ×21 fichiers** (~1000 LOC) : `GENOME_PATH` + poll mtime + `XGenomeWatch` copiés partout ; dont 6 sites « parse silencieux » (`unwrap_or_else(default)`) qui **avalent les erreurs TOML** sans warn (anti-observabilité). | 21 fichiers listés §Dev Senior ; `elements.rs:520`, `ultimate_config.rs:162`, etc. | `GenomeFile<T>` générique dans `forgia-genome-core` (qui existe et n'est consommé par aucun des 21) + warn systématique. 1 story Standard. |
| H4 | **44 TODO « port from V1 »** : squelettes fantômes dans `forgia-combat` (melee entier commenté, combat_juice) et `forgia-effects` (`arena_feedback` : kill/damage sounds **débranchés**). | `forgia-combat/src/melee.rs:13-65`, `combat_juice.rs`, `forgia-effects/src/arena_feedback.rs:16-60` | Décider : porter ou supprimer. Ne pas laisser 44 TODO polluer la navigation. |
| H5 | **Gaps observabilité** : 2 capteurs écrits mais **jamais enregistrés** (`sys_write_assets_sensor`, `sys_write_vram_sensor`) ; `forgia2_rpg_health` stale (sensor_health le signale) ; `check-orphans` placeholder — exactement le gate qui aurait attrapé ces capteurs. | `forgia-observability/src/assets_load_sensor.rs:29`, `vram_sensor.rs:91` | Brancher les 2 capteurs, fixer le producer rpg_health, implémenter check-orphans réel. |
| H6 | **2 crates orphelines mortes** : `forgia-weapon-hitscan` (150 LOC, jamais importée — le hitscan réel vit dans forgia-fps) et `forgia-qa-autopilot` (822 LOC, jamais importée) ; + allowlist no-scaffold : 3 entrées vers crates supprimées. ⚠️ **Correction post-audit (2026-07-19)** : le claim « dep du bin racine » de l'agent était FAUX — les lignes 129-190 du Cargo.toml racine sont des `[workspace.dependencies]` (déclarations), le bin `forgia` ne dépend que de `forgia-game`. Coût réel des orphelines = bruit de navigation + temps `--workspace`, PAS du bloat binaire. **Preuve de pourrissement (story-662)** : 2 tests de qa-autopilot (`smoke_bot_*`) échouent — le drain emit_bug→sink est cassé depuis un refacto amont, jamais détecté (tests jamais exécutés). | Cargo.toml racine (workspace.dependencies) ; `xtask/no-scaffold-allowlist.toml` | Allowlist purgée ✅ (story-662). Suppression des 2 crates = décision à valider (destructif + sync ARCHITECTURE.md). |
| H7 | **Code mort terrain** (~400 LOC) : 7 presets jamais retournés par `all_presets()`, section Load/Save JSON morte, stubs `pipeline_diag` no-op, 2 systèmes jamais enregistrés (`biome_registry_reload_system`, `setup_biome_materials`). | `map_gen_config.rs:207-779`, `pipeline_diag.rs:9-39`, `biome_registry.rs:241`, `biomes.rs:859` | Supprimer (ou brancher si voulu). Track FORGE, non bloquant ship. |
| H8 | **14 `#[allow(dead_code)]` + 1 `allow(unused_imports)`** — interdits par CLAUDE.md §3 ; dont `forgia-ui/lib.rs:375` annoté « à supprimer story-457 » jamais fait. | Liste §Dev Senior (lod.rs ×6, foliage, proc_walk, genome-core, rpg, skeleton-embedder, barks, ui [CHAUD], xtask [CHAUD]) | Purger au fil des fixes. |

### 🟡 MOYENS (14)

| # | Finding | Localisation |
|---|---|---|
| M1 | Hardcodes éco/progression : `SOULS_PER_WAVE 5`/`SOULS_PER_BOSS 25`, `RUN_BASE_XP 40` (**`roguelite_progression.toml` existe !**), `BASE_PLAYER_HP 100`, `REROLL_COST 30`, `WEAPON_MASTERY_DMG_PER_LEVEL 0.04` (dette déjà actée) | `run.rs:771`, `progress.rs:21`, `meta_shop.rs:37`, `coffre_forgeron.rs:30`, `weapon_select.rs:258` [CHAUD] |
| M2 | Hardcodes divers : stations heal/radius, loot_room, fireball IA (`roguelite_enemies.toml` existe), dash double-tap, merchant tailles, minimap 55 m, fog/ambient (`roguelite_render.toml` existe) | §Dev Senior top 20 |
| M3 | ~85 chemins GLB en dur hors registre : `decor.rs` ~40 (le genome `roguelite_decor.toml` existe mais ne porte pas les chemins), `worldgen_village.rs` ~45 **doublonnant le résolveur `forgia-village-kit`** déjà existant | `decor.rs:59-127` [CHAUD], `worldgen_village.rs:69-166` |
| M4 | Incohérence racine genomes : `config/genomes/` vs `assets/genomes/` selon les crates | `forgia-rpg/lib.rs:314-317`, `observability/config.rs:304` |
| M5 | `RwLock.expect("gait lock poisoned")` ×3 en hot-path anim → un panic isolé = crash permanent de l'anim ensuite | `gait_genome.rs:124-135` → `unwrap_or_else(\|e\| e.into_inner())` |
| M6 | `boss_xz_opt.unwrap()` fragile (invariant non typé, 2 Options dérivées de la même source) | `forgia-stage/src/layout.rs:162` |
| M7 | Écritures sensors JSON non atomiques (`fs::write` direct) sur 6+ sites vs pattern persist.rs — auto-guéri à 1 Hz, debug-only | `forgia-stage/lib.rs:458`, `forgia-anchor/lib.rs:294`, exporter, vram, sensor_health |
| M8 | **UserSettings** (pause_menu) écrits en `fs::write` **non atomique** — le seul « save » utilisateur sans tmp+rename | `forgia-ui-lib/src/pause_menu.rs:686` |
| M9 | ~25 sensor writers en Update sans `run_if` d'état (throttle interne 1 Hz OK, mais fetch payé hors mode) ; `forgia-effects` : **0 run_if dans toute la crate** | §Perf top offenders |
| M10 | Doublons gameplay : popup 3D flottant ×2 (`kill_popup.rs` vs `fps/score.rs`), AoE radial ×2 (`shockwave.rs` vs `boucherie_rocket.rs`), lumen cap 8000 copié ×3, `load_recipe` ×3 | §Dev Senior |
| M11 | Tuples add_systems à 13-16 éléments (reco interne : blocs de 10) + 3 fns à 12 params pile (limite) | `forgia-rpg/lib.rs:201,224`, `mode-roguelite/lib.rs:425` [CHAUD], `dialogue.rs:119` |
| M12 | `forgia2_render.json` severity=**critical** mais incohérent avec perf_diag au même timestamp (cameras_3d=0 → capture probable au Menu) — **à re-vérifier live avant de traiter comme bug rendu** | sensor du 2026-07-03 |
| M13 | Freezes confirmés par `forgia2_perf_diag` : 8 freezes, frame_max 109,8 ms, corrélés scène statique 13k+ entités (confirme l'audit du 2026-07-01 — PAS les bots ni les VFX) | Déjà en roadmap (Later/perf arène) |
| M14 | 13 TODO « story-471..479 refactor abandonné » (dont `sys_unstick_bots` supprimé à re-implémenter) + 16 TODO divers | `hud.rs`, `run.rs`, `lib.rs:606` [CHAUD] |

### 🔵 BAS (non comptés au score)

`panic!` documentés du qa-harness (voulus) ; `DoomProjectile` résidu V1 ; API publiques 0 usage (`with_cooldown`, `by_category`, `next_f64`…) ; crash wgpu `SurfaceAcquireSemaphores` au **teardown** (shutdown, lié au dossier outline connu) ; `format!` par spawn d'ennemi dans `waves.rs` (par vague, pas par frame).

---

## ❌ Faux positifs écartés (vérifiés manuellement — ne pas re-signaler)

| Signalé par | Claim | Réalité |
|---|---|---|
| Agent perf | `(track_left_mouse_state, drain_left_click_edge)` enregistré 4× dans Update (`forgia-fps/lib.rs:1382-1551`) | Les 4 occurrences sont des `#[test]` avec `App::new()` isolés. Le vrai câblage : 1× `RunFixedMainLoop` + 1× `FixedUpdate`, gatés par état. **Propre.** |
| Agent perf | `sys_update_stats` enregistré 2× (lenoir/bourrasque) | 2e occurrence = test. 1 seule registration réelle, gated `GameSet::Combat`. |
| Agent perf | `sys_trigger_combat_barks` enregistré 2× | Idem, tests. 1 registration réelle chaînée dans `GameSet::Effects`. |
| Agent QA (non conclu) | B0001 sur `color_grading.rs:231` | Vérifié : 2 queries `Entity` read-only + mutation via `Commands` — pas de `Query<&mut T>` séparée. **OK.** |

---

## 1. Architecture & couplage (Architect)

### Métriques
- **319 fichiers .rs · 111 710 LOC · 62 crates** (+ bin racine `forgia` + xtask = 64 membres)
- Top crates : forgia-mode-roguelite **25 210** · forgia-terrain 9 400 · forgia-observability 5 664 · forgia-ui-lib 5 493 · forgia-stage 4 450 · forgia-rpg 4 233

### God files > 1200 LOC (Critique — 10)

| Fichier | LOC | Types pub | Note |
|---|---|---|---|
| forgia-rpg/src/lib.rs | 2513 | 10 | Track FORGE — 4 god-fns dedans |
| forgia-mode-roguelite/src/elements.rs | 2067 | **69** | Fourre-tout réactions/status → 4 modules |
| xtask/src/main.rs | 2016 | 0 | 15+ sous-commandes → `cmd/*.rs` [CHAUD] |
| forgia-anim-locomotion/src/locomotion.rs | 1897 | 37 | 3 god-fns (343/317/223 LOC) |
| forgia-mode-roguelite/src/hud.rs | 1784 | 6 | Monolithe de systèmes privés |
| forgia-stage/src/lib.rs | 1695 | 28 | `spawn_stage_arena_on_request` 533 LOC |
| forgia-fps/src/lib.rs | 1564 | 23 | `fire_weapon_minimal` **491 LOC** (le nom ment) |
| forgia-mode-roguelite/src/decor.rs | 1479 | 19 | `plan_decor_set` 242 LOC → table-driven |
| forgia-skeleton-template/src/lib.rs | 1390 | 31 | Data vs runtime à séparer |
| forgia-stage/src/layout.rs | 1382 | 14 | Parsing vs résolution |

+ 20 fichiers 800-1200 (Haute) et 33 fichiers 500-800 (Moyenne) — liste complète conservée dans les données d'agents. Plus grosse fn du workspace : `generate_chunk_lod` 654 LOC/12 params (`chunk_sdf.rs:51`).

### Couplage (extraits)

| Crate | IN | OUT | Note |
|---|---|---|---|
| forgia-core | 42 | 0 | Hub foundation sain |
| forgia-genome-core | 15 | 1 | Hub data sain — mais ignoré par les 21 fichiers à boilerplate genome (H3) |
| forgia-ui | 4 | 8 | ⚠️ **dep forgia-mode-roguelite = inversion de couche majeure** (H1) |
| forgia-ui-lib | 5 | 8 | ⚠️ dep forgia-mode-fps-arena |
| forgia-rpg | 2 | 25 | God-orchestrator (FORGE, toléré) |
| Cycles | — | — | **Aucun** |

---

## 2. Code mort, doublons & hardcoding (Dev Senior)

Compteurs : `#[allow(dead_code)]` **14** · TODO/FIXME **73** (44 port-V1, 13 refactor abandonné, 16 divers) · `unwrap()` 189 brut (17 en prod).

Détail complet des tables (code mort top 20 avec grep counts, doublons top 10, hardcodes top 20, chemins assets top 15, TODO tracker) : voir findings consolidés C2/H3/H4/H6/H7/M1-M4/M10 ci-dessus. Notes de fiabilité de l'agent : heuristique mot-entier (usage via macro/re-export échapperait au comptage) ; les 2 capteurs jamais enregistrés sont à **brancher**, pas à supprimer.

---

## 3. Sécurité & stabilité (QA)

Compteurs prod : unwrap **17** · expect **5** · panic! **3** (tous qa-harness, documentés) · **unsafe 0**.

### BUG REPORT
- 🟠 **Majeur** : C1 boon chaîne inerte (seul bug runtime — confirmé manuellement, voir §Findings).
- 🟡 Mineur : M5 (RwLock poison anim), M6 (unwrap layout fragile).
- 🔵 Cosmétique : M7 (sensors non atomiques), panics qa-harness (voulus).
- **Non-findings vérifiés sûrs** : foot_ik, locomotion, material_override, skeleton-embedder, soak — unwraps tous guardés. `persist.rs` : RAS, atomique by design.

### Invariants V2

| Invariant | Statut |
|---|---|
| L7 GameSet chain | ✅ (`forgia-core/lib.rs:206-235`, Update + FixedUpdate) |
| B0001 Added séparé de &mut | ✅ (1 seul site candidat, vérifié propre) |
| 1 handler ESC | ✅ (`forgia-ui/lib.rs:436` unique) |
| Time Real vs Virtual | ⚠️ Non concluant (5 fichiers lisent Real, tous plausibles : UI/menu/hitstop/diag — pas de balayage inverse exhaustif) |
| MenuCamera2d jamais sur FpsCamera | ✅ (indicatif) |
| Hanabi pre-spawn Startup | ⚠️ Non vérifié (à cocher au prochain passage dans forgia-effects) |
| 2 types Health | ✅ partout **sauf C1** — tous les autres sites (elements, poi, shockwave, ultimate_apply, waves, boucherie_rocket, enemy_scaling, mode-fps-arena) respectent le camp avec commentaires de garde |

---

## 4. Performance (Perf)

### Sensors (⚠️ tous datés 2026-07-03 — relance runtime nécessaire avant toute conclusion « actuel »)
- `forgia2_perf` : fps 241, frame 4,17 ms, **bound_hint=cpu_bound** (cohérent audit 07-01)
- `forgia2_perf_diag` : **8 freezes**, max 109,8 ms, 13 217-13 673 entités, enemies=2-8, particles=0-13 → confirme scène statique = le coupable (M13)
- `forgia2_sensor_health` : warn — `forgia2_rpg_health` stale (H5)
- `forgia2_toon` : outline attached_cameras=0 (connu, outline OFF)
- `forgia2_render` : critical mais capture probable au Menu (M12 — revérifier live)
- `forgia2_crash.previous` : panic wgpu au teardown (shutdown, pas gameplay)

### run_if
- forgia-mode-roguelite : 16/~140 blocs Update sans gate (quasi tous des sensor writers throttlés 1 Hz en interne) ; forgia-effects : **0 run_if dans la crate** ; détail M9.
- Pattern à préférer : `run_if(in_state(GameMode::Roguelite))` plutôt que gate manuel en 1re ligne de body (fetch payé quand même — ex. `energy.rs:189`).

### Couverture partielle (repasse dédiée à planifier)
Allocations par-frame / queries larges / clones lourds : listes candidates relevées mais non triées ligne-à-ligne. Priorités de la repasse : `boucherie_rocket.rs` (raycast raymarch par-frame pendant le vol des roquettes ×N roquettes actives), `boss_portal.rs:221` (garde idempotente du ground-snap à confirmer), queries multi-lignes des gros systèmes (`waves.rs`, `run.rs`, `decor.rs`, `forgia-fps/lib.rs`).

---

## 5. Recommandations — Top 10 priorisées

1. **[PROCESS/P0]** Vider l'arbre chaud : committer/merger le WIP de l'autre terminal (16 j non commité = risque de perte + bloque tout le reste). Puis committer stories 656-659.
2. **[CRITIQUE]** C1 — fixer le boon chaîne (`forgia_combat::Health` + `CombatHitEvent`, ~10 lignes + test). Dès merge.
3. **[CRITIQUE]** C2 — dé-hardcoder shockwave/boucherie_rocket/ultimate_tech vers genome TOML (`roguelite_ultimate.toml` existe déjà). Data-only, zéro risque.
4. **[PROCESS]** Résorber wip-check 44→≤3 par reclassement des statuts (la plupart sont REVIEW/DONE de fait) via `xtask story-index`.
5. **[HAUT]** H3 — `GenomeFile<T>` dans forgia-genome-core (~1000 LOC dédupliquées + warn sur TOML corrompu). 1 story Standard.
6. **[HAUT]** H5 — brancher les 2 capteurs morts + fixer rpg_health stale + implémenter `check-orphans` réel (c'est le gate qui aurait attrapé H5).
7. **[HAUT]** H6+H7+H8 — purge express : 2 crates orphelines, presets terrain morts, 14 allow(dead_code), allowlist périmée. 1 session de nettoyage.
8. **[HAUT]** H1 — casser l'inversion forgia-ui → forgia-mode-roguelite (contrat events/resources).
9. **[HAUT]** H4 — trancher les squelettes port-V1 (porter les kill/damage sounds d'arena_feedback — utile au chantier « Voix/SFX armes » du NEXT — supprimer le reste).
10. **[POST-SHIP]** C3 — splits god-files (commencer par `mode-roguelite/lib.rs` build 601 → sub-plugins, `elements.rs` 69 pubs). **Pas avant le ship P0.**

### Séquencement recommandé (3 vagues)

- **Vague 0 — Débloquer** (1 session) : reco 1 + 4 + Ship P0 de la roadmap (binaire dist + vérif victoire). Rien d'autre n'est propre tant que l'arbre traîne.
- **Vague 1 — Corriger** (1-2 sessions) : recos 2, 3, 6, 7 — que du Quick/Standard, aucun risque de régression, gros gain de fiabilité.
- **Vague 2 — Assainir** (post-ship P0) : recos 5, 8, 9, 10 + repasse perf dédiée (§4).

**Verdict** : le checkup était le bon réflexe au bon moment — mais la réponse n'est PAS une refonte big-bang avant le ship. 0 bloquant, invariants tenus : on corrige la Vague 1, on ship, on assainit ensuite.

---

## 6. Métriques codebase

- Fichiers .rs : **319** · LOC : **111 710** · Crates : **62**
- Fichiers >1200 LOC : **10** · 800-1200 : 20 · 500-800 : 33
- unwrap() : 189 (17 prod) · panic! prod : 3 (voulus) · unsafe : **0**
- TODO/FIXME : **73** (44 port-V1 + 13 refactor abandonné + 16 divers)
- `#[allow(dead_code)]` : **14** (interdits)
- Stories IN_PROGRESS : **44** (limite : 3)
- Sensors canoniques : 14/14 format OK ; 2 capteurs codés jamais branchés

---

## 7. Session runtime fraîche — 2026-07-19 19:45-19:51 (complément live)

> 1 run Roguelite (défaite salle 3 vague 1, 275 s), capteurs frais dépouillés. ⚠️ **Binaire = exe debug du 03/07 non rebuildé** (mtime vérifié) — la session reflète le code d'il y a 16 jours ; valide pour les questions ci-dessous (code antérieur au 03/07), mais toute prochaine validation runtime doit suivre un rebuild.

| Question ouverte | Verdict runtime |
|---|---|
| **Fond noir lobby** | 🎯 **ROOT CAUSE CONFIRMÉE** : 0 `Camera3d` active en Menu/Lobby — seule `MenuCamera2d` (2D, clear=None) existe ; skybox et toon logguent « re-attached to **0** Camera3d(s) » au boot ; `forgia2_render.json` severity=critical `cameras_3d_active=0, mesh3d_visible=1/37`. L'arène de fond ne PEUT pas rendre : il n'y a pas de caméra pour elle. Fix = spawner une Camera3d lobby (ou garder la FpsCamera en Lobby) — à coordonner avec le chantier hub. |
| **`forgia2_render` critical (M12)** | Résolu : le critical en Menu est *réel* (rien ne rend en 3D) — c'est la signature du bug lobby ci-dessus, pas un artefact de capture. En run le rendu est sain. |
| **Capteur `victory:true` sur une défaite** | ⚠️ **Faux-ami de nommage, pas un bug d'état** : `victory_emitted` est un latch de fin de run posé AUSSI sur la défaite (`run.rs:271` « bloque transitions further ») et exporté tel quel par `sensor.rs:118`. Meta dit correctement « victoires 0 ». **Fix Quick [CHAUD]** : renommer le champ exporté (`run_ended`) + exporter la vraie victoire. Ce faux-ami a déjà coûté un diagnostic (cette session). |
| **Victoire end-to-end** | ❌ **Toujours non testée** (mort avant le boss). Chemin réel : boss tué → `boss_defeated` → porte du socle → parcours → `RunResult::Victory` émis par `loot_room.rs:822` (story-571). À retester en finissant une run. |
| **Multi-salles / portes (R2)** | ✅ Re-confirmé : 3 salles traversées, porte reposée à chaque salle (raycast dais OK ×3). Note : « `sys_start_run` fallback » emprunté au 1er spawn — chemin de secours actif, à garder en tête pour le refacto FSM (story-646). |
| **Freezes** | 8 freezes 48-102 ms, **tous aux transitions** (spawn vague/salle, `cause=gpu_or_shader_compile`), zéro en combat pur ; `bound_hint=headroom` au menu (239 fps, 4.2 ms). Piste : pré-chauffe shaders/assets au premier spawn (généraliser l'anti-trap Hanabi pre-spawn). |
| **Sensor health** | 12/12 présents, 1 stale = `forgia2_rpg_health` (H5 re-confirmé live). |
| **Stabilité** | 0 ERROR / 0 panic sur 1 170 lignes de log ; sortie propre, pas de crash teardown cette fois. |

**Recalage éco (nourrit l'audit balance)** : Or réellement collecté = **422 sur 3 salles ≈ 140/salle**, ~2× le modèle de l'audit éco (67/salle) → recaler la table de revenu avant tout tuning. Souls 30 ≈ conforme au modèle. `metal_chaud` pris au 1er coffre (le dominant, comme prédit). Log « spent 20 **souls** » pour de l'Or = le piège de nommage vivant. Curiosité à confirmer : meta save fraîche (run #1, 30 âmes) vs progress niveau 9 — saves désynchronisées (reset partiel %APPDATA% ?).

---

*Audit exécuté par 4 agents parallèles + gates xtask + contre-vérifications manuelles (C1, B0001, faux positifs perf) + session runtime 2026-07-19. Findings [CHAUD] à re-vérifier après merge de l'autre terminal. Prochain audit : post-Vague 2 ou pré-launch Steam.*
