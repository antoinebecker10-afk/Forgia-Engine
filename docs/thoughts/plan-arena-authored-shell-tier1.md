# Plan: Arène coquille authored (Tier 1) — modèle Returnal hybride

Date: 2026-06-26
Story: story-625
Audit source: `docs/audit/audit-2026-06-26-arena-render-quality-vs-parcours.md`

## Objectif

Remplacer le scatter procédural **structurel** de l'arène par une **composition authored data-driven** (un genome de layout que l'IA écrit, instancié depuis les GLB atomiques Inferno/KayKit existants), **sans casser** le pipeline procédural — qui reste fallback (stage sans layout authored) ou overlay fin (loot/spawns). Preuve : ≥ 1 section de `crypts_of_anvil` (fosse à mêlée + perchoir) recréée 100 % depuis la data, visible en jeu, walkable, anchors posés, **zéro coordonnée en dur dans le Rust**.

## Décisions d'architecture (clés)

1. **Pas de nouvelle crate.** Le layout authored a **1 seul consommateur** (`forgia-stage::spawn_stage_arena_on_request`) → règle `fine-grained-crates` = module local, pas crate. → nouveau module `crates/forgia-stage/src/authored.rs`.
2. **Pas de découpe GLB, pas de Blender.** Les pièces atomiques sont déjà des GLB individuels (`models/environment/inferno/*.glb` + `models/kaykit/dungeon/*.glb`), instanciées par `forgia-prefab::spawn_gltf_prefab` **existant**.
3. **Genome chargé via AssetServer + `Genome<T>`** (comme `roguelite_stages.toml`/`roguelite_pois.toml`, `lib.rs:312-315`) → hot-reload natif Bevy file_watcher, cohérent avec la crate.
4. **Cohabitation procédural** : si un layout authored existe pour le `stage_id` ET `suppress_procedural_modules=true` → on **skip `place_modules`** (la structure est authored) ; sinon comportement actuel **intact** (zéro régression). Floor/ramparts/lighting/POIs restent (le floor collider couvre le sol des pièces posées).
5. **Anchors from data** : une pièce avec `role` gameplay pose un `AnchorPoint` (réutilise `forgia_level_presets::parse_anchor_kind`). **Synergie boss-portal** : une pièce `role="melee_pit"` doit (a) poser `AnchorKind::MeleePit` ET (b) être nommée `Module_melee_pit_authored` — car `boss_portal::sys_reconcile_boss_gate` lit l'anchor MeleePit pour la position ET `find_dais_root` cherche le nom `Module_melee_pit*` pour solidifier le dais ([boss_portal.rs:107,397](crates/forgia-mode-roguelite/src/boss_portal.rs#L107)). Sinon la porte du boss casse.

## Schéma genome — `assets/genomes/arena_layouts.toml`

```toml
# Authored arena layouts — Tier 1 (story-625). Hot-reload Shift+F12.
# Chaque [layouts.<stage_id>] compose une arène depuis des GLB atomiques,
# placés à la main (data-driven, 0 hardcode Rust). stage_id matche
# roguelite_stages.toml. Cible bible : docs/lore/locations/crypts_of_anvil.md.

[layouts.crypts_of_anvil]
# true = la structure est authored → on coupe le scatter procédural de modules
# (le procédural reste pour POIs/loot = overlay run-to-run).
suppress_procedural_modules = true

# ── Fosse à mêlée (centrale) ─────────────────────────────────────────────────
[[layouts.crypts_of_anvil.pieces]]
prefab  = "models/environment/inferno/CirclePlatformSmall_001.glb"
pos     = [0.0, 0.0, -16.0]
rot_deg = 0.0
scale   = 2.0
role    = "melee_pit"          # → AnchorKind::MeleePit + nom Module_melee_pit_authored
walkable = true                # → collider TriMesh (on marche dessus)
section = "fosse_melee"

[[layouts.crypts_of_anvil.pieces]]
prefab  = "models/environment/inferno/Brazier_002.glb"
pos     = [5.0, 0.0, -12.0]
role    = "decor"             # pas d'anchor, pas de collider (visuel)
section = "fosse_melee"

# ── Perchoir du contremaître (bord) ──────────────────────────────────────────
[[layouts.crypts_of_anvil.pieces]]
prefab  = "models/environment/inferno/TowerBig_001.glb"
pos     = [-46.0, 0.0, 28.0]
rot_deg = 90.0
role    = "sniper_perch"      # → AnchorKind::SniperPerch
walkable = true
section = "perchoir"
```

## Types Rust — `crates/forgia-stage/src/authored.rs` (Deserialize)

```rust
#[derive(Deserialize, Clone, Debug)]
pub struct ArenaPiece {
    pub prefab: String,
    pub pos: [f32; 3],
    #[serde(default)] pub rot_deg: f32,
    #[serde(default = "default_scale")] pub scale: f32,
    #[serde(default)] pub role: String,      // "decor"|"melee_pit"|"sniper_perch"|"boss_pad"|"player_spawn"|"poi_slot"|"cover_low"|"cover_high"
    #[serde(default)] pub walkable: bool,     // collider TriMesh
    #[serde(default)] pub blocker: bool,      // collider cuboïde
    #[serde(default)] pub section: String,    // organisationnel (sensor/debug)
}
#[derive(Deserialize, Clone, Debug, Default)]
pub struct ArenaLayout {
    #[serde(default)] pub suppress_procedural_modules: bool,
    #[serde(default)] pub pieces: Vec<ArenaPiece>,
}
#[derive(Deserialize, Default, Clone, TypePath)]
pub struct ArenaLayoutsGenome {
    #[serde(default)] pub layouts: HashMap<String, ArenaLayout>,
}
fn default_scale() -> f32 { 1.0 }

/// Pure : role → Option<AnchorKind> ("decor"/"" → None). Testable headless.
pub fn role_to_anchor(role: &str) -> Option<AnchorKind> { /* parse_anchor_kind + alias */ }
```

## Fichiers autorisés (SCOPE LOCK)

- `crates/forgia-stage/src/authored.rs` — **NEW** : types, role→anchor, spawn helper, collider walkable/blocker
- `crates/forgia-stage/src/lib.rs` — `pub mod authored;` + register `Genome<ArenaLayoutsGenome>` + handle dans `StageGenomeHandles` + appel spawn authored dans `spawn_stage_arena_on_request` + skip `place_modules` si suppress
- `crates/forgia-stage/src/layout_sensor.rs` — étendre `forgia2_stage_layout.json` : `layout_source` ("authored"|"procedural"), `authored_pieces`, `authored_sections`
- `assets/genomes/arena_layouts.toml` — **NEW** data : layout authored `crypts_of_anvil` (fosse + perchoir + décor)
- `docs/stories/story-625-arena-authored-shell.md` — **NEW** story
- `docs/stories/_index.md` — entrée story-625
- (Tout autre fichier = INTERDIT — notamment NE PAS toucher `loot_room.rs`/parcours, ni le procédural `place_modules` lui-même)

## Phases

### Phase 1 — Genome + loader + spawn décor (le plus petit qui prouve la valeur) (3 fichiers)
- [ ] `authored.rs` : types `ArenaPiece/ArenaLayout/ArenaLayoutsGenome` + `default_scale` + `role_to_anchor` (pure) + tests purs (parse TOML, role→anchor)
- [ ] `lib.rs` : `pub mod authored;` + `init_asset::<Genome<ArenaLayoutsGenome>>()` + `register_asset_loader` + champ `arena_layouts: Handle<...>` dans `StageGenomeHandles` + load au `load_stage_genomes`
- [ ] `lib.rs` `spawn_stage_arena_on_request` : après ramparts, lookup `layouts.get(stage_id)` ; si présent → spawn chaque pièce via `spawn_gltf_prefab` (tag `StageArenaMarker`, name `AuthoredPiece_<section>_<i>`) ; `props_spawned += n`
- [ ] `arena_layouts.toml` : `crypts_of_anvil` avec ~6 pièces décor + fosse + perchoir
- [ ] `rtk cargo check -p forgia-stage` + `rtk cargo test -p forgia-stage`

### Phase 2 — Anchors from data + suppression procédurale (1 fichier)
- [ ] `lib.rs` : pour chaque pièce, si `role_to_anchor(role)` = Some(kind) → spawn `AnchorPoint::new(kind, idx)` + `anchor_stats.record(kind)` ; `role="melee_pit"` → name `Module_melee_pit_authored` (synergie boss-portal)
- [ ] `lib.rs` : si `layout.suppress_procedural_modules` → skip le bloc `place_modules` (section 5.5) ; sinon inchangé
- [ ] Garde non-régression : stage **sans** layout authored (`forge_sanctum`) → 0 pièce authored, procédural intact
- [ ] `rtk cargo check -p forgia-stage`

### Phase 3 — Colliders walkable/blocker (2 fichiers)
- [ ] `authored.rs` : helper collider — `walkable` → TriMesh (pattern `boss_portal::solidify_dais`), `blocker` → cuboïde AABB ; idempotent, attend mesh chargé (retry)
- [ ] `lib.rs` : système `sys_collide_authored_pieces` (Update, GameSet::Movement, run_if Roguelite) — pose les colliders sur les pièces walkable/blocker une fois le GLB chargé
- [ ] `rtk cargo check -p forgia-stage`

### Phase 4 — Observabilité (1 fichier)
- [ ] `layout_sensor.rs` : ajouter `layout_source`, `authored_pieces`, `authored_sections` à `forgia2_stage_layout.json` + `next_step` si authored attendu mais 0 pièce (GLB introuvable ?)
- [ ] `rtk cargo check -p forgia-stage` + `rtk cargo clippy -p forgia-stage` (0 warning)

### Phase 5 — Validation runtime + QA
- [ ] `rtk cargo build -p forgia -j 4` (binaire réel, OOM-safe), lancer, entrer arène crypts
- [ ] Auto-QA : sub-agents `verifier` + `qa-lead` (post-impl-auto-qa.md)
- [ ] Story-gate `cargo run -p xtask -- story-gate --story 625`

## Acceptance Criteria

- [ ] AC1 : `arena_layouts.toml` décrit la fosse à mêlée + le perchoir de `crypts_of_anvil` ; **0 coordonnée arène en dur ajoutée dans le Rust**
- [ ] AC2 : en jeu, l'arène crypts montre la composition authored (fosse centrale + perchoir au bord) au lieu du scatter aléatoire
- [ ] AC3 : la pièce `melee_pit` pose `AnchorKind::MeleePit` + nom `Module_melee_pit_authored` → la porte du boss (`boss_portal`) se pose dessus (non cassée)
- [ ] AC4 : le perchoir est **walkable** (collider TriMesh) — le joueur peut monter dessus
- [ ] AC5 : `suppress_procedural_modules=true` → plus de scatter de modules sur crypts ; `forge_sanctum` (sans layout) → procédural **intact** (non-régression)
- [ ] AC6 : `forgia2_stage_layout.json` expose `layout_source="authored"` + `authored_pieces` > 0 quand crypts chargé
- [ ] AC7 : hot-reload Shift+F12 d'`arena_layouts.toml` re-pose les pièces sans rebuild
- [ ] AC8 : 0 warning clippy `-p forgia-stage`, tests purs verts, story-gate vert

## Risques

- **R1 — Draw calls / perf hot path** : N prefabs = N SceneRoots. Mitigation : le parcours en spawne 1216 et tourne ; Tier 1 ≈ 10-20 pièces. Le spawn est OnEnter/au load (pas per-frame). Surveiller `forgia2_render.json::mesh3d_visible`. Si dérive future → instancing/merge (hors scope Tier 1).
- **R2 — Colliders walkable async** : le GLB charge en différé → collider posé trop tôt = raté. Mitigation : système retry idempotent (pattern éprouvé `boss_portal::solidify_dais`).
- **R3 — Casser la porte du boss** : si la pièce melee_pit authored ne pose pas l'anchor + le nom attendu. Mitigation : AC3 explicite + test ; garder le fallback `DAIS_FALLBACK_POS`.
- **R4 — Calibration position des pièces** (pivot GLB, scale natif inconnu) : itération au runtime via hot-reload TOML (pas de rebuild) — accepté, c'est le but du data-driven. Le sensor expose `authored_pieces` pour vérifier le chargement.
- **R5 — Param overflow Bevy 16** dans `spawn_stage_arena_on_request` (déjà proche, a `LayoutParams`) : ajouter le handle/assets authored via un `SystemParam` bundle, pas en params nus.

## Hors scope (stories suiveuses)
- Tier 2 : props signature bible (graffiti, champignons emissive, lampions), palette rose pastel, 6 sections complètes → story-626
- Tier 4 rendu : ré-activer outline, SSAO, ScatteringMedium 0.18 → story séparée
- Éditeur in-game de layout (placer les pièces à la souris) → Phase 2 moteur, différé
