---
paths:
  - "**/ui/**"
---

# UI Code Rules (Forgia)

- bevy_egui en mode immediate — pas de state UI persistant hors resources Bevy
- Pas de game state direct dans l'UI — passer par des resources ou queries
- InputBlockers: toujours utiliser le bridge decouplage camera/editeur/grimoire/chat
- Systemes dans GameSet::UI (dernier de la chaine)
- Grimoire (touche G): toutes les constantes FpsTuning modifiables runtime
- Nameplate: LOD 10Hz + frustum, NameplateCache Local, desactive en PerfMode (L5)
- Minimap: MinimapCache Local, recalcul si mouvement >0.5m ou rotation >5deg (L8)

## Regle Escape Route — OBLIGATOIRE pour tout Window > 300x300px

Toute `egui::Window::new(...)` affichant un panneau utilisateur DOIT avoir les 3 echappatoires :

1. **`.open(&mut local_bool)`** — bouton X natif egui dans la titlebar.
   Pattern : `let mut open = state.open; ... .open(&mut open) ... ; if !open { state.open = false; }`
2. **Handler `KeyCode::Escape`** dans le toggle_system associe — ferme immediatement.
3. **`.movable(true)` + `.default_pos([x, y])`** — PAS d'`.anchor(...)` fixe. Le createur doit pouvoir deplacer la fenetre.

Bonus recommande : `.collapsible(true)` pour permettre de minimiser sans fermer.

Titre : inclure `"\u{2715} (Escape)"` pour indiquer visuellement comment fermer.

**Reference** : `ui/graph_editor.rs` (pattern complet valide 2026-04-17, Wave UX escape routes).

**Anti-trap** : un panneau sans ces 3 echappatoires est une regression UX bloquante pour le createur de 14 ans. Viole `creator-simplicity.md` ("comprehensible en 3 secondes").
