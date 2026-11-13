//! Civic landmarks with models of their own: police station, clinic, water tower variants,
//! library and stadium. Same conventions as the kit: local metres, front door facing south,
//! centred on the origin, flat colour per face.

use crate::kit::{DOOR, GLASS, GLASS_D, WHITE};
use crate::obj::Obj;
use crate::rng::Rng;

/// A civic model the host can place for a civic building.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CivicModel {
    Police,
    Clinic,
    /// The classic tank on four legs.
    WaterTower,
    /// A tall round standpipe with a domed top.
    Standpipe,
    Library,
    Stadium,
}

pub const ALL_CIVIC: [CivicModel; 6] = [
    CivicModel::Police,
    CivicModel::Clinic,
    CivicModel::WaterTower,
    CivicModel::Standpipe,
    CivicModel::Library,
    CivicModel::Stadium,
];

impl CivicModel {
    pub fn name(self) -> &'static str {
        match self {
            CivicModel::Police => "police station",
            CivicModel::Clinic => "clinic",
            CivicModel::WaterTower => "water tower",
            CivicModel::Standpipe => "standpipe",
            CivicModel::Library => "library",
            CivicModel::Stadium => "stadium",
        }
    }
}

pub fn build_civic(model: CivicModel, rng: &mut Rng) -> Obj {
    match model {
        CivicModel::Police => police_station(rng),
        CivicModel::Clinic => clinic(rng),
        CivicModel::WaterTower => crate::kit::water_tower(rng),
        CivicModel::Standpipe => standpipe(rng),
        CivicModel::Library => library(rng),
        CivicModel::Stadium => stadium(rng),
    }
}

/// Two floors of blue-grey stone, a flat roof with a radio mast, a canopy over the door, a blue
/// lamp box and two patrol cars' worth of striped bays at the side.
pub fn police_station(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let stone = [116, 130, 160];
    let roof = [70, 76, 94];
    let blue = [40, 90, 170];
    o.bx_top(-10.0, -6.0, 0.0, 6.0, 6.0, 7.2, stone, roof);
    o.windows_box(-10.0, -6.0, 6.0, 6.0, 2, 3.6, GLASS, 1.8, 3.4);
    // entrance block and canopy
    o.bx_top(-3.0, -8.0, 0.0, 1.0, -6.0, 4.2, [150, 160, 182], roof);
    o.win_s(-1.8, -0.2, 0.0, 2.8, -8.0, DOOR);
    o.bx(-3.6, -9.6, 3.2, 1.6, -6.0, 3.6, [60, 64, 78]);
    // blue lamp box above the door
    o.bx(-1.9, -8.3, 4.2, -0.1, -7.7, 5.6, blue);
    o.win_s(-1.7, -0.3, 4.4, 5.4, -8.3, [236, 240, 250]);
    // mast
    o.bx(3.8, 3.8, 7.2, 4.2, 4.2, 15.0, [150, 150, 156]);
    o.bx(2.4, 3.2, 7.2, 3.4, 4.8, 8.2, [190, 192, 198]);
    o.bx(4.8, 3.4, 7.2, 5.4, 4.4, 9.6, [190, 192, 198]);
    // car bays
    o.slab(6.4, -6.0, 0.0, 14.0, 6.0, 0.1, [96, 98, 108]);
    for y in [-3.0f32, 0.0, 3.0] {
        o.slab(6.4, y - 0.1, 0.1, 14.0, y + 0.1, 0.14, WHITE);
    }
    o
}

/// A white two storey block with a red cross on the front, a glass entrance and a flat roof.
pub fn clinic(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let wall = [236, 238, 240];
    let roof = [150, 160, 172];
    let red = [208, 52, 52];
    o.bx_top(-9.0, -5.0, 0.0, 9.0, 6.0, 7.0, wall, roof);
    o.windows_box(-9.0, -5.0, 9.0, 6.0, 2, 3.5, GLASS, 1.6, 3.2);
    // glass entrance
    o.bx_top(-3.0, -7.4, 0.0, 3.0, -5.0, 3.6, [196, 210, 224], roof);
    o.win_s(-2.6, 2.6, 0.0, 2.8, -7.4, GLASS_D);
    o.win_s(-1.0, 1.0, 0.0, 2.6, -7.45, GLASS);
    // red cross on the front above the entrance
    o.win_s(-0.4, 0.4, 4.2, 6.4, -5.0, red);
    o.win_s(-1.1, 1.1, 4.9, 5.7, -5.04, red);
    // ambulance bay
    o.bx(9.0, -3.0, 0.0, 12.0, 3.0, 3.8, [226, 228, 232]);
    o.win_s(9.6, 11.4, 0.0, 3.0, -3.0, [200, 60, 60]);
    // rooftop plant
    o.bx(-5.0, 0.0, 7.0, -1.5, 3.5, 8.6, [170, 172, 178]);
    o
}

/// A tall round standpipe: slim shaft, wider top, domed roof, a ladder and a blue band.
pub fn standpipe(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    o.slab(-4.5, -4.5, 0.0, 4.5, 4.5, 0.2, [150, 152, 150]);
    o.prism(0.0, 0.0, 0.2, 20.0, 2.2, [204, 208, 214], 12, None);
    o.prism(0.0, 0.0, 20.0, 24.0, 3.4, [204, 208, 214], 12, None);
    o.pyramid(0.0, 0.0, 24.0, 3.5, 2.6, [150, 160, 172], 12, 0.0);
    // blue band with the town's mark and a ladder
    o.poly(
        &[
            [-2.0, -3.42, 20.6],
            [2.0, -3.42, 20.6],
            [2.0, -3.42, 23.0],
            [-2.0, -3.42, 23.0],
        ],
        [40, 90, 170],
        false,
        true,
        true,
    );
    o.bx(1.6, -2.45, 0.2, 1.9, -2.2, 20.0, [96, 98, 104]);
    o
}

/// A brick library: two floors, a pitched centre with a pediment and tall arched-look windows,
/// steps up to the door.
pub fn library(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let brick = [182, 104, 80];
    let roof = [86, 92, 106];
    let stone = [226, 220, 206];
    o.bx_top(-12.0, -5.0, 0.0, 12.0, 8.0, 7.0, brick, roof);
    o.gable(-5.0, -6.0, 5.0, 8.0, 7.0, 3.4, roof, stone, true, 0.3);
    o.bx(-5.0, -6.0, 0.0, 5.0, -5.0, 7.0, stone);
    for x in [-4.0f32, 4.0] {
        o.bx(x - 0.4, -7.0, 0.0, x + 0.4, -6.0, 6.6, stone);
    }
    o.win_s(-1.0, 1.0, 0.0, 3.4, -7.0, DOOR);
    o.win_s(-2.2, 2.2, 4.0, 6.0, -7.0, GLASS);
    for x in [-10.0f32, -7.4, 7.4, 10.0] {
        o.win_s(x - 0.7, x + 0.7, 1.6, 5.6, -5.0, GLASS);
    }
    // steps
    o.slab(-3.0, -9.0, 0.0, 3.0, -7.0, 0.3, [190, 186, 178]);
    o.slab(-2.4, -8.0, 0.3, 2.4, -7.0, 0.6, [196, 192, 184]);
    o
}

/// An oval-ish stadium: pitch, four stands that step up, floodlight masts at the corners.
pub fn stadium(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (hw, hd) = (30.0f32, 20.0f32);
    o.slab(
        -hw - 4.0,
        -hd - 4.0,
        0.0,
        hw + 4.0,
        hd + 4.0,
        0.15,
        [150, 152, 150],
    );
    o.slab(
        -hw + 2.0,
        -hd + 2.0,
        0.15,
        hw - 2.0,
        hd - 2.0,
        0.2,
        [88, 156, 74],
    );
    for i in 0..6 {
        let x = -hw + 5.0 + i as f32 * 10.0;
        o.slab(x, -hd + 2.0, 0.2, x + 5.0, hd - 2.0, 0.24, [100, 166, 84]);
    }
    o.slab(-0.2, -hd + 2.0, 0.24, 0.2, hd - 2.0, 0.28, WHITE);
    let seat = [196, 70, 60];
    let seat2 = [60, 90, 150];
    let conc = [186, 182, 174];
    // long stands (north and south), three steps each
    for (k, h) in [3.0f32, 5.0, 7.0].into_iter().enumerate() {
        let off = hd - 1.0 + k as f32 * 2.5;
        let col = if k % 2 == 0 { seat } else { seat2 };
        o.bx_top(-hw, off, 0.0, hw, off + 2.5, h, conc, col);
        o.bx_top(-hw, -off - 2.5, 0.0, hw, -off, h, conc, col);
    }
    // short stands (east and west)
    for (k, h) in [2.6f32, 4.2].into_iter().enumerate() {
        let off = hw + 0.2 + k as f32 * 2.5;
        let col = if k % 2 == 0 { seat2 } else { seat };
        o.bx_top(off, -hd + 1.0, 0.0, off + 2.5, hd - 1.0, h, conc, col);
        o.bx_top(-off - 2.5, -hd + 1.0, 0.0, -off, hd - 1.0, h, conc, col);
    }
    // floodlight masts
    for (x, y) in [
        (-hw - 2.0, -hd - 2.0),
        (hw + 2.0, -hd - 2.0),
        (-hw - 2.0, hd + 2.0),
        (hw + 2.0, hd + 2.0),
    ] {
        o.bx(
            x - 0.3,
            y - 0.3,
            0.0,
            x + 0.3,
            y + 0.3,
            17.0,
            [120, 124, 130],
        );
        o.bx(
            x - 1.6,
            y - 0.5,
            17.0,
            x + 1.6,
            y + 0.5,
            18.4,
            [255, 236, 150],
        );
    }
    // gate
    o.bx(-3.0, -hd - 3.8, 0.0, 3.0, -hd - 1.0, 4.0, [226, 220, 206]);
    o.win_s(-1.2, 1.2, 0.0, 2.8, -hd - 3.8, DOOR);
    o
}
