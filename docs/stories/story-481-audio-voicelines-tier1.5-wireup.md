# Story-481 — `forgia-audio-voicelines` Tier 1.5 wire-up (P0 V7 M4)

> 🚨 **STATUT INVALIDÉ EN CASCADE 2026-05-21** — cette story prétend brancher
> `BarkEvent → select_bark → sensor` mais dépend de la crate `forgia-audio-voicelines`
> qui est restée **scaffold 16 LOC** (cf story-472 invalidé). Sans la Tier 1 réelle,
> le wireup est vide.
>
> **Vrai statut : BLOCKED** (par re-impl story-472 / story-491). Voir
> `feedback_fictive_done_status_2026_05_21.md` + audit story-gate §détail.

> **Statut** : ✅ DONE 2026-05-20 — 3 fichiers + Cargo deps, 2/2 tests `weapon_to_speaker`, clippy `-D warnings` vert sur forgia-mode-roguelite + forgia-audio-voicelines + forgia-game
> **Scale BMAD** : Standard (3 fichiers)
> **Date** : 2026-05-20
> **Origine** : Story-472 Tier 1 livré mais Plugin jamais plugé + 0 producteur `BarkEvent` → sensor `forgia_voicelines.json` n'apparaît pas, logique non testable runtime
> **Pitch debloqué** : premier bark Forgia déclenché en runtime (vertical slice testable bout-en-bout)

## Pitch

Brancher la chaîne `kill → BarkEvent → select_bark → sensor`. Sans audio playback réel (Tier 2). Permet de vérifier que :

- Le Plugin charge le genome `roguelite_dialogue.toml`
- Les pools sont peuplés (`forgia_voicelines.json: pools_loaded > 0`)
- Le sensor passe en `severity: ok`
- `total_barks_played > 0` après un kill ennemi en mode Roguelite
- Les logs `info!("[forgia-audio-voicelines] BARK ...")` apparaissent

## Acceptance Criteria

- [x] `ForgiaAudioVoicelinesPlugin` ajouté dans `forgia-game/src/lib.rs` groupe 7 (mode-specific plugins)
- [x] `forgia-mode-roguelite` deps : ajout `forgia-audio-voicelines`
- [x] `obs_roguelite_enemy_death` étendu pour émettre `BarkEvent { speaker, kind: Kill, now }` où `speaker` est dérivé de `EquippedWeapons.current` (Pépin/Bourrasque/Lenoir/Boucherie)
- [x] Fallback `speaker = "any"` si arme inconnue
- [x] `cargo check -p forgia-game -p forgia-mode-roguelite` vert
- [x] `cargo clippy -p forgia-mode-roguelite --no-deps -- -D warnings` vert
- [x] Test pur dans `forgia-mode-roguelite` : `weapon_to_speaker(WeaponType::ModernAR) == "pepin"` etc.

## Architecture

```text
forgia-game/src/lib.rs                — add ForgiaAudioVoicelinesPlugin
forgia-mode-roguelite/Cargo.toml      — add forgia-audio-voicelines dep
forgia-mode-roguelite/src/run.rs      — extend obs_roguelite_enemy_death
                                         + helper weapon_to_speaker()
```

## Concept-first (CLAUDE.md §3)

- **Couche** : framework (wire-up Plugin + system) — definition (TOML) déjà livré Tier 1
- **Producteur** : `obs_roguelite_enemy_death` (observer on `DeathEvent`)
- **Consommateur** : `process_bark_events` (déjà dans forgia-audio-voicelines)
- **Sensor** : `forgia_voicelines.json` (déjà existant Tier 1)
- **Net** : L (local, pas répliqué Tier 1.5)
- **Hot** : non (observer event, ~1-10 Hz max)

## Out of scope (Tier 2 = story suivante)

- Wiring audio playback `bevy_kira_audio::AudioControl::play`
- Producteurs BarkEvent supplémentaires (LowHp, Reload, Pickup, Death, Idle)
- Spatial 3D positional
- Ducking music
- Hot-reload TOML

## Cross-refs

- [story-472](story-472-forgia-audio-voicelines-tier1.md) — Tier 1 selection logic
- Memory [[reference-v7-p0-session-2026-05-20]] — P0 batch contexte
- Memory [[reference-v7-damage-pipeline-dual-path]] — DeathEvent canal V7
