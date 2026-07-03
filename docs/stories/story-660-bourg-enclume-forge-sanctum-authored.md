# Story-660 — « Le Bourg de l'Enclume » : DA authored pour forge_sanctum (salle 2)

> **Demande user 2026-07-02** : « une vraie pièce avec sa propre direction artistique,
> différente de la première, avec d'autres packs d'assets ». Cible = `forge_sanctum`
> (la salle 2 du multi-salles story-646), jusque-là 100 % procédurale et fade.
> **Scale BMAD** : Standard (data-only : arena_layouts.toml ; 0 code Rust).
> **Statut** : LIVRÉ (non commité→commit suivant) — à valider runtime.

## Direction artistique
**Place de village-forge DIURNE** — kit **medieval_hexagon** (église/taverne/marché/
maisons/puits, jamais utilisé en arène) + rochers **nature**. Température chaude/claire
vs Cryptes sombres roses → différenciation instantanée entre salles adjacentes.

## Règles de conception appliquées (recherche web 2026-07-02, sources : Level Design
Book, 80.lv, MY.GAMES — cf rapport)
- **1 focal point « weenie »** : le Beffroi (église ×10 = 16,5 m, nord, visible partout).
- **70/30** : 3 clusters denses (Marché SE, Taverne W, Ruelle NE = couloir de flanc
  entre 2 maisons) + lanes ouvertes avec couverts épars.
- **Centre** : le Puits (×5, melee_pit) — combat rapproché autour.
- **Perchoir accessible** : 2 rochers en marches (×1.5 → ×2.5, walkable, sommet ~4,3 m).
- **AABB mesurés avant placement** (leçon story-625) : kit medieval = MINIATURES
  table-top (église 1,65 m !) → scales ×5-10 obligatoires, footprints commentés.

## Livré
13 pièces `[layouts.forge_sanctum]` (suppress_procedural_modules=true), roles
landmark/melee_pit/cover_high/cover_low/sniper_perch, blockers/walkables posés.
`validate-genomes` OK. Script AABB recréé (`scratchpad/aabb.py` — l'ancien avait péri
avec sa session ; à re-versionner un jour dans tools/).

## Reste (suites possibles)
- Éclairage/grading dédié par stage (le color_grading actuel est global Roguelite).
- Arbres (les CommonTree du pack nature ne sont pas des GLB adressables directs).
- Inc.3 story-646 : que le kind de la porte choisisse la salle/compos.
