//! Aimant — pose un objet **sur** le sol au lieu de le laisser flotter ou
//! s'enfoncer, ou l'aligne sur une grille régulière.
//!
//! Le mode courant vit dans [`crate::EditorSession::snap`] et se change à la
//! touche `F` (ou dans le panneau). Trois modes seulement, pour rester lisible :
//! **Sol** (défaut), **Grille**, **Libre**.
//!
//! La pose au sol utilise la **boîte englobante réelle** du sous-arbre : le point
//! bas de l'objet touche la surface, quelle que soit la position de son origine
//! dans le GLB (une origine au centre ou aux pieds donnent le même résultat en
//! jeu — c'est ce qui rend le placement « facile » sans réglage manuel).

use bevy::camera::primitives::Aabb;
use bevy::math::Affine3A;
use bevy::prelude::*;
use bevy_rapier3d::prelude::{QueryFilter, ReadRapierContext};
use forgia_player::prelude::Player;

use crate::select::{world_bounds, EditorDecor, EditorProp, Selection};
use crate::transform_ops::{record_transform, ActiveOp};
use crate::{EditorSession, EditorStatus};

/// Pas de la grille, en mètres. Ergonomie d'outil (pas du gameplay) : un quart
/// de mètre est assez fin pour aligner du mobilier, assez large pour être visible.
const GRID_STEP_M: f32 = 0.25;
/// Hauteur de départ du rayon au-dessus de l'objet, et portée de sondage sous lui.
const SNAP_PROBE_UP_M: f32 = 0.5;
const SNAP_PROBE_DOWN_M: f32 = 200.0;

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum SnapMode {
    /// Pose l'objet sur la première surface sous lui.
    #[default]
    Ground,
    /// Aligne la position sur une grille de `GRID_STEP_M`.
    Grid,
    /// Aucune correction.
    Off,
}

impl SnapMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ground => "Sol",
            Self::Grid => "Grille",
            Self::Off => "Libre",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Ground => Self::Grid,
            Self::Grid => Self::Off,
            Self::Off => Self::Ground,
        }
    }
}

/// Demande de pose au sol (spawn, touche `Fin`, déplacement validé en mode Sol).
///
/// La demande **survit à plusieurs frames** : juste après un spawn, la scène glTF
/// n'est pas encore instanciée, l'objet n'a donc aucune boîte englobante et la
/// pose échouerait en silence. On réessaie jusqu'à ce que la géométrie existe,
/// avec un plafond pour ne pas garder un marqueur éternel sur un asset introuvable.
#[derive(Component, Default)]
pub struct NeedsGroundSnap {
    attempts: u8,
}

/// Nombre de frames d'attente accordées à l'instanciation de la scène.
const MAX_GROUND_SNAP_ATTEMPTS: u8 = 120;
/// Remontée maximale acceptée par une pose au sol. Descendre est normal (on
/// dépose), remonter de plusieurs mètres signifie que le rayon a touché autre
/// chose que le sol : on refuse au lieu de déplacer le décor.
const MAX_SNAP_LIFT_M: f32 = 5.0;
/// Garde-fou de parcours de hiérarchie (mêmes scènes que le picking).
const MAX_SUBTREE_NODES: usize = 4096;

/// Demande d'alignement sur la grille (déplacement validé en mode Grille).
#[derive(Component)]
pub struct NeedsGridSnap;

/// `F` = change de mode d'aimant. `Fin` = pose la sélection au sol tout de suite,
/// quel que soit le mode (geste ponctuel, indépendant du réglage).
pub fn sys_snap_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    op: Res<ActiveOp>,
    selection: Res<Selection>,
    mut commands: Commands,
    mut session: ResMut<EditorSession>,
    mut status: ResMut<EditorStatus>,
) {
    if session.ui_keyboard || op.active() {
        return;
    }
    if keys.just_pressed(KeyCode::KeyF) {
        session.snap = session.snap.next();
        status.set(format!("Aimant : {}", session.snap.label()));
    }
    if keys.just_pressed(KeyCode::End) {
        for &entity in &selection.items {
            commands.entity(entity).insert(NeedsGroundSnap::default());
        }
        if !selection.items.is_empty() {
            status.set("Posé au sol".to_owned());
        }
    }
}

/// Applique les poses au sol demandées : un rayon vertical par objet, puis on
/// descend l'objet pour que le bas de sa boîte touche l'impact.
#[allow(clippy::too_many_arguments)]
pub fn sys_apply_ground_snap(
    rapier: ReadRapierContext,
    mut q_pending: Query<(Entity, &mut NeedsGroundSnap)>,
    q_player: Query<Entity, With<Player>>,
    q_children: Query<&Children>,
    q_shape: Query<(&GlobalTransform, &Aabb)>,
    q_parent: Query<&ChildOf>,
    q_global: Query<&GlobalTransform>,
    q_prop: Query<&EditorProp>,
    q_decor: Query<&EditorDecor>,
    mut q_transform: Query<&mut Transform>,
    mut scratch: Local<Vec<Entity>>,
    mut subtree: Local<Vec<Entity>>,
    mut commands: Commands,
    mut status: ResMut<EditorStatus>,
    mut edits: ResMut<crate::persist::SceneEdits>,
    mut history: ResMut<crate::history::EditHistory>,
) {
    if q_pending.is_empty() {
        return;
    }
    let Ok(ctx) = rapier.single() else {
        return;
    };
    let player_entity = q_player.single().ok();

    for (entity, mut pending) in &mut q_pending {
        let Some((center, size)) = world_bounds(entity, &q_children, &q_shape, &mut scratch) else {
            // Géométrie pas encore là : on retente, sauf si l'attente est
            // déraisonnable (asset manquant → on abandonne au lieu de boucler).
            pending.attempts = pending.attempts.saturating_add(1);
            if pending.attempts >= MAX_GROUND_SNAP_ATTEMPTS {
                warn!("[forgia-editor] pose au sol abandonnée : aucune géométrie pour {entity}");
                commands.entity(entity).remove::<NeedsGroundSnap>();
            }
            continue;
        };
        commands.entity(entity).remove::<NeedsGroundSnap>();

        // 🚨 Le rayon ne doit PAS toucher l'objet qu'on est en train de poser.
        // Sinon on le pose sur lui-même : c'est exactement ce qui a envoyé le
        // terrain du Hall au plafond (bas à −59 m posé sur son propre sommet à
        // +57 m = +116 m). Tout objet ayant son propre collider est concerné.
        subtree.clear();
        collect_subtree(entity, &q_children, &mut subtree);
        let keep =
            |candidate: Entity| !subtree.contains(&candidate) && Some(candidate) != player_entity;
        let filter = QueryFilter::default().exclude_sensors().predicate(&keep);

        let bottom = center.y - size.y * 0.5;
        let origin = Vec3::new(
            center.x,
            center.y + size.y * 0.5 + SNAP_PROBE_UP_M,
            center.z,
        );
        let Some((_, toi)) = ctx.cast_ray(
            origin,
            Vec3::NEG_Y,
            size.y + SNAP_PROBE_UP_M + SNAP_PROBE_DOWN_M,
            true,
            filter,
        ) else {
            continue;
        };
        let surface_y = origin.y - toi;
        let world_delta = Vec3::new(0.0, surface_y - bottom, 0.0);

        // Garde-fou de dernier recours : poser au sol doit *déposer*, pas
        // catapulter. Une remontée massive signifie qu'on a touché autre chose
        // que le sol — on refuse bruyamment plutôt que de déplacer le décor.
        if snap_lift_rejected(world_delta.y) {
            warn!(
                "[forgia-editor] pose au sol refusée : remontée de {:.1} m (max {MAX_SNAP_LIFT_M} m)",
                world_delta.y
            );
            status.set(format!(
                "Pose au sol refusée — remontée de {:.0} m jugée aberrante",
                world_delta.y
            ));
            continue;
        }

        apply_world_translation(
            entity,
            world_delta,
            &q_parent,
            &q_global,
            &mut q_transform,
            &mut edits,
            &mut history,
            crate::history::EditKind::Snapped,
            &q_prop,
            &q_decor,
        );
    }
}

/// Applique les alignements grille demandés (X/Z seulement : la hauteur reste
/// celle que le créateur a choisie, la grille sert à aligner un mur ou un pavage).
#[allow(clippy::too_many_arguments)]
pub fn sys_apply_grid_snap(
    q_pending: Query<Entity, With<NeedsGridSnap>>,
    q_parent: Query<&ChildOf>,
    q_global: Query<&GlobalTransform>,
    q_prop: Query<&EditorProp>,
    q_decor: Query<&EditorDecor>,
    mut q_transform: Query<&mut Transform>,
    mut commands: Commands,
    mut edits: ResMut<crate::persist::SceneEdits>,
    mut history: ResMut<crate::history::EditHistory>,
) {
    for entity in &q_pending {
        commands.entity(entity).remove::<NeedsGridSnap>();
        let Ok(global) = q_global.get(entity) else {
            continue;
        };
        let position = global.translation();
        let snapped = Vec3::new(
            snap_to_grid(position.x),
            position.y,
            snap_to_grid(position.z),
        );
        apply_world_translation(
            entity,
            snapped - position,
            &q_parent,
            &q_global,
            &mut q_transform,
            &mut edits,
            &mut history,
            crate::history::EditKind::Moved,
            &q_prop,
            &q_decor,
        );
    }
}

/// Translate une entité d'un delta exprimé en **monde**, en écrivant son
/// `Transform` **local** (le parent peut être une racine de scène transformée).
#[allow(clippy::too_many_arguments)]
fn apply_world_translation(
    entity: Entity,
    world_delta: Vec3,
    q_parent: &Query<&ChildOf>,
    q_global: &Query<&GlobalTransform>,
    q_transform: &mut Query<&mut Transform>,
    edits: &mut crate::persist::SceneEdits,
    history: &mut crate::history::EditHistory,
    kind: crate::history::EditKind,
    q_prop: &Query<&EditorProp>,
    q_decor: &Query<&EditorDecor>,
) {
    if world_delta.length_squared() < 1e-12 {
        return;
    }
    let parent_inverse = q_parent
        .get(entity)
        .ok()
        .and_then(|child_of| q_global.get(child_of.parent()).ok())
        .map(|parent| parent.affine().inverse())
        .unwrap_or(Affine3A::IDENTITY);
    let local_delta = parent_inverse.transform_vector3(world_delta);
    let Ok(mut transform) = q_transform.get_mut(entity) else {
        return;
    };
    let before = *transform;
    transform.translation += local_delta;
    let snapshot = *transform;
    record_transform(edits, entity, &snapshot, q_prop, q_decor);
    // L'aimant est une modification à part entière : c'est lui qui a un jour
    // remonté tout le terrain du Hall de 102 m. Il doit donc être journalisé et
    // annulable comme n'importe quel geste.
    if let Some((target, label, asset)) =
        crate::history::describe(q_prop.get(entity).ok(), q_decor.get(entity).ok())
    {
        history.record(
            kind,
            target,
            label,
            asset,
            Some((&before).into()),
            Some((&snapshot).into()),
        );
    }
}

fn snap_to_grid(value: f32) -> f32 {
    (value / GRID_STEP_M).round() * GRID_STEP_M
}

/// Une pose au sol qui *remonte* fortement l'objet est refusée : elle signifie
/// que le rayon a touché autre chose que le sol. Descendre reste libre — déposer
/// un objet placé en l'air est le cas nominal.
fn snap_lift_rejected(delta_y: f32) -> bool {
    delta_y > MAX_SNAP_LIFT_M
}

/// Rassemble la racine et tous ses descendants — sert à exclure l'objet posé de
/// son propre rayon de sondage. Borné : une scène pathologique ne doit pas faire
/// boucler la frame.
fn collect_subtree(root: Entity, q_children: &Query<&Children>, out: &mut Vec<Entity>) {
    out.push(root);
    let mut cursor = 0;
    while cursor < out.len() && out.len() < MAX_SUBTREE_NODES {
        let entity = out[cursor];
        cursor += 1;
        if let Ok(children) = q_children.get(entity) {
            out.extend(children.iter());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_rounds_to_quarter_meter() {
        assert_eq!(snap_to_grid(1.10), 1.0);
        assert_eq!(snap_to_grid(1.20), 1.25);
        assert_eq!(snap_to_grid(-0.60), -0.5);
    }

    #[test]
    fn dropping_is_always_allowed() {
        assert!(
            !snap_lift_rejected(-120.0),
            "déposer un objet en l'air est nominal"
        );
        assert!(!snap_lift_rejected(0.0));
    }

    #[test]
    fn small_lift_is_allowed_big_lift_is_refused() {
        // Un objet légèrement enfoncé remonte : normal.
        assert!(!snap_lift_rejected(1.5));
        // Le cas vécu : le terrain posé sur son propre sommet (+116 m).
        assert!(snap_lift_rejected(116.0));
        assert!(snap_lift_rejected(102.6));
    }

    #[test]
    fn snap_mode_cycles_through_all_three() {
        let mut mode = SnapMode::default();
        assert_eq!(mode, SnapMode::Ground);
        mode = mode.next();
        assert_eq!(mode, SnapMode::Grid);
        mode = mode.next();
        assert_eq!(mode, SnapMode::Off);
        mode = mode.next();
        assert_eq!(mode, SnapMode::Ground);
    }
}
