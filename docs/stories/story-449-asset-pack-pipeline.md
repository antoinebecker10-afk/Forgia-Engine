---
> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : trace partielle** (symbole `GameAssets`) — une partie de ce
> qu'elle décrit existe, le reste n'a pas été retrouvé.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

id: story-449
title: Asset Pack Pipeline (manifest + fetch + verify + install)
status: IN_PROGRESS
scale: BMAD Enterprise
created: 2026-05-18
workspace: V2 Rewrite
---

# Story-449 — Asset Pack Pipeline

> Industry-grade external CC0 asset pack distribution. Remplace les 9 orphan
> gitlinks `assets/models-v1/packs/kaykit-*` (513 MB) par un manifest TOML +
> lockfile SHA256 + installer CLI. Pattern Cargo.lock + Unity Addressables.

## Vision

> **Le repo Forgia commit le manifest des packs (qui/où/quel hash). Le contenu
> des packs (513 MB CC0) est gitignored. Un installer CLI Rust (fetch + verify
> SHA256 + extract zip) restitue les packs sur n'importe quelle machine en 1
> commande. Sensor + health alerte si pack manquant au boot avec next_step.**

## Concept-First Protocol

- **Étape 0 (data vs code)** : data layer `packs.toml` (manifest) + `packs.lock`
  (hashes pinned). Code layer = parsing + fetch + verify + extract. **Sources
  download = URLs externes**, pas hardcoded en Rust.
- **Étape 1 — hypothèses** :
  - (a) **Cargo.lock pattern** : manifest committed, lockfile committed, content
    fetched on demand, gitignore content. AAA-validé (Cargo, Nix, cargo-binstall).
  - (b) **Submodules git** : .gitmodules + repos miroirs. Rejeté car KayKit ne
    publie pas de repos publics + 513 MB en submodules = clones lents.
  - (c) **Cloud storage Forgia-owned** (S3) : overkill solo dev + coût + dette.
- **Choix** : (a) Cargo.lock pattern.
- **Étape 2 — cartographier** :
  - Existant : `forgia-asset-registry` (entry-level metadata, scan filesystem,
    biome compat, AABB calibration). Pas un pack manager.
  - Scaffolds vides à peupler : `forgia-assets-bundle` (16 LOC stub "Asset
    bundle pack zstd"), `forgia-asset-cdn` (16 LOC stub "CDN-fetched asset
    registry"). Règle `fine-grained-crates.md` → peupler, pas créer nouveau.
- **Étape 3 — verbaliser** : Producteur = `assets/packs.toml` (boot). Consumers
  = `forgia-asset-cdn::install_cli` (one-shot CLI), `forgia-asset-registry`
  (Startup reconcile). Sensor `forgia_pack_registry.json`. Hot path = non.
- **Étape 4 — hot path check** : N/A (boot/CLI only).
- **Étape 5 — scale-up BMAD** : 3 crates touchés (2 peuplés + 1 extension) →
  Enterprise confirmé. Story + checklist obligatoires.

## Industry references (verified sources)

| Source | Pattern adopted | URL |
|---|---|---|
| Cargo.lock | Lockfile content-addressed (SHA256 per artifact) | doc.rust-lang.org/cargo |
| Unity Addressables | `catalog.json` + `catalog.hash` sidecar | docs.unity3d.com/Packages/com.unity.addressables@2.0 |
| Unreal Asset Registry | Async metadata-only discovery before bytes | dev.epicgames.com/documentation/asset-registry-in-unreal-engine |
| cargo-binstall | Stage separation fetch → verify → install | crates.io/cargo-binstall |
| bevy_asset_loader | Dynamic RON assets (idiomatic Bevy 0.18) | github.com/NiklasEi/bevy_asset_loader |

## Architecture

```
assets/packs.toml          ← manifest committed (URLs, versions, SHA256)
assets/packs.lock          ← lockfile committed (resolved SHA256 + sizes)
assets/models-v1/packs/*/  ← gitignored (513 MB CC0 content)

┌──────────────────────────────────┐  ┌──────────────────────────────┐
│ forgia-assets-bundle              │  │ forgia-asset-cdn              │
│ (populate Tier 1, lib only)       │  │ (populate Tier 1, lib + CLI)  │
│ ─ PackManifest TOML parser        │  │ ─ Stage1 fetch (reqwest)      │
│ ─ PackLockfile content-addressed  │  │ ─ Stage2 verify (sha2)        │
│ ─ Reconcile manifest vs lockfile  │  │ ─ Stage3 extract (zip)        │
│ ─ License/URL/version validation  │  │ ─ CLI binary `install` /     │
│ ~300 LOC + tests                  │  │   `verify` / `clean`          │
│                                   │  │ ~500 LOC + tests              │
└──────────────────────────────────┘  └──────────────────────────────┘
              │                                  │
              └────────────────┬─────────────────┘
                               ▼
        ┌──────────────────────────────────────────────┐
        │ forgia-asset-registry (EXTEND existing)       │
        │ ─ Load packs.toml at Startup                  │
        │ ─ Reconcile : warn if pack missing            │
        │   with next_step "cargo run -p ..."           │
        │ ─ Sensor forgia_pack_registry.json            │
        │ +80 LOC, health side-file                     │
        └──────────────────────────────────────────────┘
```

## Acceptance criteria

- [ ] `assets/packs.toml` declaratif pour les 9 packs KayKit existants
- [ ] `assets/packs.lock` avec SHA256 réels mesurés des packs locaux
- [ ] `assets/models-v1/packs/` ajouté à `.gitignore`
- [ ] `git rm --cached` sur les 9 gitlinks orphelins
- [ ] `cargo run -p forgia-asset-cdn --bin install` télécharge + vérifie + extrait
- [ ] `cargo run -p forgia-asset-cdn --bin verify` valide hashes locaux
- [ ] `forgia_pack_registry.json` sensor (1Hz) avec found/missing/total counts
- [ ] Health side-file `forgia_pack_registry_health.json` si packs manquants
- [ ] 0 warning clippy strict `-D warnings`
- [ ] Tests : parse manifest valide/invalide, SHA256 verify ok/mismatch, zip
  extract robust to traversal, lockfile diff manifest detection
- [ ] cargo check workspace + binary forgia ok

## Plan d'exécution (4 vagues)

### Wave 1 — `forgia-assets-bundle` (manifest + lockfile parsing)
- PackManifest schema (name, version, source_url, download_url, sha256, etc.)
- PackLockfile (frozen state, content-addressed)
- TOML serde + validation
- Reconcile manifest vs lockfile (diff detection)
- 8+ unit tests
- Effort : 2-3h

### Wave 2 — `forgia-asset-cdn` (fetch + verify + install + CLI)
- Stage1 `fetch_pack(url, dest)` via reqwest tokio async
- Stage2 `verify_sha256(path, expected)` via sha2
- Stage3 `extract_zip(zip, dest, strip_root_dir)` via zip crate, anti-traversal
- Orchestrator `install_pack(manifest_entry) -> Result<Installed>`
- CLI binary : `install [pack_name]`, `verify [pack_name]`, `clean`, `status`
- Parallel installs via `tokio::spawn` + progress bars `indicatif`
- 10+ unit tests + 2 integration tests
- Effort : 3-4h

### Wave 3 — Extend `forgia-asset-registry`
- Load `assets/packs.toml` at Startup
- Reconcile : compare vs `assets/models-v1/packs/<pack_dir>/`
- Sensor `forgia_pack_registry.json` (1Hz)
- Health side-file with next_step convention QUALITY_GATE
- +1 test reconcile
- Effort : 1h

### Wave 4 — Migration repo (one-shot)
- Génère `assets/packs.toml` depuis les 9 packs présents
- Compute SHA256 réels via `forgia-asset-cdn` verify mode → écrit `packs.lock`
- `.gitignore` += `assets/models-v1/packs/*/` (avec exception `.placeholder`)
- `git rm --cached` les 9 gitlinks
- Dry-run install pour valider full pipeline
- Effort : 1h

## Stability Locks impactés

- L1 (GameAssets baseline) : N/A — packs externes hors `GameAssets` scope
- L7 (GameSet) : Startup uniquement, pas de hot path
- Aucun Lock modifié

## Out of scope Phase 1

- Compression interne `zstd` (rétention forgia-assets-bundle scope original)
- Delta updates Riot-style content-defined chunking
- UI Bevy wizard first-run download (crash propre + CLI suffisant Phase 1)
- TAR.GZ / 7z (ZIP only)
- Mirror URLs fallback (single source URL Phase 1, mirror_urls[] champ TOML
  réservé pour Phase 2)
- Cache global cross-projet (toujours par-projet Phase 1)

## Risks

- 🟡 KayKit URLs itch.io stables ? → Stocker `download_url` direct + checksum,
  re-download possible si lien mort via mirror_urls Phase 2
- 🟢 Hashing 513 MB = ~5-10s SSD, acceptable car one-shot
- 🟢 reqwest deps lourde (~50 deps transitives) → acceptable, partagé avec
  potentielles features réseau futures

## Cross-refs

- `.claude/rules/fine-grained-crates.md` — peupler scaffolds existants
- `.claude/rules/no-hardcode.md` — URLs/versions/hashes en TOML
- `.claude/rules/observability-required.md` — sensor + health obligatoires
- Memory `reference_rule_fine_grained_crates.md` — 5 scaffolds à peupler V2
- Memory `feedback_v2_tech_debt_audit_protocol.md` — audit fin de session
