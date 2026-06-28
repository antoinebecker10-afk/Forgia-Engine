# Story-633 — Dette : migrer les asset loads ad-hoc vers GameAssets (Lock L1)

> **Statut** : 🟡 BACKLOG (dette identifiée 2026-06-25, créée au moment du rebaseline asset-load).
> **Niveau BMAD** : Standard (5+ fichiers). **Origine** : pre-push asset-load gate.

## Contexte
Le **Lock L1** (ratchet anti-prolifération `asset_server.load()`, story-528) a une cible **≤ 30
call-sites**. Au 2026-06-25 on est à **91** (baseline rebaselinée 84 → 91 pour débloquer le push de
la session bus-QA). Le ratchet va dans le **mauvais sens** (creep). Chaque `asset_server.load()`
ad-hoc = chargement synchrone hors `forgia-assets::GameAssets` (preload) → risque de hitch/stutter
runtime + pas de cache partagé.

## Nouveaux call-sites acceptés au rebaseline (à migrer)
- `crates/forgia-game/src/cyber_city.rs` (1) — démo Cyber City GLB
- `crates/forgia-mode-roguelite/src/boss_portal.rs` (2) — assets portail boss
- `crates/forgia-mode-roguelite/src/merchant.rs` (2) — assets marchand
- `crates/forgia-mode-roguelite/src/weapon_select.rs` (1) — preview armes wizard
- `crates/forgia-stage/src/lib.rs` (6, budget 5) — décor stage (+1 au-delà du budget)

## Objectif
Migrer ces loads (et le reste du backlog 91→30) vers `forgia-assets::GameAssets` (handles
préchargés OnEnter, cache partagé). Faire **redescendre** le ratchet vers la cible ≤30.

## Acceptance criteria
- [ ] Les 12 nouveaux call-sites passent par GameAssets (ou justification documentée si load légitime à la volée).
- [ ] `cargo xtask asset-load` : baseline **réduite** (pas augmentée) après migration.
- [ ] 0 régression runtime (assets toujours chargés, pas de hitch ajouté).

## Cross-refs
- Lock L1 : story-528 (ratchet asset-load).
- `xtask/asset-load-allowlist.toml` (baseline courante 91).
- Rebaseline d'origine : commit session bus-QA (2026-06-25).
