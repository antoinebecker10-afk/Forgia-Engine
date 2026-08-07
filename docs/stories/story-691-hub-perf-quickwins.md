# story-691 — Hub menu : quick-wins perf (P1 de l'audit 2026-08-07)

**Statut** : ✅ DONE 2026-08-08 — validé en jeu par Antoine (« ça m'a l'air bien » +
capteurs concordants : `current_frame` figé sur 5 s sous le diorama, 6 pages
naviguées sans trou noir), commité, `xtask story-gate --story 691` PASS.

## Auto-QA (2026-08-08)

- **verifier** : VALIDÉ — check 0 erreur, clippy 0 warning (vrai cargo), 15/15
  tests, 0 `std::fs::write` restant, `is_showing()` source unique, systèmes
  dans les bons schedules, chaîne warmup→gate cohérente.
- **qa-lead** : GO-avec-réserves — 1 Majeur CORRIGÉ dans la foulée : le
  hot-toggle `ui_backdrop_enabled` était inerte à chaud (le chemin `!enabled`
  ne démontait jamais le diorama, `props_spawned` restait > 0) → démontage à
  chaud + caméra coupée + reconstruction à la réactivation
  (`arena_backdrop.rs`, `sys_rebuild_backdrop_on_change`).
- Réserves restantes ASSUMÉES (dette documentée, pas de correction ici) :
  - 🟡 `sys_propagate_preview_layers`/`sys_calibrate_previews` tournent encore
    chaque frame sur l'Accueil (coût CPU) → même chantier que le désarmement
    des BFS, déjà au « Hors scope » (P1bis).
  - 🔵 Clic souris d'onglet vu par le gate caméra avec 1 frame de retard
    (egui mute MenuPage en PostUpdate, le gate lit en Update) — imperceptible.
  - 🔵 Ordre `menu_video_tick` / `sys_rebuild_backdrop_on_change` non contraint
    dans Update — effet borné à 1 frame au boot du menu, auto-cicatrisant.
**Niveau BMAD** : Standard (5 fichiers, 1 crate + capteurs)
**Origine** : audit complet du hub (2026-08-07, `reference_audit_hub_menu_2026_08_07`) —
constats perf n°1, n°2, n°6, n°7 + qualité n°9, n°11.

## Problème

Le menu paie en permanence trois postes invisibles :

1. Le pipeline vidéo **décode ~24 WebP 1280×720/s et uploade ~88 Mo/s** de textures
   pendant que le diorama d'arène OPAQUE le recouvre (cas nominal). Choix historique
   documenté (hot-toggle sans re-preroll) devenu un coût permanent pour un besoin rare.
2. Les **2 caméras RTT 512²** (arme, personnage) rendent chaque frame des images que
   seules 2 pages sur 13 affichent — la page Accueil, la plus fréquentée, les paie pour rien.
3. Capteurs incohérents : `menu_video` écrit en `std::fs::write` **synchrone** sur le thread
   de jeu alors que `sensor_io::enqueue` existe ; `menu_hub` écrit à 1 Hz **même in-game**.

S'y ajoutent deux dettes 2-lignes : `sys_rotate_previews` sur `Time` virtuel (anti-trap
CLAUDE.md §6 « UI/menu = Real ») et les triggers LB/RB absents de `WATCHED` (naviguer aux
bumpers ne bascule pas les hints manette).

## Critères d'acceptance

- [x] AC1 — Diorama affiché ⇒ la vidéo ne décode plus : `forgia2_menu_video.json` montre
      `current_frame` FIGÉ pendant que le fond d'arène est visible ; couper
      `ui_backdrop_enabled` à chaud rend la vidéo immédiatement (cache conservé —
      exige le fix QA du démontage à chaud, inclus).
- [x] AC2 — Page Accueil ⇒ les caméras arme/personnage sont inactives (`is_active=false`) ;
      ouvrir Armes / Forgeron réactive la bonne caméra, l'aperçu s'affiche sans trou noir
      (grâce de warmup au boot du menu pour compiler les pipelines).
- [x] AC3 — Plus aucun `std::fs::write` dans forgia-ui (grep = 0) ; capteur hub à 1 s au
      menu / 30 s in-game avec écriture immédiate sur transition.
- [x] AC4 — `sys_rotate_previews` sur `Time<Real>` ; LB/RB dans `WATCHED` (naviguer aux
      bumpers passe `last_input` à `gamepad`).
- [x] AC5 — `cargo check` + clippy 0 warning + tests verts (hors échec préexistant
      `toon_config` hors scope) ; validé en jeu par Antoine le 2026-08-08, preuve
      capteur : `forgia2_menu_video.json` frame=1/cache=0 stable 5 s sous 17 props.

## Hors scope (reste au plan de l'audit)

Désarmement des BFS RenderLayers, allocations Marketplace/tooltips (P1bis) ;
suppression du hub Lobby mort (P3) ; responsive (P2).
