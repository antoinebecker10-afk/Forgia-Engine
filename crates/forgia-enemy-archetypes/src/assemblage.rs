//! Assembler un ennemi — **un seul chemin, pour toutes les zones**.
//!
//! # Pourquoi ce module existe
//!
//! Un ennemi de Forgia n'est pas une entité, c'en est **quatre** :
//!
//! ```text
//! parent   Health · ArenaBot · BotShootConfig · TargetCube · défense · plaque de nom
//!  ├─ body      capsule Sensor           → la zone de corps du tir hitscan
//!  ├─ visual    SceneRoot KayKit         → tourné de PI, descendu aux pieds
//!  └─ tête      sphère Sensor HitZone    → DÉPASSE la capsule, sinon zéro headshot
//! ```
//!
//! Chacune de ces quatre pièces porte un piège déjà payé une fois, et écrit
//! quelque part dans l'historique du dépôt :
//!
//! - le `Sensor` du corps (story-517) — sans lui le joueur bute dans les ennemis
//!   au corps-à-corps ; avec un collider plein, le hitscan marche mais le jeu
//!   colle ;
//! - la **rotation de PI** du visuel — KayKit regarde vers +Z, Bevy vers −Z. Sans
//!   elle, tous les ennemis courent à reculons ;
//! - le **décalage vertical** `-(demi_hauteur + rayon)` — le pivot KayKit est au
//!   sol, le pivot de la capsule au centre : sans ça, ils lévitent ;
//! - la sphère de tête qui **dépasse** la capsule (story-652) — enfermée dedans,
//!   elle n'est jamais le premier point touché et le headshot n'existe pas ;
//! - `foot_offset_m()` — **la même source** que celle de l'`ArenaBot`, sinon le
//!   suivi de sol et le spawn divergent et l'ennemi s'enterre.
//!
//! Recopier ça dans une seconde zone, c'est recopier cinq occasions de se
//! tromper — et n'en corriger qu'une le jour où l'une casse. D'où cette
//! fonction : **l'Expédition et l'Abîme montent le même ennemi.**
//!
//! # Ce que ce module laisse à la zone
//!
//! Il ne décide **ni où ni quand**. La position arrive toute faite, et les
//! marqueurs de cycle de vie (`DespawnOnExit`, marqueur de run, observateurs de
//! debug) s'ajoutent sur l'entité rendue. Une zone garde donc son propre
//! nettoyage, sa propre progression et son propre butin.

use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, RigidBody, Sensor};
use forgia_combat::{Health, TargetCube};
use forgia_damage::{DefenseLayer, HitZone, HitZoneTag, Mortal};

use crate::head_hitbox::{head_local_y_bind, head_radius, HeadProxy};
use crate::{skeleton_asset_path, skeleton_scale, EnemyArchetype, EnemyStatsConfig};

/// Ce qu'une zone doit fournir pour qu'un ennemi existe.
pub struct Ennemi<'a> {
    pub archetype: EnemyArchetype,
    /// Position au sol en X/Z. La hauteur des pieds est **dérivée** des stats —
    /// la zone n'a pas à la connaître, et ne peut donc pas la faire diverger.
    pub sol_xz: Vec2,
    /// Sol sous les pieds (m). Le plancher de l'arène est plat à 0 ; une carte
    /// autorée donne l'altitude de son point d'apparition.
    pub sol_y: f32,
    /// Nom lisible dans l'inspecteur et les journaux. Préfixer par la zone.
    pub nom: &'a str,
    /// Couche défensive (bouclier / armure) si la zone en a une. `None` = la
    /// Vie seule encaisse.
    pub defense: Option<DefenseLayer>,
}

/// Monte les quatre entités et rend le **parent**.
///
/// La zone y ajoute ensuite ce qui lui appartient :
///
/// ```ignore
/// let e = assembler(&mut commands, &assets, &stats, Ennemi { .. });
/// commands.entity(e).insert((MonMarqueurDeZone, DespawnOnExit(GameMode::MaZone)));
/// ```
pub fn assembler(
    commands: &mut Commands,
    asset_server: &AssetServer,
    stats_cfg: &EnemyStatsConfig,
    e: Ennemi<'_>,
) -> Entity {
    let stats = stats_cfg.for_archetype(e.archetype);
    let echelle = skeleton_scale(e.archetype);
    let tete_y = head_local_y_bind(stats.capsule_half_height, stats.capsule_radius, echelle);
    let tete_r = head_radius(stats.head_radius, echelle);
    // `#Scene0` : la convention Bevy pour la racine de scène d'un glTF.
    let squelette: Handle<Scene> =
        asset_server.load(format!("{}#Scene0", skeleton_asset_path(e.archetype)));

    let parent = commands
        .spawn((
            Name::new(e.nom.to_owned()),
            e.archetype,
            TargetCube,
            // MÊME source que le `foot_offset_m` de l'`ArenaBot` monté juste en
            // dessous : le suivi de sol repose sur leur égalité.
            Transform::from_xyz(e.sol_xz.x, e.sol_y + stats.foot_offset_m(), e.sol_xz.y),
            RigidBody::KinematicPositionBased,
            Health::new(stats.hp),
            Mortal,
            stats_cfg.arena_bot(e.archetype),
            // Sans `BotShootConfig`, un ennemi avance et ne tire jamais (story-517).
            stats_cfg.bot_shoot(e.archetype),
            crate::anim::EnemyLocoSample::default(),
            // Au-dessus du CRÂNE réel, pas du haut de la capsule : celle-ci
            // s'arrête aux épaules (story-644 / 652).
            forgia_enemy_nameplate::NameplateAnchor(tete_y + tete_r + 0.35),
            forgia_effects::prelude::AscendsOnDeath {
                tint: e.archetype.ascension_tint(),
            },
        ))
        .id();

    if let Some(couche) = e.defense {
        commands.entity(parent).insert(couche);
    }

    // Corps — `Sensor` et non collider plein : le joueur doit pouvoir traverser
    // un ennemi au contact, pendant que le rayon du tir le détecte quand même
    // (`QueryFilter::default` n'exclut pas les sensors).
    commands.spawn((
        Name::new(format!("{}_corps", e.nom)),
        ChildOf(parent),
        Transform::default(),
        Collider::capsule_y(stats.capsule_half_height, stats.capsule_radius),
        Sensor,
    ));

    // Visuel — rotation de PI (KayKit regarde +Z, Bevy −Z) et descente aux pieds.
    commands.spawn((
        Name::new(format!("{}_visuel", e.nom)),
        ChildOf(parent),
        SceneRoot(squelette),
        Transform::from_xyz(0.0, -(stats.capsule_half_height + stats.capsule_radius), 0.0)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI))
            .with_scale(Vec3::splat(echelle)),
    ));

    // Tête — sphère qui DÉPASSE la capsule, recollée sur l'os `head` du rig
    // animé par `head_hitbox::sys_bind_head_joints`.
    commands.spawn((
        Name::new(format!("{}_tete", e.nom)),
        ChildOf(parent),
        Transform::from_xyz(0.0, tete_y, 0.0),
        Collider::ball(tete_r),
        Sensor,
        HitZoneTag(HitZone::Head),
        HeadProxy {
            enemy_root: parent,
            joint: None,
        },
    ));

    parent
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La hauteur de spawn se dérive des stats, jamais d'un littéral.
    ///
    /// Le jour où une zone recopie « y = 0.9 » au lieu d'appeler
    /// `foot_offset_m()`, ses ennemis s'enterrent ou flottent d'un demi-corps —
    /// et seulement chez elle, ce qui est le plus long à diagnostiquer.
    #[test]
    fn la_hauteur_des_pieds_vient_des_stats() {
        let cfg = EnemyStatsConfig::default();
        for a in [
            EnemyArchetype::Tank,
            EnemyArchetype::Runner,
            EnemyArchetype::Sniper,
            EnemyArchetype::Boss,
        ] {
            let s = cfg.for_archetype(a);
            assert!(
                s.foot_offset_m() > 0.0,
                "{a:?} : un decalage de pieds nul enterre le corps"
            );
            assert_eq!(
                s.foot_offset_m(),
                cfg.arena_bot(a).foot_offset_m,
                "{a:?} : le spawn et le suivi de sol doivent lire la MEME valeur"
            );
        }
    }

    /// La sphère de tête doit dépasser la capsule, sinon le headshot n'existe pas.
    #[test]
    fn la_tete_depasse_la_capsule() {
        let cfg = EnemyStatsConfig::default();
        for a in [
            EnemyArchetype::Tank,
            EnemyArchetype::Runner,
            EnemyArchetype::Sniper,
        ] {
            let s = cfg.for_archetype(a);
            let e = skeleton_scale(a);
            let haut_capsule = s.capsule_half_height + s.capsule_radius;
            let haut_tete = head_local_y_bind(s.capsule_half_height, s.capsule_radius, e)
                + head_radius(s.head_radius, e);
            assert!(
                haut_tete > haut_capsule,
                "{a:?} : tete {haut_tete:.3} enfermee dans la capsule {haut_capsule:.3}"
            );
        }
    }
}
