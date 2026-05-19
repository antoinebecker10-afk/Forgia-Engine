# Refacto Viewmodel — Tier 2B (2026-05-19)

**Date** : 2026-05-19
**Scope** : Extraction du viewmodel 1P de `forgia-fps` vers `forgia-viewmodel`
**BMAD scale** : Enterprise (10+ fichiers touchés cross-crate)
**Verdict** : Tier 2A (firing path) **skip** — c'est de l'orchestrator, pas de la dette.

---

## Pourquoi

`forgia-fps` faisait 1630 LOC, mélangeait :
- Firing path (orchestrator : juice + ammo + VFX + raycast + camera)
- Viewmodel render (mesh attach, ADS pose, scope glass, auto-scale AABB)
- Genome layer (ViewmodelGenome data + lookup)
- FPS tuning sync (fps_tuning.toml → downstream Resources)

Coupling fort, navigation pénible, et `forgia-viewmodel` était un scaffold vide
malgré la règle fondatrice fine-grained-crates (`reference_rule_fine_grained_crates.md`).

## Architecture cible — Source SDK + Bevy officiel

Sources vérifiées :
- **Valve SDK 2013** ([developer.valvesoftware.com/wiki/Viewmodel](https://developer.valvesoftware.com/wiki/Viewmodel)) — `CBaseViewModel` = entité client-only séparée de `CBaseCombatWeapon`. Firing = server-authoritative, viewmodel = client-prediction.
- **Bevy official example** `first-person-view-model` ([bevy.org/examples/camera/first-person-view-model/](https://bevy.org/examples/camera/first-person-view-model/)) — dual-camera + RenderLayers pour isoler le viewmodel.
- Doom Eternal / Valorant non documentés publiquement sur ce point précis (cf brief recherche 2026-05-19).

| Module `forgia-viewmodel` | Responsabilité | Équivalent Source |
|---|---|---|
| `genome.rs` | Data layer TOML hot-reloadable (`viewmodel_arena.toml`) | `weapon_script.txt` |
| `calibration.rs` | Helpers purs : `viewmodel_transform`, `viewmodel_rotation_*`, `viewmodel_target_size`, `viewmodel_fallback_scale` | — (helpers internes) |
| `attach.rs` | Spawn/swap/auto-scale viewmodel enfant de `FpsCamera`, `CameraShake` insertion, despawn OnExit(Fps) | `CBaseViewModel::SetWeaponModel` |
| `pose.rs` | ADS state, lerp FOV/transform/rotation/speed/sensitivity | `cl_ads*` family |
| `fade.rs` | Semi-transparence ADS via `forgia-mesh-fader` (scope glass + body fader) | custom Forgia |

`ForgiaViewmodelPlugin` compose les 3 sous-plugins (attach + pose + fade) + add
`MeshFaderPlugin` idempotent + `load_viewmodel_genome` Startup.

## Ce qui reste dans `forgia-fps`

Le firing path est **orchestrator-level** — il combine 13 SystemParams cross-crate
(ammo, juice shake/recoil/fov, VFX muzzle/impact/tracer, raycast rapier, camera,
genome, hit feedback). Le sortir vers `forgia-weapon-hitscan` aurait écrasé le
crate générique existant (Component-based `Hitscan` + `TryFire` + `HitscanFired`)
qui sert un autre usage (bots IA futurs).

Resté dans `forgia-fps` :
- `LeftMouseState`, `BurstState`, `dispatch_fire_trigger` (logique pure testée)
- `FpsTuning` + sub-structs + `FpsTuningHandle` + `load_fps_tuning` + `sync_fps_tuning`
- `FireTimingCtx`, `JuiceWriters`, `HitApplyCtx`, `HitscanCtx`, `falloff_multiplier`
- `fire_weapon_minimal` + helpers (`pseudo_rand`, `find_health_ancestor`, `despawn_dead_cubes`)
- `weapon_select_system`, `track_left_mouse_state`
- `HitscanSensorState` + sensor module
- `ArenaScore`, `ammo_systems` modules

Décision : extraction `forgia-weapon-hitscan` (Tier 2A) **NON faite** — le crate
homonyme est déjà occupé par une API générique pour bots/RPG futurs.

## Consommateurs externes

- `forgia-viewmodel-calibration:24` → import changé `forgia_fps::WeaponViewmodel` → `forgia_viewmodel::WeaponViewmodel`, Cargo.toml `forgia-fps` → `forgia-viewmodel`.
- `forgia-fps:36` → import groupé `use forgia_viewmodel::{AdsState, AdsTuning, ForgiaViewmodelPlugin, ViewmodelGenomeCtx, ViewmodelGenomeEntry}` pour `fire_weapon_minimal` consumer.
- `forgia-fps/src/ammo_systems.rs:19` → import groupé `use forgia_viewmodel::{lookup_genome_entry, weapon_genome_key, ViewmodelGenome, ViewmodelGenomeEntry, ViewmodelGenomeHandle}`.

## Résultats

| Métrique | Avant | Après |
|---|---|---|
| `forgia-fps/src/lib.rs` LOC | 1630 | ~870 |
| `forgia-fps/src/` fichiers | lib + ads + scope_glass + ammo_systems + hitscan_sensor + score | lib + ammo_systems + hitscan_sensor + score (ads.rs et scope_glass.rs supprimés) |
| `forgia-viewmodel` LOC | 16 (scaffold) | ~870 (5 modules) |
| Tests `forgia-viewmodel` | 0 | 11 (calibration + fade + genome) |
| Tests `forgia-fps` | 17 | 16 (viewmodel_genome_defaults_are_safe migré) |
| Workspace `cargo check` | clean | clean |
| `clippy -D warnings` 3 crates touchées | clean | clean |
| Cycles compilation après changement viewmodel | tout `forgia-fps` rebuild | seul `forgia-viewmodel` rebuild → moins de churn |

## Limitations

- Genome `ViewmodelGenome` contient encore tous les champs gameplay (damage, fire_rate, range, pellets, falloff, juice). C'est volontaire : **une seule source de vérité TOML par arme**. Si un consommateur balance-only (bots IA) arrive, on splittera vers `forgia-weapon-genome` séparé. Pas anticipé pour éviter YAGNI.
- `AdsTuning` est exporté par `forgia-viewmodel` mais peuplé par `forgia-fps::sync_fps_tuning` (lit `fps_tuning.toml`). Couplage de lecture inversé acceptable car `AdsTuning` est la donnée brute, `sync_fps_tuning` est le pipe.
- Pas de RenderLayers V2 (le pattern Bevy officiel dual-camera) — feature optionnelle, pas dette. Si on en a besoin pour FOV mismatch viewmodel/world, on ajoutera dans `attach.rs`.

## Tier 2A — décision finale

**Skip définitif**, voire faire l'inverse : si un jour bots IA / RPG ont besoin de
hitscan, on ÉTEND `forgia-weapon-hitscan` existant (Component-based générique)
plutôt que d'extraire `fire_weapon_minimal` qui est intrinsèquement orchestrator.
