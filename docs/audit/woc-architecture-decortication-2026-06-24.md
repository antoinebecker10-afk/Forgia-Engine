# Décorticage World of ClaudeCraft → leçons pour le RPG Forgia

> Audit complet du repo open-source `levy-street/world-of-claudecraft` (MMO browser TS, ~70k LOC
> source, 373 tests, 2065 commits / 14 jours / 30+ contributeurs) confronté à l'architecture RPG de
> Forgia Rewrite (Rust/Bevy 0.18). Produit le 2026-06-24 via fan-out de 10 agents (9 sous-systèmes
> WoC + 1 cartographie Forgia). Clone local : `C:\tmp\woc` (sparse, sans assets).
>
> **But** : améliorer le RPG de Forgia. **Verdict en une phrase** : la vitesse ET la testabilité de WoC
> viennent d'**un seul choix** — un cœur de simulation déterministe, headless, sans dépendance au
> rendu. C'est la keystone que Forgia n'a pas, et tout le reste en découle.

---

## 0. Les deux architectures en un coup d'œil

| | World of ClaudeCraft | Forgia Rewrite |
|---|---|---|
| **Cœur de jeu** | `class Sim` (src/sim, 34k LOC) — **zéro** import DOM/Three/net, tourne identique en 3 hôtes | Logique soudée à Bevy `Update` + Rapier, non séparable du rendu |
| **Tick** | Fixed **20 Hz** accumulator ; ordre d'itération = ordre de tirage RNG | `Update` (frame-rate) ; `Time<Virtual>` ok mais pas de sim fixe isolée |
| **Déterminisme** | Garanti & testé (mulberry32 + tables entières anti-float-drift) | Outil présent (`forgia-rng` xoshiro256++) mais discipline non appliquée |
| **Contenu** | data-as-code TS : type-safe, validé au load, **pas** de hot-reload | genome **TOML** : hot-reload, designer-friendly, **pas** de validation référentielle |
| **Combat** | formules vanilla réelles, server-authoritative, threat/GCD/auras-tick | dual-Health (combat vs damage), `DamageEvent`, pas de threat/GCD/aura RPG |
| **Rendu** | procédural-code (canvas textures, points VFX, météo, audio) **+ GLB CC0 quand même** | Bevy + pipeline GLB + hanabi (pari AAA assumé) |
| **Réseau** | serveur autoritatif 20 Hz, interest 90 yd, identity/dynamic split, delta | **aucun** — single-player, `lightyear` déclaré mais 0 ligne câblée |
| **UI / i18n** | DOM brut + icônes canvas + i18n "sim-emits-keys" 14 locales lazy | egui ; pas d'i18n scalable |
| **Tests / RL** | **373 tests**, purity guard automatique, Gym env NDJSON, feel:smoke, perf:tour | « pas de tests » (CLAUDE.md) ; observabilité riche en revanche |
| **Gouvernance** | CLAUDE.md **par dossier** + headers token-zero, archi enforced **par test**, CI 2 paliers | CLAUDE.md monolithe + `.claude/rules` riches (concept-first, memory, story-gate) |

**Lecture** : ce ne sont pas deux implémentations du même jeu, mais deux **paris** opposés.
WoC = tout-code-déterministe-headless, contenu compilé, multijoueur d'abord, assets quasi nuls.
Forgia = ECS-rendu-couplé, contenu hot-reloadable, single-player d'abord, assets AAA importés.
Chacun gagne sur des axes différents. Le tableau §3 dit quoi voler sans trahir le pari de Forgia.

---

## 1. LEÇON #1 — Le cœur déterministe headless (la keystone)

C'est **la** chose à comprendre. Dans WoC :

```
                ┌──────────────────────────────┐
                │        class Sim             │   ← 0 dep DOM / Three / réseau
                │  tick() : SimEvent[]         │   ← sortie = union typée d'events
                │  satisfait l'interface IWorld│
                └──────────────────────────────┘
                  ↙             ↓              ↘
          Offline browser   Serveur autoritatif   Headless RL (Gym)
          (main.ts)         (server/game.ts)      (env_server.ts, stdio JSON)
          render lit IWorld setInterval 50ms      python spawn subprocess
```

Tout en découle :
- **Testabilité** : 200+ tests construisent `new Sim({seed, class})`, tickent N fois, assertent sur
  `SimEvent[]` — en **millisecondes**, sans fenêtre, sans serveur (`tests/sim.test.ts`).
- **Déterminisme prouvé** : idiome `expect(run()).toEqual(run())` — même seed + mêmes actions =
  trajectoires bit-identiques (`sim.test.ts:1502`).
- **RL gratuit** : le même `Sim` est l'environnement Gym. Un agent apprend contre **le vrai jeu**,
  pas une réimplémentation.
- **Autorité serveur triviale** : le serveur fait juste tourner `Sim`, le client est un renderer.
- **Garde mécanique** : `tests/architecture.test.ts` scanne chaque fichier `src/sim/**` et **fait
  échouer la CI** sur tout import de `render/ui/game/net/three`, tout `document.`/`window.`, tout
  `Math.random`/`Date.now`/`performance.now`. L'invariant n'est pas une convention : c'est testé.

### Détails techniques transférables
- **Tick 20 Hz, accumulator** (`types.ts:5` `TICK_RATE=20`, `DT=0.05`). `this.time`/`this.tickCount`
  sont les **seules** horloges ; le wall-clock est injecté par l'hôte (`sim.utcDay`), jamais lu dans Sim.
- **RNG** : `mulberry32` stateful séquentiel (`rng.ts:1`). L'ordre des boucles d'entités **EST** l'ordre
  des tirages — réordonner une boucle change le monde. Terrain = fonctions pures `hash2/noise2/fbm2`
  (sans état).
- **Anti-float-drift** : partout où `Math.pow(x, exposant_fractionnaire)` pourrait diverger V8 vs
  SpiderMonkey, la valeur est **précalculée en table entière** (`types.ts:1740`
  `ABOVE_LEVEL_MISS_PCT = [0,2.5,14,39,80]`).
- **Sortie = `SimEvent[]`** : aucun callback, aucun listener. Toute la surface d'effet est une union
  discriminée typée. Events avec `pid` = personnels (routés à un seul joueur) ; sans `pid` = monde.
- **Events différés avec guard** : `delayedEvents:{at, event, guard?}` drainés chaque tick ; le `guard`
  laisse annuler un event si l'état a changé (entité morte avant le proc).
- **Entité = god-struct** (~90 champs, `Map<id, Entity>`) — **pas** ECS. C'est sa faiblesse, et c'est
  exactement là que Bevy ECS de Forgia est **supérieur**.

### Application à Forgia (le gap central)
Aujourd'hui la sim Forgia est dans `Update` avec Rapier — non rejouable, non testable hors-app, non
autoritative. **Sans aller jusqu'à tout réécrire**, le mouvement à fort levier = extraire la logique de
combat/roguelite en un crate `forgia-sim` *pur* (pas de Bevy/Rapier/render), qui :
1. prend un seed + un état + des intentions, retourne des events ;
2. tourne dans `FixedUpdate` 20 Hz côté jeu ;
3. tourne **aussi** headless pour les tests et le futur RL/balance-bot.

Bevy aide ici : `Time::<Fixed>::from_hz(20.0)` + systèmes en `FixedUpdate` = l'accumulator déjà fourni.
Le purity-guard se porte en `#[test]` qui scanne les sources du crate et bannit `bevy::`, `bevy_rapier3d::`.

---

## 2. LEÇON #2 — Le headless débloque tout l'outillage qualité

WoC n'a pas « plus de discipline » que Forgia ; il a une **architecture qui rend l'outillage possible**.
Conséquences directes du cœur headless, toutes absentes de Forgia :

| Outil WoC | Ce qu'il fait | Équivalent Forgia à créer |
|---|---|---|
| `tests/*.test.ts` (373) | sim headless tické en Node, assert sur events, < 1 ms/test | `forgia-sim` testé sans app Bevy |
| `architecture.test.ts` | scanne imports interdits + non-déterminisme → fail CI | `xtask check-deps` (direction de deps entre crates) |
| `headless/env_server.ts` + `python/` | RL Gym via NDJSON stdio (obs/action sized au contenu) | `cargo run --bin forgia-env` stdio JSON |
| `quest_audit_graph.mjs` | charge le contenu, graphe de deps, checks balance (pacing, cibles partagées), **historique git par quête**, sort HTML interactif | `xtask audit-genomes` (DPS ±X%, cooldown bounds, cross-refs) |
| `feel_smoke.mjs` | boote le vrai jeu, assertions **numériques** sur le feel (dz>0.25, strafe vs turn) | headless Bevy `MinimalPlugins`, inject input, assert pos |
| `perf_tour.mjs` | tour scripté, lit `window.__game.perf.report()`, seuils env | déjà partiellement couvert par les sensors `forgia2_*.json` |
| `asset_budget.mjs` | budget MiB par catégorie d'assets, exit 1 si dépassé | **directement pertinent** (Forgia a un GLB de 185 Mo !) |
| `malware_scan.mjs` + test | scan signatures (drainers wallet, exfil) ; le test **pin** le catalogue | gate léger si Forgia va vers Web3/monétisation |

Détail RL clé (`obs.ts`) : la taille de l'observation et de l'action est **interrogée au démarrage**,
jamais hardcodée, et **scale avec le contenu** (slots d'abilities paddés à la plus grosse classe). Ajouter
une arme/un boon ne casse pas un agent entraîné. `RewardCounters` accumule des deltas, diffés par step
(`xp×0.01, kill×0.2, death×−5, questDone×5`). Pour Forgia : un bot RL qui joue le roguelite = **playtest
de balance automatisé** infini.

---

## 3. Comparaison axe par axe + ce qu'il faut voler

### 3.1 Données : data-as-code TS vs genome TOML
**Chacun a une moitié de la solution.**

| | data-as-code (WoC) | genome TOML (Forgia) |
|---|---|---|
| Type-safety à l'édition | ✅ erreur IDE | ❌ panic runtime/serde |
| Validation référentielle | ⚠️ partielle (talents validés au load par IIFE ; mais `loot.itemId` non vérifié) | ❌ aucune |
| Hot-reload | ❌ rebuild+redeploy | ✅ Shift+F12, designer-friendly |
| Accessible non-dev | ❌ il faut écrire du TS | ✅ |
| Diff review | ⚠️ syntaxe mêlée à la valeur | ✅ valeur pure |

→ **Forgia garde TOML** (le hot-reload est un avantage décisif que data-as-code **ne peut pas**
rattraper) **mais ajoute `xtask validate-genomes`** : serde-désérialise tous les TOML en CI + cross-check
que chaque ID référencé existe (élément→arme, boon→élément, loot→item). Ça ferme la **seule** faiblesse
structurelle de TOML, gratuitement. C'est le quick-win #1.

Patterns data à reprendre tels quels (de WoC `content/`) :
- `roll_group` sur les entrées de loot (drop exclusif « un parmi N », zéro logique moteur) ;
- `requires_quest` nullable (chaîne narrative complète, zéro complexité) ;
- `quest_order = [...]` explicite (ordre d'affichage découplé des clés) ;
- mechanic-bags optionnels inline sur les templates mob (`aoe_pulse`, `enrage_hp_pct`, `summon_adds`)
  plutôt qu'un sous-objet — flat, nullable, hot-reloadable ;
- groupes d'archétypes constants (`const WAR = [warrior,paladin,shaman]`) référencés partout au lieu
  de répéter la liste.

### 3.2 Combat & progression
WoC encode le **vrai** combat vanilla (server-authoritative, `mob_combat.ts`/`threat.ts`/`sim.ts`).
Forgia a un combat FPS-feel solide (hit-stop, recoil, hitmarker) mais **rien** de la profondeur RPG.
À porter pour le RPG (et partiellement pour le roguelite) :

- **1 seul roll miss+dodge** : `roll<miss → miss ; roll<miss+dodge → dodge` — moins de RNG sur le hot
  path, naturellement réplicable (`sim.ts:5226`).
- **Armor DR pure function** : `min(0.75, armor/(armor+85·L+400))` — data-driven, 0 alloc.
- **Threat table** : `HashMap<EntityId, f32>` sur le mob, switch melee/ranged à 110%/130%, taunt = set
  au top + `ForcedTarget{timer:3}`, heal-threat = `healed·0.5` splitté sur les mobs en combat. **Aucune
  notion d'aggro/tank dans Forgia aujourd'hui** — c'est la base d'un RPG de groupe.
- **GCD comme champ composant** : `gcd_remaining: f32` décrémenté en `FixedUpdate`, `off_gcd: bool` en
  genome. + règle des 5 secondes pour la régen mana.
- **Moteur d'auras tick** : `tick_timer/tick_interval` sur le composant aura ; DoT→`DamageEvent`,
  HoT→`HealEvent`, `breaks_on_damage` flag. Forgia n'a pas de buff/debuff/DoT générique.
- **Recalc de stats dirty-flag** : marqueur `StatsNeedRecalc` posé sur changement gear/buff, retiré
  après une passe — **jamais** sur le hot path combat. (Aligné avec la règle hot-path de Forgia.)
- **Progression** : `XP_TABLE` vanilla en const, scaling level-diff en **tables entières**, rested XP
  (inns), group XP `[1,1,1.166,1.3,1.43]`. Forgia a juste `XpCurve{Linear|Exponential}` — trop plat.
- **Élites en genome** : `elite:bool → hp×2.3, dmg×1.5`, pas de cas particulier par mob.
- **Grâce de portée melee en tick discret** : à 20 Hz un mob poursuivant reste « juste trop loin » ;
  bonus de portée seulement si le mob a bougé ce tick (`mob_combat.ts:64`).

### 3.3 Rendu — pari différent, quelques techniques à voler
Le claim « no 3D model files » est **partiellement faux** : WoC charge des GLB CC0 (Quaternius/KayKit),
des JPEG terrain, des HDRI. Ce qui est *réellement* procédural : géométrie terrain, **matériaux**
(textures canvas + height→normal Sobel), grass, **météo**, **VFX**, ciel low-tier, **audio**.
Forgia (pipeline GLB AAA) garde son pari, mais peut emprunter, **sans coût d'asset** :
- **Météo biome render-only** (`weather.ts`, 220 LOC) : `THREE.Points` suivant la caméra, type piloté
  par biome, cross-fade. Forgia a déjà `BiomeType` + hanabi → un effet neige/pluie par biome, jamais
  dans la sim. Polish gratuit pour le roguelite et le RPG.
- **VFX school-color + HDR-bloom** : `SCHOOL_COLORS` + multiplicateur `hdr(1.6)` pour que le bloom
  attrape la couleur. Forgia a déjà des éléments colorés (combustion, etc.) → mapper élément→couleur HDR.
- **Icônes procédurales canvas** (`icons.ts`) : 17 palettes × 20 backgrounds × N primitives, cache PNG,
  fallback déterministe `hashStr(id)`. Forgia peut peindre ses icônes d'armes/boons sur une `Image` CPU
  (crate `image`) → zéro asset d'icône à produire.
- **Render budget governor** : EMA frame-time + pression draw-call → dégrade grass/foliage/vfx/résolution
  en temps réel. À brancher sur les réglages qualité + `forgia2_diagnostics.json`.
- **Audio procédural** (`audio.ts`/`music.ts`) : SFX = `noise()`+`tone()` ; musique = scheduler 110 ms
  lookahead 0.6 s, 19 instruments synthétisés, reverb IR générée. Transposable à `bevy_kira_audio`
  (`ClockHandle` + `play_at_position`). Modèle « tout procédural sauf 1 .ogg pour le boss » = bon pour
  le pic émotionnel du roguelite.

### 3.4 Réseau — la map vers lightyear
Forgia : `lightyear=0.26.4` en dépendance, **0 ligne** dans le code, `GameSet::Network` placeholder, tout
en `Query::single()`. WoC donne le plan de référence pour quand le RPG/roguelite passera multi :
- tick serveur = `FixedUpdate` 20 Hz (lightyear s'y pin) ;
- **interest management** 90 yd entrée / 100 yd sortie (hystérésis) → `RelevanceManager` lightyear ;
- **split identity/dynamic** (nom/level/skin rarement vs pos/hp/auras chaque tick) → 2 composants à
  policies de réplication différentes ;
- **delta = `Changed<T>`** Bevy (plus idiomatique que le `maybe()` string-equality de WoC) ;
- taux dégradé par distance (full <55, /2 <80, /4 au-delà) → `send_interval` par entité ;
- **stale-input guard** : clear des touches après 0.75 s de silence (`current_tick-last>15`) ;
- **serveur autoritatif** : le client envoie des intentions (leafwing `ActionState`), jamais des
  résultats. `forgia_combat::Health` est déjà server-side dans l'esprit — garder ça.
- persistance : blob JSONB par perso, save 30 s + on-leave (retry backoff) + on-shutdown.

### 3.5 UI / i18n
- **HUD = pure view-model + thin painter** : `build_vendor_view(world) -> VendorView` (pur, testable) +
  `render_vendor(ui, view, deps)`. Exactement le pattern sensor/observable déjà dans les règles Forgia,
  appliqué à egui.
- **i18n « sim-emits-keys »** : la sim émet des **clés** (texte anglais littéral), le client relocalise à
  l'affichage → la sim reste headless/déterministe. 14 locales en chunks lazy content-hashés, seul `en`
  résident. Garde CI : un parseur AST de `sim.ts` **fail si une string émise n'est matchée par aucun
  resolver**. Modèle Rust : overlay TOML sparse + `t!(key)` proc-macro (panic en debug si clé absente),
  pseudo-locale `en_XA` (accent-push) pour voir le texte non-traduit en QA, hash de contenu pour
  détecter le stale. C'est la **bonne** façon d'avoir un RPG localisable sans polluer la sim.

### 3.6 Gouvernance AI-native — échange dans les deux sens
WoC fait mieux sur :
1. **CLAUDE.md par dossier** (21 fichiers) chargés à la demande + header HTML « ne répète pas X »
   **strippé = 0 token**. Forgia a un CLAUDE.md monolithe + `.claude/rules` chargés en entier.
2. **Invariants enforced par test** (`architecture.test.ts`), pas par convention. Forgia a `xtask
   no-scaffold`/`arch-drift` mais pas de scanner de direction de deps entre crates.
3. **CI 2 paliers** : PR-gate (anglais-only légal, rapide) vs release-gate (14 locales, complet) ;
   lint **forward-ratchet** (changed files only). Forgia lint tout le workspace à chaque fois.
4. **PRD = prompts agentiques exécutables** (`docs/prd/build-prompts.md`, self-contained).
5. **Bloc per-model** dans le CLAUDE.md racine (Sonnet=checkpoint, Opus=autonome+fan-out « 4.8
   under-spawns, fais fan-out explicite »).

Forgia fait mieux sur (à **ne pas** perdre) :
1. **Concept-First** (5 étapes, verbalisation producteur/consommateurs avant Edit) — pas d'équivalent WoC.
2. **Stability Locks** phasés (activation par milestone).
3. **Couche mémoire / session-checkpoint** (continuité cross-session) — WoC n'a rien.
4. **Anti-spéculation** (`no-speculative-fix`, `bug-triage`).
5. **Story-Done Gate** mécanique (`xtask story-gate` anti-DONE-fictif) — WoC n'a aucun garde-fou
   contre un agent qui marque « fait » sur un scaffold, malgré 30+ contributeurs.
6. **genome hot-reload** (itération balance live) — WoC doit redéployer pour changer un chiffre.
7. **Observabilité** (~60 `forgia2_*.json`) — WoC n'a aucune couche machine-readable structurée.

---

## 4. Plan d'amélioration RPG Forgia — priorisé

> Rappel cadrage (vision 2026-06-04) : **SHIP le Roguelite** d'abord ; RPG = track FORGE, autorisé s'il
> accélère le ship. D'où le split A (aide le ship) / B (investissement RPG). Risk = Low/Medium/High.

### A — Aide aussi le ship (roguelite), à fort ratio valeur/risque
| # | Action | Effort | Risk | Pourquoi |
|---|---|---|---|---|
| A1 | `xtask validate-genomes` : serde + cross-refs IDs (élément→arme, boon→élément, loot→item) en CI | ~½ j | **Low** | Ferme la seule faille de TOML ; attrape les refs mortes que le roguelite a déjà |
| A2 | `xtask audit-genomes` : checks balance (DPS ±X%, cooldown bounds, ratio dmg/cd) → rapport | ~1 j | Low | Sert direct la passe de balance roguelite (cf skill `playtest`) |
| A3 | Météo biome render-only (hanabi) + VFX élément→couleur HDR-bloom | ~1-2 j | Low | Polish gratuit, 0 asset, éléments déjà colorés |
| A4 | `asset:budget` gate (MiB par catégorie GLB) en CI/pre-push | ~½ j | Low | Forgia traîne un GLB 185 Mo ; garde la taille du repo |
| A5 | Icônes procédurales (peinture CPU `image`) pour armes/boons | ~1-2 j | Medium | Supprime la prod d'icônes ; cohérent avec le pari data-driven |
| A6 | `forgia-sim-feel` : test headless Bevy `MinimalPlugins` (inject input, assert dz/strafe) | ~2 j | Medium | Premier vrai test ; attrape les régressions de feel FPS |

### B — Investissement track RPG (FORGE), plus structurant
| # | Action | Effort | Risk | Pourquoi |
|---|---|---|---|---|
| B1 | **Extraire `forgia-sim` pur** (combat/roguelite logique, 0 Bevy/Rapier) + purity-guard test + `FixedUpdate` 20 Hz | ~1-2 sem | **High** | La keystone : débloque tests rapides, RL, autorité serveur, déterminisme |
| B2 | Moteur d'auras tick générique (DoT/HoT/buff/debuff, `breaks_on_damage`) | ~3-4 j | Medium | Brique manquante pour tout RPG ; sert aussi les éléments roguelite |
| B3 | Threat/aggro + GCD + recalc-stats dirty-flag | ~3-5 j | Medium | Profondeur RPG (tank/groupe) totalement absente |
| B4 | Quest catalogue **depuis TOML** (remplace `register_sample_quests` hardcodé) + `roll_group`/`requires_quest`/`quest_order` | ~2-3 j | Medium | Un RPG a besoin de quêtes hot-loadables, pas compilées |
| B5 | Système d'équipement/gear slots (crate `forgia-equipment` absente) | ~1 sem | Medium | Gap RPG majeur ; l'inventaire 80-slots existe mais pas d'équipement |
| B6 | i18n « sim-emits-keys » (overlay TOML + `t!` proc-macro + `en_XA`) | ~3-4 j | Medium | Localisation sans polluer la sim ; prérequis distribution |
| B7 | RL env headless (`cargo run --bin forgia-env`, NDJSON, `RewardCounters`) | ~1 sem | Medium | Playtest de balance automatisé infini (dépend de B1) |
| B8 | Gouvernance : CLAUDE.md par-crate (header token-zero) + `xtask check-deps` (direction de deps) | ~2 j | Low | Porte l'enforcement mécanique de WoC sans perdre concept-first |
| B9 | Câblage `lightyear` autoritatif (interest, identity/dynamic split, `Changed<>` delta) | ~3+ sem | **High** | Le plus gros gap MMO ; seulement si le RPG vise le multi (dépend de B1) |

### Ordre conseillé
A1 → A2 → A4 (gates rapides, immédiat) ; puis A3/A5 (polish) en parallèle d'A6 ; puis, quand on
attaque sérieusement le RPG : B1 (keystone) avant tout le reste de B, car B2/B3/B7/B9 en dépendent.
B4/B5/B6/B8 sont indépendants de B1 et peuvent avancer en parallèle.

---

## 5. Antipièges (ne pas sur-apprendre de WoC)
- WoC n'est **pas** un MMO complet : 3 zones niv. 1-20, ~90 quêtes, 5 donjons = tranche verticale très
  soignée. Ne pas copier son scope comme une cible.
- Son entité god-struct (~90 champs, `Map<id>`) est **inférieure** à l'ECS de Forgia — ne pas régresser.
- Le claim « no assets » est marketing : WoC charge bien des GLB CC0. Le pari asset AAA de Forgia est
  légitime et différent ; emprunter les *techniques* procédurales (VFX/météo/icônes/audio), pas le dogme.
- Pas de hot-reload chez WoC = douleur d'itération designer que Forgia a déjà résolue. Garder TOML.
- Ne pas casser ce qui marche (règle `no-speculative-fix`) : toutes les actions §4 sont **additives**.

---

*Sources : clone `C:\tmp\woc` @ pushed 2026-06-24 ; digests des 10 agents archivés dans le transcript de
session. Fichiers WoC les plus instructifs : `src/sim/sim.ts`, `src/sim/rng.ts`, `src/world_api.ts`,
`src/sim/content/*`, `server/game.ts`, `tests/architecture.test.ts`, `headless/env_server.ts`,
`scripts/quest_audit_graph.mjs`, `scripts/feel_smoke.mjs`, les CLAUDE.md par dossier.*
