//! Persistance des éditions — `castle_hub_edits.json` à la racine du repo.
//!
//! **Non destructif** : on ne réécrit jamais un GLB livré. Le fichier décrit
//! seulement les deux formes d'édition possibles :
//!
//! - `props` : les assets **ajoutés** par l'éditeur (asset + transform) ;
//! - `overrides` : les pièces **préexistantes** déplacées ou masquées, désignées
//!   par une clé stable `<scène>#<index>:<nom de nœud>`.
//!
//! Les cellules visuelles du Hall entrent et sortent par streaming : un override
//! est donc réappliqué à chaque instanciation de sa scène, pas une seule fois au
//! chargement (`sys_apply_overrides`).

use bevy::prelude::*;
use bevy::scene::SceneRoot;
use serde::{Deserialize, Serialize};

use crate::select::{decor_key_from_parts, EditorDecor};

/// Fichier d'édition (racine du repo, à côté de `castle_ground_tune.json`).
const EDITS_PATH: &str = "castle_hub_edits.json";
/// Version du format — toute sauvegarde est versionnée (règle `scalability`).
const FORMAT_VERSION: u32 = 1;
/// Délai de regroupement avant écriture : on ne touche pas le disque à chaque
/// pixel de souris, seulement quand l'édition s'est stabilisée.
const AUTOSAVE_DELAY_SECS: f32 = 2.0;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PropEntry {
    pub id: u32,
    /// Chemin relatif à `assets/` (chargeable tel quel par l'`AssetServer`).
    pub asset: String,
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OverrideEntry {
    pub key: String,
    /// `None` = transform inchangé (l'entrée n'existe que pour `hidden`).
    #[serde(default)]
    pub position: Option<[f32; 3]>,
    #[serde(default)]
    pub rotation: Option<[f32; 4]>,
    #[serde(default)]
    pub scale: Option<[f32; 3]>,
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct EditsFile {
    version: u32,
    #[serde(default)]
    props: Vec<PropEntry>,
    #[serde(default)]
    overrides: Vec<OverrideEntry>,
}

/// État d'édition de la scène — vérité runtime, miroir du fichier.
#[derive(Resource, Default)]
pub struct SceneEdits {
    pub props: Vec<PropEntry>,
    pub overrides: Vec<OverrideEntry>,
    dirty: bool,
    dirty_timer: f32,
    next_id: u32,
    /// Résultat de la dernière écriture — lu par le capteur (une édition non
    /// sauvegardée est une perte de travail, donc une alerte).
    pub last_save_ok: bool,
    pub last_error: Option<String>,
    pub saves: u32,
}

impl SceneEdits {
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Enregistre un asset ajouté et retourne son identifiant.
    pub fn push_prop(&mut self, asset: &str, transform: &Transform) -> u32 {
        self.next_id += 1;
        let id = self.next_id;
        self.props.push(PropEntry {
            id,
            asset: asset.to_owned(),
            position: transform.translation.to_array(),
            rotation: transform.rotation.to_array(),
            scale: transform.scale.to_array(),
        });
        self.dirty = true;
        self.dirty_timer = 0.0;
        id
    }

    pub fn update_prop(&mut self, id: u32, transform: &Transform) {
        if let Some(entry) = self.props.iter_mut().find(|entry| entry.id == id) {
            entry.position = transform.translation.to_array();
            entry.rotation = transform.rotation.to_array();
            entry.scale = transform.scale.to_array();
            self.dirty = true;
            self.dirty_timer = 0.0;
        }
    }

    pub fn remove_prop(&mut self, id: u32) {
        let before = self.props.len();
        self.props.retain(|entry| entry.id != id);
        if self.props.len() != before {
            self.dirty = true;
            self.dirty_timer = 0.0;
        }
    }

    pub fn update_override(&mut self, key: &str, transform: &Transform) {
        let entry = self.override_mut(key);
        entry.position = Some(transform.translation.to_array());
        entry.rotation = Some(transform.rotation.to_array());
        entry.scale = Some(transform.scale.to_array());
        self.dirty = true;
        self.dirty_timer = 0.0;
    }

    pub fn set_override_hidden(&mut self, key: &str, hidden: bool) {
        self.override_mut(key).hidden = hidden;
        self.dirty = true;
        self.dirty_timer = 0.0;
    }

    fn override_mut(&mut self, key: &str) -> &mut OverrideEntry {
        if let Some(index) = self
            .overrides
            .iter()
            .position(|entry| entry.key.as_str() == key)
        {
            return &mut self.overrides[index];
        }
        self.overrides.push(OverrideEntry {
            key: key.to_owned(),
            position: None,
            rotation: None,
            scale: None,
            hidden: false,
        });
        self.overrides
            .last_mut()
            .expect("entrée insérée juste au-dessus")
    }

    pub fn find_override(&self, key: &str) -> Option<&OverrideEntry> {
        self.overrides
            .iter()
            .find(|entry| entry.key.as_str() == key)
    }
}

/// Marque une racine de scène dont les overrides ont déjà été appliqués.
#[derive(Component)]
pub struct OverridesApplied;

/// Charge le fichier d'édition et respawn les assets ajoutés (OnEnter du Hall).
pub fn load_edits(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut edits: ResMut<SceneEdits>,
) {
    *edits = SceneEdits {
        last_save_ok: true,
        ..default()
    };
    let raw = match std::fs::read_to_string(EDITS_PATH) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            info!("[forgia-editor] aucun {EDITS_PATH} — Hall à son état livré");
            return;
        }
        Err(error) => {
            warn!("[forgia-editor] lecture {EDITS_PATH} impossible : {error}");
            edits.last_error = Some(error.to_string());
            return;
        }
    };
    let file: EditsFile = match serde_json::from_str(&raw) {
        Ok(file) => file,
        Err(error) => {
            // On ne réécrit PAS par-dessus un fichier illisible : le travail du
            // créateur reste sur le disque, l'éditeur démarre juste vide.
            error!("[forgia-editor] {EDITS_PATH} illisible : {error}");
            edits.last_error = Some(error.to_string());
            edits.last_save_ok = false;
            return;
        }
    };
    if file.version != FORMAT_VERSION {
        warn!(
            "[forgia-editor] {EDITS_PATH} version {} (attendu {FORMAT_VERSION}) — chargé quand même",
            file.version
        );
    }
    edits.next_id = file.props.iter().map(|entry| entry.id).max().unwrap_or(0);
    for entry in &file.props {
        let transform = transform_from(entry.position, entry.rotation, entry.scale);
        crate::library::spawn_prop_entity(
            &mut commands,
            &asset_server,
            entry.id,
            &entry.asset,
            transform,
        );
    }
    let (props, overrides) = (file.props.len(), file.overrides.len());
    edits.props = file.props;
    edits.overrides = file.overrides;
    info!("[forgia-editor] {EDITS_PATH} chargé : {props} objet(s), {overrides} override(s)");
}

fn transform_from(position: [f32; 3], rotation: [f32; 4], scale: [f32; 3]) -> Transform {
    Transform {
        translation: Vec3::from_array(position),
        rotation: Quat::from_array(rotation).normalize(),
        scale: Vec3::from_array(scale),
    }
}

/// Écrit le fichier quand l'édition s'est stabilisée (`AUTOSAVE_DELAY_SECS`).
///
/// `Time<Real>` et non `Time<Virtual>` : c'est un outil, il ne doit pas geler
/// avec la pause du jeu (anti-trap V1 « Time<Real> vs Time<Virtual> »).
pub fn sys_autosave(time: Res<Time<Real>>, mut edits: ResMut<SceneEdits>) {
    if !edits.dirty {
        return;
    }
    edits.dirty_timer += time.delta_secs();
    if edits.dirty_timer < AUTOSAVE_DELAY_SECS {
        return;
    }
    save_now(&mut edits);
}

/// Écriture immédiate (autosave arrivé à échéance, fermeture de l'éditeur,
/// sortie du Hall). Passe par un fichier temporaire puis un renommage : une
/// coupure en cours d'écriture ne laisse pas un JSON tronqué.
pub fn save_now(edits: &mut SceneEdits) {
    let file = EditsFile {
        version: FORMAT_VERSION,
        props: edits.props.clone(),
        overrides: edits.overrides.clone(),
    };
    let json = match serde_json::to_string_pretty(&file) {
        Ok(json) => json,
        Err(error) => {
            error!("[forgia-editor] sérialisation impossible : {error}");
            edits.last_save_ok = false;
            edits.last_error = Some(error.to_string());
            return;
        }
    };
    let temporary = format!("{EDITS_PATH}.tmp");
    let result = std::fs::write(&temporary, json).and_then(|()| {
        // `rename` écrase la cible sur Windows comme sur Unix quand la cible est
        // un fichier existant, ce qui rend la publication atomique.
        std::fs::rename(&temporary, EDITS_PATH)
    });
    match result {
        Ok(()) => {
            edits.dirty = false;
            edits.dirty_timer = 0.0;
            edits.last_save_ok = true;
            edits.last_error = None;
            edits.saves += 1;
            info!(
                "[forgia-editor] {EDITS_PATH} sauvegardé ({} objet(s), {} override(s))",
                edits.props.len(),
                edits.overrides.len()
            );
        }
        Err(error) => {
            error!("[forgia-editor] écriture {EDITS_PATH} impossible : {error}");
            edits.last_save_ok = false;
            edits.last_error = Some(error.to_string());
            let _ = std::fs::remove_file(&temporary);
        }
    }
}

/// Sauvegarde en quittant le Hall — aucune édition ne doit se perdre parce que
/// le délai d'autosave n'était pas écoulé.
pub fn flush_on_exit(mut edits: ResMut<SceneEdits>) {
    if edits.dirty {
        save_now(&mut edits);
    }
}

/// Réapplique les overrides aux scènes fraîchement instanciées.
///
/// Le streaming du Hall charge et décharge les cellules du château : une pièce
/// déplacée doit retrouver sa position à chaque réapparition. On ne balaie que
/// les racines dont le chemin de scène apparaît dans les overrides, et une seule
/// fois par racine (marqueur [`OverridesApplied`]).
pub fn sys_apply_overrides(
    mut commands: Commands,
    edits: Res<SceneEdits>,
    q_roots: Query<(Entity, &SceneRoot), Without<OverridesApplied>>,
    q_children: Query<&Children>,
    q_name: Query<&Name>,
    mut q_transform: Query<&mut Transform>,
    mut q_visibility: Query<&mut Visibility>,
) {
    if edits.overrides.is_empty() {
        return;
    }
    for (root, scene) in &q_roots {
        let Some(path) = scene.0.path().map(|path| path.to_string()) else {
            continue;
        };
        let Ok(children) = q_children.get(root) else {
            // Scène pas encore instanciée : on retentera à la frame suivante.
            continue;
        };
        if !edits
            .overrides
            .iter()
            .any(|entry| entry.key.starts_with(&path))
        {
            commands.entity(root).insert(OverridesApplied);
            continue;
        }
        for (index, piece) in children.iter().enumerate() {
            let name = q_name
                .get(piece)
                .map(|name| name.as_str().to_owned())
                .unwrap_or_else(|_| crate::select::DEFAULT_NODE_NAME.to_owned());
            let key = decor_key_from_parts(&path, index, &name);
            let Some(entry) = edits.find_override(&key) else {
                continue;
            };
            if let Ok(mut transform) = q_transform.get_mut(piece) {
                if let Some(position) = entry.position {
                    transform.translation = Vec3::from_array(position);
                }
                if let Some(rotation) = entry.rotation {
                    transform.rotation = Quat::from_array(rotation).normalize();
                }
                if let Some(scale) = entry.scale {
                    transform.scale = Vec3::from_array(scale);
                }
            }
            if entry.hidden {
                if let Ok(mut visibility) = q_visibility.get_mut(piece) {
                    *visibility = Visibility::Hidden;
                }
            }
            commands.entity(piece).insert(EditorDecor { key });
        }
        commands.entity(root).insert(OverridesApplied);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_prop_assigns_increasing_ids() {
        let mut edits = SceneEdits::default();
        let first = edits.push_prop("models/a.glb", &Transform::default());
        let second = edits.push_prop("models/b.glb", &Transform::default());
        assert_eq!((first, second), (1, 2));
        assert!(edits.dirty());
    }

    #[test]
    fn remove_prop_only_marks_dirty_when_it_removed_something() {
        let mut edits = SceneEdits::default();
        let id = edits.push_prop("models/a.glb", &Transform::default());
        edits.dirty = false;
        edits.remove_prop(id + 99);
        assert!(!edits.dirty(), "aucune suppression ne doit pas salir l'état");
        edits.remove_prop(id);
        assert!(edits.dirty());
        assert!(edits.props.is_empty());
    }

    #[test]
    fn override_is_created_once_then_updated_in_place() {
        let mut edits = SceneEdits::default();
        edits.update_override("scene#0:Barrel", &Transform::from_xyz(1.0, 2.0, 3.0));
        edits.set_override_hidden("scene#0:Barrel", true);
        assert_eq!(edits.overrides.len(), 1);
        let entry = edits.find_override("scene#0:Barrel").expect("entrée");
        assert_eq!(entry.position, Some([1.0, 2.0, 3.0]));
        assert!(entry.hidden);
    }

    #[test]
    fn transform_round_trip_keeps_values() {
        let source = Transform {
            translation: Vec3::new(1.5, -2.0, 30.0),
            rotation: Quat::from_rotation_y(0.7),
            scale: Vec3::new(2.0, 2.0, 2.0),
        };
        let mut edits = SceneEdits::default();
        let id = edits.push_prop("models/a.glb", &source);
        let entry = edits.props.iter().find(|e| e.id == id).expect("entrée");
        let restored = transform_from(entry.position, entry.rotation, entry.scale);
        assert!((restored.translation - source.translation).length() < 1e-6);
        assert!(restored.rotation.angle_between(source.rotation) < 1e-5);
        assert!((restored.scale - source.scale).length() < 1e-6);
    }
}
