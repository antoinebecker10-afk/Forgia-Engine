# Story-645 — R3 : sceller la boucle (Victory + maîtrise + best-run + boons pondérés)

> **Source** : `docs/audit/rapport-gunfire-like-identite-2026-07-02.md` §R3.
> **Scale BMAD** : Standard (forgia-mode-roguelite + forgia-rpg-data).
> **Statut** : ✅ DONE 2026-07-02 — commits `cab6acd` (R3.1) + `ce21f93` (R3.3) + `214a68d` (R3.4).
> 244 tests verts (forgia-mode-roguelite ; + forgia-rpg-data à côté), clippy 0-warn
> touché, binaire compile.

## Livré
- **R3.1 Victory** (`cab6acd`) : le portail Return du parcours émet `EndRunEvent(Victory)`
  (garde `boss_defeated`) — l'écran de victoire, le flush save et l'XP vivaient déjà,
  seul l'émetteur manquait (story-603 close).
- **R3.2 Maîtrise d'arme** : DÉJÀ câblée (audit périmé) — level-up `OnEnter(Defeat|Victory)`,
  `WeaponMasteryMods` composé multiplicativement, niveau affiché au wizard. Le fix R3.1 a
  débloqué le level-up sur victoire. Dette : `WEAPON_MASTERY_DMG_PER_LEVEL` const → genome.
- **R3.3 Best-run persistant** (`ce21f93`) : `MetaShopSave.{best_victory_secs, runs_played,
  victories}` (+`record_run_result` pur) figés `OnEnter(Defeat|Victory)` avant le flush ;
  overlay Victory affiche chrono + « 🏆 NOUVEAU RECORD » + victoires/runs.
- **R3.4 Tirage pondéré** (`214a68d`) : `roll_candidates_weighted` (marche cumulative,
  sans remise, poids 0 = exclu) sur les 2 tirages prod (Coffre + Loot Room) ; genome
  `[rarity_weights]` (100/45/18/6) hot-reload, miroir `RarityWeights::default()`.
- Constat : l'empilement des boons était **déjà multiplicatif** (boons_apply.rs `*=`).

## Déféré
- **Boons à trade-off** : nécessite un schéma multi-effets (`effects: Vec<...>`) → story dédiée.
- Affichage best-run au Lobby/accueil (overlay fin de run seulement pour l'instant).
