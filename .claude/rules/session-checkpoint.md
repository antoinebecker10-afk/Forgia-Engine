# Session Checkpoint (Forgia Rewrite V2) — RÈGLE PERMANENTE

> **Pattern de pause propre pour sessions longues.** Garantit zéro perte d'info entre
> sessions, même cross-terminal / cross-IDE / cross-Claude-instance. À déclencher
> sur demande user OU auto-détection indicateurs critiques.

---

## 1. Triggers

### Triggers explicites user
- "pause", "checkpoint", "stop session", "fais pause"
- "fais moi le prompt pour reprendre"
- "mémorise ce qu'on a fait", "memorise la session"

### Triggers auto-détection (signaler proactivement à l'user)
- 🚨 Session > 100k tokens (contexte va se faire compresser)
- 🚨 Plus de 5h de travail dans même session
- 🚨 Refacto multi-crates High Risk imminent (mieux vaut splitter)
- 🚨 Changement d'environnement annoncé (laptop ↔ desktop, IDE ↔ CLI)
- 🚨 Drift contextuel détecté (je commence à oublier détails / re-demander info récente)

Quand 1 indicateur déclenche : **proposer pause à l'user**, ne pas forcer.

---

## 2. Process — 4 étapes obligatoires (dans cet ordre)

### Étape 1 — Audit du livré

Lister concrètement ce qui a été accompli pendant la session :

```markdown
## ✅ Livré cette session
| # | Item | Fichiers | Compile |
|---|---|---|---|
| 1 | <feature> | path:line | ✅ |
| 2 | <bug fix> | path:line | ✅ |
```

Format obligatoire :
- **Fichiers touchés** précis (path absolu ou relatif workspace)
- **Concepts/architecture validés** (rules respectées, sensors actifs)
- **Compile state** par item (✅ green / ⚠️ warning / ❌ red)
- **Bugs résolus** avec root cause + diagnostic source (sensor / observation / etc.)

Garantit : pas de re-debug de ce qui est déjà OK.

### Étape 2 — Audit du reste

Lister ce qui était planifié mais pas livré :

```markdown
## 📋 Reste à faire
| Tâche | Effort | Risk | Dépendances |
|---|---|---|---|
| Tier 1B | 15 min | Low | Tier 1A done |
| Tier 1C | 45 min | Medium | Tier 1B done |
| Tier 2A | 1h30 | **High** | Tier 1 complet |
```

Format obligatoire :
- **Tâches restantes ordonnées** par dépendance
- **Effort estimé** par tâche (réaliste, pas optimiste)
- **Risk level** (Low/Medium/High) basé sur :
  - Low : isolated change, 0 régression attendue
  - Medium : multi-fichiers, possible breakage callers
  - High : extraction gameplay, deps cross-crate, refacto architectural
- **Dette tech identifiée** à fixer plus tard (🟠 HIGH / 🟡 MEDIUM / 🟢 LOW selon `audit-protocol.md`)

Garantit : continuité du plan, priorité explicite.

### Étape 3 — Persistence dans memories

Écrire dans `C:\Users\Antoi\.claude\projects\c--Users-Antoi-Desktop-Forgia-Rewrite\memory\` :

Ne pas écrire les nouvelles connaissances V2 dans l'ancien espace `D--Forgia`.
Le registre complet et les sources historiques restent accessibles via
`docs/AI_MEMORY_MAP.md`.

#### a) 1 fichier session (type `project`)
Recap chronologique : `session_YYYY_MM_DD_<short_slug>.md`
- Frontmatter `name/description/metadata.type: project`
- Sections : Livraisons / Bugs résolus / Décisions design / Reste à faire / Cross-refs

#### a bis) Deux clés EXACTES dans le frontmatter (adopté 2026-08-09)

Mesuré ce jour-là : le score de la recherche sémantique **ne distingue pas** une
bonne réponse d'une question absurde (0,586 pour « recette de tarte aux pommes »
contre 0,670 pour une réponse juste), et une question en français rate une cible
anglaise. **L'exact bat le flou.** Deux champs, sur les mémoires qu'on écrit
désormais — jamais de reprise rétroactive des anciennes :

Écrire dans `metadata:` (le système **normalise** de toute façon : il déplace les
champs sous `metadata` et éclate les listes inline `[a, b]` en lignes — un motif
de recherche écrit sur la forme inline ne trouverait donc **rien**) :

```yaml
metadata:
  type: reference
  concepts:            # vocabulaire partagé, cf. concept-first-table-forgia.md
    - outillage
    - grepai
  anchors:             # LE trajet qui manquait : symbole/chemin → leçon
    - .claude/hooks/tools-health.sh
    - crates/forgia-mode-roguelite/src/decor.rs
  valide_au: 2026-08-09
```

**Les deux commandes de relecture** (mesurées sur la forme normalisée) :

```bash
grep -lE "^ +- <concept>$"   *.md   # toutes les mémoires d'un concept
grep -lE "^ +- .*<chemin>"   *.md   # toutes les leçons sur un fichier
```

- **`concepts`** : mots du tableau `concept-first-table-forgia.md`, étendu au besoin.
- **`anchors`** : les symboles et chemins RÉELS. C'est le trajet absent
  aujourd'hui : `SpawnKeepout` — le type qui porte toute la règle de dégagement
  des spawns — ne figurait dans **aucune** des 890 mémoires. On ne pouvait donc
  pas aller d'un nom croisé dans le code vers la leçon qui le concerne.
  Vérifié après ajout : `grep -l "grepai-autostart.ps1" *.md` rend en **99 ms**
  les DEUX mémoires concernées, celle de juillet et celle d'août.
- **`valide_au`** : date de dernière vérification contre le code réel.

**La condition sans laquelle tout ceci ne vaut rien** : ces clés doivent être
LUES à un moment. Une métadonnée qu'on écrit sans jamais la relire reproduit
exactement l'échec constaté sur mempalace — 11 378 tiroirs, contribution nulle
à la session. Le point de lecture naturel est le gate `concept-first-gate.sh`
(il se déclenche avant chaque `Edit` sur un `.rs`), mais **ne le brancher que
quand assez de mémoires portent des ancres** : le brancher pour une seule
mémoire, c'est livrer un tuyau sans eau.

#### b) N fichiers reference (type `reference`)
Pour CHAQUE pattern réutilisable découvert pendant la session :
- `reference_<topic>.md`
- Frontmatter `metadata.type: reference`
- API publique / use cases / anti-patterns / cross-refs `[[name]]`

#### c) MEMORY.md index updaté
- Insérer N nouvelles entrées en haut sous nouvelle section dated
- **Format strict** : entrées ≤ 150 chars, une ligne, lien `[file.md](file.md)` + hook
- Pas de duplication des sections existantes (vérifier que le date+contexte est unique)

Garantit : knowledge survit à la compression contexte, nouveau Claude charge auto les memories pertinentes.

### Étape 3 bis — Verser le reste à faire dans l'Établi (adopté 2026-08-12)

**Une tâche qui ne vit que dans une mémoire de session ou dans un message de clôture
est déjà perdue** : personne ne relit une conversation close, et une mémoire ne se
lit que si la session suivante pense à la chercher.

Tout ce que l'étape 2 a listé comme « reste à faire » doit donc être versé dans le
bloc `⟦DETTE-DEBUT⟧ … ⟦DETTE-FIN⟧` de
[`docs/etabli/etabli-forgia.html`](../../docs/etabli/etabli-forgia.html), puis
republié sur son URL (cf. `/veille` étape 4 — **toujours avec le paramètre `url:`**,
sinon on crée un second établi).

Format d'une entrée :

```js
{t:'chantier'|'story', ref:'<slug court ou n° de story>', fait:false,
 risque:'haut'|'moyen'|'bas', effort:'~30 min', origine:'session JJ/MM',
 ti:'<le fait, pas le sujet>', qu:'<pourquoi, et ce que ça débloque>'}
```

Deux règles :

- **Ne jamais supprimer une ligne faite** — la cocher (`fait: true`). On veut voir
  ce qui a été soldé, sinon la section ne dit plus que ce qui reste et perd sa valeur
  de preuve.
- **Le risque est celui de l'oubli, pas de l'exécution.** Une story dont personne ne
  sait si elle est faite (statut illisible) est `risque: 'haut'` même si le travail
  est trivial.

*Origine : le 12/08, onze stories avaient survécu à une purge et cinq chantiers de fin
de session ne vivaient que dans un message. Aucun tableau ne les montrait.*

### Étape 4 — Prompt de reprise (bootstrap autonome)

Écrire un prompt **self-contained** que l'user peut copier-coller dans :
- Nouveau terminal Claude Code
- Autre fenêtre VSCode
- Web claude.ai/code
- Mobile

Structure obligatoire :

```
Workspace : <path absolu>

CONTEXTE :
<où on en est, état dernier livré, ce qui marche>

RÈGLE BLOQUANTE À RESPECTER :
<rule file path + référence memory si applicable>

REPRENDS :
▼ TIER 1 — <Titre> (~Xh total)
  Tâche A : <description>
    - Source : <fichier:ligne>
    - Move : <ce qu'il faut déplacer>
    - Pattern : <référence à pattern réussi précédemment>
    - Effort : ~X min, <Risk>

  Tâche B : ...

▼ TIER 2 — <Titre> (~Xh, HIGH RISK)
  ...

VALIDATION INTERMÉDIAIRE OBLIGATOIRE :
<commandes check entre quelles étapes>

DETTE TECH À FIXER APRÈS :
- 🟠 <priorité haute>
- 🟡 <priorité moyenne>

MEMORIES À CHARGER :
- session_YYYY_MM_DD_<slug>
- reference_<topic1>
- reference_<topic2>

FICHIERS CLÉS À LIRE AVANT CODE :
- <path1>
- <path2>

GO commence par <première tâche concrète>, valide compile, puis enchaîne.
```

Garantit : reprise zéro context drift, même par instance Claude différente.

---

## 3. Anti-patterns à bannir

- ❌ **Pause sans persistance memory** — knowledge perdu à la compression contexte
- ❌ **Prompt reprise vague** (e.g. "continue le refacto") — nouveau Claude pas autonome, drift garanti
- ❌ **Liste reste à faire sans risk levels** — user ne sait pas prioriser
- ❌ **Auto-trigger silencieux** — toujours **proposer** à l'user, jamais forcer pause
- ❌ **Memories sans cross-refs** `[[name]]` — knowledge isolé, pas réutilisable
- ❌ **MEMORY.md entrées > 200 chars** — viole convention concision, scroll fatigue
- ❌ **Skip "fichiers clés à lire avant code"** — nouveau Claude code en aveugle

---

## 4. Template prompt de reprise minimal

```
Workspace : C:\Users\Antoi\Desktop\Forgia Rewrite

CONTEXTE :
Session du <DATE> terminée. <X> livré, reste <Y> selon plan mémorisé.

REPRENDS le refacto suivant (ordre, valider compile entre chaque) :

▼ TIER 1 — <objectif>
  Tâche A : <description>
    Source : <fichier>
    Effort : ~X min, <Risk>

VALIDATION : rtk cargo check -p <crate> après chaque tâche

MEMORIES À CHARGER : session_YYYY_MM_DD, reference_<topic>

FICHIERS CLÉS : <path1>, <path2>

GO Tâche A, valide compile, puis enchaîne.
```

---

## 5. Origine

- 2026-05-16 PM — Session refacto V2 trop longue (>100k tokens), Antoine demande
  "fais moi le prompt pour reprendre dans un autre terminal". J'écris le prompt
  ad-hoc, qui fonctionne. Antoine demande "c'est quoi le process ?". J'explique.
  Antoine : "fais une rule fondatrice". → Cette règle.

- **Principe sous-jacent** : Les sessions Claude ont un contexte fini qui se
  compresse silencieusement. Les memories persistent across sessions. Un prompt
  bien construit + memories chargées = reprise quasi-identique à la session
  origine. Sans ce process, drift garanti.

---

## 6. Cross-refs

- [[reference-rule-fine-grained-crates]] — autre règle fondatrice V2
- CLAUDE.md global §11 "Memorise" — commande user pour persister memories
- `concept-first.md` (V1) — protocole avant Edit, complémentaire
- `audit-protocol.md` (V1) — séverités 🔴🟠🟡🟢 utilisées pour dette tech
