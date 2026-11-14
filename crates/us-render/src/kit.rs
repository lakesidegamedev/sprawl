//! The building kit: one parametric builder per lot kind. Each model is in local metres with its
//! front door facing south (-y), centred on the origin. Every option (roof, material, porch,
//! fence, signs, yard props and so on) is drawn from the seeded `Rng`, and the town's [`Region`]
//! tilts the odds, so a town is stable between runs and each region has its own flavour.

use crate::kit_misc as misc;
pub use crate::kit_misc::{
    big_box, car, church, factory, farm, fast_food, fire_station, gas_station, park, parking,
    school, sports, strip_mall, warehouse, water_tower,
};
use crate::obj::{Obj, Rgb};
pub use crate::parts::Region;
use crate::parts::*;
use crate::rng::Rng;
use us_gen::LotKind;

pub const DOOR: Rgb = [96, 62, 44];
pub const WALLS: [Rgb; 6] = [
    [244, 216, 150],
    [160, 204, 228],
    [240, 180, 140],
    [186, 222, 160],
    [246, 236, 214],
    [218, 186, 226],
];
pub const ROOFS: [Rgb; 5] = [
    [196, 70, 52],
    [84, 96, 124],
    [150, 84, 52],
    [52, 120, 100],
    [176, 120, 60],
];
pub const BRICK: [Rgb; 4] = [[170, 86, 64], [150, 72, 58], [186, 112, 78], [128, 92, 80]];
pub const TREE: Rgb = [88, 150, 76];
pub const TREE_D: Rgb = [70, 128, 66];
pub const CAR_COLS: [Rgb; 6] = [
    [196, 58, 52],
    [48, 84, 150],
    [226, 226, 222],
    [60, 62, 70],
    [230, 190, 60],
    [60, 130, 96],
];

pub fn tree(r: f32, h: f32, col: Rgb) -> Obj {
    let mut o = Obj::new();
    o.bx(-0.25, -0.25, 0.0, 0.25, 0.25, h * 0.35, [110, 80, 56]);
    o.prism(0.0, 0.0, h * 0.3, h * 0.75, r, col, 6, None);
    o.pyramid(0.0, 0.0, h * 0.75, r, h * 0.3, col, 6, 0.0);
    // fallen leaves: a flat triangle on the ground that only shows in autumn (the shader reads
    // the blue channel as the tree's colour step)
    let step = col[2]
        .wrapping_sub(TREE[2])
        .min(col[2].wrapping_sub(TREE_D[2]))
        .min(3);
    let a = 0.9 * r + col[2] as f32 * 1.7;
    let pt = |k: f32| {
        let ang = a + k * 2.094;
        [r * 1.6 * libm::cosf(ang), r * 1.6 * libm::sinf(ang), 0.05]
    };
    o.poly(
        &[pt(0.0), pt(1.0), pt(2.0)],
        leaf_patch_colour(step),
        false,
        false,
        true,
    );
    o
}

/// The colour of a fallen leaf patch for tree colour step 0 to 3.
fn leaf_patch_colour(step: u8) -> Rgb {
    [250, 126, 16 + step]
}

pub use crate::parts::{CONC, GLASS, GLASS_D, TRIM, WHITE};

// ---- houses ----

/// A one storey home.
pub fn house(region: Region, rng: &mut Rng) -> Obj {
    dwelling(region, 1, rng)
}

/// A two storey home.
pub fn two_storey_house(region: Region, rng: &mut Rng) -> Obj {
    dwelling(region, 2, rng)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Annex {
    None,
    Garage,
    Wing,
    Carport,
}

fn porch_kind(region: Region, wing: bool, rng: &mut Rng) -> Porch {
    // none, stoop, full, portico, hood
    let mut w: [u32; 5] = match region {
        Region::Mixed => [2, 3, 3, 1, 2],
        Region::Prairie => [1, 3, 4, 1, 2],
        Region::River => [1, 2, 7, 1, 2],
        Region::Sunbelt => [3, 2, 1, 2, 3],
        Region::Crossroads => [1, 3, 4, 1, 2],
        Region::Lake => [1, 1, 8, 0, 2],
    };
    if wing {
        w[2] = 0;
    }
    wpick(
        rng,
        &[
            (Porch::None, w[0]),
            (Porch::Stoop, w[1]),
            (Porch::Full, w[2]),
            (Porch::Portico, w[3]),
            (Porch::Hood, w[4]),
        ],
    )
}

fn annex_kind(region: Region, rng: &mut Rng) -> Annex {
    let w: [u32; 4] = match region {
        Region::Mixed => [3, 4, 2, 1],
        Region::Prairie => [3, 4, 2, 1],
        Region::River => [4, 3, 3, 0],
        Region::Sunbelt => [1, 7, 1, 2],
        Region::Crossroads => [4, 3, 2, 1],
        Region::Lake => [4, 3, 2, 1],
    };
    wpick(
        rng,
        &[
            (Annex::None, w[0]),
            (Annex::Garage, w[1]),
            (Annex::Wing, w[2]),
            (Annex::Carport, w[3]),
        ],
    )
}

fn dwelling(region: Region, storeys: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let (w, d) = rng.pick(&[
        (7.0f32, 8.0f32),
        (8.0, 8.0),
        (8.0, 9.0),
        (9.0, 8.0),
        (10.0, 8.0),
        (9.0, 9.0),
    ]);
    let mat = material(region, rng);
    let (wall, trim) = wall_colours(mat, rng);
    let roof = roof_colour(region, rng);
    let kind = roof_kind(region, rng);
    let h = if storeys == 1 { 3.0 } else { 5.8 };
    let rh = 2.6;
    let (x0, x1, y0, y1) = (-w / 2.0, w / 2.0, -d / 2.0, d / 2.0);
    let along_x = kind == RoofKind::Cross || rng.chance(0.6);
    o.bx(x0, y0, 0.0, x1, y1, h, wall);
    if rng.chance(0.35) {
        o.bx(
            x0 - 0.05,
            y0 - 0.05,
            0.0,
            x1 + 0.05,
            y1 + 0.05,
            0.45,
            shade(wall, 0.78),
        );
    }
    if storeys == 2 {
        o.bx(x0 - 0.1, y0 - 0.1, 2.85, x1 + 0.1, y1 + 0.1, 3.05, trim);
    }
    // a cross gable pushes a gabled bay out of the front
    let wing = (kind == RoofKind::Cross).then(|| {
        let c = if rng.chance(0.5) { -w / 4.0 } else { w / 4.0 };
        (c - 2.0, c + 2.0)
    });
    let top = roof_on(
        &mut o,
        kind,
        (x0, y0, x1, y1),
        h,
        rh,
        roof,
        wall,
        trim,
        along_x,
        rng,
    );
    if let Some((a, b)) = wing {
        o.bx(a, y0 - 1.2, 0.0, b, y0, h, wall);
        o.gable(a, y0 - 1.2, b, 0.0, h, rh * 0.8, roof, wall, false, 0.35);
    }
    let pitched = matches!(kind, RoofKind::Gable | RoofKind::Cross);
    // chimney
    if matches!(kind, RoofKind::Gable | RoofKind::Cross | RoofKind::Hip) && rng.chance(0.55) {
        let col = rng.pick(&[BRICKS[0], BRICKS[1], BRICKS[3], STONE, [120, 116, 112]]);
        let cx = rng.pick(&[x0 + 1.5, x1 - 1.5, 0.0]);
        let (px, py) = if kind == RoofKind::Hip {
            (cx * 0.5, 0.0)
        } else if along_x {
            (cx, 0.0)
        } else {
            (0.0, rng.pick(&[y0 + 1.5, y1 - 1.5]))
        };
        chimney(&mut o, px, py, h + 0.4, top + 0.9, col);
        if rng.chance(0.2) {
            chimney(
                &mut o,
                -px.signum() * 1.0 + px * 0.2,
                py,
                h + 0.4,
                top + 0.7,
                col,
            );
        }
    }
    // dormers or solar panels on a front slope that faces the street
    if pitched && along_x && storeys == 1 && wing.is_none() {
        if rng.chance(0.35) {
            let flat = rng.chance(0.4);
            let g = if rng.chance(0.2) { GLASS_D } else { GLASS };
            let n = 1 + rng.below(2);
            for i in 0..n {
                let c = if n == 1 {
                    rng.pick(&[0.0, w / 4.0, -w / 4.0])
                } else {
                    (i as f32 * 2.0 - 1.0) * w / 4.0
                };
                dormer(
                    &mut o,
                    c,
                    y0,
                    0.0,
                    h,
                    rh,
                    0.4,
                    wall,
                    shade(roof, 0.9),
                    g,
                    flat,
                );
            }
        } else if rng.chance(if region == Region::Sunbelt { 0.5 } else { 0.1 }) {
            solar(&mut o, x0 + 1.0, x1 - 1.0, y0, 0.0, h, rh, 0.4);
        }
    }
    if kind == RoofKind::Flat && rng.chance(0.5) {
        let n = 1 + rng.below(2);
        roof_gear(
            &mut o,
            (x0 + 0.5, y0 + 0.5, x1 - 0.5, y1 - 0.5),
            h + 0.1,
            n,
            rng,
        );
    }

    // front: door, porch and windows
    let dx = match wing {
        Some((a, _)) if a < 0.0 => w / 4.0,
        Some(_) => -w / 4.0,
        None => rng.pick(&[0.0, 0.0, w / 4.0, -w / 4.0]),
    };
    let door = rng.pick(&DOORS);
    let pk = porch_kind(region, wing.is_some(), rng);
    o.win_s(dx - 0.6, dx + 0.6, 0.0, 2.1, y0, door);
    porch(
        &mut o,
        pk,
        (x0, x1),
        y0,
        dx,
        shade(roof, 0.9),
        wall,
        trim,
        2.8,
    );
    let glass = if rng.chance(0.15) { GLASS_D } else { GLASS };
    let pat = rng.below(4);
    let deco = rng.below(4);
    let shutter = rng.pick(&SHUTTERS);
    let slots = [x0 + 1.6, 0.0, x1 - 1.6];
    let mut bay_done = false;
    for (si, &c) in slots.iter().enumerate() {
        if (c - dx).abs() < 1.5 {
            continue;
        }
        let in_wing = wing.is_some_and(|(a, b)| c > a - 0.5 && c < b + 0.5);
        let y = if in_wing { y0 - 1.2 } else { y0 };
        if !in_wing && !bay_done && si != 1 && w >= 8.0 && rng.chance(0.28) {
            bay(&mut o, c, y0, wall, trim, glass, 0.9, 2.1);
            bay_done = true;
            if storeys == 2 {
                o.win_s(c - 0.7, c + 0.7, 3.7, 5.0, y0, glass);
            }
            continue;
        }
        let (hw, z0, z1): (f32, f32, f32) = match pat {
            0 => (0.7, 0.9, 2.1),
            1 => (1.0, 1.0, 2.2),
            2 => (0.45, 0.6, 2.4),
            _ => {
                if si == 0 || (si == 1 && slots[0] == dx) {
                    (1.2, 0.9, 2.3)
                } else {
                    (0.6, 1.0, 2.1)
                }
            }
        };
        let hw = if in_wing { hw.min(1.2) } else { hw };
        if pat == 2 {
            for k in [-0.6f32, 0.6] {
                o.win_s(c + k - hw, c + k + hw, z0, z1, y, glass);
            }
        } else {
            o.win_s(c - hw, c + hw, z0, z1, y, glass);
        }
        if !in_wing {
            match deco {
                1 => {
                    for s in [-1.0f32, 1.0] {
                        let e = c + s * (hw + 0.35);
                        o.win_s(e - 0.14, e + 0.14, z0, z1, y0, shutter);
                    }
                }
                2 if pk != Porch::Full => {
                    o.bx_top(
                        c - hw,
                        y0 - 0.3,
                        z0 - 0.3,
                        c + hw,
                        y0,
                        z0 - 0.05,
                        WOOD,
                        GREEN,
                    );
                }
                3 => {
                    o.win_s(c - hw - 0.1, c + hw + 0.1, z0 - 0.17, z0 - 0.04, y0, trim);
                }
                _ => {}
            }
        }
        if storeys == 2 && !in_wing {
            o.win_s(c - hw.min(0.8), c + hw.min(0.8), 3.7, 5.0, y0, glass);
        }
    }
    if storeys == 2 {
        if matches!(pk, Porch::None | Porch::Stoop) && rng.chance(0.3) {
            balcony(&mut o, dx, 1.3, y0, 3.0, 1.1, trim, [60, 62, 68]);
            o.win_s(dx - 0.6, dx + 0.6, 3.25, 5.0, y0, GLASS);
        } else {
            o.win_s(dx - 0.6, dx + 0.6, 3.7, 5.0, y0, glass);
        }
    }
    // sides and back
    o.win_w(-1.5, 0.0, 0.9, 2.1, x0, glass);
    o.win_e(-1.5, 0.0, 0.9, 2.1, x1, glass);
    if storeys == 2 {
        o.win_w(-1.5, 0.0, 3.7, 5.0, x0, glass);
        o.win_e(-1.5, 0.0, 3.7, 5.0, x1, glass);
    }
    o.win_n(-2.0, -0.4, 0.9, 2.1, y1, glass);
    o.win_n(0.4, 2.0, 0.9, 2.1, y1, glass);

    // garage, wing or carport on the east side, with a driveway for the cars
    let ann = annex_kind(region, rng);
    let mut east = x1;
    let aw = (8.8 - x1).min(if w <= 8.0 { 5.2 } else { 3.8 });
    let ah = if storeys == 1 { 2.6 } else { 3.0 };
    match ann {
        Annex::Garage => {
            let two = aw >= 4.6;
            o.bx_top(
                x1,
                y0 + 1.0,
                0.0,
                x1 + aw,
                y0 + 6.5,
                ah,
                wall,
                shade(wall, 0.8),
            );
            if pitched || kind == RoofKind::Hip {
                o.gable(
                    x1,
                    y0 + 1.0,
                    x1 + aw,
                    y0 + 6.5,
                    ah,
                    1.1,
                    shade(roof, 0.95),
                    wall,
                    true,
                    0.3,
                );
            } else {
                o.bx(
                    x1 - 0.1,
                    y0 + 0.9,
                    ah,
                    x1 + aw + 0.1,
                    y0 + 6.6,
                    ah + 0.2,
                    trim,
                );
            }
            let gd = rng.pick(&[[200u8, 200, 196], WHITE, [150, 156, 164], shade(wall, 0.85)]);
            if two {
                let m = x1 + aw / 2.0;
                o.win_s(x1 + 0.4, m - 0.15, 0.0, 2.1, y0 + 1.0, gd);
                o.win_s(m + 0.15, x1 + aw - 0.4, 0.0, 2.1, y0 + 1.0, gd);
            } else {
                o.win_s(x1 + 0.4, x1 + aw - 0.4, 0.0, 2.1, y0 + 1.0, gd);
            }
            o.slab(
                x1 + 0.2,
                y0 - 3.4,
                0.0,
                x1 + aw - 0.2,
                y0 + 1.0,
                0.05,
                [150, 150, 146],
            );
            if rng.chance(0.3) {
                o.add_placed(
                    &misc::car(rng),
                    x1 + aw / 2.0,
                    y0 - 1.6,
                    std::f32::consts::FRAC_PI_2,
                );
            }
            east = x1 + aw;
        }
        Annex::Wing => {
            let ww = aw.min(3.6);
            o.bx(x1, y0 + 1.0, 0.0, x1 + ww, y1 - 0.5, ah, wall);
            if rng.chance(0.5) {
                o.gable(
                    x1,
                    y0 + 1.0,
                    x1 + ww,
                    y1 - 0.5,
                    ah,
                    1.4,
                    roof,
                    wall,
                    false,
                    0.3,
                );
            } else {
                o.shed(
                    x1,
                    y0 + 1.0,
                    x1 + ww,
                    y1 - 0.5,
                    ah,
                    0.9,
                    roof,
                    wall,
                    2,
                    0.25,
                );
            }
            o.win_s(x1 + 0.7, x1 + ww - 0.7, 0.9, 2.0, y0 + 1.0, glass);
            o.win_e(-0.5, 1.5, 0.9, 2.0, x1 + ww, glass);
            east = x1 + ww;
        }
        Annex::Carport => {
            let cw = aw.min(3.4);
            for (px, py) in [
                (x1 + 0.2, y0 + 1.0),
                (x1 + cw - 0.2, y0 + 1.0),
                (x1 + 0.2, y0 + 5.5),
                (x1 + cw - 0.2, y0 + 5.5),
            ] {
                o.bx(
                    px - 0.1,
                    py - 0.1,
                    0.0,
                    px + 0.1,
                    py + 0.1,
                    2.5,
                    [110, 112, 118],
                );
            }
            o.bx(x1, y0 + 0.9, 2.5, x1 + cw, y0 + 5.6, 2.7, shade(roof, 0.9));
            o.slab(
                x1 + 0.1,
                y0 - 3.4,
                0.0,
                x1 + cw - 0.1,
                y0 + 5.6,
                0.05,
                [150, 150, 146],
            );
            if rng.chance(0.7) {
                o.add_placed(
                    &misc::car(rng),
                    x1 + cw / 2.0,
                    y0 + 3.2,
                    std::f32::consts::FRAC_PI_2,
                );
            }
            east = x1 + cw;
        }
        Annex::None => {}
    }

    // front yard: walk, fence, flower bed, mailbox
    let yf = y0 - 3.4;
    let walk = rng.pick(&[[214u8, 206, 190], CONC, [196, 150, 120]]);
    if rng.chance(0.7) {
        let top = match pk {
            Porch::Full => y0 - 2.3,
            Porch::Stoop => y0 - 2.2,
            _ => y0 - 0.2,
        };
        o.slab(dx - 0.5, yf, 0.0, dx + 0.5, top, 0.05, walk);
    }
    let fk = fence_kind(region, rng);
    if rng.chance(0.55) {
        fence(
            &mut o,
            fk,
            x0 - 2.0,
            east.max(x1 + 2.0),
            yf,
            dx,
            shade(wall, 0.9),
            trim,
        );
    }
    if rng.chance(0.3) {
        let c = rng.pick(&FLOWERS);
        for s in [-1.0f32, 1.0] {
            if (dx + s * 2.4 - x0).abs() < w + 1.0 {
                o.bx_top(
                    dx + s * 2.4 - 0.9,
                    y0 - 0.9,
                    0.0,
                    dx + s * 2.4 + 0.9,
                    y0 - 0.2,
                    0.4,
                    [112, 82, 58],
                    GREEN,
                );
                o.bx(
                    dx + s * 2.4 - 0.2,
                    y0 - 0.7,
                    0.4,
                    dx + s * 2.4 + 0.2,
                    y0 - 0.4,
                    0.75,
                    c,
                );
            }
        }
    }
    if rng.chance(0.3) {
        let mx = x0 - 1.2;
        o.bx(mx - 0.05, yf + 0.5, 0.0, mx + 0.05, yf + 0.6, 1.0, WOOD);
        o.bx(
            mx - 0.2,
            yf + 0.4,
            1.0,
            mx + 0.2,
            yf + 0.8,
            1.3,
            rng.pick(&[[48, 84, 150], [196, 58, 52], [60, 62, 70]]),
        );
    }

    // back yard props
    let n = wpick(rng, &[(0usize, 3), (1, 5), (2, 2)]);
    let mut slot = if rng.chance(0.5) { -3.6 } else { 3.6 };
    for _ in 0..n {
        let k = prop_kind(region, rng);
        let py = y1 + if k == Prop::Pool { 2.0 } else { 1.8 };
        prop(&mut o, k, slot, py, rng);
        slot = -slot;
    }
    if rng.chance(0.5) {
        mirror_x(&mut o);
    }
    o
}

// ---- row houses ----

/// A terrace of `n` attached homes; `floors` of 0 picks each unit's height at random.
pub fn row_houses(region: Region, n: usize, floors: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let uw = rng.pick(&[5.0f32, 5.5, 6.0]);
    let d = rng.pick(&[9.0f32, 10.0]);
    let (y0, y1) = (-d / 2.0, d / 2.0);
    let total = n as f32 * uw;
    let mut x = -total / 2.0;
    let uniform = rng.chance(0.4);
    let base_mat = if matches!(region, Region::Prairie | Region::Crossroads | Region::Mixed) {
        wpick(rng, &[(Mat::Brick, 7), (Mat::Wood, 2), (Mat::Stucco, 1)])
    } else {
        material(region, rng)
    };
    let (base_wall, base_trim) = wall_colours(base_mat, rng);
    let mixed_use =
        matches!(region, Region::Crossroads | Region::Prairie | Region::Mixed) && rng.chance(0.25);
    let glass = rng.pick(&[GLASS, GLASS, GLASS_D]);
    for i in 0..n {
        let (wall, trim) = if uniform {
            (base_wall, base_trim)
        } else {
            let m = if rng.chance(0.6) {
                base_mat
            } else {
                material(region, rng)
            };
            wall_colours(m, rng)
        };
        let fl = if floors > 0 {
            floors
        } else {
            wpick(rng, &[(2usize, 6), (3, 3)])
        };
        let h = fl as f32 * 3.1;
        let roofc = roof_colour(region, rng);
        o.bx_top(x, y0, 0.0, x + uw, y1, h, wall, [110, 104, 100]);
        match rng.below(5) {
            0 | 4 => {
                o.bx(
                    x - 0.05,
                    y0 - 0.3,
                    h - 0.5,
                    x + uw + 0.05,
                    y0,
                    h + 0.15,
                    trim,
                );
            }
            1 => {
                o.parapet(x, y0, x + uw, y1, h, 0.6, wall, trim, shade(roofc, 0.6));
            }
            2 => {
                o.mansard(x, y0, x + uw, y1, h, 1.8, 1.0, roofc, shade(roofc, 0.8));
            }
            _ => {
                o.gable(x, y0, x + uw, y1, h, 2.2, roofc, wall, false, 0.2);
            }
        }
        if rng.chance(0.2) {
            roof_gear(
                &mut o,
                (x + 0.4, y0 + 0.6, x + uw - 0.4, y1 - 0.6),
                h,
                1,
                rng,
            );
        }
        let left = rng.chance(0.5);
        let dxc = if left { x + 1.3 } else { x + uw - 1.3 };
        let oc = if left { x + uw - 1.9 } else { x + 1.9 };
        let store = mixed_use && i % 2 == 0;
        let door = rng.pick(&DOORS);
        if store {
            let (c1, c2) = rng.pick(&AWNINGS);
            o.win_s(x + 0.4, x + uw - 0.4, 0.3, 2.9, y0, GLASS_D);
            o.win_s(dxc - 0.6, dxc + 0.6, 0.0, 2.5, y0, door);
            if rng.chance(0.7) {
                awning(
                    &mut o,
                    x + 0.3,
                    x + uw - 0.3,
                    y0,
                    2.5,
                    3.2,
                    1.6,
                    c1,
                    c2,
                    if rng.chance(0.5) { 6 } else { 1 },
                );
            }
            o.win_s(x + 0.4, x + uw - 0.4, 3.3, 3.9, y0, rng.pick(&SIGNS));
        } else {
            o.win_s(dxc - 0.55, dxc + 0.55, 0.0, 2.2, y0, door);
            match rng.below(4) {
                0 => {
                    o.bx(dxc - 0.9, y0 - 1.4, 0.0, dxc + 0.9, y0, 0.5, CONC);
                    o.bx(
                        dxc - 0.85,
                        y0 - 1.35,
                        0.5,
                        dxc - 0.75,
                        y0 - 0.1,
                        1.3,
                        [60, 62, 68],
                    );
                }
                1 => {
                    porch(
                        &mut o,
                        Porch::Hood,
                        (x, x + uw),
                        y0,
                        dxc,
                        roofc,
                        wall,
                        trim,
                        2.8,
                    );
                }
                2 => {
                    o.bx(dxc - 0.9, y0 - 1.0, 0.0, dxc + 0.9, y0, 0.3, CONC);
                    o.bx(dxc - 1.0, y0 - 1.1, 2.4, dxc + 1.0, y0, 2.55, trim);
                }
                _ => {}
            }
            if rng.chance(0.3) {
                bay(&mut o, oc, y0, wall, trim, glass, 0.9, 2.3);
            } else {
                o.win_s(oc - 0.9, oc + 0.9, 0.9, 2.3, y0, glass);
            }
        }
        for f in 1..fl {
            let z0 = f as f32 * 3.1 + 0.5;
            if rng.chance(0.25) && f == 1 && !store {
                bay(
                    &mut o,
                    x + uw / 2.0,
                    y0,
                    wall,
                    trim,
                    glass,
                    z0 + 0.3,
                    z0 + 1.7,
                );
            } else if rng.chance(0.5) {
                for cx in [x + 1.5, x + uw - 1.5] {
                    o.win_s(cx - 0.6, cx + 0.6, z0, z0 + 1.6, y0, glass);
                }
            } else {
                o.win_s(x + 1.0, x + uw - 1.0, z0 + 0.1, z0 + 1.5, y0, glass);
            }
            if rng.chance(0.2) && f == fl - 1 {
                o.win_s(x + 0.9, x + uw - 0.9, z0 + 1.7, z0 + 1.95, y0, trim);
            }
        }
        if i == 0 {
            for f in 0..fl {
                o.win_w(
                    -2.5,
                    1.0,
                    f as f32 * 3.1 + 0.6,
                    f as f32 * 3.1 + 2.2,
                    x,
                    glass,
                );
            }
        }
        if i + 1 == n {
            for f in 0..fl {
                o.win_e(
                    -2.5,
                    1.0,
                    f as f32 * 3.1 + 0.6,
                    f as f32 * 3.1 + 2.2,
                    x + uw,
                    glass,
                );
            }
        }
        x += uw;
    }
    if rng.chance(0.4) {
        mirror_x(&mut o);
    }
    o
}

// ---- shops ----

/// A shop with one or two floors.
pub fn shop(region: Region, rng: &mut Rng) -> Obj {
    let floors = wpick(rng, &[(1usize, 4), (2, 6)]);
    shop_floors(region, floors, rng)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Roofline {
    Cornice,
    Parapet,
    FalseFront,
    Mansard,
    Pitched,
    TileFascia,
}

/// A shop (or a shop with flats above) of `floors` storeys.
pub fn shop_floors(region: Region, floors: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let w = rng.pick(&[9.0f32, 10.0, 12.0, 12.0, 14.0]);
    let d = rng.pick(&[8.0f32, 10.0, 10.0, 11.0]);
    let floors = floors.max(1);
    let gh = 3.8;
    let uh = 3.2;
    let h = if floors == 1 {
        4.8
    } else {
        gh + (floors - 1) as f32 * uh
    };
    let mat = match region {
        Region::Prairie | Region::Crossroads => wpick(
            rng,
            &[
                (Mat::Brick, 8),
                (Mat::Wood, 2),
                (Mat::Stucco, 1),
                (Mat::Concrete, 1),
            ],
        ),
        Region::Sunbelt => wpick(
            rng,
            &[(Mat::Stucco, 7), (Mat::Concrete, 2), (Mat::Brick, 1)],
        ),
        Region::Lake | Region::River => {
            wpick(rng, &[(Mat::Wood, 6), (Mat::Brick, 4), (Mat::Stucco, 1)])
        }
        Region::Mixed => wpick(
            rng,
            &[
                (Mat::Brick, 5),
                (Mat::Wood, 2),
                (Mat::Stucco, 2),
                (Mat::Concrete, 1),
            ],
        ),
    };
    let (wall, trim) = wall_colours(mat, rng);
    let (x0, x1, y0, y1) = (-w / 2.0, w / 2.0, -d / 2.0, d / 2.0);
    let line = match region {
        Region::Sunbelt => wpick(
            rng,
            &[
                (Roofline::TileFascia, 4),
                (Roofline::Parapet, 5),
                (Roofline::Pitched, 1),
                (Roofline::Mansard, 1),
            ],
        ),
        Region::Prairie | Region::Crossroads => wpick(
            rng,
            &[
                (Roofline::Cornice, 4),
                (Roofline::Parapet, 3),
                (Roofline::FalseFront, 3),
                (Roofline::Pitched, 3),
                (Roofline::Mansard, 1),
            ],
        ),
        _ => wpick(
            rng,
            &[
                (Roofline::Cornice, 3),
                (Roofline::Parapet, 3),
                (Roofline::FalseFront, 1),
                (Roofline::Pitched, 5),
                (Roofline::Mansard, 2),
            ],
        ),
    };
    let roofc = roof_colour_civic(region, rng);
    let corner = rng.chance(0.3) && line != Roofline::FalseFront && line != Roofline::Pitched;
    let c = 2.6;
    if corner {
        let pts = [[x0, y0], [x1 - c, y0], [x1, y0 + c], [x1, y1], [x0, y1]];
        o.extrude(&pts, 0.0, h, wall, [120, 116, 112]);
        let cp = [
            [x0 - 0.1, y0 - 0.25],
            [x1 - c + 0.1, y0 - 0.25],
            [x1 + 0.2, y0 + c - 0.1],
            [x1 + 0.2, y1 + 0.1],
            [x0 - 0.1, y1 + 0.1],
        ];
        o.extrude(&cp, h - 0.2, h + 0.6, trim, trim);
    } else {
        o.bx_top(x0, y0, 0.0, x1, y1, h, wall, [120, 116, 112]);
        match line {
            Roofline::Cornice => {
                o.bx_top(
                    x0 - 0.1,
                    y0 - 0.25,
                    h - 0.2,
                    x1 + 0.1,
                    y1 + 0.1,
                    h + 0.6,
                    trim,
                    trim,
                );
            }
            Roofline::Parapet => {
                o.parapet(x0, y0, x1, y1, h, 0.9, wall, trim, deck_of(wall));
                o.bx(
                    x0 - 0.1,
                    y0 - 0.2,
                    h - 0.25,
                    x1 + 0.1,
                    y0 + 0.1,
                    h + 0.05,
                    trim,
                );
            }
            Roofline::FalseFront => {
                o.bx_top(x0, y0, h, x1, y0 + 0.4, h + 1.4, wall, trim);
                o.gable(x0, y0, x1, y0 + 0.4, h + 1.4, 1.5, wall, wall, false, 0.0);
                o.bx_top(x0, y0 + 0.4, h, x1, y1, h + 0.3, wall, deck_of(wall));
            }
            Roofline::Mansard => {
                o.mansard(x0, y0, x1, y1, h, 1.8, 1.2, roofc, shade(roofc, 0.8));
            }
            Roofline::Pitched => {
                o.gable(x0, y0, x1, y1, h, 2.4, roofc, wall, true, 0.3);
            }
            Roofline::TileFascia => {
                o.hip(
                    x0 - 0.1,
                    y0 - 0.1,
                    x1 + 0.1,
                    y0 + 2.0,
                    h - 0.2,
                    1.0,
                    rng.pick(&TILES),
                    0.2,
                );
                o.parapet(x0, y0 + 2.0, x1, y1, h, 0.7, wall, trim, deck_of(wall));
            }
        }
    }
    // shopfront
    let glass = rng.pick(&[GLASS_D, GLASS_D, GLASS]);
    let door_x = rng.pick(&[0.0, 0.0, -w / 4.0, w / 4.0]);
    let door_x = if corner {
        door_x.min(x1 - c - 2.2).max(x0 + 2.0)
    } else {
        door_x
    };
    let door = rng.pick(&DOORS);
    let sx1 = if corner { x1 - c - 0.4 } else { x1 - 0.6 };
    let split = rng.chance(0.55);
    if split {
        if door_x - 0.9 > x0 + 0.8 {
            o.win_s(x0 + 0.6, door_x - 0.9, 0.3, 3.0, y0, glass);
        }
        if sx1 > door_x + 0.9 + 0.8 {
            o.win_s(door_x + 0.9, sx1, 0.3, 3.0, y0, glass);
        }
        o.win_s(door_x - 0.8, door_x + 0.8, 0.0, 2.6, y0, door);
    } else {
        o.win_s(x0 + 0.6, sx1, 0.0, 3.2, y0, glass);
        o.win_s(door_x - 0.8, door_x + 0.8, 0.0, 2.6, y0, door);
    }
    let sign = rng.pick(&SIGNS);
    let style = rng.below(4);
    if style != 3 {
        o.win_s(x0 + 0.6, sx1, 3.45, 4.2, y0, sign);
    }
    if style == 1 || style == 3 {
        let bz = 2.7 + rng.f32() * 0.4;
        o.bx(x0 + 0.5, y0 - 1.3, bz, x0 + 0.62, y0 - 0.1, bz + 0.9, sign);
        o.bx(
            x0 + 0.5,
            y0 - 1.3,
            bz + 0.9,
            x0 + 0.62,
            y0 - 1.2,
            bz + 1.05,
            [60, 62, 68],
        );
    }
    if rng.chance(0.5) {
        let (c1, c2) = rng.pick(&AWNINGS);
        let n = if rng.chance(0.3) {
            1
        } else {
            6 + 2 * rng.below(3)
        };
        awning(&mut o, x0 + 0.4, sx1 + 0.2, y0, 2.6, 3.4, 1.8, c1, c2, n);
    }
    // side wall: a wrap-around window or two
    if rng.chance(0.5) {
        o.win_w(-d / 2.0 + 1.2, d / 2.0 - 1.5, 0.5, 3.0, x0, glass);
    } else {
        o.win_w(-1.0, 1.0, 1.0, 2.6, x0, GLASS);
    }
    if corner {
        let (a, b) = ([x1 - c, y0], [x1, y0 + c]);
        o.wall_quad(a, b, 0.0, 2.7, door);
        o.wall_quad(
            [a[0] + 0.5, a[1] + 0.1],
            [b[0] - 0.1, b[1] - 0.5],
            2.9,
            3.6,
            sign,
        );
        o.win_e(y0 + c + 0.4, y1 - 0.8, 0.5, 3.0, x1, glass);
    } else {
        o.win_e(-1.0, 1.0, 1.0, 2.6, x1, GLASS);
    }
    // upper floors
    if floors > 1 {
        let pat = wpick(
            rng,
            &[
                (Pattern::Grid, 5),
                (Pattern::Pairs, 3),
                (Pattern::Banded, 1),
                (Pattern::Strips, 1),
            ],
        );
        let g = rng.pick(&[GLASS, GLASS, GLASS_D]);
        let mut faces = 2 | 4;
        if !corner {
            faces |= 8;
        }
        box_windows(
            &mut o,
            (x0, y0, x1, y1),
            gh,
            floors - 1,
            uh,
            pat,
            g,
            GLASS_D,
            faces,
        );
        if corner {
            o.wall_quad([x1 - c, y0], [x1, y0 + c], gh + 0.9, gh + 2.3, g);
            for f in 0..floors - 1 {
                let z = gh + f as f32 * uh;
                o.win_s(x0 + 0.8, x1 - c - 0.8, z + 0.9, z + 2.3, y0, g);
                o.win_e(y0 + c + 0.8, y1 - 0.8, z + 0.9, z + 2.3, x1, g);
            }
        } else {
            for f in 0..floors - 1 {
                let z = gh + f as f32 * uh;
                o.win_s(x0 + 0.8, x1 - 0.8, z + 0.9, z + 2.3, y0, g);
            }
        }
    } else if rng.chance(0.4) {
        o.win_s(x0 + 0.6, sx1, 4.3, 4.55, y0, trim);
    }
    if matches!(line, Roofline::Parapet | Roofline::Cornice) && rng.chance(0.5) {
        let n = 1 + rng.below(3);
        roof_gear(&mut o, (x0 + 0.5, y0 + 1.0, x1 - 0.5, y1 - 0.5), h, n, rng);
    }
    if line == Roofline::Parapet && rng.chance(0.15) {
        let rc = rng.pick(&SIGNS);
        o.bx(-2.0, y0 + 0.6, h, 2.0, y0 + 0.9, h + 1.6, rc);
        for px in [-1.6f32, 1.6] {
            o.bx(
                px - 0.06,
                y0 + 0.6,
                h,
                px + 0.06,
                y0 + 0.9,
                h + 0.1,
                [70, 72, 78],
            );
        }
    }
    if rng.chance(0.4) {
        mirror_x(&mut o);
    }
    o
}

// ---- apartments ----

pub fn apartments(region: Region, floors: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let low = floors <= 3;
    let mat = match region {
        Region::Sunbelt => wpick(
            rng,
            &[(Mat::Stucco, 7), (Mat::Concrete, 2), (Mat::Brick, 1)],
        ),
        Region::Prairie | Region::Crossroads => wpick(
            rng,
            &[
                (Mat::Brick, 7),
                (Mat::Stucco, 1),
                (Mat::Concrete, 2),
                (Mat::Wood, if low { 2 } else { 0 }),
            ],
        ),
        Region::River | Region::Lake => wpick(
            rng,
            &[
                (Mat::Brick, 4),
                (Mat::Wood, if low { 5 } else { 1 }),
                (Mat::Stucco, 2),
                (Mat::Concrete, 2),
            ],
        ),
        Region::Mixed => wpick(
            rng,
            &[
                (Mat::Brick, 4),
                (Mat::Stucco, 3),
                (Mat::Concrete, 3),
                (Mat::Wood, if low { 2 } else { 0 }),
            ],
        ),
    };
    let (wall, trim) = wall_colours(mat, rng);
    let (w, d) = rng.pick(&[
        (16.0f32, 11.0f32),
        (18.0, 12.0),
        (18.0, 12.0),
        (20.0, 12.0),
        (17.0, 13.0),
    ]);
    let fh = 3.1;
    let h = floors as f32 * fh;
    let (x0, x1, y0, y1) = (-w / 2.0, w / 2.0, -d / 2.0, d / 2.0);
    let plan = rng.below(4);
    // plan 0 slab, 1 U with a front court, 2 L with a side wing, 3 slab with a stair tower
    let roofc = roof_colour_civic(region, rng);
    let pat = wpick(
        rng,
        &[
            (Pattern::Grid, 5),
            (Pattern::Pairs, 3),
            (Pattern::Ribbon, 2),
            (Pattern::Banded, 1),
            (Pattern::Strips, 1),
        ],
    );
    let g = rng.pick(&[GLASS, GLASS, GLASS_D]);
    let ridge_roof = low && rng.chance(0.4);
    let mut ext = (x0, y0, x1, y1);
    let mut faces: u8 = 1 | 2 | 4 | 8;
    match plan {
        1 => {
            // back block and two arms that reach toward the street, leaving a court
            let ay = y0 + 3.5;
            o.bx_top(x0, ay, 0.0, x1, y1, h, wall, [118, 114, 110]);
            for (a, b) in [(x0, x0 + 5.0), (x1 - 5.0, x1)] {
                o.bx_top(a, y0, 0.0, b, ay + 0.2, h, wall, [118, 114, 110]);
                box_windows(&mut o, (a, y0, b, ay), 0.0, floors, fh, pat, g, GLASS_D, 1);
            }
            box_windows(
                &mut o,
                (x0 + 5.0, ay, x1 - 5.0, y1),
                0.0,
                floors,
                fh,
                pat,
                g,
                GLASS_D,
                1 | 2,
            );
            o.win_w(y0 + 1.0, y1 - 1.0, fh * 0.32, fh * 0.78, x0, g);
            ext = (x0, ay, x1, y1);
            faces = 0;
            o.slab(x0 + 5.0, y0, 0.0, x1 - 5.0, ay, 0.06, [200, 196, 182]);
            let door = rng.pick(&DOORS);
            o.win_s(-1.2, 1.2, 0.0, 2.6, ay, door);
        }
        2 => {
            let wx = x0 + 6.0;
            o.bx_top(x0, y0, 0.0, x1, y1 - 3.0, h, wall, [118, 114, 110]);
            o.bx_top(x0, y1 - 3.0, 0.0, wx, y1 + 4.0, h, wall, [118, 114, 110]);
            box_windows(
                &mut o,
                (x0, y0, x1, y1 - 3.0),
                0.0,
                floors,
                fh,
                pat,
                g,
                GLASS_D,
                1 | 8,
            );
            box_windows(
                &mut o,
                (x0, y1 - 3.0, wx, y1 + 4.0),
                0.0,
                floors,
                fh,
                pat,
                g,
                GLASS_D,
                2 | 4 | 8,
            );
            faces = 0;
            ext = (x0, y0, x1, y1 - 3.0);
        }
        _ => {
            o.bx_top(x0, y0, 0.0, x1, y1, h, wall, [118, 114, 110]);
            if plan == 3 {
                let tx = rng.pick(&[-w / 4.0, w / 4.0, 0.0]);
                o.bx_top(
                    tx - 1.8,
                    y0 - 0.8,
                    0.0,
                    tx + 1.8,
                    y0 + 1.2,
                    h + 1.6,
                    shade(wall, 0.92),
                    [118, 114, 110],
                );
                o.win_s(tx - 0.5, tx + 0.5, 1.0, h + 0.8, y0 - 0.8, g);
            }
        }
    }
    if faces != 0 {
        let skip_front = plan == 3;
        box_windows(
            &mut o,
            ext,
            0.0,
            floors,
            fh,
            pat,
            g,
            GLASS_D,
            if skip_front { faces & !1 } else { faces },
        );
        if skip_front {
            // windows either side of the stair tower, written plainly
            for f in 0..floors {
                let z = f as f32 * fh;
                for cx in [-w / 2.0 + 2.0, w / 2.0 - 2.0] {
                    o.win_s(cx - 0.8, cx + 0.8, z + 1.0, z + 2.3, y0, g);
                }
            }
        }
    }
    // balconies on the front
    if plan != 1 && plan != 3 && floors > 1 {
        let style = wpick(rng, &[(0u8, 3), (1, 3), (2, 2), (3, 2)]);
        let slab = rng.pick(&[[226u8, 222, 214], trim, CONC]);
        let rail = rng.pick(&[[90u8, 96, 104], [60, 62, 68], [236, 232, 222]]);
        for f in 1..floors {
            let z = f as f32 * fh;
            match style {
                1 => {
                    for cx in [-w * 0.3, 0.0, w * 0.3] {
                        balcony(&mut o, cx, 1.6, y0, z, 1.3, slab, rail);
                    }
                }
                2 => {
                    balcony(&mut o, 0.0, w / 2.0 - 1.0, y0, z, 1.2, slab, rail);
                }
                3 => {
                    for cx in [-w * 0.3, w * 0.3] {
                        o.bx(cx - 1.4, y0 - 0.15, z + 0.1, cx + 1.4, y0, z + 1.0, rail);
                    }
                }
                _ => {}
            }
        }
    }
    if plan != 1 {
        o.win_s(-1.2, 1.2, 0.0, 2.6, y0, rng.pick(&DOORS));
        match rng.below(3) {
            0 => {
                o.bx(-2.2, y0 - 2.2, 2.7, 2.2, y0, 2.9, [60, 64, 72]);
                for px in [-2.0f32, 2.0] {
                    o.bx(
                        px - 0.08,
                        y0 - 2.1,
                        0.0,
                        px + 0.08,
                        y0 - 1.9,
                        2.7,
                        [110, 114, 120],
                    );
                }
            }
            1 => {
                o.bx(-1.8, y0 - 1.6, 0.0, 1.8, y0, 0.3, CONC);
                o.bx(-1.9, y0 - 1.0, 2.7, 1.9, y0, 2.85, trim);
            }
            _ => {}
        }
    }
    // roof
    let (rx0, ry0, rx1, ry1) = ext;
    let (rx0, ry0, rx1, ry1) = if plan == 1 {
        (x0, y0, x1, y1)
    } else {
        (rx0, ry0, rx1, ry1)
    };
    if plan == 0 || plan == 3 {
        if ridge_roof {
            o.gable(rx0, ry0, rx1, ry1, h, 2.6, roofc, wall, true, 0.4);
        } else if low && rng.chance(0.55) {
            o.hip(rx0, ry0, rx1, ry1, h, 2.4, roofc, 0.4);
        } else if floors <= 6 && rng.chance(0.25) {
            o.mansard(rx0, ry0, rx1, ry1, h, 2.0, 1.4, roofc, shade(roofc, 0.8));
        } else {
            o.parapet(rx0, ry0, rx1, ry1, h, 0.6, wall, trim, deck_of(wall));
            o.bx(-3.0, -2.0, h, 3.0, 2.0, h + 2.4, [150, 146, 140]);
            let n = rng.below(4);
            roof_gear(
                &mut o,
                (rx0 + 0.5, ry0 + 0.5, rx1 - 0.5, ry1 - 0.5),
                h,
                n,
                rng,
            );
        }
    } else {
        o.parapet(x0, y0, x1, y1, h, 0.5, wall, trim, deck_of(wall));
        if plan == 2 {
            o.parapet(
                x0,
                y1 - 3.0,
                x0 + 6.0,
                y1 + 4.0,
                h,
                0.5,
                wall,
                trim,
                deck_of(wall),
            );
        }
        roof_gear(
            &mut o,
            (x0 + 0.5, y0 + 0.5, x1 - 0.5, y1 - 0.5),
            h,
            rng.below(3),
            rng,
        );
    }
    if rng.chance(0.3) {
        mirror_x(&mut o);
    }
    o
}

// ---- offices ----

pub fn office(region: Region, floors: usize, rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let fh = 3.6;
    let (w, d) = rng.pick(&[
        (16.0f32, 16.0f32),
        (14.0, 16.0),
        (16.0, 14.0),
        (14.0, 14.0),
        (18.0, 14.0),
    ]);
    let low = floors <= 5;
    let curtain = rng.pick(&CURTAINS);
    let mat = if low {
        match region {
            Region::Sunbelt => wpick(
                rng,
                &[(Mat::Stucco, 4), (Mat::Concrete, 4), (Mat::Brick, 1)],
            ),
            _ => wpick(
                rng,
                &[(Mat::Brick, 5), (Mat::Concrete, 4), (Mat::Stucco, 1)],
            ),
        }
    } else {
        wpick(
            rng,
            &[(Mat::Concrete, 5), (Mat::Brick, 2), (Mat::Stucco, 1)],
        )
    };
    let (frame, trim) = wall_colours(mat, rng);
    let frame = if low {
        frame
    } else {
        rng.pick(&[
            [196u8, 202, 210],
            [210, 196, 170],
            [120, 132, 150],
            [150, 154, 160],
            [70, 74, 82],
        ])
    };
    let roof = deck_of(frame);
    let pat = if low {
        wpick(
            rng,
            &[
                (Pattern::Grid, 5),
                (Pattern::Ribbon, 3),
                (Pattern::Pairs, 2),
                (Pattern::Banded, 2),
            ],
        )
    } else {
        wpick(
            rng,
            &[
                (Pattern::Banded, 4),
                (Pattern::Ribbon, 3),
                (Pattern::Curtain, 4),
                (Pattern::Strips, 2),
                (Pattern::Grid, 2),
            ],
        )
    };
    let (g1, g2) = if rng.chance(0.2) {
        ([98, 156, 204], [64, 88, 118])
    } else {
        curtain
    };
    let massing = if floors < 6 {
        0
    } else {
        wpick(rng, &[(0usize, 4), (1, 3), (2, 2), (3, 2)])
    };
    let (x0, x1, y0, y1) = (-w / 2.0, w / 2.0, -d / 2.0, d / 2.0);
    let mut top_rect = (x0, y0, x1, y1);
    let mut top_z = floors as f32 * fh;
    match massing {
        1 => {
            // podium of two floors, tower set back
            let ph = 2.0 * fh;
            o.bx_top(
                x0 - 1.5,
                y0 - 1.5,
                0.0,
                x1 + 1.5,
                y1 + 0.0,
                ph,
                shade(frame, 0.9),
                roof,
            );
            box_windows(
                &mut o,
                (x0 - 1.5, y0 - 1.5, x1 + 1.5, y1),
                0.0,
                2,
                fh,
                Pattern::Banded,
                g1,
                g2,
                1 | 4 | 8,
            );
            let (tx0, ty0, tx1, ty1) = (x0 + 1.0, y0 + 1.0, x1 - 1.0, y1 - 1.0);
            let th = floors as f32 * fh;
            o.bx_top(tx0, ty0, ph, tx1, ty1, th, frame, roof);
            box_windows(
                &mut o,
                (tx0, ty0, tx1, ty1),
                ph,
                floors - 2,
                fh,
                pat,
                g1,
                g2,
                15,
            );
            top_rect = (tx0, ty0, tx1, ty1);
        }
        2 => {
            // stepped tiers
            let t1 = (floors as f32 * 0.5).max(2.0).floor();
            let t2 = (floors as f32 * 0.8).max(t1 + 1.0).floor();
            let tiers = [
                (0.0f32, t1, 0.0f32),
                (t1, t2, 1.6),
                (t2, floors as f32, 3.4),
            ];
            for (a, b, ins) in tiers {
                if b <= a {
                    continue;
                }
                let (tx0, ty0, tx1, ty1) = (x0 + ins, y0 + ins, x1 - ins, y1 - ins);
                o.bx_top(tx0, ty0, a * fh, tx1, ty1, b * fh, frame, roof);
                box_windows(
                    &mut o,
                    (tx0, ty0, tx1, ty1),
                    a * fh,
                    (b - a) as usize,
                    fh,
                    pat,
                    g1,
                    g2,
                    15,
                );
                top_rect = (tx0, ty0, tx1, ty1);
            }
        }
        3 => {
            // twin towers on a shared podium
            let ph = fh * 2.0;
            o.bx_top(x0, y0, 0.0, x1, y1, ph, shade(frame, 0.92), roof);
            box_windows(
                &mut o,
                (x0, y0, x1, y1),
                0.0,
                2,
                fh,
                Pattern::Banded,
                g1,
                g2,
                15,
            );
            for (a, b) in [(x0, -0.5), (0.5, x1)] {
                let tx0 = a + 0.5;
                let tx1 = b - 0.5;
                o.bx_top(
                    tx0,
                    y0 + 1.0,
                    ph,
                    tx1,
                    y1 - 1.0,
                    floors as f32 * fh,
                    frame,
                    roof,
                );
                box_windows(
                    &mut o,
                    (tx0, y0 + 1.0, tx1, y1 - 1.0),
                    ph,
                    floors - 2,
                    fh,
                    pat,
                    g1,
                    g2,
                    15,
                );
            }
            top_rect = (x0 + 0.5, y0 + 1.0, -1.0, y1 - 1.0);
        }
        _ => {
            o.bx_top(x0, y0, 0.0, x1, y1, top_z, frame, roof);
            box_windows(&mut o, (x0, y0, x1, y1), 0.0, floors, fh, pat, g1, g2, 15);
            if rng.chance(0.3) {
                // corner piers
                for (px, py) in [
                    (x0, y0),
                    (x1 - 0.8, y0),
                    (x0, y1 - 0.8),
                    (x1 - 0.8, y1 - 0.8),
                ] {
                    o.bx(
                        px,
                        py,
                        0.0,
                        px + 0.8,
                        py + 0.8,
                        top_z + 0.4,
                        shade(frame, 0.85),
                    );
                }
            }
        }
    }
    if massing == 3 {
        top_z = floors as f32 * fh;
    }
    // crown
    let (cx0, cy0, cx1, cy1) = top_rect;
    let (ccx, ccy) = ((cx0 + cx1) / 2.0, (cy0 + cy1) / 2.0);
    match wpick(rng, &[(0u8, 4), (1, 2), (2, 2), (3, 2), (4, 2)]) {
        0 => {
            o.bx_top(
                ccx - 5.0,
                ccy - 5.0,
                top_z,
                ccx + 5.0,
                ccy + 5.0,
                top_z + 4.0,
                frame,
                roof,
            );
            o.bx(
                ccx - 2.0,
                ccy + 1.0,
                top_z + 4.0,
                ccx + 2.0,
                ccy + 3.5,
                top_z + 6.0,
                [150, 150, 150],
            );
        }
        1 => {
            o.bx_top(
                ccx - 4.0,
                ccy - 4.0,
                top_z,
                ccx + 4.0,
                ccy + 4.0,
                top_z + 2.2,
                shade(frame, 0.9),
                roof,
            );
            o.pyramid(
                ccx,
                ccy,
                top_z + 2.2,
                4.4,
                5.0,
                rng.pick(&[[110u8, 120, 140], [90, 130, 120], [170, 150, 90]]),
                4,
                45.0,
            );
        }
        2 => {
            o.bx_top(
                ccx - 3.0,
                ccy - 3.0,
                top_z,
                ccx + 3.0,
                ccy + 3.0,
                top_z + 3.0,
                frame,
                roof,
            );
            o.bx(
                ccx - 0.1,
                ccy - 0.1,
                top_z + 3.0,
                ccx + 0.1,
                ccy + 0.1,
                top_z + 11.0,
                [150, 154, 160],
            );
            o.bx(
                ccx - 0.6,
                ccy - 0.05,
                top_z + 8.0,
                ccx + 0.6,
                ccy + 0.05,
                top_z + 8.1,
                [150, 154, 160],
            );
        }
        3 => {
            o.parapet(cx0, cy0, cx1, cy1, top_z, 0.8, frame, trim, deck_of(frame));
            roof_gear(
                &mut o,
                (cx0 + 0.5, cy0 + 0.5, cx1 - 0.5, cy1 - 0.5),
                top_z,
                3 + rng.below(3),
                rng,
            );
        }
        _ => {
            o.hip(
                cx0,
                cy0,
                cx1,
                cy1,
                top_z,
                3.0,
                rng.pick(&[[90u8, 130, 120], [110, 120, 140], [150, 90, 70]]),
                0.4,
            );
        }
    }
    // base and entrance
    let cw = rng.pick(&[
        [60u8, 64, 72],
        [120, 124, 130],
        [196, 190, 178],
        [150, 70, 60],
    ]);
    let base_y = if massing == 1 { y0 - 1.5 } else { y0 };
    match rng.below(3) {
        0 => {
            o.bx(x0 - 1.0, base_y - 3.0, 3.4, x1 + 1.0, base_y, 3.8, cw);
        }
        1 => {
            o.bx(-3.0, base_y - 2.4, 3.2, 3.0, base_y, 3.5, cw);
            for px in [-2.7f32, 2.7] {
                o.prism(px, base_y - 2.1, 0.0, 3.2, 0.25, WHITE, 6, None);
            }
        }
        _ => {
            o.slab(
                x0 - 1.0,
                base_y - 4.0,
                0.0,
                x1 + 1.0,
                base_y,
                0.1,
                [200, 196, 186],
            );
            o.bx(-2.0, base_y - 1.4, 0.0, 2.0, base_y, 3.2, shade(g1, 0.8));
        }
    }
    o.win_s(-1.0, 1.0, 0.0, 2.8, base_y - 0.02, rng.pick(&DOORS));
    if rng.chance(0.3) {
        mirror_x(&mut o);
    }
    o
}

// ---- civic ----

pub fn town_hall(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let stone = rng.pick(&[
        [230u8, 222, 200],
        [214, 206, 190],
        [236, 226, 206],
        [200, 190, 176],
    ]);
    let teal = rng.pick(&[
        [90u8, 150, 140],
        [86, 120, 160],
        [120, 150, 100],
        [150, 110, 90],
    ]);
    o.bx_top(-15.0, -8.0, 0.0, 15.0, 8.0, 9.0, stone, [126, 124, 120]);
    o.bx(-15.3, -8.3, 8.6, 15.3, 8.3, 9.5, TRIM);
    o.windows_box(-15.0, -8.0, 15.0, 8.0, 2, 4.5, GLASS, 1.4, 3.0);
    o.bx(-6.0, -12.0, 0.0, 6.0, -8.0, 0.6, CONC);
    for x in [-5.0f32, -2.5, 0.0, 2.5, 5.0] {
        o.prism(x, -10.5, 0.6, 7.8, 0.45, WHITE, 6, None);
    }
    o.bx(-6.2, -12.0, 7.8, 6.2, -8.0, 8.6, stone);
    o.gable(-6.2, -12.0, 6.2, -8.0, 8.6, 2.6, stone, stone, true, 0.2);
    o.bx(-3.2, -3.2, 9.0, 3.2, 3.2, 17.0, stone);
    o.poly(
        &[
            [-1.4, -3.26, 13.0],
            [1.4, -3.26, 13.0],
            [1.4, -3.26, 15.6],
            [-1.4, -3.26, 15.6],
        ],
        WHITE,
        false,
        false,
        true,
    );
    o.bx(-3.5, -3.5, 17.0, 3.5, 3.5, 17.6, TRIM);
    o.prism(0.0, 0.0, 17.6, 20.0, 2.6, teal, 8, None);
    o.pyramid(0.0, 0.0, 20.0, 2.7, 3.2, teal, 8, 22.5);
    o.bx(-0.1, -0.1, 23.0, 0.1, 0.1, 26.5, [80, 80, 80]);
    o.poly(
        &[
            [0.1, 0.0, 25.0],
            [2.4, 0.0, 25.0],
            [2.4, 0.0, 26.4],
            [0.1, 0.0, 26.4],
        ],
        [40, 90, 170],
        false,
        true,
        true,
    );
    o.win_s(-1.2, 1.2, 0.6, 3.6, -8.0, DOOR);
    o
}

/// The model for one lot, from its kind and a seeded RNG, in no particular region.
pub fn build_lot(kind: LotKind, rng: &mut Rng) -> Obj {
    build_lot_in(Region::Mixed, kind, rng)
}

/// The model for one lot in a region.
pub fn build_lot_in(region: Region, kind: LotKind, rng: &mut Rng) -> Obj {
    match kind {
        LotKind::House => house(region, rng),
        LotKind::Row => {
            let n = 2 + rng.below(3);
            row_houses(region, n, 0, rng)
        }
        LotKind::Shop => shop(region, rng),
        LotKind::Apartments => {
            let f = 4 + rng.below(3);
            apartments(region, f, rng)
        }
        LotKind::Office => {
            let f = rng.pick(&[7usize, 9, 12]);
            office(region, f, rng)
        }
        LotKind::Gas => gas_station(region, rng),
        LotKind::Workshop => factory(region, rng),
        LotKind::Church => church(region, rng),
        LotKind::School => school(region, rng),
        LotKind::Fire => fire_station(region, rng),
        LotKind::Water => water_tower(region, rng),
        LotKind::Parking => parking(20.0, 16.0, rng),
        LotKind::Park => park(region, 26.0, 24.0, rng),
        LotKind::Sports => sports(region, 30.0, 20.0, rng),
        LotKind::Strip => strip_mall(region, rng),
        LotKind::BigBox => big_box(region, rng),
        LotKind::Warehouse => warehouse(region, rng),
        LotKind::FastFood => fast_food(region, rng),
        LotKind::Farm => farm(region, rng),
    }
}

/// Share of a lot's collision radius a model may use: the rest is the gap between neighbours.
pub const LOT_FIT: f32 = 0.97;

/// Shrink a model, if it needs it, so every part of it (fences, garages and eaves included) lies
/// inside the lot's circle of `radius` metres around its origin. Neighbouring lots never
/// overlap by that radius, so neighbouring buildings never touch or clip.
pub fn fit_to_lot(obj: Obj, radius: f32) -> Obj {
    let cap = radius * LOT_FIT;
    let far = obj
        .polys
        .iter()
        .flat_map(|p| p.pts.iter())
        .map(|v| v[0].hypot(v[1]))
        .fold(0.0f32, f32::max);
    if far <= cap || far < 1e-3 || cap <= 0.0 {
        return obj;
    }
    let k = cap / far;
    // less height lost than width, so a squeezed building still reads as a building
    let kz = 0.5 + 0.5 * k;
    crate::levels::scaled(&obj, k, k, kz)
}

/// The model for one lot of a town of this preset, in the preset's regional style and fitted to
/// the lot's collision radius (metres).
pub fn build_lot_for(preset: us_gen::Preset, kind: LotKind, radius: f32, rng: &mut Rng) -> Obj {
    fit_to_lot(build_lot_in(Region::of(preset), kind, rng), radius)
}

pub const ALL_KINDS: [LotKind; 19] = [
    LotKind::House,
    LotKind::Row,
    LotKind::Shop,
    LotKind::Apartments,
    LotKind::Office,
    LotKind::Gas,
    LotKind::Workshop,
    LotKind::Church,
    LotKind::School,
    LotKind::Fire,
    LotKind::Water,
    LotKind::Parking,
    LotKind::Park,
    LotKind::Sports,
    LotKind::Strip,
    LotKind::BigBox,
    LotKind::Warehouse,
    LotKind::FastFood,
    LotKind::Farm,
];

/// A tree colour with one of four tiny blue-channel steps picked from where the tree stands, so
/// autumn can give each tree its own leaf colour. Differences are invisible in summer.
pub fn tree_variant(col: Rgb, x: f32, y: f32) -> Rgb {
    let h = ((x * 7.0) as i32).wrapping_mul(31) ^ ((y * 13.0) as i32).wrapping_mul(17);
    [col[0], col[1], col[2] + (h & 3) as u8]
}
