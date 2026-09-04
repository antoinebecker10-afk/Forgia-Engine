//! Terrain-side collision group bitflags — kept in sync with
//! `forgia-game/src/components/collision.rs`.
//!
//! Certifié zone propre story-349 E1 : 2 tests couvrent la distinction des
//! bits et l'inclusion de tous les groupes dans le filtre world.

use bevy_rapier3d::prelude::*;

pub const COLLISION_GROUP_PLAYER: Group = Group::GROUP_1;
pub const COLLISION_GROUP_WORLD: Group = Group::GROUP_2;
pub const COLLISION_GROUP_PROJECTILE: Group = Group::GROUP_3;
pub const COLLISION_GROUP_TARGET: Group = Group::GROUP_4;
pub const COLLISION_GROUP_GOBLIN: Group = Group::GROUP_5;

pub fn world_collision_groups() -> CollisionGroups {
    CollisionGroups::new(
        COLLISION_GROUP_WORLD,
        COLLISION_GROUP_PLAYER
            | COLLISION_GROUP_PROJECTILE
            | COLLISION_GROUP_TARGET
            | COLLISION_GROUP_GOBLIN,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collision_groups_are_distinct() {
        let groups = [
            COLLISION_GROUP_PLAYER,
            COLLISION_GROUP_WORLD,
            COLLISION_GROUP_PROJECTILE,
            COLLISION_GROUP_TARGET,
            COLLISION_GROUP_GOBLIN,
        ];
        for (i, a) in groups.iter().enumerate() {
            for (j, b) in groups.iter().enumerate() {
                if i == j {
                    continue;
                }
                assert!(
                    (a.bits() & b.bits()) == 0,
                    "group {i} bits {:#b} overlaps group {j} bits {:#b}",
                    a.bits(),
                    b.bits()
                );
            }
        }
    }

    #[test]
    fn world_filter_includes_all_other_groups() {
        let cg = world_collision_groups();
        let filter_bits = cg.filters.bits();
        for (name, g) in [
            ("PLAYER", COLLISION_GROUP_PLAYER),
            ("PROJECTILE", COLLISION_GROUP_PROJECTILE),
            ("TARGET", COLLISION_GROUP_TARGET),
            ("GOBLIN", COLLISION_GROUP_GOBLIN),
        ] {
            assert!(
                (filter_bits & g.bits()) != 0,
                "world filter dropped {name} — collisions will be silently skipped"
            );
        }
        assert_eq!(cg.memberships, COLLISION_GROUP_WORLD);
    }
}
