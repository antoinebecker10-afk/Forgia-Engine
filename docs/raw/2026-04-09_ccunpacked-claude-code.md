# Claude Code Unpacked — Internals

> Source: https://ccunpacked.dev/µ
> Ingéré: 2026-04-09

## Ce que c'est
Reverse-engineering documenté du CLI Claude Code (Anthropic). Par Zakaria O. I. A.

## Architecture
- Agent Loop 11 étapes: input → API → tools → rendu
- 1500+ fichiers source, 50+ outils, 72+ slash commands
- Multi-agent: AgentSendMessage, TaskCreate/Update/List, TeamCreate

## Features cachées (feature-flagged, pas encore activables)
- **Buddy**: pet terminal (espèce/rareté basée sur account ID)
- **Kairos**: mémoire persistante cross-session + actions background autonomes
- **UltraPlan**: planning étendu Opus, fenêtres 30min
- **Coordinator Mode**: lead agent → N workers en worktrees isolés → agrégation
- **Bridge**: remote control phone/browser avec approbation permissions
- **Daemon Mode**: `--bg` flag, tmux sous le capot
- **UDS Inbox**: communication inter-sessions via Unix domain sockets
- **Auto-Dream**: consolidation post-session automatique

## Features activables utiles
- `--resume` : reprendre session après crash
- `/effort` : scaler compute (bas=quick fix, haut=plan enterprise)
- `/ctx_viz` : visualiser consommation contexte
- TaskCreate/TaskOutput : tâches async sans bloquer session principale

## Slash commands avancées
- `/context`, `/memory`, `/compact`, `/brief`, `/add-dir`
- `/debug-tool-call`, `/perf-issue`, `/stats`, `/cost`, `/usage`
- `/export`, `/summary`, `/session`, `/files`
