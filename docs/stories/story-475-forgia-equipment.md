# Story-475 — `forgia-equipment` (P0 V7)

> ⛔ **CANCELLED 2026-08-12 — cible supprimée par [ADR-0002](../adr/ADR-0002-cleanup-crates-266-to-62.md)**
> La crate `forgia-equipment` a été supprimée le 2026-05-26 par le cleanup 266 → 62 crates,
> cinq jours après l'invalidation ci-dessous. Le travail décrit n'est donc plus
> réalisable tel quel : il n'a plus de cible. Le ratchet `cargo xtask no-scaffold`
> interdit désormais de recréer la crate à vide.
>
> **Ce fichier reste une spécification valide** : si le besoin revient, il faut une
> story neuve qui choisit un foyer existant (cf `.claude/rules/fine-grained-crates.md`
> — une crate se justifie par ses consommateurs, pas par son existence).
>
> **Statut** : CANCELLED


> 🚨 **STATUT INVALIDÉ 2026-05-21** — claims (12 tests, ~290 LOC) ne correspondent pas à la réalité :
> - `crates/forgia-equipment/src/lib.rs` reste **scaffold 16 LOC inchangé depuis V2 bootstrap**
> - story `??` (untracked) · **0 test**
>
> **Vrai statut : DRAFT**. Voir `feedback_fictive_done_status_2026_05_21.md`.

> **État d'origine (périmé, cf bandeau)** : ✅ DONE 2026-05-20 — 12 tests / 0 clippy. ~290 LOC.
> **Scale BMAD** : Standard
> **Date** : 2026-05-20
> **Origine** : Audit P0 #4. Dépend conceptuellement de loot-tables mais code-wise indépendant.

## Pitch

Equip slots simples MVP : Primary weapon, Secondary weapon, Accessory1, Accessory2. Component sur entity Player. Events `EquipRequest` (BufferedEvent) + `EquipResult` (prev/new).

## AC

- [x] `EquipmentSlot` enum (Primary / Secondary / Accessory1 / Accessory2)
- [x] `Equipment` Component (HashMap<EquipmentSlot, String>) — `String` = item_id (loose coupling)
- [x] `EquipRequest` + `EquipResult` Messages (BufferedEvent)
- [x] Méthodes `equip` (swap) + `unequip` + `get` + `has` sur Equipment
- [x] Sensor `forgia_equipment.json` 1Hz (slots count)
- [x] Tests purs : equip swap, unequip, no-stack-same-slot
- [x] cargo check + clippy + 0 hardcode

## Out of scope

- UI inventory grid (forgia-inventory Plugin séparé, régression V1 → story dédiée)
- Item stats / passives system (post-MVP)
- Set bonus (4-piece equip → bonus) — POST
