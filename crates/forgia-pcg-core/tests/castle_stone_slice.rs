//! End-to-end vertical slice on the *shipped* grey-box contracts.
//!
//! Loads the real `assets/pcg/` content-spec + kit-manifest, assembles the five
//! pieces (grande salle, entrée, mur droit, angle, tour) with the constructive
//! solver, computes the stream cells and runs the hard validators. This ties the
//! published grey-box kit to a passing spec→kit→solver→SpatialPlan→cells→validate
//! chain, so a broken contract fails CI here rather than at runtime.

use forgia_pcg_core::{
    assemble_socket_chain, compile_logical_plan, compute_stream_cells, validate_spatial_plan,
    AttachRequest, ContentSpec, KitManifest, RootPlacement, StreamingLayout,
};

const KIT: &str = include_str!("../../../assets/pcg/kits/castle_stone/1.0.0/kit.toml");
const SPEC: &str = include_str!("../../../assets/pcg/specs/hall_highlands.content-spec.toml");

fn attach(instance: &str, zone: &str, from: &str, from_socket: &str, piece: &str, socket: &str) -> AttachRequest {
    AttachRequest {
        instance_id: instance.into(),
        zone_id: zone.into(),
        from_instance_id: from.into(),
        from_socket_id: from_socket.into(),
        piece_id: piece.into(),
        socket_id: socket.into(),
    }
}

#[test]
fn castle_stone_slice_assembles_and_validates() {
    let spec = ContentSpec::parse_toml(SPEC).expect("shipped content-spec parses");
    let manifest = KitManifest::parse_toml(KIT).expect("shipped kit-manifest parses");
    let logical = compile_logical_plan(&spec, "castle-stone-slice/0.1").expect("logical plan");

    // Ordered recipe; every attachment binds two raw-opposed, same-family sockets
    // (the V1 constructive solver's contract).
    let root = RootPlacement {
        instance_id: "inst.great_hall".into(),
        piece_id: "great_hall".into(),
        zone_id: "great_hall".into(),
        translation_m: [0.0, 0.0, 0.0],
        yaw_deg: 0.0,
    };
    let attachments = [
        attach("inst.entree", "west_entry", "inst.great_hall", "entry", "entree", "door_out"),
        attach("inst.mur_a", "rampart", "inst.entree", "west", "mur_droit", "east"),
        attach("inst.tour", "tower", "inst.mur_a", "west", "tour", "attach"),
        attach("inst.angle", "rampart", "inst.entree", "east", "angle", "west"),
    ];

    let mut plan =
        assemble_socket_chain(logical, &manifest, root, &attachments).expect("slice assembles");
    assert_eq!(plan.instances.len(), 5, "five pieces placed");
    assert_eq!(plan.bindings.len(), 4, "four socket bindings");

    // Golden transforms — the Cycles preview layout (previews/slice_layout.json)
    // mirrors these; any solver/kit drift must fail here, not in the render.
    let golden = [
        ("inst.great_hall", [0.0, 0.0, 0.0]),
        ("inst.entree", [0.0, 0.0, 6.25]),
        ("inst.mur_a", [-4.0, 0.0, 6.25]),
        ("inst.tour", [-8.0, 0.0, 6.25]),
        ("inst.angle", [2.25, 0.0, 6.25]),
    ];
    for (id, translation) in golden {
        let inst = plan
            .instances
            .iter()
            .find(|i| i.id.as_str() == id)
            .expect("golden instance placed");
        for axis in 0..3 {
            assert!(
                (inst.translation_m[axis] - translation[axis]).abs() < 1e-4,
                "{id}: expected {translation:?}, got {:?}",
                inst.translation_m
            );
        }
        assert!(inst.yaw_deg.abs() < 1e-4, "{id}: yaw {}", inst.yaw_deg);
    }

    let layout = StreamingLayout::from_spec(spec.streaming.as_ref().expect("[streaming] present"));
    plan.stream_cells = compute_stream_cells(&plan, &layout);
    assert!(!plan.stream_cells.is_empty(), "cells computed from the assembly");

    let report = validate_spatial_plan(&spec, &manifest, &plan);
    assert!(
        report.is_valid(),
        "shipped slice must pass the hard validators: {:?}",
        report.violations
    );
    // Every piece carries a simple box proxy — the anti-8052-TriMesh budget holds.
    assert_eq!(report.metrics.collision_proxies, 5);
    assert!(report.metrics.collision_proxies <= 32);
    // Capsule walkability is recognized but deferred to the navmesh executor.
    assert!(
        report
            .deferred
            .iter()
            .any(|d| d.kind == "clearance" && d.executor.contains("navmesh")),
        "capsule clearance is surfaced as deferred, not silently passed"
    );
}
