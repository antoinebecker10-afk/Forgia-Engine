# story-659 — Cohérence élémentaire des projectiles/tracers + retrait du « POW! »

> ⛔ **CANCELLED 2026-08-12 — purge de refonte**
>
> Cette story est close par la refonte décrite dans [`REFONTE_GDD.md`](../REFONTE_GDD.md),
> qui redéfinit le jeu vers *Forgia: The Spared*. Sa §7 pose la règle : **les stories
> des phases se créent au fur et à mesure, aucune n'est présumée exister.** Les 143
> stories ouvertes partaient d'un plan que la refonte remplace.
>
> **Ce qu'on sait de son code : trace partielle** (fichier `hitmarker.rs`) — une partie de ce
> qu'elle décrit existe, le reste n'a pas été retrouvé.
>
> **Rien n'est supprimé.** Ce fichier reste lisible : si son sujet revient dans une
> phase de la refonte, il sert de matière première — pas de ticket à rouvrir.
>
> **Statut** : CANCELLED

> **État d'origine (périmé, cf bandeau)** : IN_PROGRESS (validation visuelle user en attente)
> **Niveau BMAD** : Quick (4 fichiers)
> **Origine** : user 2026-07-03 — « remplace les projectiles de mes armes pour que ce soit cohérent avec les éléments (électrique, feu, poison), pareil pour les projectiles ennemis, et retire les textes POW ».

## Fait

1. **Tracers alignés éléments** (`tracer.rs`) : Pépin jaune→**bleu électrique**, Lenoir orange→**cyan perce** (Bourrasque était déjà feu ✓).
2. **Teintes muzzle/impact alignées éléments** (`weapon_vfx/mod.rs`) : Pépin bleu, Bourrasque feu, Lenoir cyan, **Boucherie rouge-orange→vert poison** — flash, lumière ET sphère élémentaire racontent le même élément. Tests réécrits (assertions par teinte d'élément).
3. **Roquette Boucherie** : déjà verte poison (story-611) + **traînée de volutes poison** ajoutée (réutilise l'asset d'aura ×0.35, émetteur enfant + simulation world-space = comète). ⚠️ Pattern ChildOf non prouvé pour hanabi dans ce repo (les auras utilisent un follow-system) — si la traînée ne rend pas au test, basculer sur le follow.
4. **« POW! » supprimé** (`hitmarker.rs`, story-528 AC6) : redondant avec les VFX élémentaires + le chime weakspot. Fn retirée, état tické inoffensif conservé.

## Bloqué (dépendance autre terminal)

**Projectiles ennemis** (boules de feu, `forgia-ai-arena-bot`) : le crate est en WIP actif chez l'autre terminal (M au claim-check) — même traitement (teinte élément + traînée) à faire dès qu'il aura commité. Story suiveuse.

## Acceptance criteria

- [x] 4 armes = 4 signatures élémentaires cohérentes (tracer + muzzle + light + impact + burst)
- [x] Plus de « POW! » à l'écran
- [x] 272 tests verts (tint tests réécrits), clippy 0, build ✓
- [ ] **Validation user** : la traînée poison de la roquette rend ; chaque arme se reconnaît à sa couleur
