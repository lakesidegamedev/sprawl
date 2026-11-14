//! Parametric building parts: regions, material families, roofs, porches, bays, awnings, fences,
//! yard props, rooftop gear and window patterns. Every choice is drawn from a seeded [`Rng`], so a
//! lot always looks the same. All models are in local metres with the front facing south (-y).

use crate::obj::{Obj, Rgb};
use crate::rng::Rng;
use us_gen::Preset;

pub const GLASS: Rgb = [98, 156, 204];
pub const GLASS_D: Rgb = [64, 88, 118];
pub const WHITE: Rgb = [238, 234, 222];
pub const CONC: Rgb = [176, 172, 164];
pub const TRIM: Rgb = [226, 220, 206];
pub const STONE: Rgb = [206, 200, 188];
pub const GREEN: Rgb = [88, 140, 72];
pub const WOOD: Rgb = [150, 112, 82];

/// Where a town is: it decides which materials, roofs and yard props are common.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Region {
    /// No lean: a bit of everything.
    #[default]
    Mixed,
    /// Prairie grid town: brick and gable roofs.
    Prairie,
    /// Winding river town: painted wood and porches.
    River,
    /// Sunbelt: stucco, flat and tile roofs, pools.
    Sunbelt,
    /// Old crossroads town: brick and wood, shopfronts.
    Crossroads,
    /// Lake town: cottages, metal roofs, boats.
    Lake,
}

impl Region {
    pub const ALL: [Region; 6] = [
        Region::Mixed,
        Region::Prairie,
        Region::River,
        Region::Sunbelt,
        Region::Crossroads,
        Region::Lake,
    ];

    pub fn of(p: Preset) -> Region {
        match p {
            Preset::Grid => Region::Prairie,
            Preset::Winding => Region::River,
            Preset::Sunbelt => Region::Sunbelt,
            Preset::Crossroads => Region::Crossroads,
            Preset::LakeTown => Region::Lake,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Region::Mixed => "mixed",
            Region::Prairie => "prairie",
            Region::River => "river",
            Region::Sunbelt => "sunbelt",
            Region::Crossroads => "crossroads",
            Region::Lake => "lake",
        }
    }
}

/// Pick by weight.
pub fn wpick<T: Copy>(rng: &mut Rng, items: &[(T, u32)]) -> T {
    let total: u32 = items.iter().map(|i| i.1).sum();
    let mut k = rng.below(total.max(1) as usize) as u32;
    for (v, w) in items {
        if k < *w {
            return *v;
        }
        k -= *w;
    }
    items[0].0
}

// ---- materials ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mat {
    Brick,
    Wood,
    Stucco,
    Concrete,
}

pub const BRICKS: [Rgb; 8] = [
    [170, 86, 64],
    [150, 72, 58],
    [186, 112, 78],
    [128, 92, 80],
    [196, 140, 104],
    [110, 70, 62],
    [206, 168, 128],
    [160, 100, 84],
];
pub const PAINTS: [Rgb; 14] = [
    [244, 216, 150],
    [160, 204, 228],
    [240, 180, 140],
    [186, 222, 160],
    [246, 236, 214],
    [218, 186, 226],
    [120, 150, 180],
    [96, 128, 110],
    [210, 170, 70],
    [180, 64, 56],
    [150, 160, 170],
    [70, 92, 128],
    [232, 150, 150],
    [112, 140, 96],
];
pub const STUCCOS: [Rgb; 8] = [
    [238, 214, 170],
    [232, 184, 150],
    [246, 232, 200],
    [222, 160, 130],
    [240, 226, 176],
    [200, 170, 140],
    [190, 214, 196],
    [244, 244, 236],
];
pub const CONCS: [Rgb; 5] = [
    [176, 172, 164],
    [196, 194, 188],
    [150, 150, 150],
    [208, 200, 186],
    [164, 168, 176],
];
/// Curtain wall glass families: blue, green, bronze, grey, dark.
pub const CURTAINS: [(Rgb, Rgb); 6] = [
    ([98, 156, 204], [74, 120, 170]),
    ([96, 170, 160], [66, 128, 122]),
    ([176, 150, 110], [138, 112, 80]),
    ([150, 164, 176], [112, 126, 140]),
    ([70, 84, 104], [48, 60, 80]),
    ([128, 176, 214], [96, 140, 182]),
];
pub const SHINGLES: [Rgb; 9] = [
    [196, 70, 52],
    [84, 96, 124],
    [150, 84, 52],
    [52, 120, 100],
    [176, 120, 60],
    [74, 74, 82],
    [110, 90, 76],
    [60, 70, 84],
    [128, 60, 56],
];
pub const TILES: [Rgb; 5] = [
    [196, 98, 64],
    [210, 120, 76],
    [174, 84, 60],
    [150, 76, 56],
    [220, 150, 96],
];
pub const METALS: [Rgb; 6] = [
    [150, 160, 170],
    [120, 36, 36],
    [50, 100, 80],
    [70, 84, 110],
    [190, 196, 200],
    [60, 64, 70],
];
pub const DOORS: [Rgb; 8] = [
    [96, 62, 44],
    [170, 48, 44],
    [48, 84, 150],
    [52, 110, 80],
    [230, 190, 60],
    [236, 232, 222],
    [44, 46, 54],
    [40, 128, 130],
];
pub const SHUTTERS: [Rgb; 6] = [
    [52, 84, 120],
    [56, 100, 70],
    [40, 44, 52],
    [150, 48, 44],
    [236, 232, 222],
    [110, 76, 52],
];
pub const AWNINGS: [(Rgb, Rgb); 8] = [
    ([196, 58, 52], WHITE),
    ([40, 92, 160], WHITE),
    ([40, 130, 90], WHITE),
    ([230, 170, 40], WHITE),
    ([60, 60, 64], [220, 200, 150]),
    ([170, 60, 120], WHITE),
    ([30, 110, 120], [240, 230, 200]),
    ([226, 120, 40], [60, 60, 64]),
];
pub const SIGNS: [Rgb; 10] = [
    [40, 92, 70],
    [36, 60, 110],
    [130, 40, 40],
    [230, 170, 40],
    [196, 58, 52],
    [40, 92, 160],
    [60, 60, 66],
    [150, 70, 140],
    [30, 120, 120],
    [226, 120, 40],
];
pub const FLOWERS: [Rgb; 5] = [
    [220, 60, 70],
    [240, 200, 60],
    [240, 150, 190],
    [240, 240, 236],
    [150, 90, 200],
];

/// Material weights (brick, wood, stucco, concrete) for houses in a region.
fn mat_weights(r: Region) -> [u32; 4] {
    match r {
        Region::Mixed => [3, 3, 3, 1],
        Region::Prairie => [6, 3, 1, 0],
        Region::River => [2, 7, 1, 0],
        Region::Sunbelt => [1, 1, 8, 1],
        Region::Crossroads => [4, 4, 2, 0],
        Region::Lake => [1, 7, 2, 0],
    }
}

pub fn material(r: Region, rng: &mut Rng) -> Mat {
    let w = mat_weights(r);
    wpick(
        rng,
        &[
            (Mat::Brick, w[0]),
            (Mat::Wood, w[1]),
            (Mat::Stucco, w[2]),
            (Mat::Concrete, w[3]),
        ],
    )
}

/// Wall colour and trim colour for a material.
pub fn wall_colours(m: Mat, rng: &mut Rng) -> (Rgb, Rgb) {
    match m {
        Mat::Brick => (
            rng.pick(&BRICKS),
            rng.pick(&[TRIM, STONE, [90, 84, 80], WHITE]),
        ),
        Mat::Wood => (
            rng.pick(&PAINTS),
            rng.pick(&[TRIM, WHITE, [60, 64, 72], [236, 226, 196]]),
        ),
        Mat::Stucco => (
            rng.pick(&STUCCOS),
            rng.pick(&[WHITE, [180, 100, 70], [96, 120, 130], TRIM]),
        ),
        Mat::Concrete => (rng.pick(&CONCS), rng.pick(&[WHITE, [90, 94, 100], TRIM])),
    }
}

/// Roof colour for shops, flats and offices: the usual mix, but a dark slate is rolled again so
/// a street of them is not a row of black lids.
pub fn roof_colour_civic(r: Region, rng: &mut Rng) -> Rgb {
    for _ in 0..4 {
        let c = roof_colour(r, rng);
        let lum = (c[0] as u32 * 3 + c[1] as u32 * 6 + c[2] as u32) / 10;
        if lum >= 108 {
            return c;
        }
    }
    // a light weathered tone with a tint of the last roll
    let c = roof_colour(r, rng);
    [
        c[0].saturating_add(70).max(150),
        c[1].saturating_add(60).max(140),
        c[2].saturating_add(55).max(130),
    ]
}

/// Flat roof deck colour that goes with a wall colour: a pale weathered grey with a tint of the
/// wall, never the dark slate a flat roof used to get.
pub fn deck_of(wall: Rgb) -> Rgb {
    let m = |w: u8, g: u8| ((w as u32 + g as u32 * 2) / 3) as u8;
    [m(wall[0], 168), m(wall[1], 164), m(wall[2], 158)]
}

pub fn roof_colour(r: Region, rng: &mut Rng) -> Rgb {
    let (tile, metal) = match r {
        Region::Sunbelt => (7, 1),
        Region::Lake => (1, 4),
        Region::Mixed => (2, 2),
        _ => (1, 1),
    };
    match wpick(rng, &[(0u8, 8), (1, tile), (2, metal)]) {
        0 => {
            if rng.chance(0.6) {
                rng.pick(&SHINGLES[..5])
            } else {
                rng.pick(&SHINGLES)
            }
        }
        1 => rng.pick(&TILES),
        _ => rng.pick(&METALS),
    }
}

// ---- roofs ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoofKind {
    Gable,
    Cross,
    Hip,
    Flat,
    Mansard,
    Shed,
}

pub fn roof_kind(r: Region, rng: &mut Rng) -> RoofKind {
    use RoofKind::*;
    let w: [u32; 6] = match r {
        Region::Mixed => [3, 2, 2, 2, 1, 1],
        Region::Prairie => [6, 3, 2, 0, 1, 0],
        Region::River => [4, 3, 1, 0, 1, 1],
        Region::Sunbelt => [1, 1, 4, 4, 0, 1],
        Region::Crossroads => [3, 2, 2, 1, 2, 0],
        Region::Lake => [4, 2, 1, 0, 0, 3],
    };
    wpick(
        rng,
        &[
            (Gable, w[0]),
            (Cross, w[1]),
            (Hip, w[2]),
            (Flat, w[3]),
            (Mansard, w[4]),
            (Shed, w[5]),
        ],
    )
}

/// Put a roof of `kind` on a box whose top is at `z`; `rh` is the pitched rise. Returns the
/// height of the highest point.
#[allow(clippy::too_many_arguments)]
pub fn roof_on(
    o: &mut Obj,
    kind: RoofKind,
    (x0, y0, x1, y1): (f32, f32, f32, f32),
    z: f32,
    rh: f32,
    roof: Rgb,
    wall: Rgb,
    trim: Rgb,
    along_x: bool,
    rng: &mut Rng,
) -> f32 {
    match kind {
        RoofKind::Gable | RoofKind::Cross => {
            o.gable(x0, y0, x1, y1, z, rh, roof, wall, along_x, 0.4);
            z + rh
        }
        RoofKind::Hip => {
            o.hip(x0, y0, x1, y1, z, rh * 0.85, roof, 0.4);
            z + rh * 0.85
        }
        RoofKind::Flat => {
            o.parapet(x0, y0, x1, y1, z, 0.7, wall, trim, shade(roof, 0.6));
            z + 0.7
        }
        RoofKind::Mansard => {
            o.mansard(x0, y0, x1, y1, z, rh * 0.8, 1.3, roof, shade(roof, 0.8));
            z + rh * 0.8
        }
        RoofKind::Shed => {
            let dir = wpick(rng, &[(0u8, 3), (2, 1), (3, 1)]);
            o.shed(x0, y0, x1, y1, z, rh * 0.7, roof, wall, dir, 0.4);
            z + rh * 0.7
        }
    }
}

pub fn shade(c: Rgb, k: f32) -> Rgb {
    [
        (c[0] as f32 * k) as u8,
        (c[1] as f32 * k) as u8,
        (c[2] as f32 * k) as u8,
    ]
}

pub fn mix(c: Rgb, to: Rgb, k: f32) -> Rgb {
    let m = |a: u8, b: u8| {
        (a as f32 + (b as f32 - a as f32) * k)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [m(c[0], to[0]), m(c[1], to[1]), m(c[2], to[2])]
}

pub fn chimney(o: &mut Obj, x: f32, y: f32, z0: f32, z1: f32, col: Rgb) {
    o.bx_top(
        x - 0.4,
        y - 0.4,
        z0,
        x + 0.4,
        y + 0.4,
        z1,
        col,
        shade(col, 0.7),
    );
    o.bx(
        x - 0.55,
        y - 0.55,
        z1 - 0.15,
        x + 0.55,
        y + 0.55,
        z1 + 0.1,
        shade(col, 0.85),
    );
}

/// A dormer on the front (south) slope of a gable roof whose ridge runs along x. The roof rises
/// from `z` at the eave (y = `y0 - over`) to `z + rh` at the ridge (y = `ym`).
#[allow(clippy::too_many_arguments)]
pub fn dormer(
    o: &mut Obj,
    cx: f32,
    y0: f32,
    ym: f32,
    z: f32,
    rh: f32,
    over: f32,
    wall: Rgb,
    roof: Rgb,
    glass: Rgb,
    flat_top: bool,
) {
    let slope = |y: f32| z + rh * ((y - (y0 - over)) / (ym - (y0 - over))).clamp(0.0, 1.0);
    let (yf, yb) = (y0 + 0.9, y0 + 2.7);
    let zb = slope(yf) - 0.1;
    let zt = zb + 1.7;
    o.bx(cx - 0.9, yf, zb, cx + 0.9, yb, zt, wall);
    o.win_s(cx - 0.5, cx + 0.5, zb + 0.4, zt - 0.3, yf, glass);
    if flat_top {
        o.bx(cx - 1.05, yf - 0.15, zt, cx + 1.05, yb, zt + 0.18, roof);
    } else {
        o.gable(cx - 0.9, yf, cx + 0.9, yb, zt, 0.8, roof, wall, false, 0.2);
    }
}

/// Solar panels on the front slope of a gable roof whose ridge runs along x.
#[allow(clippy::too_many_arguments)]
pub fn solar(o: &mut Obj, x0: f32, x1: f32, y0: f32, ym: f32, z: f32, rh: f32, over: f32) {
    let slope = |y: f32| z + rh * ((y - (y0 - over)) / (ym - (y0 - over))).clamp(0.0, 1.0);
    let (a, b) = (y0 + 0.9, ym - 0.5);
    let pcol = [36, 56, 98];
    let n = ((x1 - x0) / 1.4).floor().max(1.0) as usize;
    let step = (x1 - x0) / n as f32;
    for i in 0..n {
        let (xa, xb) = (
            x0 + i as f32 * step + 0.06,
            x0 + (i + 1) as f32 * step - 0.06,
        );
        o.poly(
            &[
                [xa, a, slope(a) + 0.1],
                [xb, a, slope(a) + 0.1],
                [xb, b, slope(b) + 0.1],
                [xa, b, slope(b) + 0.1],
            ],
            pcol,
            false,
            true,
            false,
        );
    }
}

// ---- fronts ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Porch {
    None,
    Stoop,
    Full,
    Portico,
    Hood,
}

/// A porch or entrance cover on the south wall at door position `dx`.
#[allow(clippy::too_many_arguments)]
pub fn porch(
    o: &mut Obj,
    kind: Porch,
    (x0, x1): (f32, f32),
    y0: f32,
    dx: f32,
    roof: Rgb,
    wall: Rgb,
    trim: Rgb,
    height: f32,
) {
    match kind {
        Porch::None => {}
        Porch::Stoop => {
            o.bx(dx - 1.2, y0 - 1.6, 0.0, dx + 1.2, y0, 0.3, CONC);
            o.bx(dx - 1.0, y0 - 2.2, 0.0, dx + 1.0, y0 - 1.6, 0.15, CONC);
            for s in [-1.0f32, 1.0] {
                o.bx(
                    dx + s * 1.1 - 0.05,
                    y0 - 1.5,
                    0.3,
                    dx + s * 1.1 + 0.05,
                    y0 - 0.2,
                    1.0,
                    [60, 62, 68],
                );
            }
        }
        Porch::Hood => {
            o.bx(dx - 1.3, y0 - 1.1, 2.55, dx + 1.3, y0, 2.7, trim);
            for s in [-1.0f32, 1.0] {
                o.bx(
                    dx + s * 1.15 - 0.08,
                    y0 - 1.0,
                    0.0,
                    dx + s * 1.15 + 0.08,
                    y0 - 0.9,
                    2.55,
                    trim,
                );
            }
            o.bx(dx - 1.2, y0 - 1.6, 0.0, dx + 1.2, y0, 0.2, CONC);
        }
        Porch::Full => {
            let (a, b) = (x0 + 0.2, x1 - 0.2);
            o.bx(a, y0 - 2.3, 0.0, b, y0, 0.4, WOOD);
            let n = (((b - a) / 2.4).ceil() as usize).max(2);
            for i in 0..=n {
                let x = a + 0.15 + i as f32 * (b - a - 0.3) / n as f32;
                o.bx(
                    x - 0.1,
                    y0 - 2.2,
                    0.4,
                    x + 0.1,
                    y0 - 2.0,
                    height - 0.2,
                    trim,
                );
            }
            o.shed(
                a - 0.2,
                y0 - 2.6,
                b + 0.2,
                y0,
                height - 0.3,
                0.5,
                roof,
                wall,
                0,
                0.0,
            );
            for (ra, rb) in [(a, dx - 1.0), (dx + 1.0, b)] {
                if rb - ra > 0.5 {
                    o.bx(ra, y0 - 2.2, 0.9, rb, y0 - 2.08, 1.05, trim);
                }
            }
        }
        Porch::Portico => {
            o.bx(dx - 2.0, y0 - 2.6, 0.0, dx + 2.0, y0, 0.4, STONE);
            for s in [-1.4f32, 1.4] {
                o.prism(dx + s, y0 - 2.2, 0.4, height + 0.1, 0.28, WHITE, 8, None);
            }
            o.bx(
                dx - 2.1,
                y0 - 2.7,
                height + 0.1,
                dx + 2.1,
                y0,
                height + 0.4,
                trim,
            );
            o.gable(
                dx - 2.1,
                y0 - 2.7,
                dx + 2.1,
                y0,
                height + 0.4,
                1.0,
                roof,
                wall,
                false,
                0.0,
            );
        }
    }
}

/// A bay window: a small box out of the south wall with glass on three sides.
#[allow(clippy::too_many_arguments)]
pub fn bay(o: &mut Obj, cx: f32, y0: f32, wall: Rgb, trim: Rgb, glass: Rgb, z0: f32, z1: f32) {
    o.bx(cx - 1.4, y0 - 0.9, z0 - 0.5, cx + 1.4, y0, z1 + 0.5, wall);
    o.win_s(cx - 1.0, cx + 1.0, z0, z1, y0 - 0.9, glass);
    o.win_w(y0 - 0.7, y0 - 0.15, z0, z1, cx - 1.4, glass);
    o.win_e(y0 - 0.7, y0 - 0.15, z0, z1, cx + 1.4, glass);
    o.bx(
        cx - 1.55,
        y0 - 1.05,
        z1 + 0.5,
        cx + 1.55,
        y0,
        z1 + 0.7,
        trim,
    );
}

/// A small balcony on the south wall: slab and a railing, with a glass door behind.
#[allow(clippy::too_many_arguments)]
pub fn balcony(o: &mut Obj, cx: f32, hw: f32, y0: f32, z: f32, deep: f32, slab: Rgb, rail: Rgb) {
    o.bx(cx - hw, y0 - deep, z, cx + hw, y0, z + 0.22, slab);
    o.bx(
        cx - hw,
        y0 - deep,
        z + 0.22,
        cx + hw,
        y0 - deep + 0.1,
        z + 1.05,
        rail,
    );
    for s in [-hw, hw - 0.1] {
        o.bx(
            cx + s,
            y0 - deep,
            z + 0.22,
            cx + s + 0.1,
            y0,
            z + 1.05,
            rail,
        );
    }
}

/// Striped (or plain, when both colours match) awning over a south shopfront.
#[allow(clippy::too_many_arguments)]
pub fn awning(
    o: &mut Obj,
    x0: f32,
    x1: f32,
    y: f32,
    zl: f32,
    zh: f32,
    depth: f32,
    c1: Rgb,
    c2: Rgb,
    stripes: usize,
) {
    for i in 0..stripes {
        let a = x0 + i as f32 * (x1 - x0) / stripes as f32;
        let b = a + (x1 - x0) / stripes as f32;
        let col = if i % 2 == 1 { c2 } else { c1 };
        o.poly(
            &[
                [a, y - depth, zl],
                [b, y - depth, zl],
                [b, y, zh],
                [a, y, zh],
            ],
            col,
            true,
            true,
            false,
        );
    }
    // valance: a thin front strip so it reads as cloth
    o.poly(
        &[
            [x0, y - depth - 0.02, zl - 0.35],
            [x1, y - depth - 0.02, zl - 0.35],
            [x1, y - depth - 0.02, zl],
            [x0, y - depth - 0.02, zl],
        ],
        c1,
        false,
        true,
        true,
    );
}

// ---- yards ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fence {
    None,
    Picket,
    Hedge,
    LowWall,
    Chain,
    Rail,
}

pub fn fence_kind(r: Region, rng: &mut Rng) -> Fence {
    use Fence::*;
    let w: [u32; 6] = match r {
        Region::Mixed => [6, 3, 2, 1, 1, 1],
        Region::Prairie => [5, 3, 2, 0, 2, 1],
        Region::River => [5, 3, 2, 0, 1, 2],
        Region::Sunbelt => [4, 0, 1, 4, 1, 0],
        Region::Crossroads => [5, 3, 1, 1, 2, 0],
        Region::Lake => [6, 2, 2, 0, 0, 3],
    };
    wpick(
        rng,
        &[
            (None, w[0]),
            (Picket, w[1]),
            (Hedge, w[2]),
            (LowWall, w[3]),
            (Chain, w[4]),
            (Rail, w[5]),
        ],
    )
}

/// A front fence along y = `y`, from `xa` to `xb`, with a gap for the path at `gap`.
#[allow(clippy::too_many_arguments)]
pub fn fence(o: &mut Obj, kind: Fence, xa: f32, xb: f32, y: f32, gap: f32, wall: Rgb, trim: Rgb) {
    if kind == Fence::None {
        return;
    }
    let spans = [(xa, gap - 0.8), (gap + 0.8, xb)];
    for (a, b) in spans {
        if b - a < 0.6 {
            continue;
        }
        match kind {
            Fence::None => {}
            Fence::Picket => {
                o.bx(a, y - 0.04, 0.55, b, y + 0.04, 0.7, WHITE);
                o.bx(a, y - 0.04, 0.2, b, y + 0.04, 0.3, WHITE);
                let n = (((b - a) / 1.2).ceil() as usize).max(1);
                for i in 0..=n {
                    let x = a + i as f32 * (b - a) / n as f32;
                    o.bx(x - 0.07, y - 0.07, 0.0, x + 0.07, y + 0.07, 0.95, WHITE);
                }
            }
            Fence::Hedge => {
                o.bx_top(a, y - 0.4, 0.0, b, y + 0.4, 0.95, GREEN, [104, 160, 84]);
            }
            Fence::LowWall => {
                o.bx_top(a, y - 0.2, 0.0, b, y + 0.2, 0.85, wall, trim);
            }
            Fence::Chain => {
                o.bx(a, y - 0.02, 0.0, b, y + 0.02, 1.15, [156, 160, 168]);
                o.bx(a, y - 0.05, 1.1, b, y + 0.05, 1.18, [110, 114, 120]);
            }
            Fence::Rail => {
                o.bx(a, y - 0.05, 0.45, b, y + 0.05, 0.6, WOOD);
                o.bx(a, y - 0.05, 0.85, b, y + 0.05, 1.0, WOOD);
                let n = (((b - a) / 2.4).ceil() as usize).max(1);
                for i in 0..=n {
                    let x = a + i as f32 * (b - a) / n as f32;
                    o.bx(
                        x - 0.08,
                        y - 0.08,
                        0.0,
                        x + 0.08,
                        y + 0.08,
                        1.1,
                        [120, 88, 62],
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    Shed,
    Pool,
    Trampoline,
    Garden,
    Swing,
    Table,
    Boat,
    Woodpile,
    Doghouse,
    Kayaks,
    Hammock,
    Bike,
}

pub fn prop_kind(r: Region, rng: &mut Rng) -> Prop {
    use Prop::*;
    let w: [u32; 12] = match r {
        Region::Mixed => [3, 2, 2, 3, 2, 2, 1, 1, 1, 0, 1, 1],
        Region::Prairie => [4, 1, 3, 3, 2, 2, 0, 2, 2, 0, 0, 1],
        Region::River => [3, 0, 2, 4, 2, 2, 1, 3, 1, 1, 1, 1],
        Region::Sunbelt => [1, 8, 2, 2, 1, 3, 0, 0, 0, 0, 1, 1],
        Region::Crossroads => [3, 1, 2, 3, 2, 2, 0, 1, 2, 0, 0, 2],
        Region::Lake => [2, 0, 1, 2, 1, 2, 5, 2, 1, 4, 2, 0],
    };
    wpick(
        rng,
        &[
            (Shed, w[0]),
            (Pool, w[1]),
            (Trampoline, w[2]),
            (Garden, w[3]),
            (Swing, w[4]),
            (Table, w[5]),
            (Boat, w[6]),
            (Woodpile, w[7]),
            (Doghouse, w[8]),
            (Kayaks, w[9]),
            (Hammock, w[10]),
            (Bike, w[11]),
        ],
    )
}

/// A yard prop centred at (x, y), facing south.
pub fn prop(o: &mut Obj, kind: Prop, x: f32, y: f32, rng: &mut Rng) {
    match kind {
        Prop::Shed => {
            let c = rng.pick(&[WOOD, [196, 70, 52], [110, 130, 150], WHITE, [96, 128, 100]]);
            o.bx(x - 1.2, y - 1.3, 0.0, x + 1.2, y + 1.3, 2.3, c);
            o.shed(
                x - 1.2,
                y - 1.3,
                x + 1.2,
                y + 1.3,
                2.3,
                0.6,
                shade(c, 0.7),
                c,
                0,
                0.2,
            );
            o.win_s(x - 0.5, x + 0.5, 0.0, 1.9, y - 1.3, [70, 56, 46]);
        }
        Prop::Pool => {
            o.slab(
                x - 3.0,
                y - 1.9,
                0.0,
                x + 3.0,
                y + 1.9,
                0.1,
                [214, 208, 192],
            );
            o.poly(
                &[
                    [x - 2.3, y - 1.2, 0.14],
                    [x + 2.3, y - 1.2, 0.14],
                    [x + 2.3, y + 1.2, 0.14],
                    [x - 2.3, y + 1.2, 0.14],
                ],
                [80, 170, 224],
                false,
                true,
                true,
            );
        }
        Prop::Trampoline => {
            o.prism(x, y, 0.55, 0.7, 1.7, [48, 50, 60], 10, Some([34, 36, 46]));
            for k in 0..4 {
                let a = k as f32 * 1.571 + 0.4;
                o.bx(
                    x + 1.5 * libm::cosf(a) - 0.06,
                    y + 1.5 * libm::sinf(a) - 0.06,
                    0.0,
                    x + 1.5 * libm::cosf(a) + 0.06,
                    y + 1.5 * libm::sinf(a) + 0.06,
                    0.55,
                    [110, 114, 120],
                );
            }
        }
        Prop::Garden => {
            let beds = 2 + rng.below(2);
            for i in 0..beds {
                let bx = x - 2.0 + i as f32 * 2.0;
                o.slab(
                    bx - 0.8,
                    y - 0.8,
                    0.0,
                    bx + 0.8,
                    y + 0.8,
                    0.22,
                    [112, 82, 58],
                );
                let c = rng.pick(&FLOWERS);
                for (fx, fy) in [(-0.4f32, -0.4f32), (0.4, 0.3), (0.0, 0.0)] {
                    o.bx(
                        bx + fx - 0.15,
                        y + fy - 0.15,
                        0.22,
                        bx + fx + 0.15,
                        y + fy + 0.15,
                        0.6,
                        c,
                    );
                }
            }
        }
        Prop::Swing => {
            for sx in [-1.4f32, 1.4] {
                o.bx(
                    x + sx - 0.07,
                    y - 0.5,
                    0.0,
                    x + sx + 0.07,
                    y - 0.3,
                    2.0,
                    [196, 70, 52],
                );
                o.bx(
                    x + sx - 0.07,
                    y + 0.3,
                    0.0,
                    x + sx + 0.07,
                    y + 0.5,
                    2.0,
                    [196, 70, 52],
                );
            }
            o.bx(
                x - 1.5,
                y - 0.07,
                1.95,
                x + 1.5,
                y + 0.07,
                2.1,
                [196, 70, 52],
            );
            for sx in [-0.5f32, 0.5] {
                o.bx(
                    x + sx - 0.25,
                    y - 0.12,
                    0.7,
                    x + sx + 0.25,
                    y + 0.12,
                    0.78,
                    [40, 40, 48],
                );
            }
        }
        Prop::Table => {
            o.prism(x, y, 0.0, 0.8, 0.7, WOOD, 8, None);
            let c = rng.pick(&[[196, 58, 52], [230, 170, 40], [40, 92, 160], WHITE]);
            o.bx(
                x - 0.05,
                y - 0.05,
                0.8,
                x + 0.05,
                y + 0.05,
                2.3,
                [150, 150, 150],
            );
            o.pyramid(x, y, 2.3, 1.7, 0.6, c, 8, 0.0);
        }
        Prop::Boat => {
            o.bx(x - 0.9, y - 2.4, 0.0, x + 0.9, y + 2.4, 0.3, [60, 62, 70]);
            let c = rng.pick(&[WHITE, [196, 58, 52], [40, 92, 160], [226, 190, 60]]);
            o.bx_top(
                x - 0.85,
                y - 2.0,
                0.3,
                x + 0.85,
                y + 2.1,
                1.1,
                c,
                shade(c, 0.8),
            );
            o.bx(x - 0.3, y - 0.2, 1.1, x + 0.3, y + 0.8, 1.7, GLASS_D);
        }
        Prop::Woodpile => {
            for r in 0..3 {
                let w = 1.6 - r as f32 * 0.3;
                o.bx(
                    x - w,
                    y - 0.4,
                    r as f32 * 0.4,
                    x + w,
                    y + 0.4,
                    r as f32 * 0.4 + 0.4,
                    [140, 100, 66],
                );
            }
        }
        Prop::Doghouse => {
            let c = rng.pick(&[[196, 70, 52], [120, 150, 190], WOOD]);
            o.bx(x - 0.5, y - 0.6, 0.0, x + 0.5, y + 0.6, 0.8, c);
            o.gable(
                x - 0.5,
                y - 0.6,
                x + 0.5,
                y + 0.6,
                0.8,
                0.45,
                shade(c, 0.7),
                c,
                false,
                0.1,
            );
        }
        Prop::Kayaks => {
            for (i, c) in [[226, 120, 40], [40, 130, 90], [196, 58, 52]]
                .iter()
                .enumerate()
            {
                o.bx(
                    x - 1.6,
                    y - 0.4 + i as f32 * 0.5,
                    0.15 + i as f32 * 0.3,
                    x + 1.6,
                    y - 0.1 + i as f32 * 0.5,
                    0.4 + i as f32 * 0.3,
                    *c,
                );
            }
            for sx in [-1.3f32, 1.3] {
                o.bx(
                    x + sx - 0.05,
                    y - 0.5,
                    0.0,
                    x + sx + 0.05,
                    y + 1.2,
                    1.1,
                    WOOD,
                );
            }
        }
        Prop::Hammock => {
            for sx in [-1.6f32, 1.6] {
                o.prism(x + sx, y, 0.0, 2.2, 0.12, [100, 78, 56], 5, None);
            }
            let c = rng.pick(&[[196, 58, 52], [226, 190, 60], [60, 130, 150]]);
            o.bx(x - 1.4, y - 0.3, 0.9, x + 1.4, y + 0.3, 1.05, c);
        }
        Prop::Bike => {
            o.bx(
                x - 0.8,
                y - 0.04,
                0.05,
                x + 0.8,
                y + 0.04,
                0.75,
                [48, 50, 60],
            );
            o.bx(
                x - 0.3,
                y - 0.05,
                0.5,
                x + 0.5,
                y + 0.05,
                0.6,
                rng.pick(&[[196, 58, 52], [40, 92, 160]]),
            );
        }
    }
}

// ---- rooftop gear ----

/// Rooftop units on a flat roof at height `z` inside the rectangle: air units, a tank, vents, a
/// stair box, an antenna.
pub fn roof_gear(
    o: &mut Obj,
    (x0, y0, x1, y1): (f32, f32, f32, f32),
    z: f32,
    n: usize,
    rng: &mut Rng,
) {
    let (w, d) = (x1 - x0, y1 - y0);
    for _ in 0..n {
        let x = x0 + 1.5 + rng.f32() * (w - 3.0).max(0.1);
        let y = y0 + 1.5 + rng.f32() * (d - 3.0).max(0.1);
        match wpick(rng, &[(0u8, 4), (1, 1), (2, 2), (3, 1), (4, 1), (5, 1)]) {
            0 => {
                o.bx_top(
                    x - 0.9,
                    y - 0.7,
                    z,
                    x + 0.9,
                    y + 0.7,
                    z + 0.9,
                    [150, 152, 156],
                    [120, 122, 128],
                );
            }
            1 => {
                o.prism(x, y, z, z + 2.2, 1.1, [150, 110, 80], 8, None);
                o.pyramid(x, y, z + 2.2, 1.15, 0.6, [110, 84, 62], 8, 0.0);
            }
            2 => {
                o.prism(x, y, z, z + 0.7, 0.35, [140, 144, 150], 6, None);
            }
            3 => {
                o.bx(
                    x - 1.0,
                    y - 1.0,
                    z,
                    x + 1.0,
                    y + 1.0,
                    z + 2.2,
                    shade(TRIM, 0.9),
                );
            }
            4 => {
                o.bx(
                    x - 0.04,
                    y - 0.04,
                    z,
                    x + 0.04,
                    y + 0.04,
                    z + 3.2,
                    [90, 94, 100],
                );
                o.bx(
                    x - 0.5,
                    y - 0.03,
                    z + 2.6,
                    x + 0.5,
                    y + 0.03,
                    z + 2.7,
                    [90, 94, 100],
                );
            }
            _ => {
                o.poly(
                    &[
                        [x - 1.2, y - 0.8, z + 0.5],
                        [x + 1.2, y - 0.8, z + 0.5],
                        [x + 1.2, y + 0.8, z + 1.1],
                        [x - 1.2, y + 0.8, z + 1.1],
                    ],
                    [36, 56, 98],
                    false,
                    true,
                    false,
                );
            }
        }
    }
}

// ---- window patterns ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Grid,
    Ribbon,
    Strips,
    Pairs,
    Curtain,
    Banded,
}

/// Windows on one rectangular face, drawn through `place(centre, z0, z1, half_width, colour)`.
/// `len` is the wall length, `floors` and `fh` the storeys and their height, `z_base` the sill
/// base of the first floor.
#[allow(clippy::too_many_arguments)]
pub fn face_windows(
    pat: Pattern,
    len: f32,
    floors: usize,
    fh: f32,
    z_base: f32,
    glass: Rgb,
    glass2: Rgb,
    place: &mut dyn FnMut(f32, f32, f32, f32, Rgb),
) {
    let h = floors as f32 * fh;
    match pat {
        Pattern::Grid => {
            let gap = 3.0;
            let n = ((len / gap) as usize).max(1);
            for f in 0..floors {
                let (za, zb) = (
                    z_base + f as f32 * fh + fh * 0.32,
                    z_base + f as f32 * fh + fh * 0.78,
                );
                for i in 0..n {
                    let c = -len / 2.0 + (i as f32 + 0.5) * len / n as f32;
                    place(c, za, zb, 0.75, glass);
                }
            }
        }
        Pattern::Ribbon => {
            for f in 0..floors {
                let (za, zb) = (
                    z_base + f as f32 * fh + fh * 0.3,
                    z_base + f as f32 * fh + fh * 0.75,
                );
                place(0.0, za, zb, len / 2.0 - 0.7, glass);
            }
        }
        Pattern::Strips => {
            let n = ((len / 2.4) as usize).max(1);
            for i in 0..n {
                let c = -len / 2.0 + (i as f32 + 0.5) * len / n as f32;
                place(
                    c,
                    z_base + 0.8,
                    z_base + h - 0.5,
                    0.45,
                    if i % 2 == 0 { glass } else { glass2 },
                );
            }
        }
        Pattern::Pairs => {
            let bay = 3.8;
            let n = ((len / bay) as usize).max(1);
            for f in 0..floors {
                let (za, zb) = (
                    z_base + f as f32 * fh + fh * 0.3,
                    z_base + f as f32 * fh + fh * 0.78,
                );
                for i in 0..n {
                    let c = -len / 2.0 + (i as f32 + 0.5) * len / n as f32;
                    place(c - 0.65, za, zb, 0.5, glass);
                    place(c + 0.65, za, zb, 0.5, glass);
                }
            }
        }
        Pattern::Curtain => {
            let n = ((len / 1.8) as usize).max(2);
            for i in 0..n {
                let c = -len / 2.0 + (i as f32 + 0.5) * len / n as f32;
                place(
                    c,
                    z_base + 0.3,
                    z_base + h - 0.3,
                    len / n as f32 / 2.0 - 0.04,
                    if i % 2 == 0 { glass } else { glass2 },
                );
            }
        }
        Pattern::Banded => {
            for f in 0..floors {
                let (za, zb) = (
                    z_base + f as f32 * fh + 0.7,
                    z_base + f as f32 * fh + fh - 0.5,
                );
                place(0.0, za, zb, len / 2.0 - 0.5, glass);
            }
        }
    }
}

/// Windows on the four faces of an axis aligned box in pattern `pat`. `faces` is a bit mask: 1
/// south, 2 north, 4 west, 8 east. The south wall's `skip` range (x0, x1, z_top) is left free,
/// for a shopfront or an entrance.
#[allow(clippy::too_many_arguments)]
pub fn box_windows(
    o: &mut Obj,
    (x0, y0, x1, y1): (f32, f32, f32, f32),
    z_base: f32,
    floors: usize,
    fh: f32,
    pat: Pattern,
    glass: Rgb,
    glass2: Rgb,
    faces: u8,
) {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    if faces & 1 != 0 {
        face_windows(
            pat,
            x1 - x0,
            floors,
            fh,
            z_base,
            glass,
            glass2,
            &mut |c, za, zb, hw, col| {
                o.win_s(cx + c - hw, cx + c + hw, za, zb, y0, col);
            },
        );
    }
    if faces & 2 != 0 {
        face_windows(
            pat,
            x1 - x0,
            floors,
            fh,
            z_base,
            glass,
            glass2,
            &mut |c, za, zb, hw, col| {
                o.win_n(cx + c - hw, cx + c + hw, za, zb, y1, col);
            },
        );
    }
    if faces & 4 != 0 {
        face_windows(
            pat,
            y1 - y0,
            floors,
            fh,
            z_base,
            glass,
            glass2,
            &mut |c, za, zb, hw, col| {
                o.win_w(cy + c - hw, cy + c + hw, za, zb, x0, col);
            },
        );
    }
    if faces & 8 != 0 {
        face_windows(
            pat,
            y1 - y0,
            floors,
            fh,
            z_base,
            glass,
            glass2,
            &mut |c, za, zb, hw, col| {
                o.win_e(cy + c - hw, cy + c + hw, za, zb, x1, col);
            },
        );
    }
}

/// Mirror a model left to right (x to -x), keeping every face wound the right way round.
pub fn mirror_x(o: &mut Obj) {
    for p in &mut o.polys {
        for v in &mut p.pts {
            v[0] = -v[0];
        }
        p.pts.reverse();
    }
    for s in &mut o.solids {
        s.centre[0] = -s.centre[0];
    }
}
