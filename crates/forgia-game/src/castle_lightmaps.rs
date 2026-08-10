//! # castle_lightmaps — la lumière cuite du créateur, posée sur notre Hall
//!
//! C'est ici que le **rebond** apparaît. Jusqu'à présent le Hall n'avait que de
//! l'éclairage direct : tout ce qui n'était pas frappé par une source tombait au
//! plancher de l'ambiante, et monter les valeurs élargissait les flaques de
//! lumière sans jamais remplir l'entre-deux.
//!
//! Le pack est cuit avec **deux rebonds** et une **occlusion ambiante**, dans 11
//! atlas. Ce module les rebranche.
//!
//! ## La chaîne, et où vit chaque morceau
//!
//! | Étape | Où |
//! |---|---|
//! | quelle pièce lit quelle zone | `castle_hub_lightmaps.json` (`extract_lightmap_table.py`) |
//! | les atlas eux-mêmes | `lightmaps/lightmap_N.ktx2` (`convert_lightmaps.py`) |
//! | la pose sur les entités | ce module |
//!
//! La table associe **un nom de nœud** à un atlas et à un rectangle d'UV. Les
//! 6801 noms sont **globalement uniques** dans nos 45 cellules — vérifié, zéro
//! collision — donc le nom suffit à identifier une pièce, sans avoir à remonter
//! jusqu'à la cellule qui la contient.
//!
//! ## Deux pièges de convention
//!
//! **Le canal d'UV.** Bevy lit les lightmaps sur `ATTRIBUTE_UV_1`, le second jeu.
//! Nos cellules le portent sur 100 % de leurs primitives — sans lui, le composant
//! est silencieusement inopérant.
//!
//! **Le sens vertical.** Unity place l'origine d'une texture en bas à gauche,
//! glTF en haut à gauche. Impossible de trancher sans lancer le jeu : d'où
//! `flip_v` dans le genome, relu à chaud. Si les lightmaps sortent retournées,
//! c'est **une valeur à changer**, pas une régénération de la table.
//!
//! ## Échantillonnage bilinéaire obligatoire
//!
//! `bicubic_sampling` reste **faux**. Il cuit avec 2 texels de marge entre pièces
//! voisines à 4096 ; nos atlas sont réduits à 2048, il n'en reste qu'un. C'est
//! assez pour du bilinéaire, pas pour du bicubique — qui ferait baver une pièce
//! sur sa voisine.
//!
//! Contexte complet : `docs/audits/audit-2026-07-26-etude-eclairage.md`.

use bevy::pbr::Lightmap;
use bevy::prelude::*;
use forgia_core::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

/// Table pièce → zone d'atlas, générée par `tools/unity/extract_lightmap_table.py`.
const TABLE_PATH: &str = "assets/models/environment/castle/castle_hub_lightmaps.json";
/// Les atlas, relatifs à la racine des assets.
const ATLAS_PREFIX: &str = "models/environment/castle/lightmaps/lightmap_";
const SENSOR_PATH: &str = "forgia2_castle_lightmaps.json";
const SENSOR_PERIOD_SECS: f32 = 1.0;
/// Délai laissé au streaming avant de considérer qu'aucune pièce ne viendra.
const GRACE_SECS: f32 = 20.0;
/// Garde-fou de parcours de hiérarchie (un nœud porte une primitive par matériau).
const MAX_SUBTREE_NODES: usize = 64;

// ─── Données ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct TableFile {
    atlas_count: usize,
    cells: HashMap<String, HashMap<String, SlotToml>>,
}

#[derive(Deserialize, Clone, Copy)]
struct SlotToml {
    atlas: usize,
    /// `[min_u, min_v, max_u, max_v]` — le `uv_rect` de Bevy, sans conversion.
    uv: [f32; 4],
}

#[derive(Resource, Default)]
struct LightmapTable {
    /// Nom de nœud → zone. Les noms sont uniques toutes cellules confondues.
    by_node: HashMap<String, SlotToml>,
    atlases: Vec<Handle<Image>>,
}

/// Pièce repérée dont la géométrie n'est pas encore instanciée.
#[derive(Component)]
struct NeedsLightmap(SlotToml);

/// Marque une primitive à laquelle ce module a posé une lightmap.
#[derive(Component)]
struct CastleLightmapped;

#[derive(Resource, Default)]
struct LightmapStats {
    applied: u32,
    elapsed_secs: f32,
    /// Valeur de `flip_v` réellement posée, pour détecter un changement à chaud.
    applied_flip_v: bool,
    /// Gain réellement posé sur les matériaux, même usage.
    applied_exposure: f32,
    /// Matériaux du château auxquels le gain a été appliqué. Les pièces partagent
    /// une poignée de matériaux : on les tient pour ne pas re-parcourir la scène.
    materials: Vec<AssetId<StandardMaterial>>,
    /// Dernier état **dans le Hall**, conservé après en être sorti.
    ///
    /// 🚨 Sans ça, un capteur écrasé chaque seconde ne montre plus rien dès qu'on
    /// quitte le Hall — et « regarde » arrive presque toujours après en être sorti.
    last_active: Option<(f32, u32, usize)>,
}

// ─── Plugin ──────────────────────────────────────────────────────────────────

pub struct CastleLightmapsPlugin;

impl Plugin for CastleLightmapsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightmapTable>()
            .init_resource::<LightmapStats>()
            .add_systems(OnEnter(GameMode::CastleHub), sys_load_table)
            .add_systems(OnExit(GameMode::CastleHub), sys_cleanup)
            .add_systems(
                Update,
                (
                    sys_mark_pieces,
                    sys_apply_lightmaps,
                    sys_follow_flip,
                    sys_follow_exposure,
                )
                    .chain()
                    .run_if(in_state(GameMode::CastleHub)),
            )
            .add_systems(Update, sys_write_sensor.in_set(GameSet::Sensors));
    }
}

fn sys_load_table(
    mut table: ResMut<LightmapTable>,
    mut stats: ResMut<LightmapStats>,
    assets: Res<AssetServer>,
    tuning: Res<crate::castle_flames::CastleLighting>,
) {
    *stats = LightmapStats::default();
    stats.applied_flip_v = tuning.lightmaps_flip_v;
    *table = LightmapTable::default();
    if !tuning.lightmaps_enabled {
        return;
    }
    let Ok(content) = fs::read_to_string(TABLE_PATH) else {
        warn!("[castle-lightmaps] {TABLE_PATH} illisible — aucune lumière cuite");
        return;
    };
    let parsed: TableFile = match serde_json::from_str(&content) {
        Ok(parsed) => parsed,
        Err(error) => {
            warn!("[castle-lightmaps] {TABLE_PATH} illisible ({error})");
            return;
        }
    };
    for nodes in parsed.cells.values() {
        for (node, slot) in nodes {
            table.by_node.insert(node.clone(), *slot);
        }
    }
    table.atlases = (0..parsed.atlas_count)
        .map(|index| assets.load(format!("{ATLAS_PREFIX}{index}.ktx2")))
        .collect();
}

/// Repère les pièces dès que leur nœud apparaît (le streaming instancie les
/// cellules en continu, il n'y a pas de moment « tout est chargé »).
fn sys_mark_pieces(
    mut commands: Commands,
    table: Res<LightmapTable>,
    q_new: Query<(Entity, &Name), Added<Name>>,
) {
    if table.by_node.is_empty() {
        return;
    }
    for (entity, name) in &q_new {
        if let Some(slot) = table.by_node.get(name.as_str()) {
            commands.entity(entity).insert(NeedsLightmap(*slot));
        }
    }
}

/// Pose la lightmap sur chaque primitive de la pièce.
fn sys_apply_lightmaps(
    mut commands: Commands,
    table: Res<LightmapTable>,
    tuning: Res<crate::castle_flames::CastleLighting>,
    mut stats: ResMut<LightmapStats>,
    time: Res<Time<Real>>,
    q_pending: Query<(Entity, &NeedsLightmap)>,
    q_children: Query<&Children>,
    q_mesh: Query<&MeshMaterial3d<StandardMaterial>, With<Mesh3d>>,
    mut scratch: Local<Vec<Entity>>,
) {
    stats.elapsed_secs += time.delta_secs();
    for (entity, needs) in &q_pending {
        let Some(image) = table.atlases.get(needs.0.atlas) else {
            commands.entity(entity).remove::<NeedsLightmap>();
            continue;
        };
        let mut applied = false;
        scratch.clear();
        scratch.push(entity);
        let mut cursor = 0;
        while cursor < scratch.len() && scratch.len() < MAX_SUBTREE_NODES {
            let current = scratch[cursor];
            cursor += 1;
            if let Ok(material) = q_mesh.get(current) {
                commands.entity(current).insert((
                    lightmap_for(image.clone(), needs.0.uv, tuning.lightmaps_flip_v),
                    CastleLightmapped,
                ));
                // Les pièces partagent une poignée de matériaux : on note l'id une
                // fois, le gain leur sera posé par `sys_follow_exposure`.
                let id = material.id();
                if !stats.materials.contains(&id) {
                    stats.materials.push(id);
                }
                applied = true;
            }
            if let Ok(children) = q_children.get(current) {
                scratch.extend(children.iter());
            }
        }
        if applied {
            commands.entity(entity).remove::<NeedsLightmap>();
            stats.applied += 1;
        }
        // Sinon la géométrie n'est pas encore instanciée : on retentera.
    }
}

/// Rejoue la pose quand `flip_v` change à chaud, sans quitter le Hall.
fn sys_follow_flip(
    mut commands: Commands,
    tuning: Res<crate::castle_flames::CastleLighting>,
    mut stats: ResMut<LightmapStats>,
    q_lit: Query<(Entity, &Lightmap), With<CastleLightmapped>>,
) {
    if stats.applied_flip_v == tuning.lightmaps_flip_v {
        return;
    }
    stats.applied_flip_v = tuning.lightmaps_flip_v;
    for (entity, lightmap) in &q_lit {
        // Le rectangle courant est déjà retourné : le retourner à nouveau
        // ramène à l'orientation d'origine, sans relire la table.
        let flipped = Rect::new(
            lightmap.uv_rect.min.x,
            1.0 - lightmap.uv_rect.max.y,
            lightmap.uv_rect.max.x,
            1.0 - lightmap.uv_rect.min.y,
        );
        commands.entity(entity).insert(Lightmap {
            image: lightmap.image.clone(),
            uv_rect: flipped,
            bicubic_sampling: false,
        });
    }
}

/// Pose le gain sur les matériaux du château, et le suit à chaud.
///
/// 🚨 Sans lui, la lumière cuite est invisible. Ses valeurs ont une médiane de
/// 0,05 quand l'ambiante Bevy qu'elles remplacent valait 400 : au gain 1 par
/// défaut de Bevy, poser les lightmaps **assombrit** le Hall au lieu de l'éclairer,
/// puisqu'on a coupé l'ambiante, le diffus de l'éclairage par image et le soleil
/// sur la géométrie cuite.
fn sys_follow_exposure(
    tuning: Res<crate::castle_flames::CastleLighting>,
    mut stats: ResMut<LightmapStats>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut known: Local<usize>,
) {
    let target = tuning.lightmaps_exposure.max(0.0);
    // Rien à faire tant que ni le réglage ni la liste de matériaux n'ont bougé —
    // toucher un matériau le marque modifié et relance sa préparation GPU.
    if stats.applied_exposure == target && *known == stats.materials.len() {
        return;
    }
    stats.applied_exposure = target;
    *known = stats.materials.len();
    for id in &stats.materials {
        if let Some(material) = materials.get_mut(*id) {
            material.lightmap_exposure = target;
        }
    }
}

/// Construit le composant, en appliquant au besoin le retournement vertical.
fn lightmap_for(image: Handle<Image>, uv: [f32; 4], flip_v: bool) -> Lightmap {
    let uv_rect = if flip_v {
        Rect::new(uv[0], 1.0 - uv[3], uv[2], 1.0 - uv[1])
    } else {
        Rect::new(uv[0], uv[1], uv[2], uv[3])
    };
    Lightmap {
        image,
        uv_rect,
        // Voir la note du module : 1 texel de marge à 2048, le bicubique baverait.
        bicubic_sampling: false,
    }
}

fn sys_cleanup(
    mut commands: Commands,
    mut table: ResMut<LightmapTable>,
    mut stats: ResMut<LightmapStats>,
    q_lit: Query<Entity, With<CastleLightmapped>>,
    q_pending: Query<Entity, With<NeedsLightmap>>,
) {
    for entity in &q_lit {
        commands
            .entity(entity)
            .remove::<(Lightmap, CastleLightmapped)>();
    }
    for entity in &q_pending {
        commands.entity(entity).remove::<NeedsLightmap>();
    }
    *table = LightmapTable::default();
    *stats = LightmapStats::default();
}

// ─── Observabilité ───────────────────────────────────────────────────────────

fn sys_write_sensor(
    time: Res<Time<Real>>,
    mut next_write: Local<f32>,
    game_mode: Res<State<GameMode>>,
    tuning: Res<crate::castle_flames::CastleLighting>,
    table: Res<LightmapTable>,
    mut stats: ResMut<LightmapStats>,
    q_pending: Query<(), With<NeedsLightmap>>,
) {
    let now = time.elapsed_secs();
    if now < *next_write {
        return;
    }
    *next_write = now + SENSOR_PERIOD_SECS;

    let in_hub = matches!(game_mode.get(), GameMode::CastleHub);
    // 🚨 Retenir le dernier état DANS le Hall. Un capteur écrasé chaque seconde
    // ne montre plus rien dès qu'on en sort — or on regarde presque toujours
    // après coup, une fois la capture prise et le Hall quitté.
    if in_hub && stats.applied > 0 {
        stats.last_active = Some((now, stats.applied, table.atlases.len()));
    }
    let last_active = match stats.last_active {
        Some((at, applied, atlases)) => {
            format!(r#"{{"timestamp_secs":{at:.1},"applied":{applied},"atlases":{atlases}}}"#)
        }
        None => "null".to_string(),
    };
    let pending = q_pending.iter().count() as u32;
    let (severity, next_step) = severity_for_lightmaps(
        in_hub,
        tuning.lightmaps_enabled,
        table.by_node.len(),
        stats.applied,
        stats.elapsed_secs,
    );
    let json = format!(
        r#"{{"id":"castle_lightmaps","severity":"{severity}","next_step":"{next_step}","timestamp_secs":{now:.1},"in_hub":{in_hub},"enabled":{},"known":{},"applied":{},"pending":{pending},"atlases":{},"flip_v":{},"exposure":{:.0},"materials":{},"last_active":{last_active}}}"#,
        tuning.lightmaps_enabled,
        table.by_node.len(),
        stats.applied,
        table.atlases.len(),
        tuning.lightmaps_flip_v,
        stats.applied_exposure,
        stats.materials.len(),
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

/// Sévérité + action de remédiation. Extraite pour être testable sans app Bevy.
fn severity_for_lightmaps(
    in_hub: bool,
    enabled: bool,
    known: usize,
    applied: u32,
    elapsed_secs: f32,
) -> (&'static str, &'static str) {
    if !in_hub {
        return ("info", "hors du Hall — pas de lumière cuite");
    }
    if !enabled {
        return ("info", "désactivé dans [lightmaps] du genome");
    }
    if known == 0 {
        return (
            "critical",
            "table vide : régénérer castle_hub_lightmaps.json via \
             tools/unity/extract_lightmap_table.py",
        );
    }
    if elapsed_secs < GRACE_SECS {
        return ("ok", "streaming en cours");
    }
    if applied == 0 {
        return (
            "critical",
            "aucune pièce éclairée : vérifier que les noms de la table \
             correspondent aux nœuds des cellules glTF",
        );
    }
    ("ok", "aucune action")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn table() -> TableFile {
        let path = Path::new("../..").join(TABLE_PATH);
        let content = fs::read_to_string(path).expect("la table doit être versionnée");
        serde_json::from_str(&content).expect("la table doit être du JSON valide")
    }

    #[test]
    fn the_table_covers_most_of_the_hall() {
        let parsed = table();
        let pieces: usize = parsed.cells.values().map(HashMap::len).sum();
        // 6801 pièces sur les 7895 de nos cellules. Le seuil protège contre une
        // régénération qui perdrait silencieusement une famille entière — comme
        // les 818 dalles de sol, écartées par une première version de l'outil.
        assert!(pieces > 6500, "seulement {pieces} pièces raccordées");
        assert_eq!(parsed.atlas_count, 11);
    }

    /// Le nom de nœud est notre seule clé : une collision poserait la mauvaise
    /// zone d'atlas sur une pièce, sans rien signaler.
    #[test]
    fn node_names_are_unique_across_cells() {
        let parsed = table();
        let mut seen = std::collections::HashSet::new();
        for nodes in parsed.cells.values() {
            for node in nodes.keys() {
                assert!(seen.insert(node.clone()), "nom en double : {node}");
            }
        }
    }

    #[test]
    fn every_uv_rect_is_ordered_and_within_the_atlas() {
        for nodes in table().cells.values() {
            for (node, slot) in nodes {
                let [min_u, min_v, max_u, max_v] = slot.uv;
                assert!(
                    max_u > min_u && max_v > min_v,
                    "rectangle dégénéré : {node}"
                );
                // Le débord d'atlas du bake vaut quelques texels sur 4096 ; au-delà
                // c'est une erreur de lecture du binaire Unity.
                assert!(min_u > -0.01 && min_v > -0.01, "hors atlas : {node}");
                assert!(max_u < 1.06 && max_v < 1.06, "hors atlas : {node}");
                assert!(slot.atlas < 11, "atlas inconnu : {node}");
            }
        }
    }

    /// Retourner deux fois doit ramener au rectangle d'origine — c'est ce sur quoi
    /// repose le suivi à chaud de `flip_v`.
    #[test]
    fn flipping_twice_restores_the_rectangle() {
        let uv = [0.25, 0.10, 0.75, 0.40];
        let once = lightmap_for(Handle::default(), uv, true).uv_rect;
        let twice = Rect::new(once.min.x, 1.0 - once.max.y, once.max.x, 1.0 - once.min.y);
        assert!((twice.min.y - uv[1]).abs() < 1e-6);
        assert!((twice.max.y - uv[3]).abs() < 1e-6);
    }

    #[test]
    fn a_missing_table_is_critical_but_loading_is_not() {
        assert_eq!(severity_for_lightmaps(true, true, 0, 0, 30.0).0, "critical");
        assert_eq!(severity_for_lightmaps(true, true, 6801, 0, 1.0).0, "ok");
        assert_eq!(
            severity_for_lightmaps(true, true, 6801, 0, 30.0).0,
            "critical"
        );
        assert_eq!(severity_for_lightmaps(true, true, 6801, 4000, 30.0).0, "ok");
    }
}
