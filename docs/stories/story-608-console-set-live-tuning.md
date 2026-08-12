# Story-608 — Console `:set` live-tuning (canal balance IA single-param)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : trace partielle** (symbole `FpsTuning`) — une partie de ce
> qu'elle décrit existe, le reste n'a pas été retrouvé.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : À FAIRE (créée 2026-06-18 — audit migration MCP forgia V1)
> **Niveau BMAD** : Quick/Standard (brancher un consommateur existant)
> **Valeur** : MED (effort LOW) — régler un paramètre de feel sans rebuild, pilotable par l'IA
> **Origine** : capacité V1 = `set_fps_tuning` (modifie un param FpsTuning EN LIVE via bridge). En V2, la console `:set` est **câblée mais sans consommateur** (story-548) ; le hot-reload genome (Shift+F12) marche déjà mais n'est pas single-param ciblé.

## À construire
- Brancher le consommateur `ConsoleEvent::SetTuning { param, value }` (déjà émis, story-548) → mute le champ correspondant de `FpsTuning` / `Tuning` (ou pousse l'override genome) **en live**.
- Feedback console (valeur avant/après) + clamp aux bornes du gène si applicable.
- Optionnel : `:get <param>` pour lire la valeur courante.
- Pas de nouveau bridge fichier — c'est interne au jeu (la console suffit ; l'IA pilote via le récap test in-game).

## Acceptance
- [ ] `:set run_speed 8.0` dans la console modifie la vitesse en temps réel, sans rebuild.
- [ ] Valeur hors-borne → clampée + message.
- [ ] Param inconnu → message d'erreur clair (pas de panic).
