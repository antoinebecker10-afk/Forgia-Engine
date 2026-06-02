# Story-570 — RPG Data Loop Wiring (dialogue → inventaire/quête → récompense → XP)

**Statut** : DONE (code-complete + auto-QA — runtime à valider in-game, voir recap)
**Niveau BMAD** : Standard (3 crates, ~8 fichiers)
**Date** : 2026-06-02
**Origine** : Audit gap V1 → RPG V2 (session 2026-06-02). Catégorie 1 « quick-win » :
la couche de données RPG (`forgia-rpg-data`) existe mais n'est **pas câblée** au gameplay.

## Contexte (vérité terrain, lecture seule pré-impl)

- Player spawné une seule fois dans `forgia-player` (`OnEnter(AppMode::InGame)`, partagé
  FPS/Roguelite/RPG). **Pas** de `Inventory`/`QuestLog`/`XpProgress`.
- `ForgiaDialoguePlugin` câblé (`forgia-game:74`) → sessions tournent.
- `ForgiaInventoryPlugin` / `ForgiaQuestsPlugin` / `ForgiaXpCurvesPlugin` **ajoutés nulle part**.
- `ForgiaUiDialoguePlugin` (UI dialogue, `forgia-ui-lib::dialogue`) **pas ajouté** → choix invisibles.
- `advance_sessions` (`dialogue.rs:130`) : effets non routés (TODO).
- `QuestCatalogue` vide → `StartQuest("kill_goblins")` no-op.
- `advance_quests` émet `QuestCompleted` mais aucune récompense n'est grantée.
- `apply_xp_gain` consomme `XpGain` mais personne ne l'émet.

Boucle cible (sans combat, le RPG n'en a pas) :
**parler PNJ → reçoit quête + objet → valider objectif via dialogue → récompense XP + objet → niveau**.

## Acceptance Criteria

- [x] AC1 — `DialogueEffect::AdvanceQuest { tag, delta }` ajouté ; `advance_sessions` route
      `GiveItem`→`Inventory.add`, `StartQuest`→`QuestLog.start`, `AdvanceQuest`→`QuestProgress`.
- [x] AC2 — `ForgiaRpgDataPlugin` agrège inventory+quests+xp+dialogue ; remplace le solo dialogue dans forgia-game.
- [x] AC3 — `ForgiaUiDialoguePlugin` ajouté à forgia-game → panneau de dialogue visible + choix cliquables.
- [x] AC4 — Player reçoit `Inventory`/`QuestLog`/`XpProgress` `OnEnter(GameMode::Rpg)` (RPG-only, `Without<Inventory>`).
- [x] AC5 — `QuestCatalogue` peuplé (`kill_goblins`, objectif tag `kill_goblin` ×5, xp 120 + gold/potions).
- [x] AC6 — `grant_quest_rewards` sur `QuestCompleted` → émet `XpGain` + items (+ `InventoryEvent::Full` si plein).
- [x] AC7 — UI inventaire egui (touche I), gated `GameMode::Rpg`, lit les 80 slots (LOCK-INV-1 read-only confirmé).
- [x] AC8 — 0 warning clippy (rpg-data + rpg), `cargo check -p forgia-game` OK, 34 tests passent.

## Auto-QA (post-impl)

- **verifier** : VALIDÉ — LOCK-INV-1 intact (UI read-only), pas de double-add plugin, ids cohérents.
- **qa-lead** : 6 findings. BUG-570-04 (Majeur, loot perdu si inventaire plein) **corrigé** (capture
  retour + `InventoryEvent::Full`). BUG-570-01 (latence) corrigé via `.chain()`. BUG-570-03 (max_stack 99)
  documenté `TODO(item-registry)`. BUG-570-06 (debug choice orphelin) corrigé (self-start). BUG-570-02
  (sain dans la config actuelle, pas de changement). BUG-570-05 (`Local<bool>` non reseté à la ré-entrée
  RPG, cosmétique) → **follow-up**.

## Addendum 1 — PNJ de test (déclenchabilité)

Diagnostic runtime 2026-06-02 09:21 : `forgia2_npcs.json` → `npc_count_total: 0`. Les `Npc`/
`InteractablePoint` étaient **définis mais jamais spawnés** → boucle non-déclenchable.

## Addendum 2 — PNJ on-brand via le lineup (remplace les capsules)

Audit du système PNJ V1 (`world/merchants.rs::village_npc_spawn_system` + `spawn_npc` factory +
KayKit adventurers, placement demi-cercle autour du village). Constats V2 :
- V2 a ses **personnages on-brand** (`Dorin`/`Mira`/`L'Apprenti`/`Maître Forgeron Célèste`), pas
  les KayKit adventurers (absents).
- `character::spawn_character_lineup` les spawne **déjà** + attend la stabilisation du joueur
  (le teleport village décale de 20m+) → placement correct.
- 🔴 Les capsules de l'Addendum 1 spawnaient dans `spawn_world` **avant** le teleport →
  injoignables. **Supprimées.**

Décision : **les 4 personnages du lineup deviennent les PNJ** (Maître Forgeron = quêtes,
Mira = commerce, Dorin + Apprenti = lore). `Npc` + `InteractablePoint` attachés dans
`spawn_character_lineup` ; 4 arbres de dialogue keyés `npc_<name_snake>`. `TODO(story-445)` :
forgia-village-npc-spawner data-driven (comme V1 mais on-brand).

Binaires : 09:26 (capsules) → 10:21 (PNJ on-brand). Les runs 09:21/09:49 étaient en
**Arène/Roguelite** (pas RPG) → systèmes gatés `GameMode::Rpg` jamais tirés.

## Addendum 3 — 🎯 CAUSE RACINE binaire + ancrage au puits

**Cause racine des tests ratés** : `run_debug.bat` lance `forgia.exe` (package `forgia`,
root `src/main.rs` → `forgia_game::run_game()`), PAS `forgia-game.exe` (legacy). Je rebuildais
`-p forgia-game` → la lib se recompilait mais le bin `forgia` n'était **jamais relinké** → exe
stale. **Toujours `cargo build -p forgia`.** Mémoire :
`reference_forgia_exe_is_real_binary_not_forgia_game`.

Run 08:31 confirmé OK (log) : 4 trees + components + 4 PNJ spawnés. Mais placement
« derrière le joueur » → injoignables après que le joueur s'éloigne. **Corrigé** : nouvelle
`RpgVillageAnchor` (insérée par `spawn_world` = puits/centre village) ; `spawn_character_lineup`
place les 4 PNJ en **arc 90° rayon 3.5m autour du puits**, face au centre. Point fixe trouvable
(approche V1 demi-cercle autour du centre village). Binaire `forgia.exe` 10:49.

## Suite naturelle (hors scope story-570)

- Émetteurs `QuestProgress` réels (kill/collect/visit) — bloqué par l'absence de combat RPG.
- `ItemRegistry` (max_stack par item, noms d'affichage, icônes) → remplace le placeholder 99.
- Vendor/shop (économie) : `DialogueEffect::OpenShop` + UI marchand.
- Persistance save/load de `Inventory`/`QuestLog`/`XpProgress`.

## Invariants protégés

- **LOCK-INV-1** : 80 slots, pas d'expansion. L'UI affiche, ne change pas la capacité.
- Pas de pollution du player FPS/Roguelite (components RPG attachés OnEnter Rpg uniquement).
- Pas de cycle Cargo : routing intra-crate (forgia-rpg-data), wiring depuis forgia-rpg (dépend déjà de rpg-data).

## Test in-game

1. **Action** : lancer RPG, s'approcher d'un PNJ (Aldric/Lyra), **E** pour parler, cliquer les choix ; **I** pour inventaire.
2. **Redémarrage** : `cargo run` (`.rs` modifiés, pas hot-reload).
3. **Effet attendu** : panneau dialogue en bas, accepter la quête donne la dague (visible inventaire I),
   choix debug « j'ai tué les gobelins » complète la quête → niveau monte.
4. **Sensors** : `forgia2_inventory.json` (count items > 0 après GiveItem), `forgia2_quests.json`
   (status Active→Completed), `forgia2_npcs.json` (dialogue active state).
5. **Variantes si KO** : si choix invisibles → vérifier `ForgiaUiDialoguePlugin` ajouté ;
   si quête no-op → vérifier `QuestCatalogue` peuplé (id == effet `StartQuest`) ;
   si inventaire vide après don → vérifier `Inventory` greffé sur Player en RPG.
