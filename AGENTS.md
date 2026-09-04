# Agents IA : point d'entrée

Ce fichier s'applique à tout le workspace, quel que soit l'agent (Claude Code, Codex,
Cursor ou autre).

1. Lire `CLAUDE.md` : la vision, les Stability Locks, les anti-traps et les règles
   absolues s'appliquent à tous les agents.
2. Lire `ARCHITECTURE.md` avant de toucher à une crate : la liste des crates y est gardée
   mécaniquement par `cargo xtask arch-drift`.
3. Avant tout diagnostic runtime, lire les capteurs `forgia2_*.json` écrits par le jeu
   plutôt que raisonner à partir du code seul. Le registre est dans
   `docs/observability/SENSOR_REGISTRY.md`.
4. Les valeurs de gameplay vivent dans les génomes TOML (`assets/genomes/`, `config/`),
   jamais dans le code.
5. Vérifier avant de modifier, compiler après, et valider les changements en proportion
   du risque : `cargo check`, `cargo clippy --workspace --all-targets -- -D warnings`,
   `cargo test -p <crate>`, puis les gates `cargo xtask`.

Français pour la documentation et les échanges, anglais pour le code.
