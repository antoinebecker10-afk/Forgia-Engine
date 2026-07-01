# Story-637 — Anim procédurale : multi-perso + auto-pose + fallback topologie

> **Statut** : 🟡 IN_PROGRESS (créée 2026-06-30).
> **Niveau BMAD** : Standard (3 crates, 5 fichiers). **Track** : FORGE (outils RPG).

## Contexte
Les crates d'anim procédurale (animer un mesh **sans** animation bakée) étaient
inutilisables au-delà d'un perso de test (Rex) : `.single()` partout (1 seul animé),
A/T-pose non auto-détecté (caller choisit le template à la main), lookup de bones
enfants par noms hardcodés sans fallback (timeout 120 frames sur rigs non-template).
Quick wins issus de l'anim KayKit (story-636) : driver par-entité + itération multi-perso.

## Objectif
Rendre `forgia-auto-rig` + `forgia-anim-locomotion` utilisables sur N meshes auto-riggés
arbitraires. Validé sur **Cyber** (Cyber_lod.glb = 55k verts, 0 skin/0 anim = mesh nu) +
lineup village (multi-perso). Reframe « retargeting de clips bakés » = hors scope.

## Implémentation (3 quick wins)
- **QW1 multi-perso** : `LocomotionDriver(Entity)` (forgia-anim-locomotion) lie chaque
  target à son driver (Player pour Rex, soi-même pour un NPC). `procedural_locomotion`
  itère les targets (kill double `.single()`) ; capteurs/foot_ik → `.iter().next()`.
  `character.rs` : driver sur Rex/Cyber. **Le moteur supporte N persos** (capacité livrée+testée) ;
  animer le lineup PNJ = follow-up (cf qa-lead BUG-07 : observabilité encore mono-target).
- **QW2 A/T-pose auto** : variant `AutoRigTemplate::HumanoidAuto` ; pinocchio résout
  Humanoid (T) vs HumanoidApose (A) vs BipedLizard via `arm_span_half_frac` (seuil 0.30) +
  `looks_humanoid`. Cyber → `HumanoidAuto` (doit résoudre HumanoidApose tout seul).
- **QW3 fallback topologie** : `RigTopology` étendu (forearm/hand/shin/foot/neck dérivés
  par chaîne, `linear_child` = enfant au + de descendants). locomotion : `lookup("X").or(topo.child)`
  (name-priority préservée → rigs template inchangés ; rigs étrangers résolvent).

## Acceptance criteria
- [ ] `.single()` éliminé : le moteur supporte N targets (boucle + `LocomotionDriver`), testé.
- [ ] Cyber s'auto-rig en HumanoidApose **détecté** (log `HumanoidAuto → HumanoidApose`).
- [ ] Rex **non régressé** (marche + queue identiques).
- [ ] Rigs aux noms non-template : forearm/hand/neck résolus par topologie (chaîne).
- [ ] `cargo clippy` 4 crates : 0 warning ; tests purs verts (dont test dérivation enfants).

## Limitations connues (qa-lead, à tracer)
- **BUG-03** : `linear_child(left_leg)` suppose `topo.left_leg = thigh`. Si le scoring le
  classe sur le PIED (bug 2026-06-03), la dérivation shin/foot est fausse pour les rigs SANS
  noms template. Aucune régression sur rigs template (name-priority `.or()`). Fix = story scoring jambe.
- **BUG-04** : `procedural_whole_body_anim` (bob/lean root du Player) reste `.single()` (Player-only
  by design). Pas une régression ; dette symétrie multi-perso si coop split-screen un jour.
- **BUG-07** (résolu par scope) : capteurs Rex (`rex_bones_live`/`walk_pose`/`anim_full`) lisent
  le 1er target → mono-target. Avec N targets ils seraient ambigus → animer le lineup PNJ est
  reporté tant que l'observabilité n'est pas multi-target.

## Notes / risques
- Risque = casser Rex : driver=player + name-priority préservés → comportement identique.
- Corps de `procedural_locomotion` sous-indenté (diff minimal, `bones` reste owned) — cosmétique.
- Cap skinning 200k non concerné (Cyber_lod 55k). Surfacer le silent-skip = hors scope.

## Hors scope (chantiers suivants)
- Retargeting de clips bakés (squelette → noms Mixamo + `AnimationTarget` + AnimationPlayer).
- Surfacer silent-skip skinning >200k + LOD auto. Skinning Phase 2 (heat-diffusion).

## Cross-refs
- [[reference_baked_gltf_anim_via_bevy_player]] (story-636, pattern multi-perso/driver).
- `concept-first.md` étape 0 ; `no-hardcode.md` (seuil morpho = const exempt).
