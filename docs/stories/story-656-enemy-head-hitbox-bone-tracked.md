# Story-656 — Hitbox tête ennemis : suivie de l'os + capsules recalibrées sur les meshs

> Renumérotée 652→656 (2026-07-02) : collision avec `story-652-vfx-visibles-kill-burst` de
> l'autre terminal. Le code référence encore « story-652 » dans ses commentaires (non
> réécrits pour ne pas toucher `waves.rs`, fichier chaud multi-terminal).

**Statut** : IN_PROGRESS
**Niveau BMAD** : Standard (2 fichiers modifiés + 1 module neuf + 1 TOML, 1 crate)
**Crate** : `forgia-mode-roguelite`
**Date** : 2026-07-02

## Problème (diagnostic prouvé)

Antoine : « quand je tire dans la tête, parfois ça ne fait pas de dégâts ». Diagnostic :

1. **Headshots géométriquement impossibles** : le head proxy (`waves.rs:147-150`) est une
   sphère à `0.85 × half_height`, rayon `0.55 × radius` → strictement À L'INTÉRIEUR de la
   capsule body (vertical ET latéral, par construction). Le hitscan est un `cast_ray`
   premier-hit (`forgia-fps/lib.rs:953`) → la surface de la capsule est TOUJOURS touchée
   avant la sphère interne. Preuve empirique : `forgia2_combat.json` — 105 tirs, **0
   `hit_zone_head`**. Le commentaire story-517 (« sphère détectée AVANT capsule ») est faux.
2. **Capsules ≠ meshs visuels** (mesures AABB bind-pose des GLB KayKit) :
   - Skull partagé par les 3 rigs : centre y=1.735, bas 1.31, haut 2.17, demi-étendue 0.456 ;
     joint `head` à y=1.241 (modèle).
   - Runner : crâne visuel jusqu'à 2.17 m vs capsule 1.84 m → ~33 cm de tête SANS collider.
   - Boss : capsule 7.2 m vs crâne à 5.4 m → 1.8 m de vide « touchable » au-dessus de la tête,
     proxy tête flottant au-dessus du crâne visible.
3. **Proxy statique vs mesh animé** (story-636) : les anims déplacent la tête, le collider non.

## Fix (approche industrie : hitbox attachée à l'os)

- **`head_hitbox.rs` (nouveau module)** : constantes mesurées (bind pose KayKit, script
  `measure_glb_aabb.py`/`head_mesh_aabb.py` 2026-07-02), placement pur testable,
  `HeadProxy` component, `sys_bind_head_joints` (résout l'entité du joint `head` du rig,
  pattern `Without<Bound>` de story-636), `sys_track_head_proxies` (sphère recollée chaque
  frame sur le crâne animé : `joint_GT.transform_point(0.494·Y)` → parent-local),
  capteur `forgia2_head_hitbox.json`.
- **Capsules body recalibrées aux épaules** (bas du crâne + chevauchement anti-trou-du-cou,
  `H = 1.40 × scale`) — defaults `enemies.rs` + `roguelite_enemies.toml` :
  - tank hh 0.75→0.43 · runner 0.60→0.38 · sniper 0.85→0.47 · boss 2.2→0.35 (rayons inchangés)
- **Sphère tête dimensionnée sur le crâne mesuré** : rayon auto `0.456 × skeleton_scale`
  (override TOML `head_radius` par archétype, optionnel) ; position fallback pré-binding =
  `1.735 × scale` au-dessus des pieds. La sphère DÉPASSE de la capsule (haut + latéral) →
  premier-hit du raycast la touche naturellement, zéro changement dans forgia-fps.
- **NameplateAnchor** : haut du crâne + marge (avant : haut de capsule + 0.6, qui plaçait le
  nameplate DANS le crâne du Runner et 2 m au-dessus du Boss). Story-644 intent préservé.

## Hors scope (documenté)

- `forgia-mode-fps-arena/wave.rs` : pattern DIFFÉRENT (hull convexe mesh-exact par
  `AsyncSceneCollider` story-453 v2 + tête data-driven par personnage via genome). Pas le
  même défaut ; non touché.
- Multiplicateurs de dégâts tête (`head_damage_mul`, `headshot_bonus_mul`) : inchangés —
  ils deviennent simplement atteignables.

## Acceptance criteria

- [x] Test pur : pour les 4 archétypes (defaults), la sphère tête dépasse de la capsule
      ET la chevauche (pas de trou au cou) — `head_sphere_pokes_out_of_capsule_no_neck_gap`,
      `skull_top_covered_by_head_sphere`.
- [x] Test pur : override `head_radius` (Option) + severity du capteur.
- [ ] Runtime : `forgia2_combat.json` montre des `hit_zone_head` en visant les crânes.
- [ ] Capteur `forgia2_head_hitbox.json` : `bound == proxies` en vague active.
- [x] `cargo check` vert + clippy 0 warning sur les fichiers touchés + 253 tests verts
      (`cargo test -p forgia-mode-roguelite`).

## Note QA (2026-07-02)

Auto-QA sub-agents (qa-lead/verifier) bloqués par la session limit (reset 1h) — le
qa-lead a tourné (20 tool uses) mais son rapport n'a pas pu être récupéré. Passe de
self-review inline faite à la place : origines de tir bots (`shoulder_y` relatif au
centre parent abaissé) vérifiées plausibles pour les 4 archétypes ; barres défensives
(story-644) suivent le nouveau NameplateAnchor (au-dessus du crâne réel) ; hot-reload
spawn_live intact ; fps-arena non touché. À re-passer en QA complète si doute.

## Fichiers

- `crates/forgia-mode-roguelite/src/head_hitbox.rs` (nouveau)
- `crates/forgia-mode-roguelite/src/enemies.rs` (head_radius Option + capsules recalibrées)
- `crates/forgia-mode-roguelite/src/waves.rs` (spawn proxy + nameplate anchor)
- `crates/forgia-mode-roguelite/src/lib.rs` (module + plugin)
- `assets/genomes/roguelite/roguelite_enemies.toml` (capsules + doc head_radius)
