# Story-655 — Fin des sphères procédurales : bursts hanabi texturés par élément

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (fichier `element_vfx.rs`, symbole `ElementBurstAssets`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS (validation visuelle user en attente)
> **Niveau BMAD** : Standard (3 fichiers + dep)
> **Origine** : demande user 2026-07-03 « tu peux remplacer les boules procédurales maintenant par des vrais effets ? » — dernière itération de sa toute première demande de la session (audit VFX).

## Ce qui était encore des « boules »

`element_vfx.rs` spawnnait des **sphères émissives** (`Mesh3d(Sphere)` + StandardMaterial, fade par scale) sur : (a) **chaque hit élémentaire** (le feedback le plus fréquent du jeu !), (b) les **réactions** Combustion/Surcharge (2 sphères superposées). C'étaient littéralement les « boules procédurales » du feedback initial.

## Remplacement

- **4 vrais bursts hanabi texturés** (`ElementBurstAssets`, 1 par élément, construits PostStartup avec `VfxTuning`) : Feu → léchures de flamme, Poison → volutes, Électrique/Perce → étincelles 4 branches. Radial + biais montant, additif, HDR décroissant dans la couleur de l'élément.
- **Impacts** : burst au point décalé hors-surface (offset story-652) + la **PointLight colorée conservée** (elle peint le décor — c'était la bonne partie des sphères).
- **Réactions** : burst du 1er élément (grand) + halo burst du 2e (×0.6) + lumière ×3 — la fusion se lit par superposition des couleurs.
- Warmup shader : 4 dummies cachés au PostStartup (leçon anti-freeze story-594).
- `entity_ttl` calé sur le lifetime max des particules (despawn hanabi = particules coupées).
- Dep `bevy_hanabi` ajoutée à forgia-mode-roguelite (le crate construit désormais des EffectAssets).

## Reste des sphères (hors scope, connu)

`ElementVfxAssets` (mesh+mats) devient orphelin → cleanup candidat ; effets projectiles (roquette Boucherie/ultimes) toujours mesh-based ; `element_vfx.rs` = hotspot 30 édits → split module candidat.

## Acceptance criteria

- [x] Plus aucune sphère spawnée sur hit/réaction (bursts texturés + lumière)
- [x] Couleurs = genome élément ; tailles/quantités/durées = curseurs `roguelite_vfx.toml`
- [x] check + clippy (0 introduit) + 253 tests + build verts
- [ ] **Validation user** : chaque hit élémentaire éclate en vraies particules de son élément
