# Story-606 — Détecteur de fuite mémoire (timeline + alerte croissance soutenue)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_memory.json`, fichier `server.rs`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : À FAIRE (créée 2026-06-18 — audit migration MCP forgia V1)
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
