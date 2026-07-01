# Plan de production — Forgia Roguelite (Gunfire-like), pour un premier jeu

> Rédigé par ton directeur de studio. Honnête, chiffré, sans bullshit. Objectif : t'emmener de l'état actuel jusqu'à un « Gunfire-like jouable » puis « shippable ». On garde l'ADN Forgia : chaque système = **genome TOML** + **sensor d'observabilité**.

---

## 1. Où tu en es (lecture pour débutant)

Tu n'es **pas** au point zéro, et c'est important que tu le saches avant de paniquer sur la longueur de ce document. Forgia a déjà une fondation qu'un dev solo mettrait 6 mois à construire : une **arène FPS jouable** (tir hitscan/projectile, 4 armes avec identités distinctes, recul/spread/falloff déterministes), un **game-feel de tir déjà riche** (hitmarker, damage numbers, camera shake trauma, hit-stop, muzzle flash 5 couches, tracers), une **boucle roguelite** (vagues → boss → boons empilables → méta-progression en Âmes, économie dual-monnaie), un **HUD à ~72 %** (munitions, minimap, slots, monnaie, barres de vie ennemies), et surtout deux choses rares chez un débutant : **tout est data-driven** (les valeurs vivent dans des fichiers `.toml` que tu modifies sans recompiler) et **tout est observable** (des capteurs `forgia2_*.json` écrivent l'état du jeu en continu, ce qui permet de diagnostiquer sans deviner). C'est un avantage énorme : tu peux tuner ton jeu par **mesure**, pas au feeling.

**Complétude vers un « Gunfire-like jouable » : ~40 %.** (Parité gameplay ≈ 45 %, parité level-design ≈ 38 %, production/ship ≈ 35-40 %.) Traduction concrète : le **moteur de tir** est là, mais les **quatre piliers qui font qu'on reconnaît Gunfire** manquent ou sont incomplets — (1) la défense tri-couche Vie/Bouclier/Armure, (2) les 3 réactions élémentaires, (3) les inscriptions d'arme, (4) la traversée de salles au lieu d'une arène qui boucle. Et le « reste » (audio, tutoriel, sauvegarde robuste, Steam) est à peine entamé.

---

## 2. Ce qui manque DANS TA DEMANDE — les piliers qu'un premier jeu oublie

Ta question porte sur les mécaniques (compétences F/Q, boucliers, éléments, armes). C'est légitime, mais c'est **la partie que tu maîtrises déjà mentalement**. Le vrai risque de ton premier jeu, ce sont les piliers que tu n'as **pas** mentionnés. La recherche est formelle : un dev solo sur-investit les mécaniques (~30 % du travail) et sous-investit les ~70 % qui décident si le jeu **sort, se vend et se joue**. Je les liste franchement, chacun avec : pourquoi c'est critique / où en est Forgia / le minimum viable.

### 2.1 🔴 GESTION DE SCOPE — le tueur n°1 (et tu es déjà en train de le déclencher)

**Pourquoi critique** : le scope creep est LA cause d'échec documentée des premiers jeux. Des projets de 9 mois deviennent 27 mois (×3). Ta demande contient déjà les germes : « répliquer Gunfire Reborn » = 71 armes, 148 scrolls, 4 actes, 7 boss, difficulté R1-R8. **Gunfire, c'est 5 ans et une équipe.** Si tu vises la parité 100 %, tu ne finiras jamais.
**Où en est Forgia** : le scope est ton risque n°1, pas ton code.
**Minimum viable** : 3-5 piliers cœur, backlog **gelé** (les idées vont dans une liste, PAS dans le code), timeboxing par tâche. **Ta cible réaliste = « Gunfire-lite »** : 6-8 armes, ~40 boons, **3 actes × ~4 salles**, 3-4 boss, 1 mode de difficulté + 1 palier d'ascension. Pas 71/148/4/7.

### 2.2 🔴 PLAYTESTING — commencer maintenant, pas « quand ce sera propre »

**Pourquoi critique** : c'est systématiquement repoussé, et c'est une erreur. Un jeu est testable **dès qu'une boucle jouable existe** — or tu en as déjà une. Sans testeurs externes, tu vas polir des systèmes que personne ne trouve fun.
**Où en est Forgia** : arène jouable ✅, mais **zéro trace de playtest structuré**.
**Minimum viable** : 3-5 testeurs externes toutes les 2-3 semaines, **1 seule question par session** (« l'onboarding est-il clair ? » OU « la difficulté est-elle juste ? », jamais les deux). Testeurs gratuits : amis, r/playmygame, Discords gamedev. Tu observes sans intervenir. Commence **dès la fin de la Phase P0-feel** ci-dessous.

### 2.3 🟠 GAME-FEEL / JUICE & LISIBILITÉ — ton meilleur ratio effort/qualité

**Pourquoi critique** : la recherche est nette — la **lisibilité** (télégraphe ennemi + attentes cohérentes) pèse **plus sur la frustration** que la difficulté brute. Les deux défauts célèbres de Gunfire à corriger chez toi : (a) **swap d'arme lent** qui décourage le multi-armes, (b) **morts « qui sortent de nulle part »** = échec de lisibilité du burst de dégâts entrant.
**Où en est Forgia** : le feel **offensif** est excellent (shake, hit-stop, muzzle, tracers, damage numbers — l'état confirme qu'ils existent). Ce qui manque : (1) **télégraphe ennemi** — les bots n'ont pas de windup d'attaque distinct (~0,25 s), (2) **feedback de crit/headshot** visuel/sonore distinct, (3) **lisibilité des projectiles ennemis** (couleur rouge/rose/violet nettement distincte du tir joueur), (4) **screenshake sur explosions** absent.
**Minimum viable** : chaque archétype ennemi a une silhouette + un tell d'attaque unique avant la frame de dégât ; projectiles ennemis en palette rouge/rose distincte ; swap d'arme quasi-instantané ; un « donk » audible sur headshot. **Instrumente-le** (esprit Forgia) : un capteur `forgia2_combat_readability.json` qui expose le TTK entrant moyen et le taux de morts sans dégât télégraphé — tu **mesures** la justice au lieu de la deviner.

### 2.4 🟠 AUDIO & MUSIQUE — le « secret weapon » traité en afterthought

**Pourquoi critique** : le son est le feedback instantané qui rend le tir/hit/pickup satisfaisant. C'est le pilier le plus sous-estimé qualitativement, et un différenciateur clé dans un marché saturé.
**Où en est Forgia** : fondation solide (channels SFX/Music, volume master persisté, boucles combat/break câblées, sensor audio) — mais **la musique dynamique de combat est un TODO**, les voicelines sont des popups texte, et le mix roguelite n'est pas fini. C'est **~50 % fait mais débranché**.
**Minimum viable** : SFX distincts et punchy sur **chaque** action de combat (tir/impact/kill/pickup/level-up/boon), 2-3 pistes musicales par état (menu/combat/boss), sliders Master/Music/SFX séparés (déjà en partie là). Budget zéro : bibliothèques royalty-free (Ovani Sound). Ne compose pas toi-même en priorité.

### 2.5 🟠 FTUE / ONBOARDING — les 30 premières minutes décident de la rétention

**Pourquoi critique** : en roguelite FPS dense (boons, éléments, réactions, méta), un mauvais onboarding **tue la conversion démo**. On enseigne par le **gameplay**, pas par le texte, **un** mécanisme à la fois.
**Où en est Forgia** : un **FTUE MVP existe déjà** (`ftue.rs`, save séparée, recap première mort, sensor) — bonne surprise. Mais pas de progression scriptée d'introduction des mécaniques.
**Minimum viable** : **pas de niveau tuto séparé.** Première run adoucie qui enseigne mouvement → tir → 1re arme → 1er boon → 1re réaction, avec prompts contextuels, 1 nouvelle mécanique par palier, récompense à chaque acquisition.

### 2.6 🟡 SAUVEGARDE & OPTIONS — le socle non-négociable (mais peu coûteux)

**Pourquoi critique** : leur absence fait « jeu inachevé ». Une save méta corrompue par un crash mid-write = joueur furieux.
**Où en est Forgia** : **très bon** — save méta atomique (rename), panneau Options 3 volets déjà riche (sensibilité, FOV, volume, résolution, tonemapping, MSAA, VSync). Manquent : **remapping des contrôles**, colorblind mode, toggle screenshake.
**Minimum viable** : remapping complet clavier+manette, save méta atomique + backup (déjà là), resume de run si les runs sont longues.

### 2.7 🟡 ÉQUILIBRAGE & COURBE DE DIFFICULTÉ — la mort doit être éducative

**Pourquoi critique** : la difficulté monte par **nouveaux comportements ennemis**, pas par gonflement de PV (sinon = grind). Run idéale = **20-30 min**. La mort doit se sentir éducative, pas arbitraire.
**Où en est Forgia** : boons empilables + enrage boss ✅, mais **stats ennemis hardcodées en Rust** (viole `no-hardcode`) → pas de scaling data-driven possible, difficulté fixe.
**Minimum viable** : extraire les stats ennemis en TOML (prérequis), courbe de run 20-30 min, ennemis introduits par comportement, 1 palier d'ascension post-victoire (heat/réincarnation light).

### 2.8 🟡 DISTRIBUTION STEAM — page tôt, démo avant Next Fest (et méfie-toi du folklore)

**Pourquoi critique** : c'est le pilier le plus mal chiffré par les débutants. Plancher de découvrabilité ≈ **7 000 wishlists**. **Attention** : la conversion wishlist→achat semaine 1 est tombée à **~1-4 %** en 2025 (pas les 20-25 % du folklore). La démo sortie **2-4 semaines avant** un Next Fest → ~2,5× plus de wishlists.
**Où en est Forgia** : zip portable itch.io déclaré, **aucune page Steam, aucune démo, pas de Steamworks**.
**Minimum viable** : page Steam ouverte **6-12 mois avant** le launch (capsule + trailer + GIFs), démo jouable de 15-30 min finissant sur un CTA « wishlist », participation à un Next Fest avec démo sortie **en avance**, 5-10 micro-créateurs contactés avec press kit.

> **Le message franc** : tu m'as demandé les mécaniques. Mais si tu ne bloques que **deux** piliers de cette liste, bloque **le scope (2.1) et le playtesting (2.2)**. Ce sont eux qui décident si ton jeu existe un jour.

---

## 3. Les 6 domaines — tableau de synthèse parité + geste-clé

| Domaine | Parité | Priorité | Le geste-clé (un seul mouvement qui débloque le domaine) |
|---|:---:|:---:|---|
| **Combat & défenses** | 48 % | **P0** | Introduire `DefenseLayer{Health,Shield,Armor}` (Bouclier bleu régénérant hors combat) + extraire les stats ennemis en `roguelite_enemies.toml`. C'est le pilier qui donne un sens aux éléments et au HUD. |
| **Éléments & réactions** | 38 % | **P0** | Généraliser le moteur en `HashSet<Status>` + `ReactionTable(paire→effet)`, ajouter `Shock`, re-router le matchup vers la **couche de défense** (Feu→Vie, Foudre→Bouclier, Corrosion→Armure). Débloque Miasma + Manipulation. |
| **HUD & interface** | 72 % | **P1** | Barre défensive **segmentée** joueur + ennemi (rouge/bleu/jaune, orange pré-rupture) — bloquée par la méca Shield, donc **après** P0. Quick-wins parallèles : barre de boss, munition couleur, crosshair dynamique. |
| **Level design** | 38 % | **P0** | **Dé-stubber le stage-graph** : consommer `graph.stages[depth].kind` au lieu de re-boucler l'arène, enchaîner N salles clear-to-progress. 50 % des briques existent, **dormantes**. |
| **VFX / juice** | ~65 % | **P1** | Ajouter le **télégraphe ennemi** (windup ~0,25 s) + feedback crit/headshot distinct + screenshake explosions. Le juice offensif est déjà fort ; le déficit est **défensif/lisibilité**. |
| **Production (ship)** | 35-40 % | **transversal** | Attaquer audio + FTUE + Steam **en parallèle** dès que la boucle est fun, pas à la fin. Chaque système = genome + sensor. |

**Décisions déjà prises par toi (je les intègre au plan)** : (1) **F** = compétence primaire instantanée + cooldown, **Q** = secondaire → tranche la décision ouverte D1 de l'audit (on abandonne l'Ultime 10 s comme skill principal, il devient un futur talent). (2) **Bouclier + Armure sur ennemis ET joueur** → tu valides le chantier P0-B (le plus structurant). (3) **4 éléments Feu/Poison/Électrique/Perforant** → option hybride : on **garde tes 4 éléments** et on ajoute Électrique/Choc comme conditionneur de réactions (pas de re-mapping destructeur du Poison). (4) **2 armes swappables + inscriptions échangeables au menu** → tu valides le loadout 2-armes + le système d'inscriptions.

---

## 4. Verdict Rust / Bevy — faut-il mettre à jour ?

**Reco nette, datée 2026-07-01 : NE PAS migrer Bevy maintenant. Migrer Rust oui, tout de suite.**

- **Bevy** : tu es sur **0.18.1**, la dernière stable est **0.19** (19 juin 2026). L'écosystème s'est débloqué à ~80 % depuis ton dernier board : `bevy_hanabi 0.19`, `bevy_egui 0.41`, `lightyear 0.28`, `bevy_kira_audio 0.26` ciblent tous 0.19 ✅. **MAIS un seul mur porteur bloque tout** : `bevy_rapier3d` (physique/collisions, dépendance dure de 17 crates) n'a **aucune release** pour 0.19 — la dernière (0.34.0) épingle encore `bevy ^0.18.1`. La PR #694 a des tests verts sur un fork mais reste **Draft**, bloquée en amont chez dimforge (release rapier/parry avec glamx 0.2), **sans ETA**. Cargo **refusera** tout bump 0.19 tant que rapier ne publie pas. Ce qu'on rate en attendant (contact shadows, GPU light clustering, **culling amélioré des meshes skinnés** utile pour tes ennemis animés story-636) est attractif mais **aucun n'est un débloqueur de ship**.
- **Rust** : passe à **1.96.1** (30 juin 2026) **dès maintenant** — c'est **découplé de Bevy, sans risque écosystème**. Rafraîchis le doc `build-stack` qui est stale.

**Actions concrètes** : (1) `rustup update` → 1.96.1, revalider clippy. (2) Mettre à jour le gate board `docs/migration/bevy-019-blockers.md` : cocher hanabi/egui/lightyear/kira ✅, ne laisser qu'**une** case ouverte = **rapier** (+ water hors-ship). (3) Surveiller **mensuellement** le merge de PR #694 + la publication de rapier pour 0.19 — c'est LE signal de départ. (4) **Ne bloque pas le ship du Roguelite sur cette migration.**

---

## 5. LE PLAN COMPLET par phases

> Principe d'ordonnancement : **socle combat/défenses/éléments** (donne un sens à tout) → **feel/HUD** (lisibilité) → **level-design boucle** (structure) → **armes/inscriptions** (profondeur) → **contenu** → **juice/audio/FTUE** (polish) → **ship**. Chaque story ≥ 2 crates = BMAD Standard + story-done-gate avant `DONE`. Chaque système livré avec **genome TOML + sensor**.
> Effort : **S** ≤ 2 j · **M** ~3-5 j · **L** ~1-2 sem (solo).

---

### PHASE 0 — Socle défensif & élémentaire (le cœur qui donne un sens à tout)

**Objectif** : que le jeu « se lise comme Gunfire » — barres colorées de défense + éléments qui comptent contre la bonne couche. C'est le chantier le plus structurant et le plus risqué (hot path combat). On le fait **en premier** parce que tout le reste (HUD, armes, éléments) en dépend.

| Story candidate | Effort | Crates |
|---|:---:|---|
| **P0-1** Extraire stats ennemis hardcodées → `roguelite_enemies.toml` (+ sensor existant) | S | `forgia-mode-roguelite` |
| **P0-2** `DefenseLayer{Health,Shield,Armor}` + Bouclier bleu régénérant hors combat (genome `roguelite_defense.toml` + sensor `forgia2_shield.json` + health alert) | **L** | `forgia-combat`, `forgia-mode-roguelite`, netcode |
| **P0-3** Moteur de réactions générique (`HashSet<Status>` + `ReactionTable`) + `Element::Shock`/`StatusShock` (+10 % vulnérabilité) | M | `forgia-combat`, `forgia-fps`, `forgia-mode-roguelite` |
| **P0-4** Re-router le matchup vers la couche de défense (Feu→Vie, Électrique→Bouclier, Perforant→Armure) + réactions Miasma & Manipulation | M+L | `forgia-mode-roguelite`, `forgia-ai-arena-bot` |

⚠️ **Piège documenté** : le codebase a **deux types `Health`** (combat vs damage). Les bots portent `forgia_combat::Health`. Ne route jamais un `DamageEvent` sur un bot — mute `combat::Health` + `CombatHitEvent`. Le Bouclier s'empile **par-dessus** cette Health, pas en parallèle.

**Critère de fin** : un ennemi affiche une barre rouge + bleue ; tirer du Feu fait +50 % sur la barre rouge et −25 % sur la bleue ; le bouclier se régénère 3 s après le dernier coup ; `forgia2_shield.json` écrit l'état ; les 21 tests élémentaires passent toujours.

---

### PHASE 1 — Feel, lisibilité & HUD signature

**Objectif** : rendre le combat **juste et lisible** (le levier anti-frustration n°1) + le HUD reconnaissable. C'est ici que tu corriges les deux défauts de Gunfire.

| Story candidate | Effort | Crates |
|---|:---:|---|
| **P1-1** Compétences : **F** = primaire instantanée par arme (1 charge, cooldown 14-16 s, `CooldownMul` boonable) — retirer l'empilement Ultime/sort, unifier le HUD | M | `forgia-mode-roguelite`, `forgia-combat`, `forgia-ui-lib`, `forgia-player` |
| **P1-2** Compétence **Secondaire (Q)** + charges + ressource pickup | L | `forgia-mode-roguelite`, `forgia-loot-tables`, `forgia-ui-lib` |
| **P1-3** **Télégraphe ennemi** : windup ~0,25 s par archétype (anim/son/VFX distincts) + projectiles ennemis palette rouge/rose + screenshake explosions | M | `forgia-ai-arena-bot`, `forgia-effects` |
| **P1-4** Feedback crit/headshot distinct (damage number couleur, « donk » audio, spark) + **swap d'arme quasi-instantané** | S | `forgia-effects`, `forgia-fps` |
| **P1-5** HUD : barre défensive segmentée joueur + ennemi + barre de boss dédiée + munition couleur + crosshair dynamique | M | `forgia-ui-lib`, `forgia-enemy-nameplate` |
| **P1-6** Capteur `forgia2_combat_readability.json` (TTK entrant, morts non-télégraphées, distinctivité couleur) | S | `forgia-mode-roguelite` |

**Critère de fin** : sur un playtest, un testeur externe sait **avant** de mourir d'où vient le danger ; le capteur `combat_readability` montre < 10 % de morts non-télégraphées ; le crit est audible/visible ; F et Q sont deux skills distincts au HUD.

> **JALON PLAYTEST #1** : fin de Phase 1, tu as une boucle de combat fun et lisible. **Fais tester par 3-5 personnes externes.** Question unique : « le combat est-il satisfaisant et juste ? »

---

### PHASE 2 — Boucle de niveau (traversée de salles)

**Objectif** : passer de « arène qui boucle » à « traversée de salles clear-to-progress » — la sensation Gunfire. **50 % des briques existent, dormantes.** On assemble, on ne recrée pas.

| Story candidate | Effort | Crates |
|---|:---:|---|
| **P2-1** Consommer le RunGraph : lire `graph.stages[depth].kind`, boucle `InRun{stage}` sur `boss_defeated`, enchaîner 3 salles combat + 1 boss | M | `forgia-mode-roguelite`, `forgia-stage` |
| **P2-2** Réactiver le portail de choix (`draw_portal_overlay` + `stage_kind_display`, déjà dead_code) — preview de salle typée + choix de chemin | S | `forgia-mode-roguelite` |
| **P2-3** Récompense typée par StageKind (généraliser ZoneReward : combat/élite) + `forgia2_run_progress.json` | S | `forgia-mode-roguelite` |
| **P2-4** Concept d'**ACTE** + boss par acte (`[acts]` genome, `ChapterProgress`, boss fin d'acte) — cible **3 actes × ~4 salles** | M | `forgia-stage`, `forgia-mode-roguelite` |

**Critère de fin** : une run enchaîne salle → clear → portail → choix de 2-3 salles typées → … → boss d'acte → acte suivant. Le sensor `run_progress` trace acte/salle/vague.

---

### PHASE 3 — Armes swappables & inscriptions (profondeur de build)

**Objectif** : le cœur de la puissance Gunfire — 2 armes swappables + inscriptions échangeables au menu (ta décision #4).

| Story candidate | Effort | Crates |
|---|:---:|---|
| **P3-1** Loadout **2-armes swappables** + réserves indépendantes + HUD slots (actif grand + secondaire estompé) | M | `forgia-combat`, `forgia-fps`, `forgia-ui-lib` |
| **P3-2** **Système d'inscriptions** (instance d'arme, 1-5 mods, tiers vert/bleu/orange, scaling par acte, genome `roguelite_inscriptions.toml`) + écran d'échange au menu | **L** | `forgia-combat`, `forgia-fps`, `forgia-rpg-data` |
| **P3-3** Boons : Coffre wave-clear **gratuit** + empilement **multiplicatif** exposé (+73 %) + tirage **pondéré** par rareté | M | `forgia-rpg-data`, `forgia-mode-roguelite` |

**Critère de fin** : tu ramasses une arme avec 3 inscriptions colorées, tu peux échanger une inscription au menu ; deux boons +15 % affichent +32 % (multiplicatif) ; le tirage respecte les poids de rareté.

---

### PHASE 4 — Contenu & endgame (jusqu'à la cible « lite », pas la parité 100 %)

**Objectif** : atteindre le volume minimum pour que le jeu tienne 20-30 min de run avec de la variété. **Respecte le scope lite.**

| Story candidate | Effort | Crates |
|---|:---:|---|
| **P4-1** `EnemyArchetype::Elite` + late-spawns à déclencheurs (élite en dernière vague) | M | `forgia-mode-roguelite` |
| **P4-2** Pool de salles authored par stage (`[rooms.<id>]` tiré par seed) — cible 3-4 salles/biome | L | `forgia-stage` |
| **P4-3** Difficulté par palier (1 mode + 1 ascension post-victoire, multiplicateurs data-driven) | M | `forgia-mode-roguelite`, `forgia-rpg-data` |
| **P4-4** Étendre catalogue boons ~18 → ~40 à effets distincts + trade-off/curse | M | `forgia-rpg-data` |
| **P4-5** Peddler + Statue de Bénédiction pré-boss (jalons de niveau) | M | `forgia-mode-roguelite`, `forgia-rpg-data` |

**Critère de fin** : 3 actes distincts, ~6-8 armes, ~40 boons, élites, 1 palier d'ascension. Une run complète dure 20-30 min.

> **JALON PLAYTEST #2** : run complète jouable. Question : « la run est-elle bien rythmée, la difficulté juste ? »

---

### PHASE 5 — Audio, FTUE & socle de ship (en parallèle dès la Phase 2)

**Objectif** : les piliers de production. **Ne les garde pas pour la fin** — commence audio + FTUE dès que la boucle est fun.

| Story candidate | Effort | Crates |
|---|:---:|---|
| **P5-1** Musique dynamique combat/break/boss + SFX punchy sur chaque action (royalty-free) | M | `forgia-audio`, `forgia-mode-roguelite` |
| **P5-2** FTUE : première run scriptée, 1 mécanique/palier, prompts contextuels (étend `ftue.rs`) | M | `forgia-mode-roguelite` |
| **P5-3** Remapping contrôles + colorblind mode + toggle screenshake (accessibilité) | M | `forgia-input`, `forgia-ui` |
| **P5-4** Resume de run + audit save méta (backup) | S | `forgia-mode-roguelite` |

**Critère de fin** : un nouveau joueur comprend le jeu en 30 min sans lire de texte ; le son confirme chaque action ; les contrôles sont remappables ; une run interrompue reprend.

---

### PHASE 6 — Distribution & launch

**Objectif** : sortir le jeu. **La page Steam s'ouvre pendant la Phase 2, pas ici.**

| Story candidate | Effort | Crates |
|---|:---:|---|
| **P6-1** Page Steam (capsule, trailer, GIFs) + press kit — **6-12 mois avant launch** | M | (hors code) |
| **P6-2** Démo jouable 15-30 min finissant sur CTA wishlist | M | `forgia-mode-roguelite` |
| **P6-3** Intégration Steamworks (achievements, cloud save) | L | `forgia-net`, build |
| **P6-4** Pipeline packaging Windows signé + profiling perf (cible GTX 1060 @ 60 fps) | M | build, `forgia-effects` (pooling) |

**Critère de fin** : démo sortie 2-4 semaines avant un Next Fest, page Steam live, build signé qui tourne à 60 fps stable sur hardware médian.

---

## 6. Recommandations franches

### Les 3-5 choses à faire EN PREMIER (dans cet ordre)

1. **`rustup update` → Rust 1.96.1** (15 min, zéro risque) + mettre à jour le board Bevy. Ça débloque le doute « faut-il migrer » sans rien casser.
2. **P0-1 : extraire les stats ennemis en TOML** (effort S). C'est le prérequis de TOUT le scaling de difficulté, ça respecte `no-hardcode`, et c'est un warm-up facile avant le gros morceau.
3. **P0-2 : le DefenseLayer Bouclier/Armure** (effort L, ta décision #2). C'est le pilier qui rend Forgia reconnaissable comme Gunfire. Le plus risqué (hot path combat, deux types Health) → BMAD Standard + story obligatoire + attention au piège dual-health.
4. **Poser la page Steam en tâche de fond** dès que P0 tourne. Les wishlists mettent des mois à s'accumuler ; chaque semaine sans page = wishlists perdues.
5. **Premier playtest externe** dès la fin de Phase 1.

### Ce qu'il faut DIFFÉRER (assume-le, ne culpabilise pas)

- **La migration Bevy 0.19** — bloquée par rapier, sans ETA. Surveille mensuellement, ne bloque rien dessus.
- **La parité 100 % Gunfire** : 71 armes, 148 scrolls, 4 actes, 7 boss, R1-R8. Vise **lite** (6-8/40/3/3-4/1+1). Tu ajouteras du contenu **après** le ship.
- **Le co-op / netcode** (lightyear) : solo d'abord. Le multi double la complexité.
- **Gemini (bi-armes), Vaults spatialisés, boss weakspots scriptés** : post-ship, ils dépendent d'inscriptions et de level-design matures.
- **Les voicelines TTS/comédien** : popup texte suffit pour la démo (l'ADN « armes qui parlent » passe par le texte au début).

### Les pièges de débutant à éviter

- **Scope** : chaque idée nouvelle va dans un backlog **gelé**, jamais directement dans le code. Timebox chaque tâche.
- **Perfectionnisme** : « good enough » sur un système **prouvé fun** > polish infini d'un système non testé. Tu prouves le fun par playtest, pas au feeling.
- **Contenu prématuré** : ne produis pas 50 armes/salles **avant** que les systèmes (inscriptions, room-pool) soient figés — sinon rework coûteux. Fige le moteur, puis remplis.
- **Optimiser trop tôt/trop tard** : profile avant chaque milestone (cible GTX 1060), n'optimise pas à l'aveugle. Tu as déjà `forgia2_perf.json` — sers-t'en.
- **Croire qu'un pilier de la §2 est optionnel** : audio, FTUE, save, playtesting ont chacun un minimum viable atteignable en solo. Aucun n'est optionnel.

---

### La prochaine action concrète

**Lance `rustup update` (→ Rust 1.96.1), puis ouvre `crates/forgia-mode-roguelite/src/enemies.rs` et commence P0-1 : extraire les stats hardcodées (`stats_for`, lignes ~42-116) vers un nouveau `assets/genomes/roguelite_enemies.toml` chargé au boot.** C'est petit, sans risque, ça respecte `no-hardcode`, et ça débloque tout le socle défensif de la Phase 0. Valide avec `rtk cargo check -p forgia-mode-roguelite`, vérifie que le sensor ennemis écrit toujours, puis enchaîne sur P0-2.

Tu ne construis pas Gunfire en un mois — vise **~5-7 mois** pour un « Gunfire-lite jouable et shippable » en solo à ce rythme, playtests inclus. Mais tu pars de 40 %, pas de zéro, et ta fondation data-driven + sensors te donne un avantage que peu de premiers jeux ont. Une phase à la fois, `cargo check` après chaque story, playtest tous les 15 jours. Go.
