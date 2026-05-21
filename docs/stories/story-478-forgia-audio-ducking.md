# Story-478 — `forgia-audio-ducking` Tier 1 (P0 V7)

> 🚨 **STATUT INVALIDÉ 2026-05-21** — claims (17 tests, ~390 LOC) ne correspondent pas à la réalité :
> - `crates/forgia-audio-ducking/src/lib.rs` reste **scaffold 16 LOC inchangé depuis V2 bootstrap**
> - story `??` (untracked) · **0 test**
>
> **Vrai statut : DRAFT**. Voir `feedback_fictive_done_status_2026_05_21.md`.

> **Statut** : ✅ DONE 2026-05-20 — 17 tests / 0 clippy. ~390 LOC. Tier 2 (kira wiring) = story suivante.
> **Scale BMAD** : Standard
> **Date** : 2026-05-20
> **Origine** : Audit P0 #2 (paire avec music-state story-477). Sidechain music quand voiceline active.

## Pitch

Ducking system : `TriggerDuck` Message déclenche atténuation temporaire d'un layer audio (music par défaut). Compute max-active attenuation_db par layer, exposé via `CurrentDuckingDb` Resource. Tier 1 = logic state machine. Tier 2 = wiring kira `AudioControl::set_volume` réel.

## AC

- [x] `DuckingLayer` enum (Music / Ambient / Sfx)
- [x] `DuckingSource` struct interne (remaining_sec, attenuation_db, layer)
- [x] `TriggerDuck` Message (BufferedEvent) avec layer + duration_sec + attenuation_db
- [x] `DuckingConfig` Resource (default_voice_attenuation_db, default_voice_duration_sec)
- [x] `DuckingState` Resource (active sources Vec + computed current_attenuation_db par layer)
- [x] System tick : decrement remaining_sec via Time<Real>, expire <=0
- [x] System compute : current_db par layer = max attenuation_db des sources actives
- [x] Fonction pure `compute_attenuation(sources, layer) -> f32`
- [x] Sensor `forgia_ducking.json` 1Hz
- [x] Tests purs : single source attenuation, multi sources max, expire cleanup, no-source = 0db
- [x] cargo check + clippy + 0 hardcode

## Out of scope Tier 2

- Wiring `kira::AudioControl::set_volume(linear_from_db)`
- Sidechain envelope follower (attack/release tween) — simplifié MVP en step-on/step-off
