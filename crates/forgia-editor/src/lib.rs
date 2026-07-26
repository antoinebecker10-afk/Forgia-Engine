//! # forgia-editor — éditeur de scène in-game
//!
//! Éditeur simple, ouvert au **`.` du pavé numérique**, actuellement **réservé au
//! Hall de Forgia** (`GameMode::CastleHub`). Il permet de sélectionner un asset,
//! le déplacer / tourner / redimensionner façon Blender, en ajouter depuis la
//! bibliothèque du projet, et poser proprement au sol grâce à l'aimant. Tout est
//! persisté dans `castle_hub_edits.json`, de façon **non destructive** : aucun GLB
//! livré n'est réécrit.
//!
//! ## Pourquoi une crate et pas un module de `forgia-game`
//!
//! L'éditeur est gaté sur le Hall « pour l'instant » — il a vocation à servir les
//! autres modes. Une crate dédiée évite d'avoir à l'extraire d'un orchestrateur
//! plus tard (cf `.claude/rules/fine-grained-crates.md`), et garde la logique
//! testable : sélection, magnétisme et persistance ont leurs tests unitaires.
//!
//! ## Ce que ce lot ne fait pas encore
//!
//! Les pinceaux de sol (élever / creuser / lisser) et la peinture de texture sont
//! les lots suivants. Les assets ajoutés sont **visuels** : ils ne reçoivent pas
//! de collider (une boîte fausse sur une torche ou une plante serait pire que rien
//! — à traiter avec un choix explicite de forme de collision).
//!
//! ## Clavier
//!
//! Le mappage tient compte de l'AZERTY : `KeyCode` est physique, le déplacement
//! occupe déjà les touches Z Q S D. D'où `G`/`R`/`T` et les axes `1`/`2`/`3`
//! (détail dans [`transform_ops`]).

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use bevy_egui::EguiPrimaryContextPass;
use forgia_core::prelude::*;
use forgia_input::prelude::InputBlockers;

pub mod library;
mod panel;
pub mod persist;
pub mod pick;
pub mod select;
mod sensor;
pub mod snap;
pub mod transform_ops;

pub mod prelude {
    pub use crate::{editor_is_open, EditorSession, EditorStatus, ForgiaEditorPlugin};
}

/// Ouverture / fermeture : `.` du pavé numérique. Choisi parce que le reste du
/// pavé est déjà pris par le calage du sol du Hall (`castle_ground.rs`) — que cet
/// éditeur désarme d'ailleurs pendant qu'il est ouvert.
const TOGGLE_KEY: KeyCode = KeyCode::NumpadDecimal;

/// État de la session d'édition.
#[derive(Resource, Default)]
pub struct EditorSession {
    pub open: bool,
    /// Clic droit maintenu : la souris regarde autour au lieu de servir l'UI.
    pub navigating: bool,
    pub snap: snap::SnapMode,
    pub library_open: bool,
    /// Vrai quand egui veut la souris ou le clavier. Posé par le panneau, lu à la
    /// frame suivante par les outils 3D : un clic sur un bouton ne doit pas aussi
    /// agir dans la scène derrière.
    pub ui_capture: bool,
}

/// Dernier retour d'action, affiché dans la barre d'outils.
///
/// Ressource séparée de [`EditorSession`] : les systèmes qui *lisent* la session
/// ont souvent besoin d'*écrire* un statut, et ce découpage évite de demander un
/// accès exclusif à la session pour un message. La `String` n'est allouée qu'à
/// l'action de l'utilisateur, jamais par frame.
#[derive(Resource, Default)]
pub struct EditorStatus {
    pub text: String,
}

impl EditorStatus {
    pub fn set(&mut self, text: String) {
        info!("[forgia-editor] {text}");
        self.text = text;
    }
}

/// Condition d'exécution — l'éditeur est ouvert.
pub fn editor_is_open(session: Res<EditorSession>) -> bool {
    session.open
}

/// Condition d'exécution — l'éditeur tient la main sur les touches du Hall.
/// Exportée pour que `castle_ground` désarme son calage au pavé numérique.
pub fn editor_holds_keyboard(session: Res<EditorSession>) -> bool {
    session.open
}

/// Condition d'exécution — dans le Hall, en jeu.
fn hub_ingame(game_mode: Res<State<GameMode>>, app_mode: Res<State<AppMode>>) -> bool {
    matches!(game_mode.get(), GameMode::CastleHub) && matches!(app_mode.get(), AppMode::InGame)
}

pub struct ForgiaEditorPlugin;

impl Plugin for ForgiaEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EditorSession>()
            .init_resource::<EditorStatus>()
            .init_resource::<pick::EditorRay>()
            .init_resource::<select::Selection>()
            .init_resource::<transform_ops::ActiveOp>()
            .init_resource::<transform_ops::UndoStack>()
            .init_resource::<library::EditorLibrary>()
            .init_resource::<library::SpawnQueue>()
            .init_resource::<panel::LibraryFilter>()
            .init_resource::<persist::SceneEdits>();

        app.add_systems(
            OnEnter(GameMode::CastleHub),
            (library::scan_library, persist::load_edits).chain(),
        )
        .add_systems(
            OnExit(GameMode::CastleHub),
            (
                persist::flush_on_exit,
                library::cleanup_spawned,
                select::clear_selection,
                sys_close_session,
            )
                .chain(),
        )
        // Mettre le jeu en pause referme l'éditeur : le curseur et les blockers
        // repassent sous la responsabilité de forgia-ui, sans conflit.
        .add_systems(OnExit(AppMode::InGame), sys_close_session);

        // Ouverture / curseur : tournent aussi éditeur fermé (il faut bien pouvoir
        // l'ouvrir, et restaurer l'état du jeu à la fermeture).
        app.add_systems(
            Update,
            (sys_toggle_editor, sys_cursor_and_blockers)
                .chain()
                .run_if(hub_ingame),
        );

        // Outils, dans l'ordre : viser → sélectionner → transformer.
        app.add_systems(
            Update,
            (
                pick::sys_editor_ray,
                select::sys_prune_selection,
                select::sys_hover,
                select::sys_click,
                transform_ops::sys_begin_op,
                transform_ops::sys_drive_op,
                transform_ops::sys_undo,
                select::sys_shortcuts,
                snap::sys_snap_shortcuts,
                select::sys_draw_highlight,
            )
                .chain()
                .run_if(hub_ingame)
                .run_if(editor_is_open),
        );

        // Demandes différées + persistance. Volontairement PAS gaté sur
        // « éditeur ouvert » : une pose au sol demandée juste avant la fermeture
        // doit aboutir, et les overrides doivent suivre le streaming des cellules
        // même quand l'éditeur est refermé.
        app.add_systems(
            Update,
            (
                library::sys_process_spawn_queue,
                snap::sys_apply_ground_snap,
                snap::sys_apply_grid_snap,
                persist::sys_apply_overrides,
                persist::sys_autosave,
            )
                .chain()
                .run_if(in_state(GameMode::CastleHub)),
        );

        app.add_systems(
            EguiPrimaryContextPass,
            panel::draw_editor_ui
                .run_if(hub_ingame)
                .run_if(editor_is_open),
        );

        app.add_systems(
            Update,
            sensor::sys_write_editor_sensor.in_set(GameSet::Sensors),
        );
    }
}

/// `.` du pavé numérique — bascule l'éditeur.
fn sys_toggle_editor(
    keys: Res<ButtonInput<KeyCode>>,
    mut session: ResMut<EditorSession>,
    mut op: ResMut<transform_ops::ActiveOp>,
    mut selection: ResMut<select::Selection>,
    mut status: ResMut<EditorStatus>,
    mut edits: ResMut<persist::SceneEdits>,
    mut q_transform: Query<&mut Transform>,
) {
    if !keys.just_pressed(TOGGLE_KEY) {
        return;
    }
    if session.open {
        transform_ops::cancel_active_op(&mut op, &mut q_transform);
        session.open = false;
        session.library_open = false;
        selection.clear();
        // Fermer, c'est valider : on n'attend pas le délai d'autosave.
        if edits.dirty() {
            persist::save_now(&mut edits);
        }
        info!("[forgia-editor] éditeur fermé");
    } else {
        session.open = true;
        status.set("Éditeur ouvert — clic gauche pour sélectionner".to_owned());
    }
}

/// Curseur + blockers d'input selon l'état de la session.
///
/// Modèle d'interaction : éditeur ouvert, le curseur est **libre** pour l'UI et
/// la caméra ne tourne pas. **Clic droit maintenu** = on regarde autour (curseur
/// verrouillé, `block_look` levé). Le déplacement Z Q S D reste actif en
/// permanence : c'est ce qui permet d'aller placer un objet à l'autre bout du
/// Hall sans refermer l'outil — et c'est pourquoi les raccourcis d'édition
/// évitent ce bloc de touches.
fn sys_cursor_and_blockers(
    mouse: Res<ButtonInput<MouseButton>>,
    op: Res<transform_ops::ActiveOp>,
    mut session: ResMut<EditorSession>,
    mut blockers: ResMut<InputBlockers>,
    mut q_cursor: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut was_open: Local<bool>,
) {
    if session.open {
        let navigating = mouse.pressed(MouseButton::Right) && !op.active();
        session.navigating = navigating;
        blockers.block_look = !navigating;
        blockers.block_fire = true;
        if let Ok(mut options) = q_cursor.single_mut() {
            let wanted = if navigating {
                CursorGrabMode::Locked
            } else {
                CursorGrabMode::None
            };
            if options.grab_mode != wanted {
                options.grab_mode = wanted;
                options.visible = !navigating;
            }
        }
        *was_open = true;
    } else if *was_open {
        session.navigating = false;
        session.ui_capture = false;
        blockers.block_look = false;
        blockers.block_fire = false;
        if let Ok(mut options) = q_cursor.single_mut() {
            options.grab_mode = CursorGrabMode::Locked;
            options.visible = false;
        }
        *was_open = false;
    }
}

/// Ferme la session sans passer par la touche (sortie du Hall, mise en pause).
fn sys_close_session(
    mut session: ResMut<EditorSession>,
    mut op: ResMut<transform_ops::ActiveOp>,
    mut blockers: ResMut<InputBlockers>,
    mut q_transform: Query<&mut Transform>,
) {
    if !session.open {
        return;
    }
    transform_ops::cancel_active_op(&mut op, &mut q_transform);
    session.open = false;
    session.library_open = false;
    session.navigating = false;
    session.ui_capture = false;
    blockers.block_look = false;
    blockers.block_fire = false;
    info!("[forgia-editor] éditeur refermé (sortie du Hall ou pause)");
}
