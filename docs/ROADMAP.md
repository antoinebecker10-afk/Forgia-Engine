# Forgia — ROADMAP (source de vérité unique)

> **Un seul fichier de pilotage.** Toutes les autres roadmaps sont archivées (§ Roadmaps archivées).
> Format **Now / Next / Later** (sans dates) — survit au churn, contrairement aux roadmaps datées qui pourrissent.
> Priorité absolue : **SHIP** (CLAUDE.md §1). Track FORGE (RPG / anim) seulement s'il accélère le ship.
> Dernière consolidation : **2026-07-03** (fusion de 5 roadmaps concurrentes).
> Redirigée sur le GDD : **2026-08-12** (§ Destination).

## 🧭 Qui dit quoi — la hiérarchie des documents

| Document | Rôle | Autorité |
| --- | --- | --- |
| [**GDD — Forgia: The Spared**](design/gdd-forgia-the-spared.md) | Le **quoi** : destination, 3 modes, épics E1→E10 | Structure, méta-progression, narratif, DA |
| [**REFONTE_GDD.md**](REFONTE_GDD.md) | Le **chemin** : phases 0→5, dépendances, jalons de falsification | Ordre d'exécution. **Document FINI** — archivé quand la Phase 5 est franchie |
| **Ce fichier** | Le **quand** : Now / Next / Later | Pilotage — qu'est-ce qu'on fait maintenant |
| [masterplan gunfire](audit/forgia-gunfire-masterplan-2026-07-01.md) | Le **comment**, phase par phase | Valide pour les **phases 0-3 et 5** (cœur combat + discipline de ship). ⚠️ Sa **Phase 4 « contenu & endgame »** est périmée par le GDD — l'endgame est désormais Expéditions + chasses + trempe |
| `docs/stories/story-NNN-*.md` | Le **détail** | Exécution |
| [gdd-roguelite-v1](design/gdd-roguelite-v1.md) | Feel & combat | Reste la référence du moment-à-moment (le GDD le dit explicitement) |

## 🚦 Règles de pilotage (garde-fous)

- **Limite WIP** : max **3 stories `IN_PROGRESS`** à la fois. *Stop starting, start finishing.*
  ⚠️ **État au 2026-08-12 : 208 stories, 143 ouvertes, dont 47 `IN_PROGRESS`** — limite dépassée d'un
  facteur 15. **Passe de fermeture en cours** (Antoine, 2026-08-12) pour préparer le terrain de la
  redirection GDD. Rediriger 143 stories coûte bien plus cher que 60 : fermer d'abord, rediriger ensuite.
- **Statuts normalisés, un seul vocabulaire** : `DRAFT → READY → IN_PROGRESS → REVIEW → DONE`. (Bannir `EN COURS` / `IN PROGRESS` / `CODE-COMPLETE` → mapper sur `IN_PROGRESS` / `REVIEW`.)
- **DONE mécanique** : `cargo run -p xtask -- story-gate` (jamais de DONE auto-déclaré).
- **Critère de tri de la passe en cours** : chaque story ouverte sert **(a)** un épic du GDD → garder et
  retagger · **(b)** le cœur combat / l'Abîme → garder, toujours valide · **(c)** rien dans le GDD →
  `CANCELLED` avec motif écrit.

---

## 🎯 Destination — ce vers quoi Now/Next/Later pointe

Depuis le **2026-08-09**, le « quoi » vit dans le GDD. Il **ne remplace pas** le ship en cours, il le
**positionne** : l'arène roguelite actuelle devient **l'Abîme**, un des trois modes, dont l'unique
fonction est de tremper les armes.

**Conséquence pratique, et elle est rassurante** : NOW et NEXT ci-dessous sont du **cœur combat**,
donc de l'Abîme, donc **valides tels quels**. La redirection change la destination, pas le chantier
courant. C'est LATER qui s'aligne désormais sur les épics.

| Épic | Contenu | Horizon |
| --- | --- | --- |
| **E1 Compagnon** | navmesh (vleue_navigator), suivre, verbes duo, combat d'appoint | 🔜 **premier** — cf. NEXT |
| **E2 Mode Expédition** | terrain → mode jouable, objectifs, paliers, gardiens, extraction | après E1 |
| **E3 Puissance & gates** | formule, gates univers, scaling en bande, recalibrage `power_gain_per_round` | v1 |
| **E4 Loot & chasses** | tables qualité, Épargnés + traces, reliques, journal | v1 |
| **E5 L'Oubli** | stades visuels, foyers, purge, propagation | v1 |
| **E6 Forge & trempe** | niveaux d'arme, caps par univers, XP par profondeur, matériaux | v1 |
| **E7 Hall des Épargnés** | piédestaux, refuge des créatures, trophées | v1 |
| **E8 Narratif** | Livre recâblé sur l'arc l'Oubli, barks contextuels | v1 |
| **E9 Coop humain** | netcode duo (listen-server), rattrapage duo | post-validation |
| **E10 Arène 5v5** | MOBA en vue FPS, 10 slots humains/bots, déblocage niveau joueur | post-v1 — endgame |

**Premier pas de validation** (GDD) : une Expédition solo + compagnon sur le terrain existant — teste
d'un coup H1 (branchement terrain), H2 (navmesh) et le fun du concept.

> 🧭 **L'ordre d'exécution ne vit pas ici.** Le découpage en phases, les dépendances et les jalons
> qui autorisent à passer à la suite sont dans [`REFONTE_GDD.md`](REFONTE_GDD.md).
> **Règle** : le NOW ci-dessous est toujours un **sous-ensemble de la phase courante** de ce plan, et
> ne le recopie jamais — sinon les deux fichiers divergent, comme les 5 roadmaps de juillet.

---

## 🔵 NOW — en vol (on finit ça avant d'ouvrir autre chose)

| Chantier | Statut | Détail |
| --- | --- | --- |
| **Passe de fermeture des stories** | `IN_PROGRESS` (Antoine) | 143 ouvertes → résorber avant la redirection. Critère de tri en § Règles de pilotage. **Ne pas éditer `docs/stories/` ni `_index.md` en parallèle.** |
| **story-697 — Les éléments s'appliquent mais ne réagissent JAMAIS** | `DRAFT` | 🚨 **Le plus grave.** Les réactions élémentaires sont l'**USP n°1** du GDD (« je pose, tu détones ») *et* le futur axe de composition d'équipe de E10. Elles ne partent pas aujourd'hui → le pilier sur lequel le duo est bâti n'est pas prouvé. Bloque story-611 et « par extension tout le pilier ». Origine : run de validation du 12/08, capteurs `elements` + `element_vfx`. |
| **story-696 / story-698 — hitstop muet, kills sans burst ni son** | `DRAFT` | Même run du 12/08. **696** : le hitstop ne se déclenche jamais et son capteur s'est tu → bloque 648. **698** : 51 kills → 0 burst visuel, 2 sons de mort (deux compteurs à zéro, cause commune probable : l'événement de mort) → bloque 652. Ces deux-là **sont** le dernier morceau du « kill satisfaisant » ci-dessous. |
| **Kill satisfaisant (mort en 4 temps)** | `READY` | Anticipation (hitstop + flash + scale punch) → burst → débris physiques → permanence (corps + décal élément). Ingrédients livrés (648/650/655) ; reste l'assemblage — **mais 696/698 doivent tomber d'abord**, sinon on assemble des briques muettes. **Dernier gros morceau du game-feel** : après, on arrête le polish visuel. |
| **story-596 — Ultime par arme** | `IN_PROGRESS` | Phase A DONE (validée runtime 2026-06-12), **Phase B en cours**. ⚠️ **Ne pas éditer `forgia-mode-roguelite` sans coordination** (arbre chaud, autre terminal). |
| **R2 — FSM `RoomPhase` + Inc.3 salles typées** | `IN_PROGRESS` ⚠️ | Multi-salles (Inc.1) + portail de choix (Inc.2) livrés + validés runtime. Reste : refactor FSM (`Fighting/Break/PortalChoice` — tue la classe de bugs à flags) PUIS Inc.3 (Élite = compo ×gene, Trésor/Repos/Boutique sans combat). Design complet dans story-646. **⚠️ Divergence à trancher pendant la passe** : l'en-tête de story-646 dit « Inc.1 en cours » alors que cette roadmap la donne livrée+validée — l'un des deux est périmé. |
| **Valider le Bourg de l'Enclume (story-660)** | `REVIEW` | Salle 2 authored livrée (village diurne medieval_hexagon ×5-10, 13 pièces, AABB mesurés, règles level-art : weenie / 70-30 / température). À valider visuellement in-game — ajustements = data-only (`arena_layouts.toml`, re-rentrer dans la salle suffit). |
| **Ship P0 — binaire dist + vérif victoire** | `TODO` | Lancer `scripts/build-dist.ps1` **depuis un HEAD propre**, décompresser le zip ailleurs, vérifier le lancement standalone (assets/cwd/capteurs). Vérifie **d'un coup** la victoire au runtime (câblée depuis story-571, **jamais testée**). Cf. `ROADMAP_ROGUELITE.md` § ship-gap. |

---

## 🟢 NEXT — chemin de ship immédiat (dès que NOW est vidé)

| Chantier | Détail |
| --- | --- |
| **Navmesh — `vleue_navigator` 0.15** | 🔑 **Double emploi, et c'est ce qui le rend prioritaire.** (1) Remède **structurel** aux mobs confinés / coincés (bug playtest (c) ci-dessous, aggravé par `stage_layout` à 0 abri) ; (2) **fondation de E1**, premier épic du GDD, et prérequis des sbires de E10. Compatible bevy ^0.18 **sans migration** (veille 2026-08-06). C'est l'hypothèse **H2** du GDD : le prototype est la preuve. Navmesh généré depuis `ArenaGeometry` / le terrain d'expédition. |
| **⏸️ Version web (story-695) — EN PAUSE, décision Antoine 2026-08-12** | Canal testeurs wasm/WebGPU. **Repris quand Antoine aura écrit son fichier d'exigences web** ; alors validation 3 étages (local → staging HTTPS réseau bridé → Pages) et migration une fois « propre ». **État au gel** : jeu entier validé localhost (menu+avatar+arène+équipement, capteurs verts) ; site public `antoinebecker10-afk.github.io` porte encore le build qui **panique à 1-3 min** (RenderDiagnosticsPlugin/WebGPU) — **fix commité `forgia-observability` mais PAS publié**, bundle rebuilt prêt dans `web-demo/`. Reste inc.5 (manifeste assets), inc.6 (parité : ~30 lecteurs fs → `def_io`, KTX2 jolcham, UserSettings), inc.7 (pipeline 3 étages, entamé). Détail : story-695 + audit web-port §8 + mémoire `reference_wasm_web_port_forgia`. |
| **Voix / SFX des armes** | 90 barks écrits, **0 audio**. L'identité unique de Forgia (« les armes qui parlent ») — et le GDD §5 en fait du lore : âmes de maîtres-forgerons, cœur de braise qui rougeoie quand l'arme parle. + SFX punchy sur chaque action de combat (tir/impact/kill/pickup/level-up). Royalty-free (Ovani). Masterplan P5-1. |
| **Parcours PLATFORMER entre les salles** | ✅ RunGraph consommé + portails de choix = **FAITS** (story-646 Inc.1/2). Reste l'identité « niveaux à parcours » (rapport §R2.3) : les segments platformer (kit underworld, déjà 40 % construit) deviennent les **couloirs entre salles** — traversal risk/reward au lieu du swap sur place. |
| **Icônes de statut sur nameplates (HUD Inc.3, story-644)** | burn 🔥 / poison ☠ / shock ⚡ / miasma sur le nameplate ennemi — rend les réactions élémentaires **lisibles**. Complément direct de story-697 : corriger les réactions sans les rendre lisibles ne se verra pas. Zéro collision arbre chaud (forgia-enemy-nameplate). |
| **Bugs playtest 2026-08-09 (cycles ouvrir/fermer l'arène)** | **(a) Salle 1 ≠ chapitre** : sur 7 lancements, 4× `forge_sanctum` / 2× `hauts_paturages` — le stage est tiré au `run_seed` sans lire `SelectedChapter`, alors que le menu promet l'arène du chapitre. **(b) Munitions non refill au start** : ✅ corrigé (commit `0f85868`) — à re-vérifier en jeu. **(c) Mobs « me sautent dessus » + confinés** : anneaux d'apparition 12/25 m centrés joueur (grunt 9 m/s = contact en ~2 s) + bots sans navmesh limités à `alert_radius` 25 m dans une arène de 80-90 m + 0 abri → **remède = le navmesh ci-dessus**. |
| **Fond noir du lobby** | Signalé par le user 2026-07-02 (screenshot hub « TON FORGERON »), jamais diagnostiqué — l'arène de fond ne rend pas au Lobby. À trier (peut-être lié au chantier hub). |
| **Réaction Manipulation (P0-4 Inc.4)** | Déférée : conflit de paire Feu+Élec (= Surcharge, décision de contenu à trancher) + charme = re-targeting IA dans `forgia-ai-arena-bot` (coordonner). Cf. `reference_elemental_reaction_engine_and_shock`. |
| **Télégraphe ennemi + lisibilité** | Windup ~0,25 s par archétype (anim/son/VFX) + projectiles ennemis en palette rouge distincte + screenshake explosions. **Levier anti-frustration n°1.** Masterplan P1-3. |
| **FTUE — première run scriptée** | 1 mécanique par palier, prompts contextuels (étend `ftue.rs`, déjà MVP). Pas de niveau tuto séparé. Masterplan P5-2. |
| **Playtest externe #1** | 3-5 testeurs, **1 seule question**. Dès que la boucle est fun. Apprend plus que 10 stories de polish. Jalon masterplan. |
| **Page Steam en tâche de fond** | Capsule + trailer + GIFs, **6-12 mois avant launch**. Les wishlists s'accumulent lentement — chaque semaine sans page = wishlists perdues. |

---

## 🟠 LATER — profondeur, contenu, ship infra (v0.2+)

### Aligné sur les épics du GDD

- **E2 → Mode Expédition** : brancher `forgia-terrain` en mode jouable (hypothèse **H1** — le capteur
  `rpg_player` note toujours « BiomeMap absent » côté roguelite : le branchement **est** le chantier).
  Portage V1→V2 selon la doctrine **porter = corriger** (GDD §11) : DiscoveryMap, minimap à révélation,
  spawn d'ennemis par biome, objectifs dynamiques, TriggerZones, portails+intérieurs, donjon BSP, POI.
- **E3 → Puissance** : la refonte absorbe le recalibrage de `power_gain_per_round` — son capteur est
  **en alerte** (« la puissance réelle dépasse le modèle du mur »). Ne pas le patcher isolément.
- **E4/E5/E6/E7/E8** : loot & chasses, l'Oubli, forge & trempe, Hall, narratif. Détail : GDD §6/§7.
- **HUD duo (E1)** : barre PV compagnon permanente + minimap **surface d'ordre** (GDD §4). ⚠️ Afficher
  l'IA monte la barre de qualité qu'on lui demande — la carte rend le navmesh et un chien de garde de
  désenlisement non négociables.

### Réseau — tranché, mais zéro ligne en v1

- **Architecture décidée le 2026-08-12** : **P2P / listen-server** (aucun serveur à payer), **autorité
  d'un seul côté** (les clients envoient des inputs), transport **Steam Datagram Relay** (gratuit avec
  Steamworks, NAT punch-through, IP masquées). Anti-avantage-hôte : délai d'input local + lag
  compensation + même buffer d'interpolation pour tous. Ces techniques règlent l'**équité**, jamais
  l'**intégrité** — à re-trancher pour le PvP de E10. Détail : GDD §10.
- **En v1 on n'écrit rien** : seules les **quatre portes** du GDD §10 sont tenues (contrat de slot,
  faction, FixedUpdate déterministe, autorité d'un seul côté). Elles ne coûtent rien aujourd'hui.
- ⚠️ **Correction 2026-08-12** : l'ancienne ligne « Coop / netcode (lightyear) » était un **reliquat V1**.
  Vérifié : **V2 n'a aucune dépendance réseau** — lightyear 0.26.4 n'a jamais quitté `d:/Forgia`.
  (`.claude/rules/build-stack.md` décrit encore le stack V1 et induit en erreur — à corriger.)

### Contenu & ship

- **Armes swappables (2) + inscriptions échangeables** au menu — cœur du build Gunfire. Masterplan Phase 3.
- **Contenu « lite »** (scope **GELÉ**) : 6-8 armes, ~40 boons, élites, 3 actes × ~4 salles, 2e arène
  ressentie. ⚠️ Le GDD reprofile l'endgame (Expéditions + 3 chasses + trempe) — la Phase 4 du masterplan
  est **périmée** sur ce point, pas sur le reste.
- **Audio dynamique complet** (combat/break/boss) + **accessibilité** (remapping, colorblind, toggle
  screenshake) + resume de run. Masterplan Phase 5.
- **Steam launch** : démo 15-30 min → CTA wishlist (sortie 2-4 sem avant Next Fest), Steamworks,
  packaging signé 60 fps GTX 1060. Masterplan Phase 6.

### Préparation à la publication (crates à instruire, rien à construire maintenant)

Listé pour ne pas le redécouvrir à trois semaines du launch — **aucun de ces chantiers n'est ouvert** :

- **Steamworks** — succès, sauvegardes cloud, et surtout **Steam Datagram Relay** dont dépend
  l'architecture réseau tranchée (GDD §10). Une seule intégration sert les trois.
- **Localisation (i18n)** — le jeu est **intégralement en français**. Un launch Steam exige l'anglais
  au minimum. Plus on écrit de texte (barks, Livre, quêtes) avant d'avoir le socle i18n, plus la
  reprise coûte cher — c'est une dette qui grossit toute seule.
- **Rapport de crash** — le capteur `crash` existe côté dev ; rien ne remonte depuis la machine d'un
  joueur.
- ⚠️ **Bevy 0.19 — le report TIENT** (revérifié 2026-08-12) : `bevy_rapier3d` n'a **aucune release
  depuis 0.35.0**, et `rapier3d` est passé d'alpha à **0.35.0-beta.0** — toujours une pré-release.
  La condition #2 de [`migration/bevy-019-blockers.md`](migration/bevy-019-blockers.md) vise un cœur
  **stable**. *Sa checklist dit littéralement « n'est plus `-alpha` », ce qu'une beta satisfait — à
  resserrer en « stable », sinon une session future donnera le feu vert sur une beta.*

### Dette technique

- **Capteur mémoire à étendre** : `forgia2_memory.json` n'expose qu'un instantané de working set.
  Manquent la **tendance** (MB/min) et la **charge de commit système** — sur Windows, une allocation
  échoue sur `commit_limit`, pas sur la RAM physique. Prérequis du diagnostic **OOM #2** (8 Mo refusés
  après 6 min, machine 32 Go). `sysinfo` est déjà dans l'arbre et dérive ces valeurs.
- `rustup update` → **Rust 1.96.1** (dès maintenant, zéro risque écosystème).
- Calibration HDR / bloom (checklist prête dans story-647) — débloque le glow émissif.
- Split des hotspots : `element_vfx.rs`, `weapon_vfx/mod.rs`, `status_vfx.rs`.
- LOD particules si les FPS chutent en combat dense ; 3 warnings clippy `live` (`status_vfx.rs`).
- **Bevy 0.19** : migration **possible** (2026-08-05 : `bevy_rapier3d` 0.35.0 cible ^0.19, hanabi/leafwing/scripting ✅) mais **volontairement différée** — cœur `rapier3d` en **alpha** + `bevy_water` dormant. Fenêtre & conditions : [`migration/bevy-019-blockers.md`](migration/bevy-019-blockers.md). **Ne rien bloquer dessus** ; plan B assumé = shipper sur 0.18.1.
- **Perf arène** (>60 fps, pas urgent — jeu à la cible) : profilée `cpu_bound` = **scène statique** (13k entités / 2254 meshes visibles), **PAS** les bots ni les VFX → seul levier = réduire la densité d'entités/meshes (merge géométrie statique). L'audit avait misé sur l'IA des bots : **invalidé par le profiling**. Détail : story-643 + `docs/audit/audit-2026-07-01-perfs-jeu-vs-industrie.md`. Outillé : `forgia2_perf.json` expose `bound_hint`/`render_cpu_ratio`.
- Nettoyage audio : le `set_volume` de canal (forgia-audio) est redondant depuis le fix volume instance-level (bevy_kira_audio 0.25, commit `a8b8d42`).
- **Trade-offs boons** (R3.4 déféré) : nécessite un schéma multi-effets (`effects: Vec<...>` dans BoonDef) — story dédiée. L'empilement multiplicatif + tirage pondéré par rareté sont FAITS (story-645).
- ⚠️ **Maîtrise d'arme** : valeur livrée **6 × 4 % (+20 % au plafond)** alors que le GDD M5 et l'audit balance visaient **10 × 2 % (+18 %)** — divergence assumée pour ne pas casser les saves, **à trancher** en passe de balance.
- Best-run affiché au Lobby/accueil (story-645 ne l'affiche qu'aux écrans de fin de run).
- `gen_voices.py` (proto gibberish 4 personas) : à re-versionner dans `tools/` si la voie gibberish est retenue.
- **Track FORGE** (anim vendeur, auto-rig, outils RPG) — seulement si ça reflue vers le ship.

---

## 🗄️ Roadmaps archivées (superseded par ce fichier)

Contenu conservé pour l'historique / le détail, mais **plus de source d'autorité** :

- `docs/ROADMAP_CURRENT.md` — historique des vagues V1→V7 (mai-juin 2026), état sensors.
- `docs/ROADMAP_ROGUELITE.md` — bible, benchmarks all-time, 3 gaps, backlog vendeur, **§ ship-gap détaillé** (référence de fond utile).
- `docs/ROADMAP_POST_AUDIT_2026-06-10.md` — priorisation post-audit du 10 juin.
- `docs/roadmap-rendering-pipeline-2026-05-19.md` — pipeline de rendu (mai).

*Le GDD dit **où on va**, ce fichier dit **ce qu'on fait maintenant**, le masterplan dit **comment** (phases 0-3 et 5), les stories disent **le détail**.*
