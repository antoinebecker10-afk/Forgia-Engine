# Story-574 — RPG dramatic relief (max_height 28→80)

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia_chunks_snapshot.json`, fichier `biomes.rs`, symbole `SeaLevel`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED
> **État d'origine (périmé, cf bandeau)** : IN PROGRESS (impl faite, QA en cours)
**Scale** : Standard (2 fichiers, mais change une baseline documentée + user-facing)
**Date** : 2026-06-05
**Lignée** : Audit procgen général (6 strates) → finding B2 → investigation workflow (8 agents) → décision user **B (relief dramatique)**.

## Contexte

L'audit procgen avait classé le relief plat du monde RPG (`max_height=28`, relief observé ~8-25m sur 2048m) comme **B2 "Bloquant / terrain plat pathologique"**. L'investigation adversariale (3 vérifieurs, haute confiance) a **infirmé** ce classement : `RPG_MAX_HEIGHT=28` ([forgia-rpg/lib.rs:111](../../crates/forgia-rpg/src/lib.rs#L111)) est un **choix délibéré et documenté** du vertical slice (commentaire daté 2026-05-19 anti-bandes-d'eau-parasites), **pas un bug de câblage**. Le preset showcase (180) est intentionnellement bypassé.

→ Pas de fix. Mais le user a choisi de **rendre le relief plus dramatique** (art/scope call).

## Insight technique décisif

`build_chunk_mesh` utilise `heightmap_at` ([meshing_heightmap.rs:77](../../crates/forgia-terrain/src/meshing_heightmap.rs#L77)), **pas** `heightmap_at_gen_ext`. Donc :
- Seul `TerrainConfig.max_height/sea_level` pilote le relief (gen_config features/island/redistribution ignorés sur ce path).
- Formule `h = sea_level + h_norm·(max-sea)` avec `h_norm ≥ 0` → **terrain ne descend jamais sous sea_level** → eau toujours flush → **inutile de toucher sea_level** (aucune cascade swim/eau).
- Le finding M3 (island falloff `h→0`) est dans `heightmap_at_gen_ext_impl` → **inactif sur le path RPG**.

## Change set (minimal, confirmé)

1. `RPG_MAX_HEIGHT: 28.0 → 80.0` (forgia-rpg/lib.rs:111) — amplitude 4..80m (×3.2). `RPG_SEA_LEVEL=4.0` inchangé.
2. `falloff_m: 5.0 → 25.0` (config/genomes/villages/starter_hamlet.toml) — absorbe **M9** (falloff 5m sur ~76m de dénivelé = mur 60-72°). Hot-reloadable.
3. Commentaires mis à jour (lib.rs make_map_gen_config + TOML).

## Consommateurs auto-adaptés (vérifiés, 0 edit requis)

- Plan d'eau (`SeaLevel` resource, sea_level inchangé) ✓
- Snow band `max_height*0.75/0.95` (meshing_heightmap.rs:154) → snow sur sommets >60m ✓
- Biome altitude `gen_config.max_height` (biomes.rs:248) → re-stratifie ✓
- Village `target_y` échantillonné (lib.rs:342) ✓ — gradient géré par falloff_m=25
- Spawn `safe_y`, foliage Y, mesh+collider → tous échantillonnent `heightmap_at` ✓
- bounds test `RPG_MAX_HEIGHT*1.1` (lib.rs:2285) ✓ — auto

## Acceptance criteria

- [ ] AC1 — `cargo check -p forgia-rpg` 0 erreur
- [ ] AC2 — `cargo clippy -p forgia-rpg` 0 warning sur fichiers touchés
- [ ] AC3 — Runtime : relief visiblement plus marqué (collines/montagnes ~60-80m, neige sur sommets)
- [ ] AC4 — Village reste plat et accessible (pas de mur de soutènement au bord, pas de bâtiment submergé)
- [ ] AC5 — Sensor `forgia_chunks_snapshot.json` → `max_height:80`, hauteurs échantillonnées montent

## Limites connues / follow-ups

- **Foliage slope** (audit M-cosmétique) : arbres restent verticaux sur pentes raides — DIFFÉRÉ (forgia-foliage LOCKED autre terminal). Plus visible avec relief dramatique.
- **Magnitude non hot-reloadable** : const Rust → chaque ajustement = rebuild. Option **C** (genome TOML) débloquerait le tuning live Shift+F12.
- **edge_falloff** (heightmap.rs:68, width 80m) non touché : n'affecte que l'anneau de bord (~80m), pas la zone jouable centrale.
