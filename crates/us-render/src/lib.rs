//! us-render: draws a generated Urban Sprawl town in 3D in a cel look.
//!
//! Draws textured terrain and buildings, day and night lighting, and world-space weather.
//! The crate renders offscreen to RGBA or into a host surface through
//! [`SurfaceTarget`].
//!
//! Headless renders use whatever adapter wgpu finds; on a machine without a GPU that is the
//! Mesa software Vulkan driver.

pub mod architecture;
mod calendar;
mod camera;
pub mod civic;
mod cloud_noise;
pub mod cover;
pub mod decor;
mod detail;
pub mod emergency;
mod facade;
pub mod fleet;
pub mod ground;
pub mod harbour;
pub mod instances;
pub mod kit;
mod kit_misc;
pub mod kit_more;
pub mod land;
pub mod levels;
pub mod live;
mod local_light;
mod mesh;
pub mod obj;
pub mod overlay;
pub mod park;
pub mod parts;
pub mod rail;
mod reflection;
mod renderer;
mod rng;
mod road_traffic;
pub mod roadpaint;
pub mod rock;
mod scene;
pub mod site;
pub mod trains;
pub mod trams;
pub mod weather;

pub use calendar::{Calendar, DayPhase, Season};
pub use camera::{Camera, View, pick_ground, pick_terrain, project, ray_ground};
pub use civic::{ALL_CIVIC, CivicModel, build_civic};
pub use land::Land;
pub use levels::{
    MAX_LEVEL, build_lot_level, build_lot_level_dense, build_lot_level_for, build_lot_level_in,
};
pub use mesh::{ID_GROUND, ID_NONE, Mesh, VERTEX_BYTES, Vertex};
pub use obj::{Obj, Poly, Rgb, V3};
pub use overlay::{LotInfo, Overlay, OverlayKind, use_category};
pub use parts::Region;
pub use renderer::{AdapterSummary, Frame, FrameStats, NoAdapter, Renderer, Style, SurfaceTarget};
pub use rng::Rng;
pub use road_traffic::Approach as RoadApproach;
pub use rock::RockLook;
pub use scene::{Chunk, FrameInstances, GRASS, LotState, RoadInfo, Scene};
pub use weather::WeatherLook;
