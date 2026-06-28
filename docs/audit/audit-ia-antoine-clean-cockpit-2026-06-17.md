All critique claims verified on disk. The corrections are confirmed: `veille-query.ps1` EXISTS (6.4K), V2 `.claude/` is a near-empty stub (only 3 rules + 1312-byte settings.local.json, no agents/commands/hooks/CLAUDE.md inside), V2 has NO tools/, NO docs/registry/, NO LEXIQUE_ECS.md, NO .mcp.json, NO .grepai index, 62 crates + src/main.rs, sensors = 65 forgia2_* + 48 forgia_* (113), and debug_monitor.toml does NOT exist (V2 has rpg_monitor.toml/debug_anim.toml).

Now writing the final report.

```markdown
# Audit — Cockpit IA propre dans `D:/IA Antoine` (2026-06-17)

## 1. Verdict (c'est possible ? recommandation en 5 lignes)

**GO conditionnel.** Le diagnostic est solide et **toutes les dérives V1 sont vérifiées sur disque** : `a.becker` (mauvais user), zombie BMAD story-384, grepai→V1, `RUST_DIR` mort dans validate-commit.sh, MEMORY 700 lignes, concept-first-gate qui ne matche que la structure V1 à 3 crates. Le découpage **core universel + overlay projet** est la bonne idée. **MAIS** n'exécute PAS le manifeste tel quel : il contient des erreurs factuelles (corrigées en §3/§4) et **sur-conçoit pour un solo à 1 seul projet**. Recommandation : ship d'abord les **4 victoires P0** (grepai→V2+index, tuer le zombie BMAD, fix `a.becker`, trim MEMORY) — 80% du gain d'intelligence sans risque structurel — puis le split de fichiers, et **diffère** capabilities.json / eval-harness / sensor-truth-map / BOOTSTRAP junctions jusqu'à ce qu'un 2e projet existe réellement.

---

## 2. Structure cible (couches : core universel + projects/forgia)

```
D:/IA Antoine/
├── README.md                          # "core = universel, projects/ = overlays"
├── BOOTSTRAP.md                        # [DIFFÉRÉ vague 3] junctions + env vars
│
├── core/                               # ===== UNIVERSEL (réutilisable tout projet) =====
│   ├── CLAUDE.md                       # contrat IA GÉNÉRIQUE — ZÉRO Forgia/Bevy/Lock/story
│   ├── .claude/
│   │   ├── rules/                      # gouvernance comportementale auto-chargée (corps court)
│   │   │   ├── ask-when-unclear.md  no-speculative-fix.md  post-impl-auto-qa.md
│   │   │   ├── security-anti-injection.md  observability-required.md (généralisé)
│   │   │   ├── runtime-test-recap.md (ex in-game)  multi-terminal-coordination.md
│   │   │   ├── model-selection.md  bug-triage.md
│   │   │   └── on-demand/concept-first.md   # 13K — squelette hors auto-load (tableau §6 → overlay)
│   │   ├── agents/                     # planner, implementer, verifier, qa-lead (Locks externalisés)
│   │   ├── commands/                   # research, plan, implement, verify, audit, batch-workers, verify-stories
│   │   ├── hooks/                      # déterministes, paramétrés (lib/python-path.sh + discover-paths.sh)
│   │   ├── settings.json               # deny dangereux + allow universels (cargo/git) + Stop-hook build+clippy
│   │   ├── shared-context.json  docs/
│   ├── docs/{best-practices,audit-procedures,raw-ingests}/ + log.md
│   └── production/session-state/       # runtime multi-terminal (reset à vide)
│
└── projects/
    └── forgia/                         # ===== OVERLAY (Forgia-spécifique, grepai → V2) =====
        ├── CLAUDE.md                   # base = V2 root CLAUDE.md (propre) + Locks/vision/BMAD
        ├── .mcp.json                   # forgia (rebuild V2) + grepai → "Forgia Rewrite" + index à BUILD
        ├── .claude/
        │   ├── rules/                  # build-stack, combat/terrain/player/ui/genome-code, no-hardcode,
        │   │   ├── data-driven-paths (réécrit V2)  scalability  creator-simplicity  boost-protocol
        │   │   ├── concept-first-table.md   # ← tableau §6 (water/combat/biome + sensors)
        │   │   └── fine-grained-crates  session-checkpoint  story-done-gate   # ← V2-natifs déjà présents
        │   ├── agents/                 # bevy/terrain/perf-analyst, game-maker, economy + forgia-log-patterns.md
        │   ├── commands/               # gdd, playtest, bug-loop, veille, veille-pipeline
        │   ├── hooks/                  # concept-first-gate (pattern V2), validate-commit, bmad-utils… ($FORGIA_ROOT)
        │   ├── settings.local.json     # base = V2 (1.3K propre) + allow Forgia utiles, ZÉRO a.becker
        │   └── state/bmad-task.json    # reset {} (plus story-384)
        ├── docs/{design,registry(À CRÉER),adr,stories,veille,thoughts}/ + ROADMAP_*.md
        ├── memory/{MEMORY.md ≤200l, topics/, archive/}   # ⚠️ vit dans ~/.claude → JUNCTION
        └── tools/forgia-mcp/           # À CRÉER + rebuild V2 (n'existe qu'en V1)
```

---

## 3. Manifeste de migration (corrigé par les vérifications disque)

Priorités : **P0** = bloquant intelligence/dérive · **P1** = important · **P2** = confort/différable.

### 3.1 — Règles → `core/.claude/rules/`

| Source (`D:/Forgia/.claude/rules/`) | Destination | Action | Fix | P |
|---|---|---|---|---|
| ask-when-unclear, no-speculative-fix, post-impl-auto-qa, security-anti-injection | core/ | copy | — | P1 |
| observability-required.md | core/ | copy+fix | Généraliser : `forgia_<x>.json`/`debug_monitor.toml`/`diagnostic_report.rs` → pattern abstrait "sensor JSON + toggles config + health check". Forgia = exemple | P1 |
| in-game-test-recap.md | core/**runtime-test-recap.md** | copy+fix | Renommer in-game→runtime. 5 éléments gardés. Shift+F12/TOML = exemples | P1 |
| multi-terminal-coordination.md | core/ | copy+fix | `forgia.exe`/`release-fast`/`forgia-mode-roguelite` → binaire générique via discover-paths. Garder règle mtime(source≤binaire≤sensor) | P1 |
| concept-first.md (13K) | core/**on-demand/** + overlay | copy+fix+**SCINDER** | **Le tableau §6 = ~50% du fichier et 100% Forgia → extraction OBLIGATOIRE** vers `projects/forgia/.claude/rules/concept-first-table.md`. Squelette protocole en core. §7 cross-refs `audit-protocol.md`/`engineering-directive.md` **n'existent PAS en V1** → retirer les renvois | **P0** |
| bug-triage.md | core/ | copy+fix | Lignes 61/125/126 : `RUST/.../CLAUDE.md` + `audit-protocol.md` + `engineering-directive.md` (inexistants) → re-pointer core CLAUDE.md / **supprimer renvois morts** | P1 |
| model-selection.md | core/ | copy+fix | Opus 4.7→4.8. **Retirer la baseline coût $1194 (fuite Forgia)** | P2 |

### 3.2 — Règles → `projects/forgia/.claude/rules/`

| Source | Action | Fix | P |
|---|---|---|---|
| build-stack.md | copy+fix | **Décrit V1 (3 crates ~243 .rs). V2 = 62 crates `crates/forgia-*` + `src/main.rs`.** Réécrire inventaire + cargo -p | P1 |
| combat-code, genome-code | copy+fix | Re-pointer `crates/forgia-combat`/`forgia-damage`/`forgia-fps` ; `catalogue.rs`/`genome_sync.rs` → `crates/forgia-genome-core`. **`debug_monitor.toml` n'existe pas en V2 → `rpg_monitor.toml`/`debug_anim.toml`** | P1 |
| data-driven-paths.md | **rewrite** | Front-matter pointe `forgia-game/src/combat/**` + `forgia-terrain/src/**` (V1 morts). **Réécrire en glob couvrant `crates/forgia-*/**/*.rs` ET `src/main.rs`** (pas crates-only) | **P0** |
| terrain-code, player-code, ui-code | copy+fix | `forgia-terrain`→`forgia-foliage`/`forgia-procgen-graph` ; `forgia-game/src/player`→`crates/forgia-player` ; vérifier `forgia-enemy-nameplate`. **Confirmer existence types V2** | P2 |
| editor-code.md | copy+fix | Marquer "Phase 2 différée" (Build/Edit) | P2 |
| no-hardcode, scalability, creator-simplicity, boost-protocol | copy(+fix) | no-hardcode : vérifier paths V2. boost : reset tracking phases | P2 |
| **(V2-natifs, déjà propres)** fine-grained-crates, session-checkpoint, story-done-gate | copy | Pris depuis `C:/.../Forgia Rewrite/.claude/rules/` tel quel | P1 |

### 3.3 — Agents & Commandes

| Source | Destination | Action | Fix | P |
|---|---|---|---|---|
| planner.md | core/agents | copy | — | P1 |
| implementer.md | core/agents | copy+fix | `cargo check` en dur → `$BUILD_CHECK_CMD` (défaut cargo check) | P2 |
| verifier.md | core/agents | copy+fix | Externaliser L1-L8 → lit `projects/<projet>/docs/registry/`. ⚠️ **ce dir n'existe pas en V2, à CRÉER** sinon ref vide | P1 |
| qa-lead.md | core/agents | copy+fix | Patterns logs Forgia (SpawnSnap…) → `projects/forgia/.claude/agents/forgia-log-patterns.md`. ⚠️ interim = fuite Forgia en core tant que l'overlay n'est pas écrit | P2 |
| bevy/terrain/perf-analyst, game-maker, economy | projects/forgia/agents | copy(+fix) | perf-analyst : re-lister les **113 sensors V2 réels** (65 forgia2_* + 48 forgia_*). game-maker : documenter quand l'invoquer (sinon drop futur) | P1/P2 |
| research, plan, implement, batch-workers | core/commands | copy | — | P1/P2 |
| forgia-verify.md | core/**verify.md** | rewrite | Externaliser Locks → registry overlay. Compile+clippy+scope-drift+hardcode génériques | P1 |
| ia-audit.md | core/**audit.md** | rewrite | Path `D:/Forgia/RUST/.../src/` (mort) → workspace découvert depuis git-root/cwd | **P0** |
| verify-stories.md | core/commands | rewrite | `RUST/.../docs/stories/` → `$STORIES_DIR` (défaut docs/stories/) | P1 |
| gdd, playtest, bug-loop | projects/forgia/commands | copy+fix | playtest : sensors V2 confirmés. bug-loop : `tools/run-game-log.bat` **n'existe pas en V2 (pas de tools/)** → créer ou repointer | P2 |
| ia-veille.md | projects/forgia/**veille-pipeline.md** | copy+fix | Renommer. Dépend context7 (présent) | P2 |
| veille.md | projects/forgia/commands | copy+fix | **CORRECTION : `scripts/veille/veille-query.ps1` EXISTE (6.4K). Pas de travail "créer le script"** — juste repointer le chemin | P2 |
| analyze-patterns.md | **drop** | drop | Jamais invoqué, MemPalace sans intégration claire, YAGNI | P2 |

### 3.4 — Hooks

| Source | Destination | Action | Fix | P |
|---|---|---|---|---|
| python-path.sh, anti-injection-scan.sh | core/hooks(/lib) | copy | Bon design | P1 |
| session-start.sh (20K) | core/hooks | rewrite | `RUST_DIR`/`STORIES_DIR` → discover-paths + `$PROJECT_ROOT`. **`MEMORY_DIR=C:/Users/a.becker/…` l.355 → ~/.claude**. python en dur l.355 → source python-path.sh | **P0** |
| validate-commit.sh (32K) | projects/forgia/hooks | rewrite | `RUST_DIR=D:/Forgia/RUST/…` l.38, whitelists V1, `PANIC_FILE` l.423, `ASSETS_RS` l.579 → discover-paths-from-repo (git root, docs/stories, config/genomes) | **P0** |
| concept-first-gate.sh | projects/forgia/hooks | copy+fix | Pattern `forgia-game/src/` l.41 (V1) → **glob V2 incluant `crates/forgia-*` ET `src/main.rs`**. `STATE_FILE` l.49 → ~/.claude/state/ | **P0** |
| pre-compact.sh | core/hooks | copy+fix | `MEMORY_DIR=C:/Users/a.becker/…` l.9 → ~/.claude. + lint MEMORY (warn >200 lignes / entrée >200 chars) | **P0** |
| bmad-enforce.sh | **drop** | drop | **ZOMBIE : injecte story-384 V1 à chaque prompt, `STORIES_DIR` l.8 mort. NE PAS migrer.** Remplacer par skill `/bmad-detect` optionnel ou warn passif | **P0** |
| .bmad-task.json `{scale:enterprise,story_id:story-384,edits:5858}` | projects/forgia/state/bmad-task.json | rewrite | **Reset `{}`** (source du zombie) | **P0** |
| learn-from-edit.py, learn-from-action.py | core/hooks | copy+fix | **Graceful degrade si mempalace down → fallback `~/.claude/hooks/learning.log`** (pas fail silencieux) | P1 |
| mempal-health.py | core/hooks | copy+fix | `PALACE_DIR='D:/Mémoire claude/mempalace'` l.13 → `$MEMPALACE_PALACE_PATH`. Unicode Windows + timeouts robustes | P1 |
| bmad-utils.sh | projects/forgia/hooks | copy+fix | `STATE_FILE` l.5 + `STORIES_DIR` l.6 → env + discover-from-cwd | P1 |
| concept-first-track/-reset, post-edit-check, detect-diagnostic, acon-error-capture, validate-assets, claim-task, session-start-veille | mix core/overlay | copy+fix | Paramétrer STATE_FILE/sensors/VEILLE_DIR. ⚠️ **plusieurs non lus à l'audit — paramétrer un hook non audité = cérémonie ; auditer d'abord** | P2 |
| multi-terminal-check, session-lock, terminal-register, context-show/update, post-session-causal, mempal-*, _perf-* | core/hooks | copy(+fix) | Paramétrer chemins état | P2 |

### 3.5 — Settings, MCP, CLAUDE.md, Mémoire, Docs

| Source | Destination | Action | Fix | P |
|---|---|---|---|---|
| `.mcp.json` | **SCINDER** core + overlay | rewrite | core = ollama + mempalace + context7. overlay = forgia (rebuild) + **grepai args `C:/Users/Antoi/Desktop/Forgia Rewrite`**. ⚠️ **`.grepai/` ABSENT en V2 → l'index doit être CONSTRUIT, pas juste repointé** | **P0** |
| settings.json | core/ | copy+fix | Garder deny V1 (bloque legacy = correct). Ajouter Stop-hook build+clippy. Corriger hooks a.becker | **P0** |
| settings.local.json (V1, 42K/582l) | **drop** | drop | python a.becker l.477/530/544, 400+ allow Forgia, typos `d://d/`, hooks redondants | **P0** |
| **settings.local.json (V2, 1.3K)** | projects/forgia/ | copy+fix | **Base de l'overlay** (0 a.becker, 0 RUST). ⚠️ **CORRECTION : ce n'est PAS une allow-list Forgia "propre" — ~30 lignes DEBUG/timing référençant des hooks D:/Forgia. C'est un stub, pas une base riche.** Fusionner allow Forgia utiles du V1 | **P0** |
| **CLAUDE.md (V2 root, 5.9K propre)** | projects/forgia/CLAUDE.md | copy+fix | **Bonne base overlay** (PAS le V1 237l) — vision Roguelite, Locks, BMAD | **P0** |
| CLAUDE.md (V1, 237l) | core/CLAUDE.md | rewrite | Extraire UNIQUEMENT universel (§2 rôle, §3 comportement+qualité, §6 absolues, §10 parallélisme, §11 Mémorise). Supprimer tout Forgia | **P0** |
| MEMORY.md (700l) | projects/forgia/memory/ (**junction**) | copy+fix | Trim ≤200l. Archiver <2026-05-01. Tagger `[V1-ORIGIN]` (~20%). **Vit dans ~/.claude où les hooks écrivent → JUNCTION, pas déplacement** | P1 |
| topics + archive (852K) | projects/forgia/memory/ | copy+fix | Tagger `[V1-ORIGIN]` (story-471-479, L1 136-handles) → `archive/v1-reference.md` | P2 |
| LEXIQUE_ECS.md | projects/forgia/ | copy | ⚠️ **CORRECTION : n'existe PAS en V2.** Seul V1 l'a (2026-03-08). À rapatrier de V1 + revalider contre les 62 crates, OU régénérer | P2 |
| tr-registry.yaml / architecture.yaml (V1) | projects/forgia/docs/registry/ | **CRÉER, ne PAS migrer** | ⚠️ **V2 n'a AUCUN docs/registry/. L1=136 handles est une vérité V1 — l'importer = injecter du faux. Baseline V2 à ÉTABLIR** (L1 69→50 handles, story-528 xtask check-orphans) | P1 |
| tools/forgia-mcp/ | projects/forgia/tools/ | **CRÉER + rebuild** | ⚠️ **V2 n'a PAS de tools/. L'exe n'existe qu'en V1.** Créer le dir, rebuild cwd Forgia Rewrite, repointer .mcp.json | P1 |
| .bmad/ (junction → V1) | projects/forgia/.bmad/ | copy+fix | Junction pointe V1. Re-pointer config.yaml stories/checklists V2. Découpler du zombie supprimé | P1 |
| docs best-practices/audit/raw/log + .claude/docs/ + session-state | core/docs + production | copy(+fix) | session-state reset à vide | P2 |
| docs/veille/, thoughts/, design/, stories/, ROADMAP_* | projects/forgia/docs/ | copy | Contenu Forgia → overlay | P2 |
| questionnaire-validation, coaching, SECURITY_ACTION_REQUIRED, hyworld-audit, captured/, *.bak | **drop** ×6 | drop | Marketing / obsolète / mort / vide / backups (git suffit) | P2 |

---

## 4. Réparations obligatoires en transit (dé-V1 — ne jamais migrer la dérive)

1. **grepai → V2 (le levier #1).** Args = `C:/Users/Antoi/Desktop/Forgia Rewrite` au lieu de `D:/Forgia/RUST/Forgia/Forgia`. **`.grepai/` est ABSENT en V2 → étape de build d'index OBLIGATOIRE et non budgétée dans le manifeste.** Sans ça, Explore/Grep rapportent des chemins V1 morts à chaque session.
2. **`a.becker` → détection dynamique.** 5 sites confirmés : pre-compact.sh:9, session-start.sh:355, settings.local.json:477/530/544 (+ .bak dropés). Remplacer par `source core/.claude/hooks/lib/python-path.sh` ou `~`. NB : `.mcp.json` mempalace utilise déjà le bon Python — **ne pas y toucher**.
3. **Zombie BMAD — SUPPRIMER.** bmad-enforce.sh (story-384 injectée à chaque UserPromptSubmit, STORIES_DIR l.8 mort) + reset `.bmad-task.json` `{}`. Jamais une injection systématique — un skill optionnel ou warn passif.
4. **Deny-list : conserver le blocage V1** (`RUST/Forgia/Forgia`) — c'est correct, ça empêche l'IA de toucher le legacy. **Ajouter** un Stop-hook bloquant build-check+clippy avant "fait". Retirer les chemins a.becker des hooks.
5. **MEMORY trim 700→≤200.** Le système-reminder lui-même warn ("696 lignes/124KB"). Archiver <2026-05-01, tagger `[V1-ORIGIN]` les ~20% V1. Étendre pre-compact.sh pour linter. **Action ROI #1, à ship indépendamment du reste.**
6. **discover-paths.sh** remplace `RUST_DIR`/`STORIES_DIR`/`ASSETS_RS`/`PANIC_FILE` en dur (validate-commit.sh 32K, session-start.sh 20K). Whitelists relatives au repo.
7. **Patterns V2 corrects.** data-driven-paths.md + concept-first-gate.sh : `forgia-game/src/**` (V1 à 3 crates) → glob couvrant **`crates/forgia-*/**/*.rs` ET `src/main.rs`** (V2 = 62 crates + un binaire racine). Un glob crates-only **rate `src/main.rs`**.
8. **Doublons V2 propres à PRÉFÉRER** : CLAUDE.md V2 root (5.9K) > V1 (237l) ; settings.local.json V2 (1.3K) > V1 (42K). **MAIS** le `.claude/` V2 est un **stub quasi-vide** (3 règles, settings.local stub, **0 agent / 0 commande / 0 hook / pas de CLAUDE.md interne**) — l'overlay se **construit**, il ne s'"hérite" pas d'un V2 déjà riche.
9. **Cross-refs morts.** `audit-protocol.md` et `engineering-directive.md` (cités dans concept-first §7, bug-triage l.61/125/126) **n'existent PAS dans `D:/Forgia/.claude/rules/`** (V1-only, jamais copiés). Soit les rapatrier de V1, soit **retirer les renvois**.
10. **Corrections factuelles confirmées disque** : `veille-query.ps1` **existe** (pas de "créer le script") ; V2 = **pas de tools/, pas de docs/registry/, pas de LEXIQUE_ECS.md, pas de .mcp.json, pas de .grepai, pas de debug_monitor.toml** (V2 = rpg_monitor.toml/debug_anim.toml) ; sensors V2 = **113 (65 forgia2_* + 48 forgia_*)**, pas seulement forgia2_*.

---

## 5. Ce qui rend l'IA plus intelligente (les vrais leviers, pas une UI)

**Classés par ROI réel pour un solo zéro-budget — pas tous égaux.**

**À FAIRE MAINTENANT (load-bearing) :**
- **grepai → V2 + build d'index.** Le plus gros gain d'ancrage. Aujourd'hui Explore/Grep retournent des chemins V1 morts. P0.
- **MEMORY à récence bornée + dédup.** Le levier #1 anti context-rot. Les entrées >30j et `[V1-ORIGIN]` sortent de l'auto-load → ce que l'IA charge est TOUJOURS le contexte V2 actif.
- **Graceful degradation MCP.** learn-from-edit/action.py + mempal-health dépendent de mempalace/ChromaDB (timeout 3s). En zéro-budget/local, les serveurs ne tournent pas toujours → fallback log `~/.claude/hooks/learning.log` pour continuer d'apprendre MCP éteint.

**JUSTIFIÉ mais à séquencer après le split :**
- **capabilities.json** (62 crates + 113 sensors + commandes build/run, machine-lisable). J'ai dû énumérer crates+sensors à la main pour cet audit — un manifeste à jour = l'IA arrive ancrée. **Mais doit être GÉNÉRÉ par scan, pas écrit à la main** (sinon il dérive comme le reste).
- **sensor-truth-map généré** sur les **deux préfixes** (`forgia2_*` ET `forgia_*` — la richesse réelle, le plan ne modélisait que forgia2_*) → table `sensor → champs → feature` réinjectée dans concept-first-table.md + performance-analyst.
- **Applicateur de staleness** sur le tableau §6 concept→fichier:ligne : un check qui marque `stale` toute ligne dont le path n'existe plus en V2. La règle le demande déjà mais **rien ne l'applique** → il dérive (les sensors forgia2_* supposés sont déjà à confirmer).

**NICE-TO-HAVE, NE PAS sur-investir (YAGNI solo 1 projet) :**
- Eval harness via ollama (juge local) : utile mais **pas load-bearing**, différer.
- BOOTSTRAP.md junctions exécutable : n'a de valeur **qu'au 2e projet**, qui n'existe pas. Différer.
- Registre ADR hors CLAUDE.md (Build/Edit différé 2026-06-02, pivot 2026-06-04) : allège la constitution auto-chargée. Bon mais P2.

**Garde-fou context-rot (critique d.) :** core/CLAUDE.md **et** projects/forgia/CLAUDE.md s'auto-chargent **tous les deux** + MEMORY + (futur) capabilities.json. **Le contexte auto-chargé pourrait GROSSIR vs aujourd'hui** si non budgété strictement. Budget cible : les deux CLAUDE.md combinés ≤ taille actuelle, MEMORY ≤200l, concept-first §6 **hors** auto-load.

---

## 6. Décision structurelle à trancher (à faire AVANT toute copie — change la destination de ~30 fichiers)

La recommandation initiale (option B) reposait sur une **prémisse fausse vérifiée** : le `.claude/` V2 n'est PAS une "base overlay propre déjà amorcée", c'est un **stub quasi-vide** (3 règles + 1 settings.local stub de 1.3K, aucun agent/commande/hook/CLAUDE.md interne). Donc "enrichir ce qui est là" = "**construire l'overlay quasi de zéro in-repo**". Antoine doit trancher en connaissance de cause :

- **(A) Cockpit unique relocalisé.** `D:/IA Antoine` = seul foyer ; le code V2 branche sa config via junction `.claude → D:/IA Antoine/projects/forgia/.claude`. **+** un seul cockpit portable. **−** maintenir les junctions ; config V2 plus "in-repo".
- **(B) Hybride in-place (reco révisée).** Core dans `D:/IA Antoine` ; overlay Forgia maintenu **dans le repo V2** (`Forgia Rewrite/.claude`, versionné avec le code) ; `projects/forgia/` du cockpit = **junction** vers le `.claude` + `.mcp.json` V2. **+** config versionnée avec le code, pas de désync. **−** le `.claude` V2 étant un stub, c'est un **build, pas un héritage** — il faut y créer agents/commands/hooks/CLAUDE.md interne.
- **(C) Relocate complet + retraite `D:/Forgia`.** Figer `D:/Forgia` (legacy), construire `D:/IA Antoine`, le repo V2 garde un `.claude` minimal héritant via junction. **+** legacy clairement gelé. **−** plus de travail de junction initial.

**Ma reco : (B) avec les yeux ouverts** — core portable dans `D:/IA Antoine`, overlay construit IN-REPO V2 (versionné), `projects/forgia/` = junction. C'est le moins de duplication/désync **à condition d'accepter que l'overlay V2 est à bâtir**, pas à adopter.

---

## 7. Plan d'exécution ordonné (3 vagues, zéro-budget, anti big-bang)

**VAGUE 1 — P0, quelques heures, 80% du gain, risque quasi-nul (ship indépendant du reste) :**
1. **grepai → V2 + construire l'index** (`.grepai/` absent → build obligatoire).
2. **Tuer le zombie BMAD** : supprimer bmad-enforce.sh + reset `.bmad-task.json` `{}`.
3. **Fix `a.becker` → Antoi** (5 sites) via `source python-path.sh`.
4. **Trim MEMORY 700→~200** + archiver <2026-05-01 + tagger `[V1-ORIGIN]`.

**VAGUE 2 — split core/overlay (P0/P1) :**
5. Trancher A/B/C (§6) — **bloque** toute copie.
6. CLAUDE.md : extraire universel → core, garder V2 root comme overlay.
7. Règles : copy+fix vers core/overlay ; **scinder concept-first §6** → concept-first-table.md overlay ; retirer cross-refs morts.
8. data-driven-paths.md + concept-first-gate.sh : **glob V2 (`crates/forgia-*/**` + `src/main.rs`)**.
9. discover-paths.sh + réécrire session-start.sh / validate-commit.sh.
10. settings : scinder allow universels (core) / Forgia (overlay sur base V2 1.3K) ; dropper V1 42K + .bak ; ajouter Stop-hook build+clippy.
11. **CRÉER** (pas migrer) : `projects/forgia/docs/registry/` (baseline V2 L1=69→50), `tools/forgia-mcp/` (rebuild V2), rapatrier/régénérer LEXIQUE_ECS.md.
12. Graceful degrade mempalace (learn-from-*, fallback learning.log).
13. Junction mémoire (~/.claude) + repointer .bmad config V2.

**VAGUE 3 — DIFFÉRÉE jusqu'à preuve de besoin / 2e projet (anti-YAGNI) :**
14. capabilities.json **généré** (scan crates+sensors+cmds).
15. sensor-truth-map généré sur **forgia2_* ET forgia_*** (113 fichiers).
16. Applicateur de staleness tableau §6 (CI locale).
17. BOOTSTRAP.md junctions exécutable (valeur uniquement au 2e projet).
18. Eval harness ollama (juge local).
19. ADR registry hors CLAUDE.md.

**Drops immédiats (toutes vagues) :** bmad-enforce.sh, analyze-patterns.md, settings.local.json V1, *.bak ×3, questionnaire-validation, coaching-presentation, SECURITY_ACTION_REQUIRED, hyworld-integration-audit, docs/captured/.
```
