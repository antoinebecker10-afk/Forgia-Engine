# Forgia V2

> Moteur de jeu Bevy 0.18 / Rust — dual-mode FPS Arena + RPG OpenWorld.
> Workspace propre repensé 2026-05-14 après audit V1 (12 % shippable / 88 % dette).

## État actuel

**Phase 0 — Bootstrap workspace** (en cours)

13 crates, ~14k LOC cible (vs 205k V1), 0 plugin orphan dès jour 1, 0 `#[allow(dead_code)]`.

## Quickstart

```bash
# Setup (Rust 1.85+)
rustup default stable

# Compile
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace

# Run
cargo run -p forgia-game --release-fast
```

## Structure

13 crates dans `crates/` :

| Crate | Rôle |
|---|---|
| `forgia-core` | États (AppMode/GameMode/WorldMode), GameSet ordering, 0 dep workspace |
| `forgia-assets` | GameAssets Resource + preload + whitelist L1 reborn |
| `forgia-input` | Leafwing PlayerAction AZERTY + KeybindRegistry |
| `forgia-player` | KinematicCharacterController + caméra 1P/3P + spawn/respawn |
| `forgia-combat` | Gunfeel V5-F partagé FPS/RPG (12 fichiers V1 portés VERBATIM Phase 2) |
| `forgia-effects` | Hanabi VFX + audio combat + HitFlashCache (pre-spawn dummy anti-freeze) |
| `forgia-terrain` | Procédural OpenWorld (port verbatim V1, désactivé FPS) |
| `forgia-fps` | Mode FPS Arena : modules KayKit assemblés, bots IA |
| `forgia-rpg` | Mode RPG OpenWorld : quêtes, NPCs (squelette V1, dev V2.M2) |
| `forgia-ui` | Menu + HUD partagés (1 seul handler ESC, MenuCamera2d isolé) |
| `forgia-sensors` | 12 sensors max (vs 95 V1), health monitor, watchdog OS |
| `forgia-game` | bin — wire tout + GameMode switching |
| `xtask` | Automation : check-orphans, schedule-dump, baseline E1/E2 |

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) — graphe deps + GameSet + patterns terrain à reproduire
- [CONTRIBUTING.md](CONTRIBUTING.md) — setup day-1, conventions, BMAD workflow
- [CLAUDE.md](CLAUDE.md) — IA contract Forgia
- [docs/adr/ADR-0001-pivot-v2.md](docs/adr/ADR-0001-pivot-v2.md) — décision V2 elle-même

## Roadmap V2

| Phase | Durée | Livrable |
|---|---|---|
| 0 Bootstrap | 1 sem | Workspace + window 1920×1080 |
| 1 Hello World | 1.5 sem | Player FPS bouge sur ground placeholder |
| 2 Gunfeel V5-F | 4 sem | **DÉCISION GO/NO-GO** — gunfeel = V1 ? |
| 3 Arena modulaire | 5 sem | DM solo vs 4 bots, 5 min sans crash |
| 4 UI/menu propre | 4 sem | 30 cycles menu sans bug |
| 5 Sensors | 3 sem | 12 sensors stack |
| 6 Polish + Steam ship | 5 sem | 5 inconnus 30 min ≥ 7/10 |

**Ship cible** : Q1-Q2 2027 (réaliste), fallback Bots Brawl Q4 2026 si discipline tenue.

## Référence V1

Le code V1 reste vivant en mode **bug-fix only** dans `D:/Forgia/`.
Voir [audit pré-pivot](../Forgia/RUST/Forgia/Forgia/docs/audits/state-of-forgia-2026-05-14.md)
et [plan V2 complet](../Forgia/RUST/Forgia/Forgia/docs/audits/PLAN_V2_FOUNDATIONS_2026-05-14.md).
