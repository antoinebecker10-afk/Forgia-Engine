# Build & Stack (Forgia)

## Build Commands
```bash
# Workspace structure: forgia-engine (lib), forgia-terrain (lib), forgia-game (bin)
cargo build -p forgia-game          # Debug (opt-level 1, deps opt-level 3)
cargo run -p forgia-game            # Run debug
cargo build -p forgia-game --release # Release (thin LTO, codegen-units=1)
cargo clippy --workspace -- -W warnings  # Lint (0 warnings obligatoire)
```
Pas de tests. Cible : 1920x1080 borderless fullscreen.

## Stack technique
| Crate | Version | Role |
|-------|---------|------|
| bevy | 0.18.1 | Moteur ECS |
| bevy_rapier3d | 0.33 | Physique, collisions |
| bevy_egui | 0.39.1 | UI immediate-mode |
| leafwing-input-manager | 0.20 | Input AZERTY |
| bevy_hanabi | 0.18 | Particules GPU |
| bevy_water | 0.18 | Eau procedurale |
| bevy_kira_audio | 0.25 | Audio |
| noise | 0.9 | Perlin terrain |
| fast-surface-nets | 0.2 | Meshing SDF voxel |
| bevy_mod_scripting | 0.19 | Scripting Luau |
| lightyear | 0.26.4 | Networking UDP |

## Codebase
- ~243 fichiers .rs, 3 crates (forgia-game ~198, forgia-engine ~15, forgia-terrain ~15)
- 17 modules forgia-game: ai, combat, components, debug, effects, gamemode, inventory, network, persistence, player, quests, sky, terrain, triggers, ui, vehicle, world
- 18 plugins, 5 collision groups (G1-G5)
