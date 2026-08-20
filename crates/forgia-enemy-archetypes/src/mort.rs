//! enemy_death.rs — la mort d'un ennemi roguelite (2026-08-05).
//!
//! Avant : `despawn_dead_cubes` (forgia-fps) faisait tout — `DeathEvent` puis
//! `try_despawn()` dans la même frame. Le corps disparaissait sèchement.
//!
//! Maintenant : les ennemis roguelite portent `AscendsOnDeath`, ce qui les
//! exclut de ce balayage. **Ce module devient leur seul point d'entrée de
//! mort** : il émet `DeathEvent` (le butin en dépend), retire ce qui doit
//! cesser, puis lance l'envol de `forgia-effects::death_ascension`.
//!
//! ## Pourquoi le strip vit ICI et pas dans forgia-effects
//!
//! Seul le mode connaît ses composants. `forgia-effects` ne sait rien d'`ArenaBot`
//! ni de `TargetCube` — s'il tentait de les retirer, il faudrait qu'il dépende
//! des crates de gameplay, et la dépendance partirait à l'envers.
//!
//! ## Ce qui doit cesser, et pourquoi (chacun a une raison)
//!
//! - `ArenaBot` / `BotShootConfig` : sans ça l'IA continue de piloter le corps
//!   horizontalement et **combat la montée** — le cadavre partirait en biais.
//! - `TargetCube` : c'est le filtre de TOUTES les requêtes de combat (visée,
//!   dégâts, défense). Le retirer rend le corps intraversable par le jeu — les
//!   balles ne se « perdent » plus dedans pendant son envol.
//! - `RigidBody` : le corps n'est plus une entité physique, il est piloté en
//!   `Transform` (exactement comme en V1).
//! - `NameplateAnchor` : une plaque de nom qui suit un corps qui monte au ciel
//!   se lit comme un bug.
//! - Les enfants porteurs de `Collider` sont despawnés : sinon la capsule reste
//!   plantée au sol pendant que le corps s'envole.

use bevy::prelude::*;
use forgia_core::prelude::{a_du_combat, GameSet};
use bevy_rapier3d::prelude::Collider;
use forgia_combat::Health;
use forgia_damage::{DamageKind, DeathEvent};
use forgia_effects::prelude::{Ascending, AscendsOnDeath, DeathAscensionStats, DeathAscensionTuning};

use crate::EnemyArchetype;

/// Détecte les ennemis morts qui doivent s'envoler et lance leur ascension.
///
/// `Without<Ascending>` est le garde anti-répétition : sans lui, l'entité
/// resterait à HP 0 et ré-émettrait `DeathEvent` à chaque frame — donc du butin
/// en boucle. C'est la même classe de piège que le `Dying` du V1.
pub fn sys_start_death_ascension(
    mut commands: Commands,
    tuning: Res<DeathAscensionTuning>,
    mut stats: ResMut<DeathAscensionStats>,
    q_dead: Query<
        (Entity, &Transform, &Health),
        (
            With<AscendsOnDeath>,
            With<EnemyArchetype>,
            Without<Ascending>,
        ),
    >,
    q_children: Query<&Children>,
    q_is_collider: Query<(), With<Collider>>,
) {
    for (entity, xf, hp) in &q_dead {
        if !hp.is_dead() {
            continue;
        }

        // 1) Le butin d'abord : l'observer lit `Transform` + `EnemyArchetype`,
        //    tous deux conservés. Émis AVANT les retraits, dans la même file.
        commands.trigger(DeathEvent {
            target: entity,
            source: None,
            final_kind: DamageKind::Physical,
        });

        // 2) Ce qui doit cesser (cf. en-tête du module).
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<forgia_combat::TargetCube>();
            ec.remove::<forgia_ai_arena_bot::ArenaBot>();
            ec.remove::<forgia_ai_arena_bot::BotShootConfig>();
            ec.remove::<bevy_rapier3d::prelude::RigidBody>();
            ec.remove::<forgia_enemy_nameplate::NameplateAnchor>();
        }

        // 3) Les colliders enfants — sinon la capsule reste au sol.
        if let Ok(children) = q_children.get(entity) {
            for child in children.iter() {
                if q_is_collider.get(child).is_ok() {
                    if let Ok(mut ec) = commands.get_entity(child) {
                        ec.try_despawn();
                    }
                }
            }
        }

        // 4) L'envol. La rotation et le Y de départ sont LUS sur la pose de
        //    mort : un ennemi mort en pente ou de dos doit basculer depuis là
        //    où il est, pas depuis une pose supposée.
        //    `try_insert` et non `insert` : un observer de `DeathEvent` peut
        //    despawner la cible pendant le flush de cette même file. Le garde
        //    d'`on_bot_death` couvre le cas connu ; celui-ci couvre les futurs —
        //    c'est la convention du codebase pour toute écriture sur une entité
        //    qui vient de mourir (cf. `elements.rs`, `ultimate_apply.rs`).
        commands.entity(entity).try_insert(Ascending::new(
            xf.rotation,
            xf.translation.y,
            xf.translation,
            tuning.fall_secs,
        ));
        stats.started_total = stats.started_total.saturating_add(1);
    }
}

/// Monte la mort d'un ennemi.
///
/// # Pourquoi ce plugin existe, et ce qu'il a coute
///
/// Ce systeme vivait dans la ZONE Arene, gate `in_state(GameMode::Roguelite)`.
/// Son propre commentaire disait deja : « si ce systeme ne tourne pas, plus
/// AUCUN ennemi ne meurt » — parce que `AscendsOnDeath`, pose par
/// `assemblage::assembler`, EXCLUT l'ennemi du balayage de `forgia-fps`.
///
/// Resultat mesure en jeu le 2026-08-18 : les 11 ennemis nes dans l'Expedition
/// etaient **immortels**. Pas par oubli d'une mecanique — par construction : une
/// brique posait un composant dont le seul consommateur etait gate sur une autre
/// zone. C'est la cinquieme occurrence du meme defaut dans la journee.
///
/// La mort appartient donc a l'ennemi, comme son animation et sa hitbox de tete.
/// Gate sur la CAPACITE « cette zone se bat », plus sur un nom de zone.
pub struct EnemyDeathPlugin;

impl Plugin for EnemyDeathPlugin {
    fn build(&self, app: &mut App) {
        // Dans `GameSet::Combat` : apres les degats de la frame, avant les effets.
        app.add_systems(
            Update,
            sys_start_death_ascension
                .in_set(GameSet::Combat)
                .run_if(a_du_combat),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le contrat qui compte : un ennemi mort ne doit produire son butin
    /// qu'UNE fois, même si le système repasse plusieurs frames de suite.
    ///
    /// On le vérifie sur le filtre lui-même (`Without<Ascending>`) plutôt que
    /// sur un compteur runtime : c'est le filtre qui porte l'invariant, et un
    /// test qui l'encode empêche qu'on le retire par mégarde.
    #[test]
    fn ascending_bodies_are_excluded_from_the_death_sweep() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<DeathAscensionStats>();
        app.insert_resource(DeathAscensionTuning::default());
        app.add_systems(Update, sys_start_death_ascension);

        // Un corps déjà en train de s'envoler : mort, marqué, ET `Ascending`.
        let already = app
            .world_mut()
            .spawn((
                Transform::default(),
                Health::new(10.0),
                AscendsOnDeath::default(),
                EnemyArchetype::Runner,
                Ascending::new(Quat::IDENTITY, 0.0, Vec3::ZERO, 0.3),
            ))
            .id();
        app.world_mut().get_mut::<Health>(already).unwrap().current = 0.0;

        app.update();

        // Il n'a pas été recompté : le filtre l'a bien exclu.
        assert_eq!(
            app.world().resource::<DeathAscensionStats>().started_total,
            0,
            "un corps déjà en ascension ne doit jamais redéclencher sa mort"
        );
    }

    #[test]
    fn a_living_enemy_is_left_alone() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<DeathAscensionStats>();
        app.insert_resource(DeathAscensionTuning::default());
        app.add_systems(Update, sys_start_death_ascension);

        let alive = app
            .world_mut()
            .spawn((
                Transform::default(),
                Health::new(10.0),
                AscendsOnDeath::default(),
                EnemyArchetype::Tank,
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().resource::<DeathAscensionStats>().started_total,
            0
        );
        assert!(
            app.world().get::<Ascending>(alive).is_none(),
            "un ennemi vivant ne s'envole pas"
        );
    }

    #[test]
    fn each_archetype_has_its_own_trail_colour() {
        // La teinte est le seul moyen de lire QUI vient de mourir sans avoir vu
        // le kill : deux archétypes ne doivent pas partager la même.
        let tints = [
            EnemyArchetype::Tank,
            EnemyArchetype::Runner,
            EnemyArchetype::Sniper,
            EnemyArchetype::Boss,
        ]
        .map(|a| a.ascension_tint());
        for i in 0..tints.len() {
            for j in (i + 1)..tints.len() {
                assert_ne!(
                    (tints[i].red, tints[i].green, tints[i].blue),
                    (tints[j].red, tints[j].green, tints[j].blue),
                    "deux archétypes partagent la même traînée"
                );
            }
        }
    }
}

