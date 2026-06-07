# Forgia Rewrite — Session State

> ⚠️ Working tree = **2 streams non commités mélangés** (46 fichiers sur `e940192`) :
> stream RIG/ANIM (ci-dessous 2026-06-05) + stream ROGUELITE/PROCGEN (2026-06-04 plus bas).
> Commit à SCOPER par stream. Cf MEMORY.md (sessions 2026-06-05).

---

# 2026-06-07 — STREAM WORLDGEN : forgia-worldgen P0+P1 (STOP, NON COMMITÉ)

> Détail complet : memory `project_session_2026_06_07_worldgen_p0_p1` + `reference_worldgen_crate_architecture`.
> Story `docs/stories/story-578-worldgen-procgen.md` (P0+P1 DONE).

## Livré (compile : `cargo check -p forgia` OK, 10 tests, clippy 0)
- **Crate `forgia-worldgen` créé** (registry RON + points + spawn budgété + sensor + debug viz).
- P0 : `assets/registry/asset_meta.ron` (107 modules ithappy, grounding pivot `ground_offset`).
- P1 : `ForgiaWorldgenPlugin` (demo **F7** = rangée modules grounded devant caméra, **F8** gizmos AABB).
  Découplé : `GroundSampler` injecté (0 dép forgia-terrain). Sensor `forgia2_worldgen.json`.

## Fichiers worldgen (à committer SCOPÉS)
crates/forgia-worldgen/** · assets/registry/asset_meta.ron · assets/models/environment/platformer/one_file_assets.glb ·
Cargo.toml (3 lignes : member + 2 workspace deps) · crates/forgia-game/{Cargo.toml,src/lib.rs} (2 lignes) ·
docs/stories/story-578-worldgen-procgen.md

## ▶️ REPRISE
1. **Jeu FERMÉ** → `cargo build -p forgia --profile release-fast -j 2` (relink bloqué cette session = exe locké).
2. Roguelite → **F7** = ~8 modules variés flush au sol (preuve grounding). Sensor `forgia2_worldgen.json` registry_modules:107.
3. Si OK → **commit scopé worldgen** (NE PAS embarquer rig/anim/foliage de l'autre terminal).
4. **P2** = recette TOML hot-reload (layout grille → hameau data-driven).

---

# 2026-06-05 — STREAM RIG : Rex A-pose + locomotion polish (STOP, reprise demain)

> Détail complet : memory `project_session_2026_06_05_rex_apose_locomotion`. **Rien commité.**

## Livré (staged, compile, tests verts — voir `cargo test -p forgia-anim-locomotion`=18, `-p forgia-auto-rig`=28)
- Épaules larges (template `skeleton_biped_lizard.toml` + builder `forgia-skeleton-template` synchro 1e-6).
- A-pose : `[stance_offsets]` clavicule 40° + bras 25° (diagonale continue).
- Skinning head-fix (`forgia-auto-rig/skinning.rs`) : verts région-tête excluent os de bras → clavicule penchée sans déformer écailles. **Re-rig requis.**
- Float : `forgia-rpg/character.rs` `ground_hug` → `clamp(-0.5, 0.4)` bidirectionnel.
- Respiration : bob corps idle→0 (torse breath spine gardé).
- Roulis : `ROLL_WADDLE_AMP` 0.06→0.03, `PELVIC_ROLL_AMP` 0.12→0.08.
- **Swing-axis fix (DERNIER, NON validé runtime)** : `compose_inherited_stance_swing` (axe = (clav×arm)⁻¹·flex) — les mains twistaient car stance clavicule hérité non compensé. Workflow-dérivé + test falsifiable `clavicle_inherited_stance_swing_world_axis_is_lateral`.

## Fichiers rig (à committer SCOPÉS, pas les ~38 autres)
skeleton_biped_lizard.toml · forgia-anim-locomotion/{locomotion,proc_walk}.rs · forgia-auto-rig/{skinning,debug_gizmos}.rs · forgia-rpg/character.rs · forgia-skeleton-template/lib.rs · forgia-skeleton-embedder/lib.rs

## ▶️ REPRISE
1. Jeu FERMÉ → `cargo build -p forgia` (PAS forgia-game).
2. Entrer RPG (re-rig pour skinning) + **MARCHER** de profil → mains basculent avant-arrière (sensor `forgia_anim_full.json` hand world_dir oscille Z, X ~constant ; marker `forgia_rig_bones.json`=`APOSE-SKINFIX_2026-06-05`).
3. Si OK → auto-QA (verifier + qa-lead) → **commit scopé rig**.
4. Tuning hot-reload restant (stance TOML) : A-pose + ouverte arm 25→15 ; épaules ↓ clavicule 40→30.

---

# 2026-06-04 — STREAM ROGUELITE : pivot vision + ship-audit + story-566 (STOP)

> Snapshot pour reprise. **Rien commité cette session** (working tree déjà sale autre terminal sur anim/rig).

## 🎯 CE QUI S'EST PASSÉ — pivot vision + ship-audit Roguelite

1. **Audit général crates** : workspace SAIN — `cargo check`/`clippy --workspace` = 0 erreur / 0 warning (123 crates), runtime 141 FPS, gates xtask verts.
2. **Deep-dive 28 sensors "missing"** : 25 faux positifs (scanner xtask aveugle aux `const PATH:&str`) + 3 vrais (pack_registry runtime dormant zéro-consommateur, forgia_textures legacy V1).
3. **PIVOT VISION (majeur)** : Forgia = moteur IA-natif (créateur importe assets, l'IA construit), PLUS de funnel publish/monétise. Priorité = **SHIP le Roguelite** (FPS roguelite type Gunfire Reborn). RPG = track FORGE (outils anim/rig refluent). → `docs/vision/FORGIA_VISION_2026-06-04.md` (copy site prête).
4. **CLAUDE.md §1 réécrits** (×2 : `d:\Forgia\CLAUDE.md` + V2) selon le pivot. Autorisé par user (Lock §6).
5. **Ship-audit Roguelite** (sub-agent game-maker) → `docs/audit/roguelite-ship-readiness-2026-06-04.md`. Verdict : **~40% MVG, non shippable**. 4 piliers manquants = armes distinctes, boons perceptibles, boons atteignables, méta+persistance. Le gap = CÂBLER ce qui est à moitié construit (pas ajouter du contenu).

## 🔴 TÂCHE EN COURS — story-566 (recalibrage éco) BLOQUÉE sur décision archi

J'ai commencé story-566 (le quick win du chemin critique). **Concept-first a attrapé un piège AVANT tout Edit** :
- L'éco roguelite = **constantes hardcodées** dans `forgia-mode-roguelite/run.rs` (SOULS_PER_WAVE=5:722, SOULS_PER_BOSS=25:725, SOUL_WISP_VALUE=2:495).
- **Seul chemin de lecture genome roguelite** = `RogueliteRunConfig` parsé dans **`forgia-stage/graph.rs:225`** — et **roguelite ne le lit PAS**. forgia-stage = crate contendue (Cargo.toml:36 "édité autre terminal", lib.rs modifié non-commité).
- Donc AC2 "externaliser en gènes" ≠ effort S → **effort M, cross-crate, touche crate sensible**.
- AC4 "souls_cost par boon déjà supporté" = **introuvable au grep** (hypothèse draft fausse, à revérifier).

**DÉCISION ARCHI EN ATTENTE (user doit trancher demain)** :
- **Approche A** : étendre `RogueliteRunConfig` (forgia-stage/graph.rs) — touche crate sensible.
- **Approche B (RECO)** : nouvelle Resource éco-config LOCALE à forgia-mode-roguelite — zéro edit forgia-stage.

**2 options proposées au user** :
- (1) Trancher B + story-566 complète (genes + read-path local + sensor + QA sub-agents).
- (2) Quick-fixes locaux sûrs d'abord : AC6 hearts double-dip (run.rs:355-375, cœur soigne ET donne souls), AC1 3e coffre (waves.rs).

## ▶️ REPRISE DEMAIN
1. User tranche A/B + option (1) ou (2) pour story-566.
2. Chemin critique Phase 0 (ship Roguelite) : **559(B) impact tir → 566 éco → 564 gimmicks armes → 565 boons perceptibles → 569 méta+persistance** + onboarding + honnêteté UI. ~25-30j solo.
3. `forgia-mode-roguelite` working tree sale (Cargo.toml + lib.rs M, audio.rs untracked = slice-A audio prior session) → check `git status` + risque orphan-file audio.rs avant tout add.

## ⚠️ Autre terminal actif (standup) : anim/rig/foliage/rpg/skeleton (marathon rig Rex). Éviter ces crates.
