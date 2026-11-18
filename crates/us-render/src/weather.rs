//! What the weather looks like, as the renderer needs it: how much snow lies, how wet the
//! ground is, how dark the sky is, a lightning flash, the wind, and where flood water stands.
//! The host works these out from the simulation and sets them on the scene each frame. A scene
//! without weather keeps the plain calendar look (snow on the ground all winter).

use crate::instances::{Instance, MARKER, tint};
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb};

/// Colour of flood water.
pub const WATER: Rgb = [78, 134, 190];
/// Outline ids reserved for flood water tiles (they draw without ink).
pub const WATER_ID_BASE: u32 = 10_900_000;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeatherLook {
    /// The host drives the look. When false the calendar decides (snow all winter, no wet).
    pub active: bool,
    /// Snow lying on ground and roofs, 0 to 1.
    pub snow: f32,
    /// Wet ground and roads, 0 (dry) to 1 (soaked).
    pub wet: f32,
    /// Active rainfall intensity, independent of how much water remains on the roads.
    pub rain: f32,
    /// Active snowfall intensity.
    pub snowfall: f32,
    /// Cloud cover: darker light and no hard shadows, 0 to 1.
    pub overcast: f32,
    /// Ground mist density from 0 (clear) to 1 (dense banks).
    pub ground_fog: f32,
    /// A lightning flash right now, 0 to 1.
    pub flash: f32,
    /// Wind in metres per second (east, north); trees lean downwind.
    pub wind: [f32; 2],
    /// Flood water: (x, y, size in metres) of each square tile.
    pub flood: Vec<[f32; 3]>,
}

/// A flat square of water, 1 m a side, in the marker colour.
pub fn water_tile_mesh() -> Mesh {
    let mut o = Obj::new();
    let h = 0.5;
    o.poly(
        &[[-h, -h, 0.0], [h, -h, 0.0], [h, h, 0.0], [-h, h, 0.0]],
        MARKER,
        false,
        false,
        true,
    );
    let mut m = Mesh::default();
    let mut id = 0u32;
    o.emit(&mut m, &mut id, 0.0, 0.0, 0.0, 1.0);
    m
}

impl WeatherLook {
    /// One water tile instance per flood patch, lying just above the ground.
    pub fn water_instances(&self, height: impl Fn(f32, f32) -> f32) -> Vec<Instance> {
        self.flood
            .iter()
            .enumerate()
            .map(|(k, &[x, y, size])| Instance {
                pos: [x, y, height(x, y) + 0.22],
                theta: 0.0,
                scale: size,
                tint: tint(WATER),
                id: WATER_ID_BASE + k as u32,
            })
            .collect()
    }
}
