# Forgia — Session Log

> Journal chronologique append-only. Chaque entrée = une action significative.
> Format: `## [YYYY-MM-DD] type | description`
> Types: `session`, `fix`, `feat`, `ingest`, `audit`, `decision`, `research`
> Grepable: `grep "^## \[" docs/log.md | tail -20`

---

## [2026-04-09] session | Weapon VFX pipeline AAA (story-303 DONE)
5 phases: mesh→hanabi migration, muzzle flash multi-layer, impact decals, shell casings, tracer ribbons.

## [2026-04-09] session | MemPalace v3 integration (story-302/304/305 DONE)
KG 860 entities, split resources, GameSet mapping 98%, hooks auto-save.

## [2026-04-09] fix | Terrain infinite loop carve_sphere (worm outside chunk bounds)

## [2026-04-09] fix | Loading screen deadlock (wait for spawn chunk mesh)

## [2026-04-09] ingest | Karpathy LLM Wiki Pattern
Pattern 3 couches (raw/wiki/schema). Adopté: log.md + docs/raw/. Forgia déjà en avance sur le wiki layer (MemPalace KG > markdown plat).

## [2026-04-09] ingest | ccunpacked.dev — Claude Code internals
Features cachées: --resume, /effort, /ctx_viz, Coordinator Mode (feature-flagged). On utilise déjà 90% des patterns avancés.

## [2026-04-09] ingest | career-ops (santifer) — AI job search pipeline
12 patterns architecturaux analysés. 3 adoptés: verify-stories (tracker integrity), analyze-patterns (retrospective), batch-workers (parallel orchestration).

## [2026-04-09] feat | 3 nouvelles skills Claude Code
- `/verify-stories` — Lint pipeline stories (orphelins, fantômes, doublons, metadata, cohérence statuts)
- `/analyze-patterns` — Rétrospective structurée (hotspots, bugs récurrents, tendances, recommandations)
- `/batch-workers` — Orchestration N agents parallèles avec state file + merge + synthèse

## [2026-04-09] fix | /verify-stories premier run — 23 problèmes détectés, 6 critiques corrigés
- 6 doublons ID résolus: story-302/303/304/015/015b/109 → renommés story-306 à 311
- 1 fichier doublon supprimé (story-109-ia-assistant-creator.md)
- 16 orphelins classés dans _index.md (Cycle 20 + section orphelins)
- next_id BMAD: 302 → 312
- Reste: ~45 priorités en format court (P0 vs P0-critical) — convention de fait acceptée

## [2026-04-09] fix | 4 désync stories ↔ code corrigées (audit croisé AAA)
- story-175 (Boss Fights): TODO → DONE (3 phases, enrage, loot — code complet depuis 03/31)
- story-250 (Weather): TODO → IN_PROGRESS (rain/snow hanabi 75% implémenté)
- story-177 (Quests): TODO → IN_PROGRESS (TOML data-driven, 4 types objectifs, 75%)
- story-176 (Marchands): TODO → IN_PROGRESS (7 rôles, shops fonctionnels, 70%)
- 6 features non trackées identifiées: volumetric fog, water shader, decals, shadow cascades, death/respawn, persistence

## [2026-04-09] fix | 3 fixes P1 deep bugs
- castle.rs: 4 gardes `.max(1.0)` contre division par zéro (L315, 379, 826, 1174)
- water.rs Vec::new(): faux positif — run once guard, pas per-frame. Pas de fix nécessaire.
- Orphans supprimés: nightmare_fredbear.glb (102 MB) + Kelotor_specgloss_backup.glb (15 MB) = 117 MB récupérés
- pine_forest (3 GB) conservé: référencé dans vegetation_config.json
- cargo check: 0 errors, 0 warnings

## [2026-04-09] fix | Grass flottante + saut trop bas + sol vide
- **Grass y_offset**: grass mesh builder ignorait TerrainConfig.y_offset (-10.0) → herbe 10m au-dessus du sol. Fix: propager y_offset dans GrassChunkSnapshot → build_grass_mesh_from_sdf
- **Jump impulse**: 6.5 → 14.0 (hauteur 0.66m → 3.5m), gravity 32 → 28. Genome character_default.toml mis à jour.
- **Entity budget**: High 1200 → 3000, Ultra 3000 → 5000. Sol vide causé par cap trop bas pour map 4096m.
- **Kill slowmo**: désactivé (feedback utilisateur, casse le flow)
- Erreurs cargo: 7 erreurs dans hover_vfx.rs (autre terminal), 0 erreur dans mes fichiers

## [2026-04-09] feat | Merge 14 fichiers Desktop → Main (4229 lignes, commentés)
Fichiers copiés depuis C:\Desktop\Forgia, modules déclarés mais commentés (API incompatible):
- ai_assistant/ (429L) — NL parser FR+EN → ForgiaAction → executor
- combat/gcd.rs (108L) — GCD 1.5s WoW + spell queue 400ms
- player/equipment.rs (443L) — Attachement os (casque, cape, bouclier, ailes)
- ui/spell_bar.rs (249L) — HUD WoW cooldown sweep radial
- debug/startup_checks.rs (334L) — Validation boot + auto-fix NaN
- terrain/chunk_inspector.rs (394L) — F12 overlay debug chunks
- terrain/rock_scatter.rs (420L) — Rochers procéduraux pentes
- terrain/volumetric_fog.rs (207L) — Fog height-based vallées/pics
- lua_commands.rs (541L) — Game-level Lua sync + commands
- ui/lua_script_ui.rs (278L) — Lua-created UI elements
- ui/script_editor.rs (261L) — Éditeur Lua Ctrl+L
Adaptation requise: find_bone pub, GameAssets champs, forgia_engine::scripting API, LodChunkManager→ChunkManager

## [2026-04-09] feat | 9/14 modules Desktop actives (3664 lignes)
- Batch 1: gcd.rs, startup_checks.rs, ai_assistant/ (4 fichiers) — compile direct
- Batch 2: chunk_inspector.rs (LodChunkManager→ChunkManager, entities→loaded_entities), rock_scatter.rs, volumetric_fog.rs — 3 renames
- Batch 3: spell_bar.rs — compile direct (deps FireCooldown/IceCooldown/ShieldCooldown existent)
- Batch 4: equipment.rs — find_bone→pub, assets placeholder (sword/shield/wings→None+warn), syntax fix
- 3 champs FpsTuning ajoutes: gcd_base(1.5), gcd_min(0.75), spell_queue_window_ms(400)
- Restent commentes: lua_commands.rs, lua_script_ui.rs, script_editor.rs (scripting API changee)
- cargo check: 0 errors, 0 warnings

## [2026-04-09] session | Memorise — session 8h cloturee
3 skills, story cleanup (6 doublons + 4 desync + 16 orphelins), grass y_offset, rover PI, texture array attempt (rollback), 9 modules Desktop merges. Session la plus productive.

## [2026-04-09] feat | story-309 Terrain Texture Array — Phase 1-2 implementees
- TerrainMat flipped: TriplanarTerrainMaterial → TerrainArrayMaterial (3 bindings vs 31)
- init_terrain_array_material: charge 10 diff.jpg biomes (toutes 1024x1024)
- finalize_terrain_texture_array: assemble Image 2D array runtime (10 layers, Rgba8UnormSrgb)
- terrain_array.wgsl: triplanar sampling depuis texture_2d_array, slope rock blend, PBR
- sync_terrain_material_params branche (moss/wetness/snow hot-reload)
- MaterialPlugin swap dans terrain/mod.rs
- cargo check: 0 errors, 0 warnings
- A VALIDER EN JEU: terrain texture + FPS

## [2026-04-09] audit | story-308 — toutes les phases déjà faites
- Phase 1 (triplanar): déplacé story-309 (intentionnel, 7 FPS)
- Phase 2 (biome sky): DONE — refactoré compute_sky_state + apply_sky_state, 10Hz, biome blend lerp
- Phase 3 (combat juice): DONE — re-enabled 04-07, 6 systèmes actifs (trauma/hitstop/slowmo/flash/weapon)
- Phase 4 (UI dashboard): DONE — enabled:false par défaut
- Phase 5 (grass): DONE — vertex shader revert
- Phase 6 (clouds): stub intentionnel (alpha overdraw 4km), backlog P2
- Story-308 → DONE
