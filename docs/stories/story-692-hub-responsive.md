# story-692 — Hub menu : responsive (P2 de l'audit 2026-08-07)

**Statut** : ✅ DONE 2026-08-08 — validé en jeu par Antoine (« c'est parfait ») après
un stress-test au drag de fenêtre : `hauteur utile = 1080 points` à toutes les tailles
standard, 8 reconstructions du backdrop suivant chaque aspect (1.25 → 5.65 !), boot
direct borderless, settings lus du chemin canonique. Inclut 2 retouches post-validation
demandées en jeu : titre FORGIA sans wrap + centré sur l'axe de la carte
(`ROOT_CARD_INNER_W`/`ROOT_CARD_MARGIN_X` partagées), et nav aux largeurs MESURÉES
(galley réel au lieu de `CHAR_W=9.0`) + wrap impossible + espace traînant retiré.

## Auto-QA (2026-08-08)

- **verifier** : VALIDÉ — check/clippy/tests verts sur les 4 crates, sources
  uniques tenues (`ui_scale_for`, `window_aspect`, aspect dérivé de l'image),
  même lecture disque pour le boot et la fenêtre initiale, débounce correct.
- **qa-lead** : GO-avec-réserves, zéro Bloquant/Majeur. Contre-vérifié dans les
  SOURCES de bevy_egui 0.39.1 : `pixels_per_point = scale_OS × scale_factor` —
  la toile fait bien ~1080 points à 1080p/100 %, 1080p/125 %, 720p. Grep : le
  HUD in-game ne lit `window.height()` nulle part (points egui purs partout),
  `forgia_viewport_h` n'a qu'un producteur et un consommateur, pas de fuite
  d'image RTT au resize, pas de race resize/rebuild (même .chain()).
- Constats assumés (pas de correctif ici) :
  - 🟡 `user_settings.toml` CORROMPU (pas absent) → défauts → borderless même si
    le joueur avait « windowed » : comportement pré-existant dont l'effet change
    avec le nouveau défaut. Candidate story de suivi (`.corrupt.bak` + toast).
  - 🔵 Sous ~702 px de haut (hors presets), le clamp 0.65 cesse de garantir la
    toile 1080 — compromis lisibilité documenté dans le code.
  - 🔵 1 frame de latence entre écriture du scale et application bevy_egui —
    perceptible uniquement pendant un drag de resize, couvert par le débounce.
**Niveau BMAD** : Standard (6 fichiers, 4 crates + génome)
**Origine** : audit 2026-08-07, constats responsive n°1 (aucune échelle globale — la nav
~1330 px déborde sous ~1550 px de large logique, y compris au preset 720p offert et à
Windows 125 %), n°2 (fenêtre par défaut Windowed, pas le borderless cible, clampée à
1009 px par l'OS), n°3 (backdrop RTT 16:9 figé : étiré +31 % en 21:9, flou ×2 en 1440p).

## Approche

1. **UN facteur d'échelle global** (`EguiContextSettings.scale_factor = hauteur/1080`,
   clampé) : le design 1080p existant devient une toile logique constante — tous les px
   absolus du hub ET du HUD in-game (même contexte egui) redeviennent corrects à toute
   résolution, sans toucher aux layouts. `sys_publish_viewport_h` publie désormais des
   POINTS egui (hauteur/échelle) pour que les plafonds de scroll suivent.
2. **Fenêtre initiale depuis les settings** : `user_settings.toml` est lu AVANT la
   création de la fenêtre (helper partagé avec `load_user_settings_at_boot`) — plus de
   flash fenêtré→borderless au boot, et le défaut passe à `borderless` (la cible
   officielle ; un TOML existant garde son choix).
3. **Backdrop à l'aspect réel** : l'image RTT et la caméra dérivent leur aspect de la
   fenêtre au spawn, et se reconstruisent sur resize (débouncé : taille stable N frames).
   Clamp du gène `ui_backdrop_height_px` monté à 1440 (le 1440p n'est plus un upscale forcé).

## Critères d'acceptance

- [x] AC1 — À 1280×720 (preset des Options) : la nav tient à l'écran, plus aucun
      chevauchement des chips ; idem à Windows 125 %. À 1080p : rendu inchangé (échelle 1.0).
      Preuve log : `hauteur utile = 1080 points` constant aux tailles standard.
- [x] AC2 — Le jeu démarre directement en borderless (sans flash fenêtré) quand les
      settings le demandent ou n'existent pas ; un TOML `windowed` explicite est respecté.
      Preuve log : settings chargés + backdrop spawné à l'aspect fenêtre dès la frame 1.
- [x] AC3 — Le backdrop n'est plus étiré : aspect image == aspect fenêtre, y compris
      après un resize en cours de session. Preuve log : 8 reconstructions suivant les
      aspects du stress-test (1.90, 1.33, 2.09, 1.52, 1.25, 5.65…), hauteur budget 720.
- [x] AC4 — `cargo check` + clippy 0 warning + tests verts ; auto-QA verifier VALIDÉ +
      qa-lead GO-avec-réserves ; validé en jeu par Antoine le 2026-08-08.

## Hors scope

Repère unique de l'Accueil pour l'ultrawide 21:9 (constat n°4 — exige d'ancrer panneau
équipement et personnage dans le même repère) ; option joueur « taille de l'UI »
(slider ×0.8-1.5 sur la même échelle) ; portrait Forgeron 512²→280×340 (constat n°8).
