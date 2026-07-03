# Story-654 — Nameplate v2 : vie seule, plaques de bouclier/armure, icônes de statut

> **Statut** : IN_PROGRESS (validation visuelle user en attente)
> **Niveau BMAD** : Standard (5 fichiers)
> **Origine** : feedback user 2026-07-03 — « on ne doit voir que la vie [sur la barre], le reste ça doit ressembler à des plaques de bouclier comme dans d'autres jeux, et les dots doivent se voir comme une icône poison/feu/électrique à côté de la barre HP ». + « VFX trop opaques ».

## Design (langage visuel standard du genre)

1. **Barre principale = LA VIE SEULE** (rouge cartoon, fond noir, bord net — inchangée).
2. **Bouclier/Armure = PLAQUES segmentées** (façon Apex/DRG) : chaque plaque vaut N PV (`shield_segment_hp`/`armor_segment_hp` = 20, genome hot-reload) et **saute** quand la couche passe sous son seuil — « il reste 2 plaques bleues » se lit d'un coup d'œil. Tank : 3 plaques bleues + 4 jaunes. Boss : 8+8 (cap lisibilité).
3. **Statuts = ICÔNES à côté de la barre** (façon WoW/Gunfire) : flamme/nuage/étincelle (textures Kenney teintées élément) apparaissent à droite du nameplate quand le DoT s'applique, disparaissent à l'expiration — slots fixes feu/poison/élec. L'aura sur le corps reste l'ambiance ; l'icône est l'information.
4. **Flammes + poison en blending ADDITIF** — brillent au lieu de faire un blob opaque (feedback « trop opaque »).

## Implémentation

- `forgia-enemy-nameplate` : `NameplateShieldFill/ArmorFill` portent un `threshold` ; `build_nameplate_for` spawne les rangées de plaques (mesh partagé par rangée, seuils figés au spawn) ; `update_defense_bars` bascule la **Visibility** par plaque (plus de scale de barre). Genes `*_segment_hp` (tuning genome hot-reload).
- `forgia-mode-roguelite/status_vfx.rs` : `StatusIcon { target, kind }` + `spawn_status_icon` (quad texturé teinté, enfant du nameplate root → billboard/despawn avec lui) branché dans les 3 attach ; despawn ciblé dans les 3 detach.
- `forgia-effects/status.rs` : `AlphaMode::Add` sur flamme + nuage.

## Acceptance criteria

- [x] Barre HP = vie seule ; plaques par couche avec seuils ; icônes par statut, slots fixes
- [x] Tout data-driven (segment_hp genome, tailles dérivées du nameplate — creator-simplicity : 0 slider nouveau exposé)
- [x] check + clippy (0 introduit) + build verts
- [ ] **Validation user** : plaques qui sautent lisibles, icônes visibles au DoT, flammes lumineuses non-opaques

## Suite

Icônes dédiées (vraies icônes dessinées vs textures particules) · nombre de stacks poison sur l'icône · effets projectiles (roquette/ultimes) sur les curseurs VFX.
