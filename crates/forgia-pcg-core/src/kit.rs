//! Kit manifests and formal socket compatibility.

use crate::{PcgError, StableId};
use serde::{Deserialize, Serialize};

pub const KIT_MANIFEST_SCHEMA_V1: &str = "forgia.kit-manifest/v1";
const DEFAULT_POSITION_TOLERANCE_M: f32 = 0.001;
const DEFAULT_NORMAL_DOT_MAX: f32 = -0.999;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum KitError {
    #[error("kit-manifest TOML invalid: {0}")]
    Parse(String),
    #[error("unsupported kit-manifest schema `{found}`; expected `{KIT_MANIFEST_SCHEMA_V1}`")]
    UnsupportedSchema { found: String },
    #[error("invalid kit id: {0}")]
    InvalidId(#[from] PcgError),
    #[error("piece `{0}` is duplicated")]
    DuplicatePiece(String),
    #[error("socket `{socket}` on piece `{piece}` is invalid: {reason}")]
    InvalidSocket {
        piece: String,
        socket: String,
        reason: &'static str,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct KitManifest {
    pub schema_version: String,
    pub id: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub extends: Vec<String>,
    #[serde(default)]
    pub pieces: Vec<PieceSpec>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct PieceSpec {
    pub id: String,
    pub asset: String,
    #[serde(default)]
    pub provides: Vec<String>,
    /// Local solid AABB in metres (Y-up Forgia). Optional so legacy fixtures and
    /// the socket-only unit tests keep parsing; the hard validators only enforce
    /// world bounds/clearance for pieces that declare it.
    #[serde(default)]
    pub bounds_m: Option<Bounds>,
    /// Physics proxy descriptor. Its mere presence marks the piece as a collider
    /// for the `max_collision_proxies` budget (the 8 052-TriMesh castle lesson).
    #[serde(default)]
    pub collision: Option<CollisionProxy>,
    #[serde(default)]
    pub lods: Vec<Lod>,
    #[serde(default)]
    pub sockets: Vec<SocketSpec>,
}

/// Local axis-aligned bounding box, metres, Y-up Forgia.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// Simple collision proxy (`box` / `proxy_mesh`); never a per-mesh TriMesh at
/// runtime. `asset` is optional so an analytic box proxy needs no GLB.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct CollisionProxy {
    pub kind: String,
    #[serde(default)]
    pub asset: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Lod {
    pub asset: String,
    #[serde(default)]
    pub max_distance_m: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SocketSpec {
    pub id: String,
    pub family: String,
    pub role: SocketRole,
    pub gender: SocketGender,
    pub frame: SocketFrame,
    pub aperture: Aperture,
    /// Free volume required around the interface (capsule joueur, porte animée,
    /// roue, câble). Distinct from the solid mesh; consumed by the contextual
    /// clearance validator. Optional so socket-only fixtures keep parsing.
    #[serde(default)]
    pub clearance: Option<Clearance>,
    #[serde(default)]
    pub accepts: Vec<AcceptRule>,
}

/// Clearance volume around a socket, expressed in the piece's local frame.
/// `box` uses `half_extents_m`; `cylinder` uses `radius_m`/`length_m`.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Clearance {
    pub shape: String,
    #[serde(default)]
    pub half_extents_m: Option<[f32; 3]>,
    #[serde(default)]
    pub radius_m: Option<f32>,
    #[serde(default)]
    pub length_m: Option<f32>,
}

impl Clearance {
    /// Conservative local half-extents of the clearance volume as an AABB.
    /// A cylinder is bounded by its enclosing box (`radius` on the two lateral
    /// axes, `length/2` on the axis) — conservative is correct for a *free
    /// space* guard: over-reserving space never hides a real obstruction.
    pub fn local_half_extents(&self) -> [f32; 3] {
        match self.shape.as_str() {
            "cylinder" => {
                let r = self.radius_m.unwrap_or(0.0);
                let half_len = self.length_m.unwrap_or(0.0) * 0.5;
                // Axis unknown at this layer → bound every axis by max(r, half_len).
                let e = r.max(half_len);
                [e, e, e]
            }
            _ => self.half_extents_m.unwrap_or([0.0, 0.0, 0.0]),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SocketRole {
    Structural,
    Portal,
    Utility,
    Decor,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SocketGender {
    Male,
    Female,
    Neutral,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SocketFrame {
    pub origin_m: [f32; 3],
    pub forward: [f32; 3],
    pub up: [f32; 3],
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Aperture {
    pub shape: String,
    #[serde(default)]
    pub width_m: Option<f32>,
    #[serde(default)]
    pub height_m: Option<f32>,
    #[serde(default)]
    pub depth_m: Option<f32>,
    #[serde(default)]
    pub radius_m: Option<f32>,
    #[serde(default)]
    pub length_m: Option<f32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AcceptRule {
    pub family: String,
    #[serde(default)]
    pub gender: Option<SocketGender>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SocketCompatibility {
    pub position_tolerance_m: f32,
    pub normal_dot_max: f32,
}

impl Default for SocketCompatibility {
    fn default() -> Self {
        Self {
            position_tolerance_m: DEFAULT_POSITION_TOLERANCE_M,
            normal_dot_max: DEFAULT_NORMAL_DOT_MAX,
        }
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum SocketMismatch {
    #[error("socket families differ: `{left}` vs `{right}`")]
    Family { left: String, right: String },
    #[error("socket genders are incompatible")]
    Gender,
    #[error("one socket does not accept the other")]
    Acceptance,
    #[error("apertures are incompatible")]
    Aperture,
    #[error("socket frames are not opposed/aligned")]
    Orientation,
}

impl KitManifest {
    pub fn parse_toml(source: &str) -> Result<Self, KitError> {
        let manifest: Self =
            toml::from_str(source).map_err(|error| KitError::Parse(error.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), KitError> {
        if self.schema_version != KIT_MANIFEST_SCHEMA_V1 {
            return Err(KitError::UnsupportedSchema {
                found: self.schema_version.clone(),
            });
        }
        StableId::parse(self.id.clone()).map_err(KitError::InvalidId)?;
        let mut ids = self
            .pieces
            .iter()
            .map(|piece| piece.id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        if let Some(duplicate) = ids
            .windows(2)
            .find_map(|pair| (pair[0] == pair[1]).then_some(pair[0]))
        {
            return Err(KitError::DuplicatePiece(duplicate.to_string()));
        }
        for piece in &self.pieces {
            StableId::parse(piece.id.clone()).map_err(KitError::InvalidId)?;
            for socket in &piece.sockets {
                validate_socket(&piece.id, socket)?;
            }
        }
        Ok(())
    }
}

fn validate_socket(piece: &str, socket: &SocketSpec) -> Result<(), KitError> {
    let finite = socket
        .frame
        .origin_m
        .iter()
        .chain(socket.frame.forward.iter())
        .chain(socket.frame.up.iter())
        .all(|value| value.is_finite());
    if !finite || vector_len(socket.frame.forward) <= 0.0 || vector_len(socket.frame.up) <= 0.0 {
        return Err(KitError::InvalidSocket {
            piece: piece.to_string(),
            socket: socket.id.clone(),
            reason: "frame must contain finite non-zero forward and up vectors",
        });
    }
    if socket.family.is_empty() || socket.id.is_empty() || socket.aperture.shape.is_empty() {
        return Err(KitError::InvalidSocket {
            piece: piece.to_string(),
            socket: socket.id.clone(),
            reason: "id, family and aperture shape are required",
        });
    }
    Ok(())
}

impl SocketSpec {
    /// Checks semantic and geometric interface compatibility. Placement is
    /// solved separately; normals must be opposite, while both up vectors must
    /// point in the same direction.
    pub fn compatible_with(
        &self,
        other: &Self,
        policy: &SocketCompatibility,
    ) -> Result<(), SocketMismatch> {
        if self.family != other.family {
            return Err(SocketMismatch::Family {
                left: self.family.clone(),
                right: other.family.clone(),
            });
        }
        if !genders_compatible(self.gender, other.gender) {
            return Err(SocketMismatch::Gender);
        }
        if !accepts(self, other) || !accepts(other, self) {
            return Err(SocketMismatch::Acceptance);
        }
        if !apertures_compatible(&self.aperture, &other.aperture, policy.position_tolerance_m) {
            return Err(SocketMismatch::Aperture);
        }
        let forward_dot = normalized_dot(self.frame.forward, other.frame.forward);
        let up_dot = normalized_dot(self.frame.up, other.frame.up);
        if forward_dot > policy.normal_dot_max || up_dot < -policy.normal_dot_max {
            return Err(SocketMismatch::Orientation);
        }
        Ok(())
    }
}

fn accepts(socket: &SocketSpec, other: &SocketSpec) -> bool {
    socket.accepts.iter().any(|rule| {
        rule.family == other.family && rule.gender.is_none_or(|gender| gender == other.gender)
    })
}

fn genders_compatible(left: SocketGender, right: SocketGender) -> bool {
    matches!(
        (left, right),
        (SocketGender::Male, SocketGender::Female)
            | (SocketGender::Female, SocketGender::Male)
            | (SocketGender::Neutral, SocketGender::Neutral)
    )
}

fn apertures_compatible(left: &Aperture, right: &Aperture, tolerance: f32) -> bool {
    left.shape == right.shape
        && dimensions_match(left.width_m, right.width_m, tolerance)
        && dimensions_match(left.height_m, right.height_m, tolerance)
        && dimensions_match(left.depth_m, right.depth_m, tolerance)
        && dimensions_match(left.radius_m, right.radius_m, tolerance)
        && dimensions_match(left.length_m, right.length_m, tolerance)
}

fn dimensions_match(left: Option<f32>, right: Option<f32>, tolerance: f32) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => {
            left.is_finite() && right.is_finite() && (left - right).abs() <= tolerance
        }
        (None, None) => true,
        _ => false,
    }
}

fn vector_len(vector: [f32; 3]) -> f32 {
    dot(vector, vector).sqrt()
}

fn normalized_dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    dot(left, right) / (vector_len(left) * vector_len(right))
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    const KIT: &str = r#"
schema_version = "forgia.kit-manifest/v1"
id = "forgia.castle.stone@1.0.0"
[[pieces]]
id = "wall.straight.4m"
asset = "render/wall.glb#Scene0"
[[pieces.sockets]]
id = "door_front"
family = "portal.door"
role = "portal"
gender = "female"
frame = { origin_m = [0.0, 1.5, -0.25], forward = [0.0, 0.0, -1.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "rect", width_m = 1.6, height_m = 3.0, depth_m = 0.5 }
accepts = [{ family = "portal.door", gender = "male" }]
[[pieces]]
id = "door.oak.large"
asset = "render/door.glb#Scene0"
[[pieces.sockets]]
id = "frame_back"
family = "portal.door"
role = "portal"
gender = "male"
frame = { origin_m = [0.0, 1.5, 0.25], forward = [0.0, 0.0, 1.0], up = [0.0, 1.0, 0.0] }
aperture = { shape = "rect", width_m = 1.6, height_m = 3.0, depth_m = 0.5 }
accepts = [{ family = "portal.door", gender = "female" }]
"#;

    #[test]
    fn compatible_door_sockets_match_formally() {
        let manifest = KitManifest::parse_toml(KIT).expect("valid manifest");
        let wall = &manifest.pieces[0].sockets[0];
        let door = &manifest.pieces[1].sockets[0];
        assert_eq!(
            wall.compatible_with(door, &SocketCompatibility::default()),
            Ok(())
        );
    }

    #[test]
    fn bad_aperture_is_rejected() {
        let manifest = KitManifest::parse_toml(KIT).expect("valid manifest");
        let wall = &manifest.pieces[0].sockets[0];
        let mut door = manifest.pieces[1].sockets[0].clone();
        door.aperture.width_m = Some(2.0);
        assert_eq!(
            wall.compatible_with(&door, &SocketCompatibility::default()),
            Err(SocketMismatch::Aperture)
        );
    }
}
