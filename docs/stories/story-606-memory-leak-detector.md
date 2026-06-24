# Story-606 — Détecteur de fuite mémoire (timeline + alerte croissance soutenue)

> **Statut** : À FAIRE (créée 2026-06-18 — audit migration MCP forgia V1)
> **Niveau BMAD** : Standard
> **Valeur** : MED — RAM dans le budget (~1.8 GB) mais une fuite tue une session longue
> **Origine** : feature V1 non migrée. Spec = `tools/forgia-mcp/src/server.rs` (`read_memory_leaks`, `forgia_memory_leaks.json`). En V2, `forgia2_memory.json` n'émet qu'un **point unique** ; la crate `forgia-qa-telemetry` (growth_rate) a été **planifiée mais jamais créée**.

## À construire
- Ring de 20 snapshots (1 / 30 s) : process_memory_mb + breakdown par catégorie d'asset (Image/Mesh/Material/Scene/AnimationClip/AudioSource).
- Calcul `growth_rate` (EWMA ou régression sur la fenêtre) + **alerte** si croissance soutenue > 10 MB/30 s pendant 3 cycles consécutifs, avec **attribution** (catégorie suspecte).
- Écrire `forgia2_memory_leaks.json` { timeline[], alert?, suspect_category }.
- Health check associé (severity + next-step) — cf `observability-required`.
- Crate : `forgia-observability` (ou concrétiser `forgia-qa-telemetry`).

## Acceptance
- [ ] Timeline remplie en run (20 points glissants).
- [ ] Pas d'alerte en steady-state ; alerte + catégorie quand on injecte une fuite synthétique (leak volontaire en debug).
- [ ] 0 alloc hot path (snapshot toutes les 30 s, pas par frame).
