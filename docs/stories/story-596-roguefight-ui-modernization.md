# Story-596 — Roguefight UI Modernization (roadmap A→D)

> **Statut** : Phase A DONE — validée runtime par Antoine 2026-06-12 (« top ») après fix BUG-596-01. Phase B en cours.
> **Scale BMAD** : Standard (Phase A ~8 fichiers, crates forgia-ui-lib / forgia-ui / forgia-mode-roguelite)
> **Origine** : audit esthétique UI 2026-06-11 (session Claude) — l'UI a une DA documentée
> (bible v1 cartoon kid-friendly, story-558 Phase 7) mais un rendu « prototype » :
> aucune font custom, aucun `set_style` egui, 3 styles d'écrans différents, Enclume en texte brut.

## Constat (audit 2026-06-11)

1. **Aucune font custom** — `assets/fonts/` vide, 0 `set_fonts` dans le workspace → font egui par défaut partout (tueur n°1 du « moderne »).
2. **Aucun thème egui global** — boutons/sliders natifs gris egui (menu, Victory, pause) en clash avec la palette Forge.
3. **3 styles d'écrans** : Defeat cartoon bois+or (✅ Phase 7) vs Victory noir+vert+boutons bruts (`hud.rs:493`) vs reward cards gris-violet vs Enclume brun.
4. **Enclume des Âmes = 4 lignes de texte brut** (`meta_shop.rs:445`) — le hub méta vu à chaque run.
5. **Drift couleurs** : `speaker_color` (hud.rs:613) ≠ `forge_persona_color` (style.rs:263), littéraux FORGE_OR/BRAISE recopiés.
6. **Modaux sans transition** + icônes emoji système ; `ease_out_back` copié 3×.

## Roadmap

### Phase A — Fondations (CETTE STORY, session 2026-06-11)

- [x] A1. Fonts OFL téléchargées : `assets/fonts/LilitaOne-Regular.ttf` (display cartoon) + `Poppins-Regular.ttf` (+SemiBold en réserve) — Google Fonts, ship OK.
- [x] A2. `forgia-ui-lib::theme` (NOUVEAU) : `FontDefinitions` embarquées (`include_bytes!`), famille `forge-display` (Lilita One), Proportional → Poppins ; `forge_style()` Visuals Forge (boutons charbon chaud, hover or, corner_radius 12) ; `ForgeThemePlugin` apply-once + sensor `forgia2_ui_theme.json` + 4 tests.
- [x] A3. Branchement via `ForgiaUiHudPlugin` (déjà wiré par forgia-game — **forgia-game non touché**, diff autre terminal). Guard `is_plugin_added`.
- [x] A4. Dédup couleurs : `speaker_color` délègue à `forge_persona_color` ; littéraux boon_visual/meta_shop → constantes ; `ease_out_back` centralisé dans `style.rs` (2 copies supprimées) ; constantes `FORGE_PANEL` + `FORGE_PANEL_LIGHT`.
- [x] A5. Victory overlay → pattern cartoon Defeat (bois+or, `cartoon_btn` partagé, titre display « VICTOIRE ! » or).
- [x] A6. `cartoon_btn` + `forge_panel_frame` extraits dans `style.rs` (la closure locale du Defeat devient le composant partagé).
- [x] A7. Menu principal : titre display « FORGIA » 84px C_PRIMARY, scrim dégradé vertical (mesh 40→170 alpha), boutons cartoon (bois/or CTA/métal), retrait footer « Phase 1 — Hello World jouable ». (dep `forgia-ui-lib` ajoutée à `forgia-ui`)
- [x] A8. Harmonisation fonds : reward cards + Enclume sur `FORGE_PANEL`, headings display (Coffre, Enclume, reward, Defeat, Victory, wave counter, compteurs monnaie, panneau AMÉLIORATIONS).

**Validation 2026-06-11** : `cargo check` + `cargo clippy --no-deps` 0 warning (forgia-ui-lib, forgia-ui, forgia-mode-roguelite) ; `cargo test -j 4` : 130 passed. Auto-QA sub-agents non lancés (spend limit API atteint ce jour) → self-review manuelle : Locks intacts (L1 : embed, 0 call-site `asset_server.load()`), pas de gène requis (pas de seuil/toggle), sensor présent. **Runtime non validé** — récap test in-game fourni en session.

**BUG-596-01 (2026-06-12, crash au boot)** : panic frame 1 `FontFamily::Name("forge-display") is not bound to any fonts` (epaint fonts.rs:808, système `main_menu_ui`). Cause : `set_fonts` appelé depuis `EguiPrimaryContextPass` = pendant la passe → fonts reconstruites seulement au `begin_pass` suivant, mais le menu dessinait la famille display dès la frame 1. **Fix** : `sys_apply_forge_theme` déplacé en `PreUpdate`, `.after(EguiPreUpdateSet::InitContexts).before(EguiPreUpdateSet::BeginPass)` — les fonts sont enregistrées avant la première passe egui, plus aucune fenêtre de panique. Leçon : `ctx.set_fonts`/`set_style` JAMAIS depuis la passe egui si une famille nommée custom est utilisée (epaint panique, pas de fallback).

**Critères d'acceptance Phase A** :
- `cargo check` + `clippy` 0 warning sur les 3 crates.
- In-game : tous les textes UI rendus en Poppins/Lilita One (plus de font egui défaut).
- Boutons natifs (menu, pause/settings) aux couleurs Forge sans édition par-widget.
- Victory visuellement cohérent avec Defeat.
- Sensor `forgia2_ui_theme.json` : `applied=true` + fonts listées.

### Phase B — Écrans-clés (session suivante)

- B1. Enclume des Âmes refondue en 4 cartes cliquables : icône painter, nom display, pips de rang (●●●○○), coût + icône âme (wisp), souris + clavier, bouton « FORGER » central.
- B2. Helper `modal_pop_in` (scale ease-out-back 200ms + scrim fade) appliqué à Coffre / Enclume / reward cards / Defeat / Victory.
- B3. Hover lift + glow or sur les cartes du Coffre et des reward cards.

### Phase C — Juice & cohérence HUD

- C1. Count-up animé Or/Âmes + pop scale de l'icône au gain.
- C2. HUD in-run unifié palette Forge : HP bar / wave counter / panneau AMÉLIORATIONS passent de `C_BG_DARK` froid à charbon chaud + liseré or (déjà le cas minimap/portrait/slots). Décision DA validée par « go » roadmap.
- C3. Pause menu + settings habillés Forge (héritent déjà du thème A2 ; ajuster sliders/headings).

### Phase D — Finitions ship

- D1. Icônes painter custom remplaçant les emojis système (⚒ 🪙 ◇ 🗺 🎲 ↻ ✕ ⇧).
- D2. `HudLayout` : grille de marges unifiée (pad 24 / gutter 8) + échelle UI genome (hot-reload Shift+F12).
- D3. Passe cohérence FR des strings UI.

## Notes techniques

- bevy_egui 0.39.1 = egui 0.32 : `FontData::from_static` + `Arc`, `CornerRadius` (ex-Rounding), `StrokeKind`.
- Fonts embarquées au binaire (`include_bytes!` via `CARGO_MANIFEST_DIR`) — pas de call-site `asset_server.load()` (Lock L1 neutre).
- `egui::RichText::strong()` ne change PAS la graisse (pas de bold map) — Poppins-SemiBold réservé à un usage explicite futur.
- Multi-terminal 2026-06-11 : diff autre terminal = forgia-debug/forgia-game/forgia-observability → forgia-game interdit d'édition, theme branché via ForgiaUiHudPlugin.
