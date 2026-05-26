# Audit 93 scaffolds — preparation stories 516+

**Date** : 2026-05-23
**Origin** : story-515 ratchet a baseline 93 scaffolds restants après stories 512+513
**Owner** : Antoine + Claude

---

## Synthèse

93 crates pures scaffolds (<50 effective LOC). Boilerplate identique : `struct ForgiaXxxPlugin; impl Plugin { build() { TODO } }`. Deps : `bevy + forgia-core` uniquement. **Majorité = 0 consumer**.

Exceptions (consumers existants) : `forgia-juice-hit-stop`, `forgia-juice-recoil`, `forgia-input-*`, `forgia-audio-*`, `forgia-genome-core/registry/village`.

---

## 7 clusters identifiés

| # | Cluster | Crates | Pattern | Consumers | Recommandation |
|---|---|---|---|---|---|
| 1 | AI Subsystems | 9 (blackboard, bt, flocking, formation, goap, navmesh, perception, state-machine, utility) | Plugin identique | 0 | **DELETE** |
| 2 | UI Widgets | 12 (credits, gauges, inventory, loadscreen, menu, minimap, notifications, objectives, settings-panel, tooltip + 2) | Plugin identique | 0 | **FUSION** → `forgia-ui-lib` |
| 3 | Weapon Types | 5 (beam, charged, melee, projectile + 1) | Plugin identique | 0 | **FUSION** → `forgia-weapon-system` |
| 4 | Genome Foundation | 8 (ai-personality, balance, catalogue, economy, registry, sync, validator + 1) | Mixed | 3 (core/registry/village) | **IMPLEMENT** 3 / **DELETE** 5 |
| 5 | Input Systems | 5 (azerty-qwerty, gamepad, keybind, rebind-ui, recording) | Plugin identique | parent `forgia-input` a 5 consumers | **IMPLEMENT** |
| 6 | Audio Subsystems | 7 (core, ducking, footsteps, mixer, music-state, occlusion, voicelines) | Plugin identique | 17 imports détectés | **IMPLEMENT** (hierarchical OK) |
| 7 | Physics/VFX/Misc | 45+ (physics, ragdoll, cloth, vfx-*, water, underwater, time, scene, events-bus, skill-tree, status-effects, marketplace, steam, anticheat, oxr, grid, navigator, sensors, scripting-luau, silk, coedit-crdt, crafting, equipment, founders-pass, gauge, imports, mod-outline, ribbons, shape-library, signals, telemetry-funnel, trails, asset-*, animation-*, player-* x4) | Plugin identique | 0 majoritaire | **DELETE** progressif |

---

## Top 3 candidats fusion (analogues story-513)

### 1. `forgia-ui-*` → `forgia-ui-lib` (12 crates)
- Boilerplate strictly identique (`ForgiaUiXxxPlugin`)
- 0 consumer détecté → 0 risque fusion
- Pattern macro `define_simple_ui_widget!` réutilisable depuis story-513
- ETA : ~3-4h

### 2. `forgia-weapon-*` → `forgia-weapon-system` (5 crates)
- Identique Plugin pattern
- 0 consumer détecté (ni `forgia-combat` ne les use)
- Domaine logique cohérent (melee, projectile, beam, charged = subtypes armes)
- ETA : ~2h

### 3. `forgia-animation-blend` + `forgia-animation-mixamo` → `forgia-animation-lib` (2 crates)
- Boilerplate identique
- 0 consumer détecté
- ETA : ~1h

---

## Roadmap proposée — 5 stories follow-up

| Story | Type | Cible | Estimate |
|---|---|---|---|
| **story-516** | DELETE vague | AI (9) + VFX (3) + Cloth, Ragdoll, Scripting, Oxr, Anticheat, Steam, Marketplace = **20 crates** | BMAD Standard, ~4h |
| **story-517** | FUSION vague | UI (12) + Weapons (5) → 2 meta-crates = **17→2 crates** | BMAD Standard, ~6h (reuse macro pattern story-513) |
| **story-518** | Genome cleanup | Keep 3 (core/registry/village), delete/implement 5 = **5 crates** | BMAD Standard, ~3h |
| **story-519** | Input + Audio | IMPLEMENT stubs (consumers existent) = **12 crates** | BMAD Enterprise (impl réel) |
| **story-520** | Player controllers + Misc | 4 player + 4 misc = **8 crates delete/move** | BMAD Standard, ~3h |

**Total** : ~55 DELETE + 17 FUSION + 12 IMPLEMENT = **84/93 crates traités**.

Reste 9 crates non catégorisées (à audit case-by-case) : foundation legit potentiels (forgia-time, forgia-scene, forgia-events-bus, forgia-signals etc.) qui pourraient mériter implementation rapide ou allowlist permanente.

---

## Allowlist mise à jour potentielle

Après stories 516-520 ship, `xtask/no-scaffold-allowlist.toml` devrait contenir :
- ~2 foundation crates (forgia-core, forgia-rng)
- ~3-5 nouveaux meta-crates fusionnés (forgia-ui-lib, forgia-weapon-system, forgia-animation-lib...)
- ~9 crates implémentées (genome/input/audio foundations)

Soit ~15 entries vs 95 actuelles. Cible : **80% de réduction allowlist**.

---

## Anti-pattern à éviter

- ❌ DELETE en bulk sans vérifier consumers à chaque crate (cf bug story-512 vs story-468 forgia-net-*)
- ❌ FUSION speculative sur crates dont les Settings diffèrent (cf [[reference-pp-fusion-macro-pattern]] outliers outline/toon hand-written)
- ❌ IMPLEMENT sans story dédiée (les stubs ont 0 LOC métier — il faut design + tests + sensor)

---

## Cross-refs

- `docs/stories/story-513-pp-fusion-postprocess.md` (pattern fusion source)
- `xtask/no-scaffold-allowlist.toml` (baseline 93 scaffolds listées)
- [[reference-pp-fusion-macro-pattern]] memory
- [[reference-xtask-no-scaffold-ratchet]] memory
