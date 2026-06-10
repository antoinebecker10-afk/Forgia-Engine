# Forgia V2

> **Moteur de jeu IA-natif** (Rust / Bevy 0.18.1). Le créateur décrit son jeu et importe
> ses assets, l'IA le construit. Le moat : un codebase **observable** (~100 sensors JSON
> runtime) et **data-driven** (~105 genomes TOML hot-reload) conçu pour être piloté par
> agents IA de façon fiable.

## État actuel (2026-06-10)

**Priorité Phase 0 : SHIPPER le Roguelite** — FPS roguelite type Gunfire Reborn
(`crates/forgia-mode-roguelite`). Boucle jouable : vagues + boss → Victoire/Défaite,
boons, éléments par arme, méta-progression persistée. Ship-readiness ≈ 55-60 % MVG.

Le mode RPG OpenWorld (`forgia-rpg`) est le **track FORGE** : banc d'essai des outils
(terrain streamé, auto-rig, villages procéduraux) qui refluent vers le Roguelite.

- **62 crates**, ~88k LOC, 1 000+ tests — voir [ARCHITECTURE.md](ARCHITECTURE.md)
- Vision : [docs/vision/FORGIA_VISION_2026-06-04.md](docs/vision/FORGIA_VISION_2026-06-04.md)
- Roadmap exécution : [docs/ROADMAP_POST_AUDIT_2026-06-10.md](docs/ROADMAP_POST_AUDIT_2026-06-10.md)

## Quickstart

```bash
# Setup (Rust stable, Windows 10+ cible production)
rustup default stable

# Vérifier (toutes ces commandes sont testées et fonctionnent)
cargo check --workspace
cargo clippy --workspace --no-deps
cargo test -p forgia-mode-roguelite        # tests par crate (voir note)

# Lancer le jeu (binaire canonique = `forgia`, package racine)
cargo run                                   # debug
cargo run --profile release-fast            # itération rapide optimisée
.\run_debug.ps1                              # lance le build existant + log forgia2_run.log
```

> **Note `cargo test --workspace`** : casse actuellement en local (builds concurrents /
> artefacts incrémentaux — voir story-592). La CI et le dev testent **par crate** :
> `cargo test -p <crate>`. Chaque crate passe isolément.

## Le pattern Forgia (ce qui rend le moteur pilotable par IA)

1. **Sensors** : chaque feature écrit `forgia2_<feature>.json` à 1 Hz
   (`{id, severity, next_step, ...}`). Un agent diagnostique l'état du jeu en lisant
   des fichiers, sans le lancer. Registre : `docs/observability/SENSOR_REGISTRY.md`.
2. **Genomes** : les valeurs gameplay vivent dans `assets/genomes/*.toml` et
   `config/`, hot-reloadables (Shift+F12). Pas de hardcode gameplay.
3. **Gates** : `cargo xtask asset-load | no-scaffold | sensor-audit | story-gate |
   verify-sensors-format` — des ratchets mécaniques contre la dérive et le « DONE fictif ».
4. **Stories** : tout travail Standard+ a sa story dans `docs/stories/` avec critères
   falsifiables.

## Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) — les 62 crates réelles, assemblage, GameSet, sensors
- [CONTRIBUTING.md](CONTRIBUTING.md) — setup, conventions, workflow BMAD
- [CLAUDE.md](CLAUDE.md) — contrat IA du workspace
- [docs/audit/audit-2026-06-10-full-codebase.md](docs/audit/audit-2026-06-10-full-codebase.md) — dernier audit complet (16 domaines)
- [docs/ROADMAP_ROGUELITE.md](docs/ROADMAP_ROGUELITE.md) — design/contenu Roguelite
- ADR : [docs/adr/](docs/adr/) — décisions structurantes

## Jalons (roadmap post-audit)

| Jalon | Contenu | Cible |
|---|---|---|
| M0 Filet | push+CI, P0 gameplay, crash hook | ✅ 2026-06-10 (story-592) |
| M1 Moat honnête | docs vraies, sensors véridiques, gates actifs | juin 2026 |
| M2 Démo jouable interne | gameplay + plancher Steam (settings, KTX2) | mi-juillet 2026 |
| M3 Démo publique | packaging, page Steam/itch, playtests | sept. 2026 (Next Fest oct.) |
| M4 Ship 1.0 | contenu élargi, i18n EN | Q4 2026 – Q1 2027 |
| M5 Phase 1 moteur | « le créateur importe ses assets » | 2027 |

## Référence V1

Le code V1 (`D:/Forgia/`) est en mode bug-fix only ; il sert de carrière de patterns
(streaming async, VRAM breakdown) re-portés à la demande.
