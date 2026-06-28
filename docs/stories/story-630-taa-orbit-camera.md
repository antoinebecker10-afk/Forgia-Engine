# Story-598 — Anti-aliasing sur la caméra orbitale 3P (RPG + CyberCity)

> **Statut** : ABANDONNÉE 2026-06-16 — tout AA ajouté (TAA puis SMAA, avec `Msaa::Off`)
> sur `RpgOrbitCamera` CASSE le rendu de la scène (géométrie absente, seul le skybox
> s'affiche / écran noir). Revert complet → RPG + CyberCity re-fonctionnent (validé user).
> **Aucun changement conservé.** Cette caméra est fragile aux composants de post-process /
> prepass : même classe que le revert HDR+Bloom (story-550, commit `7d8d1dc`).

## Conclusion (honnête)

- **Cause réelle** : `Msaa::Off` + `TemporalAntiAliasing` (ou `Smaa`) posés sur `RpgOrbitCamera`
  cassent la passe de rendu de CETTE caméra → plus de géométrie. TAA → skybox seul (gradient) ;
  SMAA → noir. Le **default MSAA** (sans rien) rend correctement.
- **Fausse piste écartée** : j'avais soupçonné l'anti-clip raycast de `forgia-camera-orbit`
  (`orbit_follow`, lib.rs:314). FAUX — l'anti-clip est présent dans le binaire qui FONCTIONNE.
  Hypothèse haute-confiance mais erronée ; le test-first (revert plutôt que fix spéculatif) a évité
  d'éditer à tort du code commité d'un autre terminal.
- **Reste à comprendre** (si on veut un jour de l'AA en 3P) : pourquoi cette caméra casse avec
  tout composant post-process/prepass. Piste : conflit avec `OutlineSettings`/`ToonSettings`
  appliqués aux Camera3d (`forgia-mode-roguelite/toon_config.rs`, `forgia-postprocess`), ou setup
  render de la caméra. Chantier séparé, NON prioritaire (l'AA n'est pas un ship-blocker).
- **Décision** : pas d'AA sur la caméra orbitale 3P. Le MSAA par défaut reste en place.

---
_Historique de la tentative ci-dessous (conservé pour ne pas refaire l'erreur)._

## BUG-598-01 — TAA blanchit la passe principale (2026-06-16)

`TemporalAntiAliasing` posé au spawn de `RpgOrbitCamera` → **écran nu** (ciel/gradient
rendus, **zéro géométrie**), screenshot user. Ses prepass `DepthPrepass` + `MotionVectorPrepass`
auto-insérés cassent la passe opaque sur cette caméra — **même classe de bug** que le `Hdr`+`Bloom`
retiré de `cyber_city.rs:220` (cette caméra a un setup de rendu fragile aux nodes de render-graph
ajoutés). Donc : **pas seulement le post-hoc**, le spawn-time aussi.

**Pivot = SMAA** : post-process morphologique pur, **aucun prepass / motion vector** → ne touche pas
le render-graph de la même façon, ne peut pas blanchir la passe. Plus faible que TAA sur le
scintillement spéculaire temporel, mais net, fiable, et améliore les arêtes. Décision cohérente
`no-speculative-fix` : on revient au fallback documenté plutôt que de creuser le render-graph Bevy.

> **Suite possible** si on veut vraiment TAA (néons cyber) : investiguer pourquoi le prepass blanchit
> (compat matériaux GLB/terrain en prepass ? `Camera.hdr` requis ? ordre des nodes ?) — chantier
> séparé, non bloquant pour shipper du AA correct.
> **Scale BMAD** : Quick→Standard (2 fichiers : forgia-rpg/character.rs + éventuellement Cargo.toml feature)
> **Origine** : discussion AA 2026-06-16 — la Cyber City (néons/spéculaire) et le RPG (feuillage) sont
> les cas où le MSAA est inutile et le TAA brille. Best-practices vérifiées (Bevy PR #7291, docs.rs
> bevy_anti_alias, AMD RCAS PR #7422, guides AA 2026).

## Décision

Anti-aliasing **par-caméra** : `Msaa` est un composant par-caméra dans Bevy 0.18, donc on peut
activer TAA sur la **caméra orbitale 3P** (`RpgOrbitCamera`, partagée RPG + CyberCity via
`spawn_rex_character`) **sans toucher** la caméra FPS du Roguelite (qui garde son MSAA — net pour
les armes 1P, cf. discussion : TAA floute, mauvais pour la visée rapprochée).

## Pourquoi au spawn et pas en post-hoc

`cyber_city.rs:220` documente que `Hdr`+`Bloom` insérés **après coup** sur la caméra orbitale
cassaient la passe principale (écran ClearColor nu). TAA est plus invasif (2 prepass auto-insérés
+ jitter) → posé **au spawn** dans `character.rs`, pas via un système post-hoc.

## Implémentation

`forgia-rpg/src/character.rs`, spawn `RpgOrbitCamera` (~l.161) :

- `Msaa::Off` (TAA + MSAA mutuellement exclusifs — sinon warn Bevy + no-op TAA)
- `TemporalAntiAliasing::default()` — auto-insère `TemporalJitter` + `MipBias` + `DepthPrepass` + `MotionVectorPrepass` (via `#[require(...)]`)
- `ContrastAdaptiveSharpening { sharpening_strength: 0.4, denoise: true, .. }` — RCAS récupère le flou TAA (best-practice : 30-50 %).

Plugin : `AntiAliasPlugin` est ajouté par `DefaultPlugins` **si feature `bevy_anti_alias`** (tirée par
`3d_bevy_render`). Vérifié à la compilation — si `bevy::anti_alias` introuvable, ajouter
`"bevy_anti_alias"` aux features bevy du Cargo.toml racine.

## Notes d'implémentation (2026-06-16)

- `bevy::anti_alias` **résout sans ajout de feature** : `bevy_anti_alias` est tirée par `3d_bevy_render` (déjà actif, jeu 3D) → `AntiAliasPlugin` est dans `DefaultPlugins`. Pas de modif Cargo.toml.
- Piège chemin : `Msaa` n'est PAS dans `bevy::camera` ni le prelude — il vit dans `bevy::render::view::Msaa` (le crate TAA l'importe via `view::Msaa`). `bevy::camera::Msaa` = erreur E0432.
- CAS défaut Bevy = `sharpening_strength: 0.6` (un peu fort) → fixé à **0.4** + `denoise: true` (best-practice RCAS+TAA 30-50 %).

## Critères d'acceptance

- [x] check + clippy 0 warning sur forgia-rpg.
- [ ] Runtime : entrer en RPG ou CyberCity → arêtes/néons lissés, **moins de scintillement** vs MSAA.
- [ ] Caméra FPS Roguelite **inchangée** (toujours MSAA, armes nettes).
- [ ] Pas de régression « écran nu » (la passe principale rend bien — le risque du post-hoc Bloom).
- [ ] Ghosting Rex acceptable en mouvement (skinned motion vectors).

## Variantes si KO (runtime)

- **Écran nu / passe cassée** → le spawn-time ne suffit pas ; fallback SMAA (`Smaa::default()`, pas de prepass) au lieu de TAA.
- **Ghosting fort sur Rex** → baisser l'historique TAA ou désactiver TAA en RPG, garder en CyberCity (gate `GameMode`).
- **Trop flou** → monter `sharpening_strength` (0.5-0.6) ; **trop de halos** → descendre (0.3).
- **Néons ternes** : hors-scope — réintroduire `Hdr`+`Bloom` AU SPAWN (séparé, déjà retiré une fois car posé post-hoc).

## Références

- [Bevy PR #7291 TAA](https://github.com/bevyengine/bevy/pull/7291) · [docs.rs TemporalAntiAliasing](https://docs.rs/bevy_anti_alias/latest/bevy_anti_alias/taa/struct.TemporalAntiAliasing.html) · [Bevy PR #7422 RCAS](https://github.com/bevyengine/bevy/pull/7422)
