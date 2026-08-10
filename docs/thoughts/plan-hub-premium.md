# Plan: Hub Premium — le menu devient un vrai hub de roguelite (egui)

Date: 2026-08-05
Story: story-678 (ID réservé via `xtask story-ids`, fichier story à créer au lancement de l'implémentation)

## Objectif

Transformer le menu (11 pages egui, fonctionnel mais « classeur d'onglets ») en hub de
roguelite au standard indie premium (cible Hades/Dead Cells, pas Fortnite) : le hub
reçoit la mort, vend la prochaine run, et répond au joueur (son, motion, manette).
On reste 100 % egui — aucune migration de framework (décision actée ce jour).

## Fichiers autorisés (SCOPE LOCK)

- `docs/stories/story-678-hub-premium.md` — la story (à créer, statut READY)
- `docs/stories/_index.md` — enregistrement story
- `crates/forgia-ui/src/lib.rs` — menu : accueil, transitions, pastilles, focus manette
- `crates/forgia-ui/src/avatar_backdrop.rs` — NOUVEAU module : fond RTT vivant (pattern `weapon_preview.rs`)
- `crates/forgia-ui/src/weapon_preview.rs` — lecture pattern RTT uniquement ; retouche minime si factorisation caméra RTT
- `crates/forgia-ui-lib/src/style.rs` — micro-anims boutons (scale-punch), style focus manette
- `crates/forgia-audio/src/lib.rs` — `UiSfxEvent` + lecture SFX UI (canal SFX existant)
- `crates/forgia-mode-roguelite/src/meta_shop.rs` — `LastRunSummary` + flags « vu » dans `MetaShopSave` (`#[serde(default)]`, version-safe)
- `crates/forgia-mode-roguelite/src/waves.rs` — UNIQUEMENT l'écriture du `LastRunSummary` en fin de run (aucune logique gameplay)
- `crates/forgia-observability/src/lib.rs` — enregistrement capteur `forgia2_menu_hub.json` au registre
- `assets/audio/roguelite/sfx/ui_*.ogg` — NOUVEAUX (~6 sons, pipeline audio propriétaire validé le 2026-08-05)
- `assets/genomes/roguelite_ui.toml` — NOUVEAU genome (toggles + volumes + durées exposées)

**Tout fichier non listé = INTERDIT.** En particulier : `hud.rs` (HUD in-game hors périmètre),
`boons_apply.rs`, `rounds.rs`, tout le gameplay.

## Phases

> Chaque phase est **shippable et validable manette en main indépendamment**
> (`feedback_valider_chaque_feature_en_jeu` : une phase non validée en jeu bloque la suivante).
> `rtk cargo check -p <crate>` après chaque fichier, clippy vrai-cargo en fin de phase.

### Phase 1 : Le menu répond — son d'UI (3 fichiers + 6 assets, ~1-2 j)

- [ ] Produire ~6 SFX UI via le pipeline audio propriétaire : `ui_hover`, `ui_click`,
      `ui_tab`, `ui_buy`, `ui_unlock`, `ui_denied` (48 kHz, loudnorm, conventions du pack)
- [ ] `forgia-audio/lib.rs` : `UiSfxEvent` (Message) + système de lecture sur le canal SFX,
      volume à l'INSTANCE (jamais au canal — `reference_kira_channel_volume`), gate `AppMode::Menu` non requis (réutilisable in-game plus tard)
- [ ] `forgia-ui/lib.rs` : émission sur interactions — hover en **front montant**
      (une fois par entrée de widget, sinon spam 60 Hz), clic, changement d'onglet
- [ ] `roguelite_ui.toml` : `ui_audio_enabled`, `ui_sfx_volume` (bornés, hot-reload)
- build/check après

### Phase 2 : Le menu bouge — transitions & micro-anims (2 fichiers, ~1-2 j)

- [ ] `forgia-ui/lib.rs` : transition de page fade+slide (~150 ms, `ctx.animate_value_with_time`),
      aucune page ne s'affiche plus « sec »
- [ ] `forgia-ui-lib/style.rs` : scale-punch au clic sur `cartoon_btn`/`glass_btn`
- [ ] Durées = consts nommées (exception UI cosmétique de `no-hardcode`) ; toggle
      `ui_motion_enabled` dans `roguelite_ui.toml` (accessibilité)
- build/check après

### Phase 3 : L'accueil vend la prochaine run (4 fichiers, ~2 j)

- [ ] `meta_shop.rs` : `LastRunSummary { chapter, rounds_reached, death_cause, souls_earned, record_beaten }`
      dans `MetaShopSave` (`#[serde(default)]` — les saves existantes restent valides)
- [ ] `waves.rs` : écrire le summary aux DEUX sorties de run (mort + victoire) —
      une seule définition de la grandeur (`feedback_une_grandeur_ecrite_deux_fois`)
- [ ] `forgia-ui/lib.rs` : refonte `draw_root_landing` → carte CHAPITRE EN COURS
      (n°, titre, menace ×N, record), bandeau DERNIÈRE RUN, gros bouton « FORGER — Chapitre N »,
      header persistant Âmes ◇ + Puissance sur toutes les pages
- build/check après

### Phase 4 : Les pastilles — économie d'attention (2 fichiers, ~1 j)

- [ ] `meta_shop.rs` : flags « vu » (`seen_equipment`, `seen_chapters`) + helper
      « achat Enclume abordable avec les Âmes courantes »
- [ ] `forgia-ui/lib.rs` : pastille dorée sur les onglets Enclume (abordable),
      Forgeron (pièce non vue), Livre (chapitre fraîchement débloqué)
- build/check après

### Phase 5 : Le fond vivant (2 fichiers, ~2-3 j)

- [ ] `forgia-ui/avatar_backdrop.rs` (NOUVEAU) : RTT plein écran derrière les panels —
      avatar équipé en pied + éléments de DA de l'univers du chapitre sélectionné,
      caméra en dérive lente. **RTT obligatoire, jamais de scène 3D directe au menu**
      (`reference_menu_hub_is_not_the_lobby`), layers dédiés, `RenderLayers` propagé
      aux enfants GLB (`reference_rtt_3d_preview_in_egui_bevy018`)
- [ ] Résolution RTT bornée + gene `ui_backdrop_enabled` (coût GPU au menu maîtrisé)
- build/check après

### Phase 6 : La manette (2 fichiers, ~3-5 j — le plus gros)

- [ ] `forgia-ui/lib.rs` : système de focus — ordre de focus par page, surbrillance
      dorée visible, A = activer, B = retour, LB/RB = onglet précédent/suivant,
      barre de hints boutons en pied d'écran (souris toujours fonctionnelle)
- [ ] `forgia-ui-lib/style.rs` : style de l'anneau de focus
- [ ] Bascule auto souris ↔ manette sur dernière entrée détectée
- build/check après

### Transverse (fin de chantier)

- [ ] Capteur `forgia2_menu_hub.json` : page courante, visites cumulées par page (`*_session`,
      front montant — `reference_capteur_instantane_ne_valide_rien`), sfx joués, focus manette
      actif, présence du LastRunSummary. Health : « MENU MUET » (0 sfx alors que `ui_audio_enabled`),
      « BACKDROP MORT » (0 frame RTT alors qu'activé)
- [ ] Enregistrement au registre observability + allowlist asset-load si le gate le demande
- [ ] `xtask story-gate --story 678` avant tout DONE

## Acceptance Criteria

- [ ] AC1 : chaque interaction menu (survol, clic, onglet, achat, refus) joue un SFX distinct ; volume/toggle genome, hot-reload
- [ ] AC2 : plus aucun changement de page « sec » — fade+slide ~150 ms, désactivable (accessibilité)
- [ ] AC3 : l'accueil affiche chapitre en cours + avatar + bandeau dernière run (mort OU victoire) + header Âmes/Puissance persistant ; « FORGER » lance le chapitre sélectionné en 1 clic
- [ ] AC4 : pastilles Enclume/Forgeron/Livre pilotées par l'état réel (abordable / non-vu / débloqué)
- [ ] AC5 : fond RTT vivant derrière le menu, thème = univers du chapitre sélectionné, désactivable
- [ ] AC6 : menu 100 % navigable à la manette, focus toujours visible, hints en pied, souris intacte
- [ ] AC7 : `forgia2_menu_hub.json` écrit + 2 health checks avec next-step
- [ ] AC8 : 0 warning clippy workspace, tests verts, saves méta antérieures chargées sans migration manuelle, story-gate vert

## Risques

- **Concurrence avec le cycle de validation en cours** (courbe de puissance, run à valider)
  → la story naît `READY`, l'implémentation ne démarre **qu'après** la run de validation ; aucune des deux boucles ne partage de fichiers chauds (`waves.rs` = seul point de contact, Phase 3 — vérifier `git diff` avant)
- **Spam du hover-sfx** (immediate mode = re-render 60 Hz) → front montant par `egui::Id` mémorisé, testé au capteur (`sfx_played_session` doit rester ~1 par interaction)
- **Coût GPU du backdrop RTT au menu** → résolution bornée par gene + toggle ; mesurer via `forgia2_perf.json` avant/après
- **Cycle de deps forgia-ui ↔ forgia-audio** → `UiSfxEvent` défini côté `forgia-audio`, `forgia-ui` ne fait qu'émettre ; si le sens des deps l'interdit, event dans une crate déjà partagée (vérification au 1er fichier de la Phase 1)
- **Save méta corrompue par le nouveau champ** → `#[serde(default)]` + test de chargement d'une save pré-678 (AC8)
- **Régression sur les 11 pages existantes** → chaque phase validée manette en main avant la suivante ; la Phase 6 (focus) est la seule qui touche toutes les pages → en dernier, exprès

## Estimation

~10-15 jours au total, mais **chaque phase livre seule** : après P1+P2 (2-4 j) le menu
« répond » déjà ; P3 (2 j) donne la boucle mort→hub→repart ; P5-P6 peuvent glisser
sans bloquer un playtest. Ordre optimisé rendement/effort décroissant.
