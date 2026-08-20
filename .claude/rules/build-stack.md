# Build & Stack (Forgia V2)

> Réécrite 2026-08-20 — l'ancienne version décrivait le stack **V1** (3 crates,
> « pas de tests », lightyear) et induisait les sessions en erreur. Chiffres du jour.

## Commandes

```bash
cargo forgia-dev                    # LA commande de dev : Tracy + BRP (cf. outillage.md §5bis)
cargo build -p forgia               # binaire canonique = `forgia` (package racine)
cargo clippy --workspace -- -W warnings   # 0 warning obligatoire
cargo run -p xtask -- <gate>        # les cliquets (story-gate, context-budget, …)
```

⚠ `cargo run -p forgia-game` = exe stale silencieux. ⚠ RTK fausse clippy
(`reference_rtk_wraps_cargo_hides_clippy_lints`).

## Stack

Bevy **0.18.1** (pinné jusqu'au ship) · bevy_rapier3d 0.33 · bevy_egui 0.39 ·
bevy_hanabi 0.18 · bevy_kira_audio 0.25 · leafwing-input-manager 0.20 (AZERTY) ·
vleue_navigator 0.15 (navmesh). **Aucune dépendance réseau** (lightyear = V1, jamais porté).

## Volume (mesuré 2026-08-20)

**68 crates** · ~158 k LOC · **2 234 tests** · 147 capteurs · gates xtask en CI.
L'état exact des crates : `ARCHITECTURE.md` (gardé par `arch-drift`).
