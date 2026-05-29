# Story-544 — Cleanup 3 crates orphelines (weapon-hitscan + postprocess + spline)

**Status** : DRAFT
**Priorité** : 🟡 P2 — code mort, hygiène workspace
**Scale BMAD** : Standard (story + checklist + decision)
**Origine** : audit wiring 2026-05-27 (`docs/audit/wiring-2026-05-27.md` §1)

## Problème

3 crates sont 100% orphelines (0 Cargo incoming réel, 0 Plugin wiré, 0 consumer pub items) :

1. **forgia-weapon-hitscan** — Plugin défini, aucune dep, aucun `use`. Probablement scaffold extracté de `forgia-combat` jamais réintégré.
2. **forgia-postprocess** — issue de la fusion story-513 (45 `forgia-pp-*` → 1 via macro `define_simple_pp_effect!`). Macro fonctionne mais les effets `toon` + `outline` ne sont jamais `add_plugins(...)` dans le pipeline rendu.
3. **forgia-spline** — déclarée en dep dans `forgia-village-generator/Cargo.toml` mais **aucun `use forgia_spline::`** dans le code Rust → dep morte.

## Décision préalable (à valider user)

**Pour chaque crate, choisir** :

- **A. DELETE** — supprimer crate du workspace (rm + Cargo.toml members), ratchet xtask no-scaffold n'aura plus à les whitelister
- **B. RE-WIRE** — création d'une story dédiée pour réactiver (ex : story-545 "postprocess shader pipeline")
- **C. KEEP allowlist** — ajouter à `xtask/no-scaffold-allowlist.toml` avec justification (asset bundling futur, etc.)

Recommandation par défaut :
| Crate | Reco | Justification |
|---|---|---|
| forgia-weapon-hitscan | A. DELETE | Hitscan vit dans `forgia-combat` (sensor 2026-05-22 montre logique active), extraction abandonnée |
| forgia-postprocess | B. RE-WIRE | Fusion story-513 livrée, effets prêts à activer (toon = direction artistique cartoon bible v1) |
| forgia-spline | A. DELETE dep | Au minimum `cargo remove forgia-spline -p forgia-village-generator` ; delete crate si confirmé inutile partout |

## Critères d'acceptation

- [ ] AC1 — User valide A/B/C par crate (cf tableau ci-dessus)
- [ ] AC2 — Pour DELETE : `Cargo.toml` workspace `members` updated, dossier `crates/X/` supprimé via git
- [ ] AC3 — Pour RE-WIRE : story enfant créée (ex story-545) avec scope précis
- [ ] AC4 — Pour KEEP : entry ajoutée à `xtask/no-scaffold-allowlist.toml` avec ligne justification
- [ ] AC5 — `cargo check --workspace` clean
- [ ] AC6 — `cargo xtask no-scaffold` clean (pas de nouvelle violation)
- [ ] AC7 — MEMORY.md V2 nettoyé des refs `forgia-stage-arena`/`forgia-stage-graph`/`forgia-loot-tables`/`forgia-anchor-kit` (citées dans memories V1 mais inexistantes en V2)

## Cross-refs

- Story-512/513/515 — workspace cleanup vagues 1-4 (déjà mergées)
- Story-516 — cleanup AI+VFX+misc 20 crates (en cours)
- `xtask no-scaffold` — ratchet existant
- [reference_pp_fusion_macro_pattern.md](../../../../d--Forgia/memory/reference_pp_fusion_macro_pattern.md)

## Test post-cleanup

1. **Action** : `cargo check --workspace` puis `cargo run -p forgia-game --profile release-fast`
2. **Redémarrage requis**
3. **Effet attendu** : compile clean, boot menu accessible, modes Arena + RPG fonctionnels (régression check)
4. **Sensor** : `forgia2_diagnostics.json` boot success, `forgia2_entities.json` count player + UI sains
5. **Variantes si KO** : si DELETE casse un test ou un consumer caché, revert immédiat + investigate via `cargo tree` reverse
