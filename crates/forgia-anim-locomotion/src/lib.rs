//! # forgia-anim-locomotion
//!
//! Animation procédurale locomotion (walk cycle anatomique, pose-agnostic compose).
//! Story-482 P1 extraction depuis `forgia-rpg/character.rs`.
//!
//! ## Architecture
//!
//! - **Pose-agnostic** : bind rotations capturées à `cache.ready = true`, anim
//!   COMPOSE avec bind au lieu d'OVERWRITE (story-481 Action 2).
//! - **Découplage driver/target** : `LocomotionState` Component sur l'entité qui
//!   drive l'anim (position+vitesse), `LocomotionTarget` marker sur le character
//!   qui reçoit l'anim. Permet plusieurs characters animés par plusieurs drivers.
//! - **Pure functions séparées** : `proc_walk` module = math walk cycle testable
//!   sans Bevy World (12 unit tests anatomiques).
//!
//! ## Usage
//!
//! ```ignore
//! // Côté driver (Player) :
//! commands.entity(player).insert(LocomotionState::default());
//!
//! // Côté target (character avec mesh + skeleton) :
//! commands.entity(rex).insert((
//!     LocomotionTarget,
//!     LocomotionBoneCache::default(),
//!     ProcBodyAnim::default(),
//! ));
//!
//! // Côté plugin (forgia-rpg) :
//! app.add_systems(Update, (
//!     attach_locomotion_bones,
//!     procedural_locomotion,
//!     write_walk_pose_sensor,
//!     write_rex_bones_live_sensor,
//! ).in_set(GameSet::Movement).run_if(in_state(GameMode::Rpg)));
//! app.init_resource::<WalkPoseSensorTimer>();
//! app.init_resource::<RexBonesLiveSensorTimer>();
//! ```

pub mod foot_ik;
pub mod locomotion;
pub mod proc_walk;

pub use foot_ik::*;
pub use locomotion::*;
