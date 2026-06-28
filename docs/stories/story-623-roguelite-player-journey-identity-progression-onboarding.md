# Story-623 — Parcours joueur Roguelite : Identité + Progression + Onboarding

**Statut** : EN COURS — **Phase E (identité MVP) livrée** 2026-06-25 (`identity.rs` isolé : nom par défaut + édition non-bloquante par presets/texte + couleurs cosmétiques, save `identity_save.toml`, presets `roguelite_identity.toml`, sensor `forgia2_identity.json`, 5 tests verts). **Reste MVP : Phase G (juice Âmes)**. (Design original révisé 2026-06-25 après critique adversariale.)
**Niveau BMAD** : Enterprise (≥2 crates touchées, 6 phases livrables séparément). **MVP** = Standard (E-min + G-min, ~2 modules + 2 fichiers existants).
**Vision** : SHIP Roguelite (priorité 1). Un nouveau joueur — profil casual/kid style Roblox — doit pouvoir se sentir **propriétaire** de son personnage (nom + couleur simple), choisir son arme, **collecter ses Âmes avec plaisir** et (plus tard) **voir son niveau monter**, le tout cousu dans un onboarding qui ne perd personne.

> **Pivot validé (project_vision_pivot 2026-06-04)** : Forgia = moteur IA-natif, ship le Roguelite type Gunfire Reborn. Cette story sert le SHIP : elle transforme une boucle fonctionnelle mais anonyme (« le joueur n'a pas de nom, pas d'identité ») en une boucle **attachante et lisible**, sans ajouter de friction ni trahir les principes de la story-597.

> **Note révision 2026-06-25** : suite à critique adversariale, le MVP §9 est réduit de 3 features à 2 (**E-min + G-min**). Phase F (niveau/XP) — la plus risquée (recalcul, persistance, cross-feature) — est **sortie du MVP** et devient un incrément suiveur. Détail des arbitrages : §13.

---

## 0. Recherche fondatrice (2026-06-25 — voir aussi research story-597 du 2026-06-11)

Tous les chiffres sont recoupés sur 2 sources minimum (URLs dans la synthèse session). Lignes directrices retenues :

1. **Sélection, jamais création.** Aucun roguelite à succès n'a d'éditeur de perso (sliders). Modèle dominant : 1-2 héros gratuits + reste débloqué en monnaie méta (Gunfire, RoR2, Roboquest). Dead Cells : **l'arme EST l'identité**. → Forgia : pas d'éditeur ; l'arme + une **couleur cosmétique légère** (style Roblox), cosmétique pur.
2. **Jouer AVANT de nommer.** Les meilleurs jeux gardent 76-83 % des joueurs à 10 min vs 42-54 % pour les pires (Google Play / deltaDNA). Apple/Inworld/NN-g : différer le setup, mettre le gameplay < 30 s. → Nom demandé **après la 1re mort**, jamais en porte d'entrée. Nom **par défaut généré** (preset forge), éditable **sans jamais forcer**.
3. **Kids = pas de clavier obligatoire.** Lecture + motricité fine limitées chez les 6-8 ans (NN/g, Gapsy). → Presets cliquables (icônes), champ texte **optionnel** pour les plus grands. Modèle Roblox display-name.
4. **Niveau de compte : OUI mais en mode « titre/cosmétique », PAS « stat permanente ».** RoR2 = zéro stat méta (skill > grind) ; la critique documentée (ResetEra) : le stat-meta « ruine » le roguelite. Garde-fou Hades = séparer puissance (Miroir/Enclume) et difficulté (Heat). → **Le niveau Forgia ne donne JAMAIS de stat NI ne gate l'accès à la puissance** : il débloque titres/cosmétiques. L'Enclume garde **le monopole de la puissance ET son accès libre dès le départ**.
5. **XP ≠ Âmes, et XP versée même en défaite.** Gunfire : Soul Essence (= dépense) distincte du niveau de Talent. DRG : XP de participation. → **Âmes = monnaie de dépense (inchangée)** ; **XP = participation (vagues + boss + survie), versée même à la mort** (« mort = professeur », cohérent 597).
6. **Cadence niveau : vite au début.** 1→2 en 1 run (même un échec), 1→5 en ~3-4 runs ; le juice (anim+son+pop-up) au level-up est critique (GDC « Juice It or Lose It »).
7. **Collecte d'Âmes satisfaisante.** Aimantation (tween vers joueur), son layered + pitch ramping en série, pop/scale à l'absorption, compteur HUD qui bounce. La mort affiche « +N Âmes gagnées » AVANT le bouton retour (Supergiant « every run counts »).

---

## 1. Principes VERROUILLÉS (hérités 597 + nouveaux — ne pas affaiblir à l'implémentation)

**Hérités de story-597 (inchangés)** :
- **P1 — Aucun écran/niveau « tutoriel »** : la run 1 enseigne en jouant ; les hints sont la seule couche explicite.
- **P2 — Hint = one-shot À VIE**, ≤ 8 mots, vocabulaire enfant 10 ans, voix Maître Forgeron, jamais bloquant.
- **P3 — Le gating des éléments (`ElementUnlocks`) EST la progressive disclosure** : ne pas le court-circuiter.
- **P4 — Le DPS affiché ne peut pas mentir** : même fonction que le dégât réel (voir Phase D de 597).
- **P5 — Zéro hardcode** : textes, déclencheurs, échelles, presets = TOML/genome hot-reloadable.

**Nouveaux principes verrouillés de cette story** :
- **P6 — Identité = SÉLECTION, jamais CRÉATION.** Presets cosmétiques (couleur), pas de sliders. Test « créateur 14 ans » (`creator-simplicity.md`) obligatoire sur tout paramètre exposé.
- **P7 — Nom JAMAIS forcé, JAMAIS avant la 1re run.** Défaut généré au boot (preset forge), édition **différée, optionnelle et non-bloquante** (bouton crayon au Lobby, aucun modal qui interrompt le « rejouer »). Clavier jamais obligatoire pour un kid.
- **P8 — Le niveau de compte ne donne JAMAIS de stat de puissance ET ne gate JAMAIS l'accès à l'Enclume.** Il débloque uniquement titres/cosmétiques. L'Enclume garde le monopole stat ET reste librement accessible dès le départ (ne pas ralentir l'accès à la puissance pour un kid). Garde-fou : si un jour le niveau touche la puissance, copier le modèle Heat/Pacte (knob de difficulté optionnel).
- **P9 — XP de participation, versée même en défaite.** XP ≠ Âmes (deux compteurs distincts, jamais mélangés dans le même widget). « Toute run paie » s'applique aux deux.
- **P10 — Un profil unique auto-save.** Pas de save-slots multiples (sur-ingénierie pour un casual solo). Le nom = clé naturelle d'un futur slot, archi gardée prête, multi-slot NON construit.

---

## 2. Vision & FLOW d'onboarding idéal (étape par étape, depuis le 1er lancement)

Référence du flow existant (scout) : `Boot → Menu → RunState::Lobby → InRun{0..} → Boss → Defeat/Victory → Lobby/Menu`. On **n'insère AUCUN écran avant le gameplay**.

| # | Étape | Quand | SHIP-critique ? | Principe |
|---|---|---|---|---|
| 1 | Menu → bouton **« 🎲 ROGUELITE RUN »** | Boot | déjà fait | — |
| 2 | **Run 1 démarre, Pépin imposé**, zéro choix de perso | 1er lancement | **SHIP** | P1, P7 |
| 3 | Hints contextuels ≤8 mots (1 notion à la fois) | en run | **SHIP** (597 Phase A) | P2, P3 |
| 4 | 1re vague nettoyée = **win-state** + **Âmes magnétisées juicy** | en run | **SHIP** | P9 (juice) |
| 5 | Mort run 1 → écran Defeat : **« +N Âmes gagnées »** + 3 lignes prof + flèche Enclume | 1re mort | **fait** (597 Phase B inc1) | P9, mort=prof |
| 6 | Retour Lobby → nom par défaut **déjà affiché** + **bouton crayon** pour éditer (jamais forcé) | post-mort run 1 | **SHIP** | P7, P6 |
| 7 | **Pop-up « NIVEAU 2 ! » juicy** (XP de la run 1 versée) | retour Lobby | nice-to-have fort (hors MVP) | P8, P9 |
| 8 | Enclume : **1 seul achat fléché** (Vitalité) + aperçu Âmes — **accès libre dès le départ** | 1re visite | **SHIP** (597) | P8 |
| 9 | Relancer → **wizard arme** (Pépin surligné « Recommandé », ← →) | run 2 | déjà fait (612/613) | — |
| 10 | **Sélection couleur** débloquée en Âmes (chapeau = backlog, voir §10) | run 2+ | nice-to-have | P6 |

**Règle d'or mesurée** : rien entre le lancement et le 1er tir. Identité et niveau se **gagnent par la mort**, pas par un menu d'accueil. **Aucun modal one-shot ne bloque le retour au jeu** (le nom s'édite quand le joueur veut, ou jamais).

---

## 3. Découpage en PHASES livrables indépendamment

Phases A/B/C/D = continuité directe de story-597 (héritées, statut rappelé). Phases E/F/G/H = nouvelles. **Chaque phase ship seule** ; l'ordre recommandé est **E → G (= MVP §9)**, puis F, puis H, puis le reliquat A/D de 597.

### Phase A — Hints contextuels (HÉRITÉE 597) — À FAIRE
Voir story-597 §Phase A (catalogue `roguelite_hints.toml`, triggers `first_pickup`/`first_elemental_hit`/etc., bulle BD courte, persistance `ftue_save.toml`). **Aucun changement de design ici** ; rappelée pour le flow §2 étape 3. SHIP-critique.

### Phase B — Script de première mort (HÉRITÉE 597) — **DÉJÀ FAIT (inc1, 2026-06-19)**
`ftue.rs` + `FtueSave` (`ftue_save.toml`) + `forgia2_ftue.json` + `hud::draw_defeat_overlay` (1re mort = 3 lignes pédago + flèche Enclume + Âmes forgées cette run). Flag one-shot `first_death_recap_seen`. **Reste** (inc suivant 597) : dialogue 3 lignes au **retour Lobby** + highlight Enclume. SHIP-critique (le cœur est livré).

### Phase C — Sensor funnel `forgia2_ftue.json` (HÉRITÉE 597) — partiel
Champs funnel (`first_kill_secs`, `first_death_secs`, `hints_seen`, `return_run2`…). Étendre avec les nouveaux signaux Identité/Niveau (voir Phases E/F sensors). Enregistré au sensor-registry (gate story-546/620).

### Phase D — Lisibilité de puissance / DPS (HÉRITÉE 597) — À FAIRE
Voir story-597 §Phase D (`effective_dps()` pure, golden test anti-mensonge, sensor `forgia2_power.json`). Inchangé. SHIP-critique pour la lisibilité, mais indépendant de l'identité/niveau.

---

### Phase E — IDENTITÉ : nom + couleur (NOUVELLE) — SHIP-critique (MVP)

**Objectif** : le joueur a un nom et une couleur à lui, choisis SANS friction, APRÈS la 1re run. Le nom apparaît sur les écrans Defeat/Victory (« **\<Nom\> est tombé vague 3 — 840 Âmes** ») → la mort devient affective.

**Reco tranchée** :
- **Création vs sélection → SÉLECTION** (P6). Pas d'éditeur, pas de sliders. Apparence = un **preset de couleur** (couleur de tenue) parmi un petit roster, cosmétique pur (zéro impact gameplay, lisibilité kid, pas de pay-to-win).
- **Nom avant ou après run 1 → APRÈS, et jamais forcé** (P7). Au boot, un nom **par défaut généré** (preset thématique forge, ex. « Forgeron-Écarlate », « Petit-Marteau ») est attribué silencieusement. Au 1er retour Lobby post-mort, le nom par défaut est **déjà affiché** ; un **bouton crayon non-bloquant** permet de l'éditer quand le joueur veut. **Aucun modal one-shot** n'interrompt le flux « rejouer ».
- **Saisie kid-friendly** : ouvrir l'éditeur (clic crayon) affiche une roue de **presets de noms cliquables** (icônes, 0 lecture requise) + champ texte `egui::text_edit_singleline` **facultatif** pour les plus grands (modèle Roblox display-name : non-unique, sans friction). Aucun clavier obligatoire. Fermer sans choisir = garde le défaut.

> **Décision MVP (critique §3)** : pas de modal de prompt forcé. Le nom par défaut est valide et affiché immédiatement ; l'édition est une action volontaire via bouton crayon. Le flux « rejouer » n'est jamais interrompu.

**Design concret** :
- Nouveau module `crates/forgia-mode-roguelite/src/identity.rs` + `IdentityPlugin`.
- Resource runtime `PlayerIdentity { name: String, skin_id: String, named: bool }` (`named` = true dès qu'un défaut est attribué ; un flag séparé `name_edited: bool` trace l'édition volontaire pour le funnel).
- Bouton crayon `draw_name_edit_button` au Lobby (EguiPrimaryContextPass, non-bloquant) ; ouvre un panneau d'édition latéral, pas un modal plein écran.
- Panneau **couleur** au Lobby (side-panel GAUCHE : perso à gauche, arme au centre via weapon_select, Enclume à droite — alignement scout §3). Cliquer un preset = preview immédiate sur le perso.
- Couleurs **purement cosmétiques** : appliquer une couleur de `StandardMaterial` sur la tenue ; AUCUNE modif de stat (P6/P8), AUCUN mesh enfant ajouté (voir §10 pour le chapeau).

> **Décision MVP (critique §2)** : **pas de mesh chapeau** au MVP. Attacher un mesh enfant suppose un point d'ancrage de tête fiable, non vérifié — et la mémoire « Mixamo rig non interchangeable » + story-601 (le moteur n'anime que Rex) imposent un spike rig préalable. MVP = couleur de matériau seulement. Chapeau = backlog §10 avec spike d'ancrage.

> **Décision MVP (critique §1)** : **pas d'achat de skin en Âmes au MVP** (évite le câblage économie + meta_shop + preview pour un cosmétique). MVP = nom + 1 couleur par défaut affichée + sélection parmi les couleurs **gratuites** déjà débloquées. L'achat de couleurs en Âmes = backlog §10.

**Data-driven (no-hardcode, P5)** :
- `assets/genomes/roguelite/roguelite_identity.toml` (nouveau) :
  ```toml
  [[name_preset]]
  id = "forgeron_ecarlate"
  label = "Forgeron Écarlate"

  [[skin]]
  id = "default"
  label = "Apprenti"
  color = [0.8, 0.5, 0.2]   # RGB tenue
  souls_cost = 0            # défaut gratuit

  [[skin]]
  id = "azur"
  label = "Azur"
  color = [0.2, 0.4, 0.9]
  souls_cost = 0            # MVP : gratuit ; coût Âmes = backlog §10
  ```
- Labels respectent `creator-simplicity.md` : labels courts (« Tenue »), max 5-8 visibles, valeurs par défaut « ça marche ».

**Persistance (P10, profil unique)** : **étendre `MetaShopSave`** (déjà la source des déblocages permanents en Âmes — un seul fichier de progression cohérent). Ajouter, versionné :
```toml
player_name = "Forgeron Écarlate"
name_edited = false
equipped_skin = "default"
unlocked_skins = ["default"]
```
Save atomique (pattern `persist.rs` write-tmp→rename) sur : édition nom validée, changement couleur, OnExit Roguelite, OnEnter Victory/Defeat. Migration version-safe (défauts si champs absents → compat saves existants).

**Sensor (observability-required)** : `forgia2_identity.json` (1 Hz), champs :
`{ "named": bool, "name_edited": bool, "name_len": u32, "equipped_skin": str, "unlocked_skins_count": u32, "edit_button_shown": bool, "timestamp_secs": f32 }`.
Health check : `IDENTITY_EDIT_UNREACHABLE` (severity warn) si `first_death_recap_seen == true` mais `edit_button_shown == false` après N retours Lobby (le bouton d'édition ne s'affiche pas → identité inaccessible). Enregistré au sensor-registry (gate 620).

> **Décision (critique §2)** : le health check ne se déclenche **pas** sur « jamais nommé » (ce serait un faux positif : garder le défaut est un choix légitime, P7). Il se déclenche si le **moyen d'éditer** est inaccessible.

**ACs testables (Phase E)** :
- [ ] check + clippy 0 sur `forgia-mode-roguelite`
- [ ] Test pur : round-trip `MetaShopSave` étendu (name/name_edited/skin) ; migration depuis un save sans ces champs → défauts appliqués, pas de panic
- [ ] Test pur : sélection d'une couleur non débloquée → refusée (pas d'équipement d'un skin absent de `unlocked_skins`)
- [ ] Runtime : nouvelle partie (saves supprimés) → run 1 SANS aucun prompt nom (P7) ; après 1re mort + retour Lobby → nom par défaut affiché + bouton crayon visible, **le retour au jeu n'est jamais bloqué**
- [ ] Runtime : éditer le nom via preset cliquable (sans clavier) → s'affiche sur l'écran Defeat suivant « \<Nom\> est tombé… »
- [ ] Runtime : changer de couleur → perso change immédiatement, persiste après relance
- [ ] Sensor `forgia2_identity.json` écrit, enregistré au registry ; health `IDENTITY_EDIT_UNREACHABLE` déclenche si bouton jamais montré

---

### Phase F — NIVEAU / XP (NOUVELLE) — nice-to-have fort (HORS MVP, incrément suiveur)

> **Décision (critique §1)** : Phase F est **sortie du MVP**. La story-623 elle-même note « le jeu ship sans » (§7). C'est le morceau le plus risqué (recalcul, nouvelle resource persistée, cross-feature avec l'Enclume). Elle reste pleinement spécifiée ici et se livre en incrément suiveur, après E+G validés runtime.

**Objectif** : le joueur VOIT son niveau monter (« tu progresses ») sans que ça touche sa puissance (P8) ni doublonne l'Enclume.

**Reco tranchée** :
- **Niveau de compte OUI, en mode titre/cosmétique** (P8). Jamais de stat, **jamais de gate d'accès à l'Enclume**.
- **XP = participation versée même en défaite** (P9) : `XP_per_wave`, `XP_per_boss`, `XP_per_run_survived_secs` (data-driven). Pas 1 XP = 1 Âme : compteurs distincts, widgets distincts (niveau = bandeau Lobby ; Âmes = compteur Enclume).
- **Ce que le niveau débloque** : **titres et presets cosmétiques uniquement**. Le niveau dit « tu avances » ; l'Enclume (librement accessible dès le départ) dit « tu choisis ta puissance ».

> **Décision (critique §2)** : **pas de gate d'onglet Enclume par niveau**. Verrouiller la puissance derrière le niveau ralentit l'accès pour un kid — l'inverse de l'onboarding. Le gate de contenu vit déjà dans l'Enclume (paliers boons story-616), il n'a pas besoin du niveau de compte. Cela retire aussi le doublon de progression Lobby (critique §4).

- **Cadence** : 1→2 en 1 run (même échec), 1→5 en ~3-4 runs ; courbe douce d'abord, raide ensuite.

**Design concret** :
- Module `crates/forgia-mode-roguelite/src/player_level.rs` (mode-specific, 1 seul consommateur réel = roguelite).

> **Décision (critique §1)** : `player_level.rs` vit dans `forgia-mode-roguelite`, **PAS** dans `forgia-rpg-data`. Justifier une crate partagée par « futur RPG » viole `fine-grained-crates.md` §5 (créer au besoin, pas en réserve — 1 consommateur réel). Le ratchet `no-scaffold`/arch-drift mordrait. Extraction en module data partagé seulement si/quand un 2e consommateur réel apparaît.

- Resource `PlayerProgress { total_xp: u64, level: u32, xp_into_level: u64, xp_for_next: u64 }`.
- Fonction **pure** `level_for_xp(total_xp, &curve) -> (level, into, for_next)` (testable, déterministe, recalcul **sur événement** uniquement — fin de vague/boss/run — RIEN par frame, hot path safe).
- HUD Lobby : **bandeau permanent** (niveau + barre XP) à côté du compteur d'Âmes, jamais le même widget (P8). Pop-up « NIVEAU N ! » juicy au retour de run (anim scale + son, GDC juice).

**Data-driven (P5)** : `assets/genomes/roguelite/roguelite_progression.toml` :
```toml
[xp_rewards]
per_wave = 100
per_boss = 500
per_run_second = 1        # survie récompensée même en défaite

[curve]                    # XP cumulée requise par niveau
thresholds = [0, 300, 800, 1600, 2800, 4500]   # douce->raide

[[unlock]]
level = 5
kind = "cosmetic"
target = "title_apprenti_aguerri"
```

**Persistance** : `total_xp` + `level` dans `MetaShopSave` (versionné, défauts si absents).

**Sensor** : `forgia2_progression.json` (1 Hz) : `{ "level", "total_xp", "xp_into_level", "xp_for_next", "xp_earned_run", "level_recalc_calls", "last_levelup_secs", "timestamp_secs" }`. Health `PROGRESSION_FROZEN` (warn) : run complétée mais `xp_earned_run == 0` (XP ne coule pas). Enregistré au registry.

> **Décision (critique §5)** : le compteur `level_recalc_calls` est exposé au sensor — il rend l'AC « jamais par frame » **mécaniquement testable** (assert runtime : `level_recalc_calls` ≈ nombre d'événements, pas ~60/s), au lieu d'une vérif code-review non falsifiable.

**ACs testables (Phase F)** :
- [ ] check + clippy 0
- [ ] Test pur : `level_for_xp` monotone, déterministe, frontières exactes (`thresholds[n]` → niveau n) ; courbe hot-reloadée → recalcul cohérent
- [ ] Test pur : XP versée **identique** en Victory et en Defeat pour les mêmes vagues franchies (P9)
- [ ] Runtime : `forgia2_progression.json → level_recalc_calls` reste proportionnel aux événements (vagues/boss/run), **PAS** ~60/s → preuve que le recalcul est `on_event`/`OnEnter`, pas `Update` nu
- [ ] Runtime : finir run 1 (même en mourant) → retour Lobby → barre XP se remplit + pop-up « NIVEAU 2 ! »
- [ ] Le niveau ne modifie AUCUNE stat (PV/dégâts inchangés au level-up) — assert via sensor combat/power
- [ ] Le niveau ne gate AUCUN onglet Enclume (accès puissance libre) — vérif runtime onglets ouverts dès niveau 1
- [ ] Sensor `forgia2_progression.json` écrit + registry + health `PROGRESSION_FROZEN`

---

### Phase G — JUICE de collecte d'Âmes (NOUVELLE) — SHIP-critique (MVP)

**Objectif** : ramasser une Âme = micro-événement sensoriel gratifiant (game feel, rétention kid). Aujourd'hui la collecte est un compteur silencieux (`sys_pickup_souls` walk-over).

**Reco tranchée** : appliquer le playbook « Juice It or Lose It » sur le wisp d'Âme existant, sans toucher l'économie (montants inchangés, P-no-regress story-558).

**Design concret** (sur le système de wisps existant `run.rs` `sys_pickup_souls`) :
- **Aimantation** : à portée, le wisp vole vers le joueur (tween/easing), ramassé à l'absorption, plus sur contact sec.
- **Pop/scale** (squash & stretch) à l'absorption + sparkle (réutiliser pipeline VFX hanabi existant — `reference_hanabi_vfx_on_enemies`).
- **Son layered + pitch ramping** : « bling » + whoosh ; **pitch qui monte en série** quand plusieurs Âmes arrivent (combo) — réutilise `bevy_kira_audio`.
- **Compteur HUD animé** : le total d'Âmes **bounce/roule** vers le haut, jamais un saut instantané ; « ding » de fin de vague.
- **Écran Defeat** (déjà là, Phase B) : « **+N Âmes gagnées cette run** » en gros AVANT le bouton + « Plus que X Âmes pour \<prochain achat\> ».

**Data-driven (P5)** : section dans `roguelite_run.toml` (ou nouveau `roguelite_juice.toml`) :
```toml
[souls_juice]
magnet_radius = 4.0
magnet_speed = 12.0
absorb_scale_pop = 1.4
pitch_base = 1.0
pitch_step = 0.06         # +6% par Âme en série
pitch_max = 1.8
hud_counter_lerp = 8.0    # vitesse du roulement compteur
```
Tout en TOML, hot-reload. Bornes « sliders » pour respecter `creator-simplicity.md`.

**Sensor** : étendre `forgia2_economy.json` (ou `forgia2_run.json`) : `{ "souls_collected_this_run", "souls_collect_events", "last_collect_secs" }` → permet de **valider que le juice fire** (chaque collecte = un event visible). Health `SOULS_SILENT_COLLECT` (info/warn) : Âmes gagnées (`earned_run` monte) mais 0 `souls_collect_events` → le juice ne se déclenche pas.

**Régression économie (critique §5)** :

> **Pré-requis bloquant** : avant impl, vérifier qu'un golden test d'économie existe (montants Âmes/vague/boss). **Recherche au scout : à confirmer.** Si absent → **le créer dans cet incrément** (golden : run scriptée déterministe → total Âmes attendu par vague/boss vs `roguelite_run.toml`). Sans golden, l'AC « économie inchangée » est un trou ; le juice ne doit modifier QUE le ressenti, pas les montants.

**ACs testables (Phase G)** :
- [ ] check + clippy 0 ; **aucune régression d'économie** — golden test économie présent (créé si absent, voir pré-requis) → montants Âmes/vague/boss identiques avant/après juice
- [ ] Test : aimantation = fonction de mouvement bornée (pas d'alloc hot path, tween sur composant)
- [ ] Runtime : tuer un ennemi → l'Âme vole vers le joueur, pop+sparkle à l'absorption, compteur HUD roule (pas de saut)
- [ ] Runtime : série de kills → pitch du son monte progressivement, plafonné
- [ ] Runtime : écran Defeat → « +N Âmes » affiché en gros + ligne « plus que X pour … »
- [ ] Sensor `forgia2_economy.json` : `souls_collect_events` > 0 quand on ramasse ; health `SOULS_SILENT_COLLECT` couvre le cas muet
- [ ] Hot-reload `magnet_speed`/`pitch_step` → effet visible sans rebuild

---

### Phase H — Couture du FLOW d'onboarding (NOUVELLE) — nice-to-have (polish, hors MVP)

**Objectif** : orchestrer E+F+G+597 en une séquence sans couture qui suit le tableau §2, et instrumenter le funnel complet.

**Design concret** :
- Système d'orchestration léger (pas de god-file) qui garantit l'ordre §2 : récap mort (B) → pop-up niveau (F) au retour Lobby, sans collision de pop-up (un seul pop-up actif à la fois). **Le bouton crayon nom (E) n'est pas un modal** → pas de séquencement nécessaire avec lui (il coexiste sans bloquer).
- Garde « un pop-up à la fois » : si récap mort + pop-up niveau coïncident au 1er retour Lobby, les séquencer (queue), jamais empiler.
- Étendre `forgia2_ftue.json` (Phase C) avec : `name_edited_secs`, `first_levelup_secs`, `first_skin_changed` (bool) → funnel identité/progression complet.

**Sensor** : pas de nouveau fichier — agrège dans `forgia2_ftue.json` (funnel) les jalons E/F/G. Cibles directionnelles : 1er level-up vu run 1 ; le nom peut rester au défaut (pas une cible d'échec).

**ACs testables (Phase H)** :
- [ ] Au 1er retour Lobby post-mort : séquence ordonnée récap mort → pop-up niveau, **jamais 2 pop-up simultanés**
- [ ] `forgia2_ftue.json` contient `first_levelup_secs` peuplé après le 1er cycle ; `name_edited_secs` peuplé seulement si édition volontaire
- [ ] Aucun double-déclenchement (one-shot respecté à travers la queue)

---

## 4. Fichiers (crates RÉELS — vérifiés au scout)

| Fichier | Phase | Rôle |
|---|---|---|
| `crates/forgia-mode-roguelite/src/identity.rs` (nouveau) | E | `PlayerIdentity`, bouton/panneau édition nom, panneau couleur, `IdentityPlugin`, sensor identity |
| `assets/genomes/roguelite/roguelite_identity.toml` (nouveau) | E | presets nom + couleurs (gratuites au MVP) |
| `crates/forgia-mode-roguelite/src/player_level.rs` (nouveau) | F | `level_for_xp` pure + curve + tests (**mode-specific**, pas rpg-data) |
| `assets/genomes/roguelite/roguelite_progression.toml` (nouveau) | F | rewards XP + courbe + déblocages cosmétiques par niveau |
| `crates/forgia-mode-roguelite/src/run.rs` (existant) | F+G | versement XP (vague/boss/survie) ; aimantation Âmes dans `sys_pickup_souls` |
| `crates/forgia-mode-roguelite/src/hud.rs` (existant) | F+G+E | bandeau niveau+XP, compteur Âmes animé, nom sur Defeat/Victory |
| `crates/forgia-mode-roguelite/src/meta_shop.rs` (existant) | E (backlog)+F | achat couleur en Âmes (backlog §10) ; cosmétiques par niveau |
| `crates/forgia-mode-roguelite/src/persist.rs` + `MetaShopSave` (existant) | E+F | champs name/name_edited/skin/total_xp/level versionnés, migration safe |
| `assets/genomes/roguelite/roguelite_run.toml` (existant) ou `roguelite_juice.toml` (nouveau) | G | params juice collecte |
| `crates/forgia-mode-roguelite/src/sensor.rs` (existant) | E+F+G+H | `forgia2_identity.json`, `forgia2_progression.json`, extension `forgia2_economy.json` + `forgia2_ftue.json` |
| `crates/forgia-mode-roguelite/src/ftue.rs` (existant) | C+H | extension funnel (name_edited/levelup jalons) |
| `crates/forgia-mode-roguelite/src/lib.rs` (existant) | E+F | wire `IdentityPlugin` |

⚠️ **Multi-terminal** (`multi-terminal-coordination.md`) : `forgia-mode-roguelite` est gros et souvent touché. Au moment d'implémenter : standup (`git status -s`), claim (`git diff HEAD --name-only` sur les fichiers ci-dessus), build-baseline sain avant 1er Edit. Préférer créer les nouveaux modules isolés (`identity.rs`, `player_level.rs`) avant de toucher `run.rs`/`hud.rs` partagés.

---

## 5. Data-driven & observabilité (récap conformité règles projet)

- **no-hardcode (P5, `no-hardcode.md`)** : 3-4 fichiers TOML nouveaux/étendus ; tout texte, coût, courbe, paramètre juice est hot-reloadable. 0 constante magique en code.
- **creator-simplicity** : labels courts (« Tenue », « Niveau »), presets bornés, valeurs défaut « ça marche », test « ado Roblox » sur chaque paramètre exposé. Pas de sliders complexes.
- **observability-required** : 1 sensor par feature — `forgia2_identity.json` (E), `forgia2_progression.json` (F), extension `forgia2_economy.json` (G), funnel `forgia2_ftue.json` (H). Chacun avec health check + enregistrement au sensor-registry (gate 620). Seuils config-driven.
- **scalabilité / hot path** : recalcul niveau & DPS sur événement uniquement (jamais par frame, vérifié via `level_recalc_calls`) ; aimantation Âmes = tween sur composant, 0 alloc dans la boucle ; sensors 1 Hz.
- **kid-friendly** : nom différé/jamais forcé + presets cliquables (pas de clavier obligatoire), cosmétique sans pay-to-win, 1 pop-up à la fois, niveau sans stat ET sans gate d'accès (lisibilité + skill + accès puissance préservés).
- **fine-grained-crates** : `player_level.rs` **reste mode-specific** (`forgia-mode-roguelite`, 1 consommateur réel) ; extraction en data partagé seulement si 2e consommateur réel avéré.

---

## 6. Questions ouvertes — TRANCHÉES (avec justification recherche)

| Question | Décision | Justification |
|---|---|---|
| Création OU sélection de perso ? | **SÉLECTION** (presets couleur), jamais d'éditeur/sliders | Aucun roguelite à succès n'a d'éditeur ; Dead Cells = l'arme est l'identité ; sliders = friction kid (NN/g) |
| Nom AVANT ou APRÈS la 1re run ? | **APRÈS, et jamais forcé** (défaut affiché + bouton crayon), défaut généré au boot | Jouer < 30 s (Apple/deltaDNA) ; setup différé ; flux « rejouer » jamais interrompu |
| Saisie nom = clavier ? | **Presets cliquables + texte OPTIONNEL** | Lecture/motricité 6-8 ans limitées ; modèle Roblox display-name |
| Niveau de compte OUI/NON ? | **OUI**, en mode titre/cosmétique | « tu progresses » ; Gunfire/DRG ; visible = rétention |
| Le niveau donne-t-il des stats / gate l'Enclume ? | **NON aux deux** (titres/cosmétiques uniquement ; Enclume libre dès le départ) | Anti-pattern stat-meta (RoR2/ResetEra) ; gate d'accès = friction anti-onboarding kid ; Enclume = monopole + accès libre |
| XP gagnée comment ? | **Participation** (vague+boss+survie), **versée même en défaite**, ≠ Âmes | Gunfire (Talent ≠ dépense) ; « mort = professeur » ; deux compteurs distincts |
| Chapeau / mesh cosmétique ? | **BACKLOG** (couleur seule au MVP), spike rig préalable | Mémoire « rig non interchangeable » + story-601 (anim Rex only) ; ancrage tête non vérifié |
| Achat de couleurs en Âmes ? | **BACKLOG** (couleurs gratuites au MVP) | Évite câblage économie+meta_shop+preview pour du cosmétique non-critique au ship |
| Save-slots multiples ? | **NON** (1 profil auto-save) | Sur-ingénierie casual solo ; Hades auto-save ; nom = clé future de slot si besoin avéré |
| Skin/cosmétique = gameplay ? | **NON** (pur cosmétique) | Pas de pay-to-win, lisibilité kid (Roblox) |

---

## 7. SHIP-critique vs nice-to-have

**SHIP-critique (sans ça, l'onboarding kid est cassé)** :
- Phase E (identité : nom différé non-bloquant + couleur) — donne l'attachement
- Phase G (juice Âmes) — donne le plaisir de collecte (game feel)
- Phase A (hints 597) + Phase B reliquat (dialogue retour Lobby) — guident sans tutoriel
- Phase D (DPS 597) — lisibilité de puissance

**Nice-to-have (renforce, pas bloquant pour ship)** :
- Phase F (niveau/XP) — fort recommandé (rétention) mais le jeu ship sans → **incrément suiveur, hors MVP**
- Phase H (couture funnel + sensors avancés)
- Backlog §10

---

## 8. QA / Auto-QA (règle post-impl-auto-qa, story Enterprise)

Avant tout « DONE » par phase :
- `cargo check -p forgia-mode-roguelite` + clippy 0 warning
- Tests purs : voir ACs par phase (round-trip save+migration, `level_for_xp`, XP défaite=victoire, économie golden)
- Sub-agent **verifier** : build/lint, cohérence TOML→catalogue→consommateur (aucun preset mort), invariants intacts
- Sub-agent **qa-lead** : BUG REPORT (concurrence pop-up, alloc hot path aimantation, dérive numérique XP, double one-shot, recalcul par frame)
- **story-done-gate** : `cargo run -p xtask -- story-gate --story 623` vert (G1 git-tracked ; E/F mappent des fichiers réels → G3 LOC ; G4 si claim N tests). Si la story reste docs-only d'orchestration → skip-list justifié.

---

## 9. MVP RECOMMANDÉ (le plus petit incrément qui livre le besoin sans gold-plating)

> **Besoin user** : créer perso + nom + arme + âmes (+ niveau plus tard). Arme (612/613) et âmes (591) **existent déjà**. Le delta réel SHIP = **nom + juice de collecte**.
>
> **MVP = Phase E (identité min) + Phase G (juice Âmes)**, en réutilisant l'existant (weapon_select 612/613 = « arme » ; MetaSouls = « âmes » ; Defeat overlay 597 = « mort »).

Contenu MVP, dans l'ordre :
1. **E-min** : nom par défaut généré au boot, **affiché** au retour Lobby + **bouton crayon non-bloquant** (preset cliquable + texte optionnel) + sélection parmi couleurs **gratuites** (≥ 2). Nom affiché sur Defeat. Persist `MetaShopSave` étendu (migration safe). Sensor `forgia2_identity.json`. **Pas de chapeau, pas d'achat Âmes, pas de modal forcé.**
2. **G-min** : aimantation + pop + son des Âmes + compteur HUD animé + « +N Âmes » sur Defeat. Sensor `forgia2_economy.json` étendu. **Économie inchangée** (golden test présent ou créé).

Résultat MVP : un nouveau joueur fait sa run 1 (Pépin), meurt, voit son perso **nommé par défaut + éditable s'il veut + recolorable**, **voit ses Âmes voler vers lui avec du juice**, repart choisir/améliorer son arme (existant). Boucle d'attachement de base complète, **sans niveau tutoriel, sans friction clavier, sans modal bloquant, sans stat de niveau, sans crate partagée prématurée**.

Effort indicatif MVP : 1 module nouveau (`identity.rs`) + 2-3 fichiers existants touchés (`run.rs`, `hud.rs`, `persist.rs`) + 1-2 TOML + 2 sensors. **Niveau BMAD Standard.**

**Incréments suiveurs ordonnés** : F-min (niveau visible) → E-full (achat couleurs + chapeau après spike rig) → F-full (titres) → H (couture funnel).

---

## 10. Backlog des extensions (post-MVP)

- **Achat de couleurs en Âmes** (E : `souls_cost > 0` + câblage meta_shop + preview) — sorti du MVP par la critique.
- **Chapeaux / mesh cosmétiques** — **précédé d'un spike rig** (ancrage tête fiable ; mémoire « rig non interchangeable », story-601). Bloquant : confirmer un point d'attache stable avant tout mesh enfant.
- **Phase F (niveau/XP)** — incrément suiveur direct, déjà spécifié §Phase F (sans gate Enclume).
- **Roster de skins étendu** + onglet cosmétique dédié à l'Enclume.
- **Titres débloqués par niveau** (Phase F full).
- **Couture funnel + sensors avancés** (Phase H) : queue de pop-up, jalons `name_edited_secs`/`first_levelup_secs`.
- **Leaderboard local d'Âmes / meilleurs runs** indexé par nom (le nom devient clé).
- **Save-slots multiples** — UNIQUEMENT si télémétrie montre du partage de PC (P10).
- **Extraction `player_level` en data partagé** — UNIQUEMENT si un 2e consommateur réel (RPG) apparaît (`fine-grained-crates.md`).
- **Hints Phase A complète** (catalogue + 5 triggers) si pas livré avec 597.
- **DPS Phase D complète** (3 surfaces UI + golden test) si pas livré avec 597.
- **i18n** des presets nom/skin (`text_en`) si story-492 active.

---

## 11. Récap test in-game (règle in-game-test-recap — à servir à la livraison MVP)

1. **Action** : supprimer `config/meta_shop_save.toml` + `config/ftue_save.toml` → `cargo run -p forgia` → Roguelite → jouer run 1 avec Pépin, ramasser des Âmes, mourir.
2. **Redémarrage** : rebuild requis (`.rs`) ; ensuite TOML (`roguelite_identity.toml`, juice) hot-éditables.
3. **Effet attendu** : Âmes qui volent vers le joueur + pop/son (G) ; écran Defeat « \<NomDéfaut\> est tombé… + N Âmes » ; retour Lobby → nom par défaut affiché + bouton crayon (clic = presets sans clavier) ; changer couleur → perso change immédiatement. **Le retour au jeu n'est jamais bloqué par un modal.**
4. **Où observer** :
   - `forgia2_identity.json → named:true, name_edited:true, equipped_skin:"azur"` après édition+couleur
   - `forgia2_economy.json → souls_collect_events > 0`
5. **Variantes si KO** :
   - Bouton crayon absent → vérifier `first_death_recap_seen == true` dans `ftue_save.toml` + health `IDENTITY_EDIT_UNREACHABLE`
   - Âmes sans juice → health `SOULS_SILENT_COLLECT` + hot-reload `magnet_speed` ×2 pour confirmer le câblage
   - Économie modifiée par erreur → relancer le golden test économie
   - Aucun effet visible → artefact stale ? comparer `mtime(forgia.exe)` vs sources (`multi-terminal-coordination.md` §5), confirmer build de `-p forgia` (pas `-p forgia-game` stale)

Ping-moi quand tu as observé.

---

## 12. Cross-refs

- story-597 (FTUE — Phases A/B/C/D héritées ; Phase B inc1 fait)
- story-612 / 613 (wizard arme + déblocage — « choix d'arme » du flow)
- story-616 (paliers boons Enclume — gate de contenu existant ; remplace le besoin d'un gate par niveau)
- story-601 (anim A-pose / blocages rig — pré-requis du chapeau backlog §10)
- story-591 (Enclume / `MetaSouls` / `MetaShopSave`)
- story-558 (économie Âmes — montants à ne PAS régresser, Phase G)
- story-620 (sensor-registry hygiene — enregistrement des nouveaux sensors)
- `reference_hanabi_vfx_on_enemies` (pipeline VFX réutilisé pour le juice Âmes)
- `reference_apose_autorig_blockers_story_601` (blocages rig — chapeau backlog)
- Règles : `concept-first.md`, `no-hardcode.md`, `observability-required.md`, `creator-simplicity.md`, `fine-grained-crates.md`, `multi-terminal-coordination.md`, `post-impl-auto-qa.md`, `story-done-gate.md`, `in-game-test-recap.md`

---

## 13. Journal des arbitrages critique (2026-06-25)

Points VALIDES intégrés (8) :

| # | Critique | Action |
|---|---|---|
| 1a | Phase F sur-scope le MVP | F **sortie du MVP** → incrément suiveur (§9, §7) |
| 1b | `player_level.rs` en `forgia-rpg-data` viole fine-grained-crates | déplacé en **module `forgia-mode-roguelite`** (§Phase F, §4, §5) |
| 1c | Achat skins Âmes alourdit le MVP | **couleurs gratuites au MVP**, achat → backlog §10 |
| 2a | Mesh chapeau contredit l'anti-trap rig | **couleur seule au MVP**, chapeau → backlog avec spike rig (§Phase E, §10) |
| 2b | Gate onglet Enclume par niveau = friction onboarding | **gate retiré** ; P8 = niveau cosmétique pur, Enclume libre dès le départ (§Phase F) |
| 3 | Modal nom one-shot bloque le « rejouer » | **bouton crayon non-bloquant**, nom défaut affiché, jamais forcé (P7, §Phase E) |
| 5a | AC « jamais par frame » non testable | **compteur `level_recalc_calls`** au sensor, assert runtime (§Phase F) |
| 5b | Golden test économie pas garanti d'exister | **pré-requis bloquant** : vérifier/créer le golden avant juice (§Phase G) |

Point 4 (doublon de progression Lobby) : **résolu indirectement** par 2b — sans gate Enclume, le niveau n'est qu'un titre, pas une 2e jauge de puissance concurrente de l'Enclume.

Aucun point écarté comme faux positif : la critique était entièrement actionnable.
