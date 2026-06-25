# Plan recommandé — RPG Forgia avec QA/debug intégré (boucle de vérification)

> Date : 2026-06-24. Suite du décorticage WoC ([woc-architecture-decortication](../audit/woc-architecture-decortication-2026-06-24.md)).
> Établi après cartographie **de l'existant QA** (4 agents). Statut : PROPOSITION à valider.
>
> **Recommandation en une phrase** : ne pas *construire* un système QA — **allumer celui qui existe
> déjà (~70 % fait mais débranché)** et poser la keystone déterministe (`FixedUpdate`), pour que
> chaque feature RPG suivante soit livrée *prouvée* par une boucle test+sensor+bot.

---

## 0. État des lieux — ce qui existe DÉJÀ (et qu'on ne reconstruit pas)

L'affirmation « pas de tests » de `CLAUDE.md`/`build-stack.md` est **périmée**. Réalité mesurée :

| Brique QA/debug | Statut réel | Détail |
|---|---|---|
| Tests unitaires | ✅ **~450-500 `#[test]`** sur ~50 crates | roguelite 143, terrain 133, stage 99, observability 90, combat 37, rpg-data 41, fps 34 — headless purs |
| `forgia-observability` | ✅ **~70 sensors 1 Hz** | schéma `{id,severity,next_step,timestamp}`, `checks.rs` (6 cross-checks), `SENSOR_REGISTRY.md` |
| `forgia-debug` | ✅ overlay **F3** + console dev | `:teleport :heal :god :dump_sensors :spawn`, file-poll découplé |
| `forgia-qa-core` | 🟡 **réel mais NO-OP** | `BugReport`/`BugBus`/`Severity` (42 tests), câblé au binaire **mais feature `qa-runtime` jamais activée → émission morte** |
| `forgia-qa-harness` | 🟡 **réel, 0 consommateur CI** | `TestApp` builder + `CollectingBugSink` + golden frames (37 tests), personne ne l'appelle |
| `forgia-qa-autopilot` | 🟡 **réel, jamais lancé** | `Bot`/`SmokeBot`/`SoakBot`/`BotRunner` (soak tests OK), absent de la CI |
| `forgia-qa-replay` | 🔴 **cassé** | keyboard-only, pas de keybind, `DefaultHasher` non-déterministe, `forgia_repro` inexistant — **ADR-0004 PROPOSED depuis 2026-06-10, non tranché** |
| xtask gates | ✅ 7 vivants | `story-gate no-scaffold arch-drift sensor-audit asset-load story-ids verify-sensors-format` (+ 3 stubs) |
| CI | ✅ check/clippy(-D warnings)/test/fmt/asset-load | **mais 4/5 gates pre-push absents de la CI** (bypassables `--no-verify`) |
| `forgia-rng` | ✅ xoshiro256++ déterministe | utilisé en **procgen seulement**, pas dans le combat |
| **`FixedUpdate`** | 🔴 **ZÉRO occurrence** | tout le gameplay tourne en `Update` (variable) ; Rapier en pas variable |

**Conclusion** : la machine QA est construite mais le moteur n'est pas branché. Le travail à fort
levier n'est pas « créer », c'est **activer, câbler, fermer la boucle** — puis la rendre *fiable* via
le déterminisme. C'est aussi ce qui respecte `no-speculative-fix` (ne pas réécrire ce qui marche).

---

## 1. Le système QA/debug cible — 5 couches, chacune mappée sur une crate existante

```
┌─ Couche 5 — RUNTIME DEBUG + BUG BUS ────────────────────────────────────────┐
│ forgia-observability (70 sensors) + forgia-debug (F3) + forgia-qa-core       │
│ (BugReport activé) + health aggregator → "regarde" = diagnostic en 1 lecture │
├─ Couche 4 — SCÉNARIO / SOAK (bot-driven) ───────────────────────────────────┤
│ forgia-qa-autopilot SmokeBot/SoakBot lancés en CI sur scénarios roguelite+RPG│
├─ Couche 3 — INTÉGRATION SYSTÈME (headless Bevy) ────────────────────────────┤
│ forgia-qa-harness TestApp : boot app minimale, inject, tick, assert (CI)     │
├─ Couche 2 — UNITAIRE + DÉTERMINISME (headless rapide) ──────────────────────┤
│ ~450 #[test] existants + tests run()==run() (rendus possibles par FixedUpdate)│
├─ Couche 1 — GATES STATIQUES (compile/CI) ───────────────────────────────────┤
│ xtask : 7 existants + validate-genomes + check-deps + stubs activés (tous CI)│
└─────────────────────────────────────────────────────────────────────────────┘
```

**La keystone transverse** : un **`FixedUpdate` 20 Hz** pour la logique gameplay + discipline de seed.
Sans lui, couches 2-4 (déterminisme, replay, autopilot reproductible) sont du théâtre. C'est la leçon
#1 de WoC (cœur déterministe headless) appliquée à l'ECS Bevy de Forgia.

### Le « contrat de vérification » — Definition of Done par story
Chaque story livrée doit cocher (c'est ÇA, le QA intégré) :
1. **Test unitaire** headless (logique pure) — couche 2
2. **Test `TestApp`** (le système dans un vrai schedule) — couche 3
3. **Sensor** `forgia2_<feature>.json` avec `severity` + `next_step` actionnable — couche 5
4. **Scénario bot** OU **test déterminisme** `run()==run()` quand applicable — couches 2/4
5. **Gates xtask verts** (`validate-genomes`, `story-gate`, …) — couche 1
6. **Auto-QA** sous-agents `verifier` + `qa-lead` avant DONE (règle existante `post-impl-auto-qa`)
7. **Récap test runtime** (règle existante `in-game-test-recap`)

---

## 2. Phase 0 — ALLUMER la machine QA (fondation, aide le ship, priorité 1)

> But : passer de « 70 % construit, débranché » à « boucle de vérification vivante ». Sert
> directement le **ship roguelite** (régression détectée automatiquement sur ce qu'on shippe).

| # | Story proposée | Quoi | Effort | Risk |
|---|---|---|---|---|
| **0.1** | `FixedUpdate` gameplay 20 Hz + discipline seed | Introduire `Time::<Fixed>::from_hz(20.)` ; migrer la logique combat/roguelite tick vers `FixedUpdate` ; tuer le fallback non-déterministe `default_seed_from_clock` (`roguelite/run.rs:845`) → toujours seeder + logguer ; lint anti-`thread_rng` en gameplay | ~1 sem | **High** |
| **0.2** | Activer `qa-core` (`qa-runtime`) | Décider le switch (profil `dev`/`qa` on, release off) ; `BugReport`/`BugBus` émettent réellement ; `FileSink` → `forgia2_bugs.json` | ~1-2 j | Low |
| **0.3** | Câbler `qa-harness` en CI | Crate `tests/` (ou job) qui boote `TestApp::new().build()`, tick, assert sur events/sensors — **1er consommateur réel** ; 3-4 tests RPG d'amorçage | ~2-3 j | Low |
| **0.4** | Câbler `qa-autopilot` en CI | Lancer `SmokeBot` (court) + `SoakBot` (entity-drift) sur un run roguelite headless en CI (ou nightly si trop lent) | ~2-3 j | Medium |
| **0.5** | `xtask validate-genomes` + gates en CI | serde-désérialise tous les TOML `assets/genomes/**` + cross-check IDs (élément→arme, boon→élément, loot→item, quête→mob) ; **ajouter à la CI les 4 gates pre-push manquants** (forward-ratchet) | ~2 j | Low |
| **0.6** | Hygiène sensors | Dédupliquer les writers (`forgia2_stage.json` ×3, `forgia_prefab.json` ×2) ; enregistrer les 7 sensors orphelins ; étendre `verify-sensors-format` (13→couverture large) ; **rendre `forgia2_health.json` actif en Roguelite** (aujourd'hui RPG-gated → aveugle sur le jeu qu'on ship) | ~2-3 j | Low |
| **0.7** | Trancher ADR-0004 (replay) | Soit **réparer** (hasher déterministe — dépend de 0.1 —, capture souris, keybind, binaire `forgia_repro`), soit **remiser proprement** (retirer le plugin mort du binaire). Ne pas laisser un plugin cassé câblé | ~3-4 j si fix | Medium |

**Ordre Phase 0** : 0.5 + 0.6 d'abord (Low risk, gains immédiats, indépendants) ‖ 0.2/0.3 (activation) →
**0.1 (la keystone)** → 0.4 puis 0.7 (dépendent du déterminisme de 0.1).

> Note risk 0.1 : c'est le seul morceau High de la fondation. Faire un **spike isolé** d'abord (un
> seul système combat en `FixedUpdate`, mesurer la stabilité Rapier/feel), valider, puis généraliser.
> Le `feel:smoke` (via `TestApp`) garde le ressenti FPS pendant la migration.

---

## 3. Phase 1 — Profondeur combat RPG (chaque feature passe le contrat §1.DoD)

> Débloquée par Phase 0 : maintenant chaque livraison sort *prouvée*.

| # | Story | Quoi | Vérification intégrée | Effort | Risk |
|---|---|---|---|---|---|
| 1.1 | Unifier dual-Health | Fusionner `forgia_combat::Health` / `forgia_damage::Health` (dette M4 connue) | test régression 2 chemins + `TestApp` | ~3-4 j | Medium |
| 1.2 | Moteur auras-tick | DoT/HoT/buff/debuff générique en `FixedUpdate` (`tick_timer`/`tick_interval`/`breaks_on_damage`) | test `run()==run()` + `forgia2_auras.json` + `TestApp` | ~3-4 j | Medium |
| 1.3 | Threat/aggro + GCD + recalc dirty-flag | table threat sur mob (switch 110/130 %), taunt, GCD champ composant, `StatsNeedRecalc` (jamais hot path) | `forgia2_threat.json` + `SmokeBot` « tank tient l'aggro » | ~4-5 j | Medium |

---

## 4. Phase 2 — Systèmes de contenu RPG

| # | Story | Quoi | Vérification | Effort | Risk |
|---|---|---|---|---|---|
| 2.1 | Quêtes depuis TOML | Remplacer `register_sample_quests` hardcodé (`forgia-rpg/lib.rs:181`) par catalogue genome + `roll_group`/`requires_quest`/`quest_order` | `validate-genomes` (cross-refs) + `TestApp` complétion quête | ~3 j | Medium |
| 2.2 | Équipement / gear slots | Crate `forgia-equipment` (absente) : slots tête/torse/arme/anneau au-dessus de l'inventaire 80-slots | tests + extension `forgia2_inventory.json` | ~1 sem | Medium |
| 2.3 | Audit balance contenu | `xtask audit-genomes` : DPS ±X %, bornes cooldown, ratios, cross-refs → rapport HTML/JSON (modèle `quest_audit_graph` WoC) | gate CI optionnel | ~1-2 j | Low |

---

## 5. Phase 3 — Scale & polish (track FORGE long terme)

| # | Story | Quoi | Dépend de | Risk |
|---|---|---|---|---|
| 3.1 | RL balance-bot | `cargo run --bin forgia-env` (NDJSON stdio, `RewardCounters`) — playtest balance automatisé infini | 0.1 + 0.4 | Medium |
| 3.2 | i18n « sim-emits-keys » | sim émet des clés, UI relocalise ; overlay TOML sparse + `t!` proc-macro + pseudo-locale `en_XA` ; garde CI parse des émissions | — | Medium |
| 3.3 | `lightyear` autoritatif | interest mgmt 90 yd, split identity/dynamic, delta `Changed<>`, intentions client | 0.1 (FixedUpdate) | **High** |
| 3.4 | Polish procédural | météo biome (hanabi, render-only), VFX élément→couleur HDR-bloom, icônes CPU (`image`) | — | Low |

---

## 6. Séquencement global recommandé

```
SHIP-aligné (fais maintenant) ─────────────────────────────────────────────
  Phase 0.5 + 0.6  (gates + sensors, Low, immédiat)
        │
        ├─ 0.2 + 0.3  (activer qa-core + harness en CI)
        │
        └─ 0.1  KEYSTONE FixedUpdate+seed (spike d'abord)  ──┐
                                                              ├─ 0.4 autopilot CI
                                                              └─ 0.7 replay (fix/remise)
FORGE / RPG (ensuite, chaque story = contrat DoD §1) ──────────────────────
  Phase 1 (combat) → Phase 2 (contenu) → Phase 3 (scale)
  1.x dépendent de 0.1 (FixedUpdate) ; 2.x indépendants ; 3.1/3.3 dépendent de 0.1
```

Cadrage vision (2026-06-04) : **Phase 0 sert le ship** (régression auto sur le roguelite). Phases 1-3
sont track FORGE — autorisées au rythme où elles n'éclipsent pas le ship.

---

## 7. Pourquoi ce plan plutôt qu'un autre (justification)
- **Respecte l'existant** (`no-speculative-fix`) : 0 réécriture de ce qui marche ; on branche.
- **La keystone d'abord, mais isolée** : `FixedUpdate` (0.1) est le seul High risk de la fondation et
  conditionne replay/autopilot/RL ; on le fait en spike validé, pas en big-bang.
- **QA intégré = contrat DoD**, pas une phase finale : impossible de marquer DONE sans test+sensor+gate.
- **Sert le ship** : tout Phase 0 rend le roguelite (ce qu'on ship) automatiquement testé/observé.
- **Mécaniquement gardé** : `story-gate` + `validate-genomes` + auto-QA empêchent le DONE-fictif (dette
  historique du repo, cf `story-done-gate.md`).

---

## 8. Risques & garde-fous
| Risque | Garde-fou |
|---|---|
| `FixedUpdate` casse le feel FPS / Rapier | Spike isolé + `feel:smoke` via TestApp avant généralisation |
| Activer `qa-runtime` ralentit le runtime | Feature off en release ; profil `qa`/`dev` seulement |
| CI trop lente (autopilot/soak) | SmokeBot en PR, SoakBot en nightly |
| Scope creep RPG vs ship | Phase 0 seule est « obligatoire ship » ; le reste est gated par la vision |
| Migration seed casse la reproductibilité procgen | `forgia-rng` déjà déterministe ; corriger d'abord modulo-bias (audit 2026-06-10) |

---

## 9. Prochain pas concret proposé
Écrire **story-0.5 `xtask validate-genomes`** (le quick-win Low risk qui sert le ship et amorce la
couche 1) OU le **spike story-0.1 FixedUpdate** (la keystone). Recommandation : commencer par 0.5+0.6
(gains sûrs, zéro risk) en parallèle du spike 0.1.

*Sources : décorticage WoC + audit existant QA (forgia-qa-*, observability, xtask, CI, déterminisme),
digests archivés dans le transcript de session 2026-06-24.*
