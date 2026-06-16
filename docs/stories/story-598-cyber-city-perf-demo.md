# Story-598 — Cyber City perf demo (3P Rex walkable)

**Statut** : DONE (rendu validé runtime 2026-06-16 ; polish en cours)
**Scale** : Standard (multi-crates)
**Track** : FORGE (banc d'essai moteur) — stress-test rendu/VRAM sur GLB lourd

## Objectif

Entrée de menu "Cyber City démo" qui charge un GLB lourd (`cyberpunk_city.glb`,
~185 Mo, ~1815 meshes) pour stress-tester le moteur, parcourable **en 3e
personne avec Rex animé**.

## Livré

- `GameMode::CyberCity` (forgia-core).
- `forgia-game/src/cyber_city.rs` : plugin self-contained — scène GLB scalée ×10
  + colliders `AsyncSceneCollider{TriMesh}` (rues/murs walkable), placement
  joueur **au coin du périmètre** via AABB (caméra dégagée), éclairage + brume
  + ambiante (luminosité montée 2026-06-16 après retour user « trop sombre »).
- Réutilisation du pipeline Rex 3P du RPG via élargissement de gating
  `rex_third_person_active = Rpg || CyberCity` (forgia-rpg), sans dupliquer la
  locomotion mono-perso ni spawner le monde RPG.
- Anti-clip caméra orbitale (forgia-camera-orbit) : raycast Rapier dans
  `orbit_follow` → la caméra ne traverse plus sol/murs (bénéficie aussi au RPG).
- Bouton menu (forgia-ui).

## Décisions / pièges

- **Hdr + Bloom retirés** : posés après-coup sur la caméra orbitale, ils
  cassaient la passe principale (écran ClearColor nu). À ré-introduire en
  activant HDR **à la création** de la caméra (cf audit B/A2).
- **Asset 185 Mo non commité** dans git (bloat). Reste local ;
  `assets/models/environment/cyberpunk_city.glb`.

## Audit / pistes (2026-06-16)

Voir le récap d'audit : B1 filtrage anisotrope (P0), A2 bloom néon HDR-at-creation
(P1), B2 KTX2 VRAM (P1), D3 vrai contrôle 3P (P1), C1 draw calls (mesure F2),
E1 CITY_SCALE en genome (no-hardcode).
