//! squelette_gizmo — VOIR le squelette, au lieu de lire ses quaternions.
//!
//! # Pourquoi ce module existe
//!
//! Le 2026-08-16, « les bras sont dans le dos » a coûté trois allers-retours et
//! une capture d'écran. Pendant ce temps, `forgia_bone_trace.json` écrivait
//! 25 Ko de positions et de rotations toutes les deux secondes.
//!
//! Les deux mesuraient la même chose. Une seule était lisible en une seconde.
//!
//! Un relevé se **cite** dans un diagnostic — c'est irremplaçable. Mais il ne se
//! **regarde** pas : personne ne reconstruit une posture de mémoire à partir de
//! douze triplets d'angles d'Euler. Les deux sont nécessaires, et il n'y avait
//! que le premier.
//!
//! # Ce qui est dessiné
//!
//! - un segment par os, du joint vers son parent : c'est la silhouette ;
//! - un repère (rouge/vert/bleu = X/Y/Z **locaux**) sur les os **surveillés** —
//!   c'est ce qui répond à « dans quel sens tourne cet os », la question que les
//!   angles d'Euler posent sans y répondre ;
//! - la cible des mains quand on vise, si un système en publie une.
//!
//! Les os surveillés sont les mêmes que ceux du relevé (`bone_trace`) : une
//! seule liste, un seul endroit à changer.

use bevy::mesh::skinning::SkinnedMesh;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use forgia_core::prelude::*;

use crate::bone_trace::BoneTraceConfig;

/// La touche qui montre les os. F3 : libre dans tout le projet (vérifié), et
/// c'est la convention des moteurs du marché pour un affichage de debug.
const TOUCHE: KeyCode = KeyCode::F3;

/// Longueur des repères d'axes, en fraction de l'os qu'ils décorent. Un repère
/// de taille fixe est illisible : minuscule sur une cuisse, énorme sur une
/// phalange.
const AXE_FRACTION: f32 = 0.45;

/// Rayon sous lequel un os est jugé « sans longueur » et ne reçoit pas d'axes —
/// sinon les vingt phalanges d'une main noient la silhouette.
const OS_MINIMUM_M: f32 = 0.02;

#[derive(Resource, Default)]
pub struct SqueletteVisible(pub bool);

/// Une cible que n'importe quel système peut publier pour la voir dessinée.
///
/// C'est le point que l'IK de visée cherche à atteindre. Le dessiner à côté de
/// la main **transforme une question en évidence** : « la main est-elle où on
/// lui demande » se répond alors sans capteur, sans angle, sans capture d'écran.
#[derive(Resource, Default)]
pub struct CiblesDebug(pub Vec<(Vec3, Color)>);

pub struct SqueletteGizmoPlugin;

impl Plugin for SqueletteGizmoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SqueletteVisible>()
            .init_resource::<CiblesDebug>()
            .add_systems(Update, (basculer, dessiner).chain().in_set(GameSet::Sensors));
    }
}

fn basculer(touches: Res<ButtonInput<KeyCode>>, mut visible: ResMut<SqueletteVisible>) {
    if touches.just_pressed(TOUCHE) {
        visible.0 = !visible.0;
        info!(
            "[squelette] affichage des os {}",
            if visible.0 { "ON" } else { "OFF" }
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn dessiner(
    visible: Res<SqueletteVisible>,
    cfg: Res<BoneTraceConfig>,
    cibles: Res<CiblesDebug>,
    mut gizmos: Gizmos,
    q_skinned: Query<&SkinnedMesh>,
    q_global: Query<&GlobalTransform>,
    q_parent: Query<&ChildOf>,
    q_name: Query<&Name>,
) {
    if !visible.0 {
        return;
    }

    // Les squelettes, dédupliqués : le chien porte huit maillages skinnés qui
    // partagent la même liste de joints. Les dessiner huit fois coûterait huit
    // fois le prix pour exactement la même image.
    let mut vus: HashSet<Entity> = HashSet::default();
    for skinned in q_skinned.iter() {
        let Some(&racine) = skinned.joints.first() else {
            continue;
        };
        if !vus.insert(racine) {
            continue;
        }

        for &os in &skinned.joints {
            let Ok(gt) = q_global.get(os) else { continue };
            let p = gt.translation();

            // Le segment vers le parent : c'est lui qui fait la silhouette.
            // Un nuage de points ne se lit pas ; une chaîne, oui.
            let longueur = q_parent
                .get(os)
                .ok()
                .and_then(|par| q_global.get(par.parent()).ok())
                .map(|pgt| {
                    let pp = pgt.translation();
                    gizmos.line(pp, p, Color::srgb(0.35, 0.85, 1.0));
                    pp.distance(p)
                })
                .unwrap_or(0.0);

            let surveille = q_name
                .get(os)
                .map(|n| cfg.os_surveilles.iter().any(|s| s == n.as_str()))
                .unwrap_or(false);
            if !surveille || longueur < OS_MINIMUM_M {
                continue;
            }
            // Le repère LOCAL de l'os. C'est la seule chose qui répond à « dans
            // quel sens tourne-t-il » — la question sur laquelle douze angles
            // d'Euler devinés se sont cassé les dents.
            let l = longueur * AXE_FRACTION;
            let r = gt.rotation();
            gizmos.line(p, p + r * Vec3::X * l, Color::srgb(1.0, 0.25, 0.25));
            gizmos.line(p, p + r * Vec3::Y * l, Color::srgb(0.25, 1.0, 0.25));
            gizmos.line(p, p + r * Vec3::Z * l, Color::srgb(0.35, 0.5, 1.0));
        }
    }

    for (p, couleur) in &cibles.0 {
        gizmos.sphere(*p, 0.05, *couleur);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_touche_de_debug_ne_marche_sur_les_pieds_de_personne() {
        // F1..F4 sont libres ; Shift+F12 est le rechargement de génome, F12 seul
        // la capture. Un raccourci de debug qui vole une touche de jeu se
        // découvre au pire moment.
        assert!(matches!(
            TOUCHE,
            KeyCode::F1 | KeyCode::F2 | KeyCode::F3 | KeyCode::F4
        ));
    }

    #[test]
    fn les_axes_sont_proportionnels_a_l_os() {
        // Un repère de taille fixe est illisible : minuscule sur une cuisse,
        // énorme sur une phalange. Et un os sans longueur n'en reçoit pas.
        assert!(AXE_FRACTION > 0.1 && AXE_FRACTION < 1.0);
        assert!(OS_MINIMUM_M > 0.0);
    }

    #[test]
    fn l_affichage_est_eteint_par_defaut() {
        // Un debug allumé par défaut finit dans une capture d'écran de
        // présentation — et coûte son prix à chaque frame de tout le monde.
        assert!(!SqueletteVisible::default().0);
    }
}
