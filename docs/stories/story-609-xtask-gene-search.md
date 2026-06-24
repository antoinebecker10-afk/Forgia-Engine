# Story-609 — `cargo xtask gene-search` (introspection genome cross-pack)

> **Statut** : À FAIRE (créée 2026-06-18 — audit migration MCP forgia V1)
> **Niveau BMAD** : Standard (sous-commande xtask)
> **Valeur** : MED — outillage balance data-driven (pas bloquant ship)
> **Origine** : capacités V1 = `list_genomes` / `query_genome` / `search_gene` (MCP). En V2, **pas de recherche cross-pack** d'un gène → on grep à la main. Audit : 71/114 packs gardent le schéma `[[genes]]` (min/max/chromosome), 43 sont des structs Serde plates.

## À construire
- Sous-commande `cargo xtask gene-search <terme>` qui scanne `assets/genomes/**` + `config/genomes/**` :
  - `list` : tous les packs `[[genes]]` avec count genes/constraints/domain.
  - `query <pack>` : détail d'un pack (genes id/chromosome/min/max/default + constraints + extends).
  - `search <mot>` : tous les gènes matchant (pack, gène, chromosome, range, default).
- Tolérer les 43 packs au schéma plat (les lister à part, sans crash).
- Pas de serveur MCP : un xtask suffit (cohérent avec `cargo xtask asset-load` existant).

## Acceptance
- [ ] `cargo xtask gene-search search damage` liste tous les gènes « damage » à travers les packs.
- [ ] Les packs au schéma plat sont signalés, pas ignorés silencieusement, pas de panic.
- [ ] 0 dépendance réseau / budget.

---
> **Note** : capteurs LOW non priorisés (hors stories) — timeline perf 600-frames (couverte par `forgia2_lag_events`), auto-dégradation VRAM guardian (estimation `forgia2_vram` suffit), `mapgen_config` (track RPG, différé).
