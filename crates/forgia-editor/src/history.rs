//! Historique des modifications — journal consultable et **annulable à la main**,
//! entrée par entrée.
//!
//! `Ctrl+Z` annule la dernière transformation ; l'historique répond à un autre
//! besoin : *« qu'est-ce que j'ai touché, et comment je reviens en arrière sur
//! CE truc-là »*. Il persiste dans `castle_hub_edits.json`, donc il survit à un
//! redémarrage — sans quoi « annuler à la main » ne vaudrait que pour la session
//! en cours.
//!
//! Chaque entrée porte l'état **avant** et **après**, ce qui rend l'annulation
//! indépendante de l'ordre : annuler l'entrée #3 ne casse pas #4, chacune sait
//! restaurer sa propre cible.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::persist::SceneEdits;
use crate::select::{EditorDecor, EditorProp};
use crate::EditorStatus;

/// Profondeur maximale du journal. Au-delà, les plus anciennes entrées tombent :
/// un journal non borné finirait par peser dans le fichier de scène.
const MAX_RECORDS: usize = 200;
/// Seuil sous lequel un ajustement n'est pas jugé digne d'une entrée (bruit de
/// magnétisme sub-millimétrique).
const MIN_RECORDED_MOVE_M: f32 = 0.001;

/// Transform sérialisable — même représentation que les entrées de scène.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct TransformData {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

impl From<&Transform> for TransformData {
    fn from(transform: &Transform) -> Self {
        Self {
            position: transform.translation.to_array(),
            rotation: transform.rotation.to_array(),
            scale: transform.scale.to_array(),
        }
    }
}

impl TransformData {
    pub fn to_transform(self) -> Transform {
        Transform {
            translation: Vec3::from_array(self.position),
            rotation: Quat::from_array(self.rotation).normalize(),
            scale: Vec3::from_array(self.scale),
        }
    }

    fn moved_from(self, other: Self) -> bool {
        Vec3::from_array(self.position).distance(Vec3::from_array(other.position))
            > MIN_RECORDED_MOVE_M
            || self.scale != other.scale
            || self.rotation != other.rotation
    }
}

/// Objet visé par une modification.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum EditTarget {
    /// Asset ajouté par l'éditeur.
    Prop { id: u32 },
    /// Pièce préexistante du décor.
    Decor { key: String },
}

/// Nature de la modification.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    Added,
    Moved,
    Rotated,
    Resized,
    Snapped,
    Deleted,
    Hidden,
}

impl EditKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "Ajouté",
            Self::Moved => "Déplacé",
            Self::Rotated => "Tourné",
            Self::Resized => "Redimensionné",
            Self::Snapped => "Posé au sol",
            Self::Deleted => "Supprimé",
            Self::Hidden => "Masqué",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EditRecord {
    /// Numéro d'ordre, affiché et utilisé comme identifiant d'annulation.
    pub seq: u32,
    pub kind: EditKind,
    pub target: EditTarget,
    /// Nom lisible de la cible (nom de fichier de l'asset, nom de nœud glTF).
    pub label: String,
    /// Chemin de l'asset — nécessaire pour re-instancier un objet supprimé.
    #[serde(default)]
    pub asset: Option<String>,
    /// Horodatage epoch (secondes) — affiché en « il y a … ».
    #[serde(default)]
    pub at_epoch_secs: u64,
    pub before: Option<TransformData>,
    pub after: Option<TransformData>,
    #[serde(default)]
    pub reverted: bool,
}

impl EditRecord {
    /// Une entrée déjà annulée ne peut plus l'être ; un ajout dont on a perdu le
    /// chemin d'asset non plus (rien à re-instancier).
    pub fn can_revert(&self) -> bool {
        if self.reverted {
            return false;
        }
        match self.kind {
            EditKind::Deleted => self.asset.is_some() && self.before.is_some(),
            EditKind::Added | EditKind::Hidden => true,
            _ => self.before.is_some(),
        }
    }

    pub fn summary(&self) -> String {
        format!("#{} · {} · {}", self.seq, self.kind.label(), self.label)
    }
}

#[derive(Resource, Default)]
pub struct EditHistory {
    pub records: Vec<EditRecord>,
    next_seq: u32,
}

impl EditHistory {
    /// Journalise une modification. Retourne `false` si elle a été jugée sans
    /// effet (déplacement sous le seuil) et donc pas enregistrée.
    pub fn record(
        &mut self,
        kind: EditKind,
        target: EditTarget,
        label: impl Into<String>,
        asset: Option<String>,
        before: Option<TransformData>,
        after: Option<TransformData>,
    ) -> bool {
        // Un geste validé sans mouvement réel ne mérite pas une ligne.
        if let (Some(before), Some(after)) = (before, after) {
            if !after.moved_from(before) {
                return false;
            }
        }
        self.next_seq += 1;
        if self.records.len() == MAX_RECORDS {
            self.records.remove(0);
        }
        self.records.push(EditRecord {
            seq: self.next_seq,
            kind,
            target,
            label: label.into(),
            asset,
            at_epoch_secs: now_epoch_secs(),
            before,
            after,
            reverted: false,
        });
        true
    }

    /// Réaligne le compteur après un chargement depuis le disque.
    pub fn adopt(&mut self, records: Vec<EditRecord>) {
        self.next_seq = records.iter().map(|record| record.seq).max().unwrap_or(0);
        self.records = records;
    }

    pub fn pending_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| !record.reverted)
            .count()
    }
}

/// Horloge murale, uniquement pour l'affichage « il y a … ». Un échec de lecture
/// donne 0 : l'entrée reste utilisable, seule sa date est inconnue.
fn now_epoch_secs() -> u64 {
    web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

/// Âge lisible d'une entrée (« il y a 3 min »).
pub fn age_label(at_epoch_secs: u64) -> String {
    if at_epoch_secs == 0 {
        return "date inconnue".to_owned();
    }
    let elapsed = now_epoch_secs().saturating_sub(at_epoch_secs);
    match elapsed {
        0..=59 => format!("il y a {elapsed} s"),
        60..=3599 => format!("il y a {} min", elapsed / 60),
        _ => format!("il y a {} h", elapsed / 3600),
    }
}

/// Demandes d'annulation posées par le panneau, traitées en `Update`.
#[derive(Resource, Default)]
pub struct RevertQueue(pub Vec<u32>);

/// Annule les entrées demandées.
///
/// L'annulation passe **d'abord par le fichier d'édition**, l'entité vivante
/// n'étant mise à jour que si elle est chargée : une cellule du château peut être
/// déchargée par le streaming au moment où on annule, et il faut quand même que la
/// correction s'applique à son retour.
#[allow(clippy::too_many_arguments)]
pub fn sys_process_revert_queue(
    mut queue: ResMut<RevertQueue>,
    mut history: ResMut<EditHistory>,
    mut edits: ResMut<SceneEdits>,
    mut status: ResMut<EditorStatus>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut q_transform: Query<&mut Transform>,
    q_props: Query<(Entity, &EditorProp)>,
    q_decor: Query<(Entity, &EditorDecor)>,
) {
    if queue.0.is_empty() {
        return;
    }
    let requested: Vec<u32> = queue.0.drain(..).collect();
    let mut reverted = 0u32;

    for seq in requested {
        let Some(index) = history.records.iter().position(|record| record.seq == seq) else {
            continue;
        };
        if !history.records[index].can_revert() {
            continue;
        }
        let record = history.records[index].clone();
        let applied = revert_record(
            &record,
            &mut edits,
            &mut commands,
            &asset_server,
            &mut q_transform,
            &q_props,
            &q_decor,
        );
        if applied {
            history.records[index].reverted = true;
            reverted += 1;
        }
    }

    if reverted > 0 {
        status.set(format!("{reverted} modification(s) annulée(s)"));
    }
}

#[allow(clippy::too_many_arguments)]
fn revert_record(
    record: &EditRecord,
    edits: &mut SceneEdits,
    commands: &mut Commands,
    asset_server: &AssetServer,
    q_transform: &mut Query<&mut Transform>,
    q_props: &Query<(Entity, &EditorProp)>,
    q_decor: &Query<(Entity, &EditorDecor)>,
) -> bool {
    match (&record.kind, &record.target) {
        // Annuler un ajout = retirer l'objet.
        (EditKind::Added, EditTarget::Prop { id }) => {
            edits.remove_prop(*id);
            if let Some((entity, _)) = q_props.iter().find(|(_, prop)| prop.id == *id) {
                commands.entity(entity).despawn();
            }
            true
        }
        // Annuler une suppression = réinstancier l'objet à sa place d'origine.
        (EditKind::Deleted, EditTarget::Prop { .. }) => {
            let (Some(asset), Some(before)) = (record.asset.as_ref(), record.before) else {
                return false;
            };
            let transform = before.to_transform();
            let new_id = edits.push_prop(asset, &transform);
            crate::library::spawn_prop_entity(commands, asset_server, new_id, asset, transform);
            true
        }
        // Annuler un masquage = rendre la pièce visible.
        (EditKind::Hidden, EditTarget::Decor { key }) => {
            edits.set_override_hidden(key, false);
            true
        }
        // Toute transformation : on repose l'état d'avant.
        (_, target) => {
            let Some(before) = record.before else {
                return false;
            };
            let transform = before.to_transform();
            match target {
                EditTarget::Prop { id } => {
                    edits.update_prop(*id, &transform);
                    if let Some((entity, _)) = q_props.iter().find(|(_, prop)| prop.id == *id) {
                        if let Ok(mut current) = q_transform.get_mut(entity) {
                            *current = transform;
                        }
                    }
                }
                EditTarget::Decor { key } => {
                    edits.update_override(key, &transform);
                    if let Some((entity, _)) =
                        q_decor.iter().find(|(_, decor)| decor.key.as_str() == key)
                    {
                        if let Ok(mut current) = q_transform.get_mut(entity) {
                            *current = transform;
                        }
                    }
                }
            }
            true
        }
    }
}

/// Décrit la cible d'une modification pour le journal : identifiant stable, nom
/// lisible, et chemin d'asset s'il y en a un (nécessaire pour ressusciter un objet
/// supprimé). Prend des composants et non des `Query` pour rester utilisable
/// depuis n'importe quel système, quelle que soit la forme de ses requêtes.
pub fn describe(
    prop: Option<&EditorProp>,
    decor: Option<&EditorDecor>,
) -> Option<(EditTarget, String, Option<String>)> {
    if let Some(prop) = prop {
        return Some((
            EditTarget::Prop { id: prop.id },
            asset_label(&prop.asset),
            Some(prop.asset.clone()),
        ));
    }
    if let Some(decor) = decor {
        return Some((
            EditTarget::Decor {
                key: decor.key.clone(),
            },
            decor_label(&decor.key),
            None,
        ));
    }
    None
}

/// Nom lisible d'un asset : `models/kaykit/dungeon/barrel.gltf` → `barrel`.
pub fn asset_label(asset: &str) -> String {
    asset
        .rsplit('/')
        .next()
        .and_then(|file| file.rsplit_once('.').map(|(stem, _)| stem))
        .unwrap_or(asset)
        .to_owned()
}

/// Nom lisible d'une clé de décor : `…/castle_terrain_unity.glb#Scene0#0:node`
/// → `castle_terrain_unity`. Sans ça, l'historique afficherait des chemins
/// illisibles et l'annulation à la main serait un jeu de devinettes.
pub fn decor_label(key: &str) -> String {
    let (scene, node) = key.split_once('#').unwrap_or((key, ""));
    let scene = asset_label(scene);
    let node = node.rsplit_once(':').map(|(_, name)| name).unwrap_or("");
    if node.is_empty() || node == crate::select::DEFAULT_NODE_NAME {
        scene
    } else {
        format!("{scene} · {node}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(x: f32) -> TransformData {
        TransformData {
            position: [x, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
        }
    }

    #[test]
    fn no_op_gesture_is_not_recorded() {
        let mut history = EditHistory::default();
        let recorded = history.record(
            EditKind::Moved,
            EditTarget::Prop { id: 1 },
            "barrel",
            None,
            Some(data(0.0)),
            Some(data(0.0)),
        );
        assert!(!recorded);
        assert!(history.records.is_empty());
    }

    #[test]
    fn real_move_is_recorded_with_increasing_seq() {
        let mut history = EditHistory::default();
        assert!(history.record(
            EditKind::Moved,
            EditTarget::Prop { id: 1 },
            "barrel",
            None,
            Some(data(0.0)),
            Some(data(2.0)),
        ));
        assert!(history.record(
            EditKind::Moved,
            EditTarget::Prop { id: 1 },
            "barrel",
            None,
            Some(data(2.0)),
            Some(data(5.0)),
        ));
        assert_eq!(
            history.records.iter().map(|r| r.seq).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(history.pending_count(), 2);
    }

    #[test]
    fn history_is_bounded_and_keeps_the_recent_end() {
        let mut history = EditHistory::default();
        for i in 0..(MAX_RECORDS + 25) {
            history.record(
                EditKind::Moved,
                EditTarget::Prop { id: 1 },
                "barrel",
                None,
                Some(data(i as f32)),
                Some(data(i as f32 + 1.0)),
            );
        }
        assert_eq!(history.records.len(), MAX_RECORDS);
        assert_eq!(
            history.records.last().map(|r| r.seq),
            Some((MAX_RECORDS + 25) as u32)
        );
    }

    #[test]
    fn adopt_resumes_numbering_after_reload() {
        let mut history = EditHistory::default();
        history.adopt(vec![EditRecord {
            seq: 41,
            kind: EditKind::Moved,
            target: EditTarget::Prop { id: 1 },
            label: "barrel".to_owned(),
            asset: None,
            at_epoch_secs: 0,
            before: Some(data(0.0)),
            after: Some(data(1.0)),
            reverted: false,
        }]);
        history.record(
            EditKind::Moved,
            EditTarget::Prop { id: 1 },
            "barrel",
            None,
            Some(data(1.0)),
            Some(data(3.0)),
        );
        assert_eq!(history.records.last().map(|r| r.seq), Some(42));
    }

    #[test]
    fn deleted_prop_needs_asset_and_before_to_be_revertible() {
        let mut record = EditRecord {
            seq: 1,
            kind: EditKind::Deleted,
            target: EditTarget::Prop { id: 7 },
            label: "barrel".to_owned(),
            asset: None,
            at_epoch_secs: 0,
            before: Some(data(0.0)),
            after: None,
            reverted: false,
        };
        assert!(
            !record.can_revert(),
            "sans chemin d'asset, rien à réinstancier"
        );
        record.asset = Some("models/a.glb".to_owned());
        assert!(record.can_revert());
        record.reverted = true;
        assert!(!record.can_revert(), "on n'annule pas deux fois");
    }

    #[test]
    fn labels_stay_readable() {
        assert_eq!(asset_label("models/kaykit/dungeon/barrel.gltf"), "barrel");
        assert_eq!(
            decor_label("models/environment/castle/castle_terrain_unity.glb#Scene0#0:node"),
            "castle_terrain_unity"
        );
        assert_eq!(
            decor_label("models/environment/castle/cell_12.glb#Scene0#3:SM_Barrel"),
            "cell_12 · SM_Barrel"
        );
    }

    #[test]
    fn age_label_scales_with_elapsed_time() {
        let now = now_epoch_secs();
        assert!(age_label(now).ends_with(" s"));
        assert_eq!(age_label(now.saturating_sub(600)), "il y a 10 min");
        assert_eq!(age_label(now.saturating_sub(7200)), "il y a 2 h");
        assert_eq!(age_label(0), "date inconnue");
    }
}
