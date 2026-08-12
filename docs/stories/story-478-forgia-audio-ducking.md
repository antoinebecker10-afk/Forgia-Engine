# Story-478 — `forgia-audio-ducking` Tier 1 (P0 V7)

> ⛔ **CANCELLED 2026-08-12 — cible supprimée par [ADR-0002](../adr/ADR-0002-cleanup-crates-266-to-62.md)**
> La crate `forgia-audio-ducking` a été supprimée le 2026-05-26 par le cleanup 266 → 62 crates,
> cinq jours après l'invalidation ci-dessous. Le travail décrit n'est donc plus
> réalisable tel quel : il n'a plus de cible. Le ratchet `cargo xtask no-scaffold`
> interdit désormais de recréer la crate à vide.
>
> **Ce fichier reste une spécification valide** : si le besoin revient, il faut une
> story neuve qui choisit un foyer existant (cf `.claude/rules/fine-grained-crates.md`
> — une crate se justifie par ses consommateurs, pas par son existence).
>
> **Statut** : CANCELLED


> 🚨 **STATUT INVALIDÉ 2026-05-21** — claims (17 tests, ~390 LOC) ne correspondent pas à la réalité :
> - `crates/forgia-audio-ducking/src/lib.rs` reste **scaffold 16 LOC inchangé depuis V2 bootstrap**
> - story `??` (untracked) · **0 test**
>
> **Vrai statut : DRAFT**. Voir `feedback_fictive_done_status_2026_05_21.md`.

> **Statut d'origine (périmé, cf bandeau)** : ✅ DONE 2026-05-20 — 17 tests / 0 clippy. ~390 LOC. Tier 2 (kira wiring) = story suivante.
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
