# Research — Best practices tutoriels / onboarding / FTUE (focus roguelite)

> Deep-research 2026-06-11 — 5 angles, 21 sources, 95 claims extraits, 25 vérifiés (3 votes adversariaux chacun), 24 confirmés / 1 réfuté.
> Contexte d'application : Roguelite Forgia (FPS type Gunfire Reborn, cartoon kid-friendly, hub Lobby, déblocage d'éléments par portail).

---

## TL;DR — 4 piliers vérifiés

1. **Enseigner par l'action, pas par le texte** — tutoriel invisible (George Fan, GDC 2012), conveyance par le level design, ≤8 mots à l'écran.
2. **La mort est le professeur** — la connaissance des mécaniques EST la méta-progression implicite du genre (Kasavin/Hades) ; retirer la douleur de la mort, jamais une run « gaspillée ».
3. **Instrumenter le funnel FTUE** — le churn se joue dans les toutes premières minutes ; mesurer time-to-first-X et drop-off par étape.
4. **Public jeune = charge cognitive réduite** — mémoire de travail plus petite, but ET moyen énoncés explicitement, prompts pris au pied de la lettre (NN/g).

---

## 1. Principes généraux (confiance haute)

### Teach by doing / tutoriel invisible — George Fan, GDC 2012 (3-0, ×4 claims)
« The best way for a player to learn is to actually perform actions in the game » ; « We strive to make it not feel like a tutorial at all ». Le design de Plants vs. Zombies a permis à des non-joueurs complets (dont la mère du designer — anecdote auto-rapportée) de finir le jeu.
Sources : [GDC Vault](https://www.gdcvault.com/play/1015541/How-I-Got-My-Mom), [Game Developer](https://www.gamedeveloper.com/design/gdc-2012-10-tutorial-tips-from-i-plants-vs-zombies-i-creator-george-fan).

### Anti text-dump : ≤8 mots à l'écran (3-0, ×3 claims)
« There should be a maximum of eight words on the screen at any given moment » (Fan — cible, pas règle absolue). Les gros blocs de texte sont « extremely intimidating » ; **14 % des adultes US/UK ont un âge de lecture < 11 ans** (gameaccessibilityguidelines.com, données NCES/BIS ; PIAAC 2023 encore plus défavorable).
Sources : Game Developer (ibid.), [gameaccessibilityguidelines.com](https://gameaccessibilityguidelines.com/use-simple-clear-language/).

### Progressive disclosure (3-0, ×2 claims)
La volonté d'apprendre croît avec l'investissement : « as I play it and become invested in the game, I have more of a willingness to learn » (Fan — dans PvZ, l'argent n'arrive qu'après 10 niveaux). Front-loader tout l'apprentissage = « overwhelming and boring ». Corroboré par Celia Hodent, GDC 2016 « The Gamer's Brain Part 2: UX of Onboarding ».

### Just-in-time contextual tips (3-0, confiance medium — source blog corroborée par Hodent GDC 2016)
Délivrer l'info au moment exact où elle est actionnable, sans bloquer la progression. « When you teach the player something when they can actually do it, it gives context » (Hodent). Le texte d'écran de chargement « will likely not be remembered well ».
Source : [UX Collective](https://uxdesign.cc/games-ux-building-the-right-onboarding-experience-a6e99cf4aaea).

### Conveyance par le level design (3-0, confiance medium)
Le wall-jump de Mega Man X est auto-appris dans une fosse sans autre sortie, maîtrise récompensée par des items placés — aucun prompt. ⚠️ Claim adjacent **RÉFUTÉ 1-2** : « MMX enseigne TOUTES ses mécaniques sans un seul mot » = overclaim.
⚠️ **Contre-point important** : Andersen et al., CHI 2012 (45 000 joueurs) — les tutoriels explicites augmentent le playtime **jusqu'à +29 % dans les jeux COMPLEXES**. « Gameplay-first » est une heuristique, pas une interdiction du texte. Un FPS roguelite avec éléments + méta-progression est un jeu « complexe » → mix optimal.

---

## 2. Étude de cas roguelite : Hades (confiance haute)

### La mort comme professeur — Kasavin (3-0)
« Even in the hardest-core roguelike game where it resets you completely to nothing [...] there is something that you carry forward, which is your knowledge of the mechanics » (GDC Podcast ép. 16). **L'apprentissage EST la méta-progression implicite** → compter sur la répétition des runs, pas sur un tutoriel exhaustif en run 1.

### Retirer la douleur de la mort (3-0, ×4 claims)
« It was an explicit goal of our early development, to take the pain out of dying and having to restart » ; « the moment of death isn't about rage-quitting [...] feel the time you spent wasn't a waste ». Continuité narrative à travers les morts (boss qui se souviennent, dialogue qui tracke l'état).
Sources : [Game Developer](https://www.gamedeveloper.com/design/how-supergiant-weaves-narrative-rewards-into-i-hades-i-cycle-of-perpetual-death), GDC Podcast.

⚠️ **Couverture partielle** : seul Hades a survécu à la vérification. Aucune claim vérifiée sur Dead Cells, Gunfire Reborn, Risk of Rain 2, Slay the Spire — recherche complémentaire ciblée nécessaire avant de figer le design du hub.

---

## 3. Métriques FTUE (confiance haute pour la pratique, medium pour les chiffres)

### Funnel d'onboarding instrumenté = pratique standard (3-0)
« Onboarding Funnel: Measure each step in your first-time user experience (FTUE) and where players drop off » — 1er cas d'usage documenté par [GameAnalytics](https://docs.gameanalytics.com/products-and-features/analytics-iq/funnels/). Métriques : time-to-first-kill, first-death, first-unlock, complétion run 1, retour session 2 / rétention J1.

### Benchmarks deltaDNA 2016 (3-0 mais confiance medium — vieux, mobile F2P, vendor)
275 jeux F2P : première session moyenne = 9 min ; au-dessus → 31 % rétention J1, en dessous → 20 % ; ~20 % des installs perdus dans les 2 premières minutes ; ~50 % ne reviennent pas après la session 1. Reco source : première session 10-20 min engageante, ne pas front-loader, finir dans un état qui incite au retour.
⚠️ Corrélation (pas causalité), données mobile 2016 → repères directionnels seulement pour un roguelite PC 2026. Direction corroborée GameAnalytics 2024-2025.
Source : [Game Developer (miroir)](https://www.gamedeveloper.com/business/how-first-session-length-impacts-game-performance) (URL deltaDNA primaire morte).

---

## 4. Public jeune / kid-friendly (confiance haute — NN/g, extrapolé du web/apps)

Trois findings NN/g (tests utilisateurs 3-12 ans + psycho du développement, 3-0 ×3) :
1. **Mémoire de travail plus petite que les adultes** (corroboré peer-reviewed : Gathercole) → minimiser l'info à retenir d'un moment à l'autre ; jamais plus d'une notion nouvelle par salle.
2. **Énoncer explicitement le BUT et le COMMENT** — sinon confusion et mécanique non apprise (cas « Counting to 100 », starfall.com).
3. **Les 7-11 ans prennent les instructions au pied de la lettre** (fillette abandonnant le trackpad parce que le prompt disait « use the mouse ») → prompts exacts, ne pas sur-spécifier le périphérique (« Tire » plutôt que « Clic gauche pour tirer »).
Source : [NN/g kids-cognition](https://www.nngroup.com/articles/kids-cognition/).

---

## 5. Application au Roguelite Forgia

| Best practice | Traduction Forgia |
|---|---|
| Tutoriel invisible | Aucun écran « tutoriel ». Zone 1 = le tutoriel (parcours existant enseigne tir/mouvement/ramassage). |
| Progressive disclosure | Le **déblocage d'éléments par portail existant EST le mécanisme** — run 1 = tir+mouvement seuls ; éléments, boons, Enclume des Âmes introduits un par un runs 2-5. |
| Just-in-time | Hints one-shot data-driven (`roguelite_hints.toml`, pipeline bulle BD existante), flags « vu » persistés dans le save méta (story-591). Prompt « feu débloqué » à la 1re arme élémentaire EN MAIN, pas dans un menu. |
| ≤8 mots, lecture 10 ans | Les bulles BD intro respectent déjà ≈10 mots — garder cette contrainte pour tous les hints. |
| Mort jamais gaspillée | Retour hub après mort = TOUJOURS donner quelque chose (âmes, dialogue Maître Forgeron réagissant à la cause de la mort, nouvel élément visible). Jamais d'écran de défaite sec. Le script de **première mort** = moment clé : 3 phrases qui enseignent la méta-boucle. |
| Conveyance | Enseigner saut/dash par un obstacle infranchissable autrement + récompense visible derrière (pièce/cœur du GLB ithappy). |
| Funnel instrumenté | `forgia2_ftue.json` : first_kill, first_death, first_element_unlock, run1_completed, hub_visited, return_run2 (conforme observability-required.md). |
| Première session | Cible directionnelle : premier kill < 2 min, première mort « riche » < 15 min. |
| Kid-friendly | But de zone explicite (« Atteins le portail »), 1 notion/salle, prompts littéraux exacts. |

---

## 6. Limites & questions ouvertes

- **Gunfire Reborn / Dead Cells / RoR2 / StS** : ✅ couvert par la recherche complémentaire `research-2026-06-11-roguelite-case-studies.md` (2026-06-11, même jour).
- **Benchmarks PC premium récents** (Steam, GameDiscoverCo) à trouver pour remplacer les chiffres mobile 2016.
- **Skippable vs non-skippable** : aucune donnée chiffrée n'a survécu — seule la nuance Andersen CHI 2012 (tutoriels explicites utiles dans les jeux complexes).
- **FPS × enfants** : visée souris, sensibilité par défaut, motion sickness — intersection non couverte par les sources vérifiées.
- Tension non tranchée par les sources : conveyance pure vs tutoriels explicites — pour Forgia (jeu complexe), un **mix** est probablement optimal.

## Sources principales (qualité)

- **Primaires** : GDC Vault (Fan 2012, diegetic interface 2021), GDC Podcast ép. 16 (Kasavin), NN/g (×2), docs GameAnalytics, deltaDNA (morte, miroir Game Developer).
- **Secondaires** : Game Developer (×3), gameaccessibilityguidelines.com, Mistplay.
- **Blogs** (corroborés) : UX Collective, Egoraptor/Sequelitis (1 claim réfuté — source qui exagère parfois), game-wisdom, psychologyofgames, Appcues.
