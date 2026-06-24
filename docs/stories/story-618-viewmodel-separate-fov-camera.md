# Story-618 — FOV viewmodel séparé (2e caméra + RenderLayers) + placement bras réglable

**Statut** : CODE COMPLETE — en attente validation runtime (risque render : surveiller crash/écran marron)
**Niveau BMAD** : Standard (forgia-player + forgia-viewmodel + forgia-fps + roguelite toon)
**Créée** : 2026-06-24
**Origine** : suite story-617 — les bras/arme se déforment au FOV joueur (120° → « gros tubes »,
mains qui ne tiennent pas l'arme). Décision user : FOV viewmodel séparé + placement réglable.

---

## Problème

Le viewmodel (arme + bras) est rendu par la **caméra monde** (FpsCamera), donc au FOV joueur
(jusqu'à 120°). À FOV élevé, tout ce qui est près de la caméra est énormément étiré/grossi →
l'arme et surtout les avant-bras paraissent géants et déformés. C'est le problème FPS classique
« world FOV ≠ viewmodel FOV ».

## Solution (pattern Bevy officiel `first-person-view-model`)

- **2e caméra `ViewmodelCamera`** enfant de FpsCamera, FOV fixe (~68°) indépendant du monde,
  `order: 1`, `ClearColorConfig::None` (composite par-dessus), `RenderLayers::layer(1)`.
- Le **viewmodel (arme + bras) passe sur `RenderLayers::layer(1)`** (root + descendants GLB/primitives
  propagés) → invisible pour la caméra monde (layer 0), rendu uniquement par la caméra viewmodel.
- **Exclusions** (sinon double-rendu / crash) :
  - toon (`sys_apply_toon_settings` & co) → `Without<ViewmodelCamera>` (évite le crash render-graph
    documenté + le weapon reste rendu, non cel-shadé pour l'instant).
  - skybox (`attach_skybox_to_camera`) → `Without<ViewmodelCamera>` (sinon skybox peint l'écran).
- **Tunable + rollback** : `[viewmodel_fov] enabled + fov_deg` (hot-reload). `enabled=false` →
  despawn caméra + retire les RenderLayers (retour au comportement pré-618). Placement global des
  bras déjà réglable via `[viewmodel_arms]` (offset/scale).

## Marqueur

`ViewmodelCamera` vit dans **forgia-player** (dép commune viewmodel + roguelite, zéro cycle), à côté
de `FpsCamera`/`CameraFov`.

---

## Acceptance Criteria

- [ ] AC1 — Au FOV joueur 120°, l'arme + les bras ne sont plus déformés/géants (rendus à ~68°).
- [ ] AC2 — Le monde reste au FOV joueur (slider inchangé) ; seul le viewmodel a son FOV propre.
- [ ] AC3 — L'arme ne clippe jamais dans les murs (caméra viewmodel = depth séparé, dessine au-dessus).
- [ ] AC4 — Aucun crash render / écran marron (toon + skybox exclus de la caméra viewmodel).
- [ ] AC5 — `[viewmodel_fov] enabled=false` (hot-reload) revient proprement à l'état pré-618.
- [ ] AC6 — `[viewmodel_arms]` (offset/scale) repositionne les mains sur l'arme (hot-reload).
- [ ] AC7 — `cargo check`/`clippy` vert, 0 warning.

---

## Caveats v1 (notés)

- Le **muzzle flash / tracers** restent en espace monde (layer 0, FOV monde) → léger décalage vs
  le bout du canon (rendu FOV viewmodel). À porter sur le layer viewmodel en suivi si gênant.
- L'arme n'est **pas cel-shadée** sur la caméra viewmodel (toon exclu) → léger écart de style.
  Suivi possible : ajouter le toon à la caméra viewmodel (à valider vs historique crash).
