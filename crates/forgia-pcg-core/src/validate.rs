//! Hard validators for an assembled `SpatialPlan`.
//!
//! A hard violation makes the candidate non-exportable (cf pcg-methodology §6A
//! "hard/soft"): world bounds, plan budgets, contextual clearance and
//! reachability. Everything is data-driven — limits come from the
//! [`ContentSpec`], never hardcoded — and deterministic: instances/bindings are
//! visited in their stable recipe order, constraints in spec order.
//!
//! What this layer *cannot* prove at the plan level (capsule walkability on a
//! navmesh, lock-and-key dominance, triangle/VRAM budgets) is **not** silently
//! passed: it is recorded as a [`DeferredCheck`] naming the executor that owns
//! it (bot/navmesh, mission solver, `gltf-transform`).

use crate::kit::{Clearance, KitManifest, PieceSpec, SocketSpec};
use crate::{ContentSpec, KitInstance, SpatialPlan};
use serde::Serialize;
use std::collections::{BTreeSet, VecDeque};

/// Touching faces must not count as an intrusion; this epsilon (0.1 mm) keeps a
/// wall flush against a doorway from reading as "blocking" it.
const OVERLAP_EPS: f32 = 1e-4;

/// Counts a `SpatialPlan` can actually prove. Approximations are labelled: one
/// render mesh per instance is assumed for `visible_meshes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct PlanMetrics {
    pub instances: u32,
    pub stream_cells: u32,
    pub collision_proxies: u32,
    pub visible_meshes: u32,
}

/// A recognized constraint whose proof belongs to another executor. Surfacing it
/// (instead of dropping it) keeps validation honest without failing the plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeferredCheck {
    pub constraint_id: String,
    pub kind: String,
    pub executor: String,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum HardViolation {
    #[error("world bounds: structure extent on {axis} is {extent_m:.2}m > max {max_m:.2}m")]
    WorldBounds {
        axis: char,
        extent_m: f32,
        max_m: f32,
    },
    #[error("non-finite transform on instance `{instance}`")]
    NonFiniteTransform { instance: String },
    #[error("budget `{target}`: {metric} = {actual} exceeds max {max}")]
    Budget {
        target: String,
        metric: String,
        actual: u32,
        max: u32,
    },
    #[error("constraint `{id}`: {metric} = {actual} violates `{op} {limit}`")]
    ConstraintBudget {
        id: String,
        metric: String,
        actual: u32,
        op: String,
        limit: u32,
    },
    #[error("clearance: socket `{instance}/{socket}` is blocked by unbound instance `{by}`")]
    ClearanceBlocked {
        instance: String,
        socket: String,
        by: String,
    },
    #[error("constraint `{id}`: zone `{to}` is not reachable from `{from}`")]
    Unreachable {
        id: String,
        from: String,
        to: String,
    },
    #[error("constraint `{id}`: unknown budget metric `{metric}`")]
    UnknownBudgetMetric { id: String, metric: String },
    #[error("constraint `{id}`: unsupported operator `{op}`")]
    UnsupportedOperator { id: String, op: String },
    #[error("constraint `{id}`: unknown kind `{kind}`")]
    UnknownConstraintKind { id: String, kind: String },
    #[error("constraint `{id}` of kind `{kind}` is malformed: {reason}")]
    MalformedConstraint {
        id: String,
        kind: String,
        reason: &'static str,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidationReport {
    pub metrics: PlanMetrics,
    pub violations: Vec<HardViolation>,
    pub deferred: Vec<DeferredCheck>,
}

impl ValidationReport {
    /// A plan is exportable only if no hard violation survived.
    pub fn is_valid(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Kept for `pub use` symmetry with the crate root; a plain type alias so the
/// budget shape is nameable from the re-export without a second struct.
pub type ValidationBudget = crate::BudgetSpec;

/// Runs every plan-level hard validator and returns a structured report.
pub fn validate_spatial_plan(
    spec: &ContentSpec,
    manifest: &KitManifest,
    plan: &SpatialPlan,
) -> ValidationReport {
    let metrics = plan_metrics(manifest, plan);
    let mut violations = Vec::new();
    let mut deferred = Vec::new();

    check_finite(plan, &mut violations);
    check_world_bounds(spec, manifest, plan, &mut violations);
    check_budgets(spec, &metrics, &mut violations);
    check_clearance(manifest, plan, &mut violations);
    check_constraints(spec, &metrics, plan, &mut violations, &mut deferred);

    ValidationReport {
        metrics,
        violations,
        deferred,
    }
}

/// Metrics a plan can prove on its own. `visible_meshes` assumes one render mesh
/// per instance; the true mesh/triangle count is an offline (gltf-transform) gate.
pub fn plan_metrics(manifest: &KitManifest, plan: &SpatialPlan) -> PlanMetrics {
    let collision_proxies = plan
        .instances
        .iter()
        .filter(|inst| {
            find_piece(manifest, inst.kit_piece.as_str())
                .is_some_and(|piece| piece.collision.is_some())
        })
        .count() as u32;
    let instances = plan.instances.len() as u32;
    PlanMetrics {
        instances,
        stream_cells: plan.stream_cells.len() as u32,
        collision_proxies,
        visible_meshes: instances,
    }
}

fn check_finite(plan: &SpatialPlan, out: &mut Vec<HardViolation>) {
    for inst in &plan.instances {
        let finite = inst.translation_m.iter().all(|v| v.is_finite()) && inst.yaw_deg.is_finite();
        if !finite {
            out.push(HardViolation::NonFiniteTransform {
                instance: inst.id.to_string(),
            });
        }
    }
}

/// World-bounds gate: the whole structure's union AABB must fit inside a box of
/// `max_extent_m` (size-based, so it is anchor-agnostic and unambiguous). Pieces
/// without declared `bounds_m` contribute their origin point.
fn check_world_bounds(
    spec: &ContentSpec,
    manifest: &KitManifest,
    plan: &SpatialPlan,
    out: &mut Vec<HardViolation>,
) {
    let Some(bounds) = &spec.bounds else {
        return;
    };
    let mut umin = [f32::INFINITY; 3];
    let mut umax = [f32::NEG_INFINITY; 3];
    let mut any = false;
    for inst in &plan.instances {
        if !inst.translation_m.iter().all(|v| v.is_finite()) {
            continue; // reported by check_finite
        }
        let (mn, mx) = match find_piece(manifest, inst.kit_piece.as_str())
            .and_then(|piece| piece.bounds_m.as_ref())
        {
            Some(b) => world_aabb(b.min, b.max, inst.translation_m, inst.yaw_deg),
            None => (inst.translation_m, inst.translation_m),
        };
        any = true;
        for axis in 0..3 {
            umin[axis] = umin[axis].min(mn[axis]);
            umax[axis] = umax[axis].max(mx[axis]);
        }
    }
    if !any {
        return;
    }
    let axes = ['x', 'y', 'z'];
    for axis in 0..3 {
        let extent = umax[axis] - umin[axis];
        if extent > bounds.max_extent_m[axis] + OVERLAP_EPS {
            out.push(HardViolation::WorldBounds {
                axis: axes[axis],
                extent_m: extent,
                max_m: bounds.max_extent_m[axis],
            });
        }
    }
}

fn check_budgets(spec: &ContentSpec, metrics: &PlanMetrics, out: &mut Vec<HardViolation>) {
    // BTreeMap → deterministic target order.
    for (target, budget) in &spec.budgets {
        let checks = [
            ("stream_cells", budget.max_stream_cells, metrics.stream_cells),
            (
                "visible_meshes",
                budget.max_visible_meshes,
                metrics.visible_meshes,
            ),
            (
                "collision_proxies",
                budget.max_collision_proxies,
                metrics.collision_proxies,
            ),
        ];
        for (metric, limit, actual) in checks {
            if let Some(max) = limit {
                if actual > max {
                    out.push(HardViolation::Budget {
                        target: target.clone(),
                        metric: metric.to_string(),
                        actual,
                        max,
                    });
                }
            }
        }
    }
}

/// Contextual clearance: each socket's free volume must not be intruded by the
/// *solid* AABB of another instance — **unless** the two instances share a
/// binding, because a door seated in its wall's door socket is an intentional
/// overlap. This is deliberately not a naive global AABB non-overlap rule.
fn check_clearance(manifest: &KitManifest, plan: &SpatialPlan, out: &mut Vec<HardViolation>) {
    for a in &plan.instances {
        let Some(piece_a) = find_piece(manifest, a.kit_piece.as_str()) else {
            continue;
        };
        for socket in &piece_a.sockets {
            let Some(clearance_aabb) = socket_clearance_world_aabb(socket, a) else {
                continue;
            };
            for b in &plan.instances {
                if b.id == a.id || bound_together(plan, a, b) {
                    continue; // self or intentional (door-in-wall) overlap
                }
                let Some(piece_b) = find_piece(manifest, b.kit_piece.as_str()) else {
                    continue;
                };
                let Some(bounds_b) = piece_b.bounds_m.as_ref() else {
                    continue; // cannot test a piece with no solid bounds
                };
                let solid_b = world_aabb(bounds_b.min, bounds_b.max, b.translation_m, b.yaw_deg);
                if aabb_overlap(clearance_aabb, solid_b) {
                    out.push(HardViolation::ClearanceBlocked {
                        instance: a.id.to_string(),
                        socket: socket.id.clone(),
                        by: b.id.to_string(),
                    });
                }
            }
        }
    }
}

fn check_constraints(
    spec: &ContentSpec,
    metrics: &PlanMetrics,
    plan: &SpatialPlan,
    out: &mut Vec<HardViolation>,
    deferred: &mut Vec<DeferredCheck>,
) {
    for constraint in &spec.constraints.hard {
        match constraint.kind.as_str() {
            "reachability" => check_reachability(constraint, plan, out),
            "budget" => check_constraint_budget(constraint, metrics, out),
            // Recognized but proven elsewhere — surface, do not pass silently.
            "clearance" => deferred.push(DeferredCheck {
                constraint_id: constraint.id.clone(),
                kind: "clearance".into(),
                executor: "navmesh/bot capsule".into(),
            }),
            "dominance" => deferred.push(DeferredCheck {
                constraint_id: constraint.id.clone(),
                kind: "dominance".into(),
                executor: "mission/lock-key solver".into(),
            }),
            other => out.push(HardViolation::UnknownConstraintKind {
                id: constraint.id.clone(),
                kind: other.to_string(),
            }),
        }
    }
}

fn check_reachability(
    constraint: &crate::HardConstraint,
    plan: &SpatialPlan,
    out: &mut Vec<HardViolation>,
) {
    let Some(from) = &constraint.from else {
        out.push(HardViolation::MalformedConstraint {
            id: constraint.id.clone(),
            kind: "reachability".into(),
            reason: "missing `from` zone",
        });
        return;
    };
    if constraint.to.is_empty() {
        out.push(HardViolation::MalformedConstraint {
            id: constraint.id.clone(),
            kind: "reachability".into(),
            reason: "empty `to` list",
        });
        return;
    }
    for to in &constraint.to {
        if !zone_reachable(plan, from, to) {
            out.push(HardViolation::Unreachable {
                id: constraint.id.clone(),
                from: from.clone(),
                to: to.clone(),
            });
        }
    }
}

fn check_constraint_budget(
    constraint: &crate::HardConstraint,
    metrics: &PlanMetrics,
    out: &mut Vec<HardViolation>,
) {
    let metric = constraint.metric.as_deref().unwrap_or_default();
    let actual = match metric {
        "collision_proxies" => metrics.collision_proxies,
        "stream_cells" => metrics.stream_cells,
        "visible_meshes" => metrics.visible_meshes,
        "instances" => metrics.instances,
        other => {
            out.push(HardViolation::UnknownBudgetMetric {
                id: constraint.id.clone(),
                metric: other.to_string(),
            });
            return;
        }
    };
    let op = constraint.op.as_deref().unwrap_or("<=");
    // Budgets are integer counts; a fractional literal is floored deliberately.
    let limit = constraint.value.unwrap_or(0.0).max(0.0) as u32;
    let satisfied = match op {
        "<=" => actual <= limit,
        "<" => actual < limit,
        ">=" => actual >= limit,
        ">" => actual > limit,
        "==" => actual == limit,
        other => {
            out.push(HardViolation::UnsupportedOperator {
                id: constraint.id.clone(),
                op: other.to_string(),
            });
            return;
        }
    };
    if !satisfied {
        out.push(HardViolation::ConstraintBudget {
            id: constraint.id.clone(),
            metric: metric.to_string(),
            actual,
            op: op.to_string(),
            limit,
        });
    }
}

// ── graph / geometry helpers ────────────────────────────────────────────────

/// Plan-level reachability = structural connectivity over *all* socket bindings
/// between an instance of `from_zone` and one of `to_zone`. Capsule walkability
/// on a navmesh is a separate, deferred proof — this never claims to prove it.
fn zone_reachable(plan: &SpatialPlan, from_zone: &str, to_zone: &str) -> bool {
    let goals: BTreeSet<&str> = plan
        .instances
        .iter()
        .filter(|i| i.zone.as_str() == to_zone)
        .map(|i| i.id.as_str())
        .collect();
    let starts: Vec<&str> = plan
        .instances
        .iter()
        .filter(|i| i.zone.as_str() == from_zone)
        .map(|i| i.id.as_str())
        .collect();
    if goals.is_empty() || starts.is_empty() {
        return false;
    }
    let mut visited: BTreeSet<&str> = BTreeSet::new();
    let mut queue: VecDeque<&str> = VecDeque::new();
    for start in starts {
        if visited.insert(start) {
            queue.push_back(start);
        }
    }
    while let Some(current) = queue.pop_front() {
        if goals.contains(current) {
            return true;
        }
        for neighbour in neighbours(plan, current) {
            if visited.insert(neighbour) {
                queue.push_back(neighbour);
            }
        }
    }
    false
}

fn neighbours<'a>(plan: &'a SpatialPlan, id: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    for binding in &plan.bindings {
        if binding.from_instance.as_str() == id {
            out.push(binding.to_instance.as_str());
        } else if binding.to_instance.as_str() == id {
            out.push(binding.from_instance.as_str());
        }
    }
    out
}

fn bound_together(plan: &SpatialPlan, a: &KitInstance, b: &KitInstance) -> bool {
    plan.bindings.iter().any(|binding| {
        (binding.from_instance == a.id && binding.to_instance == b.id)
            || (binding.from_instance == b.id && binding.to_instance == a.id)
    })
}

fn find_piece<'a>(manifest: &'a KitManifest, id: &str) -> Option<&'a PieceSpec> {
    manifest.pieces.iter().find(|piece| piece.id == id)
}

fn socket_clearance_world_aabb(
    socket: &SocketSpec,
    instance: &KitInstance,
) -> Option<([f32; 3], [f32; 3])> {
    let clearance: &Clearance = socket.clearance.as_ref()?;
    let half = clearance.local_half_extents();
    let centre = socket.frame.origin_m;
    let local_min = [centre[0] - half[0], centre[1] - half[1], centre[2] - half[2]];
    let local_max = [centre[0] + half[0], centre[1] + half[1], centre[2] + half[2]];
    Some(world_aabb(
        local_min,
        local_max,
        instance.translation_m,
        instance.yaw_deg,
    ))
}

/// World AABB of a local box after a yaw-only rotation and translation. All eight
/// corners are rotated so a yawed footprint is enclosed correctly.
fn world_aabb(
    local_min: [f32; 3],
    local_max: [f32; 3],
    translation: [f32; 3],
    yaw_deg: f32,
) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for &x in &[local_min[0], local_max[0]] {
        for &y in &[local_min[1], local_max[1]] {
            for &z in &[local_min[2], local_max[2]] {
                let rotated = crate::solver::rotate_y([x, y, z], yaw_deg);
                for axis in 0..3 {
                    let world = rotated[axis] + translation[axis];
                    min[axis] = min[axis].min(world);
                    max[axis] = max[axis].max(world);
                }
            }
        }
    }
    (min, max)
}

fn aabb_overlap(a: ([f32; 3], [f32; 3]), b: ([f32; 3], [f32; 3])) -> bool {
    (0..3).all(|axis| a.0[axis] < b.1[axis] - OVERLAP_EPS && b.0[axis] < a.1[axis] - OVERLAP_EPS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::KitManifest;
    use crate::{
        compile_logical_plan, ContentSpec, KitInstance, LogicalPlan, SocketBinding, SpatialPlan,
        StableId, StreamCell,
    };

    // A kit with solid bounds, a box collider and a doorway clearance on the
    // portal socket. Structural joints carry no clearance (they are meant to be
    // filled), so only doorways are guarded.
    const KIT: &str = r#"
schema_version = "forgia.kit-manifest/v1"
id = "forgia.fixture.stone@1.0.0"
[[pieces]]
id = "wall.door"
asset = "render/wall.glb"
bounds_m = { min = [-2.0, 0.0, -0.25], max = [2.0, 4.0, 0.25] }
collision = { kind = "box" }
[[pieces.sockets]]
id = "door_front"
family = "portal.door"
role = "portal"
gender = "female"
frame = { origin_m = [0.0, 1.5, -0.25], forward = [0.0, 0.0, -1.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "rect", width_m = 1.6, height_m = 3.0, depth_m = 0.5 }
clearance = { shape = "box", half_extents_m = [0.9, 1.5, 1.2] }
accepts = [{ family = "portal.door", gender = "male" }]
[[pieces]]
id = "door.oak"
asset = "render/door.glb"
bounds_m = { min = [-0.8, 0.0, -0.1], max = [0.8, 3.0, 0.1] }
collision = { kind = "box" }
[[pieces.sockets]]
id = "frame_back"
family = "portal.door"
role = "portal"
gender = "male"
frame = { origin_m = [0.0, 1.5, 0.1], forward = [0.0, 0.0, 1.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "rect", width_m = 1.6, height_m = 3.0, depth_m = 0.5 }
accepts = [{ family = "portal.door", gender = "female" }]
[[pieces]]
id = "blocker"
asset = "render/blocker.glb"
bounds_m = { min = [-1.0, 0.0, -1.0], max = [1.0, 3.0, 1.0] }
collision = { kind = "box" }
"#;

    const SPEC: &str = r#"
schema_version = "forgia.content-spec/v1"
id = "forgia.fixture.hall@1.0.0"
kind = "structure"
[generation]
seed = 7
[bounds]
max_extent_m = [40.0, 12.0, 40.0]
anchor = "world_origin"
[budgets.desktop]
max_collision_proxies = 8
max_visible_meshes = 32
max_stream_cells = 4
[[zones]]
id = "great_hall"
kind = "space.hall"
[[zones]]
id = "west_entry"
kind = "space.entrance"
parent = "great_hall"
[[constraints.hard]]
id = "hall_accessible"
kind = "reachability"
from = "west_entry"
to = ["great_hall"]
[[constraints.hard]]
id = "physical_proxy_only"
kind = "budget"
metric = "collision_proxies"
op = "<="
value = 8
"#;

    fn manifest() -> KitManifest {
        KitManifest::parse_toml(KIT).expect("valid kit")
    }

    fn spec() -> ContentSpec {
        ContentSpec::parse_toml(SPEC).expect("valid spec")
    }

    fn logical() -> LogicalPlan {
        compile_logical_plan(&spec(), "test").expect("logical plan")
    }

    fn instance(id: &str, piece: &str, zone: &str, translation: [f32; 3], yaw: f32) -> KitInstance {
        KitInstance {
            id: StableId::parse(id).unwrap(),
            kit_piece: StableId::parse(piece).unwrap(),
            zone: StableId::parse(zone).unwrap(),
            translation_m: translation,
            yaw_deg: yaw,
        }
    }

    fn binding(from: &str, from_socket: &str, to: &str, to_socket: &str) -> SocketBinding {
        SocketBinding {
            from_instance: StableId::parse(from).unwrap(),
            from_socket: from_socket.into(),
            to_instance: StableId::parse(to).unwrap(),
            to_socket: to_socket.into(),
        }
    }

    /// A wall + the door seated in its door socket, both in reachable zones.
    fn valid_plan() -> SpatialPlan {
        SpatialPlan {
            logical: logical(),
            instances: vec![
                instance("instance.wall", "wall.door", "great_hall", [0.0, 0.0, 0.0], 0.0),
                instance("instance.door", "door.oak", "west_entry", [0.0, 0.0, -0.35], 0.0),
            ],
            bindings: vec![binding(
                "instance.wall",
                "door_front",
                "instance.door",
                "frame_back",
            )],
            stream_cells: vec![StreamCell {
                id: StableId::parse("cell.main").unwrap(),
                bounds_min_m: [-2.0, 0.0, -2.0],
                bounds_max_m: [2.0, 4.0, 2.0],
                dependencies: vec![],
            }],
        }
    }

    #[test]
    fn door_in_wall_is_intentional_overlap_not_a_violation() {
        let report = validate_spatial_plan(&spec(), &manifest(), &valid_plan());
        assert!(
            report.is_valid(),
            "door seated in its wall must pass: {:?}",
            report.violations
        );
        assert_eq!(report.metrics.collision_proxies, 2);
        assert_eq!(report.metrics.stream_cells, 1);
    }

    #[test]
    fn unbound_blocker_in_doorway_is_flagged() {
        let mut plan = valid_plan();
        // A blocker straddling the doorway clearance, bound to nothing.
        plan.instances.push(instance(
            "instance.blocker",
            "blocker",
            "great_hall",
            [0.0, 0.0, -1.2],
            0.0,
        ));
        let report = validate_spatial_plan(&spec(), &manifest(), &plan);
        assert!(report.violations.iter().any(|v| matches!(
            v,
            HardViolation::ClearanceBlocked { by, .. } if by == "instance.blocker"
        )));
    }

    #[test]
    fn world_bounds_violation_is_detected() {
        let mut plan = valid_plan();
        // Push the door far away so the union extent blows the 40m X budget.
        plan.instances[1].translation_m = [100.0, 0.0, 0.0];
        let report = validate_spatial_plan(&spec(), &manifest(), &plan);
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, HardViolation::WorldBounds { axis: 'x', .. })));
    }

    #[test]
    fn collision_proxy_budget_is_enforced() {
        // Tight budget of 1 proxy vs a 2-proxy plan → both the named budget and
        // the constraint budget fire.
        let tight = SPEC.replace("max_collision_proxies = 8", "max_collision_proxies = 1");
        let tight = tight.replace("value = 8", "value = 1");
        let spec = ContentSpec::parse_toml(&tight).unwrap();
        let report = validate_spatial_plan(&spec, &manifest(), &valid_plan());
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, HardViolation::Budget { metric, .. } if metric == "collision_proxies")));
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, HardViolation::ConstraintBudget { .. })));
    }

    #[test]
    fn reachability_fails_when_zone_is_disconnected() {
        let mut plan = valid_plan();
        plan.bindings.clear(); // sever the only door link
        let report = validate_spatial_plan(&spec(), &manifest(), &plan);
        assert!(report.violations.iter().any(|v| matches!(
            v,
            HardViolation::Unreachable { to, .. } if to == "great_hall"
        )));
    }

    #[test]
    fn unknown_constraint_kind_is_flagged_not_ignored() {
        let with_unknown = SPEC.replace(
            "[[constraints.hard]]\nid = \"physical_proxy_only\"",
            "[[constraints.hard]]\nid = \"mystery\"\nkind = \"teleportation\"\n[[constraints.hard]]\nid = \"physical_proxy_only\"",
        );
        let spec = ContentSpec::parse_toml(&with_unknown).unwrap();
        let report = validate_spatial_plan(&spec, &manifest(), &valid_plan());
        assert!(report.violations.iter().any(|v| matches!(
            v,
            HardViolation::UnknownConstraintKind { kind, .. } if kind == "teleportation"
        )));
    }

    #[test]
    fn recognized_but_unprovable_constraints_are_deferred() {
        let with_deferred = SPEC.replace(
            "[[constraints.hard]]\nid = \"physical_proxy_only\"",
            "[[constraints.hard]]\nid = \"capsule\"\nkind = \"clearance\"\nradius_m = 0.3\nheight_m = 2.0\n[[constraints.hard]]\nid = \"physical_proxy_only\"",
        );
        let spec = ContentSpec::parse_toml(&with_deferred).unwrap();
        let report = validate_spatial_plan(&spec, &manifest(), &valid_plan());
        assert!(report.is_valid(), "deferred is not a violation");
        assert!(report
            .deferred
            .iter()
            .any(|d| d.kind == "clearance" && d.executor.contains("navmesh")));
    }
}
