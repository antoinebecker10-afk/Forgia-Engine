# Best-practices — Wizard de pré-run Roguelite (sélection perso / arme / stats)

> **Audit internet + reco tailored Forgia** — 2026-06-23.
> Origine : demande user « comment fonctionnent les interfaces pour choisir
> ton personnage, ton arme, avec niveaux/dégâts/types de dégâts ; fais un audit
> internet et un rapport pour améliorer notre wizard ».
> Story de mise en œuvre : [story-612](stories/story-612-roguelite-weapon-select-wizard.md).

---

## 0. TL;DR

6 références du genre auditées (Gunfire Reborn, Roboquest, Hades, Borderlands,
Brotato, Dead Cells). Toutes les bonnes interfaces de sélection reposent sur
**4 briques** : *choix* (héros/arme) → *carte de stats lisible* (≤ 6 metrics) →
*type de dégât + rareté codés couleur* → *moment de comparaison* (delta vs équipé).

**État Forgia** : il n'y a **aucun wizard** — le joueur démarre avec un kit fixe
(Pépin par `Default`), zéro stat affichée. **Mais 90 % de la donnée existe déjà** :
stats de combat par arme ([`viewmodel_arena.toml`](../assets/genomes/viewmodel_arena.toml)),
4 éléments avec multiplicateurs vs archétypes ([`roguelite_elements.toml`](../assets/genomes/roguelite/roguelite_elements.toml)),
rareté dans le loot pool. Le travail n'est pas de *créer* de la donnée, c'est de
la *montrer*.

**Reco** : commencer par un **écran de choix d'arme de départ** (réutilise 100 %
de l'existant, surface les 4 éléments construits en story-582, différenciateur =
afficher le matchup élément↔ennemi que presque aucun concurrent ne montre).

---

## 1. ⚠️ Découverte critique (concept-first) — la vraie source de stats

Avant toute UI, le traçage producteur/consommateur a révélé un piège :

| Genome | Statut | Valeurs |
|---|---|---|
| [`viewmodel_arena.toml`](../assets/genomes/viewmodel_arena.toml) | ✅ **VRAIE source de combat** (lue par `forgia-fps` via `ViewmodelGenomeEntry`) | Pépin dmg **28** × **6/s** ; Bourrasque 11 × 11 ; Lenoir 50 × 0.8 ; Boucherie roquette |
| [`roguelite_weapons.toml`](../assets/genomes/roguelite/roguelite_weapons.toml) | ❌ **GENOME MORT** (0 consommateur Rust) | Pépin dmg **18** × 0.25 s/coup — diverge |

**Conséquence** : une carte de stats branchée sur `roguelite_weapons.toml`
**mentirait** sur les vrais dégâts. Le wizard DOIT lire `viewmodel_arena.toml`.

**Piège de nommage associé** : l'enum `WeaponType` est legacy V1 et ne matche pas
la persona V2 :

| `WeaponType` | Persona V2 réelle | Clé genome | Élément |
|---|---|---|---|
| `ModernAR` | Pépin (pistolet) | `pepin` | Explosif |
| `AssaultRifle` | Bourrasque (SMG) | `bourrasque` | Feu |
| `Shotgun` | **Madame Lenoir (sniper !)** | `madame_lenoir` | Perforant |
| `RocketLauncher` | Boucherie (lance-roquettes) | `boucherie` | Poison |

DPS réels (damage × fire_rate × pellets) : **Pépin 168 · Bourrasque 121 ·
Lenoir 40** (mais one-shot tête) **· Boucherie** = roquette 70 AOE (genome
`damage=0`, dégâts portés par l'explosion → cas spécial à étiqueter « AOE »).

---

## 2. L'audit internet — les 6 lois du genre

### Loi #1 — Chaque choix a une identité résumable en une phrase
Roboquest : « Commando = dégâts explosifs bruts », « Recon = mêlée + mobilité ».
Au moment du choix, on montre un **rôle**, pas des stats abstraites.

### Loi #2 — La rareté n'est pas qu'une couleur, c'est une promesse de complexité
Roboquest : 5 raretés (Common→Uncommon→Superior→Epic→Fantastic) ; plus c'est rare,
plus l'arme gagne d'affixes. Borderlands : nom coloré par rareté + jusqu'à 5 lignes
de bonus. La couleur sert de **pré-tri visuel** (< 200 ms).

### Loi #3 — Le DPS est la stat de comparaison universelle
Les dégâts seuls trompent (18 dmg × 4/s > 50 dmg × 0.5/s). Tous les bons tooltips
affichent un **DPS dérivé** comme score synthétique. Layout : ≤ 6 metrics,
2 colonnes, icônes custom, grille d'espacement stricte (4 px). Piège n°1 : noyer
le joueur d'infos ou cacher les stats clés → décision ralentie, flow cassé.

### Loi #4 — Le type de dégât = un badge visuel, jamais un nombre noyé
Borderlands : plaque élémentaire + `xN`. Dead Cells : l'icône **change de couleur**
selon le scaling (rouge/violet/vert). Idéalement montrer **« fort contre / faible
contre »** — rare et précieux, la plupart des jeux laissent deviner les matchups.

### Loi #5 — Donne une règle de scaling mémorisable
Dead Cells : « le DPS double tous les +5 stats ». Brotato : tiers I→IV, 2 armes
identiques fusionnent. Le joueur a une règle mentale simple, pas une formule opaque.

### Loi #6 — Comparer = afficher le delta, pas deux blocs côte à côte
Pattern canonique (Borderlands/Destiny) : à la prise, flèches ↑ vertes / ↓ rouges
vs l'arme équipée. Le cerveau lit « +12 dmg, −0.3 s recharge » instantanément.

---

## 3. État actuel Forgia (ancré code)

| Brique | État | Référence |
|---|---|---|
| Choix de héros | ❌ Aucun (1 perso « Apprenti » fixe) | — |
| Choix d'arme de départ | ❌ Aucun (Pépin via `Default`) | [run.rs sys_start_run](../crates/forgia-mode-roguelite/src/run.rs) |
| Stat block d'arme | ❌ Aucun (ni lobby, ni HUD) | [hud.rs draw_weapon_slots](../crates/forgia-mode-roguelite/src/hud.rs) |
| Données de stats | ✅ Existe (vraie source) | [viewmodel_arena.toml](../assets/genomes/viewmodel_arena.toml) |
| Types de dégâts | ✅ Existe (4 éléments + matchups) | [elements.rs](../crates/forgia-mode-roguelite/src/elements.rs) |
| Rareté | ⚠️ Loot pool only, pas affichée | `roguelite_loot.toml` |
| Comparaison à la prise | ❌ Aucune | [hud.rs](../crates/forgia-mode-roguelite/src/hud.rs) |
| Meta-shop (upgrades permanents) | ✅ Bon | [meta_shop.rs](../crates/forgia-mode-roguelite/src/meta_shop.rs) |

**Verdict** : pas un wizard incomplet — un **lobby sans wizard**. Dette faible car
la donnée est là.

---

## 4. Recommandations — phasé (aligné priorité SHIP, minimaliste)

### 🟢 Phase 0 — MVP (réutilise 100 % de la donnée) → **story-612**
Écran « Choix de l'arme de départ » au Lobby :
1. Choisir l'arme de départ parmi les 4 (au lieu de Pépin imposé).
2. Carte de stats lue de `viewmodel_arena.toml` : DMG · Cadence · **DPS** · Chargeur
   · Recharge · Portée (≤ 6 metrics, Loi #3).
3. Badge élément (Loi #4) + **« Fort vs / Faible vs »** depuis les multiplicateurs
   existants — *différenciateur*, enseigne le système d'éléments (story-582).
4. Une phrase d'identité par arme (Loi #1).
5. Synergie : le choix arme l'élément de départ (`sys_reset_element_unlocks` lit
   déjà `EquippedWeapons.current`).

### 🟠 Phase 1 — Lisibilité in-run
6. Stat block au survol/switch dans le HUD (Gunfire-like).
7. Comparaison à la prise de loot (Loi #6) : flèches ↑/↓ vs l'arme du slot.
8. Bordure de carte colorée par rareté (réutilise les couleurs du loot pool).

### 🟡 Phase 2 — Profondeur (quand le ship est sécurisé)
9. Affixes/inscriptions façon Gunfire/Roboquest (rareté → lignes de bonus, Loi #2).
10. Sélection de héros réelle avec rôle en 1 phrase, quand 2-3 héros existent.
11. Règle de scaling affichée liée aux upgrades meta-shop (Loi #5).

---

## 5. Maquette (Phase 0)

```
┌──────────────────────────────────────────────────────┐
│   CHOISIS TON ARME DE DÉPART                  ‹ 1/4 › │
├────────────────┬─────────────────────────────────────┤
│   PÉPIN        │  Élément :  EXPLOSIF (splash AOE)    │
│  Pistolet      │                                     │
│  ricocheur     │  DMG / coup     28                  │
│                │  Cadence        6.0 /s              │
│                │  DPS            168                 │
│                │  Chargeur       12                  │
│                │  Recharge       1.2 s               │
│                │  Portée         80 m                │
│                ├─────────────────────────────────────┤
│                │  Fort vs   Coureur   ×1.4           │
│                │  Faible vs Boss      ×1.0           │
├────────────────┴─────────────────────────────────────┤
│  ‹ PÉPIN ›  BOURRASQUE   LENOIR   BOUCHERIE           │
│        ← → choisir   ·   ENTRÉE lancer la run        │
└──────────────────────────────────────────────────────┘
```

---

## 6. Mapping data : existe vs à ajouter

| Élément UI | Source | À ajouter ? |
|---|---|---|
| DMG, Cadence, Chargeur, Recharge, Portée | `viewmodel_arena.toml` | ✅ existe |
| **DPS** | `damage × fire_rate × pellets` | 🔧 fn pure (cas roquette `damage=0` → « AOE ») |
| Badge élément | `ElementConfig` (Resource) | ✅ existe |
| Fort/Faible vs | multiplicateurs matchup | ✅ existe → max/min sur 4 archétypes |
| Couleur persona | `hud::speaker_color` | ✅ existe |
| État du choix | — | 🔧 `StartingWeaponChoice` Resource |
| Application au start | `EquippedWeapons.current` | 🔧 `OnExit(RunState::Lobby)` |

---

## 7. La grande idée

Le moteur a déjà payé en code un système d'éléments riche (4 types + matchups vs
archétypes) que **le joueur ne voit jamais**. Le wizard n'est pas cosmétique :
c'est ce qui rend *visible et choisi* un système déjà construit. C'est
`observability-required.md` appliqué au game design — une feature que le joueur ne
voit pas n'existe pas.

---

## Sources (audit internet, 2026-06-23)

- [Gunfire Reborn — Weapon Stats](https://gunfirereborn.fandom.com/wiki/Weapon_Stats) · [Inscriptions/rareté](https://gunfirereborn.fandom.com/wiki/Inscriptions) · [Hero Stats](https://gunfirereborn.fandom.com/wiki/Hero_Stats)
- [Roboquest — Weapons (raretés & affixes)](https://roboquest.miraheze.org/wiki/Weapons) · [Classes ranked (rôles)](https://www.thegamer.com/roboquest-classes-ranked/)
- [Borderlands — Item Card](https://borderlands.fandom.com/wiki/Item_card) · [Borderlands 2 Weapons](https://borderlands.fandom.com/wiki/Borderlands_2_Weapons)
- [Hades — Boons UI & raretés](https://hades.fandom.com/wiki/Boons)
- [Brotato — Weapons (tiers I-IV)](https://brotato.wiki.spellsandguns.com/Weapons) · [Characters](https://brotato.wiki.spellsandguns.com/Characters)
- [Dead Cells — Stats & color scaling](https://deadcells.wiki.gg/wiki/Stats)
- [Designing Weapon Stat UI Screen (Medium)](https://medium.com/@r4ravikumar/designing-weapon-stat-ui-screen-for-competitive-shooter-game-94f29c305dae)
- [Game UI Database — comparison & boon selection patterns](https://www.gameuidatabase.com/index.php?tag=13)
