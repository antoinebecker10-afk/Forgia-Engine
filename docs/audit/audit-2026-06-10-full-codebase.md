# Audit complet du codebase Forgia Rewrite — 2026-06-10

> **Demande** : « regarde tout le code, fais un audit/bilan/rapport, évalue le travail, recommande.
> Objectif : concurrencer les leaders du marché. »
>
> **Méthode** : workflow multi-agents (16 auditeurs parallèles + contre-vérification adversariale
> des findings P0/P1 + preuves mécaniques exécutées : clippy, cargo test, greps quantitatifs).
> 51 agents, ~5,1M tokens, 1 253 tool calls.
> **Couverture** : 9 domaines audités par agents avec vérification adversariale (core-engine,
> terrain-world, village-procgen, ui-render, observability-qa, docs-process, tests, perf, marché) ;
> 7 domaines couverts en lecture directe par l'orchestrateur après épuisement de la limite mensuelle
> de dépense API (animation-rig, combat-fps, game-modes, data-genome, code-quality, ai-readiness,
> ship-readiness), en s'appuyant sur les audits internes existants
> ([animation 2026-06-07](../audits/audit-2026-06-07-animation-system.md),
> [ship-readiness 2026-06-04](./roguelite-ship-readiness-2026-06-04.md)) et des greps vérifiés.
>
> **État du workspace au moment de l'audit** : 62 crates, 274 fichiers .rs, ~80–88k LOC,
> HEAD `cc5f5b1`, 25 fichiers non commités (multi-terminal actif), binaire release-fast stale.

---

## 1. Verdict exécutif

**Note globale : 5,4 / 10 au standard "outil pro de l'industrie" — mais avec une distribution
bimodale rare.** Ce codebase contient des éléments authentiquement au-dessus du standard industrie
(observabilité sensors, ratchets xtask anti-"DONE fictif", data-driven genome, streaming terrain,
rigueur de test sur les crates pures) côte à côte avec des trous de niveau "projet hobby"
(CI jamais verte, 54 commits non poussés, settings joueur indigents, crash reporting absent,
docs d'architecture fausses ×4).

**Le diagnostic central** : Forgia souffre d'un écart systématique entre **ce qui est déclaré**
(docs, sensors, règles, catalogues) et **ce qui est câblé**. Pour un projet dont le moat affiché
est « un codebase qu'une IA pilote de façon fiable », chaque doc périmé, chaque sensor qui ment,
chaque gate décoratif attaque directement le produit lui-même. La bonne nouvelle : la correction
de cet écart est surtout du **travail de gouvernance court** (jours, pas mois), et la fenêtre
marché (12–24 mois) reste ouverte.

| Domaine | Score | Source |
|---|---|---|
| Cœur moteur & assemblage | 6,0 | agent + vérif. |
| Terrain / monde ouvert / worldgen | 6,5 | agent + vérif. |
| Villages & procgen | 4,5 (vérif. → ~5,5 : dette déjà planifiée story-586) | agent + vérif. |
| UI / menus / rendu / VFX | 5,0 | agent (vérif. partielle) |
| Observabilité & QA | 5,5 | agent + vérif. |
| Docs & process | 4,5 | agent + vérif. |
| Tests | 5,5 | agent + mesures locales |
| Performance | 5,5 | agent + vérif. |
| Marché / positionnement | 4,0 | agent (10 recherches web) |
| Animation & auto-rig | 5,5 | audit interne 06-07 + lecture directe |
| Combat & feel FPS | 5,5 | lecture directe |
| Modes de jeu (Roguelite/RPG) | 6,0 | lecture directe |
| Data-driven & genomes & assets | 6,0 | lecture directe |
| Qualité de code mesurée | 6,5 | mesures mécaniques |
| AI-readiness (le moat) | 4,0 | lecture directe |
| Ship-readiness Roguelite | 5,0 (~55-60 % MVG) | audit 06-04 + delta commits |

---

## 2. Preuves mécaniques (exécutées pendant l'audit)

| Mesure | Résultat | Lecture |
|---|---|---|
| `cargo clippy --workspace --no-deps` | **0 erreur, 1 warning** (doc indent, meta_shop.rs:14) | La barre « 0 warning » tient quasi parfaitement sur 62 crates. Remarquable. |
| `cargo test --workspace` | **ÉCHEC de build** — bevy_hanabi ne compile pas sous profil test (`From<AlphaMode> for BlendState` manquant, unification de features) | Le gate de test workspace est cassé — cohérent avec le `continue-on-error: true` de la CI. |
| `cargo test -p` (worldgen, genome-core, village-generator, rpg-data, qa-core) | **130 tests, 0 échec, 0,04 s** | Les tests passent par crate. Le problème est le gate, pas les tests. |
| `#[test]` workspace | **1 066** sur 172 fichiers | Volume au-dessus de la moyenne indie. 12 crates à 0 test. |
| `.unwrap()` / `.expect(` | 225 occurrences / 57 fichiers (massivement dans tests et crates QA) | Chemins runtime quasi propres — vérifié 0 unwrap hors tests sur les 10 crates du cœur. |
| `unsafe` | **0** dans tout le workspace | Exemplaire. |
| TODO/FIXME/HACK | 78 / 18 fichiers (concentrés dans le port V1 de forgia-combat : melee.rs 16, weapons.rs 8, combat_juice.rs 8) | Dette localisée et identifiable. |
| God-files | forgia-rpg/lib.rs **2 345** LOC, locomotion.rs 1 762, stage/lib.rs 1 450, hud.rs 1 383 | 4 fichiers au-dessus du seuil de confort IA/humain. |
| Sensors JSON à la racine | ~99 fichiers uniques, 32 crates productrices | Couche d'observabilité réelle, sans équivalent chez Unity/Unreal/Godot. |
| Genomes TOML | ~105 fichiers (config/ + assets/genomes/), dont une suite roguelite complète (weapons, elements, enemies, boons, loot, meta_shop, run, toon, obstacles, audio, dialogues) | Le data-driven n'est pas un slogan — il est massif. Mais des poches de hardcode subsistent (player movement, village hex, ~60 const animation). |
| VRAM (forgia2_vram.json) | 1 209–1 334 MB d'images **Rgba8 non compressées** (16 textures d'armes à 21,3 MB pièce) | KTX2 compilé, 6 assets convertis seulement. Plus gros gain perf/effort du projet. |
| CI GitHub | **44 runs, 44 échecs, 0 vert depuis la création** ; billing mort ; dernier push 2026-05-28 (54 commits d'avance) | Confirmé adversarialement. Zéro filet + risque de perte de travail. |

---

## 3. Findings P0 (bloquants ship, tous vérifiés)

### P0-1 — On peut tirer à travers les écrans Defeat/Victory/Pause du Roguelite
`InputBlockers.block_fire` : 4 écrivains (forgia-ui/lib.rs:352,358,389,395), **0 lecteur**
(grep re-vérifié par l'orchestrateur). `fire_weapon_minimal` (forgia-fps/lib.rs:372-379) n'est
gaté que par `GameMode::Fps|Roguelite` — pendant `RunState::Defeat/Victory`, cliquer
« Nouvelle Run » déclenche un tir (son, VFX, munitions). Visible par tout joueur à chaque fin de run.
**Fix** : early-return sur `block_fire` dans la chaîne Combat + test de régression
(`block_fire=true → 0 CombatHitEvent`). Supprimer ou câbler `block_movement` (0 écrivain, 0 lecteur).
**Effort : S (heures).**

### P0-2 — CI morte depuis toujours + 54 commits non poussés : zéro filet, risque existentiel de perte
Vérifié adversarialement (confiance haute) : 44/44 runs CI en échec (billing GitHub),
`continue-on-error: true` sur le job test, dernier push 2026-05-28, 25 fichiers non commités,
copie unique du travail sur un seul disque (et `.git.backup-pre-filter-2026-05-21` prouve qu'un
incident d'historique est déjà arrivé). **Fix** : push immédiat + push quotidien ; réparer le
billing ou passer la CI en runner self-hosted ; matrice `cargo test -p` par crate en attendant
le fix d'unification de features. **Effort : S-M (1 journée).**

### P0-3 — Stratégique : zéro distribution pendant que la fenêtre se referme
Le marché valide la thèse (Astrocade 20M users/56M$, Roblox 4D open beta, Series AI 36M$,
Unity 6.3 agentique, Genie 3) mais Forgia a 0 jeu shippé, 0 utilisateur externe. La différenciation
technique (moteur natif Rust observable/déterministe piloté par agents) est réelle et rare — et
invérifiable tant que rien n'est shippé. Fenêtre estimée : **12-24 mois**.
**Fix** : le ship du Roguelite est l'unique KPI ; tout arbitrage de scope = « ça avance le ship ? ».

---

## 4. Findings P1 par thème (sélection vérifiée)

### Thème A — « Le déclaré ment au câblé » (attaque directe du moat IA-natif)
1. **ARCHITECTURE.md décrit 258 crates pour 62 réelles** (×4), pointe des crates et fichiers
   supprimés (forgia-sensors, forgia-camera-fps, system_set.rs), classe le Roguelite « réservé
   Phase Build/Edit » alors que c'est LA priorité. Vérifié ligne par ligne. README : commande de
   run invalide (`--release-fast` n'existe pas ; forgia-game n'a plus de [[bin]]) — **100 % des
   commandes documentées échouent**. La règle `.claude/rules/fine-grained-crates.md` prône encore
   les 237 crates, l'inverse de la doctrine post-cleanup.
2. **Sensors qui mentent** : `forgia2_toon.json` rapporte `outline_enabled=true` alors que le plugin
   outline est désactivé (crash wgpu, `let _ = outline;` ×3) ; le pipeline village MORT écrit
   2 sensors/s (`forgia_village.json` status=idle) pendant que le village VIVANT n'en a aucun ;
   43/45 shaders post-process sont des stubs passthrough « TODO: implement » présentés en catalogue.
3. **Gates décoratifs** : `asset-load` FAIL (84 call-sites vs baseline 80, cible 30),
   `sensor-audit` FAIL (3 orphelins + 29 entrées registry à trier), `story-gate` 20/33 PASS —
   rien ne bloque (CI morte, 0 hook local). CONTRIBUTING.md affirme « CI bloque cargo test » : faux.
4. **forgia2_health.json aveugle en Roguelite** : les 6 checks croisés sont RPG-gated ; le rituel
   de session IA lit en premier un fichier qui ne reflète jamais le mode du jeu à shipper.

### Thème B — Robustesse runtime
5. **Crash reporting absent** : sentry = dépendance fantôme (0 référence code, absent du
   Cargo.lock), aucun `panic::set_hook`. La story-471 (analytics/sentry) est une "DONE fictive"
   invalidée. Un playtesteur qui crash ne laisse aucune trace. **Bloquant avant tout playtest externe.**
6. **Stutter métronome 5 s auto-infligé** : `memory_sensor` fait `sysinfo refresh_processes(All)`
   toutes les 5 s sur le thread de jeu — corrélation exacte avec les spikes 30-50 ms de
   forgia2_lag_events.json (vérifié, confiance haute, fix 1 ligne : `Some(&[pid])` + déport async).
7. **Zéro FixedUpdate dans le workspace** : mouvement, hitscan, DoT, obstacles 100 %
   frame-rate-dependent (188 FPS dev vs 60 joueur). Hypothèque aussi le coop lightyear annoncé.
8. **Player controller hors GameSet (Lock L7 violé) + physique de mouvement hardcodée**
   (speed=5.0, jump=6.5, gravity=18.0 en littéraux — le feel du jeu ship n'est pas itérable
   en hot-reload alors que tout le reste l'est).
9. **Textures non compressées** : ~1 209 MB d'images Rgba8 (340 MB rien que pour 16 textures
   d'armes 2048²). KTX2/basis compilé, 6 assets convertis (barks, story-588). Inshippable
   sur GPU 2-3 GB. Pipeline xtask GLB→KTX2 = ÷4-6.
10. **Budget foliage reverté** (story-583) : populate_new_chunks repeuple tous les chunks prêts
    en 1 frame + scan O(PathNetwork) par arbre. Track FORGE seulement, mais c'est LE stutter RPG mesuré.
11. **Burst LOD2 non budgété** : ~430 tiles spawnés en 1 frame à l'entrée monde/téléport RPG
    (le doc-comment « 1-3 tiles/frame » est faux — vérifié). + Query<&Transform> **non filtré**
    pour trouver le « joueur » dans 3 systèmes LOD (le premier Transform arbitraire de l'ECS,
    ça marche par accident ; même pattern dans forgia-audio).

### Thème C — Tests & QA
12. **`cargo test --workspace` ne compile pas** (unification features bevy_hanabi/bevy_water) ;
    la CI le masque par `continue-on-error`. Les 1 066 tests ne protègent aucun merge.
13. **Distribution des tests inversée vs risque** : `apply_damage` (cœur combat, HealthGuard,
    DeathEvent) = 0 test ; `loot_room.rs` (868 LOC, source des bugs runtime récents) = 0 test ;
    genome-core (fondation data-driven) = 0 test ; 12 crates/62 à 0 test. La couche systems
    Bevy ≈ 0 % couverte. Estimation logique métier couverte : 15-25 %.
14. **4 crates QA (~4 300 LOC) = infrastructure morte** : 0 producteur BugReport, drain compilé
    no-op (feature qa-runtime jamais activée), replay clavier-seul jamais déclenché ni consommé
    (hash KeyCode irréversible !), binaire `forgia_repro` documenté mais inexistant, autopilot
    sans aucun dépendant. Vérifié adversarialement dans le détail. Décision binaire : brancher
    pour de vrai (panic hook + 3-4 émetteurs + 1 SmokeBot en CI) ou sortir du workspace.

### Thème D — UX / ship
15. **Settings sous le plancher Steam** : sensibilité + FOV seulement. Pas de volume (le mot
    « volume » n'existe nulle part dans le workspace), pas de résolution/fullscreen (1920×1080
    forcé), pas de rebind. Refund/review négative quasi garantie.
16. **Outils debug shippés sans gate** : overlay F2, console ~/F1, wireframe Rapier F10,
    démos worldgen — `forgia-game` n'a aucune section [features]. + `user_settings.toml`
    persisté dans `assets/` (read-only en install Steam).
17. **i18n inexistante et incohérente** : FR et EN mélangés dans le même écran
    (PAUSED/Resume vs AMÉLIORATIONS/ÉNERGIE). Décider FR partout = moindre effort.
18. **Pattern anti-freeze hanabi « obligatoire Phase 0 » = TODO** : hitch de compile shader
    probable au premier tir de chaque session (le freeze 25 s V1 est documenté).

---

## 5. Les 7 domaines couverts en lecture directe

### 5.1 Animation & auto-rig (5,5/10) — la crown jewel est à moitié sertie
L'audit interne du 2026-06-07 (4 sous-agents) reste exact : **couche données du squelette = qualité
AAA** (TOML genome, registry, hot-reload, validation, tests de régression, 0 alloc hot-path,
crates acycliques, backend Pinocchio morphology-agnostic) — c'est l'actif différenciant du moteur.
**Mais la couche mouvement reste largement hardcodée Rex/biped-only** : story-579 incr.1 (gait
genome) et la marche direction-aware (a29ee69) ont entamé la migration, il reste ~40-50 const
(idle/personnalité/root motion/foot IK jeté), `ArticulatedBones` figé biped, résolution d'os par
noms en dur sans fallback. **Le point le plus grave pour le ship : le Roguelite a 0 personnage
animé** (ennemis = cubes/bots non animés ; pas de hook attaque/mort/hit-react). Le pipeline
voxelize→medial-axis→embed→skin est réel et généralisable, mais aujourd'hui exercé sur Rex + PNJ.

### 5.2 Combat & feel FPS (5,5/10) — fonctionnel, dual-Health toujours là
- **Deux types Health confirmés** (grep) : `forgia_combat::Health` (lib.rs:102, ennemis, muté en
  direct par hitscan/éléments) vs `forgia_damage::Health` (lib.rs:17, joueur, via DamageEvent).
  Risque permanent de no-op silencieux (déjà mordu : chain boon). Unification = story dédiée,
  ou a minima renommage (`EnemyHealth`) + doc dans le lexique.
- Le système d'éléments par-arme (elements.rs 916 LOC + genome + VFX + progression Phase B au
  portail) est la vraie différenciation d'armes livrée — testé (echantillon vert), data-driven, sensorisé.
- Le port gunfeel V1 (combat_juice, recoil, hitmarker) porte 32 TODO/FIXME — dette identifiée.
- SFX tir/impact livrés (story-559, commit e940192) — l'audit 06-04 (D10/D11 ❌) est partiellement périmé.
- `tick_cooldown` sans gate ni filtre pollue Changed<Hitscan> chaque frame (perf, mineur).

### 5.3 Modes de jeu (6/10) — la boucle existe, le contenu est mince
État réel forgia-mode-roguelite (22 modules, ~11,4k LOC) : run 3 vagues + boss → Victory/Defeat →
relance ; 18 boons catalogués + coffre 3 cartes + reroll ; éléments par arme + déblocage au
portail ; économie or/âmes ; **méta-progression AVEC persistance disque livrée**
(meta_shop.rs:172-231, `meta_shop_save.toml`, test roundtrip) — les blockers B4/B5 de l'audit
06-04 sont résolus ou quasi ; dialogue d'intro (onboarding partiel, B7) ; parcours/obstacles
animés Fall Guys ; POI gameplay. **Restent** : variété réelle (4 archétypes d'ennemis dont 3 aux
mêmes stats ; 1 stage visité par run vs « 4 stages » affiché — honnêteté UI B8), gimmicks d'armes
au-delà des éléments (B1 partiel), boons perceptibles (B2 partiel via VFX éléments).
forgia-rpg = 4 155 LOC dont un lib.rs god-file de 2 345 lignes à découper. forgia-mode-fps-arena
n'est plus un produit mais une dépendance structurelle du Roguelite (TargetCube/ArenaBot) — à
requalifier en lib partagée assumée ou à fusionner.

### 5.4 Data-driven, genomes & assets (6/10)
~105 genomes TOML couvrant armes/ennemis/boons/éléments/loot/méta/toon/audio/dialogues/biomes/
worldgen — le pattern (parse + fallback + hot-reload mtime + sensor) est mûr et homogène.
**Mais** : forgia-genome-core = 0 test (le socle de tout le data-driven) ; Lock L1 en dérive
mesurée (84 call-sites vs baseline 80, cible 30, 3 fichiers hors allowlist) ; poches de hardcode
documentées (player movement, village hex, anim) ; textures non compressées (cf P1-9) ;
asset-cdn/asset-registry à clarifier (rôle réel vs ambition). Licences : CREDITS.md présents
dans les packs assets — bon réflexe, à compléter par un fichier de crédits agrégé au ship.

### 5.5 Qualité de code mesurée (6,5/10)
Clippy quasi parfait, 0 unsafe, unwrap hors tests ≈ 0 sur le cœur, conventions de crates
homogènes (lib.rs + Plugin + sensor + tests), nommage concept-first grep-able. En face :
4 god-files >1 300 LOC, ~76 fichiers avec `#[allow]` dont des allow crate-wide qui masquent
~2 500 LOC dormantes (forgia-terrain `#![allow(dead_code)]`), 5 fichiers forgia-effects en
allow(dead_code) file-level, code mort UI (GameMode::Fps inatteignable encore gaté par 2 widgets),
RNG fragmenté en 4 implémentations (dont un modulo bias dans forgia-rng vs rejection sampling
ailleurs). Rien de grave, tout est listé — c'est une passe de nettoyage d'une journée.

### 5.6 AI-readiness — la question stratégique (4/10)
**Aujourd'hui, ce repo est un JEU construit par une IA, pas encore un MOTEUR utilisable par une IA
pour des jeux tiers.** Le test concret : « un créateur fournit 3 GLB et décrit un platformer » —
aucun chemin n'existe sans écrire/recompiler du Rust. Ce qui manque, dans l'ordre :
1. **Scene/game format data-driven** : les genomes tunent des valeurs, ils ne DÉFINISSENT pas un
   jeu (entités, règles, niveaux). forgia-prefab (data-driven GLTF spawn) et forgia-level-presets
   sont des embryons corrects du bon pattern.
2. **Pipeline d'import d'assets arbitraires** : worldgen est mono-kit hardcodé (one_file_assets.glb),
   l'auto-rig est exercé sur Rex ; il manque l'outil « dépose un GLB → registry + colliders +
   rig si humanoïde » de bout en bout.
3. **Scripting** : 0 crate scripting wired (bevy_mod_scripting prévu, rien de branché) — la
   couche `behaviour` du modèle 4-couches n'existe pas.
4. **GroundSampler jamais branché** : la toolbox worldgen pose tout à y=0 — l'adaptateur de
   10 lignes manquant entre la toolbox et le moteur (vérifié).
En revanche, la **navigabilité IA du code est réelle** (sensors, conventions, stories, règles) —
c'est l'infrastructure du moat ; il manque la surface produit. C'est OK en Phase 0 (ship d'abord),
mais chaque hardcode ajouté aujourd'hui (village hex, movement) creuse l'écart avec la Phase 1.

### 5.7 Ship-readiness Roguelite (~55-60 % MVG, 5/10)
Baseline 06-04 : 40 % (5/16 ✅). Delta depuis (commits vérifiés) : méta-progression + persistance
(D8/D9 ❌→✅), SFX tir (D10 🟡→✅ partiel), éléments+VFX+progression (D3/D5 ❌→🟡+), intro
dialogue (D14 ❌→🟡), obstacles/parcours (variété D15 🟡+). **Restent bloquants** :
settings volume/résolution/keybinds (P1-15), crash reporting (P1-5), gate dev-tools (P1-16),
P0-1 fire-through-UI, honnêteté UI multi-stage (B8), variété ennemis lisible (D4), onboarding
complet, packaging (icône, installeur, page Steam — rien n'existe), perf min-spec (textures).
**Chemin critique réaliste : ~20-25 jours focalisés.**

---

## 6. Avis sur le travail (franc)

**Ce qui est impressionnant** — et je pèse le mot au standard industrie :
1. La **boucle d'itération IA** (sensors JSON + genomes hot-reload + règles process + stories +
   ratchets anti-fiction) est une vraie invention d'outillage. Le story-gate qui vérifie les
   claims d'une story contre git/LOC/#[test] n'a pas d'équivalent connu, même en AAA.
2. La **discipline de fond** tient sous charge : 0 unsafe, clippy ~0 warning sur 62 crates,
   1 066 tests, des tests d'invariants à 0 ulp, du streaming budgété/hystérésis/LRU de niveau
   industrie, 21 audits internes datés dont des auto-critiques sévères. Peu d'équipes — pas
   seulement solo — tiennent ça.
3. Le **rythme** : la boucle roguelite complète (vagues, boons, éléments, méta+save, audio, VFX)
   a émergé en ~3 semaines tout en construisant le moteur dessous.

**Ce qui doit changer** :
1. **L'écart déclaré/câblé est le défaut systémique n°1.** Docs fausses, sensors menteurs,
   catalogues de stubs, gates rouges ignorés, QA fantôme. Individuellement ce sont des P1-P2 ;
   ensemble, c'est le contraire exact du moat revendiqué. La règle à instaurer : *tout ce qui
   est déclaré est vérifié mécaniquement ou marqué « stub » explicitement*.
2. **Le filet de sécurité est absent au pire moment** : CI morte + 54 commits locaux + tests
   non bloquants pendant la phase la plus productive du projet. C'est le risque existentiel
   immédiat — pas le marché, pas la perf.
3. **La dispersion two-tracks est réelle** : la moitié des P1 perf/dette de cet audit sont sur
   le track FORGE (RPG/terrain/villages) qui ne bloque pas le ship. Le test de scope
   (« ça débloque le ship ? ») existe sur le papier ; l'appliquer aussi aux fix de dette.
4. **Le feel n'est pas data-driven là où ça compte le plus** : mouvement joueur hardcodé hors
   GameSet dans un FPS dont la qualité = itération du feel. C'est l'angle mort le plus ironique
   du projet.

---

## 7. Plan d'action recommandé

### Vague 0 — Filet (1-2 jours, avant tout le reste)
1. `git push` immédiat + cadence quotidienne ; réparer billing CI ou runner self-hosted.
2. Fix P0-1 block_fire (1 gate + 1 test).
3. Panic hook → `forgia2_crash.json` {message, location, backtrace, run_state, seed} (sentry ensuite ou jamais).
4. Fix stutter 5 s (`ProcessesToUpdate::Some(&[pid])`) + valider au sensor lag_events.
5. CI : matrice `cargo test -p` sur les crates qui passent + retirer `continue-on-error`.

### Vague 1 — Honnêteté du moat (2-3 jours)
6. Réécrire ARCHITECTURE.md (62 crates réelles) + README (commande valide) + supprimer/réécrire
   fine-grained-crates.md ; ajouter un check xtask `arch-drift`.
7. Sensors véridiques : outline_attached=false ; supprimer les writers du village mort
   (exécuter le §Suite de story-586) ; marquer les 43 shaders « stub ».
8. Rebaseliner les 3 gates xtask + hook pre-push local qui les exécute.
9. Décision QA : brancher qa-runtime pour de vrai (minimum : panic hook + 3 émetteurs BugReport +
   1 SmokeBot) OU sortir les 4 crates du workspace. Pas d'entre-deux.

### Vague 2 — Ship path Roguelite (~3 semaines focalisées)
10. Settings plancher Steam : volume master + résolution/fullscreen + affichage touches.
11. Feature `dev-tools` dans forgia-game (F2/console/F10/démos hors build ship) +
    user_settings dans %APPDATA%.
12. Pipeline xtask KTX2 (armes d'abord : 340 MB → ~60 MB) + prespawn hanabi.
13. player_movement.toml (speed/jump/gravity/dash) + chaîne player dans GameSet::Movement.
14. Tests des chemins qui ont déjà mordu : apply_damage, loot_room (fonctions pures extraites),
    genome-core (TOML invalide).
15. Contenu : variété ennemis lisible (4 archétypes différenciés), honnêteté UI multi-stage,
    onboarding 30 s, passe FR partout.
16. FixedUpdate pour mouvement/cooldowns/DoT (ou preuve test que tout est dt-scalé).

### Vague 3 — Post-ship (Phase 1 moteur)
17. Unification Health (ou renommage explicite) ; RNG unique ; découpe forgia-rpg/lib.rs.
18. GroundSampler branché + worldgen multi-kit + import GLB → registry : les 3 chaînons vers
    « le créateur importe ses assets ».
19. Migration anim restante vers genome (idle/personnalité/foot IK) + anim ennemis
    (critique pour tout futur jeu).
20. Écriture sensors atomique (tmp+rename) + writer IO centralisé + roll-up des sévérités
    (un seul fichier à lire pour l'IA).

---

## 8. Conclusion

Le travail accompli est **substantiel et, par endroits, réellement différenciant** : il existe ici
un embryon crédible de la seule chose que ni Unity, ni Roblox, ni les startups vibe-coding ne
construisent — un moteur natif *observable et pilotable par agents* avec un plafond de qualité 3D
réel. Mais aujourd'hui ce moat est une **affirmation**, pas une **preuve** : la preuve s'appelle
le Roguelite shippé, et le chemin vers elle est dégagé (~3-4 semaines de travail focalisé après
la vague filet). Le danger principal n'est pas la concurrence — c'est la perte de travail
(filet absent) et l'érosion interne du moat (l'écart déclaré/câblé). Les deux se corrigent vite.
Au boulot, dans cet ordre.

---

*Audit généré le 2026-06-10. Workflow : `forgia-v2-full-audit` (run wf_26d7fd29-e0b).
Résultats bruts agents : `C:\Users\Antoi\AppData\Local\Temp\claude\audit_digest.md`.*
