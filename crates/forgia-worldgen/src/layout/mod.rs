//! Layout — turn a recipe into a city structure: **roads** (tensor field) + **parcels**
//! (recursive block subdivision). Pure 2D geometry (no ECS); the engine maps it to the world.
//! Story-578 P3.

pub mod parcels;
pub mod roads;

pub use parcels::{generate_parcels, subdivide_rect, Parcel};
pub use roads::{generate_roads, RoadKind, RoadNetwork, RoadSegment, TensorField};
