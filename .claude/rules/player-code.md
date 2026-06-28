---
paths:
  - "**/player/**"
---

# Player Code Rules (Forgia)

- Controleur: RigidBody::KinematicPositionBased + KinematicCharacterController
- Camera WoW-style: over-the-shoulder, shoulder_offset, collision 2 raycasts (L3)
- Camera collision: 1 raycast/frame + lerp asymetrique (push_time/recover_time depuis FpsTuning)
- Axes: +X droite, +Y haut, -Z avant
- GLB Blender: forward inverse 180deg, correction = ajouter PI au yaw
- Input mapping: leafwing-input-manager, clavier AZERTY
- Systemes dans GameSet::Input ou GameSet::Movement selon le role
