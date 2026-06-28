---
paths:
  - "**/editor/**"
  - "**/gamemode/**"
---

# Editor & GameMode Code Rules (Forgia)

- EditorRaycast centralise: 1 raycast/frame via EditorRaycastResult, partage par tous les outils (L4)
- toggle_editor_effects: .run_if(resource_changed::<EffectsConfig>), ecritures conditionnelles (L6)
- AppMode cycle: Play → Build → Edit → Play via TAB
- Colliders editeur: utiliser collider_config() et terrain_embed_offset() des modeles, jamais de colliders generiques
- Buildings s'enfoncent (-0.3), ennemis marchent sur le sol (0.0)
- Verifier conflits de touches avant toute assignation (V=StylePicker, F=WISP)
