# Tech Debt Plan — Forgia V2 Rewrite

**Date prévue d'exécution :** 2026-05-18
**Workspace :** `C:\Users\Antoi\Desktop\Forgia Rewrite\`
**Branche :** `main`
**Contexte :** dette accumulée sessions 2026-05-15/16 marathon + Tier 1 refacto + splice destructeur 2026-05-17

---

## Ordre d'exécution recommandé (par dépendance + ROI)

| Phase | Tâche | Effort | Risque | BMAD scale | Statut |
|---|---|---|---|---|---|
| **1** | Fix `forgia-rpg/src/character.rs` 38 erreurs LocomotionBoneCache | 30-60 min | Low | quick | ✅ DONE |
| **2** | Tests headless `fire_weapon_minimal` (semi/auto/pump/burst) | 45 min | Low | quick | ✅ DONE 2026-05-19 (6 tests dispatch + helper pur) |
| **3** | Implémenter `fire_mode = "burst"` (burst_count rafale) | 30 min | Low | quick | ✅ DONE (BurstState L62 + dispatch) |
| **4** | Hardcode hit-stop/hit-flash durations → genome | 30 min | Low | quick | ✅ DONE (hit_flash_duration/hit_stop_duration genome) |
| **5** | VFX colors per-arme (audit + dispatch) | 1h | Medium | quick | ⏳ TODO |
| **6** | Nettoyer `_suppress_unused` scope_glass.rs:137 | 5 min | Low | trivial | ✅ DONE (déjà absent du code) |
| **B1** | Weapon balance hardcoded → viewmodel_arena.toml | 30 min | Low | quick | ✅ DONE (Vague 2 forensic 2026-05-19) |
| **7** | (Optionnel) Tier 2A `forgia-weapon-hitscan` extraction | 1h30 | HIGH | standard | reuse bot IA / RPG |
| **8** | (Optionnel) Tier 2B `forgia-weapon-viewmodel` extraction | 1h | HIGH | standard | reuse |

**Total Phases 1-6 (dette pure) : ~3h30. Standard BMAD car ≥10 fichiers touchés. Story requise.**

---

## Phase 1 — forgia-rpg::character.rs (38 erreurs)

**Symptômes :**
- `error[E0425]`: cannot find value `LEFT_THIGH_NAMES`, `RIGHT_THIGH_NAMES`, `LEFT_ARM_NAMES`, `RIGHT_ARM_NAMES`, `SPINE_NAMES`, `TAIL_PREFIXES`
- `error[E0609]`: no field `left_thigh`, `right_thigh`, `left_arm`, `right_arm` on `LocomotionBoneCache`

**Cause probable :** WIP anim-layer Phase 1 (story-437/438/439 dans `D:/Forgia/docs/stories/`) — refacto Rex 3P RPG procedural walk cycle. Constantes + struct fields probablement extraits vers nouveau crate (`forgia-secondary-motion`, `forgia-camera-orbit`, `forgia-anim-debug`) sans propager les imports/fields dans character.rs.

**Étapes :**
1. Lire `crates/forgia-rpg/src/character.rs:280-340` (zone erreurs)
2. `grep -rn "LEFT_THIGH_NAMES" crates/` — localiser source actuelle
3. Soit ré-importer depuis crate extrait, soit re-déclarer en local
4. Pour `LocomotionBoneCache.left_thigh` etc. : vérifier `crates/forgia-secondary-motion/` ou `forgia-rpg/src/locomotion.rs` pour la struct
5. `rtk cargo check -p forgia-rpg` après chaque fix
6. `rtk cargo check --workspace` final = 0 errors

**Memory à consulter :** `session_2026_05_16_anim_layer_phase1_rex_3p.md` ([[session-2026-05-16-anim-layer-phase1-rex-3p]])

---

## Phase 2 — Tests headless fire_weapon_minimal

**Justification :** règle `feedback_headless_tests_reflex_2026_05_13` ([[feedback-headless-tests-reflex-2026-05-13]]) — tout système Bevy testable headless DOIT avoir tests AVANT playtest. fire system reconstruit 2026-05-17 sans tests = violation.

**Fichier :** `crates/forgia-fps/src/lib.rs` `mod tests`

**Tests à écrire :**

```rust
#[test]
fn left_mouse_state_default_idle() {
    let s = LeftMouseState::default();
    assert!(!s.held && !s.just_pressed);
}

#[test]
fn track_left_mouse_pressed_sets_both() { ... }

#[test]
fn track_left_mouse_released_clears_held_not_just_pressed() { ... }

#[test]
fn fire_dispatch_auto_uses_held() { ... }

#[test]
fn fire_dispatch_semi_uses_just_pressed() { ... }

#[test]
fn fire_dispatch_pump_multi_pellets_seed_reproducible() { ... }

#[test]
fn fire_dispatch_burst_warns_and_fallbacks_semi() { ... }
```

**Approche :** mini-App Bevy headless avec resources mockées (cf pattern memory `reference_bevy_on_enter_cross_plugin_race`). Insert `EquippedWeapons`, mock `ViewmodelGenomeHandle` avec entry custom, send `MouseButtonInput` via `App.world_mut().send_event(...)`.

**Cible :** 7 tests, exec < 50ms total.

---

## Phase 3 — fire_mode "burst" implémentation

**Spec :** `burst_count` tirs consécutifs au rythme `fire_rate` × N (e.g. N=3 à 20 shots/s = burst 150ms total), puis cooldown long. Pattern Halo BR, Apex Hemlok.

**Refacto fire_weapon_minimal :**

```rust
#[derive(Resource)]
struct BurstState {
    shots_remaining: u8,
    interval_timer: Timer,    // entre shots du burst
    cooldown_long: f32,        // cooldown après dernier shot du burst
}
```

Logic :
- Si `entry.fire_mode == "burst"` ET `left.just_pressed` ET `BurstState` absent → insert `BurstState { shots_remaining: entry.burst_count, ... }`
- Chaque frame avec `BurstState` présent : tick timer, si fini → fire 1 shot + decrement, si shots_remaining==0 → set cooldown long + remove BurstState
- ⚠ Pas de re-trigger pendant burst en cours

**Genome existant :** `burst_count` déjà dans `ViewmodelGenomeEntry` (line 134). Aucune arme V1 ne l'utilise (Pépin=semi, Bourrasque=auto, Lenoir=semi, Boucherie=pump). Tester via TOML hot-reload sur une arme.

---

## Phase 4 — Hardcode hit-stop/hit-flash → genome

**Hardcodes actuels (fire_weapon_minimal) :**
- `Timer::from_seconds(0.15, ..)` pour HitFlashTimer
- `Timer::from_seconds(0.05, ..)` pour HitStopState
- `virtual_time.set_relative_speed(0.05)` (5% speed)
- `restore_speed: 1.0`

**Genome fields à ajouter (`ViewmodelGenomeEntry`) :**

```rust
#[serde(default = "default_hit_flash_duration")]
pub hit_flash_duration: f32,  // default 0.15s

#[serde(default = "default_hit_stop_duration")]
pub hit_stop_duration: f32,   // default 0.05s

#[serde(default = "default_hit_stop_speed")]
pub hit_stop_speed: f32,      // default 0.05 (5%)
```

**Refacto :** lire entry → fallback defaults si missing. Tester sniper sensation = hit_stop 100ms vs SMG = 30ms.

---

## Phase 5 — VFX colors per-arme

**Audit avant code :** `grep -n "WeaponVfxEffects\|spawn_muzzle\|spawn_impact\|spawn_hitscan_tracer" crates/forgia-effects/src/`. Si la struct `WeaponVfxEffects` a déjà des per-weapon `EffectAsset` handles → wire dispatch. Sinon → étendre.

**Pattern V1 (memory à confirmer) :**
- ModernAR / SMG : muzzle blanc-jaune
- Sniper : long flash blanc + traînée fumée
- Shotgun : muzzle large rouge-orange
- Rocket : flash bleu

**Si reuse possible** : signature `spawn_muzzle_flash(commands, vfx, pos, dir, weapon_type)` → match WeaponType à l'intérieur. 

**Memory à consulter** : `session_2026_05_15_forgia_rewrite_fps_complete.md` (mention "Meshy LMG integration", "tracer convergent crosshair") + `reference_v2_viewmodel_genome_pattern.md`.

---

## Phase 6 — Cleanup _suppress_unused

**Fichier :** `crates/forgia-fps/src/scope_glass.rs:137`

```rust
// Suppression du marqueur `MaterialFaderCloned` (géré par forgia-mesh-fader internals).
#[allow(dead_code)]
fn _suppress_unused(_: MaterialFaderCloned) {}
```

**Action :** retirer la fn + l'import `MaterialFaderCloned` ligne 12 si plus utilisé. Vérifier compile.

---

## Phases 7-8 (optionnel) — Tier 2 fine-grained crates

**Tier 2A `forgia-weapon-hitscan`** :
- Extract de forgia-fps : `LeftMouseState` + `track_left_mouse_state` + `fire_weapon_minimal` + `pseudo_rand`
- API publique : `WeaponHitscanPlugin` + `WeaponFireRequest` event optionnel (pour bots IA + RPG combat)
- Effort 1h30, HIGH risk (cross-crate deps : forgia-combat, forgia-effects, forgia-player, forgia-genome-core)

**Tier 2B `forgia-weapon-viewmodel`** :
- Extract : `WeaponViewmodel` + `WeaponModelAssets` + `attach/update/auto_scale_viewmodel` + `viewmodel_debug.rs` + `ads.rs` + `scope_glass.rs` + helpers
- Effort 1h, HIGH risk
- Dep cross-crate : forgia-mesh-fader, forgia-crosshair, forgia-genome-core, forgia-player

**⚠ Décision séparée requise** : doivent venir APRÈS recovery du WIP fire system perdu (VS Code Timeline check user). Sinon on extrait du code partiel.

---

## Validation finale

Après Phase 1-6 :

```powershell
rtk cargo check --workspace                    # 0 errors
rtk cargo clippy --workspace --no-deps         # 0 warnings nouveaux
rtk cargo test -p forgia-fps                   # 7 tests fire pass
rtk cargo build --profile release-fast         # release vert
.\run_debug.bat                                # runtime test
```

**Smoke test runtime :**
- Digit1-4 switch arme + canon vers devant
- Pépin clic = 6 shots/s, 30 dmg
- Bourrasque maintenu = 16 shots/s spread 1.5°
- Lenoir clic = 1 shot/1.25s, 200 dmg, ADS scope fullscreen viewmodel hidden
- Boucherie clic = 1 shot/0.83s, 8 pellets cone, ~96 dmg max
- Hit-stop sniper > Hit-stop SMG (sensation différente)
- VFX muzzle taille/couleur varie par arme

---

## Decisions pendantes (à trancher au moment T)

1. **VS Code Timeline recovery** du WIP fire system 2026-05-16 PM (~840 LOC perdues : version riche avec calibration affinée). Si TROUVÉ : fusion avec ma reconstruction du 2026-05-17. Si NON trouvé : reconstruction OK, marquer Phases 7-8 comme "extraction depuis code reconstruit" (perte de fidélité acceptée).
2. **Story-437/438/439** anim-layer Phase 1 (Rex 3P RPG) : continue ou pause ? Phase 1 ci-dessus suppose qu'on stabilise character.rs MAIS ne wire pas les nouvelles features anim. À décider.
3. **fire_mode "burst"** : aucune arme V1 ne l'utilise. Implémenter quand même par hygiène, ou ajouter une arme test (Halo BR style) ?

---

## Memories à charger en début de session

- `session_2026_05_16_forgia_rewrite_4_weapons_refacto` — recap dette identifiée
- `reference_v2_fire_modes_genome_driven` — pattern dispatch + multi-pellets
- `feedback_headless_tests_reflex_2026_05_13` — règle tests obligatoires
- `feedback_v2_tech_debt_audit_protocol` — process audit dette
- `reference_rule_fine_grained_crates` — pour Tier 2A/B
- `reference_v2_237_crates_decision` — justification archi V2
- `session_2026_05_16_anim_layer_phase1_rex_3p` — pour Phase 1 character.rs
