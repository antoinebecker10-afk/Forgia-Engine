# Story-482 — `forgia-audio-voicelines` Tier 1.6 floating bark text overlay (P0 V7 M4)

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


> 🚨 **STATUT INVALIDÉ 2026-05-21** — claims (30/30 + 5/5 tests, 3 crates clippy vert) ne correspondent pas à la réalité :
> - `crates/forgia-audio-voicelines/src/lib.rs` reste **scaffold 16 LOC inchangé depuis V2 bootstrap**
> - story `??` (untracked) · **0 test** dans la crate
> - Pattern identique au batch 471-479 fictif
>
> **Vrai statut : DRAFT**. Voir `feedback_fictive_done_status_2026_05_21.md`.

> **État d'origine (périmé, cf bandeau)** : ✅ DONE 2026-05-20 — 30/30 tests voicelines + 5/5 tests speaker mapping verts, clippy `-D warnings` vert sur les 3 crates
> **Scale BMAD** : Standard (3 fichiers)
> **Date** : 2026-05-20
> **Origine** : Story-481 wire-up live (12 barks runtime). Tier 2 audio bloqué (0 WAV, 0 audio_path TOML). Pivot recommandé : retour utilisateur visuel via egui bubble.
> **Pitch debloqué** : Pépin/Bourrasque/Lenoir/Boucherie parlent à l'écran (texte), pattern Hadès portrait + bulle. Tier 2 audio shippable dès qu'un WAV existe.

## Pitch

Afficher le `text` de la `LineEntry` sélectionnée dans une bulle egui en bas-centre écran, style "portrait Hadès". Durée 3s + fade. Speaker affiché en label coloré (Pépin = vert, Bourrasque = bleu, Lenoir = violet, Boucherie = rouge).

Resource `ActiveBark { speaker, text, expires_at_secs }` mise à jour dans `process_bark_events`. HUD lit la Resource et draw conditionnellement.

## Acceptance Criteria

- [x] `BarkSelectionOutcome::Selected` étendu avec champ `text: String`
- [x] Resource `ActiveBark { speaker, text, expires_at_secs }` (None = pas de bubble)
- [x] `process_bark_events` met à jour `ActiveBark` quand Selected, expires_at = now + 3s
- [x] System egui `draw_bark_bubble` dans forgia-mode-roguelite : bottom-center, speaker label coloré, fade dernière 0.5s
- [x] Bubble disparaît si `now > expires_at_secs`
- [x] Speaker → couleur : helper `speaker_color()` (pepin=green, bourrasque=blue, lenoir=violet, boucherie=red, any=gray)
- [x] Tests : `bark_outcome_carries_text`, `active_bark_set_on_selection`, `active_bark_expires`, `speaker_color_mapping`
- [x] `cargo check + clippy -D warnings` vert sur les 2 crates touchés

## Architecture

```text
forgia-audio-voicelines/src/lib.rs   — Selected.text + ActiveBark Resource + update logic + tests
forgia-mode-roguelite/src/bark_hud.rs  — NEW egui system draw_bark_bubble
forgia-mode-roguelite/src/lib.rs       — register bark_hud module + system
```

## Concept-first

- **Couche** : framework (Resource + UI system)
- **Producteur** : `process_bark_events` (forgia-audio-voicelines)
- **Consommateur** : `draw_bark_bubble` (forgia-mode-roguelite, egui)
- **Sensor** : `forgia_voicelines.json` (déjà existant, ajoute champ `active_bark_speaker` optionnel)
- **Net** : L
- **Hot** : non (egui 1× per frame, draw conditional)

## Out of scope (Tier 2 quand assets audio dispo)

- `bevy_kira_audio::Audio::play(audio_path)` réel
- Spatial 3D
- Ducking music
- Portraits PNG des armes (style Hadès photographique)

## Cross-refs

- [story-481](story-481-audio-voicelines-tier1.5-wireup.md)
- [story-472](story-472-forgia-audio-voicelines-tier1.md)
- Hadès Kasavin GDC 2021 (portrait + bubble pattern)
