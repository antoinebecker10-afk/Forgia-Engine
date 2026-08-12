# Story-617 — Roguelite : nettoyage de l'écran de sélection (Lobby) « quick wins »

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `hud.rs`, symbole `RunState`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : CODE-COMPLETE (2026-06-24) — validation runtime à faire
> **Niveau BMAD** : Standard (`hud.rs` + `weapon_select.rs` + `forgia-crosshair` + Cargo)
> **Origine** : capture user + audit UI multi-lentilles (6 agents, 37 findings). Le Lobby
> « ne ressemblait pas à un roguelite pro ». Quick wins 1→3 du plan de refonte.

## Diagnostic central

**Tout le HUD de combat saignait au Lobby** : un seul système (`draw_wave_counter`,
`hud.rs:51-57`) appliquait le gate `RunState` ; tous les autres ne gatent que sur
`AppMode::InGame` → boons actifs (run précédente !), minimap, compteurs OR/ÂMES,
slots, portrait, crosshair restaient affichés sur l'écran de sélection.

## Ce qui est fait (quick wins)

1. **HUD combat masqué au Lobby** : run-condition `in_run_or_boss` (`hud.rs`) appliquée
   au groupe combat dans `RogueliteHudPlugin` (minimap, weapon_slots, portrait,
   shockwave, wave_counter, currency, active_boons, enemy_labels). Les overlays
   auto-gatés (Defeat/Victory/portail/reward/bark) restent dans un groupe séparé.
2. **Crosshair masqué au Lobby** : nouveau flag `forgia_crosshair::CrosshairHidden`
   (Resource), gate dans `draw_crosshair`/`draw_sniper_scope_overlay` ; togglé par
   `weapon_select` à `OnEnter/OnExit(RunState::Lobby)` (cross-crate sans cycle).
3. **Scrim plein écran** sombre (`Order::Background`, alpha 120) derrière le wizard
   → l'arène live recule, l'écran « se referme » sur la sélection (`weapon_select.rs`).
4. **Bug valeurs de stats invisibles corrigé** : `stat_row` affichait la valeur **sans
   couleur explicite** → texte egui par défaut (sombre) → invisible (seul DPS, coloré
   en or, sortait). Fix : `.color(C_TEXT_LIGHT)`. ⚠️ Les agents avaient diagnostiqué
   « largeur de colonne » — faux (le DPS, même grille, s'affichait).
5. **Flèches tofu** `←/→` → `‹/›` (glyphes présents dans Poppins ; les fontes custom
   n'ont pas U+2190/2192).
6. **Dédup Âmes** : retiré du titre du wizard (le compteur HUD tombe avec #1 ; l'Âmes
   reste dans l'Enclume = contexte d'achat).

## Critères d'acceptation

- [ ] Au Lobby : plus de panneau AMÉLIORATIONS, plus de minimap/portrait/slots, plus de crosshair.
- [ ] Le fond (arène) est assombri par un scrim ; les panneaux ressortent.
- [ ] Toutes les valeurs de stats s'affichent (pas seulement DPS).
- [ ] `‹ ›` au lieu de carrés vides.
- [ ] « Âmes » affiché une seule fois (Enclume).
- [ ] En run (InRun/Boss), tout le HUD combat réapparaît normalement.
- [x] `cargo check` + 143 tests + clippy clean (mes crates) + binaire `-j 4` OK.

## Test runtime

1. `cargo run -p forgia -j 4` → ROGUELITE RUN → **Lobby** : vérifier que seuls le titre,
   la carte d'arme (gauche), l'arme 3D (centre, scrim derrière), l'Enclume (droite) et
   le sélecteur (bas) sont visibles — **aucun crosshair, aucun panneau de boons, aucune minimap**.
2. Vérifier les **valeurs de stats** (DMG/Cadence/Chargeur/Recharge/Portée/Tête) toutes lisibles.
3. `ENTRÉE` → en combat : le HUD complet (crosshair, boons, minimap, OR/ÂMES) **revient**.

## Reste (suivi, non bloquant)

- **HP (100/100) + AMMO** (bas-gauche/bas-droite) viennent de `forgia-ui-lib` (cross-mode) —
  pas encore gatés au Lobby (besoin d'un flag équivalent côté forgia-ui-lib). ~15 min.
- Étapes 4-6 du plan (mise en scène 3D du modèle = socle + lumière ; hiérarchie ; juice).
- Coordination : `status_vfx.rs` / `forgia-effects` édités par un autre terminal en parallèle.
