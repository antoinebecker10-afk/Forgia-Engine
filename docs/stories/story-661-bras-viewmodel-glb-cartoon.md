# Story 661 — Bras viewmodel GLB cartoon (remplace les poings procéduraux)

**Statut** : IN_PROGRESS
**Niveau BMAD** : Standard
**Créée** : 2026-07-19
**Audit source** : [audit-2026-07-19-bras-viewmodel-blender-cartoon.md](../audit/audit-2026-07-19-bras-viewmodel-blender-cartoon.md)
**Crates** : forgia-viewmodel (+ pipeline Blender hors-Rust dans `tools/blender/`)

---

## Contexte

Les bras first-person actuels sont procéduraux (capsules/sphères,
`crates/forgia-viewmodel/src/arms.rs`) et assument leur plafond (ligne 14 :
« procédural = stylisé. Réalisme poussé = mesh de mains riggé (asset) »).
L'audit 2026-07-19 conclut : le meilleur rendu pour la DA « cartoon rigolo »
= base CC0 riggée+animée existante, cartoonisée par script Blender (mains
exagérées, 4 doigts, gants de forgeron, palette d'aplats), exportée GLB.
Génération 3D IA écartée (mains = pire failure mode documenté).

## Décisions

- **Base** : PSX First Person Arms — Drillimpact (itch.io, **CC0**, GLB+Blend,
  ~17 anims, variante gants). Fallback : cartoon FPS Arms DJMaesen (CC-BY).
- **Pipeline** : scripts Blender 5.0 headless versionnés dans `tools/blender/`
  (reproductible ; PAS de retouche manuelle non scriptée).
- **DA** : gants de forgeron couleur signature, mains ×1,5–2, aplats
  palette (metallic 0/roughness 1), lisibilité silhouette (réf TF2/Roboquest).
- Le placement auto par-arme, le sway/bob procédural (FixedUpdate) et
  `update_arms_visibility` sont **conservés** — on ne remplace que les meshes.

## Incréments

### Inc.0 — Pipeline Blender + asset cartoonisé (cette story, hors Rust)
- [x] Base téléchargée dans `assets/models/arms/src/drillimpact/` + licence tracée (LICENSE.txt, CC0 vérifiée sur la page itch)
- [x] `tools/blender/inspect_glb.py` — inspection (rig/anims/tris/matériaux)
- [x] `tools/blender/cartoonize_arms.py` — mains ×1,6 (mesh+bones, falloff poids), gants cuir caramel plats, peau pêche 3 tons, matériau flat
- [x] Export `assets/models/arms/fps_arms_cartoon.glb` (732 Ko — 1 176 tris, 52 bones, 18 anims, texture aplats embarquée ; vérifié par ré-inspection : ni couteau ni objets de démo)
- [ ] Rendus preview (`assets/models/arms/previews/`) validés par Antoine AVANT intégration Rust

**Piège documenté (2026-07-19)** : le .blend source a « X-Axis Mirror » actif →
toute édition scriptée d'un bone `.L` est répliquée sur le `.R` ; nos passes L
puis R composaient un ×S² (mesh déchiqueté). Fix : `arm.data.use_mirror_x = False`
avant `mode_set(EDIT)`. Diag par prints étagés (mesh ×1,6 OK vs bones ×2,56 = 1,6²).
Autres pièges : preview Blender en AgX = albedos délavés → `view_transform =
"Standard"` pour juger les couleurs ; `pose_position = 'REST'` pour un rendu bind
pose (vider l'action ne reset PAS la pose) ; le .blend contient `knife_dummy`
(parenté `handIK.R`), cubes de démo et widgets WGT-* → export `use_selection`
strict mesh+armature.

### Inc.1 — Swap statique dans forgia-viewmodel (livré 2026-07-20, en test)
- [x] Pipeline : exports **par-côté** `fps_arm_L/R.glb` (588 tris/côté, 27 bones,
      normalisés convention procédurale : poignet origine, avant-bras -Y, doigts +Y,
      paume +Z, pouce ±X → `position_hands` inchangé)
- [x] `spawn_arms` : branche GLB (1 SceneRoot par main) + **toggle A/B hot-reload**
      `use_glb` (despawn/respawn au changement) ; procédural conservé en fallback
- [x] Tuning : `use_glb` + `glb_scale` (fps_tuning.toml [viewmodel_arms] →
      FtViewmodelArms → sync hot-reload) ; taille baked déterministe par le
      pipeline (mètres réels) + échelle TOML = pas de scale hardcodé
- [x] Capteur `forgia2_viewmodel_arms.json` (1 Hz, `arms_sensor.rs`) : mode actif,
      états de chargement des 2 GLB, alerte critical si GLB failed (gate
      observability-required — livré suite audit qa-lead). 4 tests unitaires.
- [ ] Mapping `ArmCosmetics` (Peau/Gantelet/Cyber) sur les matériaux GLB —
      **différé** : dormant en mode GLB (pas d'ArmMaterialHandles inséré), le
      look vient de la texture aplats ; story suiveuse si besoin
- [x] **Calibration placement (2026-07-20, retour screenshot user)** : outil
      WYSIWYG `tools/blender/preview_ingame.py` (reproduit genome+tuning+autoscale
      +FOV, vues player/side/ghost/hands-only + marqueurs) → itéré 12× offline.
      Résultat : **mains-only** (mitaines ×1,35, avant-bras coupé — kinks
      inalignables), origine = centre paume, pose `grab@12` (fermée) bakée en
      rest, ROLL par-main baké (R 180°/L 0°), ancres TOML recalibrées sur la
      silhouette (grip_x .12/grip_drop -.15/grip_back .14 ; barrel_x -.12/
      barrel_drop -.18/barrel_fwd .18). GLB par-côté SANS anims (invalidées par
      le re-rest ; piège importeur = action fantôme dans les previews).
- [x] **Retour user #2 (2026-07-20)** : avant-bras restaurés (coupe au COUDE —
      la coupe poignet « mitaines-only » était une sur-correction basée sur les
      previews menteuses ; en rest les avant-bras sont droits, pas de kink) +
      **cosmétiques Forge branchées sur GLB** (qa-lead #1 résolu proprement) :
      observer `on_arm_scene_ready` (`On<SceneInstanceReady>`, pattern
      enemy_rig_debug) clone le matériau GLB par main + `apply_arm_style_glb`
      (teinte normalisée par canal max = préserve la luminosité de la texture ;
      Peau/Gantelet métal/Cyber émissif) + `ArmGlbMaterial` consommé par
      `sync_arm_cosmetics` → le picker couleur+style du début de partie marche
      en GLB. Event-driven, zéro scan par-frame. Clippy 0 warning, 15 tests.
- [ ] Test in-game : les 2 mains suivent l'arme (hipfire + ADS + sniper hide)
      + le choix couleur/style Forge teinte les bras GLB

**Dette notée** : arms.rs = hotspot (~750 LOC, procédural + GLB + cosmétiques) →
split en module `arms_glb.rs` à faire APRÈS le merge de l'arbre de l'autre
terminal (refactor structurel maintenant = conflit garanti).

**Auto-QA 2026-07-20 (verifier + qa-lead)** — compile ✅ clippy -D warnings ✅
15 tests ✅ :
- ❌ **Faux positif verifier** : « despawn() ligne 462 = fuite d'entités, utiliser
  despawn_recursive() » — RÉFUTÉ mécaniquement : doc source bevy_ecs 0.18
  (`EntityCommands::despawn` : « this will recursively despawn Children ») ;
  `despawn_recursive` n'existe plus en 0.18. Aucun changement.
- 🟠 qa-lead #1 : picker Forge (styles bras, `identity.rs:331`) = no-op silencieux
  en mode GLB → **différé** (identity.rs claimé par l'autre terminal) ; options :
  masquer le picker si `use_glb`, ou tinter le matériau GLB via SceneInstanceReady,
  ou baker 3 variantes GLB. À traiter avant un build joueur où le picker est visible.
- 🟠 qa-lead #3 : observabilité absente → **corrigé** (capteur ci-dessus).
- 🟡 qa-lead #2 : `sys_wire_enemy_anim` (enemy_anim.rs:370) rescane à vie les
  AnimationPlayer des bras (jamais résolus en EnemyArchetype) — négligeable (2
  entités) mais à scoper **avant Inc.2** (marker d'exclusion ou bound-negative).
- 🔵 qa-lead #4 : gap 1 frame au toggle A/B (dev-only) + hypothèse multi-root
  non confirmée → vigilance via le capteur, pas de fix spéculatif.
- Confirmé par qa-lead : les 18 clips GLB ne s'auto-jouent PAS (pas
  d'AnimationGraph), visibilité sniper OK par propagation, hot path propre.

### Inc.2 — Animations bakées (draw/reload/inspect)
- [ ] `AnimationPlayer`/`AnimationGraph` sur le rig bras (pattern story-636)
- [ ] Hybride : bob/sway procédural conservé, clips bakés pour les gestes doigts

## Critères d'acceptance

- [ ] L'asset final est CC0-clean (licence tracée dans le repo)
- [ ] Rendu in-game jugé « cartoon rigolo » par Antoine (validation visuelle)
- [ ] 0 régression : placement par-arme, ADS, masquage sniper, cosmétiques
- [ ] 0 warning clippy ; capteur `forgia2_viewmodel_arms.json` étendu si besoin
- [ ] Pipeline rejouable : `blender --background --python tools/blender/cartoonize_arms.py` reproduit le GLB

## Risques

- Skinning de la base à qualité inconnue (→ inspection Inc.0 avant tout)
- Rendu viewmodel non toon-shadé (caméra séparée) → l'aplat doit « lire » sans
  la ramp ; sinon étendre le toon au viewmodel (story suiveuse)
- Multi-terminal : arms.rs est modifié non-commité (M dans git status) →
  coordonner avant Inc.1
