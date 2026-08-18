# Audit des strates — The Spared (Lobby · Arène · Expédition), 2026-08-18

> **Question posée** : entre les strates hautes (les trois zones où le joueur se
> tient) et les strates basses (les crates partagées), quels liens sont cassés,
> absents ou branchés à l'envers ?
>
> **Méthode** : mesure mécanique sur la sortie, pas lecture d'intention.
> `python tools/ai/strates.py` — livré avec cet audit, rejouable, avec ligne de
> base. Tous les nombres ci-dessous en sortent ou d'un `grep` cité.
>
> **Contexte de session** : un autre terminal éditait le code Rust pendant
> l'audit (`crates/forgia-game/src/lib.rs` modifié 22 s avant la mesure). **Aucun
> fichier `.rs` n'a été touché.** Les livrables sont un outil, ce rapport, une
> règle corrigée, la mémoire.

---

## 0. Le compte

| Contrôle | Constats | Ce que ça veut dire |
|---|---:|---|
| **C1** plugins déclarés jamais atteints par l'assemblage | **11** / 147 | ils compilent, ils ont des tests, ils ne tournent pas |
| **C2** génomes sans aucun consommateur Rust | **61** / 141 (**43 %**) | la couche *definition* ment à 43 % |
| **C3** crates partagées qui dépendent d'une zone | **4** | le partagé est devenu inséparable d'un mode |
| **C4** modules de zone importés du dehors | **22** (dont **18** depuis `forgia-mode-roguelite`) | une zone est devenue une bibliothèque sans changer de nom |

**Portée déclarée** — ce contrôle mesure des **liens**. Il ne dit rien de la
*justesse* de ce qui est branché, des dépendances tierces, du contenu des TOML,
ni de l'ordre des systèmes. Un lien présent peut être mauvais.

---

## 1. La carte réelle des strates

Ce que l'architecture prétend (`fine-grained-crates.md` : orchestrateur fin,
logique dans les crates atomiques) :

```
   zones :   menu-hub      mode-roguelite      mode-expedition
                 ↓               ↓                    ↓
 partagé :  ui-lib · combat · damage · player · effects · crosshair · audio …
                 ↓
   socle :  forgia-core
```

Ce que le code fait :

```
   zones :   menu-hub ─────18 modules─────► mode-roguelite ◄──── forgia-ui
                 │                              ▲   ▲                (partagé)
                 │                              │   └──── forgia-game (castle_avatar)
                 └──────────────────────────────┘
                                                                mode-expedition
                                                                (branchée à rien)
```

**`forgia-mode-roguelite` n'est pas une zone : c'est le tronc du jeu.** 966 LOC
de `lib.rs`, 27 plugins, et **18 de ses modules importés depuis d'autres
crates** — dont la moitié n'a rien d'une arène : `identity`, `equipment`,
`cosmetics`, `meta_shop`, `progress`, `chapters`, `avatar`. Ce sont des concepts
de **profil de joueur**, et ils vivent dans le mode le plus volatil du projet.

Conséquence directe, vérifiable : **le corps du personnage de l'Expédition est
posé par `crates/forgia-game/src/castle_avatar.rs`** (un fichier nommé
« castle ») **via `forgia_mode_roguelite::avatar` et `::equipment`**
(`castle_avatar.rs:26-27`, `:154 → GameMode::Expedition => cfg.corps_expedition()`).
La crate `forgia-mode-expedition` ne peut donc ni tester ni faire évoluer son
propre avatar.

---

## 2. Constat A — l'Expédition ne consomme rien de la strate partagée

`forgia-mode-expedition` déclare 9 dépendances internes. Voici ce qu'elle
n'utilise **pas du tout** (0 fichier, `grep` sur `<crate>::`) :

| Couche partagée | Utilisée par l'Expédition ? |
|---|---|
| `forgia-damage` (bouclier→armure→vie) | **non** |
| `forgia-ui-lib` (HUD, ammo, direction de dégâts) | **non** |
| `forgia-killfeed` | **non** |
| `forgia-audio` | **non** |
| `forgia-ai-arena-bot` (le seul ennemi du jeu) | **non** |
| `forgia-navmesh` | **non** |
| `forgia-effects` (VFX d'arme) | **non** — elle appelle `bevy_hanabi` en direct |
| `forgia-viewmodel` | **non** — elle a son `arme_main.rs` (1 503 l.) |
| `forgia-observability` | **non** — 6 capteurs écrits à la main |
| `forgia-stage` | **non** |

Elle consomme `core` (29), `player` (9), `streaming` (3), `crosshair` (3),
`combat` (3), le reste à 1-2.

**Ce n'est pas un défaut de code, c'est un état à nommer** : le mode E2 du GDD —
celui qui doit shipper avec l'Abîme — est aujourd'hui une carte qu'on parcourt,
sans dégâts, sans HUD, sans ennemi, sans son. Le travail restant n'est pas « la
brancher » : c'est **décider ce qui monte dans le partagé** (le HUD et les dégâts
y sont déjà) **et ce qui reste propre au mode** (la visée, la posture, la cape).

### Le corollaire déjà payé : les ressources partagées à un seul écrivain

`plugin.rs:180-200` le documente noir sur blanc : `CrosshairHidden` et
`CrosshairMode` sont des ressources **globales** dont les **seuls** écrivains
sont gatés `RunState::Lobby` (Roguelite) et `GameMode::Fps|Roguelite`. En
Expédition elles gardent la dernière valeur laissée — *« une partie de Roguelite
terminée l'œil dans la lunette suffit à supprimer le réticule ici »*.

La mesure généralise le motif : **11 ressources déclarées dans une crate
partagée n'ont qu'un seul mode écrivain** et sont lues ailleurs —
`GameplayHudVisible` et `ViewmodelForcedVisible` (déclarées dans `forgia-core`,
écrites par le seul Roguelite, lues par `forgia-viewmodel`), `PepinConfidence`,
`ActiveBoons`, `CameraShakeTuning`, `PathNetwork`, `TerrainConfig`,
`QuestCatalogue`, `DialogueRegistry`, `Souls`, `MouseLookTuning`.

> **La règle qui manquait** : une ressource déclarée dans une crate partagée doit
> être remise à son défaut à la **sortie** de la zone qui l'écrit — sinon la zone
> suivante hérite d'un état qu'aucun de ses systèmes n'a produit. C'est un
> `OnExit(GameMode::X)`, pas un `OnEnter` défensif chez chaque voisin.

---

### 2 bis. Précision — la migration vers l'Expédition existe, elle est à moitié faite

*(ajouté après relecture des stories 700-717, le 2026-08-18)*

Le §2 dit « l'Expédition ne consomme aucune couche partagée ». C'est vrai **dans
ce sens-là** — et c'est justement le point : **la migration s'est faite dans
l'autre sens.** Story-717 incrément 1 n'a pas fait consommer `forgia-ui-lib` par
l'Expédition ; elle a **édité `forgia-ui-lib` pour qu'il nomme
`GameMode::Expedition`**. C'est un *gate par énumération*, et il se mesure.

**12 prédicats multi-mode dans le codebase. 6 nomment l'Expédition, 6 non.**
Et par crate partagée (une zone qui se gate sur elle-même est exclue) :

| Crate partagée | Roguelite | Fps | **Expedition** | |
|---|---:|---:|---:|---|
| `forgia-ui-lib` | 9 | 6 | **3** | partiel — vie ✅ munitions ✅, **direction des dégâts ❌** |
| `forgia-viewmodel` | 6 | 7 | **0** | par conception (l'Expédition a son `arme_main`) |
| `forgia-observability` | 4 | 7 | **0** | **le mode est invisible au capteur partagé** |
| `forgia-killfeed` | 2 | 2 | **0** | ❌ |
| `forgia-ui` | 3 | 0 | **0** | ❌ |
| `forgia-juice-screen-flash` | 1 | 1 | **0** | ❌ |
| `forgia-ai-arena-bot` | 2 | 0 | **0** | ❌ (aucun ennemi) |
| `forgia-worldgen` | 1 | 0 | **0** | ❌ |

**Le trou le plus net est le retour de dégâts** : on prend des coups sans flash
d'écran, sans indicateur de direction, sans killfeed. Ce n'est pas un oubli
isolé — c'est ce que story-717 §constat décrivait déjà : *« trois pièces
manquaient, et la deuxième est la moins visible »*. Le gate par énumération
rend **chaque oubli silencieux** : rien n'échoue, la capacité manque, point.

> **Ce qui manque au process** : un mode ne devrait pas être une valeur qu'on
> ajoute à N listes à la main. Une **capacité** (« ce mode a du combat », « ce
> mode a un HUD ») déclarée une fois par la zone, lue par les crates partagées,
> supprime la classe entière. C'est le même geste que `Faction` (§3) : nommer la
> propriété au lieu d'énumérer les cas.

### 2 ter. Un défaut **dans le plan lui-même** : deux systèmes d'ennemis, le plan vise le mort

Il y a **deux** sources de stats d'ennemis, et elles ne servent pas le même mode :

| Source | Mode servi | Valeurs | Atteignable ? |
|---|---|---|---|
| `roguelite/roguelite_enemies.toml` | **Roguelite** — `waves.rs::spawn_wave_enemies`, chargé au Startup, hot-reload | tank 120/2,8 · runner 35/7,0 · sniper 45/3,2 · boss 800/3,5 | **oui**, c'est l'Abîme |
| `arena_bots.toml` | `forgia-mode-fps-arena` — **entièrement gaté `GameMode::Fps`** (`lib.rs:401-411`, `wave.rs:153-158`) | 1 seul bot : 200 pv, 3,5 m/s, détection 50 m | **non** — aucun menu n'y mène, seulement `FORGIA_BOOT_MODE=fps` |
| `enemies/enemy_{grunt,archer,elite}.toml` | aucun | grunt 30/9,0… | **mort** (§4) |

Or **story-713** (« donner un consommateur aux archétypes ») cible
`crates/forgia-mode-fps-arena/src/wave.rs`, et **story-715** dimensionne les
campements du Vallon sur *« ennemi 200 PV, tir 12 dmg/1,5 s, portée 35 m »* —
c'est-à-dire **le bot du mode injoignable**, pas les quatre archétypes que le
joueur affronte réellement.

Les deux stories sont **DRAFT** : rien n'est perdu. Mais telles quelles, elles
feraient shipper l'Expédition avec le TTK d'un mode mort. À corriger **avant**
de les lancer, pas après.

*(Corollaire : `forgia-ui-lib` dépend de `forgia-mode-fps-arena` — la crate UI
partagée dépend du mode injoignable. C'est une des 4 inversions du §0/C3.)*

---

## 3. Constat B — les quatre portes du GDD §10 : une est déjà entrouverte

Le GDD (2026-08-12) fixe quatre contraintes à tenir *dès maintenant*.

| Porte | État mesuré |
|---|---|
| Sim en `FixedUpdate` déterministe | ✅ tenue — `forgia-game/src/lib.rs:169-182`, 64 Hz, `Movement.before(SyncBackend)` |
| Autorité d'un seul côté | ✅ ouverte — 0 dépendance réseau dans V2 |
| Contrat de *slot* (compagnon) | ⚠️ non commencé — `forgia-navmesh` (1 123 l., `vleue_navigator`) n'a **que 3 consommateurs**, tous dans l'arène |
| **Notion de faction** | ❌ **écrite, jamais branchée** |

`forgia_core::faction::Faction` existe : 4 variantes, `is_friendly_to`,
`is_hostile_to`, tests. **Consommateurs hors de `forgia-core` : zéro.**

```
$ grep -rn "Faction" --include=*.rs crates/ src/ | grep -v "^crates/forgia-core/"
(rien)
```

Le camp d'une entité est aujourd'hui porté par **`forgia_ai_arena_bot::ArenaBot`**
(le marqueur ennemi) et **`BotTarget`** (posé par `forgia-player`). C'est
exactement le codebase que le GDD décrit comme non-rétrofitable : *« un codebase
qui suppose "le joueur contre les ennemis" ne se rétrofite pas en 5v5 »*.

La porte n'est pas encore fermée — mais elle ne s'ouvrira pas toute seule : tant
que `Faction` a 0 consommateur, elle coûte 0 et ne prouve rien.

---

## 4. Constat C — le plus coûteux : une **règle** dimensionnée sur une donnée morte

`.claude/rules/on-demand/map-design-intention.md` §2 s'ouvre sur :

> « Les archétypes existent déjà, en couche definition : `assets/genomes/enemies/`. »

Puis tient un tableau (grunt 30 pv / 9,0 m/s / vision 20 m ; archer 45 / 5,5 /
35 ; elite 120 / 5,0 / charge ×2,5) et en **dérive** toute la §2.1 : « un essaim
de 8 grunts avance 12,9 m », « la cour est trop grande pour son archétype de
mêlée », « 4 m de tir gratuit ».

Ces quatre fichiers **n'ont aucun consommateur Rust**. Le jeu lit
`assets/genomes/roguelite/roguelite_enemies.toml`, et les vrais archétypes sont :

| | pv | vitesse | détection | portée de tir |
|---|---:|---:|---:|---:|
| **tank** | 120 | 2,8 m/s | 22 m | 5 m |
| **runner** | 35 | 7,0 m/s | 40 m | 8 m |
| **sniper** | 45 | 3,2 m/s | 55 m | 28 m |
| **boss** | 800 | 3,5 m/s | 80 m | 32 m |

Aucun n'est en mêlée pure. Aucun ne charge. Le plus lent est le plus résistant —
l'inverse du modèle « essaim rapide » sur lequel la règle raisonne.

**Ce que ça coûte** : la prochaine fois que `/map` est invoqué, la règle me fait
dimensionner une salle contre des ennemis qui n'existent pas, et le résultat
*aura l'air* sourcé. C'est le défaut le plus cher du lot parce qu'il se propage
dans les livraisons suivantes sans jamais échouer.

**Corrigé dans cette passe** (§6). **Empêché de revenir** par C2 du cliquet.

---

## 5. Constat D — 11 plugins déclarés que rien ne monte

| Plugin | Fichier | Lecture |
|---|---|---|
| `ForgiaPostProcessPlugin` · `ForgiaPpOutlinePlugin` | `forgia-postprocess` | seul `ForgiaPpToonPlugin` est monté — et **par `forgia-mode-roguelite` uniquement** (`lib.rs:270`). D'où l'Expédition en PBR nu, et l'alerte capteur `toon strength>0 mais 0 Camera3d attached` |
| `ForgiaGenomeCorePlugin` · `ForgiaManifestPlugin` | `forgia-genome-core` | la crate sert par ses types, jamais par son plugin |
| `ForgiaUiLibPlugin` · `ForgiaJuiceLibPlugin` | méta-plugins | les sous-plugins sont montés un par un depuis l'assemblage |
| `ForgiaIkPlugin` · `ForgiaRigTopologyPlugin` | outillage rig | `forgia-ik` a été retiré de l'Expédition le 2026-08-18 (cf. son `Cargo.toml`) |
| `ForgiaPcgStreamPlugin` | `forgia-pcg-runtime` | crate **orpheline** aussi côté Cargo (`xtask check-orphans`) |
| `ForgiaPackRegistryPlugin` · `ForgiaVfxTracersPlugin` | | jamais atteints |

Certains sont légitimes (un méta-plugin qu'on a choisi d'éclater). C'est
justement pourquoi ils entrent en **ligne de base** plutôt qu'en défaut : le
cliquet interdit les **nouveaux**, il n'exige pas de solder les anciens.

### Pourquoi `xtask plugin-gate` ne les voyait pas

Il déclare honnêtement sa portée — et elle est étroite :

```
[plugin-gate] 37 plugin(s) contrôlé(s) sur 3 crate(s) · 41 manquement(s) toléré(s)
  portée : montage cherché dans les blocs #[cfg(test)] de la crate qui déclare le plugin
```

**37 sur 147 = 25 %.** Il vérifie qu'un plugin a un *garde de test*, pas qu'il
**tourne dans le jeu**. Les deux questions sont différentes, et c'est la seconde
qui manquait. `strates.py` C1 la pose : reachability depuis `forgia-game::run`.

---

## 6. Ce qui a été livré dans cette passe

| # | Livrable | Fichier |
|---|---|---|
| 1 | Le contrôle mécanique, 4 classes, ligne de base qui ne peut que rétrécir | `tools/ai/strates.py` · `docs/audit/strates-baseline.txt` (98) |
| 2 | Ce rapport | `docs/audit/audit-2026-08-18-strates-the-spared.md` |
| 3 | Le tableau d'archétypes corrigé + le renvoi vers la vraie source | `.claude/rules/on-demand/map-design-intention.md` §2 |
| 4 | Mémoire | `reference_strates_the_spared.md` · `feedback_une_regle_qui_cite_une_donnee_morte.md` |

**Aucun fichier `.rs` touché** — un autre terminal était actif.

---

## 7. Le process, simplifié — trois questions avant d'écrire du code

Ce que cet audit change concrètement pour la prochaine session. Remplace la
lecture de six fichiers par trois questions et une commande.

```bash
python tools/ai/strates.py --strict   # 5 s, sortie 1 si un lien casse
```

**Q1. Où va ce code ?** Si le concept touche le profil du joueur — identité,
équipement, cosmétiques, progression, boutique méta, chapitres — il est
**aujourd'hui dans `forgia-mode-roguelite`**, et c'est là qu'il faut le
compléter. Ne pas en écrire un second dans la zone où on travaille : c'est le
défaut qui a produit deux avatars superposés le 2026-08-14.

**Q2. Cette valeur, qui la lit vraiment ?** Un fichier de `assets/genomes/` sur
deux n'est lu par personne. Avant de citer un TOML — dans du code, dans une
story, dans une règle — vérifier :

```bash
grep -rl "$(basename FICHIER .toml)" --include=*.rs crates/ src/
```

Zéro résultat = donnée morte, quelle que soit sa cohérence interne.

**Q3. Cette ressource, qui l'écrit ?** Si elle est déclarée dans une crate
partagée et que son seul écrivain est gaté sur un `GameMode`, alors la zone
suivante hérite de sa valeur. Prévoir le `OnExit`, pas un garde défensif chez
chaque voisin.

---

## 8. Ce que cet audit ne couvre PAS

Honnêteté de portée. Il ne dit rien de : la **justesse** de ce qui est branché
(un lien peut exister et être mauvais) · les **dépendances tierces** (bevy,
rapier, hanabi) · le **contenu** des génomes · l'**ordre des systèmes** et les
conflits de schedule · la **performance** · le fait qu'un plugin atteint fasse
réellement quelque chose · les 61 génomes morts pris un par un — certains sont
du contenu futur légitime, aucun n'a été supprimé ici.

Et il ne tranche **aucune** décision de design : que l'Expédition n'ait ni HUD
ni ennemis est un **état**, pas un verdict.

---

## 9. Cross-refs

`.claude/rules/controle-de-la-sortie.md` (déclarer sa portée, mesurer la sortie,
naître avec sa ligne de base — les trois sont appliquées ici) ·
`.claude/rules/fine-grained-crates.md` (l'arbre de décision que C4 mesure) ·
`docs/design/gdd-forgia-the-spared.md` §10 (les quatre portes) ·
`.claude/rules/on-demand/map-design-intention.md` §2 (corrigé par cette passe) ·
`xtask plugin-gate` / `deps-mortes` / `check-orphans` (les trois cliquets
existants, dont `strates.py` couvre l'angle mort).
