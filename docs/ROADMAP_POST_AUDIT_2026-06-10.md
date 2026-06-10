# Forgia — Roadmap post-audit complet (2026-06-10)

> **Source** : [audit complet 16 domaines](./audit/audit-2026-06-10-full-codebase.md) (51 agents, vérif. adversariale).
> **Objectif** : convertir le moat technique (moteur IA-natif observable) en position de marché
> en shippant le Roguelite, dans la fenêtre de différenciation estimée à 12-24 mois.
> **À intégrer dans ROADMAP_CURRENT.md** quand le WIP de l'autre terminal sera commité
> (fichier M au moment de l'écriture — règle claim multi-terminal).
>
> Étoile polaire : **« ça avance le ship ? »** — tout arbitrage de scope se résout par cette question.

---

## Vue d'ensemble — 6 jalons

| Jalon | Nom | Contenu | Durée estimée | Cible calendrier |
|---|---|---|---|---|
| **M0** | Filet | Push+CI, P0 gameplay, crash hook, stutter | 1-2 j | semaine du 2026-06-10 |
| **M1** | Moat honnête | Docs vraies, sensors véridiques, gates actifs, décision QA | 2-3 j | mi-juin 2026 |
| **M2** | Démo jouable interne | Ship-blockers gameplay + UX plancher Steam | ~3-4 sem | mi-juillet 2026 |
| **M3** | Démo publique | Packaging, page Steam/itch, playtests externes | ~2-3 sem | sept. 2026 (Next Fest oct.) |
| **M4** | Ship 1.0 | Contenu élargi, équilibrage retours, localisation | ~6-8 sem | Q4 2026 – Q1 2027 |
| **M5** | Phase 1 moteur | « Le créateur importe ses assets » — surface produit IA | post-ship | 2027 |

Chaque jalon a un **gate de sortie falsifiable** (ci-dessous). On ne passe pas au jalon suivant
tant que le gate n'est pas vert — exception : items marqués ⏸ différables.

---

## M0 — Filet (1-2 jours) — AVANT TOUTE FEATURE

> Risque existentiel n°1 de l'audit : 54 commits sur un seul disque, CI jamais verte, zéro trace de crash.

| # | Action | Effort | Fichiers/crates | Détail |
|---|---|---|---|---|
| 0.1 | `git push` + cadence quotidienne | 5 min | — | Pousser les 54 commits. Règle permanente : push à chaque fin de session. |
| 0.2 | **Billing GitHub** (ACTION ANTOINE) | 15 min | — | Settings → Billing. Seul item hors de portée IA. Alternative si refus : runner self-hosted local. |
| 0.3 | Fix P0 `block_fire` | ~1 h | forgia-fps, forgia-input | Early-return sur `InputBlockers.block_fire` dans la chaîne Combat (fire_weapon_minimal). Supprimer ou câbler `block_movement`. Test : `block_fire=true → 0 CombatHitEvent`. |
| 0.4 | Panic hook → `forgia2_crash.json` | 1-2 h | src/main.rs (racine, libre) | {message, location, backtrace, run_state, seed}. Sentry plus tard ou jamais (retirer la dep fantôme du Cargo.toml sinon). |
| 0.5 | Fix stutter métronome 5 s | 30 min | forgia-observability ⚠️ claim | `ProcessesToUpdate::Some(&[pid])` au lieu de `All` dans memory_sensor. Valider : forgia2_lag_events.json sans spine périodique 5 s. ⚠️ crate dans le diff autre terminal → vérifier claim avant édition. |
| 0.6 | CI verte minimale | 2-3 h | .github/ci.yml | Matrice `cargo test -p` sur les crates qui compilent (130/130 prouvé localement) + retirer `continue-on-error`. Le fix d'unification features (bevy_hanabi/bevy_water) = story séparée non bloquante. |

**Gate M0** : ✅ origin/main = HEAD local · ✅ 1 run CI vert · ✅ tir impossible pendant Defeat/Victory/Pause (test) · ✅ un panic produit forgia2_crash.json · ✅ lag_events sans métronome 5 s.

---

## M1 — Moat honnête (2-3 jours)

> Défaut systémique n°1 de l'audit : l'écart déclaré/câblé. Pour un moteur dont le moat est
> « l'IA lit et agit juste », chaque doc fausse / sensor menteur / gate décoratif attaque le produit.

| # | Action | Effort | Détail |
|---|---|---|---|
| 1.1 | Réécrire ARCHITECTURE.md | 2-3 h | Contre les 62 crates réelles (1 ligne/crate : rôle, wired?, sensor). + check xtask `arch-drift` (liste doc vs Cargo.toml members). |
| 1.2 | Refresh README + CONTRIBUTING | 1 h | Commande canonique `cargo run -p forgia --profile release-fast`, 62 crates, section Roguelite=ship. 100 % des commandes documentées doivent fonctionner (testées). |
| 1.3 | Supprimer/réécrire fine-grained-crates.md | 30 min | La règle prône encore 237 crates — l'inverse de la doctrine post-cleanup. Une IA la charge à chaque session. |
| 1.4 | Sensors véridiques | 2-3 h | (a) forgia2_toon : `outline_attached=false` tant que le plugin est off ; (b) exécuter story-586 §Suite : déposer le plugin village mort + ses 2 sensors fantômes, ajouter forgia2_village.json au village hex ; (c) marquer « stub » les 43 shaders post-process (ou les retirer du catalogue). |
| 1.5 | Gates actifs | 2 h | Rebaseliner asset-load (`--fix`) + sensor-audit (3 orphelins à enregistrer, scanner à corriger pour les faux négatifs SENSOR_PATH) + hook pre-push local qui exécute les 4 gates (CI morte oblige). + gate `story-ids` (9 collisions d'ID mesurées). |
| 1.6 | **Décision QA (binaire)** | 1 h décision + 0.5-2 j | (a) Brancher pour de vrai : feature qa-runtime ON, 3-4 émetteurs BugReport (panic, HP négatif, NaN transform, wave stuck), 1 SmokeBot en CI ; OU (b) sortir les 4 crates qa-* du workspace (réactivables post-ship). Pas d'entre-deux : 4 300 LOC de fiction coûtent du build et de la confiance. |
| 1.7 | ADR-0002 (cleanup crates) + ADR-0003 (pivot vision) | 1 h | Les 2 décisions les plus structurantes du projet n'ont pas d'ADR. 30 min chacun depuis les audits existants. |
| 1.8 | run_debug.ps1 → forgia.exe + suppression bin legacy forgia-game | 30 min | Élimine à la racine la classe de bug « stale binary » (déjà coûté 3+ allers-retours). |

**Gate M1** : ✅ `cargo xtask all-gates` vert (4/4) · ✅ toute commande de doc copiée-collée fonctionne · ✅ zéro sensor décrivant un système débranché · ✅ décision QA actée par ADR.

---

## M2 — Démo jouable interne (~3-4 semaines)

> Ship-readiness mesurée : ~55-60 % MVG (baseline 06-04 : 40 % ; méta+save, SFX, éléments livrés depuis).
> Deux pistes parallèles : **A = gameplay** (cœur), **B = UX/tech plancher Steam**.

### Piste A — Gameplay (blockers restants de l'audit 06-04 + nouveaux)

| # | Story | Blocker | Effort | Détail / AC |
|---|---|---|---|---|
| A1 | Variété ennemis lisible | D4 | M (~4-5 j) | 4 archétypes différenciés en stats ET comportement ET silhouette (runt/tireur/elite/boss — les genes existent dans roguelite_enemies.toml, 3/4 ont les mêmes valeurs). AC : un joueur nomme les 4 différences après 1 run. |
| A2 | Gimmicks d'armes au-delà des éléments | B1 (partiel) | M-L (~4-6 j) | Les éléments donnent l'identité 1 (matchups) ; il faut l'identité 2 (pattern de tir) : Pépin jauge confiance, Bourrasque knockback, Lenoir one-shot scope, Boucherie salve. Stories 531-534 existantes, dégraisser au minimum perceptible en 30 s. |
| A3 | Boons perceptibles | B2 (partiel) | M (~3 j) | Sur les 18 catalogués : chaque boon acheté = retour visible (VFX/HUD/stat affichée). Réutiliser le pipeline element_vfx. AC : tirage de 5 boons → 5 effets nommables par le joueur. |
| A4 | Économie recalibrée | B3 | S (~1-2 j) | ~2/18 boons achetables par run = progression cassée. Pure passe genome (roguelite_loot/boons/run.toml). Quick win fort levier. |
| A5 | Honnêteté boucle multi-stage | B8 | S→M | Soit boucle 2-3 stages réelle (les stage_id existent), soit l'UI dit la vérité (« Vague X/3 »). AC : l'UI ne promet rien que le jeu ne livre. |
| A6 | Onboarding 30 s | B7 | S (~1-2 j) | L'intro dialogue existe (e84fc45) ; ajouter 3 hints contextuels (tir/dash/coffre) à la 1re run. AC : testeur naïf comprend l'objectif sans question en <30 s. |
| A7 | Anim ennemis minimale | nouveau (audit anim) | M (~3-4 j) | Le Roguelite a 0 perso animé. Minimum : idle+walk procédural sur les bots (le pipeline existe, gate `With<Npc>`-style séparé du driver Rex) + hit-flinch scale-punch. ⏸ différable à M3 si A1-A4 débordent. |
| A8 | FixedUpdate mouvement/cooldowns/DoT | P1 audit | M (~2-3 j) | 0 FixedUpdate dans le workspace ; le feel dépend du refresh (188 dev vs 60 joueur). Ou preuve par test que tout est dt-scalé. Obligatoire avant tout playtest externe sur machine différente. |

### Piste B — UX & tech plancher Steam

| # | Story | Source audit | Effort | Détail |
|---|---|---|---|---|
| B1 | Settings plancher | P1-15 | M (~3 j) | Volume master (bevy_kira_audio l'expose — le mot « volume » n'existe nulle part aujourd'hui), résolution/fullscreen (Window mutable), affichage touches (rebind complet = M4). Persistance dans %APPDATA% (pas assets/). |
| B2 | Feature `dev-tools` forgia-game | P1-16 | S (~1 j) | Gater ForgiaDebugPlugin (F2/console), RapierDebugRenderPlugin (F10), démos worldgen, sensors haute fréquence. Build ship = `--no-default-features`. |
| B3 | KTX2 armes + gros assets | P1-9 | M (~2-3 j) | Pipeline xtask GLB→KTX2 UASTC. Armes d'abord : 340 MB → ~60 MB. Cible : <500 MB VRAM total (min-spec GPU 2-3 GB). Pattern prouvé story-588 (barks). |
| B4 | player_movement.toml + GameSet | P1-8 | S-M (~1-2 j) | speed/jump/gravity/dash en genome hot-reload (pattern gait_genome) + chaîne player dans GameSet::Movement (Lock L7). Débloque l'itération du feel — cœur d'un FPS. |
| B5 | Prespawn hanabi | P1-18 | S (~0.5 j) | 8 dummies Visibility::Hidden au Startup (pattern documenté, freeze 25 s V1). Valider au lag_events premier tir. |
| B6 | Passe FR partout | P1-17 | S (~1 j) | FR/EN mélangés dans les mêmes écrans. FR partout = moindre effort. Strings centralisées (module/TOML) pour préparer l'i18n M4. |
| B7 | Tests des chemins qui ont mordu | P1-13 | M (~2 j) | apply_damage (réduction/kill/DeathEvent headless), loot_room (fonctions pures extraites : classification items, TRS portail, compensation Y), genome-core (TOML invalide ne crash pas). Chaque bug runtime passé = un test de régression. |
| B8 | Unification features cargo test workspace | P1-12 | S-M | Résoudre bevy_hanabi/bevy_water (ou les isoler) pour que `cargo test --workspace` compile. Restaure le gate global. |

**Gate M2** : ✅ un testeur externe fait 3 runs complets sans crash/softlock sur une machine 60 Hz GPU 3 GB ·
✅ il peut régler volume+résolution · ✅ il nomme les 4 ennemis et 5 boons · ✅ VRAM <500 MB ·
✅ build `--no-default-features` sans outils debug · ✅ CI verte sur tout le workspace.

---

## M3 — Démo publique (~2-3 semaines) — cible Steam Next Fest octobre 2026

| # | Action | Effort | Détail |
|---|---|---|---|
| 3.1 | Packaging | M (~3-4 j) | Build release reproductible (toolchain pinnée — actuellement `stable` flottant), icône, splash, nom de fenêtre, installeur/zip itch, script xtask `package`. |
| 3.2 | Page Steam + itch | M (~2-3 j + délais Valve) | 100 $ Steam Direct, capsule art, 6-8 screenshots, trailer 60 s, description. Compte Steamworks à créer TÔT (review Valve = ~2-4 sem de latence). |
| 3.3 | Playtests externes structurés | continu | 5-10 testeurs, forgia2_crash.json + sensors comme télémétrie locale, formulaire 5 questions. 1 itération d'équilibrage par vague de retours. |
| 3.4 | Boss + différenciation finale | M-L | Story-536 (mid-boss + Forgeron Noir) si le gate M2 a tenu les délais ; sinon boss simple amélioré. ⏸ scope ajustable. |
| 3.5 | Polish DA signature | M | Outline Sobel réparé (fix node_edges documenté lib.rs:140-141), emissive+bloom (Tier 2 roadmap roguelite), skybox HDR. C'est l'identité « anti-gameslop » en screenshot. |
| 3.6 | Démo Next Fest | S | Build démo limité (1 run, méta réduite), opt-in Next Fest octobre. |

**Gate M3** : ✅ page Steam live + démo téléchargeable · ✅ 10 playtests externes sans P0 ·
✅ médiane session >15 min · ✅ trailer publié.

---

## M4 — Ship 1.0 (Q4 2026 – Q1 2027)

Piloté par les retours Next Fest — scope indicatif :

- **Contenu** : 2e-3e stage visités par run (les stage_id existent), 24+ boons (catalogue story-530), mid-boss, variations de vagues (567), méta-progression élargie (hub évolutif 537).
- **Feel** : rebind complet des touches, accessibilité (échelle UI, mode daltonien — la HP bar repose sur rouge/vert), gamepad si demandé.
- **i18n** : EN (le marché Steam roguelite est anglophone à ~80 %) sur les strings centralisées en M2-B6.
- **Tech** : unification des 2 Health (forgia-damage vs forgia-combat — risque permanent de no-op), RNG unique (4 implémentations aujourd'hui), découpe forgia-rpg/lib.rs (2 345 LOC), écriture sensors atomique + writer IO centralisé + roll-up des sévérités (1 fichier à lire pour l'IA).
- **Lancement** : prix impulse (5-10 €, pattern Vampire Survivors), 1.0 + plan de patches.

**Gate M4** : ✅ 1.0 sur Steam · ✅ crash rate <1 %/session (forgia2_crash.json agrégé) · ✅ médiane >30 min · ✅ 0 P0/P1 ouvert.

---

## M5 — Phase 1 moteur : « le créateur importe ses assets » (post-ship, 2027)

> L'audit AI-readiness (4/10) : Forgia est aujourd'hui un JEU construit par une IA, pas un MOTEUR
> pour jeux tiers. Le ship du Roguelite EST la preuve marketing ; M5 construit la surface produit.
> Ordre des chantiers (du plus petit chaînon manquant au plus gros) :

1. **GroundSampler branché** (10 lignes + tests) — la toolbox worldgen pose sur le vrai terrain.
2. **Worldgen multi-kit** — AssetRegistry multi-GLB + outil d'import qui génère asset_meta.ron depuis n'importe quel GLB (AABB + rôle géométrique).
3. **Import d'assets bout-en-bout** — « dépose un GLB → registry + colliders + auto-rig si humanoïde » (le pipeline voxelize→medial-axis→embed→skin existe, l'exercer sur du tiers).
4. **Scene/game format data-driven** — étendre genome+prefab+level-presets pour DÉFINIR un jeu (entités, règles, niveaux), pas seulement le tuner. C'est le cœur du « décris ton jeu ».
5. **Scripting (couche behaviour)** — bevy_mod_scripting Luau, 0 crate wired aujourd'hui.
6. **Anim générique** — N-membres (quadrupède/ailé), reste de la migration genome (idle/personnalité/foot IK appliqué), hooks attaque/mort — prérequis pour des persos importés crédibles.
7. **Productiser l'agent-ops** — le différenciateur identifié par l'étude marché : sensors+genomes+gates comme produit (« observabilité pour agents IA dans un moteur de jeu », personne ne l'a commercialisé).

---

## Règles d'arbitrage permanentes

1. **« Ça avance le ship ? »** — sinon différé, y compris les fix de dette track FORGE (RPG/terrain/villages : la moitié des P1 perf de l'audit y vivent et ne bloquent rien).
2. **Tout déclaré est vérifié ou marqué stub** — pas de doc/sensor/catalogue qui ment (leçon n°1 de l'audit).
3. **Commit + push par milestone validé** — plus jamais 54 commits locaux / 25 fichiers flottants (règle feedback_unvalidated_wip déjà actée).
4. **Pas de nouveau hardcode couche definition** — chaque valeur gameplay nouvelle naît en genome (l'écart Phase 1 se creuse à chaque exception).
5. **Multi-terminal** : claim avant édit, crates orthogonales, binaire=preuve (mtime). Inchangé.
6. **Gates de jalon falsifiables** — on ne déclare pas un jalon passé, on le prouve (le projet a déjà l'outillage story-gate pour ça).

## Risques principaux & parades

| Risque | Probabilité | Parade |
|---|---|---|
| Perte de travail (disque unique) | moyenne, impact fatal | M0.1 — push quotidien. Réglé en 5 min. |
| Dispersion two-tracks | élevée (historique) | Règle 1 + geler FORGE sauf reflux direct vers le ship. |
| Scope creep gameplay (A2/A7/3.4) | élevée | Gates falsifiables + items ⏸ explicitement différables. |
| Fenêtre marché (Roblox 4D, Unity agentique) | moyenne à 12 mois | M3 = démo publique AVANT la fin 2026 ; le récit « vrai jeu natif possédé par le créateur » ne sera pas banalisé par les géants. |
| Backlash anti-IA (52 % devs, 85 % joueurs négatifs) | certaine | Marketing « tes assets, ta direction, un vrai jeu natif » — la qualité du Roguelite comme antithèse du gameslop. Jamais vendre « généré par IA ». |
| Bevy 0.19+ breaking changes | certaine à terme | Pin 0.18.1 jusqu'au ship (déjà acté) ; upgrade = story Enterprise post-1.0. |

## KPIs de pilotage

- **Hebdo** : ship-readiness % (grille D1-D16 de l'audit 06-04, re-scorée), commits poussés, CI verte, gates xtask verts.
- **M2+** : crash rate playtests, médiane durée session, % testeurs qui relancent une 2e run.
- **M3+** : wishlists Steam, téléchargements démo, retours qualitatifs « on dirait pas de l'IA ».

---

*Roadmap rédigée le 2026-06-10 depuis l'audit complet. À fusionner dans ROADMAP_CURRENT.md
(source de vérité) dès que le claim multi-terminal est levé. Exécution : dire « go M0 ».*
