//! us-render: draws a generated Urban Sprawl town in 3D in a cel look.
//!
//! Flat colour per face in three stepped tones, ink outlines on every building edge, hard
//! shadows from one sun. Nothing soft: no fog, glow, bloom or haze. The crate has no UI or
//! windowing code; it renders offscreen to RGBA, or into a host surface through
//! [`SurfaceTarget`].
//!
//! Headless renders use whatever adapter wgpu finds; on a machine without a GPU that is the
//! Mesa software Vulkan driver.

mod camera;
mod ground;
pub mod kit;
mod mesh;
pub mod obj;
mod renderer;
mod rng;
mod scene;

pub use camera::{Camera, View, pick_ground, project};
pub use mesh::{ID_GROUND, ID_NONE, Mesh, VERTEX_BYTES, Vertex};
pub use obj::{Obj, Poly, Rgb, V3};
pub use renderer::{AdapterSummary, Frame, FrameStats, NoAdapter, Renderer, Style, SurfaceTarget};
pub use rng::Rng;
pub use scene::{Chunk, Scene};
