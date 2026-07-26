//! Constructive PCG solver for explicit socket chains.
//!
//! This is deliberately the smallest reliable solver: it does not search or
//! guess a layout. Given an ordered recipe, it aligns each requested piece to
//! one already placed socket and emits the generic `SpatialPlan` contract.

use crate::kit::{KitManifest, PieceSpec, SocketCompatibility, SocketSpec};
use crate::{KitInstance, LogicalPlan, PcgError, SocketBinding, SpatialPlan, StableId};

#[derive(Clone, Debug, PartialEq)]
pub struct RootPlacement {
    pub instance_id: String,
    pub piece_id: String,
    pub zone_id: String,
    pub translation_m: [f32; 3],
    pub yaw_deg: f32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachRequest {
    pub instance_id: String,
    pub zone_id: String,
    pub from_instance_id: String,
    pub from_socket_id: String,
    pub piece_id: String,
    pub socket_id: String,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ConstructiveError {
    #[error("invalid stable id: {0}")]
    InvalidId(#[from] PcgError),
    #[error("unknown zone `{0}`")]
    UnknownZone(String),
    #[error("unknown piece `{0}`")]
    UnknownPiece(String),
    #[error("unknown socket `{socket}` on piece `{piece}`")]
    UnknownSocket { piece: String, socket: String },
    #[error("instance `{0}` is duplicated")]
    DuplicateInstance(String),
    #[error("source instance `{0}` has not been placed yet")]
    UnknownInstance(String),
    #[error("sockets are incompatible: {0}")]
    Incompatible(String),
    #[error("socket `{piece}/{socket}` cannot be aligned by a yaw-only placement")]
    NonPlanarSocket { piece: String, socket: String },
}

/// Applies an explicit, ordered assembly recipe. The ordering is part of the
/// content plan, so a build stays reproducible without hash-map iteration or
/// a hidden random choice. Search/WFC can later emit the same recipe format.
pub fn assemble_socket_chain(
    logical: LogicalPlan,
    manifest: &KitManifest,
    root: RootPlacement,
    attachments: &[AttachRequest],
) -> Result<SpatialPlan, ConstructiveError> {
    manifest
        .validate()
        .map_err(|error| ConstructiveError::Incompatible(error.to_string()))?;
    let mut instances = Vec::with_capacity(attachments.len() + 1);
    let mut bindings = Vec::with_capacity(attachments.len());

    let root_piece = find_piece(manifest, &root.piece_id)?;
    validate_zone(&logical, &root.zone_id)?;
    let root_instance = instance_from_root(&root, root_piece)?;
    instances.push(root_instance);

    for request in attachments {
        if instances
            .iter()
            .any(|instance| instance.id.as_str() == request.instance_id)
        {
            return Err(ConstructiveError::DuplicateInstance(
                request.instance_id.clone(),
            ));
        }
        validate_zone(&logical, &request.zone_id)?;
        let source = instances
            .iter()
            .find(|instance| instance.id.as_str() == request.from_instance_id)
            .ok_or_else(|| ConstructiveError::UnknownInstance(request.from_instance_id.clone()))?;
        let source_piece = find_piece(manifest, source.kit_piece.as_str())?;
        let source_socket = find_socket(source_piece, &request.from_socket_id)?;
        let target_piece = find_piece(manifest, &request.piece_id)?;
        let target_socket = find_socket(target_piece, &request.socket_id)?;
        source_socket
            .compatible_with(target_socket, &SocketCompatibility::default())
            .map_err(|error| ConstructiveError::Incompatible(error.to_string()))?;

        let placed = place_on_socket(request, source, source_socket, target_socket)?;
        bindings.push(SocketBinding {
            from_instance: source.id.clone(),
            from_socket: request.from_socket_id.clone(),
            to_instance: placed.id.clone(),
            to_socket: request.socket_id.clone(),
        });
        instances.push(placed);
    }

    Ok(SpatialPlan {
        logical,
        instances,
        bindings,
        stream_cells: Vec::new(),
    })
}

fn instance_from_root(
    root: &RootPlacement,
    piece: &PieceSpec,
) -> Result<KitInstance, ConstructiveError> {
    Ok(KitInstance {
        id: StableId::parse(root.instance_id.clone())?,
        kit_piece: StableId::parse(piece.id.clone())?,
        zone: StableId::parse(root.zone_id.clone())?,
        translation_m: root.translation_m,
        yaw_deg: normalize_yaw(root.yaw_deg),
    })
}

fn place_on_socket(
    request: &AttachRequest,
    source: &KitInstance,
    source_socket: &SocketSpec,
    target_socket: &SocketSpec,
) -> Result<KitInstance, ConstructiveError> {
    ensure_planar(source.kit_piece.as_str(), source_socket)?;
    ensure_planar(&request.piece_id, target_socket)?;

    let source_forward = rotate_y(source_socket.frame.forward, source.yaw_deg);
    let desired_target_forward = [-source_forward[0], 0.0, -source_forward[2]];
    let yaw_deg = normalize_yaw(
        planar_angle_deg(desired_target_forward) - planar_angle_deg(target_socket.frame.forward),
    );
    let source_origin = add(
        source.translation_m,
        rotate_y(source_socket.frame.origin_m, source.yaw_deg),
    );
    let target_offset = rotate_y(target_socket.frame.origin_m, yaw_deg);

    Ok(KitInstance {
        id: StableId::parse(request.instance_id.clone())?,
        kit_piece: StableId::parse(request.piece_id.clone())?,
        zone: StableId::parse(request.zone_id.clone())?,
        translation_m: subtract(source_origin, target_offset),
        yaw_deg,
    })
}

fn find_piece<'a>(manifest: &'a KitManifest, id: &str) -> Result<&'a PieceSpec, ConstructiveError> {
    manifest
        .pieces
        .iter()
        .find(|piece| piece.id == id)
        .ok_or_else(|| ConstructiveError::UnknownPiece(id.to_string()))
}

fn find_socket<'a>(piece: &'a PieceSpec, id: &str) -> Result<&'a SocketSpec, ConstructiveError> {
    piece
        .sockets
        .iter()
        .find(|socket| socket.id == id)
        .ok_or_else(|| ConstructiveError::UnknownSocket {
            piece: piece.id.clone(),
            socket: id.to_string(),
        })
}

fn validate_zone(logical: &LogicalPlan, id: &str) -> Result<(), ConstructiveError> {
    logical
        .zones
        .iter()
        .any(|zone| zone.id.as_str() == id)
        .then_some(())
        .ok_or_else(|| ConstructiveError::UnknownZone(id.to_string()))
}

fn ensure_planar(piece: &str, socket: &SocketSpec) -> Result<(), ConstructiveError> {
    let forward = socket.frame.forward;
    let up = socket.frame.up;
    let planar_forward = forward[0].hypot(forward[2]);
    if planar_forward <= 1e-5 || forward[1].abs() > 1e-4 || up[1] <= 0.0 {
        return Err(ConstructiveError::NonPlanarSocket {
            piece: piece.to_string(),
            socket: socket.id.clone(),
        });
    }
    Ok(())
}

pub(crate) fn rotate_y(vector: [f32; 3], yaw_deg: f32) -> [f32; 3] {
    let (sin, cos) = yaw_deg.to_radians().sin_cos();
    [
        cos * vector[0] + sin * vector[2],
        vector[1],
        -sin * vector[0] + cos * vector[2],
    ]
}

fn planar_angle_deg(vector: [f32; 3]) -> f32 {
    vector[0].atan2(vector[2]).to_degrees()
}

fn normalize_yaw(yaw_deg: f32) -> f32 {
    yaw_deg.rem_euclid(360.0)
}

fn add(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] + right[0], left[1] + right[1], left[2] + right[2]]
}

fn subtract(left: [f32; 3], right: [f32; 3]) -> [f32; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile_logical_plan, ContentSpec};

    const SPEC: &str = r#"
schema_version = "forgia.content-spec/v1"
id = "forgia.fixture.assembly@1.0.0"
kind = "structure"
[generation]
seed = 7
[[zones]]
id = "great_hall"
kind = "space.hall"
"#;

    const KIT: &str = r#"
schema_version = "forgia.kit-manifest/v1"
id = "forgia.fixture.stone@1.0.0"
[[pieces]]
id = "wall.straight"
asset = "render/wall.glb"
[[pieces.sockets]]
id = "door_front"
family = "portal.door"
role = "portal"
gender = "female"
frame = { origin_m = [0.0, 1.5, -0.25], forward = [0.0, 0.0, -1.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "rect", width_m = 1.6, height_m = 3.0, depth_m = 0.5 }
accepts = [{ family = "portal.door", gender = "male" }]
[[pieces]]
id = "door.oak"
asset = "render/door.glb"
[[pieces.sockets]]
id = "frame_back"
family = "portal.door"
role = "portal"
gender = "male"
frame = { origin_m = [0.0, 1.5, 0.25], forward = [0.0, 0.0, 1.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "rect", width_m = 1.6, height_m = 3.0, depth_m = 0.5 }
accepts = [{ family = "portal.door", gender = "female" }]
"#;

    fn logical() -> LogicalPlan {
        let spec = ContentSpec::parse_toml(SPEC).expect("valid spec");
        compile_logical_plan(&spec, "test").expect("logical plan")
    }

    #[test]
    fn aligns_a_piece_to_an_existing_socket() {
        let manifest = KitManifest::parse_toml(KIT).expect("valid kit");
        let plan = assemble_socket_chain(
            logical(),
            &manifest,
            RootPlacement {
                instance_id: "instance.wall".into(),
                piece_id: "wall.straight".into(),
                zone_id: "great_hall".into(),
                translation_m: [10.0, 0.0, 5.0],
                yaw_deg: 0.0,
            },
            &[AttachRequest {
                instance_id: "instance.door".into(),
                zone_id: "great_hall".into(),
                from_instance_id: "instance.wall".into(),
                from_socket_id: "door_front".into(),
                piece_id: "door.oak".into(),
                socket_id: "frame_back".into(),
            }],
        )
        .expect("assembly succeeds");
        assert_eq!(plan.bindings.len(), 1);
        assert_eq!(plan.instances[1].yaw_deg, 0.0);
        assert_eq!(plan.instances[1].translation_m, [10.0, 0.0, 4.5]);
    }

    #[test]
    fn rejects_an_unknown_source_instance() {
        let manifest = KitManifest::parse_toml(KIT).expect("valid kit");
        let error = assemble_socket_chain(
            logical(),
            &manifest,
            RootPlacement {
                instance_id: "instance.wall".into(),
                piece_id: "wall.straight".into(),
                zone_id: "great_hall".into(),
                translation_m: [0.0; 3],
                yaw_deg: 0.0,
            },
            &[AttachRequest {
                instance_id: "instance.door".into(),
                zone_id: "great_hall".into(),
                from_instance_id: "instance.missing".into(),
                from_socket_id: "door_front".into(),
                piece_id: "door.oak".into(),
                socket_id: "frame_back".into(),
            }],
        )
        .expect_err("missing instance rejected");
        assert_eq!(
            error,
            ConstructiveError::UnknownInstance("instance.missing".into())
        );
    }
}
