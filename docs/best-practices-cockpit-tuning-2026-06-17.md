# Best-practices IA + dev solo complexe — ajustement du cockpit D:/Forgia (2026-06-17)

## 1. Résumé exécutif

Le cockpit Forgia est, sur le fond, **plus mûr que la majorité des projets** : constitution stable (CLAUDE.md, 237 lignes) séparée des specs, règles bloquantes ancrées à un incident daté + source académique, sensors JSON comme oracle de vérification, sous-agents à contexte isolé. Ces piliers sont à conserver tels quels.

Mais une vérification fichier par fichier révèle **un risque structurel dominant non détecté jusqu'ici** : le cockpit est calibré pour le code **V1** (`RUST/Forgia/Forgia/`, crates plates `forgia-game/forgia-engine/forgia-terrain`) alors que **tout le travail réel est en V2** (62 crates sous `Forgia Rewrite/crates/`). Conséquence concrète et mesurée sur disque : le gate phare `concept-first-gate.sh` ne couvre que **31 des 286 fichiers `.rs` de V2 (~11 %)** — il matche `forgia-game/src` (binaire mince) + `forgia-terrain/src`, mais il est mort sur les ~55 autres crates (`forgia-fps`, `forgia-combat`, `forgia-rpg`…), soit ~89 % du code produit. C'est le P0 absolu, devant tout ajout.

Trois autres écarts solides : (a) **aucun hook `Stop` bloquant** — `cargo check`/`clippy` sont purement advisory (`post-edit-check.sh` exit 0 toujours) ; (b) **MEMORY.md = 696 lignes / 128 K** dépasse sa propre limite et n'est chargé que partiellement (context rot empirique) ; (c) **qa-lead n'a pas de clamp anti-faux-positif**, ce qui a déjà produit 2 faux flags (MessageReader multicast).

À l'inverse, plusieurs recommandations « ajouter un fichier/un sensor » sont à **rejeter ou fondre** : un dev solo qui maintient déjà 851 fichiers mémoire + 33 hooks + 18 règles + 9 agents n'a pas besoin de 3 nouveaux artefacts permanents.

---

## 2. Ce que Forgia fait déjà mieux que l'industrie (ne pas toucher)

- **Constitution séparée et protégée en écriture.** CLAUDE.md (237 lignes) est un artefact gouvernant stable et re-lu, exactement le modèle « constitution au sommet, immuable » de GitHub Spec Kit. Il est *protégé en écriture* par la deny-list (`Edit/Write(file_path:d:/Forgia/CLAUDE.md)`). C'est plus fort que la plupart des projets — ne pas toucher au principe.
- **Catalogage des failure modes daté + sourcé.** Chaque règle bloquante cite un incident canonique (Arena underwater, stale-binary story-482, WALL_Y story-432) + date + source (Acton CppCon, arXiv 2604.02547). C'est le « process YAGNI » de Fowler appliqué à la gouvernance : chaque règle a une preuve d'incident. Rare et précieux — garder ce critère d'admission.
- **Sensors JSON comme oracle de vérification.** La famille `forgia_*.json` + le MCP Forgia (`cargo_check`, `cargo_clippy`, `check_stability_locks`) donnent le « machine-checkable pass/fail » qu'Anthropic identifie comme le manque #1 des projets. Pour un projet « Pas de tests », c'est le bon instinct — `observability-required.md` reste non-négociable.
- **Règle binaire = preuve (mtime ordering).** `multi-terminal-coordination.md` §5 (`mtime(source) ≤ mtime(bin) ≤ mtime(sensor)`) encode précisément le perception-gap METR / OWASP LLM09 : l'IA affirme un fix qui n'a jamais tourné dans l'artefact. Très peu de cockpits formalisent ça. Load-bearing, à garder.
- **Isolation de contexte par sous-agents.** Scope outils explicite (verifier READ-ONLY ligne 22, implementer EDIT), tiers de modèle, maxTurns. C'est le pattern multi-agent Anthropic. `post-impl-auto-qa` réserve déjà le fan-out aux Standard+ (jamais Quick) — discipline coût-token correcte pour un solo zéro-budget.
- **Table concept-first §6.** Producteur/consommateurs/timing/hot/net/script par concept. METR identifie « unfamiliarity with codebase conventions » comme cause #1 du ralentissement IA ; cette table EST l'artefact à plus haut ROI contre ce tax. Le meilleur actif du cockpit. (Sa *colonne* est saine ; son *application* est bloquée par le gate mort — voir §3.)
- **Tiering BMAD scope-based + GPS Protocol.** Quick≤3 / Standard≤10 / Enterprise10+, « 1 action → compile → next ». C'est exactement le « small batch size » que DORA 2025 prescrit comme garde-fou de stabilité sous IA. Garder tel quel.

---

## 3. Écarts prioritaires

### 3.1 — [P0] Audit chemins V1 → V2 (hooks + settings + MCP root)
- **Priorité** P0 · **Effort** M · **Fichier cible** : `.claude/hooks/concept-first-gate.sh`, `.claude/settings.json`, `.mcp.json`
- **Pourquoi.** Vérifié sur disque : le cockpit gouverne V1, le produit est V2.
  - `concept-first-gate.sh` ne matche que `*forgia-game/src/*.rs | *forgia-engine/src/*.rs | *forgia-terrain/src/*.rs`. En V2 : `forgia-engine` **n'existe pas** comme crate, `forgia-game` est le binaire mince, `forgia-terrain/src` existe (le gate le couvre donc), mais les ~55 autres crates (`forgia-fps/src`, `forgia-combat/src`, `forgia-rpg/src`…) ne matchent pas. **Résultat mesuré sur disque : le gate couvre 31 fichiers `.rs` sur 286 (~11 %).** La règle phare est un no-op silencieux sur ~89 % du code réel. C'est aussi pourquoi le P0 « pas de Stop hook » est pire qu'estimé : le seul bloqueur PreToolUse existant est quasi-mort.
  - `settings.json` deny-list protège des chemins V1 : `RUST/Forgia/Forgia/.../assets.rs`, `RUST/Forgia/Forgia/CLAUDE.md`. Le `assets.rs` V2 (Lock L1) n'est pas couvert.
  - `.mcp.json` lance grepai sur `D:/Forgia/RUST/Forgia/Forgia` (V1) — la cartographie sémantique §3 étape 2 indexe l'ancien code.
- **Source.** Anthropic best-practices — *« hooks are deterministic where CLAUDE.md is advisory »* (https://code.claude.com/docs/en/best-practices). Un gate qui n'attrape rien est pire qu'absent : il crée une fausse assurance.
- **Action concrète.**
  1. Re-pointer la `case`-glob du gate sur `*crates/*/src/*.rs` (couvre les 62 crates) ; ou supprimer le gate si on préfère tout déplacer vers le Stop hook (§3.2).
  2. Ajouter à la deny-list le chemin V2 réel de `assets.rs` (Lock L1) et de `CLAUDE.md` V2 si distinct (modification de fichier protégé → à faire via demande user).
  3. Re-pointer grepai sur le workspace V2 dans `.mcp.json`.
  4. `grep -rl 'RUST/Forgia/Forgia\|forgia-game/src\|forgia-engine/src' .claude/` pour trouver les autres références V1 résiduelles dans les hooks.

### 3.2 — [P0] Hook `Stop` déterministe scopé V2
- **Priorité** P0 · **Effort** M · **Fichier cible** : `.claude/settings.json`
- **Pourquoi.** `settings.json` n'a **aucun** hook `Stop`. `post-edit-check.sh` est warn-only (vérifié : `exit 0` aux lignes 16 et 66). L'IA peut donc clore un tour avec un build cassé ou un Lock violé ; l'humain reste la boucle de vérification. C'est le gap structurel #1 d'Anthropic : sans pass/fail bloquant, la prose « `cargo check` après modification » de CLAUDE.md §3 n'est jamais mécaniquement appliquée.
- **Source.** Anthropic Claude Code best-practices — *« Give Claude a check it can run »* (https://code.claude.com/docs/en/best-practices) ; DORA 2025 *« AI amplifies the safety net »* (https://dora.dev/research/2025/).
- **Action concrète.** Ajouter un hook `Stop` qui lance `cargo check -p <crate touchée>` + `clippy` + `check_stability_locks` (réutilise le binaire MCP Forgia déjà présent) et **bloque la fin de tour si non-clean (exit 2)**. Le scoper aux **crates V2** (sinon il hérite du même angle mort V1 que §3.1). Garder `post-edit-check.sh` advisory pour le feedback rapide en cours de tour ; le Stop hook est le gate final.

### 3.3 — [P0] Élagage de l'index mémoire (context rot)
- **Priorité** P0 · **Effort** M · **Fichier cible** : `MEMORY.md` (+ `pre-compact.sh`)
- **Pourquoi.** Vérifié : MEMORY.md = **696 lignes / 128 K**. Le system-reminder avertit lui-même qu'il **n'en charge qu'une partie** → des memories récentes tombent silencieusement hors contexte. Le cap auto-imposé « ≤150 chars/entrée » est violé. C'est du context rot empirique. **Correction d'un fait précédemment erroné** : l'archive **n'est PAS sous-utilisée** — elle contient **87 fichiers** (vs 764 fichiers topiques en racine). Le problème n'est donc **pas** « archiver davantage de fichiers topiques » mais **trimmer l'index live** (`MEMORY.md`) et son chargement.
- **Source.** Chroma *Context Rot* (https://research.trychroma.com/context-rot) ; Anthropic context-engineering — index mince, just-in-time (https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents).
- **Action concrète.**
  1. Étendre `pre-compact.sh` (ou nouvelle skill `/memory-lint`) pour : (a) échouer si `MEMORY.md > ~30 K`, (b) flag les entrées >200 chars, (c) déplacer les **entrées d'index** de sessions >30 jours vers une section « anciennes sessions » ou un index secondaire, (d) vérifier que chaque topic file lié existe encore.
  2. Au bootstrap session, ne charger que les **3-5 entrées d'index les plus récentes** + à la demande, pas l'index entier. (Ne pas toucher au volume de topic files : ils sont chargés à la demande, pas dans le chemin auto.)

### 3.4 — [P1] Clamp anti-faux-positif du reviewer + dé-inflation d'emphase (item fusionné)
- **Priorité** P1 · **Effort** S · **Fichier cible** : `.claude/agents/qa-lead.md`, `.claude/agents/verifier.md`, en-têtes des règles
- **Pourquoi.** Vérifié : `qa-lead.md §"Ce que tu NE FAIS PAS"` (ligne 97) **ne contient aucun clamp** « signale uniquement les gaps de correction/critère d'acceptance » ni étape de re-vérification. Anthropic avertit qu'un reviewer cherchant des gaps en trouve toujours → sur-engineering. La mémoire Forgia le confirme : qa-lead a faux-flaggé MessageReader multicast **2×** (`feedback_messagereader_is_multicast.md`). Même racine que l'inflation d'emphase : 6 règles taguées BLOQUANTE + listes INTERDIT/OBLIGATOIRE de CLAUDE.md §3/§6 — quand tout est gras, plus rien ne ressort. Ce sont **deux angles d'un même problème** (sur-emphase produisant des findings faux-positifs), donc une seule action.
- **Source.** Anthropic best-practices — *« Tell the reviewer to flag only gaps that affect correctness… treat the rest as optional »* (https://code.claude.com/docs/en/best-practices).
- **Action concrète.**
  1. Ajouter à `qa-lead.md` (et `verifier.md`) : *« Signale UNIQUEMENT les gaps qui affectent la correction ou un critère d'acceptance explicite ; traite style/défensif comme optionnel ; n'invente jamais de story hors-scope. »* + étape *« re-vérifier chaque finding contre les sources avant de le rapporter »*.
  2. Réserver le tag **BLOQUANTE** aux 3 règles à incident prouvé + récurrent : `multi-terminal` (stale-binary), `no-speculative-fix` (WALL_Y), `concept-first` (Arena). Rétrograder `in-game-test-recap`, `post-impl-auto-qa`, `observability-required` en **« fortement recommandé »** (toujours appliquées, sans poids d'emphase saturé).

### 3.5 — [P1] Commit des milestones validés (anti WIP-mine)
- **Priorité** P1 · **Effort** S · **Fichier cible** : un paragraphe dans une règle existante (`build-stack.md` ou `multi-terminal-coordination.md`) + nudge hook + correction CLAUDE.md §11
- **Pourquoi.** MEMORY documente de façon récurrente du « NON COMMITÉ / runtime non validé » qui « détonne au rebuild » (`feedback_unvalidated_wip_detonates_on_rebuild.md` : story-583 a cassé la végé quand un rebuild a activé du WIP dormant). CLAUDE.md §11 step 5 dit même *« ne PAS créer de commit sauf si demandé »*, ce qui **renforce** le pattern. La leçon n'est qu'une feedback memory, pas une règle.
- **Source.** Fowler *Continuous Integration* — build never broken (https://martinfowler.com/articles/continuousIntegration.html) ; feedback Forgia natif.
- **Action concrète.** Ne **pas** créer un fichier-règle dédié (anti file-proliferation, cf §4). Ajouter un **paragraphe** dans une règle existante : *« Un milestone validé runtime se commit immédiatement (scopé). Le WIP non validé non commité est une mine au prochain rebuild. »* + nudge dans `multi-terminal-check.sh` (déjà lancé au SessionStart) : si `git status >12` ET une story est DONE → rappeler « commit le milestone validé maintenant ». Inverser la formulation §11 pour les milestones **VALIDÉS** (modification CLAUDE.md → demande user).

### 3.6 — [P1] Suite d'évals de régression issue des bugs passés
- **Priorité** P1 · **Effort** M · **Fichier cible** : `docs/registry/regression-evals.yaml` (V2) + skill `forgia-verify`
- **Pourquoi.** Les failure modes canoniques (Arena underwater, stale-binary, WALL_Y, false-Changed/DerefMut) ne vivent qu'en prose MEMORY. Aucun fichier runnable ne vérifie qu'un changement ne les réintroduit pas. Pour un projet « Pas de tests », c'est le filet le plus load-bearing — et il est **zéro-budget** (il lit les sensors existants, a un consommateur réel : `forgia-verify` le parcourt).
- **Source.** Anthropic *Demystifying evals* (20-50 cas issus de vrais échecs, régression ~100 % pass — https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) ; DORA 2025 robust testing.
- **Action concrète.** Créer `regression-evals.yaml` : 1 ligne par bug passé `{symptom, sensor field + valeur attendue, règle qui aurait dû l'attraper}`. Étendre `forgia-verify` pour parcourir cette liste avant de clore une story Standard+.

---

## 4. À simplifier / risque de sur-ingénierie pour un solo

**Principe directeur** : le cockpit condamne lui-même la prolifération de fichiers/règles. Plusieurs « gaps » se traitent par **fusion**, pas par ajout.

- **Construire (pas « réutiliser ») le chemin load-on-demand.** `.claude/rules/on-demand/` existe mais est **vide** — et **aucun mécanisme** (hook, pointeur CLAUDE.md, skill) ne charge depuis ce dossier. Ce n'est donc pas de l'infra prête, juste un dossier vide. Action saine quand même : déplacer le **corps verbeux** de `concept-first.md` (13 K), `multi-terminal-coordination.md` (8,9 K), `in-game-test-recap.md` vers `on-demand/`, garder en règle active un résumé impératif ~15 lignes + 1 exemple canonique, et sortir les blocs « Sources externes » (CppCon, GDC, arXiv) vers `docs/rationale/` (ce sont des distracteurs au moment de l'inférence — Chroma : un seul distracteur dégrade la sortie). Conserver le pointeur dans CLAUDE.md. **Mais facturer ça comme « bâtir le chemin load-on-demand », pas « utiliser une infra existante ».**

- **Rejeter le sensor de vélocité (`forgia_velocity.json`).** METR est réel, mais auto-logger reverts/aller-retours est de la pure cérémonie **sans boucle de consommation** : un solo n'agira pas sur un résumé hebdo de vélocité par-dessus 851 fichiers mémoire, et les « aller-retours par story » exigent un tag manuel que le hook ne peut pas inférer de façon fiable. Ajouter de l'observabilité **sur** le cockpit à un cockpit déjà saturé d'observabilité est le risque de méta-sur-ingénierie. Au mieux : un exercice manuel ponctuel, pas de l'infra permanente.

- **Ne pas créer de règle `dependency-trust.md` autonome.** Le slopsquatting (USENIX 2025) est réel, mais la fréquence est faible : un solo sur stack Bevy 0.18 **gelée** ajoute rarement une crate, et `Cargo.lock` est déjà en deny-list. **Fondre** en une puce dans `build-stack.md` : *« ne jamais ajouter une crate introduite par l'IA sans vérifier crates.io (existe, downloads non-triviaux, maintenue) »* — le verifier (qui lance déjà `cargo check`) flag toute crate externe nette dans le diff.

- **deny-list : cosmétique, pas un fardeau.** Vérifié sur disque : **31 entrées** (ni 20 ni 34 comme estimé en cours d'analyse), dont **4 pointent sur des chemins V1 morts** (`RUST/Forgia/Forgia/.../assets.rs`, `RUST/.../CLAUDE.md`). Le mélange globs grossiers + chemins absolus existe mais 31 entrées ne sont pas un fardeau de maintenance. Il n'existe **pas** de feature `deny_group` dans le schéma settings.json de Claude Code → ne pas proposer ça. Seule vraie correction : les chemins V1 → V2 (déjà couvert en §3.1).

- **Hooks (33) + `validate-commit.sh` 36 K / 669 lignes.** Beaucoup de wrappers fins multi-terminal (claim-task, session-lock, terminal-register, multi-terminal-check) → consolidables en 1 script. `validate-commit.sh` à **auditer avant** de l'appeler du bloat : 669 lignes pour un check de présence de story *semble* démesuré, mais personne n'a confirmé que ces lignes sont mortes vs font plus que le check de story. Auditer, puis trancher. Supprimer aussi le `.bak-pre-compact-2026-05-27` de la mémoire qui traîne.

- **Agents orphelins.** 9 agents définis mais CLAUDE.md §7 ne liste que « 5 skills actifs (solo) ». `bevy-specialist`, `terrain-specialist`, `performance-analyst`, `economy-designer` existent sans déclencheur clair vs `implementer` ; `game-maker` n'apparaît dans aucune règle/workflow. Carry-cost sans usage prouvé (process YAGNI). Action : soit documenter en 1 ligne chacun *quand* le solo les invoque, soit fusionner les peu-utilisés dans `implementer` avec une consigne de domaine. Supprimer `game-maker` s'il n'a jamais servi.

- **ADR — peupler le dossier vide.** `docs/adr/` existe et est **vide**. Les décisions datées (Build/Edit deferral 2026-06-02, pivot vision 2026-06-04) gonflent CLAUDE.md inline, contredisant son statut « stable ». **Correction d'un fait précédemment erroné** : il **n'y a pas** de référence stale `ROADMAP_CURRENT.md → ROADMAP.md` à corriger — `ROADMAP_CURRENT.md` existe bien (V2 docs, 23 K, à côté de `ROADMAP_ROGUELITE.md` et `ROADMAP_POST_AUDIT`) ; la réf CLAUDE.md §11 est correcte, il n'y a pas de `ROADMAP.md` cible. Action (P2, isolée) : template ADR standard (context/options/decision/status superseded-by), sortir les décisions datées de CLAUDE.md vers `docs/adr/NNN`.

---

## 5. Plan d'ajustement concret de D:/Forgia (checklist ordonnée)

Ordonné par ROI décroissant. **Ne rien ajouter avant d'avoir corrigé l'angle mort V1/V2.**

1. **[P0]** `grep -rl 'RUST/Forgia/Forgia\|forgia-game/src\|forgia-engine/src' D:/Forgia/.claude/ D:/Forgia/.mcp.json` → recenser toutes les références V1.
2. **[P0]** Re-pointer la `case`-glob de `concept-first-gate.sh` sur `*crates/*/src/*.rs` (ou retirer le gate au profit du Stop hook).
3. **[P0]** Re-pointer grepai sur le workspace V2 dans `.mcp.json` (`mcp-serve <V2 path>`).
4. **[P0]** Ajouter un hook `Stop` dans `settings.json` : `cargo check -p <crate>` + clippy + `check_stability_locks`, exit 2 si non-clean, scopé crates V2.
5. **[P0]** (via demande user, fichier protégé) Mettre à jour la deny-list : chemin V2 réel de `assets.rs` (Lock L1) + CLAUDE.md V2 si distinct.
6. **[P0]** Étendre `pre-compact.sh` / créer `/memory-lint` : fail si `MEMORY.md >30 K`, flag entrées >200 chars, basculer les entrées d'index >30 j hors du chemin auto-chargé.
7. **[P1]** Ajouter le clamp correctness-only + re-vérification dans `qa-lead.md` et `verifier.md`.
8. **[P1]** Rétrograder le tag BLOQUANTE à 3 règles (multi-terminal, no-speculative-fix, concept-first) ; les autres → « fortement recommandé ».
9. **[P1]** Ajouter le paragraphe « commit milestone validé » dans une règle existante + nudge dans `multi-terminal-check.sh` ; inverser §11 step 5 (demande user).
10. **[P1]** Créer `docs/registry/regression-evals.yaml` (1 ligne/bug passé) + faire parcourir par `forgia-verify`.
11. **[P2]** Déplacer corps verbeux des 3 grosses règles vers `rules/on-demand/` + bâtir le pointeur de chargement ; sortir « Sources externes » vers `docs/rationale/`.
12. **[P2]** Auditer `validate-commit.sh` (669 lignes), consolider les wrappers multi-terminal, supprimer les `.bak`.
13. **[P2]** Documenter ou fusionner les agents orphelins ; peupler `docs/adr/` ; fondre la garde slopsquatting dans `build-stack.md`.
14. **Rejeté** : `forgia_velocity.json` (pas de boucle de consommation), `dependency-trust.md` autonome (fondu dans build-stack), `commit-validated-milestones.md` autonome (paragraphe), `deny_group` (feature inexistante).

---

## 6. Sources clés (URLs réellement utilisées)

**Anthropic / Claude Code**
- Claude Code best-practices (hooks déterministes vs CLAUDE.md advisory ; clamp reviewer ; check exécutable) — https://code.claude.com/docs/en/best-practices
- Effective context engineering (index mince, just-in-time, NOTES.md, évals régression) — https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents

**Context rot / mémoire**
- Chroma — Context Rot — https://research.trychroma.com/context-rot

**Vélocité IA / safety net**
- METR RCT 2025 (perception gap ~40 pts, self-reported speed peu fiable) — https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/
- DORA 2025 (small batch, AI amplifies the safety net) — https://dora.dev/research/2025/

**Process / CI / décisions**
- Fowler — Continuous Integration (build never broken) — https://martinfowler.com/articles/continuousIntegration.html
- Fowler — YAGNI (process YAGNI) — https://martinfowler.com/bliki/Yagni.html
- GitHub Spec Kit (constitution stable séparée des specs) — https://github.com/github/spec-kit
- ADR — Architecture Decision Records (status superseded-by) — https://adr.github.io/

**Supply chain / sécurité LLM**
- USENIX Security 2025 — slopsquatting / package hallucination — https://arxiv.org/abs/2406.10279
- OWASP Top 10 for LLM Applications (LLM03 Supply Chain, LLM09 Misinformation) — https://owasp.org/www-project-top-10-for-large-language-model-applications/
