# Inspection runtime de Forgia par Claude et Codex

Le pont Bevy Remote Protocol (BRP) est installé comme serveur MCP `bevy-brp`.
Il permet à un agent d'inspecter les entités, composants et ressources du jeu
pendant son exécution, au lieu de raisonner uniquement à partir du code.

Le pont est volontairement désactivé par défaut. Un build ou lancement normal
ne compile pas le transport distant et n'ouvre aucun port supplémentaire.

## Utilisation

1. Redémarrer Claude/Codex après une modification de `.mcp.json`.
2. Lancer le jeu depuis la racine avec `cargo forgia-brp`.
3. Demander à l'agent d'utiliser `bevy-brp` pour inspecter le jeu vivant.

BRP écoute par défaut uniquement sur `127.0.0.1:15702`. Ne pas activer la
feature `dev-brp` dans un build distribué.

## Audit des dépendances

`cargo forgia-supply-chain` contrôle les avis RustSec, les dépendances bannies
et l'origine des crates. La configuration initiale bloque les vulnérabilités et
les sources inattendues, mais laisse les doublons historiques en avertissement
afin de ne pas transformer l'adoption de l'outil en régression.

Le premier passage a détecté de la dette déjà présente dans `Cargo.lock`, dont
`crossbeam-epoch 0.9.18` (RUSTSEC-2026-0204) et `quick-xml 0.39.4`
(RUSTSEC-2026-0194). L'outil les rend visibles sans modifier automatiquement les
versions : leurs mises à niveau doivent être traitées et testées séparément.
