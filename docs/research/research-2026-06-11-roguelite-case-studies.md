# Research — Études de cas onboarding/méta-progression : Gunfire Reborn, Dead Cells, RoR2, Slay the Spire (+ Hades hub)

> Recherche complémentaire 2026-06-11 (suite de `research-2026-06-11-tutorial-ftue-best-practices.md`).
> ⚠️ Méthode dégradée : le harness deep-research a échoué (spend limit mensuel atteint → sub-agents bloqués) ; recherche faite en direct (8 WebSearch + 12 WebFetch, fandom.com bloqué 403 → miroirs wiki.gg/guides). Vérification = fetch direct des sources, pas de votes adversariaux.

---

## TL;DR — 4 patterns transverses aux 5 jeux

1. **Aucun n'a de tutoriel séparé obligatoire** — la run 1 enseigne en jouant ; les systèmes méta n'apparaissent qu'à la première mort.
2. **Chaque run, même ratée, est convertie en progression visible AU MOMENT de la mort** (Soul Essence à dépenser / cells déposées / points de déblocage StS).
3. **Le contenu est masqué au départ et se débloque progressivement** (héros, items, routes, cartes) — la surcharge cognitive est gérée par l'*économie* du jeu, pas par des tooltips.
4. **Le moment post-mort est le centre de gravité de l'onboarding roguelite** — c'est là que la méta-boucle s'enseigne.

---

## 1. Gunfire Reborn (le modèle direct)

- **Run 1** : Crown Prince seul héros disponible ([SlytherGames FAQ](https://www.slythergames.com/2020/11/02/gunfire-reborn-commonly-asked-questions/), [GameRant](https://gamerant.com/beginner-tips-gunfire-reborn/)). **Pas de zone d'entraînement** — les joueurs en réclament une sur les forums Steam, les devs « y travaillent » ([discussion Steam](https://steamcommunity.com/app/1217060/discussions/0/4697908845437474637/)). L'apprentissage se fait en run.
- **Méta-monnaie** : Soul Essence (orbes cyan) collecté en run, dépensé **en fin de run** dans l'arbre de talents — 6 voies : Hero, Battle, Skill, Weapons, Expedition, Survival ([Fandom via recherche](https://gunfirereborn.fandom.com/wiki/Talents), [GameRant](https://gamerant.com/gunfire-reborn-how-to-farm-soul-essence/) : « players can collect Soul Essence [...] to improve a hero's talent after a run is over »).
- **⚠️ Quirk vérifié ×2 (confiance moyenne, sources guides 2020)** : le Soul Essence **non dépensé est perdu** en fin de run — « you'll lose any Soul Essence that you don't spend » ([SlytherGames](https://www.slythergames.com/2020/11/02/gunfire-reborn-commonly-asked-questions/)) ; le talent **Dimension Pouch** permet d'en conserver (cap croissant) ([GamesFuze](https://gamesfuze.com/guides/gunfire-reborn-how-does-dimension-pouch-work/), fetch 403 — titre + SlytherGames). Lecture design : **use-it-or-lose-it force le joueur à convertir CHAQUE run en progression permanente** — aucune mort ne se termine sans un achat visible. À re-vérifier sur la version actuelle.
- **Déblocage des héros** = rythmé par la progression de l'arbre : Ao Bai (talent level 25), Qing Yan (40), Lei Luo (55) ; Tao (400 SE), Qian Sui (600 SE) ; Xing Zhe/Li (DLC) ([SlytherGames](https://www.slythergames.com/2020/11/02/gunfire-reborn-commonly-asked-questions/)).
- **Mort** : tout l'équipement de run perdu (« Every weapon, Occult Scroll, or buff [...] will not be saved after you get eliminated », [TheGamer](https://www.thegamer.com/gunfire-reborn-beginner-tips-tricks/)) ; « dying is the only way they can grow » ([LDPlayer guide](https://www.ldplayer.net/blog/gunfire-reborn-beginners-guide-and-tips.html)).

**Leçon Forgia** : la fin de run = moment de dépense quasi obligatoire → l'écran/hub post-mort doit être un *magasin de progression*, pas un écran de défaite.

## 2. Dead Cells

- **Prisoners' Quarters = point de départ de CHAQUE run** : « It is the starting point of every run. After a completed or failed run the player will start back here » ([wiki.gg, vérifié](https://deadcells.wiki.gg/wiki/Prisoners'_Quarters)). Zone facile servant de tutoriel de fait ; armes de base (melee, puis ranged + shield) ramassées juste après la porte de départ ([Neoseeker](https://www.neoseeker.com/dead-cells/walkthrough/Prisoners'_Quarters)).
- **Cells perdues à la mort** : « Cells are lost upon death » ([wiki.gg, vérifié](https://deadcells.wiki.gg/wiki/Cells)) — MAIS déposables au **Collector entre les biomes** → checkpoint de méta-progression *en cours de run* : la mort ne vole que la tranche en cours, pas la run entière.
- **Runes = gating spatial** : 4 sorties dans la première zone dont 2 verrouillées (Toxic Sewers ← Vine Rune ; Dilapidated Arboretum ← Teleportation Rune) ([wiki.gg, vérifié](https://deadcells.wiki.gg/wiki/Prisoners'_Quarters)) → le monde s'élargit au fil des runs (progressive disclosure spatiale, modèle metroidvania).
- **Pas de hub séparé** : la zone de départ cumule respawn + upgrades (flask, Collector à proximité).

**Leçon Forgia** : (a) gating spatial par déblocages permanents = curiosité relancée à chaque run (« cette porte verte, c'est quoi ? ») ; (b) dépôt de monnaie en mi-run (au portail entre zones ?) = adoucit la perte à la mort sans la supprimer.

## 3. Risk of Rain 2

- **Pas de tutoriel** : onboarding = difficulté Drizzle conseillée + **Logbook au menu principal** ([GamesRadar](https://www.gamesradar.com/risk-of-rain-2-challenges/), guides Steam). Commando seul survivant au départ ([GamesRadar](https://www.gamesradar.com/risk-of-rain-2-challenges/)).
- **Challenges = moteur de déblocage unique** : « Completing a Challenge will unlock either a Survivor, a Survivor's alternate Skill or skin, Item, or Equipment » ([wiki.gg, vérifié](https://riskofrain2.wiki.gg/wiki/Challenges)).
- **Le point clé** : les items débloqués « **will start to drop in subsequent runs** » ([wiki.gg, vérifié](https://riskofrain2.wiki.gg/wiki/Challenges)) → **le loot pool GRANDIT avec la progression**. Le joueur débutant ne voit qu'un pool réduit ; la complexité de l'écosystème d'items croît exactement au rythme de ses déblocages.

**Leçon Forgia** : c'est LE modèle pour les éléments — pool réduit en run 1, chaque déblocage (portail) élargit ce que le joueur peut rencontrer. L'anti-surcharge cognitive est *structurelle*, intégrée à l'économie du loot, pas un choix d'UI.

## 4. Slay the Spire

- **Méta-progression** : « Completed or failed runs contribute points towards unlocking new characters or new relics and cards » ([Wikipedia, vérifié](https://en.wikipedia.org/wiki/Slay_the_Spire)) → **chaque run, même catastrophique, fait monter une barre de déblocage visible à l'écran de fin**.
- **Neow (PNJ de pied de tour, à CHAQUE run)** : « Neow blesses you based on the success of your previous run » ([wiki.gg, vérifié](https://slaythespire.wiki.gg/wiki/Neow)). Après un échec précoce : 2 options seulement, dont **Neow's Lament** (« Enemies in the next three combats will have one health ») = **rubber-banding doux pour débutants** — le jeu adoucit silencieusement la difficulté après un échec. Après une bonne run : 4 options plus riches.
- **Metrics-driven** (corrobore le pilier « instrumenter » du rapport 1) : métriques client + serveur pour la sélection des cartes ([GDC 2019, Giovannetti](https://www.gdcvault.com/play/1025731/-Slay-the-Spire-Metrics), [Wikipedia](https://en.wikipedia.org/wiki/Slay_the_Spire)).

**Leçon Forgia** : (a) barre de déblocage post-mort = progrès visible même en cas d'échec total ; (b) un mécanisme Neow-like = le portail/Maître Forgeron peut offrir un coup de pouce après une mort précoce (run 1-2 ratée → boon gratuit), invisible pour les bons joueurs.

## 5. Hades — la zone d'entraînement du hub (complément rapport 1)

- **Skelly** : mannequin d'entraînement **permanent dans la cour de la House of Hades** (dernière salle avant la sortie des runs, là où on équipe armes et keepsakes), **respawn infini** (« waiting to be beaten up over and over again »), entièrement **diégétique et comique** ([Indie Game Culture, vérifié](https://indiegameculture.com/guides/skelly-hades-guide/), [Fandom via recherche](https://hades.fandom.com/wiki/Skelly)). Statues cosmétiques débloquées aux heat 8/16/32.
- Placement design : le mannequin est **sur le chemin de sortie** — on teste son arme naturellement juste avant de partir en run.

**Leçon Forgia** : un mannequin dans le Lobby (golem de forge raté, ton comique bible v1), placé **entre l'Enclume et le portail de départ** — tester l'élément fraîchement débloqué devient un réflexe de sortie, sans menu ni prompt.

---

## Application Forgia — mapping direct

| Pattern vérifié | Système Forgia cible |
|---|---|
| Loot pool qui grandit avec les déblocages (RoR2) | `ElementUnlocks` : run 1 = arme simple, pool d'éléments/boons élargi portail par portail (déjà le design — le confirmer comme intentionnel et NE PAS l'affaiblir) |
| Fin de run = dépense obligatoire (Gunfire SE) | Retour hub post-mort → l'Enclume des Âmes EST l'écran de fin : âmes affichées + dépense immédiate proposée |
| Barre de déblocage même en échec (StS) | Progress visible post-mort : « prochain élément dans N âmes » |
| Neow's Lament après échec précoce (StS) | Boon gratuit du Maître Forgeron si mort en zone 1 aux runs 1-3 (rubber-banding invisible) |
| Mannequin diégétique sur le chemin de sortie (Hades) | Golem d'entraînement dans le Lobby entre Enclume et portail |
| Gating spatial par runes (Dead Cells) | Optionnel plus tard : portes d'élément dans les zones (porte de glace ← élément feu) |
| Dépôt mi-run au Collector (Dead Cells) | Optionnel : banquer les âmes au portail entre zones |
| Zone 1 = tutoriel de fait (Dead Cells) | Le parcours zone 1 existant : facile, armes posées sur le chemin, ≤1 notion par salle |

## Limites

- Sources = wikis de référence + guides ; **très peu de matière développeur primaire** trouvée (Duoyi ne publie pas ; l'article Game Developer sur Bénard/Dead Cells = annonce vidéo Twitch sans contenu design ; seul MegaCrit a un talk GDC, centré balance).
- Le quirk Soul Essence use-it-or-lose-it date de guides 2020 (Early Access) — à re-vérifier sur la version actuelle avant de s'en inspirer fortement.
- fandom.com bloque le fetch (403) → vérifications faites sur miroirs wiki.gg + presse spécialisée.
- Traitement détaillé de l'écran de première mort de RoR2 et StS non vérifié en source primaire (connaissance commune : retour menu + stats / barre de déblocage).
