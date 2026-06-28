# Forgia — Overlay projet

> Spécificités **Forgia**. Le contrat universel = `../../CLAUDE.md` (lu en premier). Cet overlay s'applique quand on travaille sur Forgia.

## Code (V2)
- **Workspace** : `C:\Users\Antoi\Desktop\Forgia Rewrite` — 62 crates sous `crates/` + `src/main.rs`.
- **Binaire réel** : `forgia.exe` (package `forgia`) → `cargo build -p forgia`. PAS `forgia-game` (legacy, relink jamais le bon bin = exe stale silencieux).
- **Legacy V1** : `D:\Forgia\RUST\Forgia\Forgia` — banc de pièces (sur GitHub), **ne pas y coder**.
- `grepai` (recherche sémantique, obligatoire concept-first §2) indexe le **workspace V2**.

## Vision (pivot 2026-06-04)
Forgia = **moteur IA-natif** (le créateur importe ses assets, l'IA construit le jeu). **Priorité absolue = SHIP le Roguelite** (FPS roguelite type **Gunfire Reborn**). RPG = track FORGE (outils anim/rig qui refluent dans le Roguelite). Filtre de scope : *« ça débloque le ship Roguelite ? »*.

## Stability Locks (modif = demande explicite utilisateur)
L1 GameAssets · L2 PerfMode F4 · L3 Camera collision · L4 EditorRaycast · L5 Nameplate LOD · L7 SystemSets (GameSet : Input→Movement→Physics→Camera→Combat→Effects→UI) · L8 Minimap cache · LOCK-INV-1 Inventory 80 slots max.

## Observabilité
~113 sensors à la racine V2 : `forgia2_*.json` (65, V2 natif) + `forgia_*.json` (48, hérités). Toute feature → son sensor (`observability-required`). Diag « regarde » = lire les JSON, pas grep aveugle.

## Stack
Bevy 0.18.1 · bevy_rapier3d 0.33 · bevy_egui 0.39 · leafwing-input-manager (AZERTY) · bevy_hanabi 0.18 · bevy_water 0.18 · bevy_kira_audio · lightyear (net) · bevy_mod_scripting (Luau). Build : `rules/build-stack.md`.

## Données vs code (4 couches — `rules/data-driven-paths.md` + `no-hardcode.md`)
framework (Rust) · **definition** (`config/genomes/*.toml`, `config/*.json`) · behaviour (Luau) · exception (story). Une valeur de balance → gène genome, jamais hardcode.

## BMAD
Conservés : convention **`story-NNN`** (`Forgia Rewrite/docs/stories/`) + 3 tiers (Quick≤3 / Standard≤10 / Enterprise 10+). Le moteur `.bmad` (v6.2.2) était un **zombie figé V1 → débranché**. v6.8 = plugin Claude Code si réactivation. Couche game-design ajoutée : commandes **`/gdd`** + **`/playtest`** (adaptées genome/sensors/stories).

## Table concept → fichier:ligne
La table des concepts Forgia (water/combat/terrain/…) avec producteur/consommateurs/timing/sensor : **`rules/concept-first-table-forgia.md`** (extraite du protocole universel `concept-first`).

---
> Sensors, Locks et chemins reflètent l'état au 2026-06-17. Une ligne dont le path n'existe plus en V2 = à re-pointer (anti-dérive).