# Story-528 — FPS Feel Accessible (Mission 1 GDD)

> **Status** : DRAFT
> **Scale BMAD** : Standard
> **Effort estimé** : ~3-4 jours
> **GDD ref** : [Mission 1 — FPS Accessible](../design/gdd-roguelite-v1.md#mission-1--fps-accessible)
> **Prérequis** : aucun (foundation tier 1)

## Pourquoi

Forgia FPS actuel = accessible mais générique. Cible commerciale (enfants Roblox + casual + femmes gameuses 20-35) exige :
- aim assist toggleable (Roblox kids, accessibility)
- dash "bondir" lore-cohérent
- mort = "tomber dans les pommes" (pas Game Over)
- feedback hit cartoon sans sang

## Acceptance Criteria

- [ ] AC1 — Aim assist `fps_tuning.toml` hot-reload, strength 0.0..1.0 toggle, default 0.5
- [ ] AC2 — Hitbox capsule generous 1.2× model size, HS zone top 25% multiplier ×2 Lenoir / ×1.5 Pépin / ×1.0 autres
- [ ] AC3 — Dash "Pas de l'Apprenti" : 4m en 0.25s, cooldown 1.5s, 2 charges UI, Espace double-tap, voiceline *"Hop !"*
- [ ] AC4 — Fatigue (renommer HP→Énergie UI) : à 0 → fade warm-orange + voix Maître Forgeron + retour hub (pas YOU DIED)
- [ ] AC5 — Hit feedback : 4 étoiles ✨ jaune 200ms + ting cartoon + flash blanc enemy 80ms
- [ ] AC6 — HS feedback : "POW!" + DING aigu + +20% knockback
- [ ] AC7 — Sensor `forgia2_fps_feel.json` : aim_assist_engagements/s, dash_uses/s, hit_feedbacks/s

## Sensors
- `forgia2_fps_feel.json` NEW
- Reuse `forgia2_player_state.json` (story-526)

## Files probably touched
- `crates/forgia-player/src/lib.rs` (dash + fatigue)
- `crates/forgia-fps/src/aim_assist.rs` NEW
- `crates/forgia-effects/src/hitmarker.rs` (extend cartoon stars)
- `crates/forgia-ui-lib/src/hud_energy.rs` NEW (cœurs cartoon)
- `assets/genomes/fps_tuning.toml` (aim_assist_strength)

## Anti-canon checklist
- Aucun "die/death/blood" dans code, doc, UI
- "Fatigue/Énergie" partout (pas "Health")
- "Bond/Hop" pas "dodge/dash" en UI

## Cross-refs
- GDD V1 Mission 1
- Bible v1 ton/anti-canon
- story-526 sensors (player_state + lag_events) précédents
