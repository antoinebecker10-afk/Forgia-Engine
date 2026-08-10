//! Offline→runtime streaming contract for an assembled `SpatialPlan`.
//!
//! Pure and Bevy-free so the cell partition and the activation *ordering* are
//! unit-testable without an ECS. The Bevy adapter (`forgia-pcg-runtime`) drives
//! this state machine; it never re-derives the order.
//!
//! Invariant (pcg-methodology §6F): on load a cell activates collision/navmesh
//! **before** render; on unload it tears down in reverse (render off first). The
//! [`CellPhase`] ladder encodes exactly that, and [`activate_order_sound`] proves
//! the order declared in the `content-spec` agrees with it.

use crate::{SpatialPlan, StableId, StreamCell, StreamingSpec};
use std::collections::BTreeMap;

/// Grid layout for partitioning a plan into stream cells. Derived from the
/// spec's `[streaming]` block — never hardcoded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StreamingLayout {
    pub cell_size_m: [f32; 3],
    pub preload_neighbors: u32,
}

impl StreamingLayout {
    pub fn from_spec(spec: &StreamingSpec) -> Self {
        Self {
            cell_size_m: spec.cell_size_m,
            preload_neighbors: spec.preload_neighbors,
        }
    }
}

/// Activation ladder for one stream cell. Loading climbs Unloaded→…→Rendered so
/// physics is live before anything is visible; unloading descends the reverse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CellPhase {
    Unloaded,
    /// Assets resident, entities spawned hidden — no collider, no render.
    Preloaded,
    /// Collision proxy + navmesh active; still not rendered.
    Physics,
    /// Fully visible.
    Rendered,
}

impl CellPhase {
    /// Next phase toward `Rendered` on load, or `None` when already there.
    pub fn next_up(self) -> Option<CellPhase> {
        match self {
            CellPhase::Unloaded => Some(CellPhase::Preloaded),
            CellPhase::Preloaded => Some(CellPhase::Physics),
            CellPhase::Physics => Some(CellPhase::Rendered),
            CellPhase::Rendered => None,
        }
    }

    /// Next phase toward `Unloaded` on unload (render torn down first).
    pub fn next_down(self) -> Option<CellPhase> {
        match self {
            CellPhase::Rendered => Some(CellPhase::Physics),
            CellPhase::Physics => Some(CellPhase::Preloaded),
            CellPhase::Preloaded => Some(CellPhase::Unloaded),
            CellPhase::Unloaded => None,
        }
    }
}

/// Partitions the plan's instances into a deterministic 3D grid of stream cells.
/// Cell AABBs are the grid bounds; `dependencies` are the occupied neighbour
/// cells within `preload_neighbors` (Chebyshev), i.e. what must preload with it.
pub fn compute_stream_cells(plan: &SpatialPlan, layout: &StreamingLayout) -> Vec<StreamCell> {
    let size = [
        layout.cell_size_m[0].max(f32::EPSILON),
        layout.cell_size_m[1].max(f32::EPSILON),
        layout.cell_size_m[2].max(f32::EPSILON),
    ];
    // BTreeMap keyed by integer coord → deterministic cell order, no HashMap.
    let mut occupied: BTreeMap<[i32; 3], ()> = BTreeMap::new();
    for instance in &plan.instances {
        occupied.insert(cell_coord(instance.translation_m, size), ());
    }

    let coords: Vec<[i32; 3]> = occupied.keys().copied().collect();
    let reach = layout.preload_neighbors as i32;
    coords
        .iter()
        .map(|&coord| {
            let dependencies = coords
                .iter()
                .filter(|&&other| other != coord && chebyshev(coord, other) <= reach)
                .map(|&other| cell_id(other))
                .collect();
            StreamCell {
                id: cell_id(coord),
                bounds_min_m: [
                    coord[0] as f32 * size[0],
                    coord[1] as f32 * size[1],
                    coord[2] as f32 * size[2],
                ],
                bounds_max_m: [
                    (coord[0] + 1) as f32 * size[0],
                    (coord[1] + 1) as f32 * size[1],
                    (coord[2] + 1) as f32 * size[2],
                ],
                dependencies,
            }
        })
        .collect()
}

/// The stream-cell id a world position maps to under `layout` — same grid rule
/// as [`compute_stream_cells`], so a runtime can assign each instance to its cell.
pub fn cell_of(pos: [f32; 3], layout: &StreamingLayout) -> StableId {
    let size = [
        layout.cell_size_m[0].max(f32::EPSILON),
        layout.cell_size_m[1].max(f32::EPSILON),
        layout.cell_size_m[2].max(f32::EPSILON),
    ];
    cell_id(cell_coord(pos, size))
}

fn cell_coord(pos: [f32; 3], size: [f32; 3]) -> [i32; 3] {
    [
        (pos[0] / size[0]).floor() as i32,
        (pos[1] / size[1]).floor() as i32,
        (pos[2] / size[2]).floor() as i32,
    ]
}

fn chebyshev(a: [i32; 3], b: [i32; 3]) -> i32 {
    (a[0] - b[0])
        .abs()
        .max((a[1] - b[1]).abs())
        .max((a[2] - b[2]).abs())
}

/// Stable, ASCII-safe cell id. Negatives are encoded `n<abs>` so the id stays in
/// the `StableId` alphabet (`[a-z0-9._-@]`) and never collides across signs.
fn cell_id(coord: [i32; 3]) -> StableId {
    let token = |v: i32| {
        if v < 0 {
            format!("n{}", v.unsigned_abs())
        } else {
            v.to_string()
        }
    };
    StableId::parse(format!(
        "cell.{}.{}.{}",
        token(coord[0]),
        token(coord[1]),
        token(coord[2])
    ))
    .expect("cell id is always in the StableId alphabet")
}

/// True iff `render` never precedes `collision_proxy`/`navmesh` on load. The
/// runtime adapter asserts this against the spec's declared `activate_order`.
pub fn activate_order_sound(order: &[String]) -> bool {
    let pos = |name: &str| order.iter().position(|s| s == name);
    let render = pos("render");
    if let (Some(r), Some(c)) = (render, pos("collision_proxy")) {
        if c > r {
            return false;
        }
    }
    if let (Some(r), Some(n)) = (render, pos("navmesh")) {
        if n > r {
            return false;
        }
    }
    true
}

/// True iff `render` is torn down before `collision_proxy` on unload.
pub fn deactivate_order_sound(order: &[String]) -> bool {
    let pos = |name: &str| order.iter().position(|s| s == name);
    match (pos("render"), pos("collision_proxy")) {
        (Some(r), Some(c)) => r < c,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KitInstance, LogicalPlan, SpatialPlan, StableId};

    fn empty_logical() -> LogicalPlan {
        // A minimal context is enough; stream cells only read `plan.instances`.
        let spec = crate::ContentSpec::parse_toml(
            r#"
schema_version = "forgia.content-spec/v1"
id = "forgia.fixture.stream@1.0.0"
kind = "structure"
[generation]
seed = 1
"#,
        )
        .unwrap();
        crate::compile_logical_plan(&spec, "test").unwrap()
    }

    fn instance(id: &str, pos: [f32; 3]) -> KitInstance {
        KitInstance {
            id: StableId::parse(id).unwrap(),
            kit_piece: StableId::parse("piece").unwrap(),
            zone: StableId::parse("zone").unwrap(),
            translation_m: pos,
            yaw_deg: 0.0,
        }
    }

    fn plan(instances: Vec<KitInstance>) -> SpatialPlan {
        SpatialPlan {
            logical: empty_logical(),
            instances,
            bindings: vec![],
            stream_cells: vec![],
        }
    }

    #[test]
    fn cells_are_deterministic_and_neighbours_link() {
        let layout = StreamingLayout {
            cell_size_m: [32.0, 24.0, 32.0],
            preload_neighbors: 1,
        };
        let p = plan(vec![
            instance("a", [1.0, 0.0, 1.0]),   // cell (0,0,0)
            instance("b", [40.0, 0.0, 1.0]),  // cell (1,0,0) — neighbour of a
            instance("c", [200.0, 0.0, 1.0]), // cell (6,0,0) — isolated
        ]);
        let cells = compute_stream_cells(&p, &layout);
        assert_eq!(cells.len(), 3);
        // Sorted by coord → deterministic order.
        assert_eq!(cells[0].id.as_str(), "cell.0.0.0");
        assert_eq!(cells[1].id.as_str(), "cell.1.0.0");
        assert_eq!(cells[2].id.as_str(), "cell.6.0.0");
        // (0,0,0) and (1,0,0) are Chebyshev-1 neighbours; (6,0,0) is alone.
        assert_eq!(cells[0].dependencies, vec![cells[1].id.clone()]);
        assert!(cells[2].dependencies.is_empty());
        // Idempotent.
        assert_eq!(compute_stream_cells(&p, &layout), cells);
    }

    #[test]
    fn negative_coords_stay_in_stable_id_alphabet() {
        let layout = StreamingLayout {
            cell_size_m: [10.0, 10.0, 10.0],
            preload_neighbors: 0,
        };
        let cells = compute_stream_cells(&plan(vec![instance("a", [-5.0, 0.0, -25.0])]), &layout);
        assert_eq!(cells[0].id.as_str(), "cell.n1.0.n3");
    }

    #[test]
    fn load_activates_physics_before_render() {
        let mut phase = CellPhase::Unloaded;
        let mut ladder = vec![phase];
        while let Some(next) = phase.next_up() {
            phase = next;
            ladder.push(phase);
        }
        assert_eq!(
            ladder,
            vec![
                CellPhase::Unloaded,
                CellPhase::Preloaded,
                CellPhase::Physics,
                CellPhase::Rendered
            ]
        );
        // Physics strictly precedes Rendered (the whole point).
        let physics = ladder
            .iter()
            .position(|p| *p == CellPhase::Physics)
            .unwrap();
        let render = ladder
            .iter()
            .position(|p| *p == CellPhase::Rendered)
            .unwrap();
        assert!(physics < render);
    }

    #[test]
    fn unload_tears_down_render_before_physics() {
        let mut phase = CellPhase::Rendered;
        let mut ladder = vec![phase];
        while let Some(next) = phase.next_down() {
            phase = next;
            ladder.push(phase);
        }
        assert_eq!(
            ladder,
            vec![
                CellPhase::Rendered,
                CellPhase::Physics,
                CellPhase::Preloaded,
                CellPhase::Unloaded
            ]
        );
    }

    #[test]
    fn declared_orders_are_checked_against_the_ladder() {
        let good_activate = vec![
            "collision_proxy".to_string(),
            "navmesh".to_string(),
            "render".to_string(),
        ];
        let bad_activate = vec!["render".to_string(), "collision_proxy".to_string()];
        assert!(activate_order_sound(&good_activate));
        assert!(!activate_order_sound(&bad_activate));

        let good_deactivate = vec!["render".to_string(), "collision_proxy".to_string()];
        let bad_deactivate = vec!["collision_proxy".to_string(), "render".to_string()];
        assert!(deactivate_order_sound(&good_deactivate));
        assert!(!deactivate_order_sound(&bad_deactivate));
    }

    #[test]
    fn layout_reads_from_spec() {
        let spec = crate::StreamingSpec {
            cell_size_m: [16.0, 8.0, 16.0],
            preload_neighbors: 2,
            activate_order: vec![],
            deactivate_order: vec![],
        };
        let layout = StreamingLayout::from_spec(&spec);
        assert_eq!(layout.cell_size_m, [16.0, 8.0, 16.0]);
        assert_eq!(layout.preload_neighbors, 2);
    }
}
