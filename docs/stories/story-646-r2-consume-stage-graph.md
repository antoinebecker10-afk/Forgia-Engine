# Story-646 — R2 : consommer le stage-graph (multi-salles clear-to-progress)

> **Source** : rapport `rapport-gunfire-like-identite-2026-07-02.md` §R2 + masterplan P2-1/P2-2.
> **Scale BMAD** : Enterprise (forgia-stage + forgia-mode-roguelite, machine d'états run).
> **Statut** : IN_PROGRESS — Inc.1 en cours.

## Constat (cartographie)
`sys_stage_dispatch` (run.rs:130) charge déjà l'arène selon `InRun{stage}/Boss{stage}` et
`RunGraph` est déjà en Resource (run.rs:735) — mais PERSONNE ne fait les transitions :
l'orchestrateur (waves.rs) joue 3 vagues dans la salle 0 et la run s'arrête là.
`graph.stages[depth].kind` jamais lu.

## Incréments
- **Inc.1 — Enchaînement multi-salles linéaire** : l'orchestrateur enchaîne
  `boss_depth` salles combat (N vagues/salle, gene `roguelite_waves_per_stage`) puis la
  salle Boss (composition boss). Transitions `InRun{s}`→`InRun{s+1}`→`Boss{n}` (le dispatch
  d'arène + alternance crypts/forge suivent tout seuls). `RogueliteWave.{stage, seen_alive}`
  (le gate anti-race devient resettable par salle). `graph.stages[depth].kind` loggé + HUD
  « SALLE s/N · VAGUE w/W ». Boucle boss→porte→parcours→Victory inchangée.
- **Inc.2 — Portail de choix** : réactiver `draw_portal_overlay` (dead_code) — 2 portes
  typées après clear, le choix pilote le variant du graph.
- **Inc.3 — Salles typées** : consommer `kind` pour la composition (Elite/Rest/Treasure)
  + récompense typée + sensor `forgia2_run_progress.json`.

## Hors scope
- Actes/biomes (P2-4). Parcours inter-salles (R2.3 du rapport, après Inc.2).
