# Story-599 — Réglages graphiques Tier 1 (VSync / MSAA / Tonemapping) dans le menu ESC

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia_pause_menu.json`, fichier `character.rs`, symbole `Camera3d`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : RÉ-IMPLANTATION PROGRESSIVE 2026-06-16 sur base stable (post story-600 qui a corrigé
> la vraie cause des écrans cassés = stage orphelin, PAS les réglages). Ordre incrémental validé un
> par un : **inc.1 Tonemapping ✅** → **inc.2 MSAA ✅** → **inc.3 VSync ✅** — TOUS validés runtime
> (sensor objectif : `fps_camera_msaa_actual:[8]`=réglage, `present_mode_actual:AutoVsync`=réglage,
> persistance OK). **Story-599 COMPLÈTE.**
> Chaque incrément : code + check/clippy + build + validation runtime AVANT le suivant.
>
> **⚠️ Leçon inc.2 (MSAA)** : appliquer MSAA à la caméra **orbitale 3P** (RPG/Cyber) la casse →
> écran marron (même fragilité que TAA/SMAA/HDR-Bloom story-550 ; isolation prouvée 2026-06-16).
> Fix : `apply_msaa_to_cameras` gaté `With<FpsCamera>` (PAS `Camera3d`). MSAA contrôlable en
> **Roguelite (mode ship)** ; en RPG/Cyber l'orbitale garde le défaut 4× (sûr). Le tonemapping
> (inc.1) est OK sur toutes les caméras (composant déjà présent, on change juste sa valeur).
>
> _(Note historique : 1ère tentative big-bang remisée le 2026-06-16 — code sain mais invalidable sur
> base instable. La base est stable depuis story-600.)_
> **Pourquoi** : implémentation complète et SAINE (check/clippy 0, 18 tests, tonemapping validé
> runtime « parfait » par Antoine), MAIS impossible à valider sur une base instable — la Roguelite
> tourne en boucle spawn/cleanup et la caméra gameplay disparaît par moments (« écran marron » =
> 0 Camera3d, prouvé par sensors `camera_msaa_actual:[]` + toon `attached_cameras:0` + skybox
> « re-attaché à 0 Camera3d »). Ce churn vient du cycle de vie Roguelite/stage/caméra
> (`forgia-mode-roguelite` + `forgia-stage` + `character.rs` Bloom/Hdr) en cours de modif par
> l'autre terminal — PAS de story-599 (qui ne mute que des composants, ne despawn rien).
> **À ré-appliquer tel quel** quand le rendu 3P/Roguelite est stable. Recette ci-dessous intacte.
> **Scale BMAD** : Standard (1 fichier principal `forgia-ui-lib/pause_menu.rs` + apply systems)
> **Origine** : discussion 2026-06-16 — le menu ESC → Options expose déjà sensibilité/FOV/volume/
> fenêtre/résolution (story-595 M2-B1). Ajouter les réglages **rendu** standards d'un menu PC,
> en se limitant à ce qui mappe sur des composants déjà câblés (pas de post-process qui casse la
> caméra 3P — cf story-598/550 : TAA/SMAA/Bloom exclus).

## Réglages ajoutés (Tier 1)

| Réglage | Hook moteur | Valeurs |
|---|---|---|
| **Anti-aliasing (MSAA)** | composant `Msaa` (bevy_render::view) sur chaque `Camera3d` | Off / 2× / 4× / 8× |
| **VSync** | `Window.present_mode` (`AutoVsync` ↔ `AutoNoVsync`) | On / Off |
| **Profil colorimétrique (tonemapping)** | composant `Tonemapping` (bevy_core_pipeline) sur chaque `Camera3d` | TonyMcMapface / ACES / AgX / Reinhard / Neutre |

(Cap FPS explicite = différé : nécessite un frame-limiter type `bevy_framepace`, pas câblé. VSync couvre le besoin principal.)

## Architecture (réutilise le pattern existant)

- 3 champs `UserSettings` avec `#[serde(default = ...)]` → **backward-compat** TOML legacy (test existant `legacy_settings_toml_parses_with_defaults`).
- 3 systèmes `apply_*` en `Update`, **idempotents** (set-if-different) → couvrent à la fois le changement de réglage ET les caméras spawnées plus tard (par-mode). Pas de gating `Changed` (le diff évite la boucle de change-detection, cf BUG-455-04).
- Sliders/sélecteurs dans `draw_settings` (même `dirty` → `set_changed()` que l'existant, anti-crash 2026-06-10).
- Sensor `forgia_pause_menu.json` enrichi (msaa/vsync/tonemapping).

## Sécurité / non-régression

- **MSAA seul est sûr** sur la caméra orbitale 3P (≠ TAA/SMAA/Bloom qui ajoutent prepass/nodes et la cassent — story-598). C'est le défaut Bevy qui marche déjà.
- **Tonemapping** : composant déjà présent par défaut (TonyMcMapface) sur toute `Camera3d` → on ne fait que changer la valeur de l'enum. `tonemapping_luts` actif (le défaut Tony rend sans erreur).
- `MenuCamera2d` (Camera2d) non ciblée (query `With<Camera3d>`).

## Critères d'acceptance

- [ ] check + clippy 0 warning sur forgia-ui-lib.
- [ ] In-game ESC → Options : 3 nouveaux contrôles, application LIVE.
- [ ] MSAA Off → crénelage visible ; 8× → arêtes lisses. Sans casser le rendu (≠ épisode TAA).
- [ ] VSync Off → FPS débridé (tearing possible) ; On → capé écran.
- [ ] Tonemapping change le rendu couleur global (Neutre = plat, ACES/AgX = filmique).
- [ ] Sauvegarde → relance conserve les réglages (%APPDATA%/Forgia/user_settings.toml).
- [ ] TOML legacy (sans les 3 champs) charge avec défauts.
