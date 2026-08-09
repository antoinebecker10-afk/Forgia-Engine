# story-694 — Hub menu : architecture (P3 de l'audit 2026-08-07)

> Renumérotée 693 → 694 le 2026-08-09 : l'autre terminal avait déjà pris 693
> (viewmodel pixel-art Pépin). Les commentaires code « story-693 » de l'incrément 1
> (hub.rs, identity.rs, weapon_select.rs) désignent CETTE story.

**Statut** : IN_PROGRESS
**Niveau BMAD** : Enterprise (cross-crates, multi-incréments)
**Origine** : audit 2026-08-07, constats architecture n°1 (frontière inversée : forgia-ui
dépend de forgia-mode-roguelite), n°2 (god-file lib.rs > 2 800 l., 6 responsabilités),
n°3 (DEUX hubs coexistent — l'ancien hub Lobby se dessine chaque frame sous l'overlay
opaque, chrome dupliqué, enum HubTab à moitié morte), n°7 (ajouter une page = 6 sites),
UX n°5 (retours incohérents). Pratiques cibles : `reference_ui_menu_industry_practices`
(pile d'écrans CommonUI, vue-rend-action, registre de pages, ViewModel).

## Plan en 5 incréments (chacun compile + auto-QA + validation avant le suivant)

1. ✅ **Suppression du hub Lobby mort** — FAIT 2026-08-09 (attente validation en jeu).
   **−1 057 lignes / +194** : HubTab + 4 gates on_*_tab + chrome + draw_identity_panel +
   draw_meta_shop_lobby + draw_chapter_select + draw_weapon_select + toggle de visibilité
   + les 2 inputs FANTÔMES (flèches et touche U actifs sous l'overlay : changer d'arme ou
   ACHETER à son insu pendant le chargement). Gardé : hide/show du HUD, section_intro,
   draw_codex_section, contenu partagé, et TOUT le sous-système de pré-chauffe pipelines
   (LobbyPrewarmSphere, story-618 — non touché). hub.rs : 441 → 111 lignes.
   Preuves : trace_callers(draw_hub_chrome)=0 appelant · grep symboles supprimés = 0 hit
   hors commentaires · check 3 crates + clippy 0 + tests 493/494 (toon_config préexistant).
   Exécuté par l'implementer (1 oubli attrapé : champ `weapon` retiré mais site de spawn
   non mis à jour — corrigé) puis vérifié mécaniquement.
2. ✅ **Découpe mécanique de lib.rs** — FAIT 2026-08-09 (attente validation en jeu).
   **lib.rs 2 869 → 249 lignes** (en-tête + wiring ForgiaUiPlugin + prelude + tests).
   Modules livrés : `menu/{nav 302, chrome 208, cursor 164, lobby_gate 60, shell 484}`
   + `menu/pages/{root 500, forgeron 542, marketplace 305, armes 64, enclume 62,
   livre 53}` — `enclume` s'ajoute aux 5 prévus (sys_menu_enclume n'avait pas d'autre
   foyer). Zéro changement de comportement : items déplacés verbatim, visibilités
   privé → pub(crate) au besoin, re-export `pub(crate) use menu::nav::MenuPage`
   pour menu_hub_sensor/weapon_preview (seule référence racine des modules internes).
   Purges voulues : paused_overlay_ui (mort, #[allow(dead_code)], jamais schedulé —
   constat n°8) + bloc « Q en Paused » dupliqué inatteignable d'escape_handler
   (constat n°9). Preuves : check 4 crates (ui, game, viewmodel, rpg) · clippy
   --all-targets 0 warning · tests 15/15 · plus aucun #[allow(dead_code)] dans la
   crate · aucun module > 1 200 l. (AC2 tenu dès cet incrément pour lib.rs < 400).
3. ✅ **NavStack** — FAIT 2026-08-09 soir (attente auto-QA + validation en jeu).
   `NavStack(Vec<MenuPage>)` dans menu/nav.rs = LA vérité (sommet = page courante,
   fond = Root, invariant jamais-vide) ; `MenuPage` n'est PLUS une Resource — le
   derive retiré a fait pointer le compilateur sur chacun des 16 sites (aucun
   oubli possible). Sémantique : onglets (barre + LB/RB) = `switch_tab` [Root,tab] ;
   drill-ins (Personnaliser→Forgeron, LE LIVRE→Livre, fiche→Marketplace) = `push` ;
   Retours = `back()` pop — le Retour du Marketplace revient d'OÙ L'ON VIENT (AC3).
   ESC + B manette au menu = `back()` dans l'UNIQUE escape_handler (anti-trap V1
   tenu), garde `wants_keyboard_input` pour l'édition du nom. Capteur menu_hub :
   + `nav_depth`/`nav_path`. Preuves : 7 tests NavStack (22/22 crate), clippy 0.
4. ✅ **Registre de pages** — FAIT 2026-08-10 (attente auto-QA + validation en jeu).
   `menu/registry.rs` : table `PAGES: &[PageDecl { id, nav_label, section_title,
   in_nav, badge: fn(&HubBadges), draw: Inline|OwnSystem|Shell }]`. La barre, le
   cycle LB/RB, les titres ET le dispatch itèrent LA table ; l'ex-`MenuPage::NAV`
   + les 2 match de libellés sont morts ; les 5 pages inline (Codex/Talents/
   Missions/Succès/Stats) sont des fns du registre (textes/largeurs/panel_id
   repris à l'octet). **Preuve AC4** : page `PreuveAc4` ajoutée avec 1 variante +
   1 fn → compile ET apparaît dans la barre sans toucher barre/dispatch/badges/
   gamepad (puis retirée). Nuance honnête : la `PageDecl` manquante ne casse PAS
   la compile — c'est le test `chaque_variante_a_exactement_une_declaration` qui
   force l'enregistrement conscient (garde par test, pas par compilateur).
   Auto-QA : verifier 7/7 PASS (fidélité à l'octet libellés/titres/textes/ordre) ;
   qa-lead 1 Majeur corrigé — la perte d'exhaustivité compilateur est comblée par
   la garde const `_EXHAUSTIVITE` (match SANS wildcard à côté de l'enum) +
   `MenuPage::TOUTES` consommée par le test (plus de liste miroir manuelle) ;
   2 Mineurs (commentaire ArenaTest périmé, double lookup decl()) + 2 Cosmétiques
   (en-tête dupliqué, doc PageDraw::Shell) corrigés aussi.
   Preuves : 4 tests de cohérence du registre (28/28 crate), clippy 0.
5. **Crate forgia-menu-hub** (~2-4 j, High) — dépend de forgia-ui ET forgia-mode-roguelite ;
   forgia-ui redevient un shell neutre (MenuCamera2d, curseur/ESC, vidéo, point
   d'injection). Un 2ᵉ mode peut enfin avoir un menu.

## Critères d'acceptance (globaux)

- [ ] AC1 — Plus aucun système d'UI Lobby dessiné sous l'overlay (mesurable : aucun
      draw_hub_chrome/on_*_tab dans le schedule) ; le gate auto-start inchangé en jeu.
- [ ] AC2 — lib.rs < 400 lignes (wiring + modules) ; aucun module > 1 200 lignes.
- [ ] AC3 — Retour/ESC remontent d'un niveau sur TOUTES les pages (NavStack).
- [ ] AC4 — Ajouter une page de test = 1 entrée de registre + 1 fn (prouvé en le faisant).
- [ ] AC5 — forgia-ui ne dépend plus de forgia-mode-roguelite (Cargo.toml) ; forgia-rpg
      et forgia-viewmodel ne tirent plus les 28k LOC du roguelite via le shell.
- [ ] AC6 — À chaque incrément : check + clippy 0 + tests verts + validation en jeu.

## Hors scope

ViewModel/HubViewModel complet (suivra le registre) ; design tokens TOML ; modales en
pile ; option « taille UI ».
