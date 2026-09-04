# Forgia Rewrite — Assets

Structure inspirée de Renzora (7 dossiers media-typés) + extensions Forgia (game + data-driven).

## Renzora parity
- `fonts/` — Typography
- `locale/` — i18n TOML
- `materials/` — .material node-graph
- `particles/` — .particle Hanabi presets
- `previews/` — UI previews
- `scripts/` — Luau gameplay scripts
- `shaders/` — WGSL (generated, layers, post_process)

## Forgia-specific
- `models/` — GLB 3D models
- `textures/` — PNG/KTX2
- `audio/` — sfx/music/voice/ambient
- `genomes/` — TOML data-driven configs (Forgia unique)
- `maps/` — Scene files
- `hdri/` — HDR env maps
- `icons/` — App icons

## Loading conventions

Chaque crate qui charge un asset DOIT déclarer son path dans son `manifest.toml` (champ `assets`). Lock L1 hérité de V1 : whitelist `asset_load_whitelist.txt` enforcée par `xtask check-orphans`.

## Hot-reload

- Genomes : Shift+F12 (debug_toggles)
- Textures/models : `bevy::file_watcher` activé en debug
