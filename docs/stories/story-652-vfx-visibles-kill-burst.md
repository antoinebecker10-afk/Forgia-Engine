# Story-652 — VFX visibles : multiplicateurs genome + burst de kill

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : il EXISTE et tourne toujours.** Les fichiers, capteurs
> ou symboles qu'elle cite ont été retrouvés dans le dépôt (capteur `forgia2_weapon_vfx.json`, symbole `VfxTuning`).
> Elle n'est pas marquée DONE pour autant : **personne ne l'a jamais validée**, et
> se l'accorder maintenant serait la DONE fictive que la purge du batch V7 a
> nettoyée le matin même. Le code reste, la promesse de validation tombe.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS (validation visuelle user en attente)
> **Niveau BMAD** : Standard (7 fichiers)
> **Origine** : feedback user 2026-07-03 « je ne vois encore aucun effet visuel des VFX » + « dans WoW y'a des flammes, des grains et plein d'autres choses ».

## Diagnostic

Le système tourne (init OK, 11 dummies warmup, 0 erreur hanabi, textures chargées) — mais les effets sont **subliminaux par design historique** : la story-450 les avait nerfés (-35 % taille, HDR ÷3) car « trop gros, bloquaient la visée ». Résultat : muzzle flash de **1,5-3,5 cm sur 25-50 ms**, sans bloom. Même parfaits, ils ne « comptent » pas face à la référence WoW/Gunfire.

**Question discriminante encore ouverte** (posée au user) : le petit flash au canon est-il visible du tout ? Si NON malgré ×2.2, il y a en plus un bug de binding texture à creuser.

## Changements

1. **`VfxTuning`** (forgia-effects) : multiplicateurs globaux `size/count/lifetime` appliqués À LA CONSTRUCTION des 11 EffectAssets — la visibilité devient un choix DATA (`roguelite_vfx.toml`, hot-reload 1Hz avec **rebuild des assets**), plus un hardcode de 2026-05. Défauts : ×2.2 taille, ×1.5 quantité, ×1.3 durée.
2. **Burst de KILL** (`create_kill_burst`, texture twirl Kenney) : 18×count volutes chaudes 10-22 cm qui s'ouvrent vers le haut (0.35-0.6 s) + PointLight 6k lumens tinté par l'arme — spawn à l'edge vivant→mort dans le fire path. Le premier « moment WoW » du kill, séquencé après le freeze hitstop-648 et pendant le knockback-650.
3. Warmup : 11e dummy (kill_burst) ; capteur `forgia2_weapon_vfx.json` (tuning actif + reload_count).

## Acceptance criteria

- [x] Multiplicateurs appliqués aux 11 effets (sizes init + gradients + counts + lifetimes)
- [x] Hot-reload = rebuild des assets (tuning LIVE en session avec le user)
- [x] Kill burst + light au point de mort, scale par arme (Boucherie plus gros)
- [x] Warmup shader du burst (pas de freeze au 1er kill)
- [ ] **Validation user** : muzzle/impacts VISIBLES, kill = explosion lisible, pas de gêne de visée (sinon on baisse en live)
- [x] check + clippy + tests + build verts

## Inc.2 (2026-07-03) — architecture industrie : offset hors-surface + additif + échelle unifiée

Feedback user : « encore trop caché dans les mobs ». Diagnostic : (a) les impacts spawnaient AU point de contact, à moitié dans le mesh (occlusion) ; (b) les impacts sur ennemis = **sphères émissives d'element_vfx**, hors du pipeline textures/tuning ; (c) blending alpha classique = les effets d'énergie se noient dans la scène rouge.

Fixes (réfs [VFXDoc](https://vfxdoc.readthedocs.io/en/latest/textures/overview/), [RealTimeVFX](https://realtimevfx.com/t/trying-to-grasp-textures-blend-modes-additive-alpha-blended/20914)) :

1. **Offset hors-surface vers le tireur** (`vfx_impact_offset_m`, 0.35 m, gene) : impacts armes + kill burst + sphères élémentaires spawnent DEVANT le corps — le standard « offset along the surface normal ».
2. **Blending additif** (`AlphaMode::Add`, hanabi natif) sur les 8 effets d'énergie (flashes, étincelles, burst, arcs shock) — le rendu « énergie qui brille » standard, sans bloom. Fumée/poussière/nuage restent en alpha.
3. **Échelle unifiée** : les sphères élémentaires (feedback de hit sur ennemi !) suivent maintenant `vfx_size_mult` comme tout le reste — un seul curseur pour tout le feedback de combat.
4. Défauts : taille ×2.2 → **×3.0**, quantité ×1.5 → **×1.8**.

## Suite

- Réponse binding si « aucun flash même ×2.2 » → diag EffectMaterial dédié
- Recette complète (flipbook explosion + ring mesh + décal) = audit §3 Inc.2
- Sourcing textures avancé : base Simon Schreibt (« Textures for VFX Database ») à miner pour les flipbooks — licences à vérifier PAR source (pas uniforme comme Kenney CC0)
