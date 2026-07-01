# Audit de parité Gunfire Reborn ↔ Forgia Roguelite

> **Objet** : cartographier système par système l'écart entre Gunfire Reborn (référence de design) et l'état courant de Forgia Roguelite, HUD compris, pour piloter la réplication complète.
> **Date** : 2026-07-01 · **Périmètre** : 7 systèmes · **Workspace** : `c:\Users\Antoi\Desktop\Forgia Rewrite`
> **Cible produit** : SHIP le Roguelite (FPS type Gunfire Reborn). Priorité absolue CLAUDE.md §1.

---

## 1. Résumé exécutif

Forgia a une **fondation roguelite solide et observable** (économie dual-monnaie, boons empilables, wave/boss loop, éléments + Combustion, HUD ammo/minimap/slots/monnaie). Le codebase est majoritairement data-driven et instrumenté (sensors `forgia2_*.json`), ce qui **dépasse Gunfire** côté outillage et permet de tuner la parité par mesure.

Les écarts se concentrent sur **quatre piliers structurants**, tous absents ou incomplets, qui empêchent le jeu de "se lire comme Gunfire" :

1. **Modèle de défense ennemie tri-couche** (Vie rouge / Bouclier bleu régénérant / Armure jaune) — totalement absent. C'est le pilier qui donne un sens au système élémentaire, au HUD ennemi et au choix d'arme. Forgia matche par **archétype** (Tank/Runner/Sniper/Boss), pas par **couleur de défense**.
2. **Les 3 réactions élémentaires** — Forgia n'a que **Combustion** (1/3). Manquent **Miasma** et **Manipulation**, plus l'élément **Foudre/Choc** qui les conditionne.
3. **Le système d'inscriptions d'arme** (1–5 modificateurs par instance, tiers vert/bleu/orange, scaling par acte) — c'est LE cœur de la puissance Gunfire, totalement absent (Forgia = 4 armes-types fixes).
4. **La Compétence Secondaire** (touche Q, charges/Supplies) — Forgia n'a qu'une compétence (F), et elle **empile deux systèmes contradictoires** (état Ultime 10s + sort instantané) sur la même touche.

**Constat transversal** : plusieurs systèmes Forgia divergent de Gunfire par **choix de design assumables** (Ultime 10s, matchup par archétype, 4 éléments dont Poison/Explosif/Perforant, shop plat vs arbre 6 branches). La roadmap ci-dessous sépare ce qui est **du contenu manquant** de ce qui est **une décision de design à trancher** (§Décisions ouvertes).

### Tableau scorecard de parité

| # | Système | Parité | Priorité | Verrou principal |
|---|---------|:------:|:--------:|------------------|
| 1 | **Éléments & réactions** | **38 %** | **P0** | Moteur de réactions générique + Foudre/Choc + Miasma/Manipulation |
| 2 | **HUD & interface** | **72 %** | **P1** | Bloc défensif Bouclier/Armure segmenté (dépend de la mécanique) |
| 3 | **Compétences de héros** | **35 %** | **P1** | Trancher les 2 systèmes sur F + Compétence Secondaire (Q) |
| 4 | **Ascensions & Scrolls (build)** | **38 %** | **P1** | Empilement multiplicatif + tirage pondéré + Coffre gratuit |
| 5 | **Armes** | **32 %** | **P2** | Système d'inscriptions (instance d'arme vs type fixe) |
| 6 | **Ennemis, défenses, boss, difficulté** | **48 %** | **P0** | Couches Vie/Bouclier/Armure + difficulté Réincarnation |
| 7 | **Économie & méta-progression** | **55 %** | **P2** | Vaults + talent-gate Exploration + arbre 6 branches |

> **Parité moyenne pondérée ≈ 45 %.** Les deux P0 (Éléments + Défenses) sont **couplés** : la défense tri-couche est le prérequis du vrai système élémentaire. Les traiter ensemble débloque la plus grande part de parité.

---

## 2. Systèmes en détail

---

### Système 1 — Éléments & réactions élémentaires — **38 %** — P0

#### (a) Spec Gunfire (chiffrée)

- **3 éléments**, chacun un statut de **durée fixe 5 s** :
  - **Feu → Brûlure** : DoT = **20 % des dégâts du tir/s** pendant 5 s (~100 % du tir cumulé). Matrice : **+50 % vs Vie**, −25 % vs Bouclier/Armure.
  - **Foudre → Choc** : **+10 % de dégâts de toutes sources** (flag actif/inactif) pendant 5 s. Matrice : **+50 % vs Bouclier**, −25 % vs Vie/Armure. Petit zap AoE à l'impact.
  - **Corrosion → Décomposition** : **−50 % vitesse de déplacement** 5 s (PAS de DoT propre). Matrice : **+50 % vs Armure**, −25 % vs Vie/Bouclier.
- **Application par tir** selon un **elemental effect chance** propre à l'arme (ex. 30 % Décomposition, 5 % Choc) — attribut d'arme, pas un taux global.
- **3 fusions** (2 statuts co-présents sur une cible) :
  - **Combustion (Feu+Corrosion)** : burst **200 %** sur cible + **100 %** dans **5 m**, stagger. Re-trigger **0,12 s/ennemi** (datamined).
  - **Miasma (Foudre+Corrosion)** : DoT **True** = **9 % PV max/s**, 5 s, **stack ×9**. Scaling : **0,75 %/s vs boss**, **0,3 %/s vs élites**.
  - **Manipulation (Feu+Foudre)** : charme 5 s (l'ennemi attaque ses alliés). **Élites/boss immunisés.**
- **Inscription Gemini** : partage le type de dégâts entre 2 armes en conservant le taux de chacune.

#### (b) État Forgia actuel

Système **fonctionnel et complet pour la voie Gunfire mono-réaction** (`crates/forgia-mode-roguelite/src/elements.rs`, stories 582/588/589/611) :

- **4 éléments** : `Fire, Poison, Explosive, ArmorPierce` (`elements.rs:45-55`).
- **Matchup par archétype** Tank/Runner/Sniper/Boss (`elements.rs:162-180`), données TOML.
- **Burn** DoT plat 8 dps / 3 s non-stackant (`elements.rs:429-435`) ; **Poison** stackant max 5, shred armure (`elements.rs:440-446`) ; **ArmorPierce** exécution <25 % PV (`elements.rs:616-632`) ; **AOE Explosif** rayon 3,5 m (`elements.rs:206-210`).
- **Combustion (Feu+Poison)** : target 200 % / zone 100 %, garde les statuts, re-trigger 0,8 s/cible (`elements.rs:222-248`, `754-786`).
- Data-driven hot-reload (`roguelite_elements.toml`), sensors `forgia2_elements.json` + `forgia2_element_vfx.json`, 21 tests, VFX Hanabi/flipbook (`element_vfx.rs`, `status_vfx.rs`).

#### (c) Écart

**Matche** (9) : trigger de fusion par co-présence de 2 statuts (`elements.rs:755-757`) ; Combustion alignée (200 %/100 %, AoE, re-trigger par cible) ; élément appliqué par tir via mapping arme→élément ; hot-reload ; feedback 100 % world-space ; observabilité complète.

**Partiel** (6) :
- **Burn** : plat 8 dps/3 s → Gunfire = **20 % du tir/s pendant 5 s**. Passer `duration 3→5` et dériver le DoT du tir déclencheur (`ev.damage × 0.20`), stocker le dps par-hit dans `StatusBurn`.
- **Moteur de réactions hardcodé** : seul `now_burn && now_poison` détecté (`elements.rs:755-756`). Généraliser en **set de statuts + table de paires → effet**.
- **Matrice matchup** : par archétype, pas par couche de défense (axe de design différent — cf Système 6).
- **Poison ≠ Décomposition** : Gunfire Corrosion = slow, pas DoT. Le DoT %HP vient de Miasma.
- **Re-trigger** : 0,8 s Forgia vs **0,12 s** Gunfire ; rayon 3,5 → 5,0 m.
- **HUD ennemi** : pas de barre segmentée ni d'icônes de debuff (cf §3 HUD).

**Manque** (structurant) :
- **Miasma** (Foudre+Corrosion) : DoT True 9 % PV max/s, stack ×9, scaling boss/élite — **effort M**.
- **Manipulation** (Feu+Foudre) : charme 5 s, immunité élites/boss — **effort L** (touche l'IA de re-targeting).
- **Élément Foudre + statut Choc** (+10 % vulnérabilité) — **effort M** — **prérequis bloquant des 2 réactions**.
- **Statut Décomposition** = slow −50 % — effort S.
- **Elemental effect chance par tir** (proc probabiliste via `CombatRng`) — effort S — change fortement le feel.
- **Durée uniforme 5 s** — effort S (casse l'équilibrage actuel).
- **Stagger sur Combustion** — effort S.
- **Gemini** — effort M — reporter (dépend d'un système d'inscriptions).

#### (d) Plan d'alignement

1. **Généraliser le moteur** : `HashSet<StatusKind>` par cible + `ReactionTable: (StatusKind, StatusKind) → ReactionEffect`. Sans ça, chaque réaction = un bloc hardcodé. **C'est la refonte-clé.**
2. **Ajouter `Element::Shock` + `StatusShock`** (flag +10 % vulnérabilité, 5 s) côté résolution de dégâts (`forgia-fps`/`forgia-combat`). Prérequis de Miasma ET Manipulation.
3. **Miasma** (True damage, ignore matchup, stack ×9, scaling par archétype) — trivial une fois moteur + Shock en place.
4. **Manipulation** — re-targeting IA (charme) + gate immunité par archétype.
5. Recaler Burn (20 % du tir/5 s), re-trigger 0,12 s, rayon 5 m via TOML.
6. **Proc par tir** : champ `effect_chance` par arme + roll `CombatRng` (déjà déterministe).

---

### Système 2 — HUD & interface — **72 %** — P1

#### (a) Spec Gunfire (layout écran complet)

- **Bas-gauche** : bloc défensif — barre **Vie (rouge)** + **Bouclier (bleu, recharge hors combat, vire orange ~1 s avant rupture)** ou **Armure (jaune, Qing Yan)**. Orange plein = total Vie+Bouclier.
- **Bas-centre** : 2 icônes de compétence — **Primaire E** (cooldown radial) + **Secondaire Q** (charges). Ressource héros collée à l'icône Primaire (Qi=nombre / Énergie=jauge / Blade Heart=nombre).
- **Bas-droite** : munitions **chargeur/réserve** + type codé couleur (**Normal vert / Lourd bleu / Spécial jaune**) + slots armes (1=Arme I, 2=Arme II, 3=Foundry infini).
- **Centre** : crosshair **unique par arme, dynamique** (rétrécit au 1er tir, s'élargit en rafale) + gimmicks (charge/marque/reload) + hitmarkers + damage floaters.
- **Au-dessus des ennemis** : barre tricolore (rouge/bleu/jaune) ; **boss = barre large dédiée + nom**.
- **Haut** : minimap + Copper (cap 65 535). **Haut-droite** : défis de vault/stage.
- **Non persistant** : Ascensions (3×6) + Scrolls (148) → **TAB (sac perso)** / **C (info équipe)**. Soul Essence encaissée en fin de run.

#### (b) État Forgia actuel

Architecture 2 tiers (`forgia-mode-roguelite/src/hud.rs` + `forgia-ui-lib`). **~80 % implémenté** :

| Élément Gunfire | Forgia | Fichier |
|---|---|---|
| Munitions bas-droite (mag/réserve, low-ammo pulse, RELOADING, ∞) | ✅ | `hud_ammo/mod.rs:83-226` |
| Slots armes bas-droite (max 4, actif OR, grisés, hotkey) | ✅ | `hud.rs:1002-1112` |
| Reload arc circulaire | ✅ | `hud_ammo/mod.rs:311-324` |
| Minimap haut-gauche (radar, blips par archétype, flèche joueur) | ✅ | `hud.rs:896-993` |
| Double compteur monnaie (Or + Âmes) haut-droite | ✅ | `hud.rs:118-195` |
| Compétence F (anneau + arc + secondes, couleur persona) | ✅ | `hud.rs:1329-1405` |
| Barres de vie ennemies 3D world-space (billboard) | ✅ | `forgia-enemy-nameplate/src/lib.rs` |
| **Damage numbers flottants + hitmarker** (l'état les listait "absent" — **FAUX, ils existent**) | ✅ | `forgia-game/src/lib.rs:110` |
| Crosshair centre (croix+dot+ticks) | ✅ (statique) | `forgia-crosshair/src/lib.rs` |
| Barre de vie joueur bas-gauche + portrait + badge nom/niveau | ✅ (mono-couleur) | `player_hp.rs`, `hud.rs:1118-1234` |
| Panneaux à la demande (boons, cartes récompense, coffre modal) | ✅ | `hud.rs:233-318`, `1412-1526`, `coffre_forgeron.rs` |
| Overlays fin de run defeat/victory + récap Âmes | ✅ | `hud.rs:367-560` |

#### (c) Écart

**Partiel** (5) :
- **Crosshair** statique → rendre dynamique par arme (spread = f(cadence+recoil), style par `WeaponType`), genome-driven.
- **Barre HP joueur** mono-couleur → préparer segments empilés pour l'arrivée du bouclier.
- **Portal / choix de zone** : `draw_portal_overlay` = stub no-op (`hud.rs:618`), dormant depuis refactor 471-479. `PORTAL_KEYS` + `stage_kind_display` existent (dead_code) → re-câbler sur un `BossPortal`/`Zone` event.
- **Ressource héros** : hearts confiance/énergie existent mais pas "collés à l'icône Primaire".
- **Barks** (bulles) : élément Forgia propre, hors parité stricte — garder texte-only.

**Manque** :
| Élément | Effort | Risque |
|---|:---:|---|
| **Barres défensives Bouclier (bleu) + Armure (jaune)**, segmentées, bleu→orange avant rupture | **L** | Élevé — d'abord une **mécanique** (composant Shield sur Health, recharge hors combat), pas un skin. Touche hot path combat + netcode. |
| Type de munition codé couleur (Normal vert/Lourd bleu/Spécial jaune) | S | Faible — enum `AmmoKind` par arme (genome) + teinte |
| **Barre de boss dédiée** (large, centre-haut, nom) | M | Moyen — `Resource BossHealthBar`, identifier l'entité boss |
| 2 icônes de compétence (Primaire E + Secondaire Q) | M | Dépend du design gameplay (Système 3) |
| Barre ennemie segmentée (bleu/jaune) | M | Dépend de la mécanique Shield/Armor (à faire après) |
| TAB sac perso / C info équipe | M | **Conflit keybind** : TAB réservé au gameplay (CLAUDE.md §1) — arbitrer |
| Légende minimap (archétype→couleur) | S | Faible |
| Toggle masquer le HUD (captures) | S | Faible — QoL |

#### (d) Plan d'alignement

**P1 — bloc défensif** (imposé par la dépendance) :
1. Composant **`Shield`** (+ `Armor` optionnel) sur l'entité joueur dans `forgia-combat`, recharge hors combat **data-driven** (genome, jamais hardcodé), couplage élémentaire (Foudre>bleu / Corrosion>jaune / Feu>rouge).
2. Ré-architecturer `player_hp.rs` en **barre segmentée empilée** rouge=Vie / bleu=Bouclier + signal **orange ~1 s** avant rupture.
3. Sensor `forgia2_shield.json` + health alert (`observability-required.md`).
4. **Scale-up BMAD Standard** (forgia-combat + forgia-ui-lib + netcode replicated) → **story obligatoire**.

**P2 en parallèle (faible risque, data-driven)** : type de munition couleur (S) + barre de boss dédiée (M) + légende minimap (S).

---

### Système 3 — Compétences de héros — **35 %** — P1

#### (a) Spec Gunfire (chiffrée)

- **Structure canonique** : **1 Primaire (cooldown, 1 charge, ~14–16 s)** + **1 Secondaire (charges/Supplies, base 3, PAS de cooldown)** + **1 Dash (Shift)** + passifs. **Pas d'ultime séparé** — la Primaire EST la grosse compétence.
- Cooldowns : Energy Orb 15 s, Leap 15 s, Tidal Aspis 14 s, Fatal Current ~16 s.
- **Réduction cooldown** : Efficient Casting −25 %, Ancient Timer −50 %, Rapid Cast −80 % ; conditionnelle (Terrific Crossfire −1 s/Smoke Grenade).
- **Multi-charges** (Power Source : jusqu'à 6 charges rechargeant une par une).
- **Ressources spéciales** : Fist Sensation (stacks/mètre), Fatal Current (timer 5 s), Dual-Wield (timer 20 s), Armure Qing Yan (jauge jaune). **Le "Qi de Qian Sui" N'EXISTE PAS.**
- Touches : **E** Primaire, **Q** Secondaire, **Shift** Dash.

#### (b) État Forgia actuel

Modèle **divergent** : État Ultime 10 s + cooldown 25 s + techniques par arme (T1/T1b/T2/T3/T4a DONE, T4b en cours) :

- **`UltimateState`** (`forgia-combat/src/ultimate.rs:22-120`) : timer 10 s + cooldown 25 s, 6 tests.
- **Input F** (`shockwave.rs:114-151`) : tente `try_activate()` de l'Ultime **puis** le sort F instantané — **deux systèmes sur la même touche**.
- **Sort F par arme** (`shockwave.rs:175-260`) : Pépin heal+push / Bourrasque repousse / Lenoir rayon / Boucherie AOE, cooldown par `WeaponType`.
- **Techniques d'Ultime** (`ultimate_apply.rs:71-258`) : Explosion / Chaîne / Perforation+Poison / Gel pendant les 10 s.
- Genome hot-reload (`ultimate_config.rs`, `roguelite_ultimate.toml`), sensors `forgia2_ultimate.json` + `forgia2_ultimate_tech.json`.
- **Dash** : `forgia-player/src/dash.rs` — bond 4 m, 2 charges, recharge 1,5 s/charge, mais **bindé double-tap Espace** (`PlayerAction::Jump`), alors que le HUD affiche "⇧ DASH".

#### (c) Écart

**Matche** (8) : compétence à cooldown existe (sort F par arme, 7–12 s) ; recharge auto dans le temps ; dash présent ; HUD radial de cooldown ; feedback d'état empoweré (bandeau) ; identité par arme data-driven ; genome/hot-reload ; observabilité.

**Partiel** (5) :
- **Deux systèmes sur F** : `UltimateState` (10 s) + `ShockwaveAbility` (instantané), même `just_pressed(F)`. Gunfire = 1 Primaire. **Trancher** (cf Décisions ouvertes).
- **HUD double** : `draw_shockwave_indicator` + `draw_ultimate_banner` = 2 widgets → unifier en 1 icône.
- **Dash bindé Espace vs HUD dit Shift** → aligner sur Shift ou corriger le hint.
- **Icône Dash** : seul un hint texte, pas d'icône + pips de charges (données `DashState` dispo).
- **Cooldown fixe 25 s** vs réduction conditionnelle Gunfire → exposer `CooldownMul` modifiable par boon.

**Manque** :
| Élément | Effort | Risque |
|---|:---:|---|
| **Compétence Secondaire (touche Q)** — 2e skill actif absent | **L** | Système gameplay complet + conflit avec l'état F |
| Ressource **Supplies** (pickup au sol, charges base 3) | M | Touche loot/pickup + compteur répliqué |
| Compteur de charges numérique Secondaire (HUD) | S | Dépend des 2 ci-dessus |
| Ressource spéciale par arme au HUD | M | Méca différente par arme |
| Réduction cooldown conditionnelle inter-skills | M | Nécessite la Secondaire |
| Multi-charges de la Primaire (Power Source) | M | Refactor cooldown→charges (comme DashState) |
| Passifs/talents propres à l'arme | L | Hors chemin critique |

#### (d) Plan d'alignement

1. **Trancher l'architecture F d'abord** (P1) : (a) mapper F/E sur le **sort instantané par arme seul** comme Primaire (1 charge, cooldown 14–16 s), (b) unifier le HUD sur `draw_shockwave_indicator` (retirer le bandeau plein écran doublon), (c) corriger le binding/hint Dash. **Réserver l'état 10 s à un futur talent.**
2. Puis livrer la **Compétence Secondaire (Q)** + charges/Supplies — le vrai gap structurant.
3. HUD : icône Primaire (E) + icône Secondaire (Q, compteur charges, **sans** anneau) + icône Dash (Shift, pips).

---

### Système 4 — Ascensions & Scrolls (build) — **38 %** — P1

#### (a) Spec Gunfire (chiffrée)

- **Ascensions** : à chaque fin de stage, **Goblet of Power** = choix **1 parmi 3** dans le pool **spécifique du héros**. **Empilables multiplicativement** (3× +20 % = **+73 %**, pas +60 %). **Perdues en fin de run** (win/lose). ~20–24 par run complète.
- **Occult Scrolls** : **148**, boons globaux, **5 raretés** (Normal bleu, Rare violet, Legendary orange, **Cursed rouge** = que des malus, Enhanced). Achat **Peddler** (Copper) ou **Vaults**. Beaucoup ont un **trade-off** (bonus + malus). Nettoyage : Cleansing Spring / Reincarnation.
- **Talents** = méta permanente en Soul Essence, **6 arbres** (Expedition/Battle/Skill/Survival/Weapon/Hero). Dimension Pouch retient 100 essence/palier.
- **Synergie** : choisir un thème, empiler, boucler des loops auto-entretenues.

#### (b) État Forgia actuel

Système **"Boons"** (architecture Hadès) — stories 529/558/591/616 :

- **`BoonDef`** (`forgia-rpg-data/src/boons.rs:133-162`) : id/name/effect/tags/rarity/weapon_filter/souls_cost.
- **`BoonsCatalogue`** TOML hot-reload (`roguelite_boons.toml`) : 18 boons (5 Common + 3 Uncommon + 2 Rare + 5 Legendary).
- **7 effets** : DamageMul/FireRateMul/HealOnKill/DamageReduction/Knockback/ChainTargets/FlatBonus.
- **Unlock légendaire par 3 tags identiques** (`boons.rs:20-24`).
- **Coffre du Forgeron** modal (`coffre_forgeron.rs`), coût en Âmes par rareté, reset run (`boons.rs:209`).
- **Enclume** méta (`meta_shop.rs`) : 4 upgrades plats + déblocages armes/paliers boons persistants.
- **Loot room** portail (`loot_room.rs`) : choix gratuit éléments/boons.

#### (c) Écart

**Matche** (10) : boucle 1-parmi-3 par wave clear (`waves.rs:284`) ; modal cartes rareté colorée ; boons empilables ; reset run ; 4 tiers de rareté ; méta persistante en Âmes ; monnaie Âmes droppée par kills/boss ; déblocages permanents ; synergie tags→légendaire ; reroll dispo.

**Partiel** (6) :
- **Empilement multiplicatif** : les mul-factors se composent mais FlatBonus/DamageReduction additifs → **garantir + exposer** le multiplicatif (afficher "+73 %" cumulé, ajouter un test).
- **Tirage uniforme** (`boons.rs:296` "rarity weighting can come later") → **pondérer par rareté** + genome de poids.
- **2 sources non unifiées** : Coffre payant (Âmes) vs Loot Room gratuit → **rendre le Coffre wave-clear gratuit** (modèle Goblet), réserver le paiement à un futur Peddler.
- **Catalogue 18 vs 148** → étendre vers ~40–60 à effet distinct.
- **Boons weapon-specific** : `weapon_filter` existe mais catalogue générique.
- **HUD boons actifs / stacks** : pas de panneau d'inventaire persistant (données `tag_counts` déjà là).

**Manque** :
| Élément | Effort | Risque |
|---|:---:|---|
| **Trade-off / malus** sur boons (bonus vs −PV/−regen/−speed) | M | Feel — éviter double-compte dans `sys_recompute_boon_mods` |
| Scrolls **Cursed** + nettoyage (Cleansing Spring/Reincarnation) | M | Dépend du trade-off |
| Marchand **Peddler** distinct (Copper, reroll 1×) | L | Économie de run |
| **Vaults** (source primaire de scrolls) | L | Level design + `forgia-stage` (multi-terminal) |
| Stacks élémentaires runtime (+3 %/stack, max 15, 5 s) | M | Hot path combat |
| Talents en **6 arbres** vs shop plat | L | Design > technique (cible 14 ans) |
| Dimension Pouch (rétention 100 Âmes/palier) | S | Sémantique diffère (Âmes déjà persistantes) |
| Structure de run en **actes thématiques** | L | Structure de jeu globale |

#### (d) Plan d'alignement

**P1 (S/M, faible risque, transforme "shop de boons" en "ascensions Gunfire")** :
1. Rendre le **Coffre wave-clear gratuit** (élimine les 2 systèmes divergents).
2. **Garantir + exposer l'empilement multiplicatif** (carte "+52 %" pour 3× +15 %) + test sur `sys_recompute_boon_mods`.
3. **Pondération par rareté** dans `roll_candidates`.

**P2** : HUD boons actifs + compteur de synergie tags (données déjà présentes).
**Puis** : trade-off/curse, Peddler/Vault (M/L, changent l'économie de décision).

---

### Système 5 — Armes — **32 %** — P2

#### (a) Spec Gunfire (chiffrée)

- **Pas de rareté de couleur portée par l'arme** (la prémisse gris/bleu/violet/orange est **inexacte** pour Gunfire). La puissance vient des **inscriptions** : **nombre 1–5** + **tier** (vert Normal / bleu Rare / orange Légendaire).
- **Scaling par Stage** : Stage 1 = 1–3 inscriptions, Stage 2 = 3–5, Stage 3 = **Gemini**.
- **2 armes principales swappables** (slots 1&2, chargeur/reload indépendants) + **slot 3 Foundry** (pistolet infini).
- **Élément FIXE** par variante (non modifiable, sauf Gemini "share element").
- **3 munitions partagées par calibre** : Normal vert / Large bleu / Special jaune.
- **Familles** : 6 Rifles / 6 SMG / 9 Pistols / 6 Shotguns / 8 Snipers / 6 Launchers / 4 Injectors + 3 Melee.
- **Crit multiplier 1×–5 %×** par arme, chance de proc élémentaire 5–50 %, **+15 % dégâts/niveau** d'upgrade.

#### (b) État Forgia actuel

**4 armes fixes** genome-driven (`viewmodel_arena.toml`), **pas de système de rareté** :

| Arme | Type | Stats clés | Fichier |
|---|---|---|---|
| **Pépin** (ModernAR) | Pistolet semi | 28 dmg, 6/s, mag 12, ×2 head, jauge confiance | `forgia-fps/src/pepin.rs` |
| **Bourrasque** (AssaultRifle) | SMG full-auto | 11 dmg, 11/s, mag 30, ×1.5 head, 0 gimmick | `forgia-fps/src/bourrasque.rs` |
| **Mme Lenoir** (Shotgun enum) | Sniper semi | 50 dmg, ×2 head one-shot, 0 falloff, scope | `forgia-fps/src/lenoir.rs` |
| **Boucherie** (RocketLauncher) | Lance-roquettes | projectile 30 m/s, AOE 70 dmg/4 m | `boucherie_rocket.rs` |

- Firing genome-driven (`forgia-fps/src/lib.rs:95-116`) : auto/semi/pump/burst, cooldown 1/fire_rate, multi-pellets + spread PRNG déterministe, falloff linéaire.
- **`AmmoSlot`** complet (`forgia-combat/src/ammo.rs:101-266`), ReloadKind Mag vs ShellPerShell.
- Sensors par arme, `ARENA_V1_WEAPONS` iterable, `DamageKind` scaffold (`forgia-damage/src/lib.rs:65-73`).

#### (c) Écart

**Matche** (11) : firing core genome-driven ; stats complètes par arme ; headshot zone-based ; falloff ; multi-pellets ; munitions par arme + ReloadKind ; HUD ammo ; sensors ; roster iterable ; projectile balistique ; scaffold DamageKind.

**Partiel** (4) :
- **Slots** : 4 armes FIXES simultanées (Digit1-4) → passer à **loadout 2-armes swappables** (`WeaponLoadout { primary, secondary, active_idx }`, AmmoSlot déjà per-weapon).
- **Alt-fire/gimmick** : `PlayerAction::AltFire` bindé (`forgia-input/lib.rs:48`) mais non consommé en Roguelite → formaliser `alt_fire` par arme.
- **Munitions par calibre** : réserves indépendantes → ajouter `AmmoFamily` + réserve commune par famille.
- **Crit variable** : ×2 head fixe → champ `crit_multiplier` + chance via `CombatRng`.

**Manque** :
| Élément | Effort | Risque |
|---|:---:|---|
| **Système d'inscriptions** (instance d'arme, 1–5 mods, tiers, scaling par acte) | **L** | **Le plus gros écart fonctionnel** — l'arme passe de "type fixe" à "instance avec rolls". Prérequis loot/drop. |
| Tri-élément lié à l'arme + procs Brûlure/Choc/Décroissance | L | Combat mute `Health` sans passer par `DamageKind` — gros refactor hot path |
| Tri-barre PV ennemi + multiplicateurs | L | Cf Système 6 |
| Fusions élémentaires exactes | L | Dépend élément+barres |
| Gemini (bi-armes) | M | Dépend inscriptions + loadout 2-armes |
| Loot/drop d'armes en monde | M | Coordonner story-474 |
| Écran d'inspection (inscriptions colorées empilées) | M | Base = wizard story-612 |
| Familles complètes (~50 armes) | L | Charge de **contenu** (assets GLB), pas technique |

#### (d) Plan d'alignement

- **Étape 1 (S/M, faible risque, gain de ressemblance immédiat)** : passer des 4 armes simultanées au **loadout 2-armes swappables** + réserves indépendantes + aligner le bloc HUD ammo. Rend Forgia "lisible comme Gunfire" sans toucher le combat core.
- **Étape 2 (le vrai cœur)** : **système d'inscriptions** — instance d'arme portant 1–5 inscriptions data-driven roulées par Stage, tiers vert/bleu/orange, appliquées par-dessus les stats base.
- **Étape 3** : tri-élément + Shield/Armor/Health + fusions, puis Gemini + loot/inspection.
- ⚠️ **NE PAS commencer par la rareté d'arme colorée** : elle n'existe pas dans Gunfire (la couleur est portée par les inscriptions).

---

### Système 6 — Ennemis, défenses, boss, difficulté — **48 %** — P0

#### (a) Spec Gunfire (chiffrée)

- **3 défenses à barres colorées** : Vie (rouge), Bouclier (bleu, **régénère hors combat**), Armure (jaune). Cumulables. **La couleur dicte l'élément.**
- **Interaction élémentaire** : +50 % de l'élément correct, −25 % des deux autres.
- **Statuts** : Burning 5 s (20 % du coup/s), Shock 5 s (+10 % toutes sources), Decay 5 s (−50 % vitesse).
- **Catégories** : Communs / Élites (mid-boss, weakspots) / Boss + adds.
- **Structure d'acte** : 4 actes × (3 combats terrain + 1 boss). Boss multi-phase (Pole Monarch 2 phases, Yoruhime-Maru multi-phase).
- **Difficulté** : Normal → Nightmare → **Reincarnation R1–R8**. PV ×2,9 (Acte 1 R1) jusqu'à **×13,6 (Acte 3/4 R7)**, défense jusqu'à **×17,1**, dégâts jusqu'à ×3,9, Âmes ×1,25→×1,55. Mécaniques exclusives : Spiritual Remnant, Dark Statue / Phantom Peddler.

#### (b) État Forgia actuel

**3 archétypes + Boss**, stats **hardcodées en Rust** (pas de TOML dédié), **Health unique** :

- **`EnemyArchetype`** Tank/Runner/Sniper/Boss (`enemies.rs:20-26`), stats `stats_for` (`enemies.rs:42-116`).
- **Vagues 1-3** (`waves.rs:71-90`), Boss vague 3 + 4 support, **enrage à 50 % PV** (`waves.rs:324-357`).
- Porte boss-gated (`boss_portal.rs`), Health unique (`forgia-combat/src/lib.rs:108-121`), HitZone Head/Body/Limb (`forgia-damage/lib.rs:91-179`).
- AI tactique 4-phase (`forgia-ai-arena-bot/src/tactical.rs`), fireballs projectiles (story-617), difficulté **fixe** (pas de modes).

#### (c) Écart

**Matche** (11) : catégories commun/tank/boss+adds ; boss enrage multi-phase ; **système élémentaire déjà présent** (l'état l'avait manqué) ; efficacité chiffrée par cible ; statuts DoT ; Combustion ; exécution sous seuil ; zones de hit ; nameplate HP flottant ; loot/âmes tiered ; structure 3 vagues + boss.

**Partiel** (5) :
- **Barres de défense colorées + interaction élément↔couleur** : **absent** — le cœur mécanique Gunfire. Introduire `DefenseLayer{Health,Shield,Armor}`, re-router le matchup vers la **couche** (Feu→Vie, Foudre→Bouclier, Corrosion→Armure).
- **3 éléments Gunfire** : Forgia a 4 mal alignés → re-mapper + ajouter Shock/Decay + fusions manquantes.
- **Boss weakspots + phases scriptées** : enrage 50 % générique → couche dominante + `HitZoneTag` + phase bouclier-à-retirer.
- **Difficulté par vague** : scaling par count seulement, stats fixes.
- **Stats ennemis data-driven** : **hardcodées** (`enemies.rs:59-116`) — **viole `no-hardcode.md`** → extraire vers `roguelite_enemies.toml` (prérequis du scaling).

**Manque** :
| Élément | Effort | Risque |
|---|:---:|---|
| **Bouclier bleu régénérant hors-combat** (couche externe) | M | Health unique → stack de couches, touche combat/hitscan/despawn |
| **Modes Normal→Nightmare→Reincarnation R1-R8** (multiplicateurs data-driven) | M | Équilibrage, prérequis = stats data-driven |
| Interaction couleur-barre↔élément (apprentissage joueur via HUD) | M | Dépend de DefenseLayer + re-mapping |
| Mécaniques Réincarnation (Spiritual Remnant, Phantom Peddler) | L | Contenu méta transversal |
| **Barre de vie de boss dédiée** segmentée par phase | S | UI additive egui |
| Débuffs du joueur (poison/malédictions/slow) | M | Flux de statuts côté joueur |
| Évolution ennemis inter-vague + grâce d'invulnérabilité post-spawn | S | Localisé |

#### (d) Plan d'alignement (P0, séquencé)

1. **Extraire les stats ennemis** hardcodées → `roguelite_enemies.toml` (prérequis `no-hardcode`, débloque le scaling).
2. **`DefenseLayer{Health,Shield,Armor}`** avec Bouclier bleu régénérant hors-combat (réutilise la mutation `forgia_combat::Health`).
3. **Re-router le matchup** `elements.rs` vers la **couche** (Feu→Vie, Foudre→Bouclier, Corrosion→Armure, +50 %/−25 %) au lieu de l'archétype.
4. **Étendre la nameplate** pour empiler barres colorées par couche.
5. Puis **difficulté Réincarnation** (multiplicateurs data-driven au spawn).

> **Ce chemin transforme le matchup par-archétype déjà fonctionnel en le vrai système couleur-de-barre de Gunfire, avec réutilisation maximale de l'existant.**

---

### Système 7 — Économie & méta-progression — **55 %** — P2

#### (a) Spec Gunfire (chiffrée)

- **Copper** (in-run) : perdu à la fin, **cap 65 535**, dépensé au Peddler + Craftsman.
- **Soul Essence** (méta) : **conservé** à la mort, drop aléatoire/garanti boss/**Vaults** (source principale, gated par talent **Exploration** à 5 essence). Dépensé dans l'arbre de Talents.
- **6 arbres** : Expedition 1805 / Battle 3575 / Skill 1300 / Survival 3645 / Weapon 1475 / Hero 500/héros — **total ~17 800**.
- **Peddler** (Copper : dumplings, munitions, armes, scrolls, refresh) + **Craftsman/Blacksmith** (upgrade arme, reforge Gemini 300 Copper rerollable).
- **Recyclage** → Copper (~1000/run). **Reincarnation/Spiritual Remnant** : Blessings en Soul Essence + refund. **Dimension Pouch** retient 100 essence/palier. **Pas de respec.**

#### (b) État Forgia actuel

Dual-monnaie **OR (in-run) + ÂMES (méta)** :

- **OR** (`forgia-rpg-data/src/loot_tables.rs:22-26`), collect walk-over, Magasin NPC (`shop.rs:15-136`).
- **ÂMES** (`MetaSouls`, `run.rs`) : Boss 4 wisps, normaux ~8 %, valeur 2, collect radius 2,5.
- **Enclume** (`meta_shop.rs:565-668`) : 4 upgrades (Vitalité +15 PV / Puissance +8 % / Armure +5 % / Pactole +50 Or), déblocages armes (613) + paliers boons (616).
- `MetaShopSave` TOML atomique (`persist.rs:15-36`), load boot / flush OnExit-Victory-Defeat.
- Maîtrise arme (`weapon_levels`), coûts boons rarity-based, loot par archétype, revive token.

#### (c) Écart

**Matche** (10) : modèle dual-monnaie (Or≙Copper reset/perte, Âmes≙Soul Essence persistante) ; séparation stricte sans conversion ; drop tiered (boss garanti/normaux chance) ; méta permanente en Âmes ; boutique in-run en Or ; HUD conforme (Or affiché, Âmes non affichées en run) ; start-gold upgrade (Pactole≙talents Copper) ; coûts boons rarity-based ; bonus stats composés.

**Partiel** (5) :
- **Arbre 6 branches** : liste plate de 4 upgrades → restructurer `roguelite_meta_shop.toml` en branches nommées avec prérequis (mécanisme rank+cost déjà là).
- **Boutique in-run UI** : `process_buy/process_sell` prêts (`shop.rs:90-135`) mais UI reportée Phase 2.
- **Peddler vs Craftsman** : un seul shop générique → ajouter une forge in-run distincte (upgrade arme + reroll inscription).
- **Coût de départ en Âmes** (Spiritual Remnant) : le Coffre-en-Âmes est un embryon.
- **HUD Or** : confirmer emplacement/icône ; **ne PAS ajouter les Âmes au HUD in-run** (fidélité Gunfire).

**Manque** :
| Élément | Effort | Risque |
|---|:---:|---|
| **Vaults** = source principale d'Âmes + talent-gate **Exploration** | **L** | Pilier n°1 Gunfire — touche stage-graph + loot-room + méta-shop |
| Cap monnaie 65 535 | S | Faible — clamp `Gold` |
| Recyclage d'items → Or (~1000/run) | M | Nouveau verbe économique |
| Reforge/reroll d'arme au Blacksmith (Or) | L | Dépend du système d'inscriptions |
| Multiplicateur de drop d'Âmes par difficulté | M | Dépend d'un état difficulté |
| Scrolls économiques (Devil's Covenant, Golden Goblet refresh) | M | Dépend boutique in-run |
| Talents marchand (réduction 5→25 %, New Look) | M | Après Peddler UI |
| Reincarnation/Spiritual Remnant complet | L | Post-ship |
| Irréversibilité arbre Talents (invariant) | S | À acter comme règle |

#### (d) Plan d'alignement

**P2** :
1. **Vaults + talent-gate Exploration** d'abord (plus grand écart structurel : Âmes distribuées passivement via wisps, sans salle-coffre ni gate). Créer (a) room Vault orientée combat, (b) talent Exploration low-cost, (c) Elite Vault plus rémunérateur.
2. En parallèle : restructurer `roguelite_meta_shop.toml` en **branches nommées** (donne la forme d'arbre 6 branches).
3. Puis câbler l'**UI boutique in-run** (back-end prêt) → débloque scrolls + talents marchand.
4. Cap 65 535 (clamp trivial, fidélité exacte).

---

## 3. HUD — réplication (inventaire élément par élément)

> **Principe Gunfire** : HUD FPS diégétique minimaliste, feedback des réactions **100 % world-space** sur l'ennemi. **Ne PAS créer d'overlay joueur "réaction déclenchée"** ; ne PAS afficher les Âmes en run.

### 3.1 Inventaire Gunfire → mapping Forgia → action

| # | Élément HUD Gunfire | Position | État Forgia | Fichier Forgia | Action |
|---|---|---|:---:|---|---|
| 1 | **Barre défensive joueur** Vie(rouge)/Bouclier(bleu→orange)/Armure(jaune) | Bas-gauche | 🟡 mono-couleur | `player_hp.rs:18-120` | **Segmenter** (dépend méca Shield/Armor) |
| 2 | Portrait + badge nom/niveau | Bas-gauche | ✅ | `hud.rs:1118-1234` | RAS |
| 3 | **Icône Compétence Primaire (E)** anneau cooldown | Bas-centre | ✅ (sur F) | `hud.rs:1329-1405` | Rebinder E, déplacer en barre de skills |
| 4 | **Icône Compétence Secondaire (Q)** compteur charges | Bas-centre | ❌ | — | **Créer** (compteur, sans anneau) |
| 5 | **Icône Dash (Shift)** + pips charges + arc recharge | Bas-centre | 🟡 hint texte faux | `hud.rs:1050` | **Créer** icône, lire `DashState`, corriger binding |
| 6 | **Ressource spéciale héros** collée à Primaire | Bas-centre | 🟡 widgets séparés | `confidence.rs`, `energy.rs` | Rattacher à l'icône Primaire |
| 7 | **Munitions** mag/réserve + **type couleur** (vert/bleu/jaune) | Bas-droite | ✅ / ❌ couleur | `hud_ammo/mod.rs:83-226` | Ajouter enum `AmmoKind` + teinte |
| 8 | **Slots armes** (I/II/Foundry, actif en avant) | Bas-droite | ✅ 4 fixes | `hud.rs:1002-1112` | Refondre en **actif grand + secondaire estompé + swap** |
| 9 | **Crosshair dynamique par arme** (spread, gimmicks) | Centre | 🟡 statique | `forgia-crosshair/src/lib.rs` | Rendre dynamique (spread = f(cadence)) |
| 10 | **Hitmarkers + damage floaters** | Centre/ennemi | ✅ (existe !) | `forgia-game/src/lib.rs:110` | RAS |
| 11 | **Barre de vie ennemie** tricolore + **icônes de statut empilées** + **compteur stacks (×9)** | Au-dessus ennemi | 🟡 fill mono + VFX aura | `forgia-enemy-nameplate/src/lib.rs`, `status_vfx.rs` | Segmenter (après Shield) + rangée d'icônes debuff + overlay compteur |
| 12 | **Barre de boss dédiée** (large, nom, phases) | Centre-haut | ❌ | — | **Créer** `BossHealthBar` |
| 13 | **Minimap** blips ennemis + **légende** + marqueurs shops/Vaults | Haut-gauche | ✅ / ❌ légende/marqueurs | `hud.rs:896-993` | Ajouter légende + icônes Peddler/Blacksmith/Vault |
| 14 | **Compteur Or/Copper** (in-run) | Haut | ✅ | `hud.rs:118-195` | Aligner icône |
| 15 | Âmes/Soul Essence **NON affichées en run** | (méta) | ✅ conforme | `meta_shop.rs:600` | **Ne PAS ajouter en run** |
| 16 | **TAB sac perso / C info équipe** (boons+ascensions non persistants) | Modal | 🟡 liste live gauche | `hud.rs:233-318` | Écran modal TAB (⚠️ conflit keybind à arbitrer) |
| 17 | Défis vault/stage | Haut-droite | ❌ | — | Différer (dépend Vaults) |
| 18 | Overlays fin de run (récap Soul Essence) | Fullscreen | ✅ | `hud.rs:367-560` | RAS |
| 19 | **Icônes de statut/debuff temporaires** joueur | Ponctuel | ❌ | — | À créer avec débuffs joueur |

### 3.2 Éléments HUD à ajouter/modifier — liste ordonnée

1. **Barre défensive segmentée** (rouge/bleu, orange pré-rupture) — **bloqué par la mécanique Shield** (§2 P1).
2. **Barre ennemie tricolore + icônes de statut empilées + compteur de stacks** — bloqué par DefenseLayer (§6).
3. **Barre de boss dédiée** (S, indépendant, gain immédiat).
4. **Type de munition codé couleur** (S, data-driven).
5. **Icône Secondaire (Q) + icône Dash (Shift)** — dépend de la décision compétences (§3 skills).
6. **Slots armes → actif grand + secondaire estompé + swap** (dépend loadout 2-armes).
7. **Crosshair dynamique par arme**.
8. **Légende minimap + marqueurs shops/Vaults**.
9. **Rattacher la ressource spéciale à l'icône Primaire** (retirer le bandeau plein écran).
10. **Panneau boons actifs** (données `tag_counts` déjà là — pur affichage).

> **Règle de fidélité** : le "80 % de ressemblance à faible coût" = slots (actif+estompé+swap) + munitions couleur + barre boss + crosshair dynamique. Les gros items (barres segmentées joueur/ennemi) sont bloqués par la mécanique Shield/Armor, à faire d'abord.

---

## 4. Roadmap de parité priorisée

> Ordre imposé par les dépendances : **le cœur (défense tri-couche + éléments 3-réactions + HUD associé) débloque tout le reste.** Skills/build/économie ensuite. Chaque story ≥ 2 crates → **BMAD Standard, story obligatoire** (CLAUDE.md §7).

### P0 — Cœur : Défenses tri-couche + Éléments 3-réactions (débloque Systèmes 1, 2, 6)

| Story | Titre | Effort | Crates | Risque |
|---|---|:---:|---|---|
| **P0-A** | Extraire stats ennemis hardcodées → `roguelite_enemies.toml` (prérequis no-hardcode) | S | `forgia-mode-roguelite` | Faible |
| **P0-B** | `DefenseLayer{Health,Shield,Armor}` + Bouclier bleu régénérant hors-combat | **L** | `forgia-combat`, `forgia-mode-roguelite`, netcode | **Élevé** — hot path combat/despawn/hitscan (piège dual-health documenté) |
| **P0-C** | Moteur de réactions générique (set de statuts + `ReactionTable` paires→effet) | M | `forgia-mode-roguelite` | Moyen — refactor `elements.rs` sans casser les 21 tests |
| **P0-D** | `Element::Shock` + `StatusShock` (+10 % vulnérabilité 5 s) | M | `forgia-combat`, `forgia-fps`, `forgia-mode-roguelite` | Moyen — résolution dégâts |
| **P0-E** | Réactions **Miasma** (True DoT %PV ×9) + **Manipulation** (charme, immunité boss/élite) | M+L | `forgia-mode-roguelite`, `forgia-ai-arena-bot` | Manipulation = re-targeting IA |
| **P0-F** | Re-router matchup `elements.rs` archétype→**couche** (Feu→Vie…) + recaler Burn (20 %/5 s), re-trigger 0,12 s | M | `forgia-mode-roguelite` | Migration tests |
| **P0-G** | HUD : barre défensive joueur segmentée + barre ennemie tricolore + icônes de statut + sensor `forgia2_shield.json` | M | `forgia-ui-lib`, `forgia-enemy-nameplate` | Dépend P0-B |

### P1 — HUD signature + Compétences + Build (Systèmes 2, 3, 4)

| Story | Titre | Effort | Crates | Risque |
|---|---|:---:|---|---|
| **P1-A** | Trancher les 2 systèmes sur F : Primaire = sort instantané par arme (1 charge, 14–16 s), unifier HUD, corriger binding Dash | M | `forgia-mode-roguelite`, `forgia-combat`, `forgia-ui-lib`, `forgia-player` | Moyen — décision de design (§5) |
| **P1-B** | Compétence **Secondaire (Q)** + charges + ressource **Supplies** (pickup) | L | `forgia-mode-roguelite`, `forgia-loot-tables`, `forgia-ui-lib` | Élevé — nouveau système |
| **P1-C** | Coffre wave-clear **gratuit** + empilement **multiplicatif** garanti/exposé + tirage **pondéré** par rareté | M | `forgia-rpg-data`, `forgia-mode-roguelite` | Faible/moyen — feel |
| **P1-D** | HUD : barre de boss dédiée + type munition couleur + légende minimap + panneau boons actifs | M | `forgia-ui-lib`, `forgia-mode-roguelite` | Faible |
| **P1-E** | Icônes Dash/Secondaire au HUD + ressource spéciale collée à Primaire | S | `forgia-ui-lib` | Faible |

### P2 — Armes (inscriptions) + Économie (Vaults) + polish (Systèmes 5, 7)

| Story | Titre | Effort | Crates | Risque |
|---|---|:---:|---|---|
| **P2-A** | Loadout **2-armes swappables** + réserves indépendantes + HUD slots (actif+estompé) | M | `forgia-combat`, `forgia-fps`, `forgia-ui-lib` | Moyen |
| **P2-B** | **Système d'inscriptions** (instance d'arme, 1–5 mods, tiers, scaling par acte) + écran d'inspection | L | `forgia-combat`, `forgia-fps`, `forgia-rpg-data` | **Élevé** — refonte structurelle arme |
| **P2-C** | **Vaults** + talent-gate **Exploration** + restructurer meta-shop en 6 branches | L | `forgia-stage`, `forgia-mode-roguelite`, `forgia-rpg-data` | Élevé — multi-terminal `forgia-stage` |
| **P2-D** | UI boutique in-run (Peddler, back-end prêt) + cap monnaie 65 535 + recyclage→Or | M | `forgia-rpg-data`, `forgia-ui-lib` | Faible |
| **P2-E** | Crosshair dynamique par arme + alt-fire formalisé | M | `forgia-crosshair`, `forgia-fps` | Faible |

### P3 — Contenu & endgame (différé post-ship de base)

| Story | Titre | Effort | Risque |
|---|---|:---:|---|
| P3-A | Trade-off/curse sur boons + Cleansing | M | Feel |
| P3-B | Gemini (bi-armes) | M | Dépend P2-B |
| P3-C | Difficulté Réincarnation R1-R8 (multiplicateurs data-driven) | M | Équilibrage |
| P3-D | Spiritual Remnant / Phantom Peddler / défis R8 | L | Contenu méta |
| P3-E | Familles d'armes complètes (~50 armes, assets GLB) | L | Charge de contenu |
| P3-F | Boss weakspots + phases scriptées (Lu-Wu/Pole Monarch-like) | M | Contenu |
| P3-G | Débuffs joueur (poison/malédictions/slow) | M | Symétrie statuts |

---

## 5. Décisions ouvertes (à trancher par le game-maker)

Ces choix déterminent la trajectoire ; ils ne sont **pas** des bugs mais des divergences de design assumables ou non.

1. **Compétence F : Ultime 10 s (Forgia) OU Primaire cooldown (Gunfire) ?**
   - Aujourd'hui **les deux** sont sur F (`shockwave.rs:139-155`), ce qui n'a pas d'équivalent Gunfire.
   - **Reco réplication stricte** : F = sort instantané par arme (1 charge, 14–16 s) ; réserver l'état 10 s à un futur talent. **Sinon** assumer l'Ultime 10 s comme signature Forgia divergente.
   - **Bloque** : P1-A, P1-B, tout le Système 3.

2. **Ajouter Bouclier/Armure aux ennemis ET au joueur ?**
   - C'est **le pilier signature** de Gunfire (barres colorées + interaction élémentaire + lecture HUD), mais un chantier **L à haut risque** (hot path combat, netcode replicated, référence `two_health_types`).
   - **Alternative low-risk** : garder le matchup par **archétype** et assumer la divergence (silhouette/couleur d'archétype comme indice visuel).
   - **Bloque** : P0-B, P0-G, barres segmentées joueur+ennemi, Système 5 tri-élément.

3. **3 éléments Gunfire (Feu/Foudre/Corrosion) OU 4 actuels (Feu/Poison/Explosif/Perforant) ?**
   - Le re-mapping (ajouter Foudre/Choc, transformer Poison en slow Décomposition) est **requis** pour Miasma/Manipulation, mais casse l'équilibrage et l'identité actuelle (Poison stackant, Perforant exécution).
   - **Option hybride** : garder 4 éléments **et** ajouter Shock comme 5e pour débloquer les réactions.

4. **Durée de statut uniforme 5 s (Gunfire) OU durées Forgia (Burn 3 s / Poison 4 s) ?**
   - Trivial en TOML mais casse l'équilibrage courant.

5. **Coffre wave-clear gratuit (Goblet Gunfire) OU payant en Âmes (Forgia actuel) ?**
   - Le payant crée 2 systèmes divergents (Coffre vs Loot Room). Gunfire = **gratuit**. Réserver le paiement à un futur Peddler en Or.

6. **Empilement multiplicatif exposé (+73 %) OU additif (+60 %) ?**
   - Le multiplicatif est la **signature Gunfire** (récompense le mono-thème). À garantir + afficher sur la carte de boon.

7. **Loadout 2-armes swappables (Gunfire) OU 4 armes simultanées (Forgia Digit1-4) ?**
   - Le 2-armes rend le HUD "lisible comme Gunfire" mais change la boucle de jeu (choix de loadout vs accès permanent).

8. **Rareté d'arme colorée ?** — **NON.** La prémisse gris/bleu/violet/orange est **inexacte** pour Gunfire : la couleur est portée par les **inscriptions**, pas par l'arme. **Ne pas implémenter de tier de rareté d'arme.**

9. **TAB = sac perso (Gunfire) OU gameplay in-game (vision Forgia CLAUDE.md §1) ?**
   - Conflit de keybind direct à arbitrer avant d'ajouter l'écran de consultation boons/ascensions.

10. **Arbre de Talents 6 branches (Gunfire) OU shop plat 4 upgrades (Forgia, cible 14 ans) ?**
    - Les arbres = complexité vs simplicité créateur. Compromis : **branches nommées avec prérequis** (garde le rank+cost existant, donne la forme sans la complexité d'un vrai arbre à embranchements).

11. **Vaults comme source principale d'Âmes (Gunfire) OU wisps passifs (Forgia) ?**
    - Le talent-gate Exploration + Vaults est le pilier de progression n°1 de Gunfire. L'adopter change la structure de run (salles optionnelles + level design).

12. **Difficulté : modes Réincarnation R1-R8 OU difficulté fixe (Forgia actuel) ?**
    - Adopter les modes = endgame + rejouabilité, mais prérequis = stats ennemis data-driven (P0-A).

---

*Audit produit le 2026-07-01. Sources Gunfire : recoupement multi-sources vérifié (fpschampion, datamine GitHub, Steam cheat sheets, NamuWiki, wikis). État Forgia : cartographie code au workspace courant. Parité moyenne ≈ 45 %. Les deux P0 couplés (Défenses + Éléments) débloquent la plus grande part de ressemblance à Gunfire.*
