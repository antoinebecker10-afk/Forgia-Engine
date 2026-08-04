# Roguelite — audit de la progression et plan (2026-08-04)

> **Portée.** Audit read-only du code et des génomes, mené avant toute modification.
> « Implémenté » signifie *présent et câblé* — **pas** *vérifié en jeu*. Aucune partie
> n'a été jouée pour produire ce document.
>
> **Origine.** Une discussion partie de la mécanique de progression de *Dicero!*
> (Habby) et de *Gunfire Reborn*, qui a dérivé vers « comment on transpose sans
> dénaturer », puis vers « cartographie d'abord ». Le résultat renverse la question
> de départ : **le socle est bien plus avancé que la conversation ne le supposait, et
> le problème n'est pas de construire mais de finir.**

---

## 1. Ce qui existe

### 1.1 Structure de run

`GameMode::Roguelite` → `RunState` en SubStates : **Lobby / InRun{stage} / Boss{stage} /
Defeat / Victory**, graine `RunSeed` xoshiro256** déterministe.

**Deux modèles de structure coexistent**, un seul pilote :

| Modèle | Fichier | État |
|---|---|---|
| Graphe de salles (portes, salles typées) | `forgia_stage::graph`, story-646 | **en sommeil** |
| Boucle de rounds (arènes qui s'enchaînent) | `rounds.rs` + `roguelite_rounds.toml`, story-677 | **active** |

`[boucle] enabled = true` → le graphe est éteint. Le TOML le dit : *« false →
comportement d'avant (le graphe de salles pilote la run) »*.

> ⚠️ `max_rounds = 0` = rounds infinis. **L'état `Victory` est aujourd'hui
> inatteignable.** Ce n'est pas un roguelite, c'est un mode survie — et ce n'est
> écrit nulle part.

### 1.2 La courbe de menace, et le mur calculé

La difficulté n'est pas posée à la main :

```
menace_pv(r) = 1.16^r × 1.22^floor(r/3)          (continu + paliers)
ttk(r)       = pv_vague(r) / (168 × 0.35 × puissance(r))
mur          = premier r où ttk(r) > 90 s
```

Les entrées sont **mesurées** : `dps_reference = 168` (Pépin, mesuré dans
`viewmodel_arena.toml`), `pv_vague_reference = 1355` (compté sur le contenu réel),
`efficacite_tir = 0.35` (un FPS ne délivre jamais son DPS théorique). Géométrique +
paliers = Risk of Rain 2 + Gunfire Reborn, assumés dans le commentaire.

API : `RoundsConfig::{threat, player_power, time_to_clear, wall_round, margin}`.

### 1.3 Combat

Quatre armes **toutes disponibles** (Digit1-4, `weapon_select_system`,
munitions séparées par arme). Deux axes indépendants par arme :

| Arme | Élément passif | Matchup max | Technique d'ultime (touche F) |
|---|---|---|---|
| Pépin | shock | ×1.4 Runner | explosion AOE 4,5 m |
| Bourrasque | fire | ×1.3 Runner | chaîne électrique, 3 rebonds |
| Lenoir | armor_pierce | **×2.0 Tank** | couloir perforant 25 m |
| Boucherie | poison | ×1.4 Tank | gel de zone 2,5 s |

Ultime : 10 s actif / 25 s cooldown. Ennemis : Tank 120 pv / Runner 35 / Sniper 45 /
Boss, anneaux de spawn par archétype.

### 1.4 Économie — deux monnaies

| | **Or** (in-run) | **Âmes** (`MetaSouls`, persisté disque) |
|---|---|---|
| Source | Tank 5 / Sniper 3 / Runner 2 / Boss 40 | fin de vague + boss |
| Dépenses | Trempe (20/28/39/55/77), atouts (20→80), marchand | Enclume, armes, paliers d'atouts |

Une seule passerelle méta → in-run : le **« Second souffle »** (revive) acheté en
Âmes chez le marchand. Volontaire (décision du 2026-06-20), mais le modèle du mur
l'ignore.

### 1.5 Méta — déjà très Hades / Gunfire

- **L'Enclume des Âmes**, 4 lignes à **faces exclusives** (modèle Miroir de Nuit
  d'Hades) : Vitalité/Cuirasse, Puissance/Fortune, Armure/Endurance, Pactole/Étincelle.
  Les rangs sont **conservés** à la permutation, gratuite et réversible.
- **Déblocage d'armes** : Bourrasque 60 · Lenoir 150 · Boucherie 250
- **Déblocage de paliers de rareté d'atouts** : uncommon 80 · rare 200 · legendary 400
- **Maîtrise d'arme** : +1 niveau par run terminée, cap 6, +4 %/niveau

### 1.6 Atouts

**28 atouts** : 9 common · 7 uncommon · 5 rare · 7 legendary.
**6 tags** : precision (10) · chaos (9) · fire (6) · knockback (6) · chain (6) ·
ricochet (5). Poids : 100 / 45 / 18 / 6. Seuil de synergie : **3 tags identiques**.

Récompenses : 1 atout/round, +1 équipement tous les 2 rounds, 1 round de répit tous les 5.

### 1.7 Direction artistique et parcours

- **4 palettes de props** : `inferno` · `donjon` · `paturages` · `bourg` (story-671)
- **6 ambiances** (sol + ciel + brouillard + grading) : `forge_ardente` ·
  `crypte_suintante` · `halles_de_bois` · `necropole_glacee` · `gorges_d_ocre` ·
  `cime_de_pierre` (story-676)
- **Grille 3×3 de pièces** dans l'arène, arbre couvrant (connexité **impossible à
  violer par construction**) + 50 % de boucles, portes de 4 m, **tout à plat**
  (story-683)
- **Choix de porte fonctionnel** : `portal_choices` / `portal_pick` / `room_kind`,
  ce dernier **consommé** par `wave_comp::compose` depuis story-669
- **Porte physique à collider** ouverte sur condition : `boss_portal.rs`

---

## 2. Diagnostic

### 🔴 A. Cinq sources de puissance, aucune mesure du total

| # | Source | Monnaie | Portée | Gain max |
|---|---|---|---|---|
| 1 | Trempe | Or | la run | **×1.75** (mesuré) |
| 2 | Atouts `damage_mul` | Or | la run | non borné, multiplicatif |
| 3 | Équipement (5 pièces) | loot | la run | rareté × per_tier |
| 4 | Enclume « Puissance » | Âmes | permanent | +40 % |
| 5 | Maîtrise d'arme | runs jouées | permanent | +20 % |

**L'architecture est propre** : `boons_apply::sys_recompute_boon_mods` est le *seul*
écrivain de `PlayerCombatMods.damage_mul`, une ligne par source, et le fire path le
lit une fois. Il n'y a **pas de refactor à faire**.

Le problème est ailleurs : le modèle du mur ne connaît qu'un chiffre agrégé,
`gain_puissance_par_round = 0.34`, posé à la main.

> Un modèle de difficulté calibré au dixième, qui calcule contre une abstraction de
> la puissance joueur au lieu de sa somme réelle. **Le mur est calculé, mais pas
> contre le bon adversaire.**

### 🔴 B. Cinq systèmes, une seule nature

Trempe, Enclume-Puissance, Maîtrise, « Métal chaud », Équipement : **tous donnent des
dégâts**. Le joueur ne ressent pas cinq progressions, il ressent une barre alimentée
par cinq robinets dont trois sont invisibles.

C'est ce que les deux références évitent :
- **Dicero** — dés = *combinaisons*, talents = *stats*, gear = *slots*. Trois natures.
- **Gunfire Reborn** — armes = *effets*, Occult Scrolls = *règles*, maîtrise = *stats*.

### 🔴 C. Le pilier « toutes les armes vivantes » est tarifé

Trempe complète = **20+28+39+55+77 = 219 Or**. Revenu d'un round (2 vagues, round 0)
= **67 Or**.

- Tremper **une** arme ≈ 3,3 rounds de revenu brut, sans acheter un seul atout
- Tremper **deux** armes = 438 Or ≈ 6,5 rounds

**L'économie interdit matériellement de tremper une seconde arme.** Le joueur qui suit
le matchup et passe sur le Lenoir contre un Tank échange un ×1.75 acquis contre un
×2.0 situationnel — le gain net est marginal. Avec la Maîtrise (jusqu'à +20 % sur
l'arme investie), le switch devient perdant.

> **Correction 2026-08-04, mesurée en jeu.** La Trempe est **additive entre paliers**
> (`damage_mul_for_level = 1 + level × 0.15`), donc ×1.75 au plafond — pas ×2.01
> comme écrit dans une première version de ce document. L'erreur venait d'une lecture
> du commentaire « multiplicatif » de `roguelite_progression.toml`, qui qualifie en
> réalité le produit avec les boons et la méta, pas la composition entre paliers.
> Le même ×2,01 apparaît dans l'en-tête de `rounds.rs` : à corriger là aussi.

Le matchup dit « change d'arme ». L'économie répond « spécialise-toi ».

### 🟠 D. La synergie de tags ne paie pas

3 tags identiques → le légendaire devient *tirable*, à un poids de 6/169 ≈ **3,5 %**.
Un effort dont la récompense est un tirage à 3,5 % n'est pas un effort. Dans Dicero,
le brelan **est** le multiplicateur, immédiatement.

### 🟠 E. Le parcours est un menu, pas un lieu

Le choix de porte passe par `hud::draw_portal_overlay` — un panneau egui. Le joueur
**clique** une porte, il ne la **franchit** pas. Et la boucle de rounds a assumé la
suppression du parcours : *« des arènes qui s'enchaînent, sans parcours ni carte »*.

Système complet, dont la sortie est branchée sur un bouton au lieu d'un seuil.

### 🟡 F. Trous techniques

- `FlatBonus { stat }` : `match` sur deux stats connues, `_ => debug!`. Une stat
  inconnue produit un atout **inerte qui compile** et que le joueur paie.
- Pas de plafond sur `ActiveBoons.active` (`Vec`, « Duplicates allowed (stacking) »)
- `weapon_filter` déclaré, **jamais lu** — tous les atouts sont universels, ce qui
  protège le pilier **par accident**
- `roguelite_elements.toml` : `always_on = true` (mode TEST) court-circuite la
  progression élémentaire prévue (départ armé + 1 par portail)

---

## 3. Cible

### 3.1 Structure retenue (décision du 2026-08-04)

```
LIVRE 1 (univers)
├─ Chapitre 1  [DA inferno]     10 rounds → boss → gros loot   ← 1 RUN, ~15 min
├─ Chapitre 2  [DA donjon]      10 rounds → boss → gros loot
├─ Chapitre 3  [DA paturages]   10 rounds → boss → gros loot
├─ Chapitre 4  [DA bourg]       10 rounds → boss → gros loot
└─ Chapitre 5  [DA à créer]     10 rounds → BOSS FINAL DU LIVRE
```

**Une run = un chapitre = 10 rounds ≈ 15 min**, le format de Dicero. Le Livre est la
campagne : battre le chapitre N ouvre le N+1. Deux des trois niveaux existent déjà
(§1.7) ; il manque le niveau « Livre » et le câblage chapitre → DA.

### 3.2 La courbe doit être par CHAPITRE

Sur 50 rounds, la courbe actuelle donne :

| Round | Multiplicateur PV | PV d'un Tank |
|---|---|---|
| 10 | ×8 | 960 |
| 25 | ×200 | 24 000 |
| **50** | **×40 300** | **4 840 000** |

À DPS ~2 500 (socle ×3,38 + équipement + atouts), tuer **un** tank au round 50
prendrait **32 minutes**. La courbe fait exactement ce pour quoi elle a été écrite —
tuer le joueur tôt dans une boucle infinie — et elle est hors sujet d'un facteur mille
sur un parcours fini.

**Correctif :** croissance continue **dans** le chapitre (×4,4 sur 10 rounds), palier
franc **au changement de chapitre** (×2 → ×16 sur 5 chapitres), total ≈ **×70** en fin
de Livre. Tank à 8 400 PV, tué en 3,3 s, vague nettoyée en ~40 s dans le budget de 90 s.

Bénéfice gratuit : **le palier de difficulté et le changement de DA tombent au même
instant.** Le joueur voit la marche qu'il monte. C'est le modèle des actes de Gunfire.

Critère de calibration, falsifiable et testable :

> le mur tombe **avant le round 10** pour un joueur qui ne prend rien ·
> **après le round 10** pour un joueur qui prend tout.

Aujourd'hui, avec `gain_puissance_par_round = 0.34`, un joueur qui prend tout gagne
×18,7 sur 10 rounds contre une menace de ×8 : **le mur ne tombe jamais dans un
chapitre**. C'est le premier chiffre à corriger.

### 3.3 Une source de puissance = une nature de puissance

| Couche | Nature | Horizon | Donne | Ne donne jamais |
|---|---|---|---|---|
| **Éléments** | Situation | permanent | qui est fort contre quoi | des dégâts bruts |
| **Atouts** | Règles | la run | comment le tir se comporte | des stats plates |
| **Trempe** | Rythme | la run | la montée pendant la run | du permanent |
| **Enclume** | Puissance + confort | permanent | le pont entre chapitres | — |

> **Correction assumée.** Un conseil antérieur — « sortir les dégâts de la méta » —
> était calibré pour une boucle infinie. Avec 5 chapitres qui démarrent chacun un
> palier plus haut, **la méta est le seul véhicule qui traverse les runs** : elle doit
> garder de la puissance. Le symptôme était juste, le remède faux.

**Ce qui reste vrai** : la Maîtrise d'**arme** pousse à la mono-arme. La passer à la
maîtrise d'**élément** règle C et D d'un geste, et devient le pont inter-chapitres
*et* la récompense du pilier DOOM — deux fonctions pour un système.

### 3.4 Le parcours : franchir, pas cliquer

À la fin d'un round : break de 15 s (déjà calibré, *« sweet spot Hadès Chamber
transition »*) → **N portes s'ouvrent physiquement** au bord du complexe, chacune
portant son `StageKind` lisible de loin → le joueur **marche** et traverse → fondu
court → round suivant.

`portal_pick` est écrit par un déclencheur de franchissement au lieu d'un clic.
**L'orchestrateur en aval ne change pas d'une ligne**, et la brique physique existe
déjà (`boss_portal.rs`).

Le sentiment d'*avancer vers* le boss, trois leviers déjà disponibles :
- **le grading interpolé sur les 10 rounds** (chaque ambiance a son bloc `[.grade]`) —
  le round 8 ne ressemble pas au round 2 dans la même DA, sans un mot d'UI
- **le compteur de chapitre au HUD** (round 7/10 → BOSS)
- **la porte du boss visible** depuis les derniers rounds

**Contrainte non négociable : tout reste à plat.** Les bots n'ont ni navmesh ni
gravité (`bot_tactical_movement` en XZ pur). Pièces et couloirs : oui. Escaliers,
paliers, étages : **non** — un couloir qui monte est un couloir où les ennemis ne
suivent pas.

---

## 4. Plan

| Vague | Contenu | Coût | Risque | Dépend de |
|---|---|---|---|---|
| **V0** | Capteur `forgia2_power.json` — total + décomposition en 5, face à la menace | petit | nul | — |
| **V2** | `FlatBonus` bruyant · plafond ×1.3 sur atout filtré par arme | petit | faible | — |
| **V1** | La structure (détail ci-dessous) | moyen | balance | V0 |
| **V3** | Maîtrise d'arme → maîtrise d'élément | moyen | balance | V0 |
| **V4** | Atouts : Entretien / Corps / Récolte, sur les Gravures | moyen | faible | V2 |
| **V5** | Le sac Dicero : plafond · doublon=upgrade · tags au HUD · synergie qui paie | moyen | moyen | V4 |
| **V6** | Charme de la Pompe (« Retournement », 3 tags poison) | gros | élevé | V5 |
| — | Double saut — **différé** (l'IA ne saute pas, pas de navmesh) | — | — | — |

### V1 en détail

| | |
|---|---|
| 1.1 | `max_rounds = 10` → `Victory` atteignable |
| 1.2 | Courbe re-dérivée sur 10 rounds (critère §3.2) |
| 1.3 | Palier de difficulté au changement de chapitre, aligné sur le changement de DA |
| 1.4 | **Portes franchies, pas cliquées** — généraliser `boss_portal` aux transitions |
| 1.5 | Grading interpolé sur les 10 rounds + compteur de chapitre au HUD |
| 1.6 | Livre/Chapitre/Round en couche definition ; chapitre → DA |
| 1.7 | Gros loot de fin de chapitre = choix entre 3 natures (équipement / atout légendaire / Âmes) |

**V0 passe avant tout le reste.** Tant que la puissance réelle n'est pas mesurée,
chaque réglage de V1 est un pari.

### Contenu, en parallèle

- Une **5ᵉ palette de props** pour le chapitre 5 (4 palettes, 6 ambiances aujourd'hui)
- Ré-indexer `stage_palette` par **chapitre** au lieu de stage
- **Vérifier que la graine change entre les rounds** d'un chapitre — sinon les
  10 rounds se ressemblent

### À trancher

- **La boucle de rounds a éteint le graphe de salles.** La cible les réconcilie : la
  boucle pilote la **difficulté**, le graphe pilote le **parcours**. Ils ne se marchent
  pas dessus — ils ont été écrits l'un après l'autre sans être mariés.
- **`always_on = false` ?** « Départ armé + 1 élément par portail » devient la montée
  élémentaire d'un chapitre. Axe de progression intra-run déjà écrit, à rebrancher.

---

## 5. Cross-refs

- `.claude/rules/map-design-intention.md` — spec de combat, archétypes, porte de sortie
- `.claude/rules/map-design-patterns.md` — 14 patterns de construction
- `.claude/rules/spawn-clearance.md` — l'IA ne saute pas, pas de navmesh
- `docs/design/boucles-roguelite-etat-et-benchmarks-2026-07-31.md` — audit précédent
  (plusieurs constats depuis corrigés : `room_kind`/`difficulty_budget` sont lus
  depuis story-669, `ActiveBoons::reset_run` est appelé depuis `run.rs:858`)
- Stories : 529 (atouts) · 591/680 (Enclume) · 610 (marchand) · 613/616 (déblocages) ·
  646 (multi-salles) · 653 (Trempe) · 669 (composition dérivée) · 671 (palettes) ·
  675 (équipement) · 676 (ambiances) · 677 (boucle de rounds) · 683 (pièces)
