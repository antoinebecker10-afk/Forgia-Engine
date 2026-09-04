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

use crate::history::{age_label, EditHistory, RevertQueue};
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
    mut revert_queue: ResMut<RevertQueue>,
    mut status: ResMut<EditorStatus>,
    mut edits: ResMut<SceneEdits>,
    mut commands: Commands,
    library: Res<EditorLibrary>,
    history: Res<EditHistory>,
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
        &mut edits,
        &library,
        &history,
        &selection,
        &op,
        &undo,
    );
    if session.library_open {
        draw_library(ctx, &library, &mut filter, &mut queue, &mut session);
    }
    if session.history_open {
        draw_history(ctx, &history, &mut revert_queue, &mut session);
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

    // Drapeaux lus à la frame suivante par les outils 3D. Deux drapeaux distincts :
    // un clic sur un panneau ne doit pas sélectionner derrière lui (pointeur), et
    // taper dans le champ de filtre ne doit pas déclencher G/R/T (clavier) — mais
    // le simple survol d'un panneau ne doit RIEN bloquer au clavier.
    session.ui_pointer = ctx.wants_pointer_input();
    session.ui_keyboard = ctx.wants_keyboard_input();
}

#[allow(clippy::too_many_arguments)]
fn draw_toolbar(
    ctx: &egui::Context,
    session: &mut EditorSession,
    status: &mut EditorStatus,
    edits: &mut SceneEdits,
    library: &EditorLibrary,
    history: &EditHistory,
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

            ui.horizontal(|ui| {
                if ui.button("Historique (H)").clicked() {
                    session.history_open = !session.history_open;
                }
                ui.label(format!("{} modification(s)", history.pending_count()));
            });

            ui.separator();
            ui.label(format!(
                "Ajoutés : {} · Déplacés : {} · Sélection : {}",
                edits.props.len(),
                edits.overrides.len(),
                selection.items.len()
            ));
            let hidden = edits.hidden_count();
            if hidden > 0 {
                // Une pièce masquée n'est plus sélectionnable : sans ce bouton,
                // « Suppr » sur du décor serait sans retour.
                if ui
                    .button(format!("Restaurer {hidden} pièce(s) masquée(s)"))
                    .clicked()
                {
                    let restored = edits.restore_all_hidden();
                    status.set(format!("{restored} pièce(s) restaurée(s)"));
                }
            }
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
            if session.hover_blocked {
                ui.label(
                    egui::RichText::new(
                        "Décor de fond (terrain / végétation) — non éditable comme un objet",
                    )
                    .color(egui::Color32::from_rgb(255, 200, 100)),
                );
            }
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

/// Journal des modifications — chaque ligne est annulable indépendamment.
///
/// Ordre **anti-chronologique** : la dernière bêtise est en haut, c'est celle
/// qu'on cherche en priorité.
fn draw_history(
    ctx: &egui::Context,
    history: &EditHistory,
    revert_queue: &mut RevertQueue,
    session: &mut EditorSession,
) {
    let mut open = true;
    egui::Window::new("Historique des modifications")
        .default_pos([380.0, 320.0])
        .default_size([420.0, 420.0])
        .open(&mut open)
        .show(ctx, |ui| {
            if history.records.is_empty() {
                ui.label("Aucune modification enregistrée.");
                return;
            }
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} en vigueur / {} au total",
                    history.pending_count(),
                    history.records.len()
                ));
                if ui.button("Tout annuler").clicked() {
                    // De la plus récente à la plus ancienne : annuler dans l'ordre
                    // inverse évite qu'une entrée ancienne soit réécrite par une
                    // plus récente restée en vigueur.
                    for record in history.records.iter().rev() {
                        if record.can_revert() {
                            revert_queue.0.push(record.seq);
                        }
                    }
                }
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for record in history.records.iter().rev() {
                    ui.horizontal(|ui| {
                        if record.reverted {
                            ui.label(egui::RichText::new(record.summary()).weak().strikethrough());
                            ui.label(egui::RichText::new("annulé").weak().italics());
                        } else {
                            ui.label(record.summary());
                            if ui
                                .add_enabled(record.can_revert(), egui::Button::new("Annuler"))
                                .clicked()
                            {
                                revert_queue.0.push(record.seq);
                            }
                        }
                    });
                    ui.label(
                        egui::RichText::new(format!(
                            "    {} · {}",
                            age_label(record.at_epoch_secs),
                            movement_summary(record)
                        ))
                        .small()
                        .weak(),
                    );
                }
            });
        });
    if !open {
        session.history_open = false;
    }
}

/// Résumé chiffré d'une entrée : ce qui a bougé, et de combien. C'est ce qui
/// permet de repérer l'entrée fautive sans la tester (« +102,6 m en Y »).
fn movement_summary(record: &crate::history::EditRecord) -> String {
    match (record.before, record.after) {
        (Some(before), Some(after)) => {
            let delta = Vec3::from_array(after.position) - Vec3::from_array(before.position);
            let scale_ratio = Vec3::from_array(after.scale).length()
                / Vec3::from_array(before.scale).length().max(f32::EPSILON);
            if delta.length() > 0.001 {
                format!("Δ {:+.2} ; {:+.2} ; {:+.2} m", delta.x, delta.y, delta.z)
            } else if (scale_ratio - 1.0).abs() > 0.001 {
                format!("taille ×{scale_ratio:.2}")
            } else {
                "rotation".to_owned()
            }
        }
        (None, Some(after)) => format!(
            "posé en {:.1} ; {:.1} ; {:.1}",
            after.position[0], after.position[1], after.position[2]
        ),
        (Some(before), None) => format!(
            "était en {:.1} ; {:.1} ; {:.1}",
            before.position[0], before.position[1], before.position[2]
        ),
        (None, None) => "—".to_owned(),
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
                    egui::RichText::new(format!(
                        "+ {} autre(s) sélectionné(s)",
                        selection.items.len() - 1
                    ))
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

            // Trois champs et non une taille unique : une pièce du château peut
            // porter une échelle **miroir** (négative), qu'un `splat` uniforme
            // écraserait silencieusement.
            ui.horizontal(|ui| {
                ui.label("Taille");
                changed |= drag(ui, &mut transform.scale.x, "X", 0.01);
                changed |= drag(ui, &mut transform.scale.y, "Y", 0.01);
                changed |= drag(ui, &mut transform.scale.z, "Z", 0.01);
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
                crate::transform_ops::record_transform(edits, entity, &snapshot, q_prop, q_decor);
            }
        });
}

fn drag(ui: &mut egui::Ui, value: &mut f32, prefix: &str, speed: f32) -> bool {
    ui.add(
        egui::DragValue::new(value)
            .speed(speed)
            .prefix(format!("{prefix} "))
            .max_decimals(3),
    )
    .changed()
}
