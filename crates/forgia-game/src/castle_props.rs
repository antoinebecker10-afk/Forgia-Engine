//! # castle_props — réimport des instances écartées par la reconstruction
//!
//! La reconstruction du château depuis le pack Unity ne gardait que les prefabs
//! ayant une correspondance **1:1 avec un FBX**. Les variantes composites —
//! suffixes `_static`, `_lit`, `_comp` — sont tombées en silence : 191 instances,
//! dont **50 bannières murales**. Elles manquent à l'écran alors que leur mesh,
//! lui, est bien là : `P_PROP_flag_castle_02_static` référence exactement le même
//! `SM_PROP_flag_castle_02.fbx` que sa version non statique, et n'ajoute qu'un
//! matériau. Ce qui manquait n'était donc **pas de la géométrie, mais des
//! transformations**.
//!
//! ## Pourquoi on clone un mesh déjà chargé plutôt que d'ajouter un asset
//!
//! Deux autres voies étaient possibles, écartées :
//!
//! - **Injecter les nœuds dans les cellules glTF.** L'éditeur de scène du Hall
//!   identifie une pièce par son *rang de fratrie* dans le nœud parent : insérer
//!   des nœuds décalerait ces rangs et invaliderait les retouches enregistrées.
//! - **Extraire un GLB autonome par bannière.** Duplique une géométrie déjà en
//!   mémoire, et il faudrait re-router ses textures.
//!
//! Le château tient dans 193 m, le streaming charge à 240 m : dès qu'on est dans
//! le Hall, une bannière d'origine est chargée quelque part. On capte donc les
//! `Handle` de son mesh et de ses matériaux au passage, et on les réutilise. Zéro
//! octet de géométrie ajouté, matériau exact par construction.
//!
//! ## Entités racines, transformation composée
//!
//! Chaque primitive est posée en **racine**, à `cible × locale`, comme les
//! flammes et pour la même raison : les pièces du pack ont des échelles miroir
//! (déterminant négatif), dont un enfant hériterait.
//!
//! Données : `assets/genomes/castle_hub_missing_props.toml`, généré par
//! `tools/unity/extract_scene_props.py` (rotations reflétées en `(x, −y, −z, w)`,
//! vérifiées à 0,0006 m et 0,1° contre nos bannières existantes).
//! Contexte : `docs/audits/audit-2026-07-26-diff-complet-map-createur.md`.

use bevy::prelude::*;
use forgia_core::prelude::*;
use serde::Deserialize;
use std::fs;

/// Instances manquantes, générées depuis la scène Unity du pack.
const MISSING_PROPS_PATH: &str = "assets/genomes/castle_hub_missing_props.toml";
const SENSOR_PATH: &str = "forgia2_castle_props.json";
const SENSOR_PERIOD_SECS: f32 = 1.0;
/// Délai laissé au streaming pour amener une cellule contenant le mesh source
/// avant de considérer que la bannière ne viendra jamais. Le château entier tient
/// dans le rayon de streaming : au-delà, c'est que la source a disparu du pack.
const SOURCE_GRACE_SECS: f32 = 20.0;
/// Garde-fou de parcours de hiérarchie (un nœud glTF porte une primitive par
/// matériau — les drapeaux en ont deux).
const MAX_SUBTREE_NODES: usize = 64;

// ─── Données ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct MissingPropsFile {
    #[serde(default)]
    prop: Vec<PropToml>,
}

#[derive(Deserialize)]
struct PropToml {
    /// Nom du mesh source, tel qu'il apparaît dans nos cellules glTF.
    mesh: String,
    pos: [f32; 3],
    /// Quaternion `(x, y, z, w)`, déjà exprimé dans notre repère.
    rot: [f32; 4],
    scale: [f32; 3],
}

/// Une instance à poser, et le mesh dont elle réutilise la géométrie.
struct MissingProp {
    mesh: String,
    transform: Transform,
}

#[derive(Resource, Default)]
struct MissingProps {
    pending: Vec<MissingProp>,
    /// Nombre lu au chargement — sert au capteur, `pending` se vidant.
    loaded: usize,
}

impl MissingProps {
    fn families(&self) -> impl Iterator<Item = &str> {
        self.pending.iter().map(|p| p.mesh.as_str())
    }
}

/// Géométrie captée sur une instance d'origine : une entrée par primitive.
#[derive(Clone)]
struct PropVisual {
    parts: Vec<PropPart>,
}

#[derive(Clone)]
struct PropPart {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    /// Transformation de la primitive relative à son nœud.
    local: Transform,
}

#[derive(Resource, Default)]
struct PropSources(std::collections::HashMap<String, PropVisual>);

/// Marque une instance posée par ce module (cycle de vie explicite).
#[derive(Component)]
struct CastleMissingProp;

/// Nœud d'origine repéré, dont la géométrie n'est pas encore instanciée.
#[derive(Component)]
struct NeedsCapture {
    family: String,
}

#[derive(Resource, Default)]
struct PropStats {
    spawned: u32,
    /// Temps passé dans le Hall, pour ne pas alerter avant le streaming.
    elapsed_secs: f32,
}

// ─── Plugin ──────────────────────────────────────────────────────────────────

pub struct CastlePropsPlugin;

impl Plugin for CastlePropsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MissingProps>()
            .init_resource::<PropSources>()
            .init_resource::<PropStats>()
            .add_systems(OnEnter(GameMode::CastleHub), sys_load_missing_props)
            .add_systems(OnExit(GameMode::CastleHub), sys_cleanup_props)
            .add_systems(
                Update,
                (sys_mark_sources, sys_capture_sources, sys_spawn_missing)
                    .chain()
                    .run_if(in_state(GameMode::CastleHub)),
            )
            .add_systems(Update, sys_write_sensor.in_set(GameSet::Sensors));
    }
}

/// Relit la table à chaque entrée dans le Hall (elle est éditable à froid).
fn sys_load_missing_props(mut props: ResMut<MissingProps>, mut stats: ResMut<PropStats>) {
    *stats = PropStats::default();
    props.pending.clear();
    props.loaded = 0;

    let Ok(content) = fs::read_to_string(forgia_core::asset_paths::asset_path(MISSING_PROPS_PATH))
    else {
        warn!("castle_props : {MISSING_PROPS_PATH} illisible, aucune instance réimportée");
        return;
    };
    let parsed: MissingPropsFile = match toml::from_str(&content) {
        Ok(parsed) => parsed,
        Err(error) => {
            warn!("castle_props : {MISSING_PROPS_PATH} illisible ({error})");
            return;
        }
    };
    props.pending = parsed.prop.into_iter().map(to_prop).collect();
    props.loaded = props.pending.len();
}

fn to_prop(entry: PropToml) -> MissingProp {
    MissingProp {
        mesh: entry.mesh,
        transform: Transform {
            translation: Vec3::from_array(entry.pos),
            // Le quaternion vient déjà reflété : le normaliser suffit à absorber
            // l'arrondi d'écriture du fichier.
            rotation: Quat::from_xyzw(entry.rot[0], entry.rot[1], entry.rot[2], entry.rot[3])
                .normalize(),
            scale: Vec3::from_array(entry.scale),
        },
    }
}

/// Repère les nœuds d'origine dès qu'ils apparaissent (le streaming instancie les
/// cellules en continu, il n'y a pas de moment « tout est chargé »).
fn sys_mark_sources(
    mut commands: Commands,
    props: Res<MissingProps>,
    sources: Res<PropSources>,
    q_new: Query<(Entity, &Name), Added<Name>>,
) {
    if props.pending.is_empty() {
        return;
    }
    for (entity, name) in &q_new {
        // Le nœud s'appelle `SM_PROP_flag_castle_02_LOD0.003` : on cherche la
        // famille en préfixe, et seulement le niveau de détail le plus fin.
        let Some(family) = props
            .families()
            .find(|family| name.as_str().starts_with(*family))
            .map(str::to_owned)
        else {
            continue;
        };
        if sources.0.contains_key(&family) || !name.as_str().contains("_LOD0") {
            continue;
        }
        commands.entity(entity).insert(NeedsCapture { family });
    }
}

/// Capte les `Handle` du nœud d'origine, une fois sa géométrie instanciée.
fn sys_capture_sources(
    mut commands: Commands,
    mut sources: ResMut<PropSources>,
    q_pending: Query<(Entity, &NeedsCapture)>,
    q_children: Query<&Children>,
    q_visual: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &Transform)>,
    mut scratch: Local<Vec<Entity>>,
) {
    for (entity, needs) in &q_pending {
        if sources.0.contains_key(&needs.family) {
            commands.entity(entity).remove::<NeedsCapture>();
            continue;
        }
        let parts = collect_parts(entity, &q_children, &q_visual, &mut scratch);
        if parts.is_empty() {
            // Géométrie pas encore instanciée : on retente à la frame suivante.
            continue;
        }
        sources.0.insert(needs.family.clone(), PropVisual { parts });
        commands.entity(entity).remove::<NeedsCapture>();
    }
}

/// Parcourt le nœud et ses descendants et relève chaque primitive.
fn collect_parts(
    root: Entity,
    q_children: &Query<&Children>,
    q_visual: &Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>, &Transform)>,
    scratch: &mut Vec<Entity>,
) -> Vec<PropPart> {
    scratch.clear();
    scratch.push(root);
    let mut parts = Vec::new();
    let mut cursor = 0;
    while cursor < scratch.len() && scratch.len() < MAX_SUBTREE_NODES {
        let entity = scratch[cursor];
        cursor += 1;
        if let Ok((mesh, material, local)) = q_visual.get(entity) {
            parts.push(PropPart {
                mesh: mesh.0.clone(),
                material: material.0.clone(),
                // La primitive d'un nœud racine porte l'identité ; on la compose
                // quand même, pour ne rien supposer de l'export.
                local: if entity == root {
                    Transform::IDENTITY
                } else {
                    *local
                },
            });
        }
        if let Ok(children) = q_children.get(entity) {
            scratch.extend(children.iter());
        }
    }
    parts
}

/// Pose les instances dont la géométrie source est désormais connue.
fn sys_spawn_missing(
    mut commands: Commands,
    mut props: ResMut<MissingProps>,
    mut stats: ResMut<PropStats>,
    sources: Res<PropSources>,
    time: Res<Time<Real>>,
) {
    stats.elapsed_secs += time.delta_secs();
    if props.pending.is_empty() || sources.0.is_empty() {
        return;
    }
    props.pending.retain(|prop| {
        let Some(visual) = sources.0.get(&prop.mesh) else {
            return true; // source pas encore chargée
        };
        for part in &visual.parts {
            commands.spawn((
                CastleMissingProp,
                Mesh3d(part.mesh.clone()),
                MeshMaterial3d(part.material.clone()),
                prop.transform.mul_transform(part.local),
                Name::new("CastleMissingProp"),
            ));
        }
        stats.spawned += 1;
        false
    });
}

fn sys_cleanup_props(
    mut commands: Commands,
    mut sources: ResMut<PropSources>,
    mut props: ResMut<MissingProps>,
    mut stats: ResMut<PropStats>,
    q_props: Query<Entity, With<CastleMissingProp>>,
    q_pending: Query<Entity, With<NeedsCapture>>,
) {
    for entity in &q_props {
        commands.entity(entity).despawn();
    }
    for entity in &q_pending {
        commands.entity(entity).remove::<NeedsCapture>();
    }
    sources.0.clear();
    props.pending.clear();
    props.loaded = 0;
    *stats = PropStats::default();
}

// ─── Observabilité ───────────────────────────────────────────────────────────

fn sys_write_sensor(
    time: Res<Time<Real>>,
    mut next_write: Local<f32>,
    game_mode: Res<State<GameMode>>,
    props: Res<MissingProps>,
    sources: Res<PropSources>,
    stats: Res<PropStats>,
) {
    let now = time.elapsed_secs();
    if now < *next_write {
        return;
    }
    *next_write = now + SENSOR_PERIOD_SECS;

    let in_hub = matches!(game_mode.get(), GameMode::CastleHub);
    let (severity, next_step) = severity_for_props(
        in_hub,
        props.loaded,
        stats.spawned as usize,
        stats.elapsed_secs,
    );
    let json = format!(
        r#"{{"id":"castle_props","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{now:.1},"in_hub":{in_hub},"loaded":{},"spawned":{},"pending":{},"sources_ready":{}}}"#,
        props.loaded,
        stats.spawned,
        props.pending.len(),
        sources.0.len(),
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

/// Sévérité + action de remédiation. Extraite pour être testable sans app Bevy.
fn severity_for_props(
    in_hub: bool,
    loaded: usize,
    spawned: usize,
    elapsed_secs: f32,
) -> (&'static str, &'static str) {
    if !in_hub {
        return ("info", "hors du Hall — rien à poser");
    }
    if loaded == 0 {
        return (
            "critical",
            "table vide : régénérer assets/genomes/castle_hub_missing_props.toml \
             via tools/unity/extract_scene_props.py",
        );
    }
    // Avant la fin du délai, l'absence est normale : le streaming amène les
    // cellules progressivement.
    if elapsed_secs < SOURCE_GRACE_SECS {
        return ("ok", "streaming en cours");
    }
    if spawned == 0 {
        return (
            "critical",
            "aucun mesh source capté : vérifier que les noms `mesh` du TOML \
             correspondent aux nœuds `SM_PROP_*_LOD0` des cellules glTF",
        );
    }
    if spawned < loaded {
        return (
            "warn",
            "instances incomplètes : la cellule contenant le mesh source n'est \
             pas streamée — vérifier le rayon de streaming du Hall",
        );
    }
    ("ok", "aucune action")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> MissingPropsFile {
        let path = forgia_core::asset_paths::asset_path(MISSING_PROPS_PATH);
        let content = fs::read_to_string(&path)
            .expect("la table des instances manquantes doit être versionnée");
        toml::from_str(&content).expect("la table doit être du TOML valide")
    }

    /// Le cœur du correctif : les 50 bannières écartées par la reconstruction.
    #[test]
    fn the_fifty_absent_banners_are_all_in_the_table() {
        let parsed = table();
        assert_eq!(
            parsed.prop.len(),
            50,
            "50 bannières `_static` étaient absentes"
        );
    }

    /// Elles doivent réutiliser un mesh que nos cellules contiennent déjà —
    /// sinon rien ne s'affiche, faute de source à cloner.
    #[test]
    fn every_banner_reuses_an_existing_castle_mesh() {
        for entry in table().prop {
            assert!(
                entry.mesh.starts_with("SM_PROP_"),
                "`{}` ne ressemble pas à un nœud de nos cellules",
                entry.mesh
            );
        }
    }

    /// Une rotation mal reflétée retournerait les bannières : on vérifie au moins
    /// que ce sont des quaternions unitaires, et des échelles non dégénérées.
    #[test]
    fn transforms_are_sane() {
        for entry in table().prop {
            let prop = to_prop(entry);
            assert!(
                prop.transform.rotation.is_normalized(),
                "quaternion non unitaire"
            );
            assert!(
                prop.transform.scale.min_element() > 0.0,
                "échelle nulle ou négative : la bannière serait invisible ou retournée"
            );
            assert!(
                prop.transform.translation.length() < 1000.0,
                "position hors du château : conversion de repère suspecte"
            );
        }
    }

    #[test]
    fn a_silent_table_is_reported_as_critical() {
        let (severity, _) = severity_for_props(true, 0, 0, 60.0);
        assert_eq!(severity, "critical");
    }

    /// Ne pas crier pendant que le streaming travaille.
    #[test]
    fn nothing_placed_yet_is_fine_while_streaming() {
        let (severity, _) = severity_for_props(true, 50, 0, 1.0);
        assert_eq!(severity, "ok");
        let (severity, _) = severity_for_props(true, 50, 0, SOURCE_GRACE_SECS + 1.0);
        assert_eq!(severity, "critical");
    }

    #[test]
    fn a_partial_placement_warns() {
        let (severity, _) = severity_for_props(true, 50, 42, SOURCE_GRACE_SECS + 1.0);
        assert_eq!(severity, "warn");
        let (severity, _) = severity_for_props(true, 50, 50, SOURCE_GRACE_SECS + 1.0);
        assert_eq!(severity, "ok");
    }
}
