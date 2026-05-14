# Genomes reference

Port from V1 `D:\Forgia\RUST\Forgia\Forgia\config\genomes\` (~100 TOML files).

Schema : flat key=value, hot-reloadable Shift+F12, validated by `forgia-genome-validator`.

Each genome category maps to one or more crates :
- `weapons/` → forgia-weapon-* + forgia-combat
- `biomes/` → forgia-terrain + forgia-foliage + forgia-audio-biome
- `enemies/` → forgia-ai-* + forgia-mode-fps-arena
- `economy/` → forgia-xp-curves + forgia-loot-tables + forgia-genome-economy
- `debug_monitor/` → forgia-observability + forgia-qa-*
