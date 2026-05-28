# Story-557 — Audit + plan de restoration assets V1 legacy paths

> **Status** : DRAFT (audit done, plan à valider)
> **Scale BMAD** : Standard
> **Effort estimé** : ~1j audit complet + plan, ~2-5j exec selon stratégie

## Symptôme

`forgia2_run.log` produit 100+ erreurs `Path not found` au boot/streaming pour assets V1 legacy. Commit V2 `184e091` a gitignored `assets/*-v1/` (purge git 45GB→454MB) mais le code V2 référence toujours ces paths.

Memory existant : [reference_v2_asset_paths_legacy_models_v1.md](../../../memory/reference_v2_asset_paths_legacy_models_v1.md).

## Scope quantifié (agent diag 2026-05-28)

| Pattern | Occurrences | Crates | Status multi-terminal |
|---|---|---|---|
| `models-v1/` | 16 | forgia-asset-registry, forgia-assets-bundle, forgia-terrain | mixte (forgia-asset-registry SAFE, forgia-terrain LOCKED) |
| `audio-v1/` | 12 | forgia-audio | LOCKED autre terminal |
| `textures-v1/` | 12 | forgia-rpg (7), forgia-terrain (4), forgia-observability (1) | LOCKED autre terminal |

Estimation : **~30% des paths V1 ont un équivalent V2** nommé différemment (e.g. `pbr/jolcham_oak_bark_01/` au lieu de `textures-v1/terrain/`). Audio et models V2 très incomplets.

## Acceptance Criteria

### Phase 1 — Audit complet (DRAFT validé par cette story)

- [x] AC1 — Quantification occurrences par pattern + crate (fait)
- [x] AC2 — Status multi-terminal SAFE vs LOCKED par crate (fait)
- [ ] AC3 — Inventaire path-par-path : tableau (V1 path) → (V2 equivalent existe ? oui/non) → (action proposée)
- [ ] AC4 — Estimation taille assets à restaurer si stratégie (A)

### Phase 2 — Stratégie (à choisir)

3 options évaluées :

**A — Restaurer assets V1** depuis backup V1 desktop, copy local sans git
- Pour : zéro touche code, dev local marche immédiatement
- Contre : pas reproducible (autres machines/CI), git ignore les fichiers, taille disque
- Estimation : ~500MB-2GB selon scope

**B — Re-pointer paths V1 → V2** où équivalent existe + downscope ailleurs
- Pour : reproducible, propre, aligne avec V2 architecture
- Contre : ~70% des paths n'ont PAS d'équivalent V2 → faut acquérir/régénérer ces assets
- Estimation : touche 3 crates SAFE + 4 LOCKED → coordination multi-terminal lourde

**C — Gitignore + dummy fallbacks** dans le code (Asset::default si Path not found)
- Pour : silence les errors, ship-compatible (asset manquant = pas crash)
- Contre : masque les bugs, dégrade visuel silencieusement, dette technique
- Estimation : ~1j

### Phase 3 — Exec stratégie choisie (effort variable)

## Risques multi-terminal

- forgia-audio, forgia-terrain, forgia-rpg, forgia-observability **LOCKED** par autre terminal actif (15 fichiers modifiés WIP). Phase 3 attendra push autre terminal.
- forgia-asset-registry SAFE (déjà modifié = on peut continuer dessus).

## Cross-refs

- Memory : [reference_v2_asset_paths_legacy_models_v1.md](../../../memory/reference_v2_asset_paths_legacy_models_v1.md)
- Memory : [reference_v2_kaykit_assets_minimal_copy.md](../../../memory/reference_v2_kaykit_assets_minimal_copy.md) (pattern restoration ciblée KayKit)
- Commit V2 purge : `184e091` (gitignored `assets/*-v1/`)
