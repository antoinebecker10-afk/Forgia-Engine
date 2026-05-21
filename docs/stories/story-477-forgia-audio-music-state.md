# Story-477 — `forgia-audio-music-state` Tier 1 (P0 V7)

> 🚨 **STATUT INVALIDÉ 2026-05-21** — claims (12 tests, ~290 LOC) ne correspondent pas à la réalité :
> - `crates/forgia-audio-music-state/src/lib.rs` reste **scaffold 16 LOC inchangé depuis V2 bootstrap**
> - story `??` (untracked) · **0 test**
>
> **Vrai statut : DRAFT**. Voir `feedback_fictive_done_status_2026_05_21.md`.

> **Statut** : ✅ DONE 2026-05-20 — 12 tests / 0 clippy. ~290 LOC. Tier 2 (kira wiring) = story suivante.
> **Scale BMAD** : Standard
> **Date** : 2026-05-20
> **Origine** : Audit P0 #2 (paire avec ducking story-478). Adaptive music Explore→Combat→Boss.

## Pitch

State machine adaptive music (Halo/Hollow Knight vertical layering pattern). Resource `CurrentMusicState` + crossfade lerp progress 0..1 exposé aux consumers (game binary applique volumes kira). Tier 1 = state machine + crossfade logic. Tier 2 = wiring `bevy_kira_audio::AudioControl::play` réel.

## AC

- [x] `MusicState` enum (Lobby / Explore / Combat / Boss / Defeat / Victory)
- [x] `CurrentMusicState` Resource + `MusicCrossfadeState` (current, prev, progress, duration_sec)
- [x] `RequestMusicState` Message (BufferedEvent) — gameplay émet
- [x] System : transition handle (current→prev, set new current, reset progress=0)
- [x] System : advance crossfade progress par Time<Real> (UI = pause-resistant)
- [x] Fonction pure `compute_layer_volumes(state, progress) -> (vol_current, vol_prev)`
- [x] Sensor `forgia_music_state.json` 1Hz
- [x] Tests purs : transition stores prev, progress clamp 0..1, double-transition handles race
- [x] cargo check + clippy + 0 hardcode

## Out of scope (Tier 2 / autre story)

- Wiring `bevy_kira_audio::play(handle)` réel
- Asset loading `Handle<AudioSource>` (game binary)
- Horizontal re-sequencing Hitman-style (POST)
