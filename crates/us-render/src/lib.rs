//! us-render: draws a generated Urban Sprawl town in 3D in a cel look.
//!
//! Flat colour per face in three stepped tones, ink outlines on every building edge, hard
//! shadows from one sun. Nothing soft: no fog, glow, bloom or haze. The crate has no UI or
//! windowing code; it renders offscreen to RGBA, or into a host surface through
//! [`SurfaceTarget`].
//!
//! Headless renders use whatever adapter wgpu finds; on a machine without a GPU that is the
//! Mesa software Vulkan driver.

mod calendar;
mod camera;
pub mod civic;
pub mod ground;
pub mod instances;
pub mod kit;
mod kit_misc;
pub mod land;
pub mod levels;
pub mod live;
mod mesh;
pub mod obj;
pub mod overlay;
pub mod parts;
mod renderer;
mod rng;
mod scene;

pub use calendar::{Calendar, DayPhase, Season};
pub use camera::{Camera, View, pick_ground, pick_terrain, project, ray_ground};
pub use civic::{ALL_CIVIC, CivicModel, build_civic};
pub use land::Land;
pub use levels::{MAX_LEVEL, build_lot_level, build_lot_level_for, build_lot_level_in};
pub use mesh::{ID_GROUND, ID_NONE, Mesh, VERTEX_BYTES, Vertex};
pub use obj::{Obj, Poly, Rgb, V3};
pub use overlay::{LotInfo, Overlay, OverlayKind, use_category};
pub use parts::Region;
pub use renderer::{AdapterSummary, Frame, FrameStats, NoAdapter, Renderer, Style, SurfaceTarget};
pub use rng::Rng;
pub use scene::{Chunk, FrameInstances, GRASS, LotState, RoadInfo, Scene};
