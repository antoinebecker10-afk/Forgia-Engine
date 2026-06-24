# Story-605 — Prédicteur de visibilité HUD (« caché par design » vs « cassé »)

> **Statut** : À FAIRE (créée 2026-06-18 — audit migration MCP forgia V1)
> **Niveau BMAD** : Standard (étendre le pattern HP existant)
> **Valeur** : **HIGH** — crosshair/hotbar/viewmodel sont centraux au FPS Roguelite
> **Origine** : feature V1 non migrée. Spec = `tools/forgia-mcp/src/server.rs` (`read_ui_state`, `forgia_ui_state.json`). Pattern V2 existant = `forgia2_player_hp_diag.json` (uniquement la barre de PV).

## Problème
Quand un widget HUD n'apparaît pas, on ne sait pas si c'est **normal** (caché par design : menu, mort, pause) ou un **bug**. V1 prédisait, pour chaque widget, s'il *devrait* être visible + un verdict explicatif. V2 ne le fait que pour la barre de PV.

## À construire
- Étendre le pattern `forgia2_player_hp_diag` → `forgia2_hud_state.json` :
  - état modes (run_state, pause, dead, menu),
  - par widget { crosshair, hotbar/loadout, viewmodel, minimap, ammo, quest } : `expected_visible: bool` + raison,
  - `verdict` global (ex. « crosshair attendu mais absent → BUG » vs « hotbar cachée car menu → OK »).
- Refresh 1 Hz. Crate : `forgia-observability` + lecture de l'état UI (`forgia-ui` / `forgia-hud`).

## Acceptance
- [ ] En run normal : tous les widgets core `expected_visible=true`, verdict OK.
- [ ] En pause/menu/mort : widgets gameplay `expected_visible=false`, verdict OK (pas de faux positif « cassé »).
- [ ] Si on force-cache le crosshair en run → verdict signale l'incohérence.
