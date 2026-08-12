# Story-605 — Prédicteur de visibilité HUD (« caché par design » vs « cassé »)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : trace partielle** (fichier `server.rs`) — une partie de ce
> qu'elle décrit existe, le reste n'a pas été retrouvé.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : À FAIRE (créée 2026-06-18 — audit migration MCP forgia V1)
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
