//! Bibliothèque d'assets — ce qu'on peut ajouter dans le Hall.
//!
//! La liste est **balayée sur le disque** au lieu d'être maintenue à la main :
//! tout GLB/glTF présent sous `assets/models/` et `assets/models-v1/` apparaît,
//! rangé par dossier. Le dossier EST le tri (« trier correctement ») — c'est déjà
//! la façon dont les packs sont organisés, dupliquer ce classement dans un TOML
//! ne ferait que créer une deuxième vérité à garder synchronisée.
//!
//! Le balayage a lieu une seule fois, à l'entrée dans le Hall.

use bevy::prelude::*;
use bevy::scene::SceneRoot;

use crate::select::{EditorProp, Selection};
use crate::snap::{NeedsGridSnap, NeedsGroundSnap, SnapMode};
use crate::{EditorSession, EditorStatus};

/// Racines balayées, relatives au dossier `assets/`.
const LIBRARY_ROOTS: [&str; 2] = ["models", "models-v1"];
/// Dossiers ignorés : rendus intermédiaires, sources de travail, et les cellules
/// du château qui **sont** le Hall (les replacer n'a aucun sens et elles pèsent
/// des dizaines de Mo).
const SKIPPED_DIRECTORIES: [&str; 6] = [
    "previews",
    "src",
    "cells",
    "castle_stream_cells",
    "castle_stream_cells_grass",
    "castle_stream_cells_textured",
];
/// Garde-fou de profondeur du balayage (les packs sont plats ou peu imbriqués).
const MAX_SCAN_DEPTH: usize = 8;

pub struct LibraryEntry {
    /// Chemin chargeable par l'`AssetServer` (relatif à `assets/`).
    pub asset: String,
    /// Nom de fichier sans extension — ce que voit le créateur.
    pub label: String,
}

pub struct LibraryGroup {
    /// Chemin du dossier, relatif à `assets/`.
    pub label: String,
    pub entries: Vec<LibraryEntry>,
}

#[derive(Resource, Default)]
pub struct EditorLibrary {
    pub groups: Vec<LibraryGroup>,
    pub total: usize,
    pub scanned: bool,
    pub error: Option<String>,
}

/// Assets à instancier, remplie par le panneau et consommée en `Update` : le
/// panneau reste de l'affichage, le spawn reste de la logique.
#[derive(Resource, Default)]
pub struct SpawnQueue(pub Vec<String>);

/// Marqueur de nettoyage — tout ce que l'éditeur instancie disparaît en quittant
/// le Hall (le fichier d'édition, lui, garde la scène).
#[derive(Component)]
pub struct EditorSpawned;

/// Balaye la bibliothèque une fois, à l'entrée dans le Hall.
pub fn scan_library(mut library: ResMut<EditorLibrary>) {
    library.groups.clear();
    library.total = 0;
    library.error = None;

    for root in LIBRARY_ROOTS {
        let path = std::path::Path::new("assets").join(root);
        if !path.is_dir() {
            continue;
        }
        collect_directory(&path, root, 0, &mut library);
    }

    library.groups.sort_by(|a, b| a.label.cmp(&b.label));
    for group in &mut library.groups {
        group.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }
    library.scanned = true;
    if library.total == 0 {
        library.error = Some("aucun asset trouvé sous assets/models".to_owned());
        warn!("[forgia-editor] bibliothèque vide — lancement hors racine du repo ?");
    } else {
        info!(
            "[forgia-editor] bibliothèque : {} asset(s) dans {} dossier(s)",
            library.total,
            library.groups.len()
        );
    }
}

fn collect_directory(
    directory: &std::path::Path,
    relative: &str,
    depth: usize,
    library: &mut EditorLibrary,
) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            library.error = Some(format!("{}: {error}", directory.display()));
            return;
        }
    };
    let mut models = Vec::new();
    let mut subdirectories = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        // Nom possédé : `path` est déplacé plus bas dans la liste des sous-dossiers.
        let Some(name) = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            continue;
        };
        if path.is_dir() {
            if !SKIPPED_DIRECTORIES.contains(&name.as_str()) {
                subdirectories.push((path, format!("{relative}/{name}")));
            }
            continue;
        }
        let is_model = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| {
                extension.eq_ignore_ascii_case("glb") || extension.eq_ignore_ascii_case("gltf")
            })
            .unwrap_or(false);
        if !is_model {
            continue;
        }
        let label = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(name.as_str())
            .to_owned();
        models.push(LibraryEntry {
            asset: format!("{relative}/{name}"),
            label,
        });
    }

    if !models.is_empty() {
        library.total += models.len();
        library.groups.push(LibraryGroup {
            label: relative.to_owned(),
            entries: models,
        });
    }
    for (path, child_relative) in subdirectories {
        collect_directory(&path, &child_relative, depth + 1, library);
    }
}

/// Instancie un asset placé par l'éditeur. Fonction partagée par le spawn
/// interactif, la duplication et le rechargement du fichier d'édition — une
/// seule définition de « à quoi ressemble un prop ».
pub fn spawn_prop_entity(
    commands: &mut Commands,
    asset_server: &AssetServer,
    id: u32,
    asset: &str,
    transform: Transform,
) -> Entity {
    commands
        .spawn((
            EditorProp {
                id,
                asset: asset.to_owned(),
            },
            EditorSpawned,
            SceneRoot(asset_server.load(format!("{asset}#Scene0"))),
            transform,
            Name::new(format!("EditorProp#{id}")),
        ))
        .id()
}

/// Consomme la file de spawn : place l'asset au point visé, l'enregistre et le
/// sélectionne (on peut enchaîner directement sur `G` pour l'ajuster).
pub fn sys_process_spawn_queue(
    mut queue: ResMut<SpawnQueue>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    ray: Res<crate::pick::EditorRay>,
    session: Res<EditorSession>,
    mut edits: ResMut<crate::persist::SceneEdits>,
    mut selection: ResMut<Selection>,
    mut status: ResMut<EditorStatus>,
) {
    if queue.0.is_empty() {
        return;
    }
    let mut spawned = Vec::new();
    for asset in queue.0.drain(..) {
        let transform = Transform::from_translation(ray.placement_point());
        let id = edits.push_prop(&asset, &transform);
        let entity = spawn_prop_entity(&mut commands, &asset_server, id, &asset, transform);
        // La boîte englobante n'est connue qu'une fois la scène instanciée : le
        // système de magnétisme réessaie jusque-là. Un objet neuf est toujours
        // posé au sol si l'aimant est actif, même en mode Grille (qui ne corrige
        // que X/Z) — c'est ce qui évite un asset planté dans le sol au 1er clic.
        match session.snap {
            SnapMode::Ground => {
                commands.entity(entity).insert(NeedsGroundSnap::default());
            }
            SnapMode::Grid => {
                commands
                    .entity(entity)
                    .insert((NeedsGroundSnap::default(), NeedsGridSnap));
            }
            SnapMode::Off => {}
        }
        spawned.push(entity);
        status.set(format!("Ajouté : {asset}"));
    }
    if !spawned.is_empty() {
        selection.items = spawned;
    }
}

/// Retire les assets instanciés par l'éditeur en quittant le Hall.
pub fn cleanup_spawned(mut commands: Commands, q_spawned: Query<Entity, With<EditorSpawned>>) {
    for entity in &q_spawned {
        commands.entity(entity).despawn();
    }
}
