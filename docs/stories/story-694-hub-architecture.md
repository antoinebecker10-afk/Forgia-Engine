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
2. **Découpe mécanique de lib.rs** (~1-2 j, Low) — modules `menu/{nav,chrome,cursor,
   lobby_gate,shell}` + `menu/pages/{root,forgeron,marketplace,armes,livre}` dans la
   MÊME crate, zéro changement de comportement, re-exports pour les consommateurs
   (menu_hub_sensor, weapon_preview). Purge du code mort au passage (paused_overlay_ui,
   bloc ESC dupliqué — constats mineurs n°8/9).
3. **NavStack** (~½ j, Medium) — `Vec<MenuPage>` : le retour devient DÉRIVÉ (pop),
   ESC/B remontent d'un niveau partout, le Retour du Marketplace cesse de téléporter.
4. **Registre de pages** (~1 j, Medium) — `PageDecl { id, label, titre, in_nav, badge,
   draw }` : nav, badges et hints itèrent LA table ; ajouter une page = 1 déclaration.
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
