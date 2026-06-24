# Story-597 — Roguefight UI Modernization, Phases C & D (suite de story-596)

> **Statut** : TODO — bloquée par la validation runtime de story-596 Phase B
> **Scale BMAD** : Standard (crates forgia-ui-lib / forgia-mode-roguelite / forgia-ui)
> **Parent** : `story-596-roguefight-ui-modernization.md` (roadmap complète + constat audit)

## ⚠️ Reprise — à faire AVANT d'attaquer cette story

1. **Valider runtime la Phase B** (NON commitée au 2026-06-12 01:35, binaire frais `release-fast` 01:34) :
   - Modaux animés (slide-up + rebond + scrim) : Coffre, Defeat, Victory, cartes de zone, Enclume.
   - Enclume en cartes : souris active au Lobby (curseur libéré), achat au clic, pips de rang, bouton ⚒ FORGER.
   - Hover lift 6px cartes Coffre + Enclume.
2. **Commit scopé Phase B** dès validation. Fichiers : `forgia-ui-lib/src/{style.rs, hud/coffre_forgeron.rs}`, `forgia-mode-roguelite/src/{hud.rs, meta_shop.rs}`, `forgia-ui/src/lib.rs`, story-596, story-597.
3. Si KO : variantes documentées dans le récap de session (fallback `sys_break_look_override` pour le curseur Lobby, painter expand pour le lift).

## Phase C — Juice & cohérence HUD

- [ ] C1. **Count-up animé Or/Âmes** : quand `Gold.current` / `MetaSouls.current` change, le nombre tween vers la nouvelle valeur (~0,4s) + pop scale de l'icône (pièce/wisp) via `ease_out_back`. État : `Local<f32>` valeur affichée par compteur dans `draw_currency_counters` (hud.rs). Pattern : `ctx.animate_value_with_time` ou lerp manuel sur `Time`.
- [ ] C2. **HUD in-run unifié palette Forge** (décision DA actée par « go » roadmap) : HP bar (`forgia-ui-lib/hud/player_hp.rs`), wave counter roguelite, panneau AMÉLIORATIONS passent de `C_BG_DARK` (froid, style Arena) à un fond charbon chaud + liseré or — cohérent minimap/portrait/slots déjà migrés. ⚠️ `player_hp.rs` est cross-mode (Fps|Roguelite) : gater le restyle Roguelite OU assumer pour les deux (l'Arena nue n'est plus accessible menu depuis 2026-06-04).
- [ ] C3. **Pause menu + settings habillés** : héritent déjà du thème global (story-596 A2) ; passer les headings en `display_text`, vérifier contraste sliders sur fond chaud, boutons → `cartoon_btn`.

## Phase D — Finitions ship

- [ ] D1. **Icônes painter custom** remplaçant les emojis système (⚒ 🪙 ◇ 🗺 🎲 ↻ ✕ ⇧) — helpers painter dans `style.rs` (la pièce d'or et le wisp existent déjà : `draw_soul_wisp`, closure pièce dans `draw_currency_counters` à extraire).
- [ ] D2. **Échelle UI + opacité genome** : genes `ui_scale` / `ui_hud_opacity` (hot-reload Shift+F12) + `HudLayout` grille de marges unifiée (pad 24 / gutter 8) consommée par les widgets painter.
- [ ] D3. **Passe cohérence FR** des strings UI restantes (« WAVE » → « VAGUE » ?, « ENEMIES » → « ENNEMIS », « NEXT IN » → « SUIVANT DANS » — décision user requise, le mix EN/FR actuel est volontaire ou pas ?).
- [ ] D4. (relevé en passant) Bug latent pré-existant : pause/resume pendant Defeat/Victory/Lobby re-grab le curseur (`OnEnter(InGame)` → `grab_cursor` aveugle aux RunState). Fix : `grab_cursor` conditionnel ou re-release au resume.

## Critères d'acceptance

- check + clippy 0 warning ; tests existants verts (132 au 2026-06-12).
- C1 : les compteurs ne « sautent » plus, ils défilent ; falsifiable en ramassant de l'or.
- C2 : plus aucun panneau `C_BG_DARK` visible en run Roguelite.
- D2 : `ui_scale` modifiable TOML + Shift+F12 sans restart, sensor mis à jour.
- Récap in-game obligatoire à chaque livraison (`.claude/rules/in-game-test-recap.md`).

## Références techniques

- Design system : `forgia-ui-lib/src/style.rs` (palette + `cartoon_btn`/`forge_panel_frame`/`modal_intro`/`ease_out_back`/`draw_soul_wisp`) et `theme.rs` (`display_text`/`display_font`, fonts embarquées, `forge_style`).
- Pièges connus : memory `reference_egui_set_fonts_timing_panic` (set_fonts timing) + `reference_forge_ui_design_system` (anim state chaque frame, painter clipping, RunState global gating).
