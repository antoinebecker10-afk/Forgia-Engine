# Story-476 — `forgia-status-effects` (P0 V7)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : trace partielle** (fichier `lib.rs`) — une partie de ce
> qu'elle décrit existe, le reste n'a pas été retrouvé.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> 🚨 **STATUT INVALIDÉ 2026-05-21** — claims (19 tests, ~440 LOC) ne correspondent pas à la réalité :
> - `crates/forgia-status-effects/src/lib.rs` reste **scaffold 16 LOC inchangé depuis V2 bootstrap**
> - story `??` (untracked) · **0 test**
>
> **Vrai statut : DRAFT**. Voir `feedback_fictive_done_status_2026_05_21.md`.

> **État d'origine (périmé, cf bandeau)** : ✅ DONE 2026-05-20 — 19 tests / 0 clippy. ~440 LOC.
> **Scale BMAD** : Standard
> **Date** : 2026-05-20
> **Origine** : Audit P0 #5

## Pitch

Timeline status effects avec stacking (Hadès elemental pattern). `StatusEffects` Component sur entité, Update tick consomme `Time<Virtual>` (compatible hit-stop). Stack si même kind, max_stacks cap.

## AC

- [x] `StatusKind` enum (Burn / Slow / Stun / Poison / Bleed / BuffDamage / BuffSpeed / Lifesteal)
- [x] `StatusEffect` struct (kind, intensity, duration_remaining_sec, stacks, max_stacks)
- [x] `StatusEffects` Component (Vec<StatusEffect>)
- [x] `ApplyStatusEffect` Message + `StatusExpired` Message
- [x] Apply : si même kind existe, increment stacks (clamped) + refresh duration
- [x] Tick : decrement duration_remaining_sec par delta_seconds() Time<Virtual>
- [x] Expire : retain |s| s.duration_remaining_sec > 0
- [x] Sensor `forgia_status_effects.json` 1Hz (total_active, kind_distribution)
- [x] Tests purs : apply new, stack same kind, max stacks clamp, tick decrement, expire cleanup
- [x] cargo check + clippy + 0 hardcode

## Out of scope

- Side-effects gameplay (damage tick burn, speed mult slow) — caller responsability (forgia-combat, forgia-player)
- Visual VFX overlay (forgia-vfx-impact-library post-MVP)
- Sound triggers per status (forgia-audio-voicelines hooks)
