//! Opérations de transform façon Blender : **G** déplacer, **R** tourner,
//! **T** taille, contraintes d'axe **1/2/3** (X/Y/Z), `Ctrl` = pas fixes,
//! `Maj` = précision fine, clic gauche / `Entrée` = valider, `Retour arrière` =
//! annuler l'opération, `Ctrl+Z` = annuler la dernière transformation validée.
//!
//! ## Pourquoi pas littéralement G/R/S + X/Y/Z
//!
//! `KeyCode` est **physique** (positions QWERTY). Sur le clavier AZERTY d'Antoine,
//! le déplacement occupe le bloc `KeyW/KeyA/KeyS/KeyD` — soit les touches
//! **Z Q S D**. « S » pour *scale* et « Z » pour l'axe Z tomberaient donc sur
//! *reculer* et *avancer*. D'où **T** (taille) et **1/2/3** pour les axes, avec
//! la légende affichée en permanence dans le panneau.
//!
//! ## Repère de calcul
//!
//! Le geste est calculé en **monde** (les axes 1/2/3 et les vecteurs caméra y
//! vivent), puis reprojeté dans le repère du parent avant écriture : une pièce de
//! décor est enfant d'une racine de scène qui a sa propre transformation (calage
//! du sol, cellule de château), et c'est bien son `Transform` **local** qui est
//! persisté puis réappliqué au rechargement.

use bevy::input::mouse::MouseMotion;
use bevy::math::Affine3A;
use bevy::prelude::*;

use crate::persist::SceneEdits;
use crate::select::{EditorDecor, EditorProp, Selection};
use crate::snap::{NeedsGridSnap, NeedsGroundSnap, SnapMode};
use crate::{EditorSession, EditorStatus};

/// Sensibilités du geste — ergonomie de l'outil de création, sans effet sur le
/// gameplay : `const` nommées plutôt que gènes TOML (cf `no-hardcode` §exception).
/// Le déplacement est proportionnel à la distance pour rester 1:1 à l'écran.
const MOVE_M_PER_PIXEL: f32 = 0.0022;
const ROT_RAD_PER_PIXEL: f32 = 0.006;
const SCALE_PER_PIXEL: f32 = 0.004;
/// Pas fixes quand `Ctrl` est maintenu.
const SNAP_MOVE_M: f32 = 0.25;
const SNAP_ROT_DEG: f32 = 15.0;
const SNAP_SCALE_STEP: f32 = 0.1;
/// Multiplicateur `Maj` — précision fine.
const FINE_MULTIPLIER: f32 = 0.2;
/// Échelle plancher : un objet ne doit pas pouvoir disparaître par écrasement.
const MIN_SCALE: f32 = 0.01;
/// Profondeur de la pile d'annulation.
const UNDO_DEPTH: usize = 32;

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum OpKind {
    #[default]
    None,
    Move,
    Rotate,
    Scale,
}

impl OpKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "—",
            Self::Move => "Déplacer",
            Self::Rotate => "Tourner",
            Self::Scale => "Taille",
        }
    }
}

#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum OpAxis {
    /// Libre : dans le plan de la caméra (déplacer) ou autour de son axe de vue.
    #[default]
    View,
    X,
    Y,
    Z,
}

impl OpAxis {
    pub fn label(self) -> &'static str {
        match self {
            Self::View => "libre",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    fn world_vector(self, camera: &GlobalTransform) -> Vec3 {
        match self {
            Self::X => Vec3::X,
            Self::Y => Vec3::Y,
            Self::Z => Vec3::Z,
            Self::View => camera.forward().as_vec3(),
        }
    }
}

/// Un objet en cours de transformation. On mémorise l'état de départ pour que
/// chaque frame recalcule depuis l'origine du geste (annulation exacte, pas de
/// dérive d'accumulation).
struct OpItem {
    entity: Entity,
    start_local: Transform,
    start_world: Affine3A,
    parent_inverse: Affine3A,
}

#[derive(Resource, Default)]
pub struct ActiveOp {
    pub kind: OpKind,
    pub axis: OpAxis,
    /// Déplacement souris cumulé depuis le début du geste, en pixels.
    accumulated: Vec2,
    /// Pivot monde = barycentre de la sélection au début du geste.
    pivot: Vec3,
    items: Vec<OpItem>,
}

impl ActiveOp {
    pub fn active(&self) -> bool {
        self.kind != OpKind::None
    }

    fn reset(&mut self) {
        self.kind = OpKind::None;
        self.axis = OpAxis::View;
        self.accumulated = Vec2::ZERO;
        self.items.clear();
    }
}

/// Transformations d'avant-geste, pour `Ctrl+Z`.
#[derive(Resource, Default)]
pub struct UndoStack {
    steps: Vec<Vec<(Entity, Transform)>>,
}

impl UndoStack {
    fn push(&mut self, step: Vec<(Entity, Transform)>) {
        if step.is_empty() {
            return;
        }
        if self.steps.len() == UNDO_DEPTH {
            self.steps.remove(0);
        }
        self.steps.push(step);
    }

    pub fn depth(&self) -> usize {
        self.steps.len()
    }
}

/// Démarre un geste (`G`/`R`/`T`) ou change son axe (`1`/`2`/`3`).
pub fn sys_begin_op(
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<EditorSession>,
    selection: Res<Selection>,
    q_transform: Query<&Transform>,
    q_global: Query<&GlobalTransform>,
    q_parent: Query<&ChildOf>,
    mut op: ResMut<ActiveOp>,
    mut status: ResMut<EditorStatus>,
) {
    if session.ui_capture || session.navigating {
        return;
    }

    // Changement d'axe pendant un geste ; rappuyer sur le même axe libère.
    if op.active() {
        for (key, axis) in [
            (KeyCode::Digit1, OpAxis::X),
            (KeyCode::Digit2, OpAxis::Y),
            (KeyCode::Digit3, OpAxis::Z),
        ] {
            if keys.just_pressed(key) {
                op.axis = if op.axis == axis { OpAxis::View } else { axis };
            }
        }
        return;
    }

    let kind = if keys.just_pressed(KeyCode::KeyG) {
        OpKind::Move
    } else if keys.just_pressed(KeyCode::KeyR) {
        OpKind::Rotate
    } else if keys.just_pressed(KeyCode::KeyT) {
        OpKind::Scale
    } else {
        return;
    };
    if selection.items.is_empty() {
        status.set("Rien de sélectionné — clic gauche sur un objet d'abord".to_owned());
        return;
    }

    op.reset();
    op.kind = kind;
    let mut sum = Vec3::ZERO;
    for &entity in &selection.items {
        let Ok(local) = q_transform.get(entity) else {
            continue;
        };
        let Ok(global) = q_global.get(entity) else {
            continue;
        };
        let parent_inverse = q_parent
            .get(entity)
            .ok()
            .and_then(|child_of| q_global.get(child_of.parent()).ok())
            .map(|parent| parent.affine().inverse())
            .unwrap_or(Affine3A::IDENTITY);
        sum += global.translation();
        op.items.push(OpItem {
            entity,
            start_local: *local,
            start_world: global.affine(),
            parent_inverse,
        });
    }
    if op.items.is_empty() {
        op.reset();
        return;
    }
    op.pivot = sum / op.items.len() as f32;
    status.set(format!("{} — 1/2/3 = axe, clic = valider", kind.label()));
}

/// Applique le geste en cours, le valide ou l'annule.
#[allow(clippy::too_many_arguments)]
pub fn sys_drive_op(
    mut motion: MessageReader<MouseMotion>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    session: Res<EditorSession>,
    q_camera: Query<&GlobalTransform, With<forgia_player::prelude::FpsCamera>>,
    mut commands: Commands,
    mut op: ResMut<ActiveOp>,
    mut edits: ResMut<SceneEdits>,
    mut undo: ResMut<UndoStack>,
    mut status: ResMut<EditorStatus>,
    mut q_transform: Query<&mut Transform>,
    q_prop: Query<&EditorProp>,
    q_decor: Query<&EditorDecor>,
) {
    if !op.active() {
        // Purge le buffer : un mouvement fait hors geste ne doit pas s'appliquer
        // au geste suivant.
        motion.clear();
        return;
    }

    // Annulation du geste : tout revient à l'état de départ.
    if keys.just_pressed(KeyCode::Backspace) {
        for item in &op.items {
            if let Ok(mut transform) = q_transform.get_mut(item.entity) {
                *transform = item.start_local;
            }
        }
        status.set("Geste annulé".to_owned());
        op.reset();
        return;
    }

    for event in motion.read() {
        op.accumulated += event.delta;
    }

    let Ok(camera) = q_camera.single() else {
        return;
    };
    let coarse_snap = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let fine = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let delta = delta_affine(&op, camera, coarse_snap, fine);

    for item in &op.items {
        if let Ok(mut transform) = q_transform.get_mut(item.entity) {
            let world = delta * item.start_world;
            let local = item.parent_inverse * world;
            *transform = Transform::from_matrix(Mat4::from(local));
            transform.scale = transform.scale.max(Vec3::splat(MIN_SCALE));
        }
    }

    // Validation : clic gauche (hors panneau) ou Entrée.
    let confirm = (mouse.just_pressed(MouseButton::Left) && !session.ui_capture)
        || keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::NumpadEnter);
    if !confirm {
        return;
    }

    let kind = op.kind;
    let mut undo_step = Vec::with_capacity(op.items.len());
    for item in &op.items {
        undo_step.push((item.entity, item.start_local));
        let Ok(transform) = q_transform.get(item.entity) else {
            continue;
        };
        record_transform(&mut edits, item.entity, transform, &q_prop, &q_decor);
        // L'aimant s'applique après un déplacement validé : c'est exactement le
        // « aimant » demandé (poser proprement au lieu de flotter / s'enfoncer).
        if kind == OpKind::Move {
            match session.snap {
                SnapMode::Ground => {
                    commands.entity(item.entity).insert(NeedsGroundSnap::default());
                }
                SnapMode::Grid => {
                    commands.entity(item.entity).insert(NeedsGridSnap);
                }
                SnapMode::Off => {}
            }
        }
    }
    undo.push(undo_step);
    status.set(format!("{} validé", kind.label()));
    op.reset();
}

/// Compose la transformation monde du geste (identité si le geste est nul).
fn delta_affine(op: &ActiveOp, camera: &GlobalTransform, coarse: bool, fine: bool) -> Affine3A {
    let screen = Vec2::new(op.accumulated.x, -op.accumulated.y);
    let precision = if fine { FINE_MULTIPLIER } else { 1.0 };

    match op.kind {
        OpKind::None => Affine3A::IDENTITY,
        OpKind::Move => {
            let distance = (op.pivot - camera.translation()).length().max(1.0);
            let unit = MOVE_M_PER_PIXEL * distance * precision;
            let offset = match op.axis {
                OpAxis::View => {
                    let raw = camera.right().as_vec3() * screen.x * unit
                        + camera.up().as_vec3() * screen.y * unit;
                    if coarse {
                        snap_vec(raw, SNAP_MOVE_M)
                    } else {
                        raw
                    }
                }
                axis => {
                    let world_axis = axis.world_vector(camera);
                    let amount = screen_amount(screen, world_axis, camera) * unit;
                    let amount = if coarse {
                        snap_scalar(amount, SNAP_MOVE_M)
                    } else {
                        amount
                    };
                    world_axis * amount
                }
            };
            Affine3A::from_translation(offset)
        }
        OpKind::Rotate => {
            let mut angle = screen.x * ROT_RAD_PER_PIXEL * precision;
            if coarse {
                angle = snap_scalar(angle, SNAP_ROT_DEG.to_radians());
            }
            let axis = op.axis.world_vector(camera).normalize_or(Vec3::Y);
            pivoted(
                op.pivot,
                Affine3A::from_quat(Quat::from_axis_angle(axis, angle)),
            )
        }
        OpKind::Scale => {
            let mut factor = 1.0 + screen.x * SCALE_PER_PIXEL * precision;
            if coarse {
                factor = snap_scalar(factor, SNAP_SCALE_STEP);
            }
            let factor = factor.max(MIN_SCALE);
            let scale = match op.axis {
                OpAxis::View => Vec3::splat(factor),
                OpAxis::X => Vec3::new(factor, 1.0, 1.0),
                OpAxis::Y => Vec3::new(1.0, factor, 1.0),
                OpAxis::Z => Vec3::new(1.0, 1.0, factor),
            };
            pivoted(op.pivot, Affine3A::from_scale(scale))
        }
    }
}

/// Encadre une transformation par le pivot (rotation/échelle autour de la sélection).
fn pivoted(pivot: Vec3, inner: Affine3A) -> Affine3A {
    Affine3A::from_translation(pivot) * inner * Affine3A::from_translation(-pivot)
}

/// Projette le geste souris sur la direction écran de l'axe monde visé : tirer
/// « vers la droite » sur un axe qui pointe à droite l'allonge, quelle que soit
/// l'orientation de la caméra.
fn screen_amount(screen: Vec2, world_axis: Vec3, camera: &GlobalTransform) -> f32 {
    let camera_space = camera.affine().inverse().transform_vector3(world_axis);
    let direction = camera_space.truncate();
    if direction.length_squared() < 1e-6 {
        // Axe quasi parallèle à la vue : plus de direction écran exploitable,
        // on retombe sur le geste horizontal.
        return screen.x;
    }
    screen.dot(direction.normalize())
}

fn snap_scalar(value: f32, step: f32) -> f32 {
    (value / step).round() * step
}

fn snap_vec(value: Vec3, step: f32) -> Vec3 {
    Vec3::new(
        snap_scalar(value.x, step),
        snap_scalar(value.y, step),
        snap_scalar(value.z, step),
    )
}

/// Écrit le transform courant dans le fichier d'édition (prop ou override décor).
pub fn record_transform(
    edits: &mut SceneEdits,
    entity: Entity,
    transform: &Transform,
    q_prop: &Query<&EditorProp>,
    q_decor: &Query<&EditorDecor>,
) {
    if let Ok(prop) = q_prop.get(entity) {
        edits.update_prop(prop.id, transform);
    } else if let Ok(decor) = q_decor.get(entity) {
        edits.update_override(&decor.key, transform);
    }
}

/// `Ctrl+Z` — restaure les transforms d'avant la dernière validation.
pub fn sys_undo(
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<EditorSession>,
    op: Res<ActiveOp>,
    mut undo: ResMut<UndoStack>,
    mut edits: ResMut<SceneEdits>,
    mut status: ResMut<EditorStatus>,
    mut q_transform: Query<&mut Transform>,
    q_prop: Query<&EditorProp>,
    q_decor: Query<&EditorDecor>,
) {
    if session.ui_capture || op.active() {
        return;
    }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if !ctrl || !keys.just_pressed(KeyCode::KeyZ) {
        return;
    }
    let Some(step) = undo.steps.pop() else {
        status.set("Rien à annuler".to_owned());
        return;
    };
    for (entity, transform) in step {
        if let Ok(mut current) = q_transform.get_mut(entity) {
            *current = transform;
            record_transform(&mut edits, entity, &transform, &q_prop, &q_decor);
        }
    }
    status.set("Annulé".to_owned());
}

/// Annule le geste en cours (fermeture de l'éditeur, sortie du Hall).
pub fn cancel_active_op(op: &mut ActiveOp, q_transform: &mut Query<&mut Transform>) {
    if !op.active() {
        return;
    }
    for item in &op.items {
        if let Ok(mut transform) = q_transform.get_mut(item.entity) {
            *transform = item.start_local;
        }
    }
    op.reset();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_rounds_to_step() {
        assert_eq!(snap_scalar(0.31, 0.25), 0.25);
        assert_eq!(snap_scalar(0.40, 0.25), 0.5);
        assert_eq!(snap_scalar(-0.31, 0.25), -0.25);
    }

    #[test]
    fn pivoted_rotation_keeps_pivot_fixed() {
        let pivot = Vec3::new(3.0, 1.0, -2.0);
        let affine = pivoted(
            pivot,
            Affine3A::from_quat(Quat::from_axis_angle(Vec3::Y, 1.234)),
        );
        let moved = affine.transform_point3(pivot);
        assert!((moved - pivot).length() < 1e-5);
    }

    #[test]
    fn pivoted_scale_keeps_pivot_fixed() {
        let pivot = Vec3::new(-4.0, 12.0, 7.5);
        let affine = pivoted(pivot, Affine3A::from_scale(Vec3::splat(2.5)));
        assert!((affine.transform_point3(pivot) - pivot).length() < 1e-5);
    }

    #[test]
    fn undo_stack_is_bounded() {
        let mut stack = UndoStack::default();
        for i in 0..(UNDO_DEPTH + 10) {
            stack.push(vec![(Entity::PLACEHOLDER, Transform::default())]);
            assert!(stack.depth() <= UNDO_DEPTH, "débordement à l'étape {i}");
        }
        assert_eq!(stack.depth(), UNDO_DEPTH);
    }

    #[test]
    fn empty_step_is_not_pushed() {
        let mut stack = UndoStack::default();
        stack.push(Vec::new());
        assert_eq!(stack.depth(), 0);
    }
}
