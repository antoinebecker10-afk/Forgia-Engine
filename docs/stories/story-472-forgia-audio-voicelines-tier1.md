# Story-472 — `forgia-audio-voicelines` Tier 1 selection logic (P0 V7)

> ⛔ **CANCELLED 2026-08-12 — cible supprimée par [ADR-0002](../adr/ADR-0002-cleanup-crates-266-to-62.md)**
> La crate `forgia-audio-voicelines` a été supprimée le 2026-05-26 par le cleanup 266 → 62 crates,
> cinq jours après l'invalidation ci-dessous. Le travail décrit n'est donc plus
> réalisable tel quel : il n'a plus de cible. Le ratchet `cargo xtask no-scaffold`
> interdit désormais de recréer la crate à vide.
>
> **Ce fichier reste une spécification valide** : si le besoin revient, il faut une
> story neuve qui choisit un foyer existant (cf `.claude/rules/fine-grained-crates.md`
> — une crate se justifie par ses consommateurs, pas par son existence).
>
> **Statut** : CANCELLED


> 🚨 **STATUT INVALIDÉ 2026-05-21** — audit RPG/Roguelite a révélé que les claims ci-dessous (22/22 tests, ~620 LOC) ne correspondent pas à la réalité :
> - `crates/forgia-audio-voicelines/src/lib.rs` reste **scaffold 16 LOC inchangé depuis V2 bootstrap**
> - Ce fichier story est `??` (untracked)
> - **0 test** présent dans `src/`
>
> **Vrai statut : DRAFT**. Voir `feedback_fictive_done_status_2026_05_21.md`.

> **État d'origine (périmé, cf bandeau)** : ✅ DONE 2026-05-20 — 22/22 tests verts, 0 clippy `-D warnings`, scaffold 16 LOC → ~620 LOC peuplé. Selection logic deterministe testable. Tier 2 (wiring audio playback `bevy_kira_audio::play`) = story suivante.
> **Scale BMAD** : Standard
> **Date** : 2026-05-20
> **Origine** : Audit maturité crates 2026-05-19 — **P0 vrai ship-blocker** : sans ça "armes parlantes" = mensonge marketing
> **Pitch debloqué** : Pépin/Bourrasque/Madame Lenoir/Boucherie réagissent à kill/lowhp/idle/reload/pickup

## Pitch

Peupler scaffold `forgia-audio-voicelines` avec **système barks Hadès-pattern** (Kasavin GDC 2021) :

- `BarkEvent { speaker, kind, ctx }` BufferedEvent (PR #19647)
- `LinePool` parsé depuis `assets/genomes/roguelite/roguelite_dialogue.toml` (déjà créé, 25 pools/genes)
- `BarkSelector` : cooldown per-ligne + speaker lock + priority override (priority > 150 = ignore cooldown)
- Weighted random selection xorshift32 déterministe (testable)
- Sensor `forgia_voicelines.json` 1Hz

**Tier 1 = selection logic + tests purs.** Tier 2 (story suivante) = wiring audio playback réel via `bevy_kira_audio 0.25`.

## Acceptance Criteria

- [x] `BarkKind` enum : `Kill | LowHp | Idle | Reload | Pickup | Death | StageCleared`
- [x] `BarkEvent` BufferedEvent (pas EntityEvent — multi-consumer ordonné via chain)
- [x] `LineEntry` + `LinePool` + `BarkContext` types serde-friendly
- [x] `VoicelinesConfig` Resource parsée depuis TOML (genes + pools)
- [x] `BarkSelector` Resource : `last_played_at: HashMap<String, f64>` + `current_speaker_lock`
- [x] `select_bark()` fonction pure : (config, selector, event, now) → Option<&LineEntry>
- [x] Priority > priority_death_override (150) **override** cooldown ET speaker_lock
- [x] Speaker lock applique pendant `bark_global_speaker_lock_sec` (default 2.5s)
- [x] Weighted random déterministe (seed-based xorshift32)
- [x] Sensor `forgia_voicelines.json` 1Hz : `{ severity, total_barks_played, last_speaker, lock_remaining_sec, next_step }`
- [x] Tests purs : default state, cooldown skip, priority override, speaker lock, weighted determinism
- [x] `cargo check -p forgia-audio-voicelines` vert
- [x] `cargo clippy -p forgia-audio-voicelines --no-deps -- -D warnings` vert
- [x] Aucun hardcode (rule no-hardcode.md)

## Architecture

```text
forgia-audio-voicelines/
  src/lib.rs    — Bark types + Selector + parser TOML + sensor writer + tests purs

assets/genomes/roguelite/
  roguelite_dialogue.toml   — déjà existant (25 [[pool]] + [[genes]])
```

## Patterns Hadès appliqués

- **"What would these characters notice?"** — pools indexés par (speaker, event)
- **Anti-spam** — speaker lock + per-line cooldown
- **Priority override** — Death > Idle, critical events ignorent cooldown

Sources :
- [Kasavin GDC 2021 "Breathing Life into Greek Myth"](https://www.gdcvault.com/play/1026975/)
- [PR #19647 BufferedEvent split](https://github.com/bevyengine/bevy/pull/19647)
- Memory `reference_industry_roguelite_patterns_2026_05_19`

## Out of scope Tier 1 (Tier 2 = story suivante)

- Wiring audio playback via `bevy_kira_audio::AudioControl::play`
- Spatial 3D positional via `Emitter`/`Receiver` components
- Ducking music quand voice active (cf forgia-audio-ducking P0 #2)
- Hot-reload TOML genome (file_watcher Bevy 0.18)
- i18n IDs Fluent (Tier 2/3 — bevy_fluent compat 0.18 à vérifier)
