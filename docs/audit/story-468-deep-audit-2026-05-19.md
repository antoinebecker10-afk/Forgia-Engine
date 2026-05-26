# Story-468 — Deep Audit 360° (5 agents parallèles)

> **Date** : 2026-05-19
> **Objet** : audit indépendant en profondeur du plan story-468 (3e jeu Forgia, roguelite FPS coop)
> **Méthode** : 5 agents general-purpose en parallèle (ECS Bevy, Pipeline contenu, Persistence/Steam, Perf/Tests, Risk/Timeline)
> **Total sources URL vérifiables** : ~120
> **Verdict global** : **3 BLOQUANTS** à corriger avant impl + **8 AJUSTER** + reste validé

---

## Tableau de synthèse exécutive

| ID | Sujet | Verdict | Impact | Action |
|---|---|---|---|---|
| **A2** | DamageEvent comme EntityEvent multi-observer | 🔴 **BLOQUANT** | Ordre observers NON garanti Bevy 0.18 | `BufferedEvent` + 3 systems `.chain()` |
| **E1** | Timeline 12 sem 1.0 / EA stable | 🔴 **BLOQUANT** | Aucun roguelite indé n'a fait < 18 mois | Re-cadrer : Next Fest = **démo vertical slice solo-only** |
| **B4** | Strings FR inline dans genome TOML | 🔴 **BLOQUANT** ship EN | Steam EN requis 99% marchés | Refacto i18n IDs + .ftl avant ship |
| A3 | par_iter_mut().for_each() syntax | 🟡 AJUSTER | API correcte 0.18 | Note dans plan, retirer "32" comme seuil dur |
| A4 | StageScoped custom vs DespawnOnExit<S> | 🟡 AJUSTER | Over-engineering | Démarrer natif, custom seulement si conditionnel |
| A6 | 258 crates schedule overhead | 🟡 AJUSTER | Possible 8-15% slowdown (Issue #11932) | 1 SystemSet par crate + bench dès >1500 systems |
| A7 | set_if_neq sur set_state | 🟡 AJUSTER | Breaking 0.17→0.18, re-trigger OnEnter | Audit obligatoire tous `next_state.set()` |
| A8 | "Avg < 6ms" frame budget | 🟡 AJUSTER | Non sourcé, irréaliste | Cible révisée `p50 < 12 ms, p99 < 16.6 ms` |
| C1 | bevy_persistent crate | 🟡 AJUSTER | Maintenance flou | Roll-your-own RON/TOML + serde_flow |
| C9 | Steam Deck Verified | 🟡 AJUSTER | +2x ventes documenté | Prioriser Linux +30j post-Windows |
| E4 | Marketing Next Fest absent du plan | 🟡 AJUSTER | <1k wishlists = +322 wishlists median | Steam page sem 3-4, devlogs, 30-50h dédiées |
| Q1 | lightyear Steam transport custom | ✅ **VALIDÉ post-correction** | `lightyear_steam` officiel existe | Aucune ligne transport custom à écrire |
| Q3 | RNG déterministe pattern | ✅ **VALIDÉ post-correction** | `rand_xoshiro::Xoshiro256StarStar` | Host-authoritative, pas de float det rapier |
| Q4 | bevy_rapier3d 0.34 (pas 0.33) | ✅ **VALIDÉ post-correction** | CLAUDE.md à corriger | Bump Cargo.toml |
| Reste | Architecture, scaffolds, patterns | ✅ VALIDÉ | Plan original solide | — |

---

## 1. BLOQUANTS — actions avant impl

### 1.1 — BLOQUANT A2 : DamageEvent ne peut PAS être EntityEvent multi-observer

**Problème** : doc Bevy 0.18 Observer explicite :

> *"Currently, Bevy does not provide a way to specify the relative ordering of observers watching for the same event. Their ordering is considered to be arbitrary. It is recommended to make no assumptions about their execution order."*

Et : *"You cannot use SystemSet to order observers watching the same event."*

**Impact Forgia** : si DamageEvent émet vers 3 observers (Effects hit-stop, UI damage numbers, Sensors kill log), l'ordre est non garanti, ce qui :
- Désynchronise hit-stop avec damage number
- Peut ignorer un sensor avant qu'un observer drop l'event

**Décision** :
- **`DamageEvent` = `BufferedEvent`** (PR #19647), consommé par 3 systems `.chain()` dans `GameSet::Effects`.
- **`EntityEvent` réservé aux Hit zones boss** (propagation `ChildOf` parent collider — pattern Overwatch hit zones documenté `examples/ecs/observer_propagation.rs`).
- **`Event` simple = events globaux** non ciblés (RunStartedEvent, StageClearedEvent).

**Sources** :
- [Observer docs.rs](https://docs.rs/bevy/latest/bevy/ecs/observer/struct.Observer.html)
- [PR #19647 Event Split](https://github.com/bevyengine/bevy/pull/19647)
- [PR #20731 Event Rearchitecture](https://github.com/bevyengine/bevy/pull/20731)
- [observer_propagation.rs example](https://github.com/bevyengine/bevy/blob/main/examples/ecs/observer_propagation.rs)

### 1.2 — BLOQUANT E1 : timeline 12 sem irréaliste pour 1.0/EA

**Données chiffrées comparaisons indé** :
| Jeu | Équipe | EA → 1.0 | Total dev |
|---|---|---|---|
| Roboquest | ~4 devs | 3 ans | 5+ ans |
| Gunfire Reborn | ~10 devs | 18 mois | 3-4 ans |
| Voidigo | ~5 devs | 2 ans 4 mois | 4+ ans |
| Katanaut | 1 solo | direct 1.0 | **3 ans** |

**Aucun roguelite FPS solo en 12 semaines** dans l'historique 2020-2026. Même avec workspace V2 mûr (258 crates, 2 jeux), le sous-projet roguelite + coop netcode honnête = 6-12 mois.

**Re-cadrage** :
- **Cible Next Fest 2026 = démo vertical slice SOLO-ONLY** (coop = post-démo)
- **Pas de "ship 1.0 12 sem"** dans la communication
- **Go/no-go semaine 6** obligatoire (si M3 gameplay loop pas atteint → drop scope drastique)
- **Buffer 30%** sur tous les milestones

**Burn-out risk** : data 2025 dit 60-70% indé solo en burn-out actif si 55h+/sem 2 semaines consécutives. AI assist gain réel = **+10-15% net** (pas +55% labo benchmarks). 12 sem soutenable = ~480-550h, pas 900h.

**Sources** :
- [Katanaut solo dev 3 years](https://www.gamesradar.com/games/roguelike/game-dev-hard-mode-discovered-as-solo-indie-releases-metroidvania-inspired-roguelike-right-between-silksong-and-hades-2-i-had-zero-awareness-that-they-were-dropping/)
- [Roboquest 1.0 Nov 2023](https://www.savingcontent.com/2023/11/07/roboquest-version-1-0-is-now-live-on-on-xbox-and-pc-via-steam-and-epic-games-store/)
- [Indie burnout 60% Wayline 2025](https://www.wayline.io/blog/burnout-journaling-future-2025)
- [Octoverse AI productivity 19% slowdown reality](https://github.blog/ai-and-ml/generative-ai/how-ai-is-reshaping-developer-choice-and-octoverse-data-proves-it/)

### 1.3 — BLOQUANT B4 : strings FR inline dans genome TOML

**Problème** : `roguelite_dialogue.toml` actuel a 24 barks × 4 armes en FR inline. Ship Steam = EN minimum requis (99% marchés).

**Décision** :
- Refacto **avant ship** : strings → IDs Fluent dans TOML, .ftl files par locale.
- Stack : `bevy_fluent 0.13+` (vérifier compat 0.18.1 avant impl, fallback mini-i18n maison si retard upstream).
- Effort : ~96 strings × 2 locales = 192 entrées = **1-2 jours**.
- Sentinel pattern : tant que pas i18n, le plan note explicitement "ship blocker EN".

**Sources** :
- [bevy_fluent GitHub](https://github.com/kgv/bevy_fluent)
- [Bevy i18n discussion #5874](https://github.com/bevyengine/bevy/discussions/5874)
- [EAA impact gaming Bird & Bird](https://www.twobirds.com/en/insights/2026/the-impact-of-the-european-accessibility-act-on-online-gaming-and-gaming-devices)

---

## 2. AJUSTER — modifications plan story-468

### 2.1 Frame budget : abandonner "< 6 ms"

**Cible révisée sourcée** :
- **Plancher** : Steam Deck/Machine Verified = 30 FPS @ 1080p = `p99 < 33 ms`
- **MVP primary** : 60 FPS @ 1080p RTX 3060 (#1 GPU Steam survey 4.15%) = `p99 < 16.6 ms`
- **Stretch** : 120 FPS PC haut de gamme = `p99 < 8.3 ms`

Budget répartition proposée 16.6 ms : Input 0.5 + Physics 2.0 + Combat 1.0 + AI 1.5 + Effects 1.0 + UI 0.5 + Sensors 0.1 + Replication 1.5 + Render 6.5 + Marge 2.0.

### 2.2 Architecture observers

**Règle** :
- `BufferedEvent` pour multi-consumer ordonné via `.chain()` (DamageEvent, PickupEvent, StageCleared, RunStarted)
- `EntityEvent` pour cibles entité avec propagation `ChildOf` (HitZoneEvent boss multi-collider seulement)
- `Event` simple pour Observer global ponctuel (rare)

### 2.3 Stage cleanup : démarrer natif

- **`DespawnOnExit<RunState::InRun{stage}>`** suffit. Bug #21832 fixé en 0.18.1.
- **NE PAS** appeler `enable_state_scoped_entities()` (déjà implicite, double-despawn bug #20866).
- `StageScoped<StageId>` custom seulement si cleanup conditionnel apparaît (boss persiste dans Reward stage).

### 2.4 SubStates + ComputedStates

- `RunState` = SubStates (état mutable)
- `IsBossStage` = ComputedStates (pure fonction de `RunState`)
- **`set_if_neq()` partout** (breaking 0.17→0.18 : `set()` re-trigger OnEnter même si même valeur)
- Ordre OnEnter/OnExit déterministe via `EnterSchedules<S>` / `ExitSchedules<S>` (PR #13763)

### 2.5 Performance regression detection

- **`cargo-nextest`** obligatoire (3× plus rapide vs `cargo test`, retry flaky natif)
- CI matrix ubuntu+windows + sccache (< 8 min PR gate)
- Baseline JSON `docs/baselines/perf_story_468.json` (seed=fixe, scenario=stage_1_to_boss)
- CI fail si `p99` dérive > 10% baseline
- Sensor `forgia2_perf.json` parsing comme oracle

### 2.6 Test multiplayer

- **`lightyear_crossbeam`** transport pour tests CI (pas besoin Steam SDK)
- Pattern `setup_client_server_apps(n_clients)` helper
- PR gate : "host + 1 client" (suffisant)
- Nightly : "host + 2 clients" (edge cases coop 3p)

### 2.7 Persistence / Saves

- **RON ou TOML versionné** + `serde_flow` (annotations migration) ou `#[serde(default)]` manuel
- **Per-user** méta-progression (pas per-host)
- Steam Cloud via `bevy-steamworks 0.16::RemoteStorage`, quota 100 MB
- Save par stage transition (Hadès pattern), pas par frame
- Crash mid-stage = run perdu (standard Hadès/Dead Cells), Sentry capture cause
- **Pas bevy_persistent** (maintenance flou)

### 2.8 Steam features

- 10-15 achievements MVP simples (first kill, first run, weapon mastery, coop)
- Leaderboards **friends-only par défaut** (StS 2 a reculé global → friends en 2026)
- Rich Presence : `SetRichPresence("stage", "3")` + `ActivateGameOverlayInviteDialog`
- **SKIP Workshop** MVP (roguelite procedural = peu d'UGC natif)
- DRMless défaut, pas VAC (PvE only)
- Pricing tier : **$19.99-24.99** launch (Roboquest $24.99, Gunfire $19.99 references)
- Démos Next Fest = playtime ne compte pas refund (changement 2026)
- **Linux/Steam Deck Verified +30j post-Windows** (+2x ventes documenté)

### 2.9 Telemetry

- **Phase 1 MVP** : Sentry Rust SDK crash dump (free tier 5k/mo, panic_hook)
- **Phase 2 post-launch** : PostHog opt-in events (free 1M/mo, EU hosting)
- **Phase 3** : Steam Stats API built-in
- RGPD : opt-in obligatoire (default OFF), exempt EAA (<10 emp + <€2M)

### 2.10 Marketing Next Fest

**Trou identifié** : pas dans le plan 6 milestones. Sans marketing :
- <1k wishlists d'entrée Next Fest → médiane outcome = **+322 wishlists seulement** (Ziva.sh 2026)
- 30k wishlists launch = Gold tier $250k revenue (Zukowski 2025)

**Effort à ajouter** : ~30-50h sur 12 sem :
- Steam page publiée semaine 3-4 (capsule, trailer 30s, screenshots, store description)
- 2 devlogs YouTube semaine 5/8
- TikTok demos 2/semaine dès semaine 4
- X/Reddit posts hebdo

### 2.11 Voice acting

**Risque pitch armes parlantes** :
- TTS pur ElevenLabs = risque "robotique" qui tue le concept High on Life vibe
- Recommandation : **budget voice acting humain semi-pro $500-1500** pour 4 voix × 24 lignes
- Fallback acceptable : ElevenLabs Creator $22/mo si budget contraint MVP

### 2.12 V1/V2 freeze decision

**Avant kickoff impl story-468** : décision binaire publique.

**Option A** — Freeze V1/V2 :
- Bug fix sécurité seulement
- Aucune nouvelle feature
- Communication user "support critique seulement"

**Option B** — Annuler story-468 :
- Si V1/V2 ont momentum commercial, focus dessus
- Roguelite reporté à 2027

**Aucune autre option soutenable solo dev** (E8 : Hopoo a fermé après tentative multi-jeux).

---

## 3. VALIDÉ — confirmés post-audit

### 3.1 Netcode : lightyear + lightyear_steam officiel

- **`lightyear_steam` existe** (shipped dans repo lightyear, wrappe `steamworks::networking_sockets` modern API).
- **0 ligne transport custom à écrire** (correction estimation initiale 3-5j R&D).
- Listen-server P2P host-authoritative = pattern majoritaire roguelite coop 2023-2026 (DRG, RoR2, Remnant 2, Roboquest tous identiques, **aucun avec host migration**).
- Bandwidth coop 1-3J faible, snapshot interp suffit.

### 3.2 RNG déterministe

- **`rand_xoshiro::Xoshiro256StarStar`** : 256-bit state serializable, `jump()` pour streams indépendants.
- Pattern `(run_seed, stage_id, encounter_idx)` confirmé par Slay the Spire (oohbleh.github.io).
- **Float déterminisme rapier NON requis** (host-authoritative = clients reçoivent positions interp). Évite la feature `enhanced-determinism` rapier (perf hit SIMD désactivé).

### 3.3 Bevy 0.18 + écosystème

- Bevy 0.18.1 stable mars 2026 (patch correctif 0.18.1)
- **bevy_rapier3d 0.34** (pas 0.33 comme dans CLAUDE.md actuel — à corriger)
- bevy_hanabi 0.18.0 (fév 2026)
- bevy_kira_audio 0.25 (jan 2026)
- bevy-steamworks 0.16 (SDK v158a bundled)
- bevy_replicon 0.40.1 (mai 2026, alternative non retenue)

### 3.4 Patterns industrie standards

- Drops sync coop = host-authoritative (DRG pattern, confirmé Memory)
- Anti-cheat skip pour PvE coop (VAC = 100 jeux compétitifs sur 41 400 Steam)
- Saves per-user pour méta-progression (Hadès, RoR2, Roboquest)
- Workshop skip MVP (roguelite procedural = peu UGC natif)

### 3.5 Asset pipeline

- Meshy v5 Pro $20/mo = full ownership commercial
- Pipeline Meshy → Blender → Pinocchio auto-rig validé (Rex+Apprenti 2026-05-18)
- 8 modèles 3D (4 armes + 3 enemies + 1 boss) = ~$60 + 2-3 sem effort (2-3 itérations/modèle)
- VRAM estimée MVP : ~250-500 MB sur 4 GB dispo confortable

### 3.6 VFX hanabi 0.18

- Pre-spawn dummy anti-25s-freeze toujours nécessaire 0.18
- 10k particules simultanées = 0 impact frame budget moderne
- Decals report post-Next-Fest, muzzle flash + hit splatter + boss telegraph suffisent MVP

---

## 4. Risk register prioritized (top 10)

| # | Risque | Impact | Prob | Mitigation |
|---|---|---|---|---|
| R1 | Burn-out semaine 6-8 | CRITIQUE | 60-70% | Go/no-go sem 6 + buffer 30% + cap 55h/sem |
| R2 | Coop netcode debug black hole | HAUT | 50% | Drop coop si retard, solo-only Next Fest |
| R3 | Scope creep "1 feature en plus" | HAUT | 80% | Feature freeze hard sem 8 |
| R4 | Maintenance V1/V2 ronge V3 | MOYEN-HAUT | 70% | Freeze V1/V2 publiquement sem 0 |
| R5 | Next Fest <1k wishlists = +322 only | HAUT | 70% sans marketing | Marketing 30-50h dans le plan |
| R6 | Voix armes parlantes ratée TTS | HAUT | 40% | Budget voice acting humain $500-1500 |
| R7 | Concurrence (HLB 1.0, Helldivers roguelite) | MOYEN | 30% | Choisir window release évitant majors |
| R8 | Dépendances solo mainteneur | MOYEN | 20% | Fork bevy-steamworks + hanabi sem 1 |
| R9 | i18n EN refacto bloque ship | MOYEN | 60% si reporté | Refacto sem 9-10, pas plus tard |
| R10 | Cohérence vision "YouTube du gaming" | LONG | 40% | Décision strategique pré-kickoff |

---

## 5. Scope révisé Next Fest démo (MVP serré)

**Cible démo Next Fest oct 2026** (réaliste 12 sem) :

| Feature | In MVP démo | Hors MVP démo (post-) |
|---|---|---|
| Mode solo run loop complète | ✅ | — |
| 1 biome roguelite | ✅ | 2 biomes supplémentaires |
| 4 armes parlantes (TTS ou VO basique) | ✅ | i18n EN polish |
| 3 enemies + 1 boss | ✅ | Variations elite + nouveau boss |
| StageGraph 4 stages + boss arena | ✅ | Choix branching multiples |
| Director difficulty scaling | ✅ | — |
| Loot tables + equipment 2 slots | ✅ | Skill tree méta-progression |
| Status effects basique | ⚠️ Mineur | Stacking complet |
| **Coop 2-3J** | ❌ **Post-démo** | ✅ M5 décalé post-Next Fest |
| Steam Cloud + Achievements | ⚠️ Minimum (10 achievements) | Leaderboards |
| Sentry crash dump | ✅ | PostHog opt-in |
| Subtitles + accessibility basics | ✅ | Color-blind modes |
| Marketing Steam page + 2 devlogs | ✅ | Influencer outreach |
| Linux Steam Deck Verified | ❌ Post-démo | ✅ +30j post-Windows |

**Communication explicite** : "Forgia Roguelite Demo - Solo Vertical Slice", PAS "1.0" ou "EA".

---

## 6. Synthèse actions immédiates

### Pré-kickoff (avant story-468 M1)

1. ✅ **Décision V1/V2 freeze** (binary) — Antoine
2. ✅ **Décision V3 cohérence vision** : roguelite premium = écart funnel "YouTube du gaming". Justifier ou pivoter pitch.
3. ✅ **Re-cadrer cible Next Fest** = démo vertical slice solo-only (pas 1.0)
4. ✅ **Mettre à jour CLAUDE.md** : `bevy_rapier3d 0.33 → 0.34`
5. ✅ **Mettre à jour story-468** : sections 3 (Acceptance), 6 (Netcode), 7 (Patterns), 11 (Validation gate)
6. ✅ **Mettre à jour ROADMAP V7** : milestones révisés, marketing ajouté
7. ✅ **Fork interne** bevy-steamworks + bevy_hanabi (semaine 1)

### M1 prep (post-merge V6 E1+E2)

1. Vérifier compat Bevy 0.18.1 sur deps critiques (rapier 0.34, hanabi 0.18.0, kira 0.25, steamworks 0.16, replicon 0.40.1)
2. Bench `cargo-nextest` baseline (8 min ubuntu cache chaud)
3. Setup Sentry Rust SDK crash dump
4. Audit `next_state.set()` workspace → migrer `set_if_neq()` partout

---

## 7. Sources canoniques les plus fortes (consolidation)

### Bevy 0.18
- [Bevy 0.18 release notes](https://bevy.org/news/bevy-0-18/)
- [Bevy migration 0.17→0.18](https://bevy.org/learn/migration-guides/0-17-to-0-18/)
- [Observer docs.rs](https://docs.rs/bevy/latest/bevy/ecs/observer/struct.Observer.html)
- [PR #19647 Event Split](https://github.com/bevyengine/bevy/pull/19647)
- [PR #13763 EnterSchedules/ExitSchedules](https://github.com/bevyengine/bevy/pull/13763)
- [Cheat Book Performance](https://bevy-cheatbook.github.io/setup/perf.html)
- [profiling.md Tracy/Chrome/Perf](https://github.com/bevyengine/bevy/blob/main/docs/profiling.md)

### Netcode
- [lightyear GitHub](https://github.com/cBournhonesque/lightyear)
- [lightyear_steam docs.rs](https://docs.rs/lightyear_steam/latest/lightyear_steam/)
- [bevy_replicon 0.40.1](https://github.com/projectharmonia/bevy_replicon)
- [bevy-steamworks 0.16](https://crates.io/crates/bevy-steamworks)

### Industrie roguelite
- [Hades dialogue Kasavin GDC 2021](https://www.gdcvault.com/play/1026975/Breathing-Life-into-Greek-Myth)
- [Diablo 3 Loot 2.0 Mosqueira GDC 2015](https://www.purediablo.com/josh-mosqueira-diablo-3-presentation-gdc-2015)
- [Dead Cells level design Deepnight](https://deepnight.net/tutorial/the-level-design-of-dead-cells-a-hybrid-approach/)
- [DRG Multiplayer wiki](https://deeprockgalactic.fandom.com/wiki/Multiplayer)
- [Slay the Spire RNG losing-seed](https://oohbleh.github.io/losing-seed/)

### Performance / Tests
- [cargo-nextest retries](https://nexte.st/docs/features/retries/)
- [proptest book](https://altsysrq.github.io/proptest-book/print.html)
- [Steam Machine Verified 30 FPS 1080p Wccftech](https://wccftech.com/steam-machine-verified-requirements-target-native-1080p-resolution-30-fps-gameplay/)
- [Steam Hardware Survey April 2026](https://store.steampowered.com/hwsurvey/videocard/)
- [wgpu MemoryBudgetThresholds](https://docs.rs/wgpu/latest/wgpu/struct.MemoryBudgetThresholds.html)

### Steam
- [Steam Cloud partner doc](https://partner.steamgames.com/doc/features/cloud)
- [ISteamUserStats](https://partner.steamgames.com/doc/api/isteamuserstats)
- [Steam refund 2026 update GameDeveloper](https://www.gamedeveloper.com/business/valve-updates-steam-refund-policy-to-cover-advanced-access-playtime)
- [Steam Deck Verified +2x sales digiexe](https://digiexe.com/blog/steam-statistics/)

### Industrie indé timeline / burnout
- [Katanaut solo 3 ans GamesRadar](https://www.gamesradar.com/games/roguelike/game-dev-hard-mode-discovered-as-solo-indie-releases-metroidvania-inspired-roguelike-right-between-silksong-and-hades-2-i-had-zero-awareness-that-they-were-dropping/)
- [Indie burnout 60% Wayline](https://www.wayline.io/blog/burnout-journaling-future-2025)
- [Octoverse 2025 AI productivity](https://github.blog/ai-and-ml/generative-ai/how-ai-is-reshaping-developer-choice-and-octoverse-data-proves-it/)
- [Zukowski Next Fest benchmarks](https://howtomarketagame.com/2025/03/26/benchmarks-how-many-wishlists-can-i-get-from-steam-next-fest/)
- [Ziva Next Fest 2026 +806 wishlists median](https://ziva.sh/blogs/steam-next-fest-2026)

### Content pipeline
- [Meshy commercial license](https://help.meshy.ai/en/articles/9992001-can-i-use-my-generated-assets-for-commercial-projects)
- [ElevenLabs pricing 2026](https://elevenlabs.io/pricing)
- [bevy_kira_audio CHANGELOG 0.25](https://github.com/NiklasEi/bevy_kira_audio/blob/main/CHANGELOG.md)
- [bevy_fluent GitHub](https://github.com/kgv/bevy_fluent)
- [Game Accessibility Guidelines](https://gameaccessibilityguidelines.com/)

---

*Document audit indépendant 5 agents parallèles. Source de vérité pour corrections story-468 + ROADMAP V7. Validité 2026-05-19.*
