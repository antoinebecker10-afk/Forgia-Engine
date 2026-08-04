# Registre commun de la mémoire Forgia

Ce registre est le point d'entrée permanent de Claude Code et Codex. Il donne
accès à toute la mémoire accumulée sans injecter plusieurs milliers de fichiers
dans chaque fenêtre de contexte.

## Sources actives

| Priorité | Contenu | Chemin |
|---|---|---|
| 1 | Index et mémoire synthétisée de Forgia Rewrite | `C:\Users\Antoi\.claude\projects\c--Users-Antoi-Desktop-Forgia-Rewrite\memory\` |
| 2 | Conversations brutes Forgia Rewrite (`.jsonl` et sous-dossiers UUID) | `C:\Users\Antoi\.claude\projects\c--Users-Antoi-Desktop-Forgia-Rewrite\` |
| 3 | Ancienne mémoire active de `D:\Forgia` | `C:\Users\Antoi\.claude\projects\D--Forgia\memory\` |

L'index principal à lire au début d'une tâche non triviale est :

`C:\Users\Antoi\.claude\projects\c--Users-Antoi-Desktop-Forgia-Rewrite\memory\MEMORY.md`

## Archives et espaces hérités

| Contenu | Chemin |
|---|---|
| Mémoire V1 exportée | `D:\Forgia\.claude-data\memory\` |
| Sauvegarde mémoire V1 | `D:\Forgia\.claude-memory-backup\` |
| Ancien workspace Desktop | `C:\Users\Antoi\.claude\projects\C--Users-Antoi-Desktop-Forgia\memory\` |
| Ancien workspace Rust Desktop | `C:\Users\Antoi\.claude\projects\C--Users-Antoi-Desktop-Forgia-RUST-Forgia-Forgia\memory\` |
| Ancien workspace Rust D: | `C:\Users\Antoi\.claude\projects\D--Forgia-RUST-Forgia-Forgia\memory\` |
| Sauvegarde du 23 mars | `C:\Users\Antoi\.claude\projects\d--Forgia---Save-23-03-26\memory\` |

Sous Windows, la casse de `D--Forgia`/`d--Forgia` ne désigne pas deux dossiers
différents.

## Mémoire versionnée dans le dépôt

- `CLAUDE.md` et `AGENTS.md` : contrats permanents des assistants ;
- `.claude/rules/` : règles spécialisées ;
- `docs/SESSION_STATE.md` : état de reprise ;
- `docs/stories/`, `docs/design/`, `docs/audit*/`, `docs/handoff/` : décisions et preuves projet ;
- code, tests, genomes et capteurs : vérité actuelle, prioritaire sur un souvenir ancien.

Les fichiers `forgia_memory_breakdown.json`, `forgia_memory_leaks.json` et
similaires décrivent la RAM/VRAM du jeu. Ils ne sont pas une mémoire de
conversation.

## Protocole de consultation

Pour chaque tâche non triviale :

1. extraire 3 à 8 termes discriminants : feature, symbole Rust, crate, genome,
   capteur, symptôme et éventuel numéro de story ;
2. chercher ces termes d'abord dans les `MEMORY.md`, puis récursivement dans les
   fichiers Markdown de toutes les sources ci-dessus ;
3. lire les références pertinentes, en privilégiant `feedback_*`, puis
   `reference_*`, puis les sessions récentes ;
4. consulter les `.jsonl` seulement si la synthèse est absente ou ambiguë ;
5. vérifier chaque conclusion historique contre le dépôt actuel avant de coder.

Mempalace est l'index sémantique canonique de ces sources. Son stockage actif est
`C:\Users\Antoi\.mempalace\palace`. Claude doit privilégier
`mcp__mempalace__mempalace_search` pour la découverte sémantique, puis ouvrir les
fichiers sources retournés. Une recherche textuelle directe reste obligatoire en
fallback si le MCP est indisponible ou ne retourne aucun résultat fiable.

Une absence de résultat doit être dite explicitement ; elle ne permet pas
d'inventer une règle ou une décision passée.

## Écriture et pérennisation

La destination canonique des nouvelles connaissances est exclusivement :

`C:\Users\Antoi\.claude\projects\c--Users-Antoi-Desktop-Forgia-Rewrite\memory\`

Chaque capitalisation durable doit créer ou mettre à jour un fichier topique et
ajouter un pointeur concis dans `MEMORY.md`. Les espaces V1 restent accessibles en
lecture et ne doivent pas recevoir les nouvelles connaissances de V2.
