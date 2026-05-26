# Story-513 — Fusion 45 `forgia-pp-*` → 1 `forgia-postprocess`

**Status** : DONE (mergée 2026-05-26 via PR #1, fusion 45→1 livrée, macro `define_simple_pp_effect!` réutilisable)
**BMAD Scale** : Standard (>3 fichiers, bounded scope, suppression + creation 1 crate)
**Created** : 2026-05-23
**Closed** : 2026-05-26
**Branch** : `cleanup/workspace-purge-vague-1-4` (deleted post-merge)
**Predecessor** : story-512 (DONE, PR #1)
**Result** : 45 forgia-pp-* fusionnées dans forgia-postprocess (-89% LOC pp, pattern macro `define_simple_pp_effect!`)

---

## 1. Contexte & motivation

Story-512 §10 a identifie story-513 comme suite immediate. Audit 2026-05-23 :

- **45 crates `forgia-pp-*`** dans le workspace
- **42 sur 45 ont exactement 52 LOC** — boilerplate 100 % identique (sauf nom + shader path)
- 3 outliers : `forgia-pp-outline` (67 LOC, edge_color + thickness + threshold), `forgia-pp-toon` (64 LOC), `forgia-pp-vignette` (52 LOC, mais semble standard)
- **45 shaders WGSL** existent dans `assets/shaders/post_process/`
- **0 consumer** — `grep forgia-pp- crates/ --include="*.rs"` retourne uniquement les declarations Cargo.toml. Aucun App n'ajoute ces plugins.

**Probleme mesurable** :
- 45 crates compilation units pour 2340 LOC dont ~95 % template identique
- Chaque effet = 4 fichiers (Cargo.toml + README.md + manifest.toml + src/lib.rs) = 180 files
- 45 workspace members + 45 workspace deps dans Cargo.toml = 90 lignes pollution

**Cette story = fusion sans perte fonctionnelle.** Tous les Settings types + Plugins + shaders preserved. Consumer API identique :
```rust
// Avant
use forgia_pp_bloom::{BloomSettings, ForgiaPpBloomPlugin};
// Apres
use forgia_postprocess::bloom::{BloomSettings, ForgiaPpBloomPlugin};
```

---

## 2. Scope exact

### Inclus

1. **Creation crate** `crates/forgia-postprocess/`
2. **Macro declarative** `define_simple_pp_effect!(Name, "path/shader.wgsl")` qui expand vers le pattern boilerplate (Settings struct strength: f32 + Plugin + FullscreenMaterial impl)
3. **42 modules simples** generes via macro (1 invocation par module)
4. **3 modules manuels** : outline, toon, vignette (Settings enrichies preservees verbatim)
5. **Mega-plugin** `ForgiaPostProcessPlugin` opt-in (n'ajoute aucun effet par defaut — chacun reste opt-in par Plugin individuel)
6. **Suppression** 45 crates `forgia-pp-*`
7. **Cargo.toml workspace** : -45 members + -45 deps + +1 member + +1 dep
8. **Baseline update** `docs/baselines/workspace_health_2026-05-23.json` → 211 → 167 crates (-44, net -44 car +1 forgia-postprocess)

### Exclus (out of scope)

- Wire-up dans une App (rien ne consomme aujourd'hui, on touche pas)
- Modification des shaders WGSL
- Suppression des shaders (preserves dans `assets/shaders/post_process/`)
- Renommage des Settings types ou Plugin types (preserves verbatim pour 0 break si consumer apparait)
- Audit "lesquels garder vs jeter" (cf story future si jamais on consomme)

---

## 3. Plan d'execution

### Phase 1 — Creation crate forgia-postprocess

- [ ] `cargo new --lib crates/forgia-postprocess`
- [ ] `crates/forgia-postprocess/Cargo.toml` : deps `bevy = { workspace = true }`
- [ ] `src/lib.rs` :
  - `pub mod` declarations pour les 45 effets
  - `pub struct ForgiaPostProcessPlugin` (mega-plugin documente comme opt-in, vide par defaut)
  - Re-exports `pub use ...` pour facilite

### Phase 2 — Macro define_simple_pp_effect!

Dans `src/macros.rs` :
```rust
#[macro_export]
macro_rules! define_simple_pp_effect {
    ($settings:ident, $plugin:ident, $shader_path:literal) => {
        // Settings struct + Default + FullscreenMaterial impl + Plugin — identique au template existant
    };
}
```

### Phase 3 — 42 modules simples via macro

Pour chaque crate parmi les 42 identiques :
- Creer `src/<name>.rs` avec :
  ```rust
  define_simple_pp_effect!(BloomSettings, ForgiaPpBloomPlugin, "shaders/post_process/bloom.wgsl");
  ```
- Verifier shader path existe dans `assets/shaders/post_process/`

### Phase 4 — 3 modules manuels (outliers)

- [ ] `src/outline.rs` : copie verbatim de `forgia-pp-outline/src/lib.rs` (Settings enrichies)
- [ ] `src/toon.rs` : copie verbatim
- [ ] `src/vignette.rs` : verif outlier reel ou simple (probablement simple — sample read decidera)

### Phase 5 — Suppression 45 crates + update Cargo.toml

- [ ] `git rm -rf crates/forgia-pp-*` (45 dirs)
- [ ] Editer `Cargo.toml` workspace : retirer 45 lignes members + 45 lignes deps, ajouter 1 member + 1 dep
- [ ] `cargo check --workspace` PASS
- [ ] `cargo clippy --workspace --no-deps` PASS

### Phase 6 — Post-impl

- [ ] Update baseline JSON (167 crates, scaffolds -42)
- [ ] Commit final + push branche
- [ ] Sub-agents `verifier` + `qa-lead` auto-QA

---

## 4. Critères d'acceptation

- [ ] AC1 : 45 crates supprimees, 1 crate `forgia-postprocess` cree
- [ ] AC2 : `cargo check --workspace` PASS (0 erreur)
- [ ] AC3 : `cargo clippy --workspace --no-deps` PASS (0 warning)
- [ ] AC4 : Les 45 Settings types + 45 Plugin types restent publics et utilisables (test : `use forgia_postprocess::bloom::BloomSettings` compile)
- [ ] AC5 : Tous les chemins shader inchanges (assets/shaders/post_process/*.wgsl)
- [ ] AC6 : Baseline JSON updated
- [ ] AC7 : LOC reduction mesurable (2340 → ~400-500 estime)

---

## 5. Critère de rollback

Si compilation casse > 15 min ou un import externe inattendu apparait → `git reset --hard HEAD~N` jusqu'au commit pre-Phase-5, redocumenter dans la story.

---

## 6. Metriques attendues

| Metrique | Avant | Apres | Delta |
|---|---|---|---|
| Crates total | 211 | 167 | -44 |
| Crates `forgia-pp-*` | 45 | 0 (fusionnes en 1) | -45 |
| Crate `forgia-postprocess` | 0 | 1 | +1 |
| LOC pp total | ~2340 | ~400-500 | -1840 |
| Files pp total | ~180 | ~50 | -130 |
| Workspace members lignes | 211 | 167 | -44 |

---

## 7. Suite

- **story-514** : split `forgia-core` god-object (Enterprise)
- **story-515** : xtask `story-gate` ratchet anti-stub
- Audit futur "lesquels des 45 effets sont reellement utilises" quand un consumer apparait

---

## Sources

- Story-512 §10 (cette story etait planifiee)
- [matklad — Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html)
- [Bevy FullscreenMaterialPlugin docs](https://docs.rs/bevy/0.18/bevy/core_pipeline/fullscreen_material/struct.FullscreenMaterialPlugin.html)
- [Joel Spolsky — Things You Should Never Do, Part I](https://www.joelonsoftware.com/2000/04/06/things-you-should-never-do-part-i/) (preserve all author intent, no rewrite of shaders)
