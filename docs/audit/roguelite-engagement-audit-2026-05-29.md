# Audit — Roguelite Engagement & Addiction (2026-05-29)

> Audit croisé : best practices industry 2024-2026 (Hadès, Slay-the-Spire,
> Vampire Survivors, Cult of the Lamb, Risk of Rain 2, Roboquest) vs état
> actuel du mode Roguelite Forgia V2. Objectif : identifier les hooks
> d'engagement manquants et prioriser ce qui rendra le jeu addictif sans
> trahir la bible cartoon family-friendly.

**Auteur** : Claude Opus 4.7 + 2 sub-agents (research + audit)
**Scope** : `crates/forgia-mode-roguelite/`, `assets/genomes/roguelite/`, +
boons/loot infra `forgia-rpg-data`
**Bible-référence** : `docs/lore/` v1 (cartoon Overwatch×Hadès×Borderlands,
cible enfants+femmes)

---

## TL;DR — 3 priorités

1. **Souls → Coffre purchase + break 15s** (story-558) : débloque la boucle
   économique. Aujourd'hui chaque kill = compteur vanity.
2. **Vlambeer juice stack** (sleep frames + screen shake + POW! texte) :
   ROI feel/dev le plus rentable, gap industry confirmé (memory
   `reference_industry_3_gaps_forgia_roguelite`).
3. **Perfect Information UI** (preview boon AVANT pick, telegraph boss
   wave 3) : pédagogie implicite anti-frustration, cible enfants.

---

## 1. Compulsion loop : durée idéale

**Best practice 2024** (Game Developer, Medium Todorović 2023, TheGamer 2024) :
sweet spot **20-30 min/run**. Plus court = pas de courbe ; plus long =
lassitude. Hadès = 30-45 min, Vampire Survivors = 30 min cap, StS = 60-90.

**État Forgia** : 3 waves + boss = **~8-15 min** (estimation TTK Tank 7.2s ×
20 ennemis cumulés + transitions).

**Verdict** : **court**, mais c'est un **atout différenciant** pour cible
enfants/femmes (sessions fragmentées). Demande compulsion loop hub→run
< 30 sec pour déclencher "just one more".

**Actions Forgia** :
- ✅ Wave structure 3 OK, ne pas allonger.
- ❌ Manque : transition Defeat → Nouvelle Run en 1 clic + voiceline maître forgeron (Hadès narrative-as-reward, Game Developer 2021).
- ❌ Manque : carry-over partiel de Souls/boons après Defeat (sinon "mort = rien" = anti-pattern documenté).

---

## 2. Variable rewards éthiques

**Best practice** : Vampire Survivors **assume slot-machine** (Galante,
designer ex-igaming) — animation coffre calquée slot, 6 premiers chests
hardcodés high-yield pour fixer attente haute (Kokutech 2024,
platinumparagon.info, jboger Substack).

**Anti-pattern documenté** : pool d'options avec **trash items**. Game
Developer "Solving RNG abuse" + Medium Jeong Hyeon-Uk : si le pool
contient des choix toujours mauvais, le joueur ne *choisit* pas, il *subit*.
Solution canonique : rerolls payants, biais d'archétype, refresh pool.

**État Forgia** :
- ✅ Loot drop Tank/Sniper/Runner différencié 5/3/2 souls + Heart conditionnel HP<40% roll<35% ([run.rs:275-325](crates/forgia-mode-roguelite/src/run.rs#L275-L325)).
- ✅ Boons catalogue 5 Common + 3 Legendary défini ([roguelite_boons.toml](assets/genomes/roguelite/roguelite_boons.toml)) avec **tags synergie**.
- ❌ Pas de Coffre/Shop : Souls = compteur sans dépense. Boons jamais offerts au joueur.
- ❌ Pas de hardcode "good first run" (Vampire Survivors pattern).
- ❌ Pas de reroll : 1 offre = pas de récupération RNG.

**Actions Forgia** : voir story-558 §AC2 "Coffre du Forgeron".

---

## 3. Build diversity : seuil combinatoire

**Heuristique observée** : StS ~75 cartes/classe, Vampire Survivors ~50,
RoR2 ~140 items. **Pas de minimum académique** mais règle empirique :
combinatoire intéressante à partir de **~15 items + 3-4 synergies
par item** (Mark Brown GMT "Why Synergies are the Secret to StS's Fun"
YouTube, Risk of Rain 2 patch notes 2025 : 18 items neufs pour ouvrir
builds avec items existants).

**État Forgia** :
- ✅ 8 boons définis (5C + 3L), 6 tags synergie (chaos, fire, precision, ricochet, chain, knockback).
- ❌ Combinatoire faible : 8 boons = 28 paires possibles. Cible MVP scope
  étendu : **~15-20 boons** pour 3 waves.
- ❌ 4 armes parlantes (genome bible) mais **joueur tire AK générique** :
  zero synergie weapon × boon (gap industry #1 dans
  `reference_industry_3_gaps_forgia_roguelite`).
- ❌ Tag stacking (3× même tag → legendary unlock) défini schema
  ([roguelite_boons.toml:11-12](assets/genomes/roguelite/roguelite_boons.toml#L11-L12)) mais zero runtime tracker.

**Actions Forgia** :
- Court terme : 15-20 boons via story-558 Phase 3 (expand catalogue).
- Moyen terme : 4 armes implémentées avec stats différentes + 3-4 synergies chacune.

---

## 4. Meta-progression : éviter le grind

**Hadès Mirror of Night** = canonique : chaque slot = choix **entre 2 effets
mutuellement exclusifs**, jamais +5% linéaire. **Cult of the Lamb** (4.5M
ventes, $90M revenue — VGChartz, KitGuru, Gameshub 2024) = dual-loop run +
village management, défaite garde une partie des ressources.

**Anti-patterns explicites** (Gamerant 2024, ResetEra 1341955) :
- **Dead Cells** cité : "runs deviennent presque pointless sans grind upgrades".
- Meta-mur : skill insuffisant avant N runs = abandon.
- Unlocks passifs sans choix = pas de décision = pas de mémoire.

**État Forgia** :
- ❌ **Aucune meta-progression**. Souls reset OnEnter ([loot_tables.rs](crates/forgia-rpg-data/src/loot_tables.rs)).
- ❌ Pas de Mirror, pas de unlock pipeline.
- ⚠️ Tag stacking schema écrit mais inactif.

**Actions Forgia** :
- Court terme (story-558) : carry-over partiel Souls Defeat (10-25%).
- Moyen terme (story future) : Atelier Maître Forgeron entre runs — choix mutuel exclusif (canon Hadès).
- À éviter absolument : grind linéaire +5% damage. Anti-pattern documenté + violation bible (cible enfants doit progresser par skill, pas par farm).

---

## 5. Onboarding & Failure as Content

**Perfect Information** (Slay-the-Spire + Into the Breach) — Jeremiah
Franczyk 2019 + Mark Brown GMT : voir TOUJOURS ce que l'ennemi va faire au
tour suivant → mort attribuable à une décision, pas un dé caché.

**Vlambeer "Art of Screenshake"** (INDIGO 2013, canonique industrie) :
hit feedback fort = la 1ère mort enseigne le système sans tutoriel.

**Hadès narrative-as-tutorial** : chaque mort déclenche dialogue qui
explique les mécaniques (Game Developer 2021).

**État Forgia** :
- ❌ **Aucun tutoriel/onboarding** (0 fichier `tutorial.rs`).
- ✅ Enemy nameplates TANK/RUNNER/SNIPER ([hud.rs:428-516](crates/forgia-mode-roguelite/src/hud.rs#L428-L516)) — readability immédiate.
- ❌ Pas de boon preview avant pick (story-558 AC requis).
- ❌ Pas de telegraph attack ennemi (Tank attack=1.8s pourrait afficher windup).
- ❌ Pas de voiceline maître forgeron sur 1ère mort (bible v1 mentionne ~200 voicelines, BarkEvent stub TODO).

**Actions Forgia** : Phase 4 story-558 — Defeat overlay enrichi avec
voiceline + stat récap.

---

## 6. Feel/Juice : stack non-négociable

**Vlambeer canonique** (référence non-dépassée, multiples reprises
2016-2024) :
- Impact effects + hit animation + knockback enemy
- **Sleep frames 0.1-0.2s à l'impact** ("barely visible, entirely different feel")
- Screen shake calibré
- Gun kickback + delay + camera lerp + player knockback
- Permanence (corps restent)

**Color coding rareté** : gris/vert/bleu/violet/orange depuis Diablo —
universellement lisible, **kids-friendly**.

**État Forgia** :
- ✅ Cel-shading toon attaché Roguelite-only ([toon_config.rs](crates/forgia-mode-roguelite/src/toon_config.rs)) + hot-reload TOML.
- ⚠️ Outline Sobel désactivé (crash wgpu dual-pass, [lib.rs:61-75](crates/forgia-mode-roguelite/src/lib.rs#L61-L75)) — TODO réactiver après fix node_edges.
- ❌ **POW! popup absent** dans crate Roguelite (story-528 a shippé dans Fps).
- ❌ **Screen shake absent** dans crate Roguelite (cross-crate forgia-damage-feedback non importé).
- ❌ **Sleep frames absents**.
- ⚠️ Hit zone head proxy sensor existe ([waves.rs:168-180](crates/forgia-mode-roguelite/src/waves.rs#L168-L180)) — multiplier dmg mais zero VFX.
- ❌ Tracer colors per archetype ([enemies.rs:149-180](crates/forgia-mode-roguelite/src/enemies.rs#L149-L180)) — ennemi tire mais joueur ne tire pas de tracer cartoon.

**Actions Forgia** : story future "Juice Vlambeer Stack Roguelite" Standard
~6 fichiers. **2e priorité après story-558**.

---

## 7. FPS Roguelite spécifique

**Référents** : Roboquest, Gunfire Reborn, Void Bastards (Rogueliker 2024).
Returnal **exclu** (TPS, pas FPS). Différentiel FPS vs 2D :
- Mouvement = mécanique défensive (dash/slide/jump), pas tour-par-tour.
- Pas de temps pour lire des chiffres → screen shake + audio cues plus forts.
- TTK plus court pour maintenir intensité.

**État Forgia** :
- TTK ennemis (player→enemy) : Health Tank 120 / Sniper 45 / Runner 35 / Boss 800. Avec shotgun proche, ~1-3 hits Runner, ~5+ hits Tank. **Cohérent FPS arcade**.
- TTK player→enemy (enemy→player) : 5.9-8.8s. **Trop long pour wave 1 kid-friendly** (player a le temps de se protéger MAIS aussi de se faire surprendre).
- ✅ Cel-shading + cartoon = différenciation marketing (Cult of the Lamb proof).

**Concurrent positioning** : Forgia peut occuper **"Cult of the Lamb meets Roboquest"** (cartoon family-friendly + FPS roguelite mécaniquement solide). Gap industry confirmé.

---

## 8. Anti-patterns à BANNIR (cible enfants/femmes)

Synthèse cross-source (Game Developer, Medium, ResetEra, Gamerant 2024-2025) :

| Anti-pattern | Pourquoi le bannir | Statut Forgia |
|---|---|---|
| Grimdark / sang / body horror | Bible v1 cartoon family-friendly | ✅ Cel-shading respecté |
| Run-killers RNG (drop unique requis) | Aucune skill ne sauve = injuste | ✅ Pas de gating loot |
| Pool d'options trash | "No real choice" = frustration | ⚠️ À surveiller en story-558 (Coffre pool) |
| Méta-grind > 5 runs avant power spike | Joueur abandonne avant payoff | ✅ N/A (zero meta actuel) |
| Texte mur de stats (Borderlands DPS soup) | Illisible pour enfants | ✅ Boons TOML textes courts FR |
| Punition cosmétique défaite (rouge sang, cris) | Décourage cible enfants | ⚠️ Defeat overlay actuel "DEFEAT" rouge — à adoucir story-558 Phase 4 |
| Tutoriel popup obligatoire | Décourage | ❌ Pas de tuto, donc pas d'anti-pattern, mais manque pédagogie implicite |
| Difficulty spike non-télégraphé | Frustration | ⚠️ Boss wave 3 sans visual telegraph |

---

## 9. Recommandations Forgia priorisées

### Tier 1 — Sprint immédiat (story-558)

1. **Souls → Coffre purchase** : débloque économie.
2. **Break 3s → 15s** : window shopping + ammo prep + breathe (best practice + user-asked).
3. **3 choix forcés au Coffre** (jamais 1 random). Préserve agency.
4. **Boon preview tooltip** avant pick (Perfect Information StS).
5. **Carry-over Souls 25% sur Defeat** (mort = pas rien).

### Tier 2 — Sprint suivant (story candidate)

6. **Vlambeer juice stack Roguelite** : POW! + sleep frames + screen shake + tracer player.
7. **Telegraph boss wave 3** : emissive rouge + sound cue + UI banner enrage.
8. **Voiceline maître forgeron sur 1ère mort** (Hadès narrative-as-tutorial, bible-aligned).

### Tier 3 — Mid-term

9. **15-20 boons** (vs 8 actuels) — combinatoire viable.
10. **4 armes implémentées** avec synergies tags.
11. **Atelier Maître Forgeron** meta-progression (choix mutuel exclusif).
12. **Tutoriel implicite via dialogue** maître forgeron + apprenti.

### À NE PAS faire

- ❌ Grind linéaire +5% damage (anti-pattern bible + industry).
- ❌ Étendre run > 20 min (cible enfants = sessions courtes).
- ❌ Pool boons avec trash (toujours 3 choix viables).
- ❌ Tutorial popup wall-of-text (préférer dialogue + tooltip).
- ❌ Méta-mur (skill doit dominer après run 1).

---

## 10. Sources

Recherche externe (sub-agent deep-research 2026-05-29) :
- Game Developer — Hadès narrative + RNG abuse
- GDC Vault — Turducken Method (Davidson)
- Mark Brown GMT YouTube — Synergies in StS
- Vlambeer — Art of Screenshake (INDIGO 2013)
- Jeremiah Games — Perfect Information killer feature (2019)
- Kokutech / platinumparagon / jboger Substack — Vampire Survivors
- VGChartz / KitGuru / Gameshub — Cult of the Lamb 4.5M / $90M
- Mechanics of Magic — Loops & Arcs Hadès (2025)
- Gamerant — Roguelite progression respect
- ResetEra 1341955 — meta-progression debate
- My Gaming Tutorials — RoR2 damage scaling 2025
- Medium Todorović — Run length optimum 2023
- Medium Jeong Hyeon-Uk — Fair RNG design

Audit code interne (sub-agent Explore 2026-05-29) :
- `crates/forgia-mode-roguelite/` (lib, run, waves, hud, sensor, stations, enemies, toon_config)
- `assets/genomes/roguelite/` (boons, toon, run, weapons, loot, dialogue)
- `crates/forgia-rpg-data/src/loot_tables.rs`

Memory références :
- `reference_industry_3_gaps_forgia_roguelite.md`
- `reference_bible_forgia_roguelite_v1.md`
- `feedback_hdr_pipeline_needs_hdr_skybox_first.md`
