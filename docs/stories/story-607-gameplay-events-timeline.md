# Story-607 — Timeline d'événements gameplay (post-mortem de run)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `server.rs`, symbole `MessageReader`).
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
> **Valeur** : MED — rejouer/débriefer une run, source de vérité chronologique
> **Origine** : feature V1 non migrée. Spec = `tools/forgia-mcp/src/server.rs` (`read_events_log`, `forgia_events_log.json`, ring 500 events typés). En V2 la couverture est **fragmentée** : victory/defeat dans `forgia2_roguelite_state`, hits/kills dans `forgia2_combat` + `forgia_killfeed` — **aucune timeline unifiée**.

## À construire
- Ring roulant ~500 events **typés et horodatés** : combat_hit, kill, damage_player, boon_picked, wave_start/clear, boss_defeated, victory/defeat, biome/stage_entered, level_up.
- Source = s'abonner (multicast `MessageReader`) aux events existants (combat, waves, roguelite_state) — **ne pas dupliquer la logique**, juste agréger.
- Écrire `forgia2_events.json` (rolling, refresh 1 s).
- Crate : `forgia-observability`.

## Acceptance
- [ ] Une run complète produit une timeline ordonnée cohérente (wave_start → hits → kills → boss_defeated → victory).
- [ ] Lecture du fichier = reconstruction de la run sans ambiguïté.
- [ ] Aucun event consommé/volé aux autres lecteurs (multicast — cf `feedback_messagereader_is_multicast`).
