# Story-537 — Méta-progression Hub Évolutif + Éclats persist (Mission 4.5 GDD)

> **Status** : DRAFT
> **Scale BMAD** : Standard
> **Effort estimé** : ~3 jours
> **GDD ref** : [Mission 4.5](../design/gdd-roguelite-v1.md#45-méta-progression)
> **Prérequis** : story-536 (boss final unlock all armes), story-538 (polish hub)

## Pourquoi

Sans méta-progression : aucune raison de relancer un run après échec ou completion. Cible : voir le hub évoluer + collectionner voicelines/cosmétiques = hook addictif vache à lait pour cible commerciale (enfants Roblox, women gamers, casual).

## Acceptance Criteria

### Monnaie éclats d'âme

- [ ] AC1 — Éclats d'âme persistent inter-run via save file (`~/.forgia/save.toml` ou cross-platform XDG)
- [ ] AC2 — Gain : 5-15 éclats par kill (varie par enemy archetype), +100 si victoire boss final
- [ ] AC3 — UI HUD top-right compteur cartoon ✨ X100 (egui ou bevy_ui)

### Hub évolution visuelle 🟡

- [ ] AC4 — Run 1 : atelier vide, 1 enclume, Maître Forgeron seul
- [ ] AC5 — Run 3 : 🔫 Pépin sur étagère (dialogue spécial unique "Première rencontre")
- [ ] AC6 — Run 5 : 💨 Bourrasque + 🎩 Lenoir libérées (cinematic accueil 30s)
- [ ] AC7 — Run 8 : 🪓 Boucherie ajoutée + Petit Champignon Lumineux ambient 🍄 suit dans hub
- [ ] AC8 — Run 10 : forge complète, 4 armes flottent autour Apprenti, statue centrale
- [ ] AC9 — Run 15+ : statues décoratives extra, Maître Forgeron prépare "surprise" (V2 hook visuel)

### Débloquables V1

- [ ] AC10 — Voicelines random unlock paliers : 250/500/1000/2500/5000 éclats (pool extensible)
- [ ] AC11 — Cosmétiques arme 3 skins/arme (bleu/cuivre/or) : 100/250/500 éclats par skin
- [ ] AC12 — +1 énergie max permanent (cap +5) : 200/500/1000/2000/4000 éclats croissants
- [ ] AC13 — Starting boon unlock (à partir run 10) : 1000 éclats → choisis 1 boon début de chaque run

### Sensors + save

- [ ] AC14 — Sensor `forgia2_meta.json` : total runs, éclats total accumulés, débloquables possédés count
- [ ] AC15 — Save file structure `forgia_save_v1.toml` versionnée (schema_version field)
- [ ] AC16 — Save corruption recovery : si fail parse → backup + restart "Run 1" propre

## Files
- `crates/forgia-rpg/src/hub_evolution.rs` NEW (state machine palier visuel)
- `crates/forgia-roguelite-meta/` NEW crate ou extend `forgia-rpg-data`
- `crates/forgia-rpg-data/src/save.rs` NEW (persistence cross-run)
- `assets/genomes/roguelite/meta_unlocks.toml` NEW (paliers + costs)

## Anti-canon
- "Débloquer" pas "earn/grind"
- "Éclats d'âme" partout (canon bible, jamais "souls" anglais)
- Maître Forgeron commente débloquages : *"Ah ! Tu as trouvé un nouvel ami !"*

## Cross-refs
- GDD V1 Mission 4.5
- Bible v1 hub Maître Forgeron + cast
- story-538 (polish visuel hub évolutif)
