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
- **Inc.2 — Portail de choix** — ✅ FAIT (non commité→commit suivant) : `draw_portal_overlay`
  RÉVEILLÉ (stub depuis 471..479) — après le clear d'une salle (hors boss), l'orchestrateur
  gèle et propose les kinds des variants du graph (cap `branching`) ; overlay portes typées
  (emoji+couleur, flèches/1-4/clic) → `portal_pick` consommé → transition + spawn.
  `RogueliteWave.{portal_choices, portal_pick, room_kind}` (room_kind = hook Inc.3).
  Fallback auto si <2 variants. Inc.1 validé runtime user au préalable.
- **Inc.3 — Salles typées** : ⏸️ **EN PAUSE 2026-07-02** (collision multi-terminal :
  l'autre terminal édite waves.rs/lib.rs — story-652 head hitbox, non commité ; décision
  user = attendre son commit). **Design validé, prêt à implémenter** :

  **A. Refactor `RoomPhase` FSM (préalable, waves.rs)** — remplace in_break/
  break_secs_left/seen_alive/portal_choices/portal_pick par UN enum :
  `Fighting{seen_alive} | Break{secs_left} | PortalChoice{doors, pick}` + helpers
  (`is_break/break_secs_left/portal_doors/portal_pick_pending/set_portal_pick/label`).
  La détection de clear n'existe QUE dans Fighting (classe de bugs du 2026-07-02 morte
  par construction). Garder victory_emitted/boss_defeated (latches run-level).
  Consommateurs à migrer : hud.rs:106 (NEXT IN), hud.rs:353 (override curseur),
  draw_portal_overlay, forgia-ui/lib.rs:516 (sync modal), sensor.rs. ⚠️ sensor JSON :
  GARDER les champs `in_break`/`break_secs_left` (dérivés de la phase) —
  forgia-observability/roguelite_health.rs les PARSE. Ajouter `"phase"` label.

  **B. Inc.3 — les portes comptent** (`advance_to_room` → effet d'entrée par kind) :
  Combat/Event(fallback documenté) → vagues normales · **Elite** → counts × gene
  `roguelite_elite_count_mult` (déf 1.5, clamp 1-3, aussi ×souls au clear) ·
  **Treasure** → 0 combat, +genes `roguelite_treasure_souls`(25)/`_gold`(50), phase=
  Break puis portes · **Shop** → coffre payant sans combat, idem · **Rest** → heal
  full + bouclier refill (commands.queue pattern existant), idem. `spawn_wave_enemies`
  gagne `count_mult: f32` (ceil). HUD : « SALLE 2/5 · ÉLITE · VAGUE 1/2 » via
  `stage_kind_display`. Sensor : + `"room_kind"`. Genes dans roguelite_run.toml +
  RunGraphConfig (graph.rs, pattern waves_per_stage).

## Hors scope
- Actes/biomes (P2-4). Parcours inter-salles (R2.3 du rapport, après Inc.2).
