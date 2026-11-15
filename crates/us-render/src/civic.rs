//! Civic landmarks with models of their own: police station, clinic, water tower variants,
//! library, stadium, hospital, high school, transit stop, university and power plant. Same conventions as the kit: local metres, front door facing south,
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
    Hospital,
    HighSchool,
    Transit,
    University,
    PowerPlant,
}

pub const ALL_CIVIC: [CivicModel; 11] = [
    CivicModel::Police,
    CivicModel::Clinic,
    CivicModel::WaterTower,
    CivicModel::Standpipe,
    CivicModel::Library,
    CivicModel::Stadium,
    CivicModel::Hospital,
    CivicModel::HighSchool,
    CivicModel::Transit,
    CivicModel::University,
    CivicModel::PowerPlant,
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
            CivicModel::Hospital => "hospital",
            CivicModel::HighSchool => "high school",
            CivicModel::Transit => "transit stop",
            CivicModel::University => "university",
            CivicModel::PowerPlant => "power plant",
        }
    }
}

pub fn build_civic(model: CivicModel, rng: &mut Rng) -> Obj {
    match model {
        CivicModel::Police => police_station(rng),
        CivicModel::Clinic => clinic(rng),
        CivicModel::WaterTower => crate::kit::water_tower(crate::kit::Region::Mixed, rng),
        CivicModel::Standpipe => standpipe(rng),
        CivicModel::Library => library(rng),
        CivicModel::Stadium => stadium(rng),
        CivicModel::Hospital => hospital(rng),
        CivicModel::HighSchool => high_school(rng),
        CivicModel::Transit => transit_stop(rng),
        CivicModel::University => university(rng),
        CivicModel::PowerPlant => power_plant(rng),
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

/// A three storey white hospital with a blue band, two lower wings, a red emergency canopy and
/// a helipad on the roof.
pub fn hospital(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let wall = [238, 240, 242];
    let roof = [150, 160, 172];
    let blue = [60, 120, 190];
    let red = [208, 52, 52];
    // main block
    o.bx_top(-8.0, -5.0, 0.0, 8.0, 6.0, 10.5, wall, roof);
    o.windows_box(-8.0, -5.0, 8.0, 6.0, 3, 3.5, GLASS, 1.6, 3.0);
    o.bx(-8.1, -5.1, 6.9, 8.1, 6.1, 7.4, blue);
    // wings
    for sx in [-1.0f32, 1.0] {
        let (a, b) = if sx < 0.0 { (-14.0, -8.0) } else { (8.0, 14.0) };
        o.bx_top(a, -4.0, 0.0, b, 5.0, 7.0, wall, roof);
        o.windows_box(a, -4.0, b, 5.0, 2, 3.5, GLASS, 1.4, 2.8);
    }
    // glass entrance with a red canopy
    o.bx_top(-3.0, -7.6, 0.0, 3.0, -5.0, 3.6, [196, 210, 224], roof);
    o.win_s(-2.6, 2.6, 0.0, 2.8, -7.6, GLASS_D);
    o.win_s(-1.0, 1.0, 0.0, 2.6, -7.65, GLASS);
    o.bx(-4.0, -9.4, 3.6, 4.0, -5.0, 4.2, red);
    o.bx(-3.7, -9.2, 0.0, -3.4, -8.9, 3.6, [226, 228, 232]);
    o.bx(3.4, -9.2, 0.0, 3.7, -8.9, 3.6, [226, 228, 232]);
    // red cross high on the front
    o.win_s(-0.5, 0.5, 7.8, 10.0, -5.0, red);
    o.win_s(-1.3, 1.3, 8.6, 9.2, -5.04, red);
    // helipad
    o.prism(
        3.5,
        0.5,
        10.5,
        10.7,
        3.2,
        [70, 76, 94],
        12,
        Some([70, 76, 94]),
    );
    o.win_s(2.6, 4.4, 10.5, 10.7, -2.8, WHITE);
    o.slab(2.9, -0.3, 10.7, 4.1, 1.3, 10.75, WHITE);
    // rooftop plant
    o.bx(-6.0, 0.0, 10.5, -2.5, 4.0, 12.0, [170, 172, 178]);
    // ambulance bay
    o.slab(-14.0, -12.0, 0.0, -6.0, -6.0, 0.1, [96, 98, 108]);
    o
}

/// A long brick high school: a centre block with a pediment and clock tower, two classroom
/// wings, a flagpole and a running track behind.
pub fn high_school(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let brick = [176, 98, 76];
    let roof = [84, 90, 104];
    let stone = [226, 220, 206];
    // wings
    o.bx_top(-14.0, -4.0, 0.0, -5.0, 4.0, 7.0, brick, roof);
    o.bx_top(5.0, -4.0, 0.0, 14.0, 4.0, 7.0, brick, roof);
    o.windows_box(-14.0, -4.0, -5.0, 4.0, 2, 3.5, GLASS, 1.4, 2.8);
    o.windows_box(5.0, -4.0, 14.0, 4.0, 2, 3.5, GLASS, 1.4, 2.8);
    // centre block
    o.bx_top(-5.0, -5.0, 0.0, 5.0, 5.0, 8.5, brick, roof);
    o.gable(-5.0, -6.0, 5.0, 5.0, 8.5, 3.0, roof, stone, true, 0.3);
    o.bx(-3.4, -6.4, 0.0, -2.6, -5.0, 8.0, stone);
    o.bx(2.6, -6.4, 0.0, 3.4, -5.0, 8.0, stone);
    o.win_s(-1.2, 1.2, 0.0, 3.2, -6.4, DOOR);
    o.win_s(-2.0, 2.0, 4.2, 6.6, -6.4, GLASS);
    o.slab(-3.0, -8.0, 0.0, 3.0, -6.4, 0.4, [190, 186, 178]);
    // clock tower
    o.bx_top(-1.4, 0.0, 8.5, 1.4, 2.8, 13.0, stone, roof);
    o.win_s(-0.7, 0.7, 10.0, 12.0, 0.0, [250, 246, 226]);
    o.pyramid(0.0, 1.4, 13.0, 2.2, 3.2, [70, 96, 110], 4, 45.0);
    // flagpole
    o.bx(8.0, -9.0, 0.0, 8.3, -8.7, 11.0, [200, 202, 208]);
    o.bx(8.3, -8.95, 9.2, 10.6, -8.75, 10.8, [190, 60, 56]);
    // running track behind
    o.slab(-12.0, 6.0, 0.0, 12.0, 15.0, 0.12, [176, 96, 70]);
    o.slab(-9.5, 7.6, 0.12, 9.5, 13.4, 0.16, [88, 156, 74]);
    o
}

/// A small bus stop: a glass shelter under a canopy, a bench, a pole with a blue sign and a
/// painted bay at the kerb.
pub fn transit_stop(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let steel = [96, 100, 112];
    let blue = [40, 90, 170];
    // pad and bay
    o.slab(-6.0, -4.0, 0.0, 6.0, 4.0, 0.12, [176, 176, 170]);
    o.slab(-6.0, -6.0, 0.0, 6.0, -4.0, 0.08, [70, 72, 80]);
    o.slab(-5.5, -5.1, 0.08, 5.5, -4.9, 0.1, WHITE);
    // shelter: posts, glass back, roof
    for x in [-3.2f32, 3.2] {
        o.bx(x - 0.15, 1.5, 0.12, x + 0.15, 1.8, 3.0, steel);
        o.bx(x - 0.15, -1.2, 0.12, x + 0.15, -0.9, 3.0, steel);
    }
    o.bx_top(-3.8, -1.6, 3.0, 3.8, 2.2, 3.4, blue, [60, 110, 190]);
    o.win_s(-3.0, 3.0, 0.4, 2.7, 1.9, GLASS);
    o.win_w(0.0, 1.5, 0.4, 2.7, -3.2, GLASS_D);
    // bench
    o.bx(-2.0, 0.6, 0.12, 2.0, 1.3, 0.55, [150, 104, 64]);
    // sign pole with a round blue sign
    o.bx(4.6, -3.2, 0.12, 4.8, -3.0, 4.4, steel);
    o.prism(4.7, -3.1, 4.4, 4.7, 0.9, blue, 10, Some(blue));
    o
}

/// A university: a stone main hall with a dome and a columned front, two long brick wings,
/// and a lawn with paths between them.
pub fn university(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let brick = [168, 96, 76];
    let stone = [228, 222, 208];
    let roof = [84, 90, 104];
    let green = [88, 156, 74];
    // lawn and paths
    o.slab(-24.0, -20.0, 0.0, 24.0, 8.0, 0.1, green);
    o.slab(-1.4, -20.0, 0.1, 1.4, -4.0, 0.14, [214, 206, 184]);
    o.slab(-22.0, -9.0, 0.1, 22.0, -7.4, 0.14, [214, 206, 184]);
    // main hall
    o.bx_top(-9.0, -4.0, 0.0, 9.0, 8.0, 11.0, stone, roof);
    o.windows_box(-9.0, -4.0, 9.0, 8.0, 3, 3.6, GLASS, 1.6, 3.2);
    o.gable(-5.0, -5.0, 5.0, 8.0, 11.0, 3.6, roof, stone, true, 0.4);
    for x in [-4.2f32, -1.4, 1.4, 4.2] {
        o.bx(x - 0.45, -5.6, 0.0, x + 0.45, -4.6, 10.6, [240, 236, 226]);
    }
    o.win_s(-1.2, 1.2, 0.0, 3.6, -4.0, DOOR);
    o.slab(-5.5, -7.0, 0.0, 5.5, -5.6, 0.5, [190, 186, 178]);
    // dome
    o.prism(0.0, 2.0, 11.0, 14.0, 3.6, stone, 12, Some(stone));
    o.pyramid(0.0, 2.0, 14.0, 3.8, 3.4, [70, 120, 112], 12, 0.0);
    o.bx(-0.15, 1.85, 17.4, 0.15, 2.15, 19.4, [200, 202, 208]);
    // wings
    for sx in [-1.0f32, 1.0] {
        let (a, b) = if sx < 0.0 {
            (-24.0, -11.0)
        } else {
            (11.0, 24.0)
        };
        o.bx_top(a, -6.0, 0.0, b, 4.0, 8.0, brick, roof);
        o.windows_box(a, -6.0, b, 4.0, 2, 4.0, GLASS, 1.5, 3.0);
        o.bx(a, -6.2, 7.2, b, -6.0, 7.6, stone);
        o.win_s(
            (a + b) / 2.0 - 1.0,
            (a + b) / 2.0 + 1.0,
            0.0,
            3.0,
            -6.0,
            DOOR,
        );
    }
    o
}

/// A coal-look power plant: a boiler house with a tall banded stack, two cooling towers, a
/// turbine hall, a coal pile and a fenced yard of transformers.
pub fn power_plant(_rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let wall = [168, 170, 176];
    let dark = [96, 100, 112];
    let conc = [196, 196, 190];
    o.slab(-15.0, -14.0, 0.0, 15.0, 14.0, 0.12, [140, 142, 138]);
    // turbine hall (long, low) and boiler house (tall)
    o.bx_top(-13.0, -4.0, 0.0, 2.0, 5.0, 7.0, wall, dark);
    o.windows_box(-13.0, -4.0, 2.0, 5.0, 1, 7.0, GLASS_D, 1.6, 3.2);
    o.bx_top(2.0, -5.0, 0.0, 10.0, 6.0, 13.0, [150, 154, 164], dark);
    o.windows_box(2.0, -5.0, 10.0, 6.0, 3, 4.3, GLASS_D, 1.0, 3.0);
    // stack with red and white bands
    for (i, c) in [
        [200, 56, 52],
        [236, 236, 232],
        [200, 56, 52],
        [236, 236, 232],
    ]
    .into_iter()
    .enumerate()
    {
        let z = 13.0 + i as f32 * 5.0;
        o.prism(6.0, 0.5, z, z + 5.0, 1.8 - i as f32 * 0.12, c, 12, Some(c));
    }
    // two cooling towers
    for (cx, cy) in [(-8.0f32, 9.0f32), (-1.5, 10.0)] {
        o.prism(cx, cy, 0.0, 9.0, 3.6, conc, 14, None);
        o.prism(cx, cy, 9.0, 13.0, 2.9, conc, 14, Some([60, 64, 72]));
    }
    // coal pile and conveyor
    o.pyramid(-9.0, -9.0, 0.12, 4.2, 3.6, [44, 44, 50], 7, 10.0);
    o.slab(-6.0, -9.4, 3.0, 1.5, -8.6, 3.3, dark);
    // transformers inside a low fence
    for (x, y) in [(11.0f32, -10.0f32), (13.0, -10.0), (11.0, -7.6)] {
        o.bx(
            x - 0.8,
            y - 0.8,
            0.12,
            x + 0.8,
            y + 0.8,
            2.2,
            [110, 130, 120],
        );
    }
    o.bx(10.0, -11.5, 0.12, 14.4, -11.3, 1.2, dark);
    o.bx(10.0, -6.2, 0.12, 14.4, -6.0, 1.2, dark);
    // door
    o.win_s(-9.0, -7.6, 0.12, 3.0, -4.0, DOOR);
    o
}
