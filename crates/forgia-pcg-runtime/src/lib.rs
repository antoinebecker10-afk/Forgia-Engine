//! # forgia-pcg-runtime
//!
//! Thin Bevy adapter that **branches an assembled `SpatialPlan` into cell
//! streaming**. It owns the activation *ordering* — collision/navmesh before
//! render on load, the reverse on unload — and the destination-cell preload; the
//! concrete spawn/collider/visibility work stays in the game layer, driven from
//! the ordered transition log this adapter emits.
//!
//! Design (cf pcg-methodology §6F / §6I "adaptateurs minces") : the deterministic
//! logic (cell partition, activation ladder) lives in `forgia-pcg-core` and is
//! reused here, so this crate adds no second streaming manager and stays testable
//! headless. It is intentionally **not** yet registered in the live app plugin
//! set — that wiring is the next milestone (the game's spawn/collider systems
//! consume [`PcgStreamState::transitions`]).

use bevy::prelude::*;
use forgia_pcg_core::{
    activate_order_sound, cell_of, compute_stream_cells, deactivate_order_sound, CellPhase,
    KitManifest, SpatialPlan, StreamingLayout, StreamingSpec,
};
use std::collections::BTreeMap;

/// One instance resolved to the assets the runtime will actually stream.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeInstance {
    pub id: String,
    pub render_asset: String,
    pub collision_asset: Option<String>,
    pub translation_m: [f32; 3],
    pub yaw_deg: f32,
}

/// A stream cell resolved for the runtime: grid bounds, preload dependencies and
/// the instances it owns.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeCell {
    pub id: String,
    pub bounds_min_m: [f32; 3],
    pub bounds_max_m: [f32; 3],
    pub dependencies: Vec<String>,
    pub instances: Vec<RuntimeInstance>,
}

/// Runtime-ready streaming plan built offline-style from a `SpatialPlan`.
#[derive(Resource, Clone, Debug)]
pub struct PcgStreamPlan {
    pub cells: Vec<RuntimeCell>,
    /// Whether the spec's declared order agrees with the enforced ladder. The
    /// driver always uses the ladder; `false` is a spec authoring error worth a
    /// warning, never silently honoured.
    pub activate_order_sound: bool,
    pub deactivate_order_sound: bool,
}

impl PcgStreamPlan {
    /// Builds the runtime plan from an assembled plan, its kit manifest (asset
    /// paths) and the spec streaming block. Deterministic — cells and their
    /// instances follow the stable grid order.
    pub fn build(plan: &SpatialPlan, manifest: &KitManifest, streaming: &StreamingSpec) -> Self {
        let layout = StreamingLayout::from_spec(streaming);
        let stream_cells = compute_stream_cells(plan, &layout);
        let cells = stream_cells
            .iter()
            .map(|cell| {
                let instances = plan
                    .instances
                    .iter()
                    .filter(|inst| {
                        cell_of(inst.translation_m, &layout).as_str() == cell.id.as_str()
                    })
                    .map(|inst| resolve_instance(inst, manifest))
                    .collect();
                RuntimeCell {
                    id: cell.id.to_string(),
                    bounds_min_m: cell.bounds_min_m,
                    bounds_max_m: cell.bounds_max_m,
                    dependencies: cell.dependencies.iter().map(|d| d.to_string()).collect(),
                    instances,
                }
            })
            .collect();
        Self {
            cells,
            activate_order_sound: activate_order_sound(&streaming.activate_order),
            deactivate_order_sound: deactivate_order_sound(&streaming.deactivate_order),
        }
    }

    fn cell(&self, id: &str) -> Option<&RuntimeCell> {
        self.cells.iter().find(|c| c.id == id)
    }
}

fn resolve_instance(
    inst: &forgia_pcg_core::KitInstance,
    manifest: &KitManifest,
) -> RuntimeInstance {
    let piece = manifest
        .pieces
        .iter()
        .find(|p| p.id == inst.kit_piece.as_str());
    RuntimeInstance {
        id: inst.id.to_string(),
        render_asset: piece.map(|p| p.asset.clone()).unwrap_or_default(),
        collision_asset: piece
            .and_then(|p| p.collision.as_ref())
            .and_then(|c| c.asset.clone()),
        translation_m: inst.translation_m,
        yaw_deg: inst.yaw_deg,
    }
}

/// One applied phase transition. The game layer performs the concrete work
/// (load/spawn hidden → insert collider+navmesh → set visible, reverse on
/// unload) from these, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellTransition {
    pub cell_index: usize,
    pub phase: CellPhase,
}

/// Load/unload requests pushed by the game (e.g. on approaching a portal).
#[derive(Resource, Default)]
pub struct PcgStreamRequests {
    queue: Vec<(String, CellPhase)>,
}

impl PcgStreamRequests {
    /// Request a cell be fully streamed in (and its neighbours preloaded).
    pub fn load(&mut self, cell: impl Into<String>) {
        self.queue.push((cell.into(), CellPhase::Rendered));
    }

    /// Request a cell be torn down (render first, then physics, then unloaded).
    pub fn unload(&mut self, cell: impl Into<String>) {
        self.queue.push((cell.into(), CellPhase::Unloaded));
    }
}

/// Live streaming state + the ordered transition log the game consumes.
#[derive(Resource, Default)]
pub struct PcgStreamState {
    pub phase: BTreeMap<String, CellPhase>,
    pub target: BTreeMap<String, CellPhase>,
    /// Ordered `(cell, new_phase)` transitions since the last drain.
    pub transitions: Vec<(String, CellPhase)>,
}

impl PcgStreamState {
    pub fn phase_of(&self, cell: &str) -> CellPhase {
        self.phase.get(cell).copied().unwrap_or(CellPhase::Unloaded)
    }

    /// Observability summary: how many cells sit at each phase.
    pub fn rendered_count(&self) -> usize {
        self.phase
            .values()
            .filter(|p| **p == CellPhase::Rendered)
            .count()
    }
}

/// Steps every requested cell one rung toward its target per frame. Because a
/// cell must pass `Physics` before `Rendered`, collision/navmesh activate a frame
/// before render; unloading descends the reverse ladder.
fn drive_pcg_stream(
    plan: Option<Res<PcgStreamPlan>>,
    mut requests: ResMut<PcgStreamRequests>,
    mut state: ResMut<PcgStreamState>,
) {
    for (cell, target) in std::mem::take(&mut requests.queue) {
        state.target.insert(cell.clone(), target);
        // Loading a cell preloads its neighbours (destination-cell preload).
        if target == CellPhase::Rendered {
            if let Some(deps) = plan
                .as_ref()
                .and_then(|p| p.cell(&cell))
                .map(|c| c.dependencies.clone())
            {
                for dep in deps {
                    let entry = state.target.entry(dep).or_insert(CellPhase::Unloaded);
                    if *entry < CellPhase::Preloaded {
                        *entry = CellPhase::Preloaded;
                    }
                }
            }
        }
    }

    // Deterministic order (BTreeMap keys), one rung per cell per frame.
    let cells: Vec<String> = state.target.keys().cloned().collect();
    for cell in cells {
        let current = state.phase_of(&cell);
        let target = state
            .target
            .get(&cell)
            .copied()
            .unwrap_or(CellPhase::Unloaded);
        let next = match current.cmp(&target) {
            std::cmp::Ordering::Less => current.next_up(),
            std::cmp::Ordering::Greater => current.next_down(),
            std::cmp::Ordering::Equal => None,
        };
        let Some(next) = next else { continue };
        state.transitions.push((cell.clone(), next));
        if next == CellPhase::Unloaded {
            state.phase.remove(&cell);
            state.target.remove(&cell);
        } else {
            state.phase.insert(cell, next);
        }
    }
}

/// Registers the streaming state resources and the activation driver. Insert a
/// [`PcgStreamPlan`] resource (built via [`PcgStreamPlan::build`]) to enable
/// neighbour preloading.
pub struct ForgiaPcgStreamPlugin;

impl Plugin for ForgiaPcgStreamPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PcgStreamRequests>()
            .init_resource::<PcgStreamState>()
            .add_systems(Update, drive_pcg_stream);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forgia_pcg_core::{
        assemble_socket_chain, compile_logical_plan, AttachRequest, ContentSpec, RootPlacement,
    };

    const KIT: &str = include_str!("../../../assets/pcg/kits/castle_stone/1.0.0/kit.toml");
    const SPEC: &str = include_str!("../../../assets/pcg/specs/hall_highlands.content-spec.toml");

    fn attach(
        instance: &str,
        zone: &str,
        from: &str,
        from_socket: &str,
        piece: &str,
        socket: &str,
    ) -> AttachRequest {
        AttachRequest {
            instance_id: instance.into(),
            zone_id: zone.into(),
            from_instance_id: from.into(),
            from_socket_id: from_socket.into(),
            piece_id: piece.into(),
            socket_id: socket.into(),
        }
    }

    fn shipped_plan() -> (ContentSpec, KitManifest, SpatialPlan) {
        let spec = ContentSpec::parse_toml(SPEC).unwrap();
        let manifest = KitManifest::parse_toml(KIT).unwrap();
        let logical = compile_logical_plan(&spec, "rt-test").unwrap();
        let plan = assemble_socket_chain(
            logical,
            &manifest,
            RootPlacement {
                instance_id: "inst.great_hall".into(),
                piece_id: "great_hall".into(),
                zone_id: "great_hall".into(),
                translation_m: [0.0, 0.0, 0.0],
                yaw_deg: 0.0,
            },
            &[
                attach(
                    "inst.entree",
                    "west_entry",
                    "inst.great_hall",
                    "entry",
                    "entree",
                    "door_out",
                ),
                attach(
                    "inst.mur_a",
                    "rampart",
                    "inst.entree",
                    "west",
                    "mur_droit",
                    "east",
                ),
                attach("inst.tour", "tower", "inst.mur_a", "west", "tour", "attach"),
                attach(
                    "inst.angle",
                    "rampart",
                    "inst.entree",
                    "east",
                    "angle",
                    "west",
                ),
            ],
        )
        .unwrap();
        (spec, manifest, plan)
    }

    #[test]
    fn builds_runtime_cells_with_resolved_assets() {
        let (spec, manifest, plan) = shipped_plan();
        let stream_plan = PcgStreamPlan::build(&plan, &manifest, spec.streaming.as_ref().unwrap());
        assert!(stream_plan.cells.len() >= 2);
        assert!(stream_plan.activate_order_sound);
        assert!(stream_plan.deactivate_order_sound);
        // Every placed instance lands in exactly one cell, with a render asset.
        let total: usize = stream_plan.cells.iter().map(|c| c.instances.len()).sum();
        assert_eq!(total, plan.instances.len());
        assert!(stream_plan
            .cells
            .iter()
            .flat_map(|c| &c.instances)
            .all(|i| i.render_asset.ends_with(".glb#Scene0")));
    }

    #[test]
    fn load_activates_physics_before_render_and_preloads_neighbour() {
        let (spec, manifest, plan) = shipped_plan();
        let stream_plan = PcgStreamPlan::build(&plan, &manifest, spec.streaming.as_ref().unwrap());
        let loaded = stream_plan.cells[0].id.clone();
        let dependency = stream_plan.cells[0].dependencies[0].clone();

        let mut app = App::new();
        app.add_plugins(ForgiaPcgStreamPlugin);
        app.insert_resource(stream_plan);
        app.world_mut()
            .resource_mut::<PcgStreamRequests>()
            .load(&loaded);
        for _ in 0..4 {
            app.update();
        }

        let state = app.world().resource::<PcgStreamState>();
        let seq: Vec<CellPhase> = state
            .transitions
            .iter()
            .filter(|(cell, _)| *cell == loaded)
            .map(|(_, phase)| *phase)
            .collect();
        let index = |p: CellPhase| seq.iter().position(|x| *x == p).expect("phase reached");
        assert!(index(CellPhase::Preloaded) < index(CellPhase::Physics));
        assert!(index(CellPhase::Physics) < index(CellPhase::Rendered));
        assert_eq!(state.phase_of(&loaded), CellPhase::Rendered);
        // Destination-cell preload brought the neighbour to Preloaded, not further.
        assert_eq!(state.phase_of(&dependency), CellPhase::Preloaded);
    }

    #[test]
    fn unload_tears_down_render_before_physics() {
        let (spec, manifest, plan) = shipped_plan();
        let stream_plan = PcgStreamPlan::build(&plan, &manifest, spec.streaming.as_ref().unwrap());
        let cell = stream_plan.cells[0].id.clone();

        let mut app = App::new();
        app.add_plugins(ForgiaPcgStreamPlugin);
        app.insert_resource(stream_plan);
        // Load fully, then unload.
        app.world_mut()
            .resource_mut::<PcgStreamRequests>()
            .load(&cell);
        for _ in 0..4 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<PcgStreamState>()
            .transitions
            .clear();
        app.world_mut()
            .resource_mut::<PcgStreamRequests>()
            .unload(&cell);
        for _ in 0..4 {
            app.update();
        }

        let state = app.world().resource::<PcgStreamState>();
        let seq: Vec<CellPhase> = state
            .transitions
            .iter()
            .filter(|(c, _)| *c == cell)
            .map(|(_, phase)| *phase)
            .collect();
        assert_eq!(
            seq,
            vec![
                CellPhase::Physics,
                CellPhase::Preloaded,
                CellPhase::Unloaded
            ]
        );
        assert_eq!(state.phase_of(&cell), CellPhase::Unloaded);
    }
}
