# Forgia V2 Rewrite — ROADMAP_CURRENT

> **Source de vérité unique** pour l'état des vagues V2 et la priorisation BMAD.
> Mise à jour à chaque livraison story ou à la commande "Memorise" (CLAUDE.md §11).
>
> **Dernière révision** : 2026-05-19 fin session marathon (6 commits livrés).
> **HEAD courant** : `51c084925`.

---

## 🌊 Vagues — état canonique

Plan original : `docs/audit/audit-2026-05-19.md` §7. Cette table est le statut **vivant**.

### V1 — Débloquer (P0) ✅ DONE

| Item | Statut | Livré par |
|---|---|---|
| Fix `LocomotionBoneCache` fields | ✅ | session 2026-05-18 |
| `forgia_combat.json` producer | ✅ | session 2026-05-18 (story-453) |
| `forgia_health.json` producer | ✅ | session 2026-05-18 (story-452) |

### V2 — Discipline & traçabilité (P1) ⚠️ 75 %

| Item | Effort | Statut | Livré par / reste à faire |
|---|---|---|---|
| ARCHITECTURE.md actualisé | 1h | ✅ | session 2026-05-19 matin |
| Sensor fusion Tier 1 (`forgia2_combat` + `forgia2_arena`) | 2h | ✅ | story-465 (commit `aae934198`) |
| Migration weapon balance → genome TOML | 30 min | ⏳ | **B1 — à faire** (`weapons.rs:110-141` hardcoded) |
| Story-458 concept-mapping doc | 30 min | ⏳ | **B2 — à faire** (`locomotion-bone-cache` ligne §6 concept-first.md) |

### V3 — Modernisation Bevy 0.18 (P1) ✅ DONE (avec SKIPs documentés)

| Item | Statut | Livré par |
|---|---|---|
| Required Components Player/TargetCube/NameplateRoot | ✅ | story-461 (session précédente) |
| Wave bots ChildOf relationships | ✅ | story-463 (session précédente) |
| Observers death/pickup/damage | ✅ partial | story-466 (DeathEvent only — DamageEvent + CombatHitEvent SKIP documenté) |
| RpgOrbitCamera vs PanCamera first-party | ✅ FALSIFIÉ | audit Vague 3 — FreeCamera/PanCamera gameplay n'existent pas en 0.18.1 |

### V4 — Tech debt P1-P2 ✅ DONE

| Item | Statut | Livré par |
|---|---|---|
| Fix tests melee TimePlugin advance_by trap | ✅ | session 2026-05-19 matin |
| Fix test weapons cycle | ✅ | session 2026-05-19 matin |
| `tech-debt-plan-2026-05-18.md` obsolète à 80 % | ✅ | (à archiver) |

### V5 — Phase 5 sensors complet (P2) ❌ Not started

Enterprise ~6h. Aligner 100 % ARCHITECTURE.md cible : 27 sensors `forgia_*` legacy → 12 sensors `forgia2_*`.

- Producteurs à migrer : 27 → 12 (Tier 1 fait, Tier 2 + 3 restants)
- `xtask verify-sensors-format` (schema enforcement)
- CI gate `forgia2_*.json count == 12`
- Fixtures `crates/forgia-sensors/tests/`

### V6 — Crates extraction (P2) ❌ Bloqué

Tier 2A/B mentionné : `forgia-weapon-hitscan`, `forgia-weapon-viewmodel`. À ne pas relancer avant V2+V3 stabilisées.

---

## 🚀 Hors plan vagues — additions session 2026-05-19

| Story | Type | Commit |
|---|---|---|
| story-464 LOS state gating (bot AI) | feat(ai) | `20fefe9d7` |
| Nameplate permanent + face-cam + cartoon | feat(ui) | `1a7ce3eff` |
| 3 fixes audit qa-lead (BUG-464-01/02/03) | fix(audit) | `9d2baeaae` |
| story-465 sensor fusion Tier 1 | feat(observability) | `aae934198` |
| story-466 DeathEvent → Observer | refactor(ecs) | `f3bd4fdf3` |
| SESSION_STATE.md snapshot | docs | `51c084925` |

---

## 🔥 Prochaine session — priorités par ROI

### Option A — Finir V2 (1h Quick × 2) **RECOMMANDÉ**

- **B1** Migration weapon balance → genome TOML (30 min) — story-467
- **B2** Concept-first doc `locomotion-bone-cache` (30 min) — story-458

Bénéfice : V2 100 % closed, hygiène Lock data-driven, post-mortem D1 traçable.

### Option B — V5 sensors complet (Enterprise ~6h)

Fusion 27 → 12 + xtask + CI gate + fixtures. Gros chantier mais ferme la Phase 5 Architecture.

### Option C — Vague 1 hit feedback story-456 (Enterprise 10h+)

Layered shield/armor + headshot/bodyshot routing + audio cue. Suite naturelle nameplate+combat. Gros impact gameplay feel.

### Option D — Git LFS migration (Standard 2h)

2.9 GB packs binaires tracked → LFS. 0 risque code, hygiène repo long-terme. Indépendant.

---

## 🚨 Backlog identifié (à ne pas oublier)

- **BUG-464-04** cosmétique : `ArenaBot::default()` hardcode `los_lost_grace_left: 2.0` au lieu de lire TacticalTuning. Diverge si genome change.
- **Race ChildOf orphelin** : ~1 warn par kill (spawn nameplate ~4ms après despawn bot). Bevy auto-corrige. Fix futur = check `target.exists()` avant spawn.
- **WIP story-456** layered hit feedback : option C ci-dessus.
- **6 hardcodes weapon balance** : couvert par B1.

---

## 📋 Validation runtime requise (avant prochain dev)

Aucun smoke-test runtime fait après les 6 commits 2026-05-19. À faire :

1. Lancer binaire en **Arena FPS**
2. Cibler bot : nameplate **visible dès spawn** + face caméra peu importe orientation bot
3. Se cacher derrière mur : bot **s'arrête ~2s après** perte LOS (pas tracking permanent)
4. Lire `forgia2_combat.json` + `forgia2_arena.json` : format `{id, severity, next_step, sources, ...}` peuplé
5. Lire `forgia_bot_ai.json` : champ `bots_in_grace` visible
6. Tuer un bot : pas de warn registry orphelin, despawn propre via Observer

---

## 📎 Liens canoniques

- Plan original vagues : [docs/audit/audit-2026-05-19.md](audit/audit-2026-05-19.md) §7
- Architecture : [ARCHITECTURE.md](../ARCHITECTURE.md)
- Session state courant : [SESSION_STATE.md](SESSION_STATE.md)
- Stories actives : [stories/](stories/)
- Concept-first règle : [.claude/rules/concept-first.md](../.claude/rules/concept-first.md)

*Source de vérité unique. Si conflit avec SESSION_STATE.md, ce fichier prime.*
