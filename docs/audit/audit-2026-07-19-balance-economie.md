# AUDIT BALANCE / ÉCONOMIE / PROGRESSION ROGUELITE — 2026-07-19

> Complément « fondamentaux jeu » de l'[audit 360° code](audit-2026-07-19-checkup-360.md) du même jour.
> Agent economy-designer + contre-vérifications manuelles (exploits diamant & alias Or/Souls : **confirmés**).
> ⚠️ Tous les findings dans `forgia-mode-roguelite/src/*` sont **[CHAUD]** (WIP autre terminal) — re-vérifier les lignes post-merge.
> Structure de run auditée : `roguelite_stage_count=4` → 3 salles combat + 1 boss, 2 vagues/salle.

---

## Résumé exécutif

**L'équilibre in-run sur 4 salles est étonnamment bien réglé** (pression vs pouvoir joueur alignés à ~3-4 % près — la Trempe a exactement son sens). Les vrais problèmes sont ailleurs :

1. **2 exploits/fuites confirmés** : stack infini « Métal chaud » (stratégie dominante unique) + **diamant du parcours = boon gratuit qui bypasse le gating ET le coût** (un compte neuf peut toucher un légendaire).
2. **2 contenus morts vendus au méta-shop** : les boons « chaîne » étant inertes (bug C1 de l'audit code), le palier « Atouts Rares » à 200 Âmes vend un pool à moitié mort.
3. **Funnel contenu 2× trop lent** sur les 3 premières runs (1re arme à 4-6 runs, variété de build à 5-8 runs — réf. Gunfire : < 3 runs).
4. **~25 valeurs d'économie hardcodées** hors genome + **2 genomes fantômes** (`roguelite_loot.toml` 100 % mort, `roguelite_weapons.toml` mort confirmé) + genes Director/coop/pacing jamais consommés.
5. **Le revenu est plat alors que la pression est linéaire** : l'équilibre actuel est un *point*, pas une *courbe* — toute extension au-delà de 4 salles décroche vers la salle 5.

---

## 1. Boucles économiques (sources → sinks)

**5 monnaies** (+2 pseudo) :

```
OR (in-run, perdu 100 % à la mort — run.rs:839-843)
  SOURCES : kills Tank +5 / Sniper +3 / Runner +2 (run.rs:341-358) ·
            Pactole départ +50/rang (max 150) · parcours 6 coins ×16 = 96 (loot_room.rs:301-302)
  SINKS   : boon au Coffre 20-90 (boons.rs:492-501 — « souls_cost » payé en OR, cf. note) ·
            reroll 30 (coffre_forgeron.rs:30) · Trempe 20/28/39/55/77 = 219 au cap ·
            marchand ammo 30 / ammo_all 70 / heal 40 (roguelite_merchant.toml)

ÂMES (méta persistante, MetaSouls — run.rs:779)
  SOURCES : +5/vague, +25/boss (run.rs:771-774) · wisps ×2 (8 %/kill ; boss = 4 wisps) ·
            parcours Coin +10 / Star +25 (loot_room.rs:58-59)
  SINKS   : Enclume stats 1 330 · armes 60/150/250 = 460 · paliers boons 80/200/400 = 680 ·
            revive 15 (roguelite_merchant.toml:40). Total Enclume ≈ 2 470.

XP JOUEUR : source = 40 + 1/s survie. SINK = AUCUN (talent_points cul-de-sac, P5 non impl. — à tracer)
HP : très généreux (full-heal à CHAQUE break waves.rs:348-354 + stations + cœurs + heal marchand)
MUNITIONS : stations gratuites ×4 + marchand.
Pseudo : Trempe (per-run/arme), Maîtrise (+4 %/run SANS CAP — dégénéré, cf. §4).
```

**Note vérifiée** : `Souls as Gold` ([run.rs:24](../../crates/forgia-mode-roguelite/src/run.rs#L24)) — l'Or in-run EST `forgia_rpg_data::loot_tables::Souls` (design story-571). Pas un bug, mais un **piège de nommage** : le `souls_cost` des boons se paie en Or. La vraie anomalie est UX : le reroll affiche « ◇ » (symbole Âmes) mais débite l'Or.

**Anomalies de flux** :
- ❌ **Diamant parcours = boon GRATUIT** (vérifié [loot_room.rs:686-694](../../crates/forgia-mode-roguelite/src/loot_room.rs#L686-L694)) : index cyclique non seedé sur **tout** le catalogue, ignore `UnlockedBoonTiers` (story-616) et le coût. Fuite + bypass du funnel.
- ❌ **Popup « +OR » crédite des Âmes** (vérifié loot_room.rs:696-699 : `Coin` → `meta.current`) — et la même salle spawne des coins *Or* valeur 16. Deux « pièces » de devises différentes dans la même salle, labels divergents.
- ⚠️ XP/talents : source sans sink (progress.rs:32).
- ℹ️ Cœur remplace la pièce d'Or au drop (run.rs:327-340) : être low-HP réduit le revenu (~-5 Or/Tank). Choix intéressant, non documenté.

## 2. Funnel méta (runs par déblocage)

Revenu Âmes par issue : mort salle 1 ≈ 7-12 · salle 2 ≈ 17-22 · salle 3 ≈ 27-34 · **victoire ≈ 73** (+30-60 parcours).

| Déblocage | Coût | Runs (early = défaites) | Verdict (réf. Gunfire : 1er déblocage < 3 runs) |
|---|---|---|---|
| Pactole/Vitalité r1 | 15/20 | 1-2 | ✅ bon hook |
| **Bourrasque** (1re arme) | 60 | **4-6** | ⚠️ trop lent pour le 1er « waouh » |
| **Atouts Peu communs** | 80 | **5-8** | ⚠️ LA variété de build arrive trop tard |
| Lenoir / Rares / Boucherie / Légendaires | 150/200/250/400 | 2-6 victoires | ✅ long-tail sain |
| Enclume complète | 2 470 | 25-34 victoires | ✅ profondeur méta |

## 3. Difficulté vs récompense par profondeur

Pression : hp ×(1+0.35·s), dmg ×(1+0.15·s) (`roguelite_progression.toml:29-31`, appliqué `enemy_scaling.rs:184-198`). Revenu : **PLAT** (67 Or + 10 Âmes/salle, compositions fixes waves.rs:100-118).

| Salle | HP× | Dmg× | Or | Puissance joueur atteignable |
|---|---|---|---|---|
| 0 | 1.00 | 1.00 | 67 | ×1.00 |
| 1 | 1.35 | 1.15 | 67 | ~×1.30 |
| 2 | 1.70 | 1.30 | 67 | ~×1.65 |
| 3 (boss) | 2.05 | 1.45 | 8 | ~×2.00 |
| *extrap. 5* | 2.75 | 1.75 | 67 | ~×2.20 |
| *extrap. 9* | 4.15 | 2.35 | 67 | ~×2.60 → **décrochage** |

✅ Alignées sur 4 salles (déficit constant ~3-4 % = la Trempe mord juste). ❌ Revenu plat + coût Trempe géométrique (×1.4) : si `stage_count` monte (genome max = 12), décrochage dès ~salle 5. Tension intra-salle (67 Or vs ~90-140 de sinks possibles) : **bonne, à préserver**.

## 4. Balance armes (viewmodel_arena.toml + EXPLOSION_DAMAGE=70 hardcodé boucherie_rocket.rs:46)

EHP : Tank 260 · Runner 65 · Sniper 85 · Boss 1 150 (→ 2 358 salle boss).

| Arme | DPS burst | DPS soutenu | vs Pépin | TTK Boss (soutenu+matchup) |
|---|---|---|---|---|
| Pépin (28×6.0) | 168 | 105 | réf. | ~22 s ✅ |
| Bourrasque (11×11) | 121 | 76 | −28 % | ~31 s ✅ |
| M. Lenoir (50×0.8) | 40 body / 80 head | 29/57 | **−76 % body** 🚩 | ~27 s (pierce ×1.5) |
| Boucherie (70 AOE×0.9) | 63 mono | ~29 | **−62 %** 🚩 | ~34 s ✅ (+ poison shred) |

- **Lenoir — breakpoint fragile** : head 100 one-shot le Sniper s0 (85) mais PAS s1 (114.75) ; la Trempe 1 (×1.15 → 115) le récupère à **0.25 pt près**. Grille élégante mais non documentée : un +0.01 sur `hp_per_stage` la brise → test unitaire ou marge (`head_damage_mul` 2.0 → 2.2).
- **Boucherie** : l'arme la plus chère (250 Âmes) au DPS mono le plus bas — sa valeur (AOE+poison) n'est pas ce que le prix vend.
- **Maîtrise sans cap** (weapon_select.rs:258, saturating_add meta_shop.rs:350-353) : ×2.96 après 50 runs. Seul multiplicande non borné de la chaîne de dégâts (perm ×1.40 max · trempe ×1.75 · boons).

## 5. Économie des boons

Poids C 100 / U 45 / R 18 / L 6 → (tous paliers) 59.2 / 26.6 / 10.7 / 3.6 %. Compte neuf = 100 % Common.

| Boon | Rareté | Coût Or | %dmg-équiv/Or | Verdict |
|---|---|---|---|---|
| Métal chaud | C | 20 | 0.75 | **Dominant + stackable ∞** 🚩 |
| Étincelle Vorace | C | 25 | 0.40 | Strictement pire, même tag |
| Marteau du Charron | U | 40 | 0.75 | OK |
| Tornade de Braise | L | 80 | 0.94 | ✅ paie sa rareté |
| **Rebond du Caillou** | R | 60 | **0 — INERTE** 🚩 | 50 % du pool Rare mort (bug C1) |
| **Chaîne des Âmes** | L | 80 | **0 — INERTE** 🚩 | 20 % du pool Legendary mort |
| Cœur du Marteau | L | 90 | ~0 | Dévalué par le full-heal au break |
| Œil de Lynx | R | 60 | ~0.33 | Sous-efficient vs Champignon (C) |

- **Stack dégénéré** : doublons autorisés (boons.rs:230-231) + cumul multiplicatif (boons_apply.rs:56) → Métal chaud ×1.15⁶ ≈ ×2.31 pour 120 Or sur ~8 coffres/run. Stratégie dominante unique = anti-variété.
- Palier « Atouts Rares » 200 Âmes = fausse monnaie tant que le bug chaîne n'est pas fixé.
- Commentaire stale genome (roguelite_boons.toml:30) : « Boss=40 souls » — faux (cœur + 4 wisps).

## 6. Incohérences data/code

| # | Incohérence | Localisation | Gravité |
|---|---|---|---|
| 1 | `stage_count` genome 4 ≠ `RunGraphConfig::default()` 5 — un parse KO change la longueur de run | roguelite_run.toml:17 vs graph.rs:240 | 🟠 |
| 2 | **`roguelite_loot.toml` 100 % MORT** (0 consommateur, ennemis inexistants) — drops réels hardcodés run.rs:327-359 | — | 🟠 supprimer |
| 3 | Genes Director morts (`director_credits_*`, `stage_credits_budget` calculé jamais lu) — vagues hardcodées waves.rs:100-118 | roguelite_run.toml:53-101 | 🟡 |
| 4 | Genes coop + pacing morts (`max_players`, `revive_*`, `*_target_duration`) | roguelite_run.toml:117-177 | 🟡 |
| 5 | `boss_stage_index` parsé mais `boss_depth()` = total−1 | graph.rs:261 vs 185 | 🟡 |
| 6 | ~25 hardcodes éco (souls/wave, drops Or, cœurs, XP, base HP, reroll, maîtrise, **EXPLOSION_DAMAGE=70** = un dégât d'ARME hors genome, coins parcours, compositions vagues, chain/knockback) | consolidé §7 spec | 🟠 |
| 7 | Reroll affiche « ◇ » (Âmes) mais débite l'Or ; popup « +OR » crédite des Âmes | coffre_forgeron.rs:162 ; loot_room.rs:696-699 | 🟠 UX |

## 7. Recommandations chiffrées

**P0 — avant tout retuning** :
1. **Fixer les boons chaîne** (= C1 audit code : mutation `forgia_combat::Health`, pas DamageEvent).
2. **Cap doublons boons = 3** par id (ou prix ×1.5/doublon) — tue Métal chaud ∞.
3. **Cap maîtrise = 10** (max ×1.36) — ferme le fluage méta infini.
4. **Diamant parcours** : `CoffreRng` + gating `UnlockedBoonTiers` + coût 40 Or.

**P1 — funnel 2 premières heures** :
5. Bourrasque 60 → **40** Âmes ; palier Uncommon 80 → **50** (compensation : Lenoir 150 → 170 si besoin).
6. `souls_per_wave` 5 → **7** (mort salle 1 : ~10 → ~14 Âmes ; ratios Enclume préservés).

**P2 — courbes** :
7. `gold_per_stage = 0.15` (drops Or ×(1+0.15·s)) — indexe le revenu sur la pression, prérequis à toute run > 4 salles.
8. Test unitaire breakpoints Lenoir (`head(100×trempe) ≥ sniper_ehp(stage)`) OU `head_damage_mul` 2.0 → 2.2.
9. Cœur du Marteau 90 → 40 Or, ou 15 HP/kill + overshield.
10. `RunGraphConfig::default()` 5 → 4 (miroir exact du genome).

### Spec migration `roguelite_progression.toml` (story à créer — le fichier existe, on étend ; les `Default` Rust restent miroirs)

```toml
[souls]                      # run.rs:771-774, 442-448, 524
per_wave = 7                 # reco P1-6 (actuel 5)
per_boss = 25
wisp_value = 2
wisp_chance = 0.08
boss_wisp_count = 4

[gold_drops]                 # run.rs:341-358 (remplace roguelite_loot.toml mort → SUPPRIMER)
tank = 5
sniper = 3
runner = 2
gold_per_stage = 0.15        # reco P2-7 (actuel 0 implicite)

[hearts]                     # run.rs:327-340
boss_heal = 40
low_hp_heal = 20
low_hp_threshold = 0.40
low_hp_chance = 0.35

[player_xp]                  # progress.rs:21,49
run_base = 40
level_base = 80
level_growth = 40

[player]                     # meta_shop.rs:37 (unifier avec forgia-player)
base_hp = 100.0

[coffre]                     # coffre_forgeron.rs:30 + boons_apply
reroll_cost = 30
duplicate_cap = 3            # reco P0-2

[mastery]                    # weapon_select.rs:258 + meta_shop.rs:350
damage_per_level = 0.04
level_cap = 10               # reco P0-3

[parcours]                   # loot_room.rs:58-59, 302
coin_gold = 16
item_coin_souls = 10
item_star_souls = 25
```

Plus : `explosion_damage = 70` → `viewmodel_arena.toml [weapons.boucherie]` ; purge des genes morts de `roguelite_run.toml` (director/coop/pacing/seed_xor) OU stories de câblage explicites — un genome que le code ignore est un mensonge de data-driven.

---

*Contre-vérifications faites : diamant gratuit ✅ confirmé (loot_room.rs:686-694), alias Souls=Or ✅ confirmé (run.rs:24, design story-571 — anomalie = UX icône). Findings [CHAUD] à re-pointer post-merge. S'articule avec la Vague 1 de l'audit 360° (la migration genome ci-dessus EN FAIT PARTIE : mêmes fichiers, même story).*
