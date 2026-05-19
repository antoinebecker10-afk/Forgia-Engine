# Forgia V2 Rewrite — ROADMAP_CURRENT

> **Source de vérité unique** pour l'état des vagues V2 et la priorisation BMAD.
> Mise à jour à chaque livraison story ou à la commande "Memorise" (CLAUDE.md §11).
>
> **Dernière révision** : 2026-05-19 fin session audit forensic + Vague 5 Session A (8 commits livrés cette session).
> **HEAD courant** : `67c20855f`.

---

## 🌊 Vagues — état canonique

Plan original : `docs/audit/audit-2026-05-19.md` §7. Cette table est le statut **vivant**.

### V1 — Débloquer (P0) ✅ DONE

| Item | Statut | Livré par |
|---|---|---|
| Fix `LocomotionBoneCache` fields | ✅ | session 2026-05-18 (résolu avant ma session) |
| `forgia_combat.json` producer | ✅ | session 2026-05-19 (story-457 commit `50444ba41`) |
| `forgia_health.json` producer | ✅ | session 2026-05-19 (story-457 commit `50444ba41`) |

### V2 — Discipline & traçabilité (P1) ⚠️ 75 %

| Item | Effort | Statut | Livré par / reste à faire |
|---|---|---|---|
| ARCHITECTURE.md actualisé | 1h | ✅ | session 2026-05-19 (commit `1b3301b37`) |
| Sensor fusion Tier 1 (`forgia2_combat` + `forgia2_arena`) | 2h | ✅ | story-465 (commit `aae934198`) — file-based aggregator 5+2 sources |
| Code mort `WeaponData` supprimé | 30 min | ✅ | commit `1b3301b37` (Vague 2 hardcode → confirmé code mort) |
| Story-458 concept-mapping doc | 30 min | ⏳ | **B2 — à faire** (`locomotion-bone-cache` ligne §6 concept-first.md) |

**Note** : "Migration weapon balance → genome TOML" du plan original a été RÉSOLU par suppression du code mort `WeaponData` (audit 0 call-site externe), pas par migration. Cohérent avec `.claude/rules/no-speculative-fix.md`.

### V3 — Modernisation Bevy 0.18 (P1) ✅ DONE (avec SKIPs documentés)

| Item | Statut | Livré par |
|---|---|---|
| Required Components Player/TargetCube/NameplateRoot | ✅ | story-461 (commit `9b74b08bb`) |
| Wave bots ChildOf relationships | ✅ | story-463 (commit `fb26eeb89`) |
| Observers death/pickup/damage | ✅ partial | story-466 (DeathEvent only — DamageEvent + CombatHitEvent SKIP justifié 8 consumers cascade) |
| RpgOrbitCamera vs PanCamera first-party | ✅ FALSIFIÉ | audit Vague 3 — FreeCamera/PanCamera gameplay n'existent pas en 0.18.1 |

Audit doc : `docs/audit/vague-3-bevy-018-idioms-2026-05-19.md` (commit `6d1836308` + correction `ca5c3b99a`).

### V4 — Tech debt P1-P2 ✅ DONE

| Item | Statut | Livré par |
|---|---|---|
| Fix tests melee TimePlugin advance_by trap | ✅ | commit `50444ba41` (helper `app_with_manual_time()`) |
| Fix test weapons cycle off-by-one | ✅ | commit `50444ba41` (`cycles_full` + `ARENA_V1_WEAPONS.len()`) |
| `tech-debt-plan-2026-05-18.md` obsolète à 80 % | ✅ | (à archiver — Phases 2/3/4/6 déjà DONE silently) |

### V5 — Phase 5 sensors complet (P2) ⚠️ Session A DONE, B+C pending

Plan Phase 5a livré : `docs/audit/vague-5-sensors-fusion-plan-2026-05-19.md` (commit `9c486bd86`).

**Cible révisée : 13 sensors `forgia2_*.json` canoniques** (vs 12 initial — séparation `health` cross-mode + `rpg_health` détail RPG valuable).

#### Session A (Tier 0 renames + Tier 1 fusion + xtask gate) ✅ DONE

| Sensor | Statut | Commit |
|---|---|---|
| `forgia2_health.json` | ✅ 149B, conforme | `380aa2f10` rename |
| `forgia2_rpg_health.json` | ✅ 1.3K, format fix | `380aa2f10` rename + `67c20855f` add `id` + rename `overall_severity` → `severity` |
| `forgia2_arena.json` | ✅ 410B, fusion 2 sources | `aae934198` story-465 (avant ma session) |
| `forgia2_combat.json` | ✅ 7.2K, fusion 5 sources | `aae934198` story-465 (avant ma session) |
| `xtask verify-sensors-format` | ✅ validates 4/4 | `380aa2f10` + `67c20855f` |

`cargo run -p xtask -- verify-sensors-format` → **OK (4/4 canonical sensors validated)**.

#### Session B (Tier 2 aggregators perf/entities/memory) ⏸️ Pending

Effort estimé ~6h. Pré-requis :
- Research `bevy::diagnostics::FrameTimeDiagnosticsPlugin` API Bevy 0.18
- Valider présence `MemoryBreakdown` Resource (grep workspace)
- Bench `Query<Entity>::iter().count()` perf sur RPG world 10k entities

Cibles : `forgia2_perf.json`, `forgia2_entities.json`, `forgia2_memory.json`.

#### Session C (lifecycle/watchdog/audio/input + cleanup) ⏸️ Pending

Effort estimé ~6h. Pré-requis :
- Confirmer syntax Bevy 0.18 `OnAdd<C>` / `OnRemove<C>` hooks (context7)
- Design Resource `GameTickCounter` pour watchdog heartbeat
- Cleanup final `default_expected_sensors` → 13 forgia2_*

Cibles : `forgia2_lifecycle.json`, `forgia2_watchdog.json`, `forgia2_audio.json`, `forgia2_input.json`, `forgia2_sensor_health.json`.

### V6 — Crates extraction (P2) ❌ Bloqué

Tier 2A/B : `forgia-weapon-hitscan`, `forgia-weapon-viewmodel`. Bloqué par recovery WIP fire system perdu 2026-05-17. À reprendre post-ship V1 ou si user remonte récup WIP.

---

## 🚀 Hors plan vagues — historique commits session 2026-05-19

### Session précédente (avant ma session)

| Story | Type | Commit |
|---|---|---|
| story-464 LOS state gating (bot AI) | feat(ai) | `20fefe9d7` |
| Nameplate permanent + face-cam + cartoon | feat(ui) | `1a7ce3eff` |
| 3 fixes audit qa-lead (BUG-464-01/02/03) | fix(audit) | `9d2baeaae` |
| story-465 sensor fusion Tier 1 | feat(observability) | `aae934198` |
| story-466 DeathEvent → Observer | refactor(ecs) | `f3bd4fdf3` |
| SESSION_STATE.md snapshot | docs | `51c084925` |

### Ma session 2026-05-19 (15 commits)

| Commit | Description |
|---|---|
| `50444ba41` | feat(audit+sensors): Vague 1+4 — sensors combat+health + tests fixes |
| `eb3c732b0` | feat(arena): story-448+449+453 — colliders + auto-calibrate + reset |
| `17634a5d4` | feat(terrain+rig+rpg): wave 5 LOD2 + auto-rig + Rex 3P |
| `d16ead641` | docs(stories): story-452 + 453-rpg-monitor docs orphelins |
| `dc740e133` | wip(hit-feedback): story-456 scaffold — forgia-enemy-nameplate crate |
| `c881e1982` | docs(roadmap): rendering pipeline 2026-05-19 |
| `bf1144842` | assets(packs): add Kenney + Quaternius CC0 (~181 MB) |
| `1b3301b37` | docs(audit)+refactor(combat): Vague 2 — ARCHITECTURE.md + code mort weapons |
| `6d1836308` | docs(audit): Vague 3 — Bevy 0.18 idioms audit + correction FreeCamera |
| `fb26eeb89` | refactor(arena): story-463 — wave bots .with_children → ChildOf |
| `9b74b08bb` | refactor(ecs): story-461 — Required Components Player + TargetCube + NameplateRoot |
| `ca5c3b99a` | docs(audit): Vague 3 — story-462 SKIP justifié (CombatHitEvent 8 consumers) |
| `9c486bd86` | docs(audit): Vague 5 Phase 5a — plan fusion sensors 29→13 |
| `380aa2f10` | refactor(sensors): Vague 5 Session A Étape 1 — renames forgia2_* + xtask verify |
| `67c20855f` | fix(sensors): Vague 5 Session A — format forgia2_rpg_health conforme + xtask étend canonical |

### Audits livrés (3 documents)

- `docs/audit/audit-2026-05-19.md` (~430 lignes) — forensic V2 général, 258 crates, 6 vagues
- `docs/audit/vague-3-bevy-018-idioms-2026-05-19.md` (~250 lignes) — audit Bevy 0.18 + corrections honnêtes
- `docs/audit/vague-5-sensors-fusion-plan-2026-05-19.md` (~264 lignes) — plan Phase 5b 18-22h en 3 sessions

---

## 🔥 Prochaine session — priorités par ROI

### Option A — V5 Session B (Tier 2 aggregators, ~6h, Enterprise)

Cibles : `forgia2_perf.json` (Bevy Diagnostics) + `forgia2_entities.json` (Query count) + `forgia2_memory.json` (MemoryBreakdown ou fallback).

Bénéfice : 3 sensors gameplay → 7/13 canonical atteints. Continuité directe Session A.
Risque : MOYEN — research Bevy 0.18 Diagnostics API + perf Query 10k entities à valider.

### Option B — V5 Session C (lifecycle/watchdog/audio/input, ~6h, Enterprise)

Cibles : `forgia2_lifecycle.json` (Observer `OnAdd<Player>`) + `forgia2_watchdog.json` (heartbeat) + `forgia2_audio.json` + `forgia2_input.json` + `forgia2_sensor_health.json`.

Bénéfice : Phase 5 complète 13/13. Risque MOYEN-ÉLEVÉ — Observers + timing-sensitive.

### Option C — Vague 1 story-456 hit feedback (Enterprise 10h+) **AAA gameplay impact**

Layered shield/armor (Apex tiers) + headshot/bodyshot routing + audio cue distinct. Fix au passage :
- Bug nameplate HP fill anchor center (commentaire code dit lui-même qu'il faut anchor left)
- Race ChildOf orphelin (~1 warn par kill, check `target.exists()`)

### Option D — Git LFS migration (Standard 2h)

2.9 GB packs binaires tracked → `git lfs migrate import --include="*.glb"`. 0 risque code, hygiène repo. Indépendant.

### Option E — Cleanup ROADMAP + archives tech-debt-plan (Quick 30min)

Archive `docs/tech-debt-plan-2026-05-18.md` (obsolète 80%). Mettre à jour audit-2026-05-19.md avec liens vers vague-5 plan. Hygiène doc seule.

---

## 🚨 Backlog identifié (à ne pas oublier)

- **BUG-464-04 cosmétique** : `ArenaBot::default()` hardcode `los_lost_grace_left: 2.0` au lieu de lire TacticalTuning. Diverge si genome change.
- **Race ChildOf orphelin** : ~1 warn par kill (spawn nameplate ~4ms après despawn bot). Bevy auto-corrige. Fix futur = check `target.exists()` avant spawn dans `forgia-enemy-nameplate::spawn_or_refresh_on_hit`.
- **Nameplate HP fill anchor center** : `forgia-enemy-nameplate/src/lib.rs:175` — commentaire code dit anchor left mais code fait scale.x = frac sans translation décalage. Visible quand HP descend.
- **WIP story-456** layered hit feedback : option C ci-dessus.
- **Tech-debt-plan-2026-05-18.md obsolète** : à archiver dans `docs/archive/` ou supprimer.
- **6 hardcodes weapons.rs:110-141** : SUPPRIMÉS comme code mort (commit `1b3301b37`), pas migrés. À retraiter quand Tier 2A `forgia-weapon-hitscan` extraction reprise (V6).

---

## 📋 Validation runtime requise (avant Session B/C)

Validations Session A passées **2026-05-19 fin session** :

1. ✅ `forgia2_health.json` + `forgia2_rpg_health.json` écrits 1Hz format conforme
2. ✅ `forgia2_arena.json` (410B) + `forgia2_combat.json` (7.2K) aggregators fonctionnels
3. ✅ Anciens `forgia_health.json` + `forgia_rpg_health.json` supprimés, plus écrits
4. ✅ `cargo run -p xtask -- verify-sensors-format` → OK (4/4 canonical sensors validated)
5. ✅ `forgia2_run.log` : 0 ERROR / 0 panic / 0 CHK-5 flood (62K logs propres)

---

## 📎 Liens canoniques

- Plan original vagues : [docs/audit/audit-2026-05-19.md](audit/audit-2026-05-19.md) §7
- Audit Bevy 0.18 : [docs/audit/vague-3-bevy-018-idioms-2026-05-19.md](audit/vague-3-bevy-018-idioms-2026-05-19.md)
- Plan Vague 5 Phase 5b : [docs/audit/vague-5-sensors-fusion-plan-2026-05-19.md](audit/vague-5-sensors-fusion-plan-2026-05-19.md)
- Architecture : [ARCHITECTURE.md](../ARCHITECTURE.md)
- Stories actives : [stories/](stories/)
- Concept-first règle : [.claude/rules/concept-first.md](../.claude/rules/concept-first.md)
- No-speculative-fix règle : [.claude/rules/no-speculative-fix.md](../.claude/rules/no-speculative-fix.md)

*Source de vérité unique. Si conflit avec SESSION_STATE.md, ce fichier prime.*
