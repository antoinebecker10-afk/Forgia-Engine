# Story-595 — M2 session 2 : Options plancher Steam (volume, affichage, touches, %APPDATA%)

> **Source** : [roadmap post-audit](../ROADMAP_POST_AUDIT_2026-06-10.md) jalon M2,
> items B1 (settings) + B6 partiel (FR pause menu).
> **Audit 2026-06-10 P1-15** : « le mot volume n'existe nulle part dans le workspace ;
> refund/review négative quasi garantie sur Steam ».
> **Scale BMAD** : Standard. **Date** : 2026-06-10. **Statut** : CODE-COMPLETE — validation runtime requise.

## Critères d'acceptance

| # | AC | Statut | Preuve |
|---|---|---|---|
| AC1 | Volume principal réglable (0-100 %), appliqué en LIVE à tout l'audio | ✅ code | `forgia_audio::UserMasterVolume` + `sys_apply_user_master_volume` au niveau CANAL kira (main+sfx+music+ambient) — compose avec les `with_volume` par-son et le master genome. Slider Options → sync event-driven |
| AC2 | Mode d'affichage réglable (Fenêtré / Plein écran sans bordure) + résolution (4 presets 16:9) | ✅ code | `apply_window_settings` mute `Window` live. **Défauts = miroir pré-595 (fenêtré 1920×1080) — zéro régression** |
| AC3 | Touches affichées dans Options (lecture seule) | ✅ | Section dépliable « Touches (AZERTY) » — réassignation = post-ship, annoncée comme telle |
| AC4 | Settings persistés dans %APPDATA%\Forgia\ (plus jamais assets/) | ✅ code | `settings_path()` + create_dir_all + **migration** : l'ancien assets/user_settings.toml est lu au boot si le canonique n'existe pas |
| AC5 | TOML legacy (pré-595) charge avec défauts | ✅ | test `legacy_settings_toml_parses_with_defaults` + roundtrip + chemin APPDATA — 11 tests verts |
| AC6 | Pause menu en FR (B6 partiel) | ✅ | PAUSE / Reprendre / Options / Quitter vers le menu / Sauvegarder / Retour |
| AC7 | Sensor étendu | ✅ | forgia_pause_menu.json += master_volume + window_mode |

## Architecture (décision notable)

Les marker types `SfxChannel`/`MusicChannel` ont été **remontés de forgia-mode-roguelite
vers forgia-audio** (foundation) : le volume user s'applique aux canaux sans dépendance
inverse ni édition des systèmes de lecture. Le diff dans la crate claimée multi-terminal
se limite à 5 lignes (re-export `pub use forgia_audio::{MusicChannel, SfxChannel}` dans
audio.rs — fichier hors du diff de l'autre terminal, baseline verte avant édition,
110/110 tests verts après). Le volume user (settings) et le master genome
(roguelite_audio.toml, mix design-time) restent deux étages distincts qui se multiplient.

## Test in-game (récap obligatoire)

1. **Action** : rebuild (`cargo run --profile release-fast`) → lancer une run → ÉCHAP →
   Options.
2. **Effets attendus** : (a) slider « Volume principal » baisse/coupe musique ET tirs
   EN DIRECT ; (b) « Plein écran (sans bordure) » bascule immédiatement, « Fenêtré » +
   preset change la taille ; (c) menu entièrement en français ; (d) Sauvegarder → log
   `settings sauvés → C:\Users\…\AppData\Roaming\Forgia\user_settings.toml` ;
   (e) relancer le jeu → réglages conservés.
3. **Sensors** : `forgia_pause_menu.json` → `master_volume` + `window_mode` reflètent
   les sliders ; `last_save_success:true` après Sauvegarder.
4. **Variantes si KO** : volume sans effet sur la musique → vérifier que la musique
   tourne sur MusicChannel (log kira) ; fenêtre ne bascule pas → tester l'autre mode
   (Bevy/winit peut exiger un focus) ; settings non conservés → vérifier la création
   du dossier %APPDATA%\Forgia.

## Reports

- Réassignation des touches : M4 (affichage seul ici, honnêtement annoncé).
- FR des HUD roguelite (hud.rs) : **claimé multi-terminal** — passe FR à la levée du claim.
- bevy_egui 0.34.3 (veille du jour : fixes WGPU) : upgrade = story dédiée post-claim
  (touche le lock de version workspace).

## Fichiers touchés

crates/forgia-audio/src/lib.rs (canaux + UserMasterVolume + amp_to_db pub + 4 tests) ·
crates/forgia-mode-roguelite/src/audio.rs (re-export 5 lignes) ·
crates/forgia-ui-lib/src/pause_menu.rs (Options FR + volume + fenêtre + touches + APPDATA + 3 tests) ·
crates/forgia-ui-lib/Cargo.toml (dep forgia-audio)
