# Story-585 — Choix de boon 1-parmi-3 au portail de fin de zone (agency Hadès)

**Statut** : EN COURS
**Niveau BMAD** : Standard
**Date** : 2026-06-09
**Cible** : SHIP Roguelite — ferme le Gap #2 (boons mécaniques = raison de refaire un run). Décision design #3 ([[project_roguelite_element_progression_design]]) : « choix 1-parmi-3 au portail de fin, agency Hadès/Gunfire ».

## Demande
Au portail de fin de zone du parcours, présenter **3 améliorations (boons)** au choix ; le joueur en prend **1 (gratuit)** puis enchaîne vers la zone suivante. Distinct du **Coffre du Forgeron** (shop, coûte des âmes, fin de wave en arène — story-558).

## Réutilisation (rien à refaire côté boons)
- `forgia_rpg_data::boons::roll_candidates(catalogue, active, count, next_index)` → tire N candidats (filtre légendaire, sans doublon).
- `ActiveBoons::apply(def, catalogue)` → applique le boon (recompute `PlayerCombatMods` via `sys_recompute_boon_mods`).
- `BoonsCatalogue` (genome TOML), `CoffreRng` (aléa), `BoonDef` (id/name/rarity).

## Implémentation (self-contained `forgia-mode-roguelite`)
| Fichier | Rôle |
|---|---|
| `loot_room.rs` | Resource `ZoneReward{phase, candidates, target}` ; portail `Next` → `NeedRoll` (au lieu de TP direct) ; `sys_roll_zone_reward` (roll 3, fallback TP si pool vide) ; `sys_zone_reward_pick` (touche 1/2/3 → `apply` + TP + close) |
| `hud.rs` | `draw_zone_reward_cards` (EguiPrimaryContextPass) : 3 cartes (nom + rareté + couleur) + « Appuie 1 / 2 / 3 » |

Choix **gratuit**, sélection **clavier 1/2/3** (pas de curseur à libérer pour v1). Le Coffre-shop reste inchangé.

## QA
- [ ] `cargo check`/clippy 0 (build isolé du foliage cassé autre terminal)
- [ ] Runtime : portail zone 1→2 → 3 cartes → 1/2/3 → boon appliqué (panneau AMÉLIORATIONS) → TP zone 2

## Reste / suite
- [ ] Pause + curseur cliquable (v2)
- [ ] Cartes = déblocage/tier d'**élément** (582) en plus des boons stat
- [ ] VFX de sélection
