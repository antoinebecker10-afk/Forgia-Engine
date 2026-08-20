# Audit des règles au filtre « context engineering Claude 5 » — 2026-08-20

> **Déclencheur** : l'article de Thariq (Anthropic, 2026-07-24) — 80 % du system
> prompt de Claude Code supprimé pour les modèles Claude 5 sans perte mesurée.
> Diagnostic : *overconstraining*. Les six bascules : règles→jugement,
> exemples→interfaces typées, tout-d'avance→divulgation progressive,
> répétition→descriptions simples, mémoire-CLAUDE.md→auto-mémoire, specs→références riches.
>
> **Le filtre appliqué ici** (clause d'exception de Thariq comprise) :
> une règle se garde si elle porte un **failure mode documenté** que le modèle ne
> peut pas déduire seul ; elle se **compresse** à l'échec + l'invariant (le récit
> va en mémoire) ; elle se **mécanise** quand un gate/genome peut la porter
> (cette couche ne consomme aucun contexte et ne bride pas le jugement) ; elle se
> **supprime** si c'est du pilotage comportemental qu'un Claude 5 fait par jugement.
>
> Continuité : [[feedback_taxe_contexte_des_regles]] (2026-08-09, ~29,7 k tokens
> mesurés) et le gate `context-budget` (xtask).

## Mesure du jour (ce qui était réellement chargé dans MA session)

| Bloc | Poids | Note |
|---|---:|---|
| 10 règles globales (`D:\IA Antoine\.claude\rules\`) | 39 973 o | toutes chargées |
| 12 règles projet chargées (dont `on-demand/` !) | 87 476 o | voir constat n°1 |
| 3 × CLAUDE.md (projet + 2 globaux) | 15 038 o | |
| MEMORY.md (index) | ~17 600 o | hors périmètre de cet audit |
| **Total règles+CLAUDE.md** | **142 487 o ≈ ~42 k tokens** | à chaque session |

Les 10 petites règles domaine (`no-hardcode`, `scalability`, `combat-code`…)
n'étaient **pas** dans le contexte initial et sont apparues à la demande en
cours de session — elles fonctionnent déjà en divulgation progressive et ne
comptent pas dans la taxe. (À confirmer avec `/context`.)

## Trois constats structurels (phase A — corrigés ce jour)

1. **`on-demand/` était chargé quand même.** Le harnais charge `.claude/rules/`
   récursivement : les deux règles map (27 827 o) étaient payées à chaque
   session depuis leur « déménagement » — le dossier était une intention, pas un
   mécanisme. ✅ **Corrigé** : déplacées vers `docs/design/`, `/map` mis à jour.
2. **Le gate `context-budget` était aveugle sur ce trou** : `read_dir` non
   récursif — le « contrôle qui passe à vide » de `controle-de-la-sortie.md` §1,
   dans le gate même qui surveille la taxe. ✅ **Corrigé** : scan récursif +
   plafond abaissé 95 000 → 72 000 (71 699 mesurés après déménagement).
3. **`build-stack.md` décrivait le stack V1** (« pas de tests » face à 2 234
   tests réels, lightyear jamais porté) — pire qu'un coût : une orientation
   fausse. ✅ **Réécrite** courte et vraie (1 336 → ~1 100 o).

**Taxe après phase A : ~114 660 o.** La suite (phase B) demande ton GO fichier
par fichier — chaque règle est porteuse, la compression est un jugement de
contenu, pas de la mécanique.

## Phase B — verdicts règle par règle (PROPOSITION, rien d'appliqué)

### Règles globales (`D:\IA Antoine\.claude\rules\`) — cible 39 973 → ~13 700 o

| Règle | Poids | Verdict | Justification / cible |
|---|---:|---|---|
| `concept-first.md` | 8 340 | **COMPRESSER** → ~3 000 | Failure mode documenté (symptom fixation). Garder : étape 0 data/code, les 5 étapes en liste, hot-path check court. Sources académiques + historique → mémoire. Le gate hook (qui vient de me bloquer — il marche) porte déjà l'enforcement. |
| `multi-terminal-coordination.md` | 7 425 | **MÉCANISER + COMPRESSER** → ~1 500 | Le standup git est déjà fait par les hooks de session. Garder : règle 5 (artefact=preuve, mtime) + le tableau qui-cède. Récit du bug canonique → mémoire. |
| `no-speculative-fix.md` | 4 631 | **COMPRESSER** → ~2 000 | Ton failure mode n°1 vécu. Garder le tableau 🟢🟡🔴 + la reset rule + « scope = demande explicite ». Exemples/patterns → mémoire. |
| `in-game-test-recap.md` | 3 684 | **COMPRESSER** → ~1 200 | Garder le template 5 points (c'est une interface, au sens Thariq). Le pourquoi → mémoire. |
| `post-impl-auto-qa.md` | 3 647 | **COMPRESSER** → ~1 200 | Garder : déclencheur + quand-skippable + les 2 subagents. Les checklists détaillées vivent déjà chez les agents `verifier`/`qa-lead`. |
| `bug-triage.md` | 3 532 | **COMPRESSER** → ~1 500 | La table symptôme→outil EST la règle. §2-3 et anti-patterns : un Claude 5 les déduit de la table. |
| `security-anti-injection.md` | 2 786 | **GARDER** (≈ tel quel) | Sécurité ≠ overconstraining : les patterns d'injection sont des données, pas du comportement. Léger trim possible. |
| `model-selection.md` | 2 383 | **SUPPRIMER** (ton GO — fichier global D:) | Obsolète : cite Opus 4.8/Sonnet 4.6/Haiku 4.5 comme actuels, tarifs et flags périmés. Contenu faux = nuisible. `/model` + jugement suffisent. |
| `observability-required.md` | 2 077 | **COMPRESSER** → ~800 | La doctrine tient en 4 lignes + le test ultime. `sensor-audit`/`capteur-gate` mécanisent déjà. |
| `ask-when-unclear.md` | 468 | **GARDER** | Déjà minimal ; encode TA préférence (1 question, pas 5) — pas déductible. |

### Règles projet (`.claude/rules/`) — cible 63 500 → ~27 000 o

| Règle | Poids | Verdict | Justification / cible |
|---|---:|---|---|
| `concept-first-table-forgia.md` | 11 083 | **GARDER** → ~8 500 | C'est le ROUTEUR (Thariq : « références riches » — exactement ça). Dégraisser la section discipline-grepai (mesures → mémoire). |
| `session-checkpoint.md` | 10 984 | **COMPRESSER** → ~3 000 | Garder : triggers + les 4 étapes en liste + template minimal §4. Les formats détaillés (a bis, 3 bis) → mémoire, relus au moment du checkpoint (c'est déjà un moment de lecture). |
| `spawn-clearance.md` | 8 438 | **COMPRESSER** → ~2 000 | **Mécanisée depuis** : `SpawnKeepout` + genome + capteur portent l'invariant. Garder : l'invariant, le tableau qui-cède, la checklist courte. Leçons chiffrées → mémoire. |
| `controle-de-la-sortie.md` | 7 277 | **COMPRESSER** → ~3 000 | Garder : les 2 obligations + §1bis/1ter en une ligne chacun + les cliquets. Tableaux d'occurrences → mémoire. |
| `outillage.md` | 5 865 | **COMPRESSER** → ~3 500 | Déjà dégraissée une fois. §3 (grepai) et §5 se resserrent ; les hooks rapportent déjà l'état. |
| `story-done-gate.md` | 5 237 | **COMPRESSER** → ~1 500 | Le gate xtask EST la règle. Garder : quand + commandes + procédure FAIL. Historique du 2026-05-21 → mémoire. |
| `fine-grained-crates.md` | 4 313 | **COMPRESSER** → ~1 800 | Garder l'arbre de décision + les gardes. Historique 266→62 → mémoire (déjà dans ADR-0002). |
| `log-digest.md` | 3 732 | **COMPRESSER** → ~1 200 | Garder : les commandes + « regarde ⇒ digest d'abord ». Mesures et récit Hermes → mémoire. |
| `boost-protocol.md` | 1 384 | **MÉCANISER** → 0 (commande) | Déclenchée par le mot « Boost » uniquement → `.claude/commands/boost.md`, comme `/map`. |
| `build-stack.md` | 1 336 | ✅ **FAIT** (~1 100) | Réécrite ce jour. |
| 10 règles domaine (no-hardcode, scalability, …) | ~15 100 | **GARDER** | Chargées à la demande (observé) — déjà au bon régime. `no-hardcode`/`genome-code` sont en plus mécanisées par `validate-genomes`. |

### CLAUDE.md — cible 15 038 → ~5 700 o

| Fichier | Poids | Verdict | Note |
|---|---:|---|---|
| Projet `CLAUDE.md` | 6 929 | **COMPRESSER** → ~4 500 — **exige ton autorisation explicite** (§8 du fichier lui-même) | Couper : §2 « Rôle de l'IA » et le DOIT/NE-DOIT-PAS comportemental (baby-sitting d'ère 2025, un Claude 5 le fait par jugement) ; §9 référence V1 → mémoire. Garder : mémoire-map, vision/priorité SHIP, lexique, Locks, anti-traps, BMAD, interdits durs. |
| `~/.claude/CLAUDE.md` (RTK) | 4 758 | **COMPRESSER** → ~1 200 | La règle d'or + le piège clippy suffisent ; les 8 tableaux d'exemples sont de la répétition (bascule n°4 de Thariq). |
| `~/CLAUDE.md` (BMAD/Ruflo) | 3 351 | **DÉPLACER** → mémoire reference | Informationnel, jamais opératif en session. |

**Cible totale phase B : ~46 400 o ≈ ~14 k tokens par session (−67 % vs ce matin).**

## Ce que cet audit ne touche PAS

`MEMORY.md` (~17,6 Ko — index mémoire, autre logique, autre arbitrage) · les
hooks (ils rapportent, ne coûtent rien en contexte) · le contenu des mémoires.

## Actions restantes (ton GO requis)

1. **GO compression** par groupe : (a) règles projet, (b) règles globales D:
   (impact cross-projets), (c) CLAUDE.md projet (autorisation §8), (d) RTK/BMAD.
   Chaque compression = le récit part en mémoire `reference_*` AVANT la coupe —
   rien ne se perd, tout descend d'un étage.
2. **Toi, en local** : `/doctor` (diagnostic officiel des sections redondantes)
   et `/context` (vérifier la liste réellement chargée, dont les règles domaine).
3. **Candidats mécaniques hérités des audits** (sessions dédiées déjà
   prompt-ées ou à prompter) : ré-injection du noyau à la compaction (hook
   `SessionStart:compact`, leçon Superpowers — tester via `hooks-securite.sh`) ;
   protocole RED/GREEN de test des règles (writing-skills) ; marqueur
   `[NEEDS CLARIFICATION]` + gate (spec-kit).

---

*Phase A appliquée le 2026-08-20 (déménagement on-demand, gate récursif +
plafond 72 000, build-stack réécrite). Phase B en attente de GO.*
