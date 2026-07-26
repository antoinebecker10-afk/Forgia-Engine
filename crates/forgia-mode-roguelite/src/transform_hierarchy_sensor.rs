//! Diagnostic des hiérarchies GLTF qui alimentent la propagation des transforms.
//!
//! Échantillonné à 1 Hz : jamais dans le hot path de frame. Il expose les plus
//! grandes racines de hiérarchie, y compris les squelettes et VFX qui ne portent
//! pas `SceneRoot`, afin de localiser le sous-arbre réellement responsable d'un
//! pic de propagation.

use bevy::prelude::*;
use bevy::scene::SceneRoot;
use std::collections::{HashMap, VecDeque};

const SENSOR_PATH: &str = "forgia2_transform_hierarchy.json";
const LAG_SENSOR_PATH: &str = "forgia2_transform_lag.json";
const SAMPLE_PERIOD_SECS: f32 = 1.0;
const TOP_ROOTS: usize = 8;
const TOP_LAG_ROOTS: usize = 8;
const LAG_THRESHOLD_SECS: f32 = 0.030;
const LAG_HISTORY_CAPACITY: usize = 8;

#[derive(Clone)]
struct TransformLagSnapshot {
    timestamp_secs: f32,
    dt_ms: f32,
    changed_transforms: u32,
    roots_json: String,
}

/// Historique borné des hitches de propagation.
///
/// Le fichier était auparavant écrasé à chaque hitch : le pic diagnostiqué par
/// `forgia2_perf_diag.json` pouvait donc être perdu avant lecture. Huit entrées
/// suffisent pour le post-mortem d'une run sans coût sur les frames normales.
#[derive(Resource, Default)]
pub struct TransformLagHistory {
    recent: VecDeque<TransformLagSnapshot>,
}

/// Écrit un instantané des grandes hiérarchies et du volume de
/// transforms modifiés durant la frame d'échantillonnage.
pub fn sys_write_transform_hierarchy_sensor(
    time: Res<Time>,
    mut accum: Local<f32>,
    roots: Query<(Entity, Option<&Name>), (With<Children>, Without<ChildOf>)>,
    scene_roots: Query<(), With<SceneRoot>>,
    children: Query<&Children>,
    changed_transforms: Query<Entity, Changed<Transform>>,
) {
    *accum += time.delta_secs();
    if *accum < SAMPLE_PERIOD_SECS {
        return;
    }
    *accum = 0.0;

    let mut largest = Vec::new();
    for (root, name) in &roots {
        let mut descendants = 0usize;
        let mut stack = children
            .get(root)
            .map(|list| list.iter().collect::<Vec<_>>())
            .unwrap_or_default();
        while let Some(entity) = stack.pop() {
            descendants += 1;
            if let Ok(list) = children.get(entity) {
                stack.extend(list.iter());
            }
        }
        largest.push((
            descendants,
            name.map(|name| name.as_str().to_string())
                .unwrap_or_else(|| format!("unnamed:{root}")),
        ));
    }
    largest.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.0));
    largest.truncate(TOP_ROOTS);

    let roots_json = largest
        .iter()
        .map(|(count, name)| {
            format!(
                r#"{{"name":{},"descendants":{count}}}"#,
                serde_json::to_string(name).unwrap_or_else(|_| "\"invalid\"".to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        r#"{{"id":"transform_hierarchy","timestamp_secs":{:.1},"scene_roots":{},"hierarchy_roots":{},"changed_transforms_this_frame":{},"largest_roots":[{roots_json}]}}"#,
        time.elapsed_secs(),
        scene_roots.iter().count(),
        roots.iter().count(),
        changed_transforms.iter().count(),
    );
    let _ = forgia_core::sensor_io::enqueue(SENSOR_PATH, json);
}

/// Capture l'origine des transforms modifiés uniquement sur une frame lente.
///
/// La sonde ne fait aucun parcours en frame normale. Sur un hitch, elle remonte
/// chaque transform modifié jusqu'à sa racine et expose les sous-arbres qui ont
/// effectivement été touchés, plutôt qu'un instantané pris une seconde plus tard.
pub fn sys_capture_transform_lag_roots(
    time: Res<Time>,
    changed_transforms: Query<Entity, Changed<Transform>>,
    child_of: Query<&ChildOf>,
    names: Query<&Name>,
    mut history: ResMut<TransformLagHistory>,
) {
    let dt_secs = time.delta_secs();
    if dt_secs < LAG_THRESHOLD_SECS {
        return;
    }

    let mut roots: HashMap<Entity, u32> = HashMap::new();
    let mut changed_count = 0u32;
    for entity in &changed_transforms {
        changed_count = changed_count.saturating_add(1);
        let mut root = entity;
        // Borne anti-hiérarchie corrompue : une racine Forgia normale est très
        // loin de 128 parents, mais la sonde ne doit jamais contribuer au hitch.
        for _ in 0..128 {
            let Ok(parent) = child_of.get(root) else {
                break;
            };
            root = parent.parent();
        }
        *roots.entry(root).or_default() += 1;
    }

    let mut roots: Vec<(Entity, u32)> = roots.into_iter().collect();
    roots.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.1));
    roots.truncate(TOP_LAG_ROOTS);
    let roots_json = roots
        .iter()
        .map(|(entity, changed)| {
            let name = names
                .get(*entity)
                .map(|name| name.as_str().to_string())
                .unwrap_or_else(|_| format!("unnamed:{entity}"));
            format!(
                r#"{{"name":{},"changed_transforms":{changed}}}"#,
                serde_json::to_string(&name).unwrap_or_else(|_| "\"invalid\"".to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let snapshot = TransformLagSnapshot {
        timestamp_secs: time.elapsed_secs(),
        dt_ms: dt_secs * 1000.0,
        changed_transforms: changed_count,
        roots_json,
    };
    if history.recent.len() >= LAG_HISTORY_CAPACITY {
        history.recent.pop_front();
    }
    history.recent.push_back(snapshot);

    let latest = history.recent.back().expect("just pushed transform lag snapshot");
    let recent_json = history
        .recent
        .iter()
        .map(|event| {
            format!(
                r#"{{"timestamp_secs":{:.2},"dt_ms":{:.2},"changed_transforms":{},"roots":[{}]}}"#,
                event.timestamp_secs, event.dt_ms, event.changed_transforms, event.roots_json
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    // Champs de tête conservés pour les lecteurs existants ; `recent` permet le
    // post-mortem de plusieurs pics au lieu du seul dernier.
    let json = format!(
        r#"{{"id":"transform_lag","timestamp_secs":{:.2},"dt_ms":{:.2},"changed_transforms":{},"roots":[{}],"recent":[{recent_json}]}}"#,
        latest.timestamp_secs,
        latest.dt_ms,
        latest.changed_transforms,
        latest.roots_json,
    );
    let _ = forgia_core::sensor_io::enqueue(LAG_SENSOR_PATH, json);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lag_history_is_bounded_and_keeps_newest_snapshots() {
        let mut history = TransformLagHistory::default();
        for index in 0..(LAG_HISTORY_CAPACITY + 2) {
            if history.recent.len() >= LAG_HISTORY_CAPACITY {
                history.recent.pop_front();
            }
            history.recent.push_back(TransformLagSnapshot {
                timestamp_secs: index as f32,
                dt_ms: 30.0,
                changed_transforms: index as u32,
                roots_json: "".to_owned(),
            });
        }
        assert_eq!(history.recent.len(), LAG_HISTORY_CAPACITY);
        assert_eq!(history.recent.front().unwrap().timestamp_secs, 2.0);
        assert_eq!(history.recent.back().unwrap().timestamp_secs, 9.0);
    }
}
