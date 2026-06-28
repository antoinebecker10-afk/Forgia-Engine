# Audit Rewrite V2 — Fonctionnel + Câblage Capteurs (préparation phase suivante)

> **2026-06-08** — Méthode : `cargo check`/`clippy` réels + audit statique multi-agents (13 unités : 4 groupes capteurs + 9 domaines), **vérification adversariale** de chaque finding critique/high (un sceptique tente de réfuter), + lecture des capteurs `forgia2_*.json` **live** d'une vraie session du jour (mtime 09:13).
> Workflow : 40 agents, ~2.6M tokens. Tous les `file:line` ci-dessous ont été lus puis re-vérifiés.
> **Limite** : analyse statique + snapshots disque. Le câblage et l'implémentation sont prouvés ; la preuve comportementale finale (ex : le screen-flash apparaît-il vraiment) nécessite un run in-game (règle `in-game-test-recap`).

---

## 0. Vérité-terrain build (orchestrateur)

| Check | Résultat |
|---|---|
| `cargo check --workspace` | ✅ propre (exit 0) |
| `cargo clippy --workspace --all-targets` | ✅ exit 0, **1 warning** : `cast_lossless` dans un test — `poi.rs:853` `slot as u64` → `u64::from(slot)` (entorse au standard 0-warning, fix `cargo clippy --fix`) |

Le rewrite **compile et lint proprement**. Le problème n'est pas la compilation — c'est l'écart *DONE déclaré vs câblé/actif*.

---

## 1. Verdict global

**La couche observabilité est SOLIDE et digne de confiance ; le contenu jouable Roguelite est à ~40% MVG avec plusieurs "DONE" fictifs et de l'infra non câblée — confirmés.**

- **Capteurs** : sur les 14 canoniques, **13 sont correctement câblés**, ordonnancés dans `GameSet::Sensors` via `ForgiaObservabilityPlugin` (ajouté `forgia-game/lib.rs:54`), et produisent des données **fraîches** (09:13 du jour). Les ~30 capteurs non-canoniques (roguelite, RPG, anim, world) sont eux aussi câblés et frais. La session capturée est une **vraie session Roguelite** (combat sur `RogueliteEnemy_W1_runner`, tick 40296, 92.2s).
- **Boucle de run** : démarre et se termine pour de vrai (`auto_start_run` → `sys_start_run` → 3 vagues → Victory/Defeat, Coffre entre vagues, loot-room).
- **Mais** les piliers du *ship* ont un écart claim-vs-réalité significatif (section 3).

**Bonne nouvelle (réfutations)** : plusieurs reproches du Friction Log sont FAUX. Boons appliqués au combat, armes à balistique distincte, firing-feel (muzzle/tracer/hit-flash/hit-stop) actif en Roguelite, SFX tir/impact qui jouent (preuve live), système d'éléments story-582 câblé end-to-end.

**Conclusion pour la suite** : on **peut** faire confiance aux capteurs pour piloter la phase suivante, à **5 défauts capteurs près** à corriger d'abord (section 4). Le gros chantier n'est pas l'observabilité mais **câbler ce qui est à moitié construit**.

---

## 2. Statut fonctionnel par domaine

| Domaine | Statut | Preuve (file:line) |
|---|---|---|
| Boucle run start→waves→Victory/Defeat | ✅ works | `run.rs:702` start, `waves.rs:231-245` victory, `run.rs:774-804` end |
| Progression multi-stage / boss | ❌ broken | seul setter `run.rs:702` `InRun{stage:0}`; RunGraph inséré `run.rs:686` jamais parcouru; `RunState::Boss` mort |
| HUD compteur vague/stage | ⚠️ partial | `hud.rs:79-80` "WAVE X / 4" vs `waves.rs:32` `WAVES_TOTAL=3` |
| Cœurs / soin pickup | ❌ broken | `run.rs:350-364` Pickup sans marqueur Heart; `loot_tables.rs:91-93` n'ajoute que de l'Or, 0 heal |
| Boons (application combat) | ✅ works | `boons_apply.rs:45-74` (7 effets) consommés `forgia-fps/lib.rs:607-811` |
| Boons (feedback per-boon) | ⚠️ partial | `BoonAppliedEvent` émis `boons.rs:394`, **0 consommateur** |
| Économie Or in-run | ✅ works (tuning) | ~115 Or/run vs commons 20-25 → ~2-4 boons/run |
| MetaSouls (méta) | ⚠️ partial | accumulé `waves.rs:235-248`; **0 décrément, 0 persistance disque** |
| Armes — balistique de base | ✅ works | `forgia-fps/lib.rs:511-660` lit `viewmodel_arena.toml`, 4 profils distincts |
| Armes — gimmicks signature | ❌ orphan | `roguelite_weapons.toml` jamais chargé/lu |
| Éléments (story-582) | ✅ works | `elements.rs` câblé `lib.rs:205-234`; live `elements.json` hits.explosive:2 |
| Firing feel (muzzle/tracer/hit-flash/hit-stop) | ✅ works | `forgia-fps/lib.rs:649-865`, gate `Fps.or(Roguelite)` |
| SFX tir/impact (Roguelite) | ✅ works | `audio.rs:426-440`; live `roguelite_audio.json` fires:13 sfx:17 |
| Screen-flash rendu | ❌ wrong_gate | `forgia-juice-screen-flash/lib.rs:221` early-return si != `GameMode::Fps` → invisible en Roguelite |
| Damage routing | ⚠️ partial | bot→player via DamageEvent (HealthGuard OK); player→enemy mutation directe `forgia-fps/lib.rs:815` (bypass) |
| IA bots (4 archétypes, LOS, raycast) | ✅ works | `enemies.rs:59-116` stats distinctes; LOS story-464 + raycast story-545 fixés + testés |
| Nameplate HP fill | ❌ broken | `forgia-enemy-nameplate/lib.rs:253-279` draine du centre (fix décrit en commentaire, jamais codé) |
| Worldgen story-578 | ✅ works (démo) | plugin `lib.rs:98`, registry 107 modules; **mais dev-tool F7/F9, spawned:0** |
| Terrain + foliage | ⚠️ RPG-only | tout gated `GameMode::Rpg` → **dormant dans le produit Roguelite** |
| Anim/rig Rex (579/496) | ✅ works (RPG-only) | pipeline réel + testé, mais **0 référence dans forgia-mode-roguelite** |
| RPG data-loop (570) + 4 panneaux UI | ✅ works (RPG-only) | dialogue→item/quête→XP réel; panneaux fonctionnels, gated `Rpg` |
| HUD stubs portal/bark/notification | ❌ stub | `hud.rs:594/634/649` corps vides, schedulés `1250/1253/1254` |
| arena_feedback (SFX kill/dmg) | ❌ stub | `arena_feedback.rs:48-50` systèmes son commentés, compteurs jamais mutés |

---

## 3. Écarts claim-vs-réalité (survivants à la vérif adversariale, tous UPHELD high)

1. **Multi-stage = FICTION** (ROG-LOOP-01). RunGraph 4-stages généré/inséré jamais traversé ; `RunState::Boss` et `InRun{stage>0}` morts. Le "boss" = ennemi archétype Boss dans la vague 3 d'un stage unique. *Grep `next.set(RunState…)` = exactement run.rs:702 + run.rs:801.*
2. **HUD "WAVE X / 4" ment** (ROG-LOOP-02 / FL-R07). Numérateur = index vague, dénominateur = compte stage (gène=4) ≠ `WAVES_TOTAL=3`. Le **capteur n'est PAS le menteur** : il expose honnêtement `waves_total:3` ET `stage_count:4`.
3. **Cœurs = pièces reskin** (BE-04). FL-R12 (double-dip) RÉFUTÉ ; bug pire : le cœur ne soigne rien, donne de l'**Or**. Un cœur de boss (valeur 40) = 40 Or, 0 PV. `run.rs:350-364` Pickup sans `Heart`, `loot_tables.rs:91-93` Or only.
4. **Gimmicks d'armes orphelins** (WPN-03). `roguelite_weapons.toml` (ricochet/lifesteal/cleave/burst) chargé par **aucun** loader genome ; grep `ricochet|lifesteal|cleave|scope_zoom` = 0 producteur Rust. Story-564 = DRAFT jamais faite. *Note : FL-R03 mal attribué — `run.rs:278 'let _ = &equipped;'` est dans l'observer de loot, pas le firing path (WPN-01).*
5. **Stubs no-op stories 471-479** (ROG-LOOP-07 / UIRPG-01). `draw_portal_overlay/bark_bubble/stage_notification` vides mais schedulés chaque frame ; `parse_music_state` toujours None ; toggles musique/météo log-only. Compilent, ordonnancés, ne font rien.
6. **arena_feedback stub** (CFD-03). Compteurs `damage_sounds_played:0` structurellement à 0 → **le capteur `arena.json damage_sounds_played` est un RED HERRING** : ne PAS l'utiliser comme signal SFX Roguelite (utiliser `roguelite_audio.json`).
7. **2 chemins parallèles d'acquisition de boons** (BE-05). Payant `boons.rs:358-394` (déduit Or + émet event) vs gratuit `loot_room.rs:455-488` (`apply()` direct, sans coût ni event). Preuve live : `coffre.json souls_spent_total:0` mais `boons.json active_count:2`.
8. **MetaSouls sans sink ni persistance** (ROG-LOOP-04 / FL-R01/R02). Jamais décrémenté, aucun `fs::write`. Reset à 0 à chaque lancement. Sink = story-569 non fait.
9. **vfx_tracers orphelin** (CFD-07). `ForgiaVfxTracersPlugin` re-exporté mais jamais `add_plugins`. Le vrai tracer = `weapon_vfx::tracer`.
10. **forgia-combat/weapons.rs vestigial** (WPN-06). `#![allow(dead_code)]`, systèmes de tir commentés. Vrai firing = `forgia-fps::fire_weapon_minimal`. Ne pas y étendre les gimmicks.

---

## 4. Défauts CAPTEURS à corriger pour faire confiance (avant phase suivante)

1. **`player_hp_diag.json` ORPHELIN** (OBS-06). Aucun producteur ; seul un reader `forgia-debug/snapshot.rs:225`. Figé 17 jours (2026-05-22). Compté à tort "present" par `migration_baseline`. → supprimer fichier + reader, ou restaurer un producteur.
2. **`assets.json` ORPHELIN** (OBS-05). `assets_load_sensor.rs` **même pas compilé** (`mod` non déclaré dans lib.rs). Fichier 17 jours stale. → supprimer ou câbler.
3. **`rpg_health` = faux warn permanent cross-mode** (SENS-rpg_health-02). Producteur gated `GameMode::Rpg` only mais surveillé cross-mode par `sensor_health` (seuil 10s) → hors RPG, `sensor_health` severity:**warn** permanent. → rendre `sensor_health` mode-aware. **D'ici là, ignorer le warn de sensor_health hors RPG.**
4. **`arena` agrégat dégradé en Roguelite** (SENS-arena-01). Source `arena_waves` gated `Fps`-only → fichier 4 jours stale → `forgia2_arena` severity=warn permanent en Roguelite. → retirer `arena_waves` des sources en Roguelite, ou le faire écrire par le mode roguelite.
5. **`sensor_health` ne surveille que 12/14 canoniques** (OBS-07). Il **manque `roguelite_state` et `fps_feel`** — exactement les 2 capteurs du mode qui ship. Si leur producteur meurt, le watchdog reste vert. → ajouter les 2 à `EXPECTED_SENSORS` (idéalement dériver d'une const partagée avec xtask).

*Autre :* `toon.json` émet un **vrai warn** légitime (0 caméra attachée → post-process invisible, SENS-roguelite-02). `migration_baseline.plugin_count=50` est un alias trompeur du compte capteurs (SENS-migration_baseline-05).

---

## 5. Angles morts capteurs (à instrumenter pour la phase suivante)

- **Damage player→enemy** non instrumenté (mutation directe `forgia-fps/lib.rs:815`, bypass pipeline `forgia-damage`) — asymétrie avec bot→player observé.
- **Progression de stage** : rien ne signale que les stages>0 ne sont jamais entrés (la fiction multi-stage est invisible aux capteurs).
- **Total réel de boons acquis/run** : `coffre_sensor.souls_spent_total` ne capte que le chemin payant (loot-room gratuit invisible).
- **Hearts/heal** : aucun compteur PV soignés vs Or gagné → le bug "cœur=pièce" est invisible.
- **Stations heal/ammo** : aucun capteur d'usage/reset.
- **First-shot hanabi freeze** : `prespawn_hanabi_dummies` stub, hitch non instrumenté.
- **Détection générique "wired but inert"** : aucun capteur ne signale les no-op (portal/bark/notification, arena_feedback, vfx_tracers).
- **Affordabilité économie live** : Or collecté vs coût boons non observé (FL-R05 déduit manuellement).

---

## 6. Actions priorisées

### P0
- **Corriger le gate screen-flash** : `forgia-juice-screen-flash/lib.rs:221` accepter Roguelite (`matches!(*game_mode.get(), GameMode::Fps | GameMode::Roguelite)`). Logique + compteurs tournent déjà ; seul le rendu egui est supprimé. **Plus gros ROI hit-feedback du mode qui ship.**
- **Réparer les cœurs** : marqueur `Heart` + `sys_collect_hearts` qui soigne `forgia_combat::Health` (clamp max), exclure des `sys_collect_pickups` (`Without<Heart>`).
- **Aligner le HUD** : dénominateur → `WAVES_TOTAL=3` (`hud.rs:79-80`) jusqu'à ce que la vraie progression existe.
- **Décision produit progression** : SOIT implémenter le vrai multi-stage (avancer `RunState`, piloter `sys_stage_dispatch`), SOIT assumer mono-stage 3-vagues et supprimer `RunState::Boss` + RunGraph inutile.

### P1
- **Gimmicks d'armes** : SOIT câbler `roguelite_weapons.toml` (charger genome + lire dans firing path), SOIT marquer "deferred"/supprimer.
- **`sensor_health` mode-aware** : ne surveiller `rpg_health` qu'en RPG + ajouter `roguelite_state`+`fps_feel` aux surveillés.
- **Agrégat arena** : retirer `arena_waves` des sources en Roguelite (ou le faire écrire par le mode).
- **Nettoyer `player_hp_diag` + `assets`** (orphelins) + filtrer `scan_sensor_files()` par fraîcheur.
- **Supprimer/finir les stubs 471-479** (portal/bark/notification, parse_music_state) + `ForgiaVfxTracersPlugin` orphelin.
- **Nameplate HP fill anchor** (`forgia-enemy-nameplate/lib.rs:254-279`) + BUG-464-04 (`los_lost_grace` hardcodé en Roguelite).

### P2
- **Persistance disque + sink MetaSouls** (story-569) OU masquer l'UI Âmes.
- **Router les boons loot-room** via helper partagé qui émet `BoonAppliedEvent` + ajouter ≥1 consommateur (sting/toast).
- **`prespawn_hanabi_dummies`** (risque freeze first-shot) + externaliser l'économie hardcodée (`roguelite_economy.toml`).
- **Réconcilier LOCK-INV-1** : code=80 slots vs CLAUDE.md=20 (UIRPG-06).

---

## 7. Matrice de câblage capteurs

| sensor | producer (file:line) | scheduled | run_if | canonical | fresh | verdict |
|---|---|---|---|---|---|---|
| health | health_sensor.rs:14 | true | none (cross-mode) | ✅ | fresh | ok |
| rpg_health | exporter.rs:16/67 (chain) | true | **Rpg ONLY** (lib.rs:203) | ✅ | stale | **wrong_gate** |
| arena | forgia2_aggregator.rs:125 | true | aggr Fps.or(Rgl); **source arena_waves Fps-only** | ✅ | stale | **wrong_gate** |
| combat | forgia2_aggregator.rs:125 (5 sources) | true | Fps.or(Roguelite) | ✅ | fresh | ok |
| perf | perf_sensor.rs:73 | true | none | ✅ | fresh | ok |
| entities | entities_sensor.rs:73 | true | none | ✅ | fresh | ok |
| memory | memory_sensor.rs:69 | true | none | ✅ | fresh | ok |
| lifecycle | lifecycle_sensor.rs:93 | true | none | ✅ | fresh | ok |
| watchdog | watchdog_sensor.rs:74 (+First) | true | none | ✅ | fresh | ok |
| sensor_health | sensor_health_sensor.rs:45 | true | none | ✅ | fresh | ok (surveille 12/14) |
| audio | audio_sensor.rs:24 | true | none | ✅ | fresh | ok |
| input | input_sensor.rs:37 | true | none | ✅ | fresh | ok |
| roguelite_state | mode-roguelite/sensor.rs:76 | true | none (cross-mode) | ✅ | fresh | ok |
| fps_feel | fps_feel_sensor.rs:40 | true | none | ✅ | fresh | ok |
| vram | vram_sensor.rs:201 | true | none | ❌ | fresh | ok (warn réel 2.5GB) |
| lag_events | lag_events_sensor.rs:114 (+First) | true | none | ❌ | fresh | ok |
| physics | physics_sensor.rs:39 | true | none | ❌ | fresh | ok |
| migration_baseline | migration_baseline.rs:79 | true | none (1×@T+5s) | ❌ | fresh | ok (compte gonflé) |
| player_state | player_state_sensor.rs:134 | true | none | ❌ | fresh | ok |
| **player_hp_diag** | **AUCUN — reader only (snapshot.rs:225)** | false | n/a | ❌ | **stale 17j** | **orphan_file** |
| **assets** | **assets_load_sensor.rs (non compilé)** | false | n/a | ❌ | **stale 17j** | **orphan_file** |
| elements | mode-roguelite/elements.rs:548 | true | none (stats run_if Rgl) | ❌ | fresh | ok |
| npcs | npcs_sensor.rs:46 | true | none | ❌ | fresh | ok |
| roguelite_audio | mode-roguelite/audio.rs:521 | true | none | ❌ | fresh | ok |
| roguelite_intro | mode-roguelite/intro_dialogue.rs:168 | true | Roguelite | ❌ | fresh | ok (omet next_step) |
| boons | boons_sensor.rs:40 | true | none | ❌ | fresh | ok |
| coffre | mode-roguelite/coffre_sensor.rs:63 | true | none | ❌ | fresh | ok |
| stage | forgia-stage/lib.rs:385 | true | none | ❌ | fresh | ok |
| stage_decor | mode-roguelite/decor.rs:1133 | true | none | ❌ | fresh | ok |
| stage_layout | forgia-stage/layout_sensor.rs:108 | true | none | ❌ | fresh | ok |
| stage_poi | mode-roguelite/poi.rs:725 | true | none | ❌ | fresh | ok |
| toon | mode-roguelite/toon_config.rs:259 | true | none | ❌ | fresh | **warn réel (0 caméra)** |
| rpg_player | rpg_player_sensor.rs:50 | true | none | ❌ | fresh | ok |
| quests | quests_sensor.rs:42 | true | none | ❌ | fresh | ok |
| inventory | inventory_sensor.rs:50 | true | none | ❌ | fresh | ok |
| worldgen | forgia-worldgen/sensor.rs:25 | true | none | ❌ | fresh | ok (spawned:0) |
| anchor | forgia-anchor/lib.rs:257 | true | none (via forgia-stage) | ❌ | fresh | ok |
| level_modules | forgia-level-presets/lib.rs:259 | true | none (via forgia-stage) | ❌ | fresh | ok |
| skeleton_template_registry | forgia-skeleton-template/lib.rs:867 | true | none (via auto-rig) | ❌ | fresh | ok |
| auto_rig | forgia-auto-rig/lib.rs:400 | true | none (idle hors RPG) | ❌ | fresh | ok (idle) |
| menu_video | forgia-ui/menu_video.rs:232 | true | none | ❌ | fresh | ok |
| walk_pose / rex_bones_live / walk_dir_probe / rex_bones | anim-locomotion/locomotion.rs | true | **Rpg** (sched rpg/lib.rs:223-225) | ❌ | fresh (RPG-relatif) | ok (diag RPG-only) |
| forgia_arena_waves (source) | forgia-mode-fps-arena/wave.rs:471 | true | **Fps ONLY** | ❌ | stale 4j | **wrong_gate (mort en Rgl)** |
| forgia_arena_feedback (source) | forgia-effects/arena_feedback.rs:51 | true | compteurs jamais mutés | ❌ | fresh 0/0 | **stub** |

**Bilan** : 13/14 canoniques OK+frais. Défauts : 2 wrong-gate canoniques (rpg_health, arena), 2 orphelins (player_hp_diag, assets), 1 watchdog incomplet (sensor_health 12/14). Tout le reste câblé et frais.
