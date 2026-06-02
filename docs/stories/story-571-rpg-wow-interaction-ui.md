# Story-571 — RPG WoW-like Interaction UI (cartoon Forge)

**Statut** : DONE
**Niveau BMAD** : Enterprise (6 phases, 3 crates, ~16 fichiers) — livré phase-par-phase
**Date** : 2026-06-02
**Dépend de** : [story-570](story-570-rpg-data-loop-wiring.md) (boucle dialogue→inventaire/quête→XP)
**Origine** : demande user « faire des interfaces d'interaction comme dans WoW » + « cliquer sur l'objet ».

## Objectif

Doter le RPG V2 des **patterns d'interaction WoW** avec habillage **cartoon Forge**
(palette `FORGE_*` de `forgia-ui-lib/style.rs`, PAS le skin gris WoW — cohérence bible
Roguelite). egui 0.33.3 / bevy_egui 0.39 / Bevy 0.18.

## Phases livrées (toutes validées in-game)

- **Phase 0 — Fondations data** : `Gold` (Component Player, départ 50) + `GoldChanged` ;
  `ItemRegistry` data-driven (`config/items/items.toml` + fallback) — nom/icône/prix/rareté/
  catégorie/max_stack/heal_amount ; intercept reward `"gold"` ; `max_stack` lu du registry
  (fin du hardcode 99) ; champ `gold` ajouté à `forgia2_inventory.json`.
- **Phase 1 — Gossip frame** : `forgia-ui-lib/dialogue.rs` réécrit — portrait (cercle+initiale),
  nom doré, texte, **chips de récompense** (XP + objets couleur rareté via QuestCatalogue+ItemRegistry),
  bouton « Accepter » vert (teal) si le choix lance une quête.
- **Phase 2 — Journal (J) + tracker** : `forgia-ui-lib/quest_journal.rs` — panneau modal listant
  quêtes actives + objectifs + barres de progression + bouton « Suivre » ; tracker compact
  bord d'écran (`TrackedQuest` Component Player).
- **Phase 3 — Sacs à icônes (I)** : `forgia-ui-lib/inventory_panel.rs` — grille 8 col, bordure
  rareté, pastille+initiales (fallback sans asset icône), count, **tooltips** au survol, or en en-tête.
  Migré depuis `forgia-rpg`.
- **Phase 3b — Inventaire cliquable** : clic gauche ramasse/pose/échange (`InventoryCursor` +
  `Inventory::take/place`), objet tenu dessiné au curseur ; clic droit utilise (`UseItemRequest`
  → `apply_item_use` soigne via `Health` + `consume_one`). `heal_amount` data-driven.
- **Phase 4 — Marqueurs ! / ?** : `draw_quest_markers` (forgia-rpg) — `QuestGiver{offers,completes}`
  sur le Maître Forgeron ; projection `world_to_viewport` (caméra `RpgOrbitCamera`) ;
  `!` or dispo → `?` gris en cours → `?` or à rendre. Culling 60m.
- **Phase 5 — Vendeur Mira** : `forgia-rpg-data/shop.rs` (`ShopInventory`/`ShopSession` +
  events Open/Buy/Sell/Close + transactions cap-safe) + `forgia-ui-lib/shop_panel.rs`
  (onglets Acheter/Vendre, or, boutons grisés si insuffisant). `DialogueEffect::OpenShop`.

## Revue adversariale (workflow 28 agents, 23 findings → 15 confirmés) + corrections

- **Majeur — quête jamais clôturée** : `DialogueEffect::TurnInQuest` + `QuestLog::turn_in`
  (Completed→TurnedIn) + tree `npc_maitreforgeron_turnin` sélectionné par `interact_system`.
  Marqueur + tracker se résolvent. Pas de double récompense.
- **Majeur — modales empilées au centre** : Resource `RpgOpenPanel` (exclusion I↔J) +
  inventaire/journal masqués si `ShopSession`/`DialogueSession` actif + pas de réouverture auto.
- **Majeur — perte d'objet inventaire** : `inv.add` géré (`InventoryEvent::Full`). *(branche
  réellement inatteignable — take libère toujours un slot — garde défensif.)*
- **Mineur — dialogue empilable sur boutique** : `interact_system` gardé `Without<DialogueSession/ShopSession>`.

**Non corrigé (justifié)** : mouvement libre avec panneau ouvert = **conforme WoW** (pas un bug) ;
ESC ne ferme pas la boutique (bouton ✕ marche) ; latence 1-2 frames quête→XP / `GoldChanged`
sans consommateur / vente min-1 or = cosmétiques sans impact runtime.

## Invariants protégés

- **LOCK-INV-1** : capacité jamais modifiée (UI lit `slots()`/`capacity()` ; take/place/swap
  restent dans les 80 slots ; sensor warn si capacity≠80).
- **Pas de cycle Cargo** : data dans `forgia-rpg-data`, UI dans `forgia-ui-lib` (dép → rpg-data),
  wiring dans `forgia-rpg`/`forgia-game`. forgia-rpg += forgia-damage (heal).
- 0 warning clippy, 39 tests, thème cartoon Forge (pas gris WoW).

## Fichiers

- `forgia-rpg-data` : gold.rs, items.rs, shop.rs (new) ; dialogue.rs, inventory.rs, quests.rs, lib.rs, Cargo.toml
- `forgia-ui-lib` : inventory_panel.rs, quest_journal.rs, shop_panel.rs (new) ; dialogue.rs, lib.rs
- `forgia-rpg` : lib.rs, character.rs, Cargo.toml
- `forgia-game` : lib.rs (5 plugins)
- `forgia-observability` : inventory_sensor.rs (gold)
- `config/items/items.toml` (new)

## Suite naturelle (hors scope)

- Icônes d'objets réelles (pack CC0) → remplacer le fallback couleur+initiales (`ItemDef.icon` prêt).
- Auto-close vendeur quand le joueur s'éloigne (>radius).
- forgia-village-npc-spawner data-driven (story-445) → remplace le spawn lineup/QuestGiver/ShopInventory hardcodé.
- Combat RPG → rend `apply_item_use` heal observable (HP < max) + émetteurs `QuestProgress` réels (kill).
