# Story-629 — Roguelite : capteur de charge combat `perf_diag` (vision complète freezes)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_element_vfx.json`, fichier `perf_diag.rs`, symbole `Roguelite`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : CODE-COMPLETE (2026-06-25)
> **Niveau BMAD** : Quick (1 module `perf_diag.rs` + 1 edit wiring)
> **Demande user** : « ajoute tous les capteurs nécessaires pour avoir une vision
> complète » (des freezes combat).

## Le trou comblé

Les capteurs existants donnent des morceaux mais **rien ne corrèle** :
- `forgia2_perf.json` : frame avg/min/max + FPS (fenêtre roulante, pas d'event).
- `forgia2_load_timing.json` : attribution **heuristique** (entity/collider delta →
  sinon `gpu_or_shader_compile` fourre-tout).
- `forgia2_element_vfx.json` : sparks d'élément seulement.

→ Impossible de dire **CE QUI était lourd quand ça a freezé**. C'est le manque que
comble `perf_diag`.

## `forgia2_perf_diag.json` — écrit chaque seconde

- **Timing** : `frame_avg_ms`, `frame_max_ms`, et **compte de spikes** par seuil
  (`spikes_30` / `spikes_45` / `spikes_60`) → combien de freezes dans la seconde + leur taille.
- **Breakdown de charge** (au même instant) : `total_entities`, `enemies`,
  `status_auras` (Hanabi DoT — suspect n°1), `element_sparks`, `particle_effects`
  (tous Hanabi), `point_lights` (coûteux), `pickups`, `meshes`, `colliders`.
- **Health check** : `severity=warn` dès qu'un spike >45 ms survient (ou stutter
  soutenu >2 spikes 30 ms/s) + `next_step` → la seconde fautive est pointée avec sa charge.

## Anneau de rétention (correctif v1→v2, 2026-06-25)

**Défaut v1 trouvé sur le 1er post-mortem** : le sensor **écrasait** le fichier chaque
seconde → il ne montrait QUE la dernière seconde. Si le freeze arrivait à t=805 et la
dernière écriture à t=879 (calme), le breakdown de la seconde fautive était **perdu**.
`load_timing` gardait le timing mais pas le breakdown (auras/particules/lumières).

**Correctif** : `perf_diag` retient désormais un **anneau** (`freezes[]`, les 8 dernières
secondes en `warn`) avec leur charge complète — même idiome que `load_timing` (`recent`,
RING=40). Lecture post-hoc fiable : `forgia2_perf_diag.json::freezes[]` survit aux secondes
calmes. Champ `freeze_count` = nombre de freezes retenus.

## Usage (capture pendant le symptôme)

1. Lancer une run, provoquer le freeze (grosse vague + feu/poison).
2. **Le jeu peut être fermé après** : l'anneau `freezes[]` retient les secondes fautives.
3. Lire `forgia2_perf_diag.json::freezes[]` : pour chaque seconde freezée, comparer
   `max_ms` (60-130 ms) avec `status_auras`/`particle_effects`/`point_lights` →
   **confirme la charge VFX particules**. Si `enemies` haut sans VFX → charge IA/physique.
   Si rien de lourd → croiser avec `load_timing` (delta entités → `scene_spawn_gltf`).

## 1er post-mortem (2026-06-25, t=805)

2 freezes capturés, **groupés sur un seul évènement de spawn** : `scene_spawn_gltf`
(+797 entités en 1 frame = 71 ms) puis +20 colliders Rapier (55 ms, 0.14 s après).
= **freeze de streaming GLB**, PAS de combat-VFX. Seconde calme suivante (t=879) :
146 FPS, 13 460 entités, 0 particule. La grosse vague feu/poison n'a pas reproduit le
symptôme cette session → capture combat à refaire avec l'anneau v2.

## Coût

Accumulation par-frame triviale (delta + 3 comparaisons) ; les `count()` ne tournent
qu'**1×/seconde** (négligeable même à 13k entités). Gaté `GameSet::Sensors` + `Roguelite`.

## Suivi

- Quand la cause est confirmée + corrigée : `load_timing` (diagnostic temporaire de
  l'autre terminal) et éventuellement `perf_diag` pourront être allégés/retirés.
- Per-system timing (quel système précis) non inclus (invasif) — le breakdown de
  charge suffit en général à pointer la catégorie fautive.
