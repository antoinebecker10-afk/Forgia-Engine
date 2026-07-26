//! Sélection — désigne la pièce visée, la surligne, gère supprimer / dupliquer.
//!
//! Deux natures d'objets éditables, volontairement distinguées :
//!
//! - [`EditorProp`] : asset **ajouté par l'éditeur**. Il existe uniquement parce
//!   que le fichier d'édition le décrit → supprimable pour de vrai.
//! - [`EditorDecor`] : pièce **préexistante** du décor (château, terrain, végétation).
//!   Elle appartient au GLB : on n'en persiste qu'un *override* de transform, et
//!   « supprimer » veut dire « masquer ».

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use bevy::scene::SceneRoot;

use crate::persist::SceneEdits;
use crate::pick::EditorRay;
use crate::transform_ops::{ActiveOp, OpKind};
use crate::EditorSession;

/// Asset placé par l'éditeur. `id` fait le lien avec l'entrée persistée.
#[derive(Component, Debug, Clone)]
pub struct EditorProp {
    pub id: u32,
    /// Chemin asset (relatif à `assets/`) — affiché dans l'inspecteur.
    pub asset: String,
}

/// Pièce de décor préexistante adoptée par l'éditeur (transform overridé).
#[derive(Component, Debug, Clone)]
pub struct EditorDecor {
    /// Clé stable inter-sessions : `<scène>#<index>:<nom de nœud>`.
    pub key: String,
}

/// Profondeur maximale remontée dans la hiérarchie pour retrouver la pièce.
/// Les scènes glTF du château tiennent largement dans cette marge.
const MAX_HIERARCHY_WALK: usize = 16;

#[derive(Resource, Default)]
pub struct Selection {
    pub hovered: Option<Entity>,
    pub items: Vec<Entity>,
}

impl Selection {
    pub fn clear(&mut self) {
        self.hovered = None;
        self.items.clear();
    }

    pub fn primary(&self) -> Option<Entity> {
        self.items.first().copied()
    }
}

/// Requêtes de résolution de hiérarchie, regroupées pour éviter des signatures
/// démesurées dans chaque système qui en a besoin.
#[derive(bevy::ecs::system::SystemParam)]
pub struct HierarchyLookup<'w, 's> {
    pub parent: Query<'w, 's, &'static ChildOf>,
    pub children: Query<'w, 's, &'static Children>,
    pub scene: Query<'w, 's, &'static SceneRoot>,
    pub name: Query<'w, 's, &'static Name>,
    pub prop: Query<'w, 's, (), With<EditorProp>>,
}

impl HierarchyLookup<'_, '_> {
    /// Remonte du mesh touché jusqu'à l'objet **éditable** :
    /// - un [`EditorProp`] si on est dans un asset placé par l'éditeur ;
    /// - sinon le nœud de plus haut niveau sous la racine de scène — c'est la
    ///   « pièce » au sens du créateur (un tonneau, une colonne), pas le mesh
    ///   primitif ni la cellule entière de streaming.
    pub fn editable_root(&self, hit: Entity) -> Option<Entity> {
        let mut current = hit;
        for _ in 0..MAX_HIERARCHY_WALK {
            if self.prop.contains(current) {
                return Some(current);
            }
            let Ok(child_of) = self.parent.get(current) else {
                // Racine atteinte sans racine de scène : l'entité elle-même.
                return Some(current);
            };
            let parent = child_of.parent();
            if self.prop.contains(parent) {
                return Some(parent);
            }
            if self.scene.contains(parent) {
                // Le parent est la racine de scène → `current` est la pièce.
                return Some(current);
            }
            current = parent;
        }
        Some(current)
    }

    /// Clé de persistance d'une pièce de décor. `None` si la pièce n'est pas
    /// directement sous une racine de scène (cas d'un objet spawné à la main).
    pub fn decor_key(&self, piece: Entity) -> Option<String> {
        let parent = self.parent.get(piece).ok()?.parent();
        let scene = self.scene.get(parent).ok()?;
        let path = scene.0.path()?.to_string();
        let index = self
            .children
            .get(parent)
            .ok()?
            .iter()
            .position(|child| child == piece)?;
        let name = self
            .name
            .get(piece)
            .map(|n| n.as_str().to_owned())
            .unwrap_or_else(|_| DEFAULT_NODE_NAME.to_owned());
        Some(decor_key_from_parts(&path, index, &name))
    }
}

/// Nom de repli pour un nœud glTF anonyme.
pub const DEFAULT_NODE_NAME: &str = "node";

/// Format **unique** de la clé de persistance d'une pièce de décor.
///
/// L'index de fratrie discrimine les nœuds homonymes (un pack en contient
/// beaucoup : `SM_MOD_wall`, `SM_MOD_wall`…) et reste stable d'une session à
/// l'autre, l'ordre des nœuds glTF étant déterministe. Les deux appelants —
/// sélection ([`HierarchyLookup::decor_key`]) et réapplication au streaming
/// ([`crate::persist::sys_apply_overrides`]) — partagent cette fonction : deux
/// formats divergents feraient silencieusement perdre les éditions.
pub fn decor_key_from_parts(scene_path: &str, sibling_index: usize, node_name: &str) -> String {
    format!("{scene_path}#{sibling_index}:{node_name}")
}

/// Survol : résout la pièce visée à partir du balayage AABB.
pub fn sys_hover(ray: Res<EditorRay>, lookup: HierarchyLookup, mut selection: ResMut<Selection>) {
    selection.hovered = ray.picked.and_then(|hit| lookup.editable_root(hit));
}

/// Clic gauche = sélectionner. `Maj` ajoute / retire de la sélection.
///
/// Ne fait rien quand le pointeur est sur un panneau egui (drapeau posé par le
/// panneau à la frame précédente), quand on navigue (clic droit maintenu) ou
/// quand une opération de transform est en cours — dans ce dernier cas le clic
/// gauche **valide** l'opération et appartient à `transform_ops`.
pub fn sys_click(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<EditorSession>,
    op: Res<ActiveOp>,
    mut commands: Commands,
    lookup: HierarchyLookup,
    mut selection: ResMut<Selection>,
) {
    if session.ui_capture || session.navigating || op.kind != OpKind::None {
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let additive = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let Some(target) = selection.hovered else {
        if !additive {
            selection.items.clear();
        }
        return;
    };

    // Une pièce de décor n'est marquée qu'au moment où on la sélectionne :
    // marquer les 7 400 pièces du château à l'avance serait du gaspillage. Aucune
    // entrée n'est écrite dans le fichier ici — sélectionner n'est pas modifier.
    if !lookup.prop.contains(target) {
        if let Some(key) = lookup.decor_key(target) {
            commands.entity(target).insert(EditorDecor { key });
        }
    }

    if additive {
        if let Some(pos) = selection.items.iter().position(|&e| e == target) {
            selection.items.remove(pos);
        } else {
            selection.items.push(target);
        }
    } else {
        selection.items.clear();
        selection.items.push(target);
    }
}

/// Supprimer (`Suppr`) et dupliquer (`Ctrl+D`).
pub fn sys_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    session: Res<EditorSession>,
    op: Res<ActiveOp>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut edits: ResMut<SceneEdits>,
    mut selection: ResMut<Selection>,
    q_prop: Query<(&EditorProp, &Transform)>,
    mut q_decor: Query<(&EditorDecor, &mut Visibility)>,
    mut status: ResMut<crate::EditorStatus>,
) {
    if session.ui_capture || op.kind != OpKind::None {
        return;
    }
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    if keys.just_pressed(KeyCode::Delete) {
        let mut removed = 0u32;
        let mut hidden = 0u32;
        for entity in std::mem::take(&mut selection.items) {
            if let Ok((prop, _)) = q_prop.get(entity) {
                edits.remove_prop(prop.id);
                commands.entity(entity).despawn();
                removed += 1;
            } else if let Ok((decor, mut visibility)) = q_decor.get_mut(entity) {
                *visibility = Visibility::Hidden;
                edits.set_override_hidden(&decor.key, true);
                hidden += 1;
            }
        }
        selection.hovered = None;
        status.set(format!("Supprimé {removed} · masqué {hidden}"));
        return;
    }

    if ctrl && keys.just_pressed(KeyCode::KeyD) {
        let mut spawned = Vec::new();
        for &entity in &selection.items {
            let Ok((prop, transform)) = q_prop.get(entity) else {
                continue;
            };
            let mut clone = *transform;
            clone.translation += Vec3::new(DUPLICATE_OFFSET_M, 0.0, DUPLICATE_OFFSET_M);
            let id = edits.push_prop(&prop.asset, &clone);
            spawned.push(crate::library::spawn_prop_entity(
                &mut commands,
                &asset_server,
                id,
                &prop.asset,
                clone,
            ));
        }
        if !spawned.is_empty() {
            status.set(format!("Dupliqué {} objet(s)", spawned.len()));
            selection.items = spawned;
        }
    }
}

/// Décalage appliqué à une copie pour qu'elle ne soit pas cachée dans l'original.
const DUPLICATE_OFFSET_M: f32 = 1.0;

/// Surligne survol (discret) et sélection (net), via la boîte englobante réelle
/// du sous-arbre — pas un cube de taille fixe qui mentirait sur l'emprise.
pub fn sys_draw_highlight(
    selection: Res<Selection>,
    q_children: Query<&Children>,
    q_shape: Query<(&GlobalTransform, &Aabb)>,
    mut scratch: Local<Vec<Entity>>,
    mut gizmos: Gizmos,
) {
    if let Some(hovered) = selection.hovered {
        if !selection.items.contains(&hovered) {
            if let Some((center, size)) = world_bounds(hovered, &q_children, &q_shape, &mut scratch)
            {
                gizmos.cube(
                    Transform::from_translation(center).with_scale(size),
                    HOVER_COLOR,
                );
            }
        }
    }
    for &entity in &selection.items {
        if let Some((center, size)) = world_bounds(entity, &q_children, &q_shape, &mut scratch) {
            gizmos.cube(
                Transform::from_translation(center).with_scale(size),
                SELECTED_COLOR,
            );
        }
    }
}

/// Or pâle translucide — cohérent avec la DA « Verre & Braise » sans dépendre
/// de `forgia-ui` (l'éditeur reste un outil, pas un écran de jeu).
const HOVER_COLOR: Color = Color::srgba(1.0, 0.85, 0.45, 0.35);
const SELECTED_COLOR: Color = Color::srgba(1.0, 0.72, 0.20, 0.95);
/// Épaisseur minimale d'une boîte de surlignage (une pièce plate resterait
/// invisible sinon).
const MIN_BOUNDS_SIZE_M: f32 = 0.05;

/// Boîte englobante **monde** d'un sous-arbre : centre + dimensions.
///
/// `scratch` est un tampon réutilisé (`Local`) : ce système tourne chaque frame,
/// il ne doit pas allouer.
pub fn world_bounds(
    root: Entity,
    q_children: &Query<&Children>,
    q_shape: &Query<(&GlobalTransform, &Aabb)>,
    scratch: &mut Vec<Entity>,
) -> Option<(Vec3, Vec3)> {
    scratch.clear();
    scratch.push(root);
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    let mut found = false;

    while let Some(entity) = scratch.pop() {
        if let Ok((tf, aabb)) = q_shape.get(entity) {
            let affine = tf.affine();
            let local_min = Vec3::from(aabb.min());
            let local_max = Vec3::from(aabb.max());
            // Les 8 coins transformés : une boîte tournée reste correctement
            // encadrée (transformer seulement min/max donnerait une boîte fausse).
            for corner in 0..8 {
                let local = Vec3::new(
                    if corner & 1 == 0 { local_min.x } else { local_max.x },
                    if corner & 2 == 0 { local_min.y } else { local_max.y },
                    if corner & 4 == 0 { local_min.z } else { local_max.z },
                );
                let world = affine.transform_point3(local);
                min = min.min(world);
                max = max.max(world);
            }
            found = true;
        }
        if let Ok(children) = q_children.get(entity) {
            scratch.extend(children.iter());
        }
    }

    if !found {
        return None;
    }
    let size = (max - min).max(Vec3::splat(MIN_BOUNDS_SIZE_M));
    Some(((min + max) * 0.5, size))
}

/// Ne laisse pas une sélection pointer une entité déspawnée (cellule de décor
/// déchargée par le streaming du Hall pendant l'édition).
pub fn sys_prune_selection(mut selection: ResMut<Selection>, q_alive: Query<Entity>) {
    selection.items.retain(|&entity| q_alive.contains(entity));
    if let Some(hovered) = selection.hovered {
        if !q_alive.contains(hovered) {
            selection.hovered = None;
        }
    }
}

/// Vide la sélection quand on quitte le Hall.
pub fn clear_selection(mut selection: ResMut<Selection>) {
    selection.clear();
}
