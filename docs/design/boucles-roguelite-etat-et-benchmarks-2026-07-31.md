# Les boucles d'un roguelite — théorie, état réel de Forgia, Gunfire Reborn, High on Life

> **Date** : 2026-07-31 · **Méthode** : 5 cartographies du code + 5 vérifications adverses
> (chaque claim rouvert, chaque `file:line` réouvert, chaque TOML grepé pour savoir s'il est
> réellement lu) + 3 recherches externes sourcées.
>
> **Règle de lecture** : rien ici ne vient des docs du projet. Les stories et les roadmaps mentent
> (`story-done-gate.md` documente pourquoi). La seule vérité citée est **le code Rust et les TOML
> réellement chargés par ce code**. Quand un chiffre est dérivé, le calcul est donné. Quand une
> source externe est une impression de joueur et non une donnée, c'est écrit.

---

## 0. Le résumé en dix lignes

Forgia a **toutes les couches d'un roguelite sauf celles qui produisent une décision**.

La boucle seconde existe (tir, dash, éléments, ultime). La boucle méta existe (Âmes, 17 rangs,
déblocages, sauvegarde en `%APPDATA%`). Entre les deux, **la boucle de run est un couloir** : 4 salles
qui rejouent la **même table d'ennemis codée en dur**, aux **mêmes positions**, avec pour seule
variable un multiplicateur de PV. Le choix de porte est écrit puis jamais relu. Le graphe de run est
généré puis jeté. Et un défaut isolé casse l'invariant fondateur du genre : **les boons ne sont jamais
remis à zéro entre deux runs**.

Gunfire Reborn — la référence explicite du projet — tient sur exactement l'inverse : un squelette
**fixe** et assumé, mais une **matrice de décision** (2 armes × 3 éléments × 3 couches défensives) qui
force un arbitrage toutes les deux secondes. High on Life, qui n'est pas un roguelite, apporte l'autre
moitié : **une arme est un verbe, pas une ligne de statistiques**.

---

## 1. La théorie — six boucles imbriquées

Le modèle de référence est celui de Daniel Cook (*Loops and Arcs*, Lost Garden) : une boucle =
**modèle mental → action → simulation → feedback → mise à jour du modèle**, et les boucles sont
**fractales** — elles tournent simultanément à plusieurs échelles. Un « arc » est une boucle brisée
dont on sort immédiatement.

> ⚠️ Les tranches de durée ci-dessous sont une **convention de praticiens**, pas une norme sourcée.
> Cook ne donne aucune durée canonique.

| # | Couche | Durée | La DÉCISION du joueur | Le FEEDBACK | La RÉCOMPENSE |
|---|---|---|---|---|---|
| 1 | **Micro / seconde** | 0,2–5 s | tactique pure : j'esquive ou je continue le DPS ? | immédiat, non ambigu | la sensation + les dégâts |
| 2 | **Rencontre** | 10–60 s | quelle cible je priorise, quelle ressource je dépense | la vague meurt / je perds des PV | survie + drop immédiat |
| 3 | **Salle** | 1–3 min | **quelle porte** = quelle récompense à quel risque | la récompense obtenue | un incrément de build |
| 4 | **Run** | 20–45 min | vers quel build je converge, quand je pivote | le build décolle ou pas | victoire, ou monnaie méta |
| 5 | **Méta** | 10–40 h | sur quoi j'investis ma monnaie persistante | la run suivante démarre différemment | puissance **ou** variance |
| 6 | **Méta-méta** | 40–100 h+ | **quelle contrainte je m'impose** | le palier franchi est affiché | primes exclusives |

### 1.1 Les quatre résultats les mieux établis

**a) La difficulté monte par les RÈGLES, pas par les PV.** C'est le résultat le plus net de toute la
recherche. Sur les **15 conditions** du Pacte de Châtiment de Hades (**63 Heat**, 64 en Hell Mode),
seules **~4 sont du scaling de statistiques**. Les 11 autres attaquent la décision et la ressource :

- *Approval Process* : **−1 option** à chaque offre de boon (3 → 2)
- *Routine Inspection* : **−3 talents** du Miroir par rang, jusqu'à 12
- *Lasting Consequences* : **−25 % d'efficacité des soins** par rang, jusqu'à −100 %
- *Underworld Customs* : **sacrifier un boon** en quittant chaque région
- *Jury Summons* : **+20 % d'ennemis** par rang (max +60 %) — la densité, pas les PV
- *Tight Deadline* : **9:00** par région, **−2:00** par rang

Dead Cells fait pareil : 1BC retire les fontaines une passe sur deux, 2BC les retire toutes, 3BC
limite à **3 charges de fiole pour toute la run**, 4BC à **zéro**. Le wiki ne publie **aucun**
multiplicateur de PV par palier — parce qu'il n'y en a pas.

**b) La longueur d'une run se mesure, elle ne s'espère pas.** Slay the Spire fournit le seul jeu de
données massif public : **18 M de runs, 1,6 M de victoires** → **9 % de win rate global**, run
**gagnante 64 min** vs run **perdue 23 min**, meilleur taux de victoire dans la bande **60–80 min**.
Housemarque a explicitement corrigé le tir entre Returnal (~1 h par biome) et Saros (**20–30 min**),
Abebe Tinari (lead game designer) : *« garder la sensation de danger… mais réduire le sentiment
d'impuissance quand on sait qu'il y a potentiellement trois heures pour revenir où j'en étais »*.

**c) Il faut 15 à 35 décisions de build par run, environ une par salle.** Slay the Spire ≈ 30-35
offres de 3 cartes sur ~51 étages. Roboquest ≈ **14 choix structurants** (perks aux niveaux 2/4/7/10,
upgrades aux 3/5/6/8/9 et 11-15) plus les armes ramassées. Hades ≈ 1 récompense annoncée par salle.
En dessous, le build n'a pas le temps d'exister ; au-dessus, chaque choix perd son poids. Et le
**nombre d'options est lui-même un paramètre de difficulté** — Hades le prouve en le réduisant à 2.

**d) La récompense s'annonce SUR la porte.** Hades affiche le type de chambre avant d'entrer ; Slay
the Spire montre toute la carte de l'acte. C'est ce qui transforme « avancer » en « choisir », et
c'est l'un des rares antidotes documentés au **choix illusoire**.

### 1.2 Deux doctrines de méta-progression, à choisir explicitement

| Voie | Exemple | Ce qu'elle donne | Le prix à payer |
|---|---|---|---|
| **(a) Persistance en STATS** | Hades — Miroir de Nuit (branche verte à partir de **300 Obscurité**) | puissance permanente, réconfort après l'échec | **oblige** à concevoir un contre-levier de difficulté (le Pacte), sinon le jeu devient plus facile à mesure qu'on y joue |
| **(b) Persistance en CONTENU / ACCÈS** | Roboquest — classes, armes, raccourcis ; Returnal — les objets découverts entrent dans le pool | **variance** permanente, courbe de skill intacte | plus lent à produire (il faut du contenu) |

Aimé Juvin (RyseUp / Roboquest) rejette nommément l'approche Rogue Legacy, qu'il qualifie de
**« simulateur de difficulté »**. Milen Ivanov, même studio : *« pas un roguelite FPS mais un FPS
roguelite »* — le game-feel prime. Ils ont passé **2,5 ans sur 5** en prototypage pur de game-feel
avant d'empiler quoi que ce soit.

### 1.3 Volumes de contenu du genre (ordres de grandeur)

| Jeu | Armes | Objets / boons | Classes / héros |
|---|---|---|---|
| **Gunfire Reborn** | **67** | **203** parchemins occultes (73 normaux / 66 rares / 39 légendaires / **25 maudits**) | 12–14 héros |
| **Roboquest** | **83** (5 familles) | 16 perks + 3 upgrades **par classe** | 7 classes |
| Enter the Gungeon | **190** | — | 4 |
| Binding of Isaac: Repentance | — | **716–719** objets | — |
| **Forgia (aujourd'hui)** | **4** | **18** boons | **1** |

Dodge Roll documente le prix caché de la combinatoire : avec des centaines d'objets qui interagissent
tous entre eux, l'ajout de contenu *« a ralenti au point de ramper »* et est devenu fragile. **La
combinatoire est une dette, il faut la budgéter.**

### 1.4 Ce que la recherche n'établit PAS (à ne pas citer comme acquis)

- Le **« hook des 3 minutes »** n'existe pas comme règle du roguelite. La littérature d'onboarding
  (gamification / F2P, Yu-kai Chou) parle de *quick win* < 90 s et des 5 premières minutes.
- **« La méta-progression est une béquille »** et **« les 10 premières runs sont frustrantes »** sont
  des impressions de joueurs très répandues (ResetEra, forums Steam), **jamais** établies par une
  étude publique.
- Supergiant énonce l'objectif *« take the sting out of failure »* mais **n'a jamais publié de cible
  en secondes** pour le retour en run.

---

## 2. Forgia aujourd'hui — la boucle réelle, couche par couche

### 2.1 Couche 1 — la seconde

**Ce qui existe.** FPS hitscan, raycast premier-hit, dégâts × zone × falloff × crit × confiance ×
vulnérabilité élémentaire, absorbés par **Bouclier → Armure → Vie**. Recul, camera shake, FOV punch,
chiffres flottants, flash d'impact, knockback par hit avec **multiplicateur par arme**, et un
**hitmarker** (4 segments diagonaux, 0,22 s, sur `CombatHitEvent`).

> **Correction d'un premier diagnostic** : la cartographie initiale concluait « pas de hitmarker »
> après avoir grepé `forgia-crosshair`. Le hitmarker vit dans une autre crate —
> `crates/forgia-effects/src/hitmarker.rs:19-120`, enregistré via `crates/forgia-ui/src/lib.rs:74`.

**Les 4 armes** (source réelle : `assets/genomes/viewmodel_arena.toml`, **pas** `roguelite_weapons.toml`
qui est mort) :

| Arme | Dégâts | Cadence | DPS brut | **DPS soutenu** | Chargeur / recharge | Portée |
|---|---|---|---|---|---|---|
| **Pépin** (pistolet semi) | 28 | 6,0/s | **168** | 105 | 12 / 1,2 s | 80 m, plein ≤ 30 m |
| **Bourrasque** (SMG auto) | 11 | 11,0/s | **121** | 76 | 30 / 1,6 s | 80 m, spread 1,5° |
| **Madame Lenoir** (sniper) | 50 | 0,8/s | **40** (80 tête) | 28,6 | 5 / 2,5 s | 300 m, **aucun falloff** |
| **Boucherie** (roquette) | 0 (hitscan neutralisé) | 0,9/s | 63 AOE | 28,6 | 3 / 1,33 s par roquette | explosion 70 dmg / r=4 m |

**Mobilité** : marche **6,5 m/s**, sprint ×1,5 (**9,75 m/s**), saut 6,5 m/s, gravité 18 m/s².
**Dash** : 4 m en 0,25 s (16 m/s), **2 charges**, 1,5 s de recharge par charge, double-tap Espace,
**direction = forward du joueur, jamais latéral** (`crates/forgia-player/src/dash.rs:23-44, 109-111`).

**Les ennemis** — 3 archétypes + 1 boss, **une seule FSM pour les quatre** :

| | PV + bouclier + armure | Vitesse | S'arrête à | Détecte à | Tir |
|---|---|---|---|---|---|
| Tank | 120 + 60 + 80 = **260** | 2,8 m/s | 3 m | 22 m | **25 dmg à 5 m** / 1,8 s |
| Runner | 35 + 30 = **65** | 7,0 m/s | 6 m | 40 m | 8 dmg à 8 m / 0,7 s |
| Sniper | 45 + 40 = **85** | 3,2 m/s | 22 m | 55 m | 18 dmg à 28 m / 1,6 s |
| Boss | 800 + 200 + 150 = **1150** | 3,5 m/s | 10 m | 80 m | 22 dmg à 32 m / 1,3 s |
| **Joueur** | 100 + 50 = **150** | 6,5 / 9,75 m/s | — | — | bouclier régén 20/s après 3 s |

**TTK réels** (Pépin, corps, salle 0, sans atout — l'absorption bouclier/armure est du **1:1 sans
réduction**, `crates/forgia-damage/src/defense.rs:134-147`) :

- → Runner **0,39 s** · → Sniper **0,50 s** · → Tank **1,55 s** · → Boss ≈ 10 s (recharges comprises)
- ennemi isolé → joueur : Runner 13,2 s · Tank 10,8 s · Sniper 13,3 s · Boss 8,9 s (**enragé 4,9 s**)
- vague 1 entière en portée : **98,4 dps théorique → 1,5 s** (plafond : boules esquivables à 26 m/s)

> **Correction** : le premier diagnostic annonçait TTK Tank 1,2 s et Runner 0,3 s — il avait oublié
> le bouclier et l'armure. Et le « one-shot tête » du sniper annoncé dans `viewmodel_arena.toml:159`
> est **faux contre un Tank** : 100 dmg ne passent pas 60 + 80 + 120.

**Ce qui manque à cette couche :**

| Manque | Preuve |
|---|---|
| **Aucun hitstop / freeze frame** | `crates/forgia-combat/src/combat_juice.rs:3` annonce « Hitstop… Kill SlowMo » ; `:186-187` dit que c'est extrait vers `forgia_juice_lib::hit_stop` — **ce module n'existe pas** (`forgia-juice-lib` n'expose que recoil / fov_punch / camera_shake / knockback), et `ForgiaCombatPlugin` (`forgia-combat/src/lib.rs:134-143`) n'ajoute que recoil et knockback. |
| **Aucun telegraph d'attaque ennemie** | `enemy_anim.rs:99` : `attack_clip: "Idle_Combat"` — une **pose**, pas un swing. Le clip est piloté par la distance (`:261`), pas par l'événement de tir, et la boule part immédiatement (`forgia-ai-arena-bot/src/lib.rs:390-400`). Le `hit_clip = "Hit_A"` du TOML est parsé mais `enum ClipKind` n'a **pas** de variante `Hit` (`enemy_anim.rs:246-283`) : chargé, jamais joué. |
| **Une seule IA pour 4 archétypes** | `forgia-ai-arena-bot/src/lib.rs:271-281` : `decide_bot_state` n'a **aucune branche par archétype**. « Rush », « kite », « charge » n'existent pas. Un Tank « de mêlée » tire en réalité une boule de feu à 5 m. |
| **Le Tank est un ennemi cassé** | `shoot_range = 5,0` mais `detect_range = 22,0` : il chasse **17 m sans rien pouvoir faire**, et refuse de tirer au-delà (`ai-arena-bot/lib.rs:369-372`). |
| **`head_damage_mul` par arme est un champ MORT** | Parsé (`forgia-viewmodel/src/genome.rs:125-126`) mais le fire path lit un multiplicateur **global de 2,0** (`forgia-fps/src/lib.rs:1065` → `hit_feedback.toml:5`). Et la carte du wizard affiche un **troisième miroir codé en dur** (Lenoir ×1,5, Boucherie ×1,0, `weapon_select.rs:133-169`) qui ne correspond ni au TOML ni au runtime. **Le jeu ment au joueur sur une stat de combat.** |
| **L'identité d'arme est presque entièrement numérique** | Aucun tir alternatif, aucun mode secondaire. Sur 4 armes, **2 seulement** ont une mécanique propre : Pépin (jauge de confiance, +2 %/stack jusqu'à +20 %) et Boucherie (projectile balistique). `bourrasque.rs:1-12` et `lenoir.rs:1-11` disent eux-mêmes : *« Ce module observe le fire path SANS le toucher »*. |
| **Les réactions élémentaires sont inatteignables** | Le moteur est complet (Combustion 200 %+100 % r=3,5 m · Surcharge 150 %/150 % r=4 m · Miasma 3 % PVmax/s ×5 stacks), mais **une arme = un élément** (`elements.rs:1171`), et une seule arme est débloquée au départ : `sys_enforce_unlocked_loadout` (`weapon_select.rs:341-363`) ramène de force sur Pépin. **Il faut 60 Âmes ET un switch en plein combat pour voir une réaction.** |
| **L'affinité du hit de base est coupée** | `roguelite_elements.toml:146` : `base_hit = false` → `build_weapon_affinity_table` retourne une map **vide** (`elements.rs:882-884`) → tout hit de base passe en `Physical` neutre. Les 4 blocs `[affinity.*]` (l.148-176) sont inertes. |
| **L'Ultime ne se charge pas** | 10 s actif / 25 s de recharge = verrou de 35 s. Rien ne le remplit : ni kills, ni dégâts, ni pickups. Aucun boon ne le touche. La technique est **imposée par l'arme équipée**. Zéro décision. |
| **Fenêtre d'exploit d'une frame** | `weapon_select_system` (Digit1-4) tourne en `Update`/`GameSet::Combat`, le garde en `Update`/`GameSet::Movement`, le tir en `FixedUpdate` — qui s'exécute **avant** `Update`. Séquence : frame N pose Bourrasque → frame N+1 FixedUpdate **tire avec Bourrasque** → frame N+1 Movement revient sur Pépin. |
| **L'IA tourne dans tous les modes** | `forgia-ai-arena-bot/src/lib.rs:234-256` : 9 systèmes chaînés **sans aucun `run_if(in_state(...))`**, et le plugin n'arrive que par transitivité `ForgiaFpsPlugin → ForgiaModeFpsArenaPlugin → ForgiaAiArenaBotPlugin`. Toute la boucle ennemie du roguelite dépend d'un `is_plugin_added` en cascade dans une crate « fps-arena ». |

**Verdict couche 1** : le socle est là et il est propre (64 Hz déterministe, éléments, défense
tri-couche, knockback par arme, hitmarker). Mais **la décision seconde-par-seconde se résume à
« viser la tête ou le corps »**. C'est un axe. Gunfire en a trois qui se croisent.

---

### 2.2 Couche 2 — la rencontre

**La composition d'une vague est une table Rust codée en dur** (`waves.rs:100-119`) :

```rust
1 => [ (Tank, 3, r=12 m), (Runner, 3, r=25 m), (Sniper, 2, r=50 m) ]   // 8 ennemis
2 => [ (Tank, 4, r=14 m), (Runner, 4, r=28 m), (Sniper, 4, r=55 m) ]   // 12
_ => [ (Boss, 1, r=12 m), (Runner, 4, r=28 m) ]                        // 5
```

La signature est `wave_composition(wave: u8)` — **elle ne reçoit ni la profondeur de salle, ni le
seed, ni le type de salle**. Et `advance_to_room` remet `current_wave = 1` à chaque nouvelle salle
(`waves.rs:504`). Donc **les salles 1, 2 et 3 spawnent exactement les mêmes 8 puis 12 ennemis**.

Pire : l'angle de l'anneau de spawn vient d'une graine **constante** —
`Xoshiro256StarStar::seed_from_u64(WAVE_BASE_SEED ^ wave)` avec
`const WAVE_BASE_SEED: u64 = 0xC0FF_EE51_C0BA_1700` (`waves.rs:143-144`). **Le RunSeed n'entre jamais
dans ce flux.** Le joueur mémorise en deux runs où arrivent les 3 Tanks.

**La seule montée en difficulté branchée** est `sys_scale_enemies_by_depth`
(`enemy_scaling.rs:164-205`, plugin bien ajouté `lib.rs:716`), qui multiplie **post-spawn** sur
`Added<EnemyArchetype>` :

- Vie + bouclier + armure × `(1 + salle × 0,35)` → salle 3 = **×2,05**
- dégâts des tirs ennemis × `(1 + salle × 0,15)` → salle 3 = **×1,45**

Conséquence chiffrée : le boss a en réalité **1640 PV**, pas 800, et ses tirs font 31,9.
Total à abattre sur une run : **≈ 14 170 points effectifs**.

> C'est **exactement** le levier que la communauté de Gunfire Reborn sanctionne (§3.6) et que le
> Pacte de Châtiment n'utilise qu'à 4/15.

**Ce qui manque** : aucun élite (l'enum `EnemyArchetype` n'a que Tank/Runner/Sniper/Boss), aucun
modificateur de rencontre, aucune densité croissante au-delà de 8 → 12, aucun soigneur, aucun
archétype qui force une priorité de cible. **Zéro différenciateur mécanique entre les 8 cibles.**

---

### 2.3 Couche 3 — la salle

**Le choix de porte existe visuellement et n'a aucune conséquence.**

Le graphe est généré à la Slay the Spire (`generate_run_graph(config, seed)`, `run.rs:682`), avec
7 `StageKind` pondérés (Combat 53 / Event 22 / Rest 12 / Elite 8 / Shop 5 + Treasure + Boss). Les
kinds du niveau suivant sont proposés sur 2 cartes (`waves.rs:392-421`), l'overlay les affiche avec
emoji et couleur (`hud.rs:670`), le joueur choisit aux flèches ou au clic.

Puis : `wave.room_kind = kind` (`waves.rs:503`).

**Et c'est tout.** `room_kind` est déclaré (`waves.rs:74`), écrit (`:503`), et lu **uniquement dans un
`info!`** (`:434`). Zéro `EventReader`, zéro `Query`, zéro branchement. Une porte « Boutique » et une
porte « Élite » produisent **la même salle, les mêmes ennemis, aux mêmes endroits, avec la même
récompense**.

Corollaires structurels :

- **Aucune salle non-combat n'existe.** `stage_id_for_depth(depth, is_boss)` (`run.rs:109-118`) **ne
  prend pas le kind en paramètre** et ne renvoie que `"crypts_of_anvil"` ou `"forge_sanctum"` selon
  la **parité** de la profondeur. Aucun autre chemin de construction de salle n'existe dans la crate.
- `forced_kind_for_depth` n'active `Rest` qu'à partir de `total >= 5` et `Treasure` qu'à `total >= 8`
  (`graph.rs:148-152`) : avec `total_stages = 4`, **ces deux branches sont mortes**.
- Le **« Difficulty Director »** RoR2 est un fantôme complet : 3 gènes TOML +
  `director_budget_for_depth` + le champ `StageNode.difficulty_budget` — **zéro lecteur** hors de
  `graph.rs` (seuls 2 littéraux de test dans `waves.rs:748-749`). Le budget de difficulté par nœud
  est calculé puis jeté. **C'est la cause structurelle du fait que le StageKind n'a aucun effet :
  la donnée existe, personne ne la consomme.**

**Le changement de salle n'a aucune existence spatiale.** Le joueur ne se déplace jamais : ni porte à
franchir, ni couloir, ni téléportation. C'est **l'arène qui est despawnée et respawnée autour d'un
joueur immobile**. `AnchorKind::PlayerSpawn` est produit (`forgia-stage/src/lib.rs:1128`) mais
**aucun consommateur n'écrit un Transform joueur**. La « progression dans le donjon » est un compteur.

**Et l'arène change moins souvent qu'annoncé.** La garde de `spawn_stage_arena_on_request` compare
**uniquement le `stage_id`**, jamais le seed (`forgia-stage/src/lib.rs:883-891`). Or salle 3 →
`crypts_of_anvil` et salle boss → `crypts_of_anvil` : **même id ⇒ aucun despawn, aucun respawn**. Le
combat de boss se déroule **littéralement dans l'instance de la salle 3**, POI compris, sans aucune
transition visuelle. Sur une run de 4 salles, **l'arène n'est reconstruite que 2 fois**.

**La salle 1 est la moins variée de toutes** — l'inverse de ce qu'on croirait : `sys_stage_dispatch`
tourne dès `RunState::Lobby` (clé `(0, false)`, `run.rs:145`) **avant que `RunSeed` n'existe** (il
n'est inséré que dans `sys_start_run`, `run.rs:734`). Le dispatch retombe donc sur
`const FALLBACK_SEED = 0xC0FF_EE51_C0BA_1700`, et la garde d'idempotence empêche tout re-dispatch
avec le vrai seed. **À la première run c'est une constante ; aux runs suivantes c'est le seed de la
run précédente.** Idem pour le décor (`decor.rs:825`, constante `0xDEC0_F00D`), qui de surcroît ne
reçoit **ni l'extent ni la profondeur** (`plan_decor_set(&cfg, &assets, seed)`, `decor.rs:990`) : le
même semis est rejoué dans la crypte de 90 m et dans la forge de 80 m.

**La récompense de salle**, elle, existe et fonctionne : à chaque vague nettoyée (6× par run) le
**Coffre du Forgeron** propose **3 boons** tirés par rareté pondérée (common 100 / uncommon 45 /
rare 18 / legendary 6), sans doublon dans un même tirage, filtrés par les paliers méta débloqués.
PV restaurés à 100 % à l'entrée du break de **15 s**.

> **Nuance importante** : pendant le Coffre **et** pendant le choix de porte, `blockers.block_look`
> et `blockers.block_fire` sont posés (`forgia-ui/src/lib.rs:1150-1161`). Le joueur ne peut **ni
> viser ni tirer** — mais les ennemis, eux, **ne sont pas gelés**. Ce n'est pas « le jeu continue
> normalement », et ce n'est pas une pause non plus.

---

### 2.4 Couche 4 — la run

**Structure** : `RunState` = Lobby / InRun{stage} / Boss{stage} / Defeat / Victory.
**4 salles** (`roguelite_stage_count = 4.0`) = 3 combat × 2 vagues + 1 boss = **7 vagues,
65 ennemis, identiques à chaque run**.

**Économie d'une run**

| Flux | Montant |
|---|---|
| **Or** gagné en kills | ≈ **201** (67 par salle de combat × 3) + 48 au boss |
| Or par coffre-fort POI | +50 (une fois par vault) |
| **Âmes** (méta) | 5/vague · 25/boss · 2/wisp (boss = 4 wisps, mob = 8 % de chance) · **+10 par pièce et +25 par étoile dans le parcours post-boss** → **~63+ par run** |
| Sinks en Or | boons 20-90 · reroll **30 (codé en dur)** · Trempe **219 au total** · munitions 30/70 · soin 40 |
| Sink en Âmes **in-run** | **« Second souffle » 15 Âmes** chez le marchand — jeton consommé à la mort (`run.rs:233`) |

> **Une seule monnaie in-run pour tous les sinks.** ~200 Or par run paient **soit** ~8 commons,
> **soit** 4 paliers de Trempe, **soit** 5 recharges de munitions. Toute dépense défensive annule la
> construction du build. Hades sépare Ténèbres / Oboles / Gemmes précisément pour ça.

**Fin de run.** Mourir = Or perdu à 100 %, Âmes conservées, overlay « LA FORGE T'A BRISÉ », XP de
participation. **Tuer le boss n'émet PAS la victoire** : ça pose `boss_defeated = true` et ouvre une
porte vers un **parcours plateforme** (GLB `platformer_underworld.glb`, 3 zones, ~3 000 descendants,
pads et checkpoints en littéraux). C'est le **portail de retour de la zone 3** qui écrit
`EndRunEvent(Victory)` (`loot_room.rs:934-943`). Le commentaire du code admet que ce n'est pas fini :
*« Condition de fin de run à brancher plus tard »* (`waves.rs:340`).

Conséquence non triviale : `victory_emitted = true` fait sortir `obs_roguelite_player_death`
immédiatement (`run.rs:246-248`), et une chute dans le parcours = respawn au checkpoint. **La run est
déjà gagnée dès la mort du boss ; le parcours est une formalité sans risque.**

**Durée estimée** : ≈ 14 170 PV ÷ 105 DPS soutenu ≈ 135 s de tir parfait → **4,5 à 7 min de combat
réel**, + 6 breaks × 15 s = 90 s, + 2 choix de porte, + le parcours → **≈ 8-15 min jusqu'à la
victoire**. C'est **sous la cible de 20-45 min** du genre — pas dramatique en soi (Saros vise 20-30),
mais à décider explicitement.

**Temps entre deux runs** : **1 frame**. `sys_auto_start_when_warm` (`forgia-ui/src/lib.rs:190-197`)
écrit `StartRunEvent` dès que `WarmupState.done`, sans action utilisateur. Le premier passage dure
90 à 900 frames (1,5 à 15 s de warmup PBR) ; ensuite le latch `done` reste vrai et le Lobby dure une
frame. **La loop tax est excellente.** Le problème est ailleurs — voir §2.5.

### 🔴 La rupture la plus grave : les boons ne sont jamais remis à zéro

`ActiveBoons::reset_run()` est défini à `crates/forgia-rpg-data/src/boons.rs:242` et
**appelé nulle part en production** — vérifié par grep direct : les deux seules occurrences du
workspace sont la définition et le test `boons.rs:728`. `sys_start_run` (`run.rs:643-758`) remet à
zéro l'Or, la vague, les PV, le chrono et le `CombatRng` — **jamais `ActiveBoons`**.

`ForgiaBoonsPlugin` fait `init_resource::<ActiveBoons>()` une fois au boot (`boons.rs:576`), et
c'est tout.

**Conséquence** : un joueur qui meurt et relance **garde tous ses boons**. À 6 coffres par run, la
3e run démarre avec ~12-18 boons empilés, tags compris (donc légendaires déjà déverrouillés). La
construction du build cesse d'être un enjeu de run et devient **un compteur de session**.

C'est l'invariant fondateur du genre — l'Interprétation de Berlin fait de la permadeath un facteur
quasi nécessaire — et il est cassé par un appel de fonction manquant.

**Deux aggravations dans la même direction :**

- **La maîtrise d'arme n'a aucun plafond** : +4 % de dégâts par run **terminée** (défaite comprise),
  `weapon_select.rs:305 + 392`, monté sur `OnEnter(Defeat)` **et** `OnEnter(Victory)`. À la 25ᵉ run
  avec la même arme : **+96 % de dégâts permanents**. Ça annule à terme le scaling ennemi et rend la
  courbe de difficulté intenable.
- `ElementUnlocks` est reset sur `OnEnter(GameMode::Roguelite)` (`lib.rs:292`) — **qui ne tire pas
  entre deux runs**, puisqu'on reste dans le mode et que seul `RunState` change.

---

### 2.5 Couche 5 — la méta

**Ce qui persiste** (4 fichiers TOML dans `%APPDATA%\Forgia\`) : Âmes, 17 rangs d'upgrades, 3 armes
déblocables, 3 paliers de boons, un niveau de maîtrise **par arme**, un niveau/XP, l'identité, et
3 compteurs (runs, victoires, meilleur temps).

| Ligne | Effet | Rangs | Coûts | Total |
|---|---|---|---|---|
| Vitalité | +15 PV | 5 | 20/40/70/110/160 | 400 |
| Puissance | +8 % dégâts | 5 | 25/50/85/130/190 | 480 |
| Armure | +5 % réduction (plafond dur 0,85) | 4 | 30/60/100/150 | 340 |
| Pactole | +50 Or de départ | 3 | 15/35/60 | 110 |
| Armes | Bourrasque / Lenoir / Boucherie | — | 60 / 150 / 250 | 460 |
| Paliers de boons | Uncommon / Rare / Legendary | — | 80 / 200 / 400 | 680 |
| | | | **TOTAL** | **2 470 Âmes** |

À ~63 Âmes par run : **≈ 39 runs pour tout maxer**. La chaîne méta → combat est bien câblée et
vivante (`run.rs:695/701-702/725` + `weapon_select.rs:310`, composés dans `boons_apply.rs:88-93`).

**Le seul vrai gate de contenu du jeu** est le palier de boons :
`meta_shop.rs:532-544` → `boons.rs:433` → `boons.rs:362` `.filter(|b| tiers.allows(b.rarity))`.

**Les problèmes :**

| Problème | Détail |
|---|---|
| **On ne peut pas dépenser entre deux runs** | Le Lobby auto-lance. Le code du hub in-game (7 onglets, achats clavier 1-7, wizard d'armes) est **vivant** — il tourne pendant les 90-900 frames de warmup du premier passage — mais **invisible** (overlay opaque) puis réduit à 1 frame. Les deux chemins de lancement manuel (bouton LANCER `hub.rs:288`, touche ENTRÉE `meta_shop.rs:582-589`) sont **inatteignables** : leur condition `warmup_ready` est exactement celle de l'auto-start. **Le seul accès à la boutique est « RETOUR AU MENU ».** |
| 🔴 **Le tuto pointe le mauvais bouton** | `hud.rs:436-441` affiche à la 1ʳᵉ mort « Dépense-les à L'Enclume », et `hud.rs:493-499` place la flèche « ↑ dépense tes Âmes ici, puis repars » **directement sous le bouton REJOUER** (`hud.rs:484`) — qui va au Lobby et relance sans rien dépenser. L'Enclume est derrière **RETOUR AU MENU** (`hud.rs:501`). **Le FTUE enseigne activement le chemin qui ne marche pas.** |
| 🔴 **Les Âmes sont perdues sur un alt-F4 en pleine run** | `sys_flush_meta_save` n'est câblé que sur `OnExit(GameMode::Roguelite)`, `OnEnter(Victory)`, `OnEnter(Defeat)` (`meta_shop.rs:922-924`). Aucun flush périodique. |
| **Aucun déblocage de contenu** | Hors les 3 armes et les 3 paliers, **tout le contenu est là dès la run 1**. Grep exhaustif : rien dans `waves.rs`, `enemies.rs` ni `graph.rs` ne lit le save. `generate_run_graph` ne prend que `(config, seed)`. **Le monde est identique run 1 et run 50.** |
| **Un seul personnage** | Grep `character\|hero\|Class` sur `forgia-mode-roguelite` : 0 occurrence gameplay. Le seul axe de variation est l'arme de départ. |
| **Les points de talent ne se dépensent jamais** | `progress.rs:60` en produit +1 par niveau ; le seul lecteur hors de `progress.rs` est `forgia-ui/src/lib.rs:448`, qui les **affiche** sous « Contenu à venir ». |
| **Le niveau joueur n'a aucun effet gameplay** | 4 consommateurs de `PlayerProgress`, **tous UI**. Aucun système de combat, de spawn ou de scaling ne lit `level`. |
| **Zéro objectif, défi ou quête** | Les onglets Missions et Succès n'appellent que `section_intro(...)` + « Contenu à venir ». |
| **CONTINUER et NOUVELLE PARTIE sont le même bouton** | `forgia-ui/src/lib.rs:690` et `:701` assignent tous deux `MenuAction::Launch(GameMode::Roguelite)`. Aucun moyen d'effacer une sauvegarde depuis le jeu. |
| **135 lignes de dialogues de hub jamais lues** | `roguelite_hub_dialogues.toml` : **0 référence** dans `crates/`. Donc zéro PNJ, zéro station interactive dans le hub — alors que le contenu (Maître Forgeron, réactions après défaite, réaction par arme libérée) est écrit. |
| **Bug serde silencieux** | `roguelite_identity.toml:7` déclare `[[name_preset]]` (singulier) ; `identity.rs:49` attend `name_presets` avec `#[serde(default)]` sans `rename`. Vec vide silencieux → **aucun nom pré-écrit affiché**. |

**Récap de fin de run indigent** : les écrans Defeat/Victory n'affichent que Âmes gagnées, Or perdu,
chrono et record. **Pas de kills, pas de dégâts, pas de salle atteinte, pas de boons pris, pas d'arme
utilisée.** Donc aucune lecture de « pourquoi j'ai perdu ».

---

### 2.6 Couche 6 — la méta-méta

**Elle n'existe pas.**

Grep sur tout `crates/` de `ascension|difficulty_tier|heat_level|prestige|new_game_plus|reincarn` :
**aucun résultat**. Le seul post-victoire est `meta_shop.rs:339` — un record de temps.

Après la première victoire, la run suivante est **strictement identique** (mêmes 4 salles, même
composition, mêmes multiplicateurs) ; seule la graine change — et la graine ne pilote presque rien.

**Ce que le seed pilote réellement** : (a) les POI et modules de l'arène via `req.seed`, (b) le
`CombatRng` des crits. **Il ne pilote pas** : la composition des vagues, les positions de spawn, le
kind de porte (non consommé), le `difficulty_budget` (mort), ni les boons proposés — le `CoffreRng`
n'est **jamais reseedé** depuis `RunSeed` (`boons.rs:420-424`, contrairement au `CombatRng` qui l'est
bien à `run.rs:681`). Son flux se poursuit d'une run à l'autre : les boons **varient**, mais de façon
non reproductible et non pilotée.

**Le seed n'est ni affiché, ni saisissable, ni partageable** : les 3 émetteurs passent `None`, et il
n'apparaît que dans un `info!` de log — alors que `graph.rs:13` cite Slay the Spire en source.

---

## 3. Gunfire Reborn — la même grille

**Références** : 95 % d'avis positifs sur **36 634** avis anglais (103 524 toutes langues), 2-5 M de
propriétaires, pic **35 153 CCU**, **2,5 M de copies** annoncées en juillet 2022. EA mai 2020 →
sortie novembre 2021.

> ⚠️ Le wiki Fandom renvoyant du HTTP 402, les données wiki viennent d'extraits d'index de recherche.
> À re-vérifier avant d'en faire une référence de balance. Trois éléments couramment cités
> **n'existent pas sous ce nom** : les « Weapon Gems » (c'est **Inscriptions**, dont une catégorie
> **Gemini**), la « salle de la roue / casino » (c'est le **coffre singulier / rouge**), et la
> « boutique de Nona » (Nona est un **héros DLC**).

### 3.1 Seconde — le vrai moteur n'est pas le tir, c'est le switch

Le joueur porte **2 armes** et le jeu l'oblige à alterner via une **matrice** :

| Élément | Chair | Bouclier | Armure |
|---|---|---|---|
| **Feu** | **+50 %** | −25 % | −25 % |
| **Foudre** | −25 % | **+50 %** | −25 % |
| **Corrosion** | −25 % | −25 % | **+50 %** |

Et les ennemis ne portent pas tous la même couche : ~26 ennemis à **armure**, ~31 à **bouclier**, le
reste en chair. **C'est ça qui force le switch, pas le TTK.**

**Deux multiplicateurs séparés et multiplicatifs** : le **Critical Hit** (skill — viser le point
faible, `CritX` propre à chaque arme) et le **Lucky Shot** (RNG pur, base 0 %, ×2 orange / ×3 violet /
×4 rouge ; à 230 % de chance, ×3 garanti dont 30 % du temps ×4). Un crit ×4 avec un lucky ×2 = **×8**.
→ *Séparer le multiplicateur de skill du multiplicateur de build permet de scaler le build sans
dévaluer la visée* — et de lire une mort comme « mauvaise visée » **ou** « mauvais build ».

**Mobilité** : **PAS de sprint**. Un **Dash universel à 2,5 s de recharge**, au sol, latéral, arrière
ou en l'air (air dash pour allonger un saut). Bonus ponctuels : **+40 %** de vitesse brièvement après
un dash, **+25 %** après ramassage de munitions.

> **C'est l'inverse exact de Forgia** (sprint continu 9,75 m/s + dash forward 4 m). Chez eux le
> désengagement est **une ressource comptée** donc une décision ; chez nous il est gratuit et continu.

### 3.2 Rencontre — la récompense vient de la COMBINAISON

Effets élémentaires : **Brûlure** (20 % des dégâts du coup/s, 5 s) · **Décomposition** (−50 % vitesse,
5 s) · **Choc** (+10 % de dégâts reçus **de toutes sources**, 5 s).

Fusions — elles **ne se déclenchent jamais depuis une seule arme** :

- **Combustion** (Brûlure + Décomposition) : **200 %** de dégâts instantanés sur la cible + **100 %**
  dans 5 m, cadence max 1×/0,12 s/ennemi
- **Miasma** (Choc + Décomposition) : **9 % des PV max/s** en dégâts vrais, 5 s, empilable ×9 — **mais
  bridé à 0,3 % des PV max sur les élites et 0,75 % sur les boss**

> Le garde-fou est aussi instructif que l'effet : un pourcentage de PV max est une bombe à retardement
> sur les sacs à PV ; ils l'ont **bridé** au lieu de le supprimer.

**Enemy Enhancements** : 1 effet primaire (nouvelle attaque ou buff de zone aux alliés) + 1 effet
secondaire (stats ou comportement) empilés sur des ennemis de base. **De la variété combinatoire à
coût d'asset nul.**

### 3.3 Salle et run

- **Structure** : 5 actes (4 de base + Phantom Abyss en DLC), **3 stages + 1 boss** chacun. Run
  complète **50-60 min** (rapports joueurs, pas une donnée officielle).
- **Coffres** : bleus (fin de stage), **verts (choix entre 3 parchemins)**, **singuliers/rouges** (un
  set d'options où chaque récompense **se paie** en PV, en cuivre ou en malédiction).
- **Vault** : zone cachée derrière un mur fissuré, **~1 par stage**, 4 familles (non-combat /
  obstacles, combat, élite, objectif spécial). **Le détour coûte du temps ET du risque.**
- **PNJ** : Colporteur (armes, munitions, parchemins) et **Artisan** (+15 % de dégâts de base par
  niveau, **2 améliorations max** par Artisan, 3 avec un talent ; reforge d'inscription Gemini pour
  **300 cuivre**).
- **Ascensions** : via des gobelets (fin de stage, vault, après un élite), **1 choix parmi 3**, propres
  au héros, **jusqu'à 3 niveaux — un niveau supérieur REMPLACE le précédent, il ne s'empile pas**.
- **Inscriptions** : 4 types (Normal, Rare, Exclusive, **Gemini**). Jusqu'à 4 + 1 Gemini au drop,
  l'Artisan peut en ajouter une 5ᵉ. **Une Gemini ne s'active que si les DEUX armes portent la même** —
  contrainte de build élégante, à voler telle quelle.
- **Mort** : perte des armes, parchemins et cuivre ; conservation de la Soul Essence. **Une
  résurrection par run**, achetable en Soul Essence en solo.
- **Rotation de boss** : le boss alternatif apparaît après **N victoires** sur le premier (5 pour
  l'acte 1, 3 pour les actes 2-3). *Une seule variable de compteur transforme la répétition en
  progression perçue, sans nouveau contenu de niveau.*

### 3.4 Méta — le pont qui empêche la méta d'être un compteur hors-ligne

La **Soul Essence** sert à l'arbre de Talents **entre** les runs (Expedition 1 805 · Battle 3 575 ·
Skill 1 300 · Survival 3 645 · Weapon 1 475 · 500 par héros ; paliers aux niveaux 10, 30, 45, 60, 75)
**et** se dépense **pendant** la run : bénédiction du Reliquat spirituel en début d'acte, Colporteur
fantôme, ouverture de certains coffres rouges, résurrection unique.

> **C'est le trait le plus réutilisable de tout ce rapport.** L'arbitrage « je thésaurise pour l'arbre
> ou je dépense pour sauver CETTE run » rend chaque mort partiellement rentable **sans** rendre la
> méta indolore.

**Déblocage des héros** par trois voies distinctes : niveau de talent (Ao Bai 25, Qing Yan 40,
Lei Luo 55), Soul Essence (Tao 400, Qian Sui 600), ou DLC. Crown Prince offert.

### 3.5 Méta-méta

**Reincarnation**, débloquée en terminant Nightmare, **8 paliers**. Le scaling est **par acte** :

| | Acte 1 | Acte 2 | Acte 3 | Acte 4 |
|---|---|---|---|---|
| PV ennemis R1 | ×2,9 | ×3,4 | ×4,0 | ×4,5 |
| **PV ennemis R7-R8** | **×5,9** | **×9,4** | **×13,6** | **×13,6** |
| Dégâts R1 | ×2,1 | ×2,0 | ×2,02 | ×2,02 |
| Dégâts R7-R8 | ×3,9 | ×3,2 | ×3,34 | ×3,34 |

Compensations : gain de Soul Essence ×1,25 → ×1,55, et remise de −45 % dégressive sur les coûts.

**R8 n'augmente PAS les stats par rapport à R7 : il ajoute des RÈGLES** (plusieurs événements-défis
par acte, événement-défi garanti sur les stages de boss). **Le studio a lui-même acté la limite du
levier statistique.**

**Bizarre Dream** : 8 modificateurs activables à la carte, solo, réservés à Reincarnation. Le plus
élégant : **Interdependent Fortunes** — le joueur prend des parchemins en plus, **mais les ennemis
reçoivent les capacités correspondantes**. La difficulté et la puissance montent par la même manette.

### 3.6 Ce que la communauté juge RATÉ — et qui nous concerne directement

- **« Bullet sponge = fastidieux, pas difficile »** : verdict récurrent sur Reincarnation (×13,6 PV).
- **« La RNG retire l'agentivité »** en haute difficulté.
- **Le level design est le point faible reconnu** : couloirs longs et rectilignes, passages étroits où
  l'on ne peut pas sauter par-dessus les projectiles, manque de verticalité — un dev a reconnu avoir
  manqué de temps. **Un jeu à 95 % d'avis positifs peut porter un level design médiocre si la boucle
  de build compense.**

### 3.7 Coop — le nombre avant les PV

À 4 joueurs : **+60 % de PV mais +75 % de spawns**. Le levier principal est la **densité**, cohérent
avec la critique bullet sponge. Non compétitive par construction : loot instancié par joueur,
relevages **illimités** (contre 1 résurrection en solo), fenêtre de relevage **16 s au premier KO,
−2 s par KO cumulé, plancher 4 s** — une pénalité qui monte sans jamais bloquer.

---

## 4. High on Life — ce qu'il y a à voler (et ce qu'il ne faut pas copier)

**Ce n'est pas un roguelite** : FPS d'aventure metroidvania de 8-12 h (15-25 h complétionniste),
**67/100 Metacritic** (Xbox) / 69 (PC) — « mixed or average », 25 avis positifs / 18 mitigés /
5 négatifs — mais **7,5 M de joueurs uniques** en un mois, plus gros lancement tiers de l'**histoire**
du Game Pass en heures jouées sur 5 jours.

**Cet écart critique/commercial est en soi la leçon centrale : la personnalité a porté un combat que
la critique juge plat.**

### 4.1 Une arme est un VERBE

Chaque Gatlian a (a) un tir principal d'archétype FPS, (b) un **Trick Shot** tiré par le « trick
hole » au dos de l'arme, qui est une **capacité-verbe**, et (c) **ce même verbe est le seul outil qui
ouvre certaines énigmes et certains chemins**.

| Arme | Munitions | Verbe | Usage en traversée / énigme |
|---|---|---|---|
| **Kenny** | 15 | **pousser** (Glob Shot : stagger + juggle) | déplacer panneaux et obstacles |
| **Knifey** | — | **tirer-vers** (Tether) | grappin sur ancrages, rails, tyroliennes, **ouvre les Luglox** |
| **Gus** | **3** | **coller** (Disc Shot + aspiration) | disque planté dans un mur rouge = plateforme ou point de grappin ; **refrappable au couteau au retour** pour relancer le combo |
| **Sweezy** | 25 | **figer** (Time Bubble) + tir chargé perçant | figer une hélice, un ventilateur, une plateforme mobile ; appuyer un bouton derrière une paroi |
| **Creature** | 6 | **retourner** (enfants autoguidés, Hypno Baby) | envoyer un enfant dans un conduit hors de portée ; retourner un ennemi posté en hauteur |
| **Lezduit** | 100 | flux plasma multi-cibles | finale uniquement |

Le jeu est structuré en **metroidvania** : chaque nouvelle arme **rouvre rétroactivement tout le monde
déjà visité**. C'est ce qui rend 5 armes mémorables — **chacune est un souvenir spatial, pas un DPS**.

Détail mécanique à noter : le trick shot ouvre une **micro-fenêtre de bullet-time à la visée**. La
capacité est **un tir**, avec une cible et un raté possible — pas un bouton de statut.

### 4.2 Progression : deux tiroirs bien séparés

- **Upgrades de CONFORT** (changent un chiffre) : Ammo Sac, Reload Tract, Glob Kidney / Chronoliver /
  Discbladder (cooldown), Womb Chamber (+1 enfant).
- **MODS de COMPORTEMENT** (changent la phrase que l'arme permet d'écrire) : **12 mods** sur 4
  Gatlians — Bounceflector 600, Multiglob 500, Magnum (×3 dégâts, 3 munitions), **Goopsuck 1 500**,
  Remote Detonator 1 500, Bullet Rewind 3 000, Heartsap 1 600, Lamaze Launcher 3 500, Neonatal
  Eruption…
- **MOBILITÉ achetable** (Bounty Suit) : Jetpack 999, Dodge Unit 1 000, Slide Bash 1 400, Kinetic Tank
  1 200, Jetpack Booster 3 000.

**~6 des 12 mods se TROUVENT dans le monde**, derrière un verrou de traversée. L'exploration devient
une source de puissance, et la branche facultative **paie** — mais exige un verbe qu'on n'a pas encore.

### 4.3 Ce que la critique démolit — et c'est exactement notre point faible

- **« The same three enemy types »** recyclés (Kotaku) ; **« a tiny handful of the same mooks »**
  (Jimquisition, 5/10).
- **« Multi-wave arena fights, which is the only way to raise the endgame stakes »** (GoSuNoob, 4/10).
  ⚠️ **C'est littéralement la structure de Forgia.**
- **« The shooting is incredibly flat, lacking in any sense of pace or momentum »** avec le seul Kenny
  (GameSpot).
- **« Enemies are particularly dumb, just swarming you with gunfire or melee attacks while only
  occasionally taking cover »** (Jimquisition).
- Le **jetpack périme les capacités de traversée des armes** une fois acheté (GoSuNoob).
- Un seul mod mal calibré (**Multiglob**) suffit à **trivialiser** le combat.
- L'humour : IGN *« outrageous humor […] really goofy, foul-mouthed guns »* vs Eurogamer *« edgelord
  cynicism and tedious, punchdown humour »*.

**Le vrai diagnostic** : la rotation d'armes n'est presque **jamais imposée mécaniquement**. Le seul
gate documenté est l'**armure de goop** des G3 (décapée au Goopsuck de Gus) et le tir chargé perçant
de Sweezy. Le reste vient de la **munition rare** (Gus = 3) et du rechargement passif hors-main : une
contrainte **économique**, pas tactique. **Le jeu donne les verbes mais ne pose jamais de phrase
obligatoire.**

### 4.4 La personnalité comme mécanique

Chaque Gatlian a son propre jeu complet de dialogues : une rencontre PNJ **change selon l'arme
équipée**, les armes commentent jusqu'à la **mise en pause**, et elles délivrent les **tutoriels en
diégèse** (Creature explique lui-même comment ses enfants fonctionnent). Zéro pop-up.

**La règle transposable** : la ligne doit être **déclenchée par un événement mesuré** (headshot,
dernière balle, sur-utilisation d'une seule arme, 3 sauts ratés d'affilée), avec cooldown par
catégorie et anti-répétition. **Une ligne réactive vaut vingt lignes d'ambiance.**

Forgia a déjà le canal (`roguelite_dialogue.toml` est chargé par `forgia-ui-lib/src/hud/barks.rs:144`,
`weapon_to_speaker` mappe l'arme au speaker). Ce qui manque, ce sont les **déclencheurs d'état**.

---

## 5. Comparaison directe

### 5.1 Les six couches, côte à côte

| Couche | **Forgia** | **Gunfire Reborn** | **Théorie / autres** |
|---|---|---|---|
| **1. Seconde** | 1 arme au départ, 1 bouton, décision = tête/corps. Sprint continu + dash forward. Pas de hitstop, pas de telegraph. | **2 armes**, matrice 3 éléments × 3 couches défensives, crit **et** lucky shot séparés. **Pas de sprint**, dash 2,5 s = ressource. | Roboquest : 2,5 ans sur 5 en prototypage de game-feel avant tout empilement. |
| **2. Rencontre** | Table Rust figée (8 puis 12), **positions constantes**, 1 seule FSM pour 4 archétypes, aucun soigneur, aucun élite. | Composition variée, ~26 ennemis à armure / ~31 à bouclier → **force le switch**. **Enemy Enhancements** = variété combinatoire à coût nul. | Hades *Jury Summons* : **+20 % d'ennemis/rang**, la densité comme levier. High on Life : le **Grentin** (soigneur volant) force la priorité de cible. |
| **3. Salle** | **Choix de porte inerte** (`room_kind` jamais relu). Aucune salle non-combat. Arène reconstruite **2 fois** sur 4 salles. Le joueur ne se déplace jamais. | Coffres bleu/vert/rouge, **Vault ~1/stage à 4 familles**, Colporteur + Artisan. Le détour **coûte**. | Hades **annonce la récompense sur la porte** ; Slay the Spire montre toute la carte. C'est l'antidote au choix illusoire. |
| **4. Run** | 4 salles, 7 vagues, **65 ennemis identiques**. Difficulté = **un seul multiplicateur** (+35 % PV/salle). ~8-15 min. **6 décisions de build réelles.** | 5 actes × (3+1), **50-60 min**. Ascensions (1 parmi 3, **remplacement** pas empilement), parchemins, inscriptions. | Cible **20-45 min**. Slay the Spire : gagnante **64 min**, perdue **23 min**, meilleur win rate **60-80 min**. **15-35 décisions/run.** |
| **5. Méta** | 2 470 Âmes → **≈39 runs**. **Aucun déblocage de contenu**, 1 seul personnage. **On ne peut pas dépenser entre deux runs.** | Talents 6 catégories + **héros débloquables par 3 voies**. **La monnaie méta se dépense AUSSI en run.** | Deux doctrines : stats (Hades, exige un contre-levier) **ou** contenu/accès (Roboquest, refuse le « simulateur de difficulté »). |
| **6. Méta-méta** | **Néant.** Grep `ascension\|heat\|prestige\|new_game_plus` = 0. | Reincarnation **8 paliers** + **Bizarre Dream** (8 modificateurs opt-in). | Hades **63 Heat / 15 conditions** · Slay the Spire **20 Ascensions** · Dead Cells **5 BC** · RoR2 Eclipse 1-8. |

### 5.2 Volume de contenu

| | Armes | Objets de build | Persos | Archétypes ennemis | Bosses | Environnements |
|---|---|---|---|---|---|---|
| **Forgia** | **4** (1 jouable au départ) | **18** boons | **1** | **3** + 1 boss | **1** | **2** arènes (24 et 13 pièces posées à la main) |
| Gunfire Reborn | **67** | **203** parchemins | 12-14 | dizaines | 8+ (rotation) | 5 actes |
| Roboquest | **83** | 16 perks + 3 upgrades/classe | 7 | ~35 visés | ~10 visés | 11 biomes visés |
| High on Life | 6 | 12 mods + ~15 upgrades | 1 | ~11 | 7 bounties | 5 zones |

**Avec 6 coffres par run et 18 boons dont 5 légendaires gatés, l'espace de build est épuisé en 2-3
runs.**

### 5.3 Le tableau qui résume tout : où va la difficulté

| Jeu | Part du levier « statistique » | Part du levier « règles / composition » |
|---|---|---|
| **Hades** (Pacte) | ~4 conditions sur 15 | **11 sur 15** |
| **Dead Cells** (BC) | aucun multiplicateur publié | **100 %** (fontaines, fioles, Malaise, 7ᵉ zone) |
| **Gunfire Reborn** (Reincarnation) | R1→R7 (jusqu'à ×13,6 PV) — **critiqué** | R8 = **règles uniquement** ; Bizarre Dream = 8 modificateurs |
| **Forgia** | **100 %** (+35 % PV/salle, +15 % dégâts) | **0 %** |

---

## 6. Diagnostic — les ruptures, classées

### 🔴 P0 — casse un invariant du genre

| # | Rupture | Preuve | Correctif |
|---|---|---|---|
| 1 | **Les boons ne sont jamais remis à zéro entre deux runs** | `ActiveBoons::reset_run()` `boons.rs:242` — **0 appel en production** (grep vérifié à la main) | Appeler dans `sys_start_run` à côté du reset de l'Or. **1 ligne.** |
| 2 | **La maîtrise d'arme est infinie et non plafonnée** | `weapon_select.rs:392`, +4 %/run **y compris en défaite**, aucun clamp | Plafonner (5-10 niveaux) et le mettre en TOML |
| 3 | **Le FTUE enseigne le chemin qui ne marche pas** | flèche « dépense tes Âmes ici » sous **REJOUER** (`hud.rs:484-499`), l'Enclume est derrière RETOUR AU MENU (`:501`) | Soit rendre le hub accessible entre deux runs, soit déplacer la flèche |
| 4 | **Les Âmes sont perdues sur un alt-F4 en pleine run** | `sys_flush_meta_save` : uniquement OnExit/Victory/Defeat (`meta_shop.rs:922-924`) | Flush périodique ou à chaque gain |

### 🟠 P1 — la boucle de run ne produit aucune décision

| # | Rupture | Preuve | Correctif |
|---|---|---|---|
| 5 | **Le choix de porte est cosmétique** | `room_kind` écrit `waves.rs:503`, lu **uniquement** dans un `info!` `:434` | Faire lire `room_kind` par `wave_composition` et par le type de récompense |
| 6 | **La composition d'ennemis est figée et le seed ne l'atteint pas** | `wave_composition(wave: u8)` `waves.rs:100` — pas de stage, pas de seed ; `WAVE_BASE_SEED` constant `:143-144` | Passer `(stage, kind, run_seed)` et consommer le `difficulty_budget` déjà calculé |
| 7 | **Le Difficulty Director est calculé puis jeté** | `StageNode.difficulty_budget` `graph.rs:363` — **0 lecteur** hors de `graph.rs` | Le brancher sur le spawn. **La donnée existe déjà.** |
| 8 | **Aucune salle non-combat n'existe** | `stage_id_for_depth` ne prend pas le kind ; `Rest` exige `total >= 5`, `Treasure` `total >= 8` — morts à 4 salles | Une salle Repos et une salle Boutique, même arène, sans vague |
| 9 | **La difficulté ne monte que par les PV** | `enemy_scaling.rs` seul système ; 0 élite, 0 modificateur, densité 8→12 | Ajouter un levier de **règles** (densité, retrait de soin, −1 option de boon) |
| 10 | **Aucune synergie entre boons** | `boons_apply.rs:52-85` : boucle plate, un seul écrivain dans `PlayerCombatMods` ; `weapon_filter` **jamais lu** | Faire lire un boon par un autre boon / une arme / un élément |
| 11 | **Ni exclusion, ni pity, ni anti-doublon** | `eligible_pool` ne filtre pas `active.active` ; le pity timer existe… dans le TOML **mort** | Anti-doublon + pity ; les gènes sont déjà écrits |
| 12 | **Une seule monnaie pour tous les sinks** | ~201 Or gagnés vs **219 Or** pour la seule Trempe | Séparer un axe « build » d'un axe « consommables » |

### 🟡 P2 — contenu et lisibilité

| # | Rupture | Preuve |
|---|---|---|
| 13 | Aucune méta-méta : ni ascension, ni heat, ni NG+ | grep = 0 résultat |
| 14 | Le pickup « cœur » **ne soigne pas**, il donne de l'Or | `run.rs:335-341` pose le même `Pickup{value}` ; `loot_tables.rs:92` fait `souls.current += value` |
| 15 | Les légendaires sont inatteignables pour un compte neuf | double verrou 400 Âmes **et** 3 tags ; `eligible_pool` filtre le palier **avant** les tags |
| 16 | Le Forgeron est un point fixe unique pour toute la run | `const MERCHANT_POS: Vec3 = (-10, 0, 12)`, spawné une fois `OnEnter(GameMode::Roguelite)` |
| 17 | 4 des 8 stations sont **hors de l'enceinte** | `station_spawn_points()` place 4 stations à ±110 m ; `arena_extent_m` = 90 / 80 |
| 18 | Le seed n'est ni affiché, ni saisissable, ni partageable | 3 émetteurs passent `None` ; seul un `info!` le sort |
| 19 | Récap de fin de run indigent | Defeat/Victory n'affichent que Âmes, Or, chrono, record |
| 20 | Le boss se bat dans l'arène de la salle 3, sans transition | garde `stage_id`-only `forgia-stage/src/lib.rs:883-891` |
| 21 | La salle 1 est générée avec une **constante**, pas le seed de la run | `RunSeed` inséré à `run.rs:734`, dispatch dès le Lobby `run.rs:145` |
| 22 | `always_on = true` supprime la progression d'éléments prévue | `roguelite_elements.toml:20` ; le TOML dit lui-même « SHIP = repasser à `false` » — et rend **inatteignable** toute la branche `ChoiceKind::Element` de `loot_room.rs:968-978` |

### ⚫ Data morte confirmée (grep sur `crates/` = 0 lecteur)

| Fichier | Taille | Contenu perdu |
|---|---|---|
| `assets/genomes/roguelite/roguelite_loot.toml` | 5,2 Ko / 180 l. | **tout le système de butin** : pity timer, pools par stage, drops d'**armes**, accessoires, buffs |
| `assets/genomes/roguelite/roguelite_weapons.toml` | 8,2 Ko | catalogue d'armes — `weapon_select.rs:11` déclare lui-même le fichier mort |
| `assets/genomes/roguelite/roguelite_hub_dialogues.toml` | 7,3 Ko / 135 l. | dialogues de hub (Maître Forgeron, réactions par arme) |
| `assets/genomes/economy/economy_default.toml` | — | PV, régén, rayon de ramassage, drop rates, prix |
| `assets/genomes/enemies/*.toml` (4 fichiers) | — | **grunt / archer / elite / boss** — un roster complet non branché ⚠️ |

> ⚠️ **Piège documentaire actif** : `.claude/rules/map-design-intention.md` §2 présente
> **grunt / archer / elite** comme LES archétypes de référence, avec leurs chiffres (9,0 m/s, vision
> 20 m…). **Ces chiffres ne sont dans aucun runtime.** Les ennemis réels sont Tank / Runner / Sniper /
> Boss, définis dans `roguelite_enemies.toml`. Toute dérivation géométrique faite sur les valeurs de
> la règle est fausse. **À corriger dans la règle.**

Autres éléments morts : `roguelite_boss_stage_index` (parsé, jamais lu — et vaut 4 alors que la run a
4 salles, index 0..3), 9 gènes sur 15 de `roguelite_run.toml` à **zéro occurrence** dans le code
(pacing, coop, revive, seed_xor, bonus coop), `music_state` et `weather_override` (stub explicite
`run.rs:184-203`), `module_palette` des 2 stages (court-circuité par `suppress_procedural_modules`),
`RunSeed::encounter_seed` (testé uniquement), `hit_clip = "Hit_A"` (chargé, jamais joué), et
`forgia-pcg-runtime` — **zéro dépendant dans le workspace**.

---

## 7. Ce qui est déjà là et sous-exploité — les leviers gratuits

C'est la partie encourageante : **la plupart des briques manquantes sont déjà écrites et branchées.**

| Brique existante | État | Ce qu'elle débloquerait |
|---|---|---|
| **Défense tri-couche** Bouclier/Armure/Vie | ✅ vivante, par archétype | La **matrice Gunfire** : il suffit que les archétypes portent des **couches différenciées** et que les éléments aient un ratio ±. **Les deux briques existent déjà, elles ne se croisent pas.** |
| **4 éléments + 3 réactions** (Combustion / Surcharge / Miasma) | ✅ codées, testées, branchées | Le système le plus riche du combat — inatteignable car 1 arme = 1 élément et 1 seule arme débloquée. **Rendre une 2ᵉ arme accessible tôt suffirait à l'allumer.** |
| **`difficulty_budget` par nœud du graphe** | ✅ calculé, 0 lecteur | Le spawn dynamique RoR2 — **il ne manque que le consommateur** |
| **`StageKind` × 7, pondérés, affichés** | ✅ générés et dessinés | Les salles typées — **il ne manque que la lecture de `room_kind`** |
| **Barks par arme** (`roguelite_dialogue.toml` + `weapon_to_speaker`) | ✅ chargé et joué | La personnalité High on Life — **il ne manque que des déclencheurs d'état** |
| **`weapon_filter` sur les boons** | ✅ champ déclaré | Des boons par arme — **jamais lu, aucune entrée ne le renseigne** |
| **Pity timer, drop tables, drops d'armes** | ✅ écrits dans `roguelite_loot.toml` | Tout le loot du genre — **fichier mort, 0 lecteur** |
| **`FORGIA_BOOT_MODE`** | ✅ vivant côté outillage | Le retour instantané en run côté **joueur** — même besoin |
| **Capteurs JSON** (`forgia2_roguelite_state.json`, `forgia2_elements.json`) | ✅ vivants | L'équilibrage par métriques façon Mega Crit — **il manque : durée de run, salle de mort, boons pris, arme portée, DPS** |

---

## 8. Ordre de traitement proposé

> Principe directeur, tiré de la recherche : **la boucle seconde d'abord, la variance ensuite, le
> contenu en dernier.** RyseUp a mis 2,5 ans sur 5 sur le game-feel avant d'empiler. Et Gunfire
> Reborn démontre qu'un jeu à 95 % d'avis positifs peut porter un level design médiocre **si la
> boucle de build compense** — l'inverse n'est jamais vrai.

**Vague 0 — les 4 lignes qui remettent le genre debout (< 1 jour)**
1. `ActiveBoons::reset_run()` dans `sys_start_run`
2. Plafonner la maîtrise d'arme (en TOML)
3. Corriger la flèche du FTUE, ou rendre le hub accessible entre deux runs
4. Flusher la sauvegarde des Âmes en continu

**Vague 1 — rendre la run décisionnelle (le cœur du problème)**
5. `wave_composition(stage, kind, seed)` — consommer le `difficulty_budget` déjà calculé
6. Faire lire `room_kind` : au minimum **Combat / Élite / Repos / Boutique**, même arène, contenu
   différent
7. **Annoncer la récompense sur la porte**, pas seulement le type
8. Seeder les positions de spawn depuis `RunSeed` (une ligne dans `spawn_wave_enemies`)
9. Reseeder le `CoffreRng` depuis `RunSeed` — le `CombatRng` le fait déjà à `run.rs:681`

**Vague 2 — la matrice de décision seconde-par-seconde**
10. Différencier les **couches défensives par archétype** et croiser avec les éléments (la matrice
    Gunfire ; les deux briques existent)
11. Rendre la 2ᵉ arme accessible **tôt** — sans elle, aucune réaction élémentaire n'est atteignable
12. Un archétype **soigneur** (le Grentin de High on Life) : c'est la profondeur la moins chère du
    marché — rien à changer au feel du tir
13. **Telegraph d'attaque** + **hitstop** : sans wind-up lisible, le dash ne sert à rien parce que
    rien ne se voit venir

**Vague 3 — variance et rétention**
14. **Enemy Enhancements** : 1 effet primaire + 1 secondaire sur les archétypes existants — variété
    combinatoire à coût d'asset nul
15. Un levier de difficulté **par les règles** (pas par les PV) : densité, retrait de soin, −1 option
16. **Rotation de boss** après N victoires — une variable de compteur
17. Ranimer `roguelite_loot.toml` (pity, drops d'armes) — **180 lignes déjà écrites**
18. Instrumenter : durée de run, salle de mort, boons pris, arme portée, DPS → **équilibrer par les
    métriques, pas par l'opinion**

**Décisions à trancher explicitement (elles ne se déduisent pas du code)**
- **Doctrine de méta** : persistance en **stats** (→ il FAUT concevoir un contre-levier type Pacte)
  ou en **contenu/accès** (→ plus sûr à petite équipe, mais il faut produire du contenu) ?
- **Durée de run cible** : on est à 8-15 min, la cible du genre est 20-45. On assume ou on rallonge ?
- **Ce qui varie entre deux runs** : à écrire noir sur blanc (règle `map-design-intention.md` §4.5,
  toujours non résolue).
- **Le désengagement doit-il coûter ?** Gunfire n'a pas de sprint et un dash à 2,5 s. Nous avons un
  sprint continu à 9,75 m/s : le joueur peut **toujours** distancer un Runner à 7,0 m/s. Donc
  l'archétype de mêlée ne peut menacer que par le nombre, l'encerclement ou le blocage de sortie.
- **Coop** : le scaling est déjà écrit en TOML (`+30 % credits`, `+50 % PV`, `+15 % dégâts` par joueur
  supplémentaire) mais **aucun de ces gènes n'est lu**. Gunfire monte à **+75 % de spawns** contre
  seulement +60 % de PV — la densité impose une contrainte de **taille de salle** qui doit entrer dans
  la spec de combat **avant** la géométrie.

---

## 9. Annexes

### 9.1 Niveaux de preuve utilisés

| Niveau | Ce que ça veut dire | Exemples ici |
|---|---|---|
| **Vérifié** | `file:line` rouvert, système confirmé enregistré dans l'App, TOML grepé | tout le §2 |
| **Documenté** | source primaire ou wiki chiffré | formule RoR2, dataset Slay the Spire, Pacte de Châtiment, contenu Roboquest, citation Tinari |
| **Ordre de grandeur** | wiki/guide sans confirmation studio | durée de run Hades 20-45 min, nombre de chambres, run Gunfire 50-60 min |
| **Impression de joueurs** | forums, jamais établi | « bullet sponge », « la RNG retire l'agentivité », « la méta est une béquille » |
| **Non documenté** | à ne pas inventer | dégâts/TTK de High on Life, taux de drop Gunfire, poids de rareté, nb d'ascensions par héros |

### 9.2 Corrections apportées aux premières cartographies

Les 5 vérifications adverses ont **réfuté 30 claims** sur ~100 et trouvé **44 systèmes ou données
inertes**. Les corrections retenues dans ce rapport :

1. **Le hitmarker existe** (`forgia-effects/src/hitmarker.rs`) — la première lecture avait grepé la
   mauvaise crate.
2. **Le hub in-game n'est pas « inaccessible »** : l'overlay est un `layer_painter`, qui ne capture ni
   pointeur ni clavier. Le hub est **invisible mais vivant** pendant 90-900 frames au premier passage,
   puis réduit à 1 frame. Ce qui est réellement mort, ce sont les deux **entrées manuelles**.
3. **Les TTK étaient sous-estimés** : bouclier et armure oubliés (Tank = 260 pts, pas 120).
4. **La Trempe est linéaire**, pas multiplicative : `1 + level × 0,15` → cap **×1,75**, pas ×2,01.
   Écart de 15 % de dégâts au cap, non négligeable pour une passe de balance.
5. **Les armes verrouillées ne sont pas jouables** : `sys_enforce_unlocked_loadout` annule le switch.
   Un compte neuf n'a **qu'une** arme, pas 4.
6. **Il existe un sink de monnaie méta IN-RUN** : « Second souffle », 15 Âmes, jeton consommé à la
   mort — la première cartographie l'avait manqué.
7. **Deux sources d'Âmes oubliées** dans le parcours post-boss : Coin +10 et Star +25. Le parcours
   est en réalité **le pic de revenu de la run**. (Bonus : le popup du Coin affiche « +OR » alors
   qu'il crédite des Âmes.)
8. **La salle de boss réutilise l'arène de la salle 3** — la garde ne compare que le `stage_id`.
9. **La salle 1 est générée avec une constante**, pas le seed de la run.
10. **`enemy_scaling` est bien branché** — la cartographie « variance » l'avait manqué et concluait à
    tort que les 4 salles étaient strictement interchangeables. Elles le sont **à un multiplicateur
    près**, ce qui est la nuance qui compte.

### 9.3 Sources externes principales

**Théorie et données** — Lost Garden / Daniel Cook *Loops and Arcs* · RogueBasin *Berlin
Interpretation* · Fox Row *Slay the Spire statistical analysis* (18 M de runs) · GDC 2019 Giovannetti
*Metrics Driven Design and Balance* · GDC 2019 Benard *Dead Cells: What the F\*n!?* · Risk of Rain 2
Wiki *Difficulty* (formule) · Hades Wiki *Pact of Punishment* + RPG Site (détail des 15 conditions) ·
Dead Cells Wiki *Boss Stem Cells* · TheGamer *Saros' Biome Runs Are Faster Than Returnal's* (Tinari) ·
ActuGaming *interview RyseUp Studios* · Game Developer *Supergiant / Hades* et *Q&A Enter the
Gungeon*.

**Gunfire Reborn** — Wiki Fandom (Occult Scrolls, Weapons, Inscriptions, Ascensions, Dash, Lucky Shot,
Elemental Effect, Chests, Vault, Craftsman, Talents, Hero, Reincarnation, Bizarre Dream, Multiplayer,
Enemy Enhancements) · gist youmukonpaku1337 (guide chiffré éléments) · fpschampion (tableau R1-R8) ·
Steam / SteamSpy · SUPERJUMP *Proving the '30 Seconds of Fun' Mantra* · NamuWiki (stages, talents).

**High on Life** — Wikipedia · Game8 (Gatlians, mods, ennemis, bounties, Goopsuck, warp discs) ·
TheGamer · VideoGamer · Prima Games · Kotaku (review + *12 Things I Wish I Knew*) · Jimquisition ·
GoSuNoob · GameSpot · Metacritic · Xbox Wire · Console Creatures · Inverse (interview Roiland/Meyr) ·
GameRant.

---

*Rapport produit le 2026-07-31. Toute affirmation sur le code de Forgia est vérifiable au `file:line`
cité. Les chiffres externes portent leur niveau de preuve.*
