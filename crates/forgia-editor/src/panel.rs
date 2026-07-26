//! Interface de l'éditeur — trois panneaux egui : barre d'outils, bibliothèque,
//! inspecteur de sélection.
//!
//! Volontairement en style egui par défaut, sans dépendre de `forgia-ui` : c'est
//! un outil de création, pas un écran du jeu. Il ne doit ni tirer la DA « Verre &
//! Braise » dans une crate d'outillage, ni créer un couplage UI de plus.
//!
//! Le panneau ne modifie **jamais** le monde directement : il pousse dans
//! [`SpawnQueue`] ou pose un marqueur. Seule exception assumée : les champs
//! numériques de l'inspecteur, qui écrivent le `Transform` sélectionné — c'est
//! leur raison d'être (placement au centimètre, impossible à la souris).

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::library::{EditorLibrary, SpawnQueue};
use crate::persist::SceneEdits;
use crate::select::{EditorDecor, EditorProp, Selection};
use crate::snap::{NeedsGroundSnap, SnapMode};
use crate::transform_ops::{ActiveOp, UndoStack};
use crate::{EditorSession, EditorStatus};

/// Filtre texte de la bibliothèque (état d'UI pur).
#[derive(Resource, Default)]
pub struct LibraryFilter(pub String);

/// Nombre d'entrées affichées au maximum quand aucun filtre n'est saisi, par
/// dossier — évite de construire 400 boutons pour le dossier `nature`.
const MAX_ENTRIES_PER_GROUP: usize = 60;

#[allow(clippy::too_many_arguments)]
pub fn draw_editor_ui(
    mut contexts: EguiContexts,
    mut session: ResMut<EditorSession>,
    mut filter: ResMut<LibraryFilter>,
    mut queue: ResMut<SpawnQueue>,
    mut status: ResMut<EditorStatus>,
    mut edits: ResMut<SceneEdits>,
    mut commands: Commands,
    library: Res<EditorLibrary>,
    selection: Res<Selection>,
    op: Res<ActiveOp>,
    undo: Res<UndoStack>,
    mut q_transform: Query<&mut Transform>,
    q_prop: Query<&EditorProp>,
    q_decor: Query<&EditorDecor>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    draw_toolbar(
        ctx,
        &mut session,
        &mut status,
        &edits,
        &library,
        &selection,
        &op,
        &undo,
    );
    if session.library_open {
        draw_library(ctx, &library, &mut filter, &mut queue, &mut session);
    }
    draw_inspector(
        ctx,
        &selection,
        &mut edits,
        &mut commands,
        &mut status,
        &mut q_transform,
        &q_prop,
        &q_decor,
    );

    // Drapeau lu à la frame suivante par les outils 3D : un clic sur un panneau
    // ne doit pas aussi sélectionner ou valider derrière lui, et taper dans le
    // champ de recherche ne doit pas déclencher G/R/T.
    session.ui_capture = ctx.wants_pointer_input() || ctx.wants_keyboard_input();
}

#[allow(clippy::too_many_arguments)]
fn draw_toolbar(
    ctx: &egui::Context,
    session: &mut EditorSession,
    status: &mut EditorStatus,
    edits: &SceneEdits,
    library: &EditorLibrary,
    selection: &Selection,
    op: &ActiveOp,
    undo: &UndoStack,
) {
    egui::Window::new("Éditeur — Hall de Forgia")
        .default_pos([16.0, 16.0])
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Aimant :");
                for mode in [SnapMode::Ground, SnapMode::Grid, SnapMode::Off] {
                    if ui
                        .selectable_label(session.snap == mode, mode.label())
                        .clicked()
                    {
                        session.snap = mode;
                        status.set(format!("Aimant : {}", mode.label()));
                    }
                }
                ui.label("(F)");
            });

            ui.horizontal(|ui| {
                if ui.button("Bibliothèque (B)").clicked() {
                    session.library_open = !session.library_open;
                }
                ui.label(format!("{} asset(s)", library.total));
            });

            ui.separator();
            ui.label(format!(
                "Ajoutés : {} · Déplacés : {} · Sélection : {}",
                edits.props.len(),
                edits.overrides.len(),
                selection.items.len()
            ));
            ui.label(format!(
                "Geste : {} ({}) · Annulations : {}",
                op.kind.label(),
                op.axis.label(),
                undo.depth()
            ));
            let save_text = if !edits.last_save_ok {
                egui::RichText::new(format!(
                    "SAUVEGARDE EN ÉCHEC — {}",
                    edits.last_error.as_deref().unwrap_or("cause inconnue")
                ))
                .color(egui::Color32::from_rgb(255, 96, 96))
            } else if edits.dirty() {
                egui::RichText::new("Modifications en attente d'écriture…")
                    .color(egui::Color32::from_rgb(255, 200, 100))
            } else {
                egui::RichText::new("Sauvegardé").color(egui::Color32::from_rgb(140, 220, 140))
            };
            ui.label(save_text);
            if !status.text.is_empty() {
                ui.label(egui::RichText::new(&status.text).italics());
            }

            ui.collapsing("Raccourcis", |ui| {
                ui.label("Clic gauche : sélectionner (Maj = ajouter)");
                ui.label("G : déplacer · R : tourner · T : taille");
                ui.label("1 / 2 / 3 : contraindre l'axe X / Y / Z");
                ui.label("Ctrl (pendant le geste) : pas fixes · Maj : précision fine");
                ui.label("Clic gauche ou Entrée : valider · Retour arrière : annuler le geste");
                ui.label("Ctrl+Z : annuler · Ctrl+D : dupliquer · Suppr : supprimer");
                ui.label("Fin : poser au sol · F : aimant · B : bibliothèque");
                ui.label("Clic droit maintenu : regarder autour · \\ : vol libre");
                ui.label("Pavé numérique . : fermer l'éditeur");
            });
        });
}

fn draw_library(
    ctx: &egui::Context,
    library: &EditorLibrary,
    filter: &mut LibraryFilter,
    queue: &mut SpawnQueue,
    session: &mut EditorSession,
) {
    let mut open = true;
    egui::Window::new("Bibliothèque")
        .default_pos([16.0, 320.0])
        .default_size([340.0, 460.0])
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Filtrer :");
                ui.text_edit_singleline(&mut filter.0);
                if ui.button("×").clicked() {
                    filter.0.clear();
                }
            });
            if let Some(error) = &library.error {
                ui.colored_label(egui::Color32::from_rgb(255, 96, 96), error);
            }
            ui.separator();

            let needle = filter.0.trim().to_lowercase();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for group in &library.groups {
                    let matching: Vec<_> = group
                        .entries
                        .iter()
                        .filter(|entry| {
                            needle.is_empty() || entry.label.to_lowercase().contains(&needle)
                        })
                        .collect();
                    if matching.is_empty() {
                        continue;
                    }
                    let header = format!("{} ({})", group.label, matching.len());
                    // Un filtre actif ouvre les dossiers : on cherche un objet,
                    // pas un dossier.
                    egui::CollapsingHeader::new(header)
                        .default_open(!needle.is_empty())
                        .show(ui, |ui| {
                            let limit = if needle.is_empty() {
                                MAX_ENTRIES_PER_GROUP
                            } else {
                                matching.len()
                            };
                            for entry in matching.iter().take(limit) {
                                if ui.button(&entry.label).clicked() {
                                    queue.0.push(entry.asset.clone());
                                }
                            }
                            if matching.len() > limit {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "… {} de plus — utilise le filtre",
                                        matching.len() - limit
                                    ))
                                    .italics(),
                                );
                            }
                        });
                }
            });
        });
    if !open {
        session.library_open = false;
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_inspector(
    ctx: &egui::Context,
    selection: &Selection,
    edits: &mut SceneEdits,
    commands: &mut Commands,
    status: &mut EditorStatus,
    q_transform: &mut Query<&mut Transform>,
    q_prop: &Query<&EditorProp>,
    q_decor: &Query<&EditorDecor>,
) {
    let Some(entity) = selection.primary() else {
        return;
    };
    let Ok(mut transform) = q_transform.get_mut(entity) else {
        return;
    };

    egui::Window::new("Sélection")
        .anchor(egui::Align2::RIGHT_TOP, [-16.0, 16.0])
        .resizable(false)
        .show(ctx, |ui| {
            if let Ok(prop) = q_prop.get(entity) {
                ui.label(format!("Ajouté · {}", prop.asset));
            } else if let Ok(decor) = q_decor.get(entity) {
                ui.label("Décor du Hall");
                ui.label(egui::RichText::new(&decor.key).small().weak());
            }
            if selection.items.len() > 1 {
                ui.label(
                    egui::RichText::new(format!("+ {} autre(s) sélectionné(s)", selection.items.len() - 1))
                        .italics(),
                );
            }
            ui.separator();

            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("Position");
                changed |= drag(ui, &mut transform.translation.x, "X", 0.02);
                changed |= drag(ui, &mut transform.translation.y, "Y", 0.02);
                changed |= drag(ui, &mut transform.translation.z, "Z", 0.02);
            });

            let (yaw, pitch, roll) = transform.rotation.to_euler(EulerRot::YXZ);
            let mut degrees = Vec3::new(yaw.to_degrees(), pitch.to_degrees(), roll.to_degrees());
            let mut rotation_changed = false;
            ui.horizontal(|ui| {
                ui.label("Rotation");
                rotation_changed |= drag(ui, &mut degrees.x, "Y°", 0.5);
                rotation_changed |= drag(ui, &mut degrees.y, "X°", 0.5);
                rotation_changed |= drag(ui, &mut degrees.z, "Z°", 0.5);
            });
            if rotation_changed {
                transform.rotation = Quat::from_euler(
                    EulerRot::YXZ,
                    degrees.x.to_radians(),
                    degrees.y.to_radians(),
                    degrees.z.to_radians(),
                );
                changed = true;
            }

            let mut uniform = transform.scale.x;
            ui.horizontal(|ui| {
                ui.label("Taille");
                if drag(ui, &mut uniform, "×", 0.01) {
                    transform.scale = Vec3::splat(uniform.max(MIN_INSPECTOR_SCALE));
                    changed = true;
                }
            });

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Poser au sol (Fin)").clicked() {
                    for &item in &selection.items {
                        commands.entity(item).insert(NeedsGroundSnap::default());
                    }
                    status.set("Posé au sol".to_owned());
                }
                if ui.button("Réinitialiser la rotation").clicked() {
                    transform.rotation = Quat::IDENTITY;
                    changed = true;
                }
            });

            if changed {
                let snapshot = *transform;
                crate::transform_ops::record_transform(
                    edits, entity, &snapshot, q_prop, q_decor,
                );
            }
        });
}

/// Échelle plancher côté inspecteur (une saisie à 0 rendrait l'objet invisible
/// et impossible à re-sélectionner).
const MIN_INSPECTOR_SCALE: f32 = 0.01;

fn drag(ui: &mut egui::Ui, value: &mut f32, prefix: &str, speed: f32) -> bool {
    ui.add(
        egui::DragValue::new(value)
            .speed(speed)
            .prefix(format!("{prefix} "))
            .max_decimals(3),
    )
    .changed()
}
