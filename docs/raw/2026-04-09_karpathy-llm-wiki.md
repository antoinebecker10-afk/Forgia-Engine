# LLM Wiki Pattern — Andrej Karpathy

> Source: https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f
> Ingéré: 2026-04-09

## Pattern

3 couches:
1. **Raw Sources** — docs immutables (articles, papers, images, data)
2. **Wiki** — markdown maintenu par l'IA (résumés, entités, concepts, comparaisons)
3. **Schema** — config (CLAUDE.md/AGENTS.md) définissant structure et conventions

3 opérations:
- **Ingest**: nouveau doc → extraire, résumer, indexer, croiser les refs
- **Query**: question → chercher wiki → répondre avec citations → filer les bonnes réponses en pages wiki
- **Lint**: audit santé (contradictions, orphelins, trous, cross-refs manquantes)

## Fichiers clés
- `index.md` — catalogue par catégorie avec one-liners
- `log.md` — append-only chronologique, préfixé `## [date] type | description`, grepable

## Tooling recommandé
- Obsidian Web Clipper (articles → markdown)
- Obsidian graph view (topologie wiki)
- Marp (slides depuis markdown)
- Dataview (queries sur frontmatter)
- qmd (search local BM25/vector + LLM re-ranking, CLI + MCP)

## Philosophie
- L'IA gère le bookkeeping (cross-refs, résumés, contradictions, cohérence)
- L'humain curate les sources et pose les questions
- Référence: Vannevar Bush, Memex (1945) — store personnel avec trails associatifs
