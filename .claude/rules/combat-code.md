---
paths:
  - "**/combat/**"
---

# Combat Code Rules (Forgia)

- JAMAIS d'import direct depuis effects/ — decouplage via events obligatoire
- Toutes les valeurs gameplay dans FpsTuning (pas de magic numbers)
- Collision groups: Joueur (G1), Monde (G2), Projectiles (G3), Cibles (G4), Gobelins (G5)
- Systemes dans GameSet::Combat, jamais dans un autre set
- Projectiles: utiliser les collision groups G3/G4, pas de raycasts manuels sauf si justifie
- AnimationGraph + AnimationNodeIndex pour les animations (Bevy 0.18)
- Marqueur AnimationAutoStarted pour eviter capture des AnimationPlayers ennemis
