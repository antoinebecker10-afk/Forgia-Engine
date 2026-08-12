# Story-614 — Wizard : aperçu d'arme 3D tournant + UI plein écran centrée

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `meta_shop.rs`, symbole `sys_calibrate_merchant`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : CODE-COMPLETE (2026-06-23) — validation runtime + tuning visuel à faire
> **Niveau BMAD** : Standard (`weapon_select.rs` + 1 edit `meta_shop.rs`)
> **Demande user** : « fais en sorte que l'on voie les armes en 3D tourner dans la
> sélection et que l'UI soit centré en plein écran ».
> Prérequis : [story-612](story-612-roguelite-weapon-select-wizard.md), [story-613](story-613-roguelite-weapon-unlocks-evolutif.md).

## Ce qui change

1. **UI plein écran centrée** : le wizard passe d'un panneau ancré à droite à un
   layout type *character-select* en 3 zones egui (`ws_top` / `ws_left` / `ws_bottom`) :
   - HAUT = titre « CHOISIS TON ARME » + Âmes ;
   - GAUCHE = carte de stats (DMG/DPS/élément/matchup/verrou) ;
   - BAS = sélecteur des 4 armes + contrôles (`←→`, `[U]`, ENTRÉE) ;
   - CENTRE = laissé libre pour l'arme 3D.
   - L'**Enclume** (`meta_shop.rs`) déplacée `CENTER_CENTER → RIGHT_CENTER` pour
     occuper la colonne droite sans recouvrir le centre.

2. **Arme 3D tournante** : le GLB `models/weapons/forgia/{key}.glb` est **parenté
   à la caméra 3D active** (toujours visible, comme un viewmodel), placé devant
   (`PREVIEW_DIST`), **auto-calibré en taille via AABB** (miroir `sys_calibrate_merchant`,
   GLB de taille native inconnue), et **tourne** (`sys_spin_lobby_preview`).
   Swap à chaque changement de sélection ; despawn `OnExit(RunState::Lobby)`.

## Architecture (concept-first)

- Producteur : `StartingWeaponChoice` (sélection) → `sys_lobby_weapon_preview`
  spawn/swap parenté caméra ; `NeedsPreviewCalibrate` → `sys_calibrate_preview`
  (scale = target/max_dim) ; `sys_spin_lobby_preview` (rotation). `PreviewState`
  Resource track l'entité/arme affichée (anti-respawn/frame).
- Robustesse : pas de render-to-texture (évite le piège RenderLayers-non-propagés
  des enfants de scène) ; l'arme est sur le layer 0, vue par la caméra principale.
  Échelle initiale 0.001 (pas de flash géant avant calibrage). Retry si caméra
  pas encore prête. Fallback : si rien ne s'affiche, l'UI reste fonctionnelle.

## Tunables (réglage à l'œil, je ne vois pas le rendu)

`PREVIEW_DIST=1.4` (m devant cam) · `PREVIEW_Y=-0.05` · `PREVIEW_TARGET=0.55`
(plus grande dim, m) · `PREVIEW_SPIN=0.9` (rad/s). Dans `weapon_select.rs`.

## Critères d'acceptation

- [ ] Au Lobby : titre en haut, stats à gauche, Enclume à droite, contrôles en bas, **centre libre**.
- [ ] Une arme 3D **tourne au centre** ; `←/→` la changent (swap du GLB).
- [ ] Taille raisonnable (auto-calibrage AABB), pas de flash géant.
- [ ] Disparaît quand la run démarre (`OnExit(Lobby)`).
- [x] `cargo check` + clippy 0 warning + 139 tests + binaire `-j 4` OK.

## Risques / suivi

- ⚠️ **Framing/échelle** non vérifiables sans rendu → peut nécessiter 1 passe de
  tuning des consts. Log `[weapon-select] aperçu 3D : <key> (cam=...)` confirme
  spawn + caméra trouvée.
- Si le viewmodel FPS s'affiche aussi au Lobby → 2 armes (à cacher, suivi).
- **Hotspot** `weapon_select.rs` (15 edits) → extraire `weapon_preview.rs` (module 3D).
- Increment 2 progression = gating paliers de boons (story-613 suivi).
