//! Ten more skyline silhouettes (families 13 to 22 of the architecture list), the street-level
//! base every downtown tower stands on (shopfronts, awnings, signs), and the district colour
//! palettes. Everything is drawn from plain numbers and the seeded plan, so a lot always looks the
//! same. Local metres, front facing south (-y), like every other model.
use crate::obj::{Finish, Obj, Rgb};
use crate::parts::{GLASS, GLASS_D, shade};

pub const FIRST: usize = 13;
pub const NAMES: [&str; 10] = [
    "Slab blocks",
    "Point towers",
    "Stepped deco spires",
    "Cylinder towers",
    "L-plan towers",
    "U-plan courtyard blocks",
    "Flatiron wedges",
    "Tapered needles",
    "Crowned towers",
    "Sky-bridge pairs",
];

const STOREY: f32 = 3.5;
/// Ground-floor retail height, metres (the shared downtown rule says 4.5 to 5).
pub const RETAIL: f32 = 4.8;
/// Residential upper floors, metres (offices and shops use `STOREY`).
pub const RESIDENTIAL: f32 = 3.0;

thread_local! {
    /// Set while a tower shaft is built to stand on a full-lot podium: its own plinth is skipped.
    static NO_BASE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Upper storey height of the tower being built.
    static STOREY_M: std::cell::Cell<f32> = const { std::cell::Cell::new(STOREY) };
}

/// Run `f` with the tower plinth switched off and the given storey height.
pub fn without_base<T>(storey: f32, f: impl FnOnce() -> T) -> T {
    NO_BASE.with(|b| b.set(true));
    STOREY_M.with(|b| b.set(storey));
    let out = f();
    NO_BASE.with(|b| b.set(false));
    STOREY_M.with(|b| b.set(STOREY));
    out
}
const CONCRETE: Rgb = [176, 173, 164];
const ROOF: Rgb = [79, 91, 103];
const COPPER: Rgb = [96, 156, 136];
const DOOR_DARK: Rgb = [52, 58, 70];

const AWNINGS: [Rgb; 8] = [
    [168, 48, 44],
    [44, 92, 132],
    [38, 104, 76],
    [196, 150, 52],
    [78, 60, 110],
    [210, 206, 196],
    [150, 58, 40],
    [52, 56, 66],
];
const SIGNS: [Rgb; 6] = [
    [244, 226, 160],
    [236, 236, 232],
    [250, 190, 96],
    [150, 214, 220],
    [240, 150, 130],
    [200, 232, 170],
];

pub fn rect_at(cx: f32, cy: f32, hw: f32, hd: f32) -> [[f32; 2]; 4] {
    [
        [cx - hw, cy - hd],
        [cx + hw, cy - hd],
        [cx + hw, cy + hd],
        [cx - hw, cy + hd],
    ]
}

fn circle(r: f32, n: usize) -> Vec<[f32; 2]> {
    (0..n)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            [r * libm::cosf(a), r * libm::sinf(a)]
        })
        .collect()
}

fn chamfered(hw: f32, hd: f32, c: f32) -> Vec<[f32; 2]> {
    vec![
        [-hw + c, -hd],
        [hw - c, -hd],
        [hw, -hd + c],
        [hw, hd - c],
        [hw - c, hd],
        [-hw + c, hd],
        [-hw, hd - c],
        [-hw, -hd + c],
    ]
}

/// A convex block with a facade finish on its walls and a membrane roof.
fn block(obj: &mut Obj, ring: &[[f32; 2]], z0: f32, z1: f32, wall: Rgb, finish: Finish) {
    let first = obj.polys.len();
    obj.extrude(ring, z0, z1, wall, ROOF);
    for face in &mut obj.polys[first..first + ring.len()] {
        face.finish = finish;
    }
    obj.polys[first + ring.len()].finish = Finish::Membrane;
}

/// A tapering block, `lo` at `z0` to `hi` at `z1`.
fn taper(obj: &mut Obj, lo: &[[f32; 2]], hi: &[[f32; 2]], z0: f32, z1: f32, wall: Rgb, f: Finish) {
    let first = obj.polys.len();
    obj.loft(lo, z0, hi, z1, wall, Some(ROOF));
    for face in &mut obj.polys[first..first + lo.len()] {
        face.finish = f;
    }
    obj.polys[first + lo.len()].finish = Finish::Membrane;
}

fn bevel_ring(hw: f32, hd: f32, c: f32) -> Vec<[f32; 2]> {
    chamfered(hw, hd, c)
}

/// Street-level base of a downtown building: a stone plinth block of `storeys`, with glass
/// shopfronts along the front and both sides, a striped awning over each front bay, a sign band
/// and a door. Cheap: a handful of flat panels per bay.
#[allow(clippy::too_many_arguments)]
pub fn base(
    obj: &mut Obj,
    cx: f32,
    cy: f32,
    hw: f32,
    hd: f32,
    storeys: usize,
    plan: usize,
    wall: Rgb,
) {
    base_with(obj, cx, cy, hw, hd, storeys, STOREY, plan, wall);
}

/// `base` with its own storey height.
#[allow(clippy::too_many_arguments)]
pub fn base_with(
    obj: &mut Obj,
    cx: f32,
    cy: f32,
    hw: f32,
    hd: f32,
    storeys: usize,
    storey: f32,
    plan: usize,
    wall: Rgb,
) {
    if NO_BASE.with(|b| b.get()) {
        return;
    }
    shopfront_base(obj, cx, cy, hw, hd, storeys, storey, plan, wall, true);
}

/// The plinth itself: `storeys` of `storey` metres, shopfront glass, awnings and signs on the
/// front (and the sides when `sides`), scaled to the storey height.
#[allow(clippy::too_many_arguments)]
fn shopfront_base(
    obj: &mut Obj,
    cx: f32,
    cy: f32,
    hw: f32,
    hd: f32,
    storeys: usize,
    storey: f32,
    plan: usize,
    wall: Rgb,
    sides: bool,
) {
    let top = storeys as f32 * storey;
    // the ground floor is `g` tall: glass, awning and sign sit in proportion
    let g = storey.max(3.2);
    let (gl, aw0, aw1, sg0, sg1) = (g * 0.62, g * 0.64, g * 0.71, g * 0.74, g * 0.9);
    block(obj, &rect_at(cx, cy, hw, hd), 0.0, top, wall, Finish::Loft);
    let front = cy - hd;
    let n = ((2.0 * hw) / 3.6).floor().max(1.0) as usize;
    let step = 2.0 * hw / n as f32;
    let awn = AWNINGS[(plan * 3 + n) % AWNINGS.len()];
    let alt = AWNINGS[(plan * 3 + n + 3) % AWNINGS.len()];
    for i in 0..n {
        let x0 = cx - hw + i as f32 * step;
        let x1 = x0 + step;
        let glass = if i % 3 == 1 { DOOR_DARK } else { GLASS_D };
        obj.wall_quad([x0 + 0.3, front], [x1 - 0.3, front], 0.2, gl, glass);
        let col = if (i + plan).is_multiple_of(2) {
            awn
        } else {
            alt
        };
        obj.bx(
            x0 + 0.15,
            front - 1.5,
            aw0,
            x1 - 0.15,
            front - 0.05,
            aw1,
            col,
        );
        obj.wall_quad(
            [x0 + 0.5, front - 0.02],
            [x1 - 0.5, front - 0.02],
            sg0,
            sg1,
            SIGNS[(i + plan) % SIGNS.len()],
        );
    }
    // the two sides get shopfront glass only
    for sx in [cx - hw, cx + hw].into_iter().filter(|_| sides) {
        let m = ((2.0 * hd) / 4.0).floor().max(1.0) as usize;
        let st = 2.0 * hd / m as f32;
        for j in 0..m {
            let y0 = cy - hd + j as f32 * st + 0.4;
            let y1 = y0 + st - 0.8;
            if sx < cx {
                obj.wall_quad([sx, y1], [sx, y0], 0.2, gl, GLASS_D);
            } else {
                obj.wall_quad([sx, y0], [sx, y1], 0.2, gl, GLASS_D);
            }
        }
    }
    // a cornice line at the top of the plinth
    obj.bx(
        cx - hw - 0.25,
        cy - hd - 0.25,
        top - 0.35,
        cx + hw + 0.25,
        cy + hd + 0.25,
        top,
        CONCRETE,
    );
}

fn floors(base: usize, scale: f32, level: u8) -> usize {
    let k = match level {
        0..=3 => 0.3,
        4 => 0.58,
        _ => 1.0,
    };
    ((base as f32 * scale * k).round() as usize).max(8)
}

fn pilaster(obj: &mut Obj, x: f32, y: f32, z0: f32, z1: f32) {
    obj.bx(x - 0.3, y - 0.3, z0, x + 0.3, y + 0.3, z1, CONCRETE);
}

fn mast(obj: &mut Obj, x: f32, y: f32, z: f32, h: f32) {
    obj.bx(
        x - 0.12,
        y - 0.12,
        z,
        x + 0.12,
        y + 0.12,
        z + h,
        [150, 150, 154],
    );
}

/// Build skyline family `idx` (0 to 9). `scale` stretches the floor count (the height falloff
/// from the centre); `level` below 5 gives the mid-rise version.
pub fn build(idx: usize, plan: usize, level: u8, scale: f32, wall: Rgb) -> Obj {
    let mut o = Obj::new();
    let s = STOREY_M.with(|b| b.get());
    let hand = if plan.is_multiple_of(2) { 1.0 } else { -1.0 };
    let podium_wall = [CONCRETE, [145, 113, 99], [142, 152, 158], [120, 112, 104]][plan % 4];
    match idx {
        0 => {
            // slab: a long thin blade, turned a quarter every other plan, on a broad low wing
            let n = floors(28 + plan % 5, scale, level);
            let hw = 10.0 + (plan % 3) as f32;
            let mut tw = Obj::new();
            block(
                &mut tw,
                &rect_at(0.0, 0.0, hw, 3.4),
                0.0,
                n as f32 * s,
                wall,
                Finish::CurtainWall,
            );
            for k in 0..3 {
                let x = -hw * 0.6 + k as f32 * hw * 0.6;
                tw.bx(
                    x - 1.2,
                    -1.0,
                    n as f32 * s,
                    x + 1.2,
                    1.0,
                    n as f32 * s + 2.4,
                    ROOF,
                );
            }
            let turn = if plan % 2 == 1 {
                std::f32::consts::FRAC_PI_2
            } else {
                0.0
            };
            o.add_placed(&tw, 0.0, if turn == 0.0 { 2.0 } else { 0.0 }, turn);
            base(&mut o, 0.0, -3.5, 9.5, 5.0, 2, plan, podium_wall);
        }
        1 => {
            // point tower: a slim chamfered shaft with one setback and a thin spire
            let n = floors(50 + plan % 7, scale, level);
            let hw = 4.2 + (plan % 3) as f32 * 0.4;
            let lower = (n as f32 * 0.82) as usize;
            block(
                &mut o,
                &bevel_ring(hw, hw, 1.3),
                0.0,
                lower as f32 * s,
                wall,
                Finish::RibbedGlass,
            );
            block(
                &mut o,
                &bevel_ring(hw * 0.78, hw * 0.78, 1.1),
                lower as f32 * s,
                n as f32 * s,
                wall,
                Finish::RibbedGlass,
            );
            let top = n as f32 * s;
            o.bx(-1.5, -1.5, top, 1.5, 1.5, top + 3.0, ROOF);
            mast(&mut o, 0.0, 0.0, top + 3.0, 9.0 + (plan % 3) as f32 * 3.0);
            base(&mut o, 0.0, -1.0, 8.5, 7.0, 3, plan, podium_wall);
        }
        2 => {
            // stepped deco: four setbacks, corner piers and a crown with a spire
            let n = floors(40 + plan % 5, scale, level);
            let hw = 8.8;
            let cuts = [0usize, n * 38 / 100, n * 62 / 100, n * 82 / 100, n];
            let mut half = hw;
            for t in 0..4 {
                let k = [1.0, 0.8, 0.6, 0.4][t];
                half = hw * k;
                let (z0, z1) = (cuts[t] as f32 * s, cuts[t + 1] as f32 * s);
                block(
                    &mut o,
                    &rect_at(0.0, 0.0, half, half * 0.8),
                    z0.max(2.0 * s * f32::from(t == 0)),
                    z1,
                    wall,
                    Finish::Deco,
                );
                for (px, py) in [
                    (-half, -half * 0.8),
                    (half, -half * 0.8),
                    (-half, half * 0.8),
                    (half, half * 0.8),
                ] {
                    pilaster(&mut o, px, py, z0, z1 + 1.6);
                }
            }
            let top = n as f32 * s;
            let ring = bevel_ring(half * 0.55, half * 0.45, 0.6);
            let tip = bevel_ring(half * 0.12, half * 0.1, 0.1);
            o.loft(&ring, top, &tip, top + 9.0, shade(wall, 1.08), Some(ROOF));
            mast(&mut o, 0.0, 0.0, top + 9.0, 8.0);
            base(
                &mut o,
                0.0,
                0.0,
                hw + 1.0,
                hw * 0.8 + 1.0,
                2,
                plan,
                podium_wall,
            );
        }
        3 => {
            // cylinder: a round ribbed shaft, a narrower drum on top, a ring plinth
            let n = floors(38 + plan % 5, scale, level);
            let r = 6.6 + (plan % 3) as f32 * 0.5;
            let lower = n * 3 / 4;
            block(
                &mut o,
                &circle(r, 16),
                0.0,
                lower as f32 * s,
                wall,
                Finish::RibbedGlass,
            );
            block(
                &mut o,
                &circle(r * 0.82, 16),
                lower as f32 * s,
                n as f32 * s,
                wall,
                Finish::RibbedGlass,
            );
            let top = n as f32 * s;
            block(
                &mut o,
                &circle(r * 0.5, 12),
                top,
                top + 4.5,
                ROOF,
                Finish::Plain,
            );
            o.prism(
                0.0,
                0.0,
                top + 4.5,
                top + 12.0,
                0.2,
                [150, 150, 154],
                6,
                None,
            );
            base(&mut o, 0.0, -0.8, 9.0, 8.0, 2, plan, podium_wall);
        }
        4 => {
            // L plan: a tall short arm and a lower long arm
            let tall = floors(34 + plan % 4, scale, level);
            let low = (tall * 2 / 3).max(8);
            block(
                &mut o,
                &rect_at(hand * 5.0, 1.0, 4.6, 8.0),
                0.0,
                tall as f32 * s,
                wall,
                Finish::CurtainWall,
            );
            block(
                &mut o,
                &rect_at(-hand * 1.6, 6.4, 8.4, 3.6),
                0.0,
                low as f32 * s,
                shade(wall, 0.94),
                Finish::CurtainWall,
            );
            let top = tall as f32 * s;
            o.bx(
                hand * 5.0 - 1.8,
                -1.0,
                top,
                hand * 5.0 + 1.8,
                2.0,
                top + 3.0,
                ROOF,
            );
            base(&mut o, 0.0, 0.0, 10.5, 9.5, 2, plan, podium_wall);
        }
        5 => {
            // U plan: three wings round an open court that faces the street
            let n = floors(27 + plan % 3, scale, level);
            let h = n as f32 * s;
            block(
                &mut o,
                &rect_at(0.0, 6.0, 9.5, 3.2),
                0.0,
                h,
                wall,
                Finish::Balconies,
            );
            for sx in [-6.8, 6.8] {
                block(
                    &mut o,
                    &rect_at(sx, 0.2, 2.7, 5.0),
                    0.0,
                    h * 0.86,
                    shade(wall, 0.96),
                    Finish::Balconies,
                );
            }
            base(&mut o, 0.0, 3.0, 10.0, 8.0, 1, plan, podium_wall);
            o.slab(-4.2, -5.0, 0.0, 4.2, 2.8, 0.22, [150, 156, 140]);
            o.bx(-1.0, 4.0, h, 1.0, 7.0, h + 3.0, ROOF);
        }
        6 => {
            // flatiron wedge: a trapezoid plan, narrow at the back, with a dark lantern
            let n = floors(30 + plan % 4, scale, level);
            let h = n as f32 * s;
            let ring = [[-10.0, -5.5], [10.0, -5.5], [3.6, 8.0], [-3.6, 8.0]];
            block(&mut o, &ring, 0.0, h, wall, Finish::DarkFacade);
            let lantern = [[-5.0, -3.0], [5.0, -3.0], [2.4, 5.0], [-2.4, 5.0]];
            block(
                &mut o,
                &lantern,
                h,
                h + 6.0,
                shade(wall, 0.8),
                Finish::CurtainWall,
            );
            o.bx(-2.0, 0.0, h + 6.0, 2.0, 3.0, h + 8.2, ROOF);
            base(&mut o, 0.0, 1.0, 10.5, 7.5, 2, plan, podium_wall);
        }
        7 => {
            // tapered needle: the whole shaft leans in toward a crown and spire
            let n = floors(48 + plan % 6, scale, level);
            let h = n as f32 * s;
            let lo = chamfered(7.6, 6.6, 1.6);
            let hi = chamfered(3.4, 3.0, 0.9);
            taper(&mut o, &lo, &hi, 2.0 * s, h, wall, Finish::CurtainWall);
            block(
                &mut o,
                &chamfered(2.4, 2.1, 0.6),
                h,
                h + 5.0,
                ROOF,
                Finish::Plain,
            );
            mast(&mut o, 0.0, 0.0, h + 5.0, 12.0 + (plan % 4) as f32 * 3.0);
            base(&mut o, 0.0, 0.0, 9.5, 8.2, 2, plan, podium_wall);
        }
        8 => {
            // crowned: a stone shaft with two setbacks, a copper mansard and a flagpole
            let n = floors(31 + plan % 5, scale, level);
            let a = n * 70 / 100;
            block(
                &mut o,
                &rect_at(0.0, 0.0, 8.4, 6.8),
                2.0 * s,
                a as f32 * s,
                wall,
                Finish::Deco,
            );
            block(
                &mut o,
                &rect_at(0.0, 0.0, 6.6, 5.4),
                a as f32 * s,
                n as f32 * s,
                wall,
                Finish::Deco,
            );
            let h = n as f32 * s;
            let lo = rect_at(0.0, 0.0, 6.9, 5.7);
            let hi = rect_at(0.0, 0.0, 3.4, 2.6);
            taper(&mut o, &lo, &hi, h, h + 8.0, COPPER, Finish::Plain);
            o.bx(-1.2, -1.2, h + 8.0, 1.2, 1.2, h + 10.5, [236, 214, 130]);
            mast(&mut o, 0.0, 0.0, h + 10.5, 8.0);
            base(&mut o, 0.0, 0.0, 9.6, 8.0, 2, plan, podium_wall);
        }
        _ => {
            // sky-bridge pair: two shafts of different height joined by a glazed bridge
            let n = floors(36 + plan % 4, scale, level);
            let h = n as f32 * s;
            let h2 = ((n as f32) * 0.8).round() * s;
            block(
                &mut o,
                &bevel_ring(4.4, 5.0, 0.9),
                0.0,
                h,
                wall,
                Finish::CurtainWall,
            );
            let mut twin = Obj::new();
            block(
                &mut twin,
                &bevel_ring(4.4, 5.0, 0.9),
                0.0,
                h2,
                shade(wall, 0.92),
                Finish::CurtainWall,
            );
            o.add_placed(&twin, hand * 9.6, 0.0, 0.0);
            let bz = (n as f32 * 0.62).round() * s;
            let (bx0, bx1) = if hand > 0.0 { (4.4, 5.2) } else { (-5.2, -4.4) };
            o.bx(bx0, -1.8, bz, bx1, 1.8, bz + 2.0 * s, GLASS);
            o.bx(
                bx0,
                -2.0,
                bz + 2.0 * s,
                bx1,
                2.0,
                bz + 2.0 * s + 0.4,
                CONCRETE,
            );
            o.bx(-1.8, -1.8, h, 1.8, 1.8, h + 3.0, ROOF);
            base(&mut o, hand * 4.8, 0.0, 14.4, 5.8, 2, plan, podium_wall);
        }
    }
    o
}

// ---- downtown utilities ----

/// A water or power service restyled to stand downtown: a brick waterworks with a banded tank on
/// its roof, or a brick substation with a row of transformers. No legs, no stacks.
pub fn downtown_utility(power: bool) -> Obj {
    let mut o = Obj::new();
    let brick: Rgb = [140, 78, 62];
    let (hw, hd) = (8.0, 6.0);
    let top = if power { 6.5 } else { 10.5 };
    block(
        &mut o,
        &rect_at(0.0, 0.0, hw, hd),
        0.0,
        top,
        brick,
        Finish::Loft,
    );
    for k in 0..4 {
        let x0 = -hw + 1.4 + k as f32 * 3.8;
        let n = if power { 1 } else { 3 };
        for f in 0..n {
            let z = 1.4 + f as f32 * 3.1;
            o.wall_quad(
                [x0, -hd - 0.02],
                [x0 + 2.0, -hd - 0.02],
                z,
                z + 2.0,
                GLASS_D,
            );
        }
    }
    o.bx(
        -hw - 0.3,
        -hd - 0.3,
        top - 0.4,
        hw + 0.3,
        hd + 0.3,
        top,
        CONCRETE,
    );
    if power {
        for k in 0..4 {
            let x = -hw + 2.0 + k as f32 * 4.2;
            o.bx(
                x - 1.0,
                hd + 0.8,
                0.0,
                x + 1.0,
                hd + 2.6,
                2.6,
                [128, 134, 140],
            );
            o.bx(
                x - 0.3,
                hd + 1.4,
                2.6,
                x + 0.3,
                hd + 2.0,
                3.4,
                [190, 190, 196],
            );
        }
    } else {
        // a wooden banded tank on the roof, capped
        let ring = circle(2.7, 10);
        block(&mut o, &ring, top, top + 4.4, [120, 92, 66], Finish::Plain);
        o.pyramid(0.0, 0.0, top + 4.4, 3.0, 1.8, ROOF, 10, 0.0);
        o.bx(-3.4, -0.2, top, 3.4, 0.2, top + 0.4, [90, 92, 96]);
    }
    o
}

// ---- street-wall blocks and wall materials ----

/// What a wall material pick (0 to 9, see `TOWER_WALLS`) is made of, as the facade finish that
/// shows it: blue glass keeps the glass curtain wall, the rest are punched-window finishes.
pub fn material_finish(pick: usize, residential: bool) -> Finish {
    match pick % TOWER_WALLS.len() {
        0..=2 => Finish::CurtainWall,
        3 | 9 => Finish::Deco,
        4..=6 => Finish::Loft,
        7 => {
            if residential {
                Finish::Balconies
            } else {
                Finish::Loft
            }
        }
        _ => Finish::DarkFacade,
    }
}

/// Re-skin a finished tower in material `pick`: every facade finish becomes the material's.
pub fn apply_material(o: &mut Obj, pick: usize, residential: bool) {
    let f = material_finish(pick, residential);
    if f == Finish::CurtainWall {
        return;
    }
    for p in &mut o.polys {
        if p.finish.has_facade() && p.finish != Finish::DarkFacade {
            p.finish = f;
        }
    }
}

/// A mid-rise street-wall block that fills the lot to its line: retail ground floor with
/// shopfronts, `floors` upper storeys of the material, a cornice, a parapet and a rooftop box.
/// `hx` and `hy` are half the frontage and depth in metres. Neighbours join into a street wall.
#[allow(clippy::too_many_arguments)]
pub fn street_block(
    hx: f32,
    hy: f32,
    floors: usize,
    residential: bool,
    pick: usize,
    plan: usize,
    roll: f32,
) -> Obj {
    let mut o = Obj::new();
    let storey = if residential { RESIDENTIAL } else { STOREY };
    let wall = tower_wall(1.0, plan, pick);
    let plinth = [CONCRETE, [145, 113, 99], [142, 152, 158], [120, 112, 104]][plan % 4];
    shopfront_base(&mut o, 0.0, 0.0, hx, hy, 1, RETAIL, plan, plinth, false);
    // shopfront glass on the sides too, but only the corner bay: neighbours hide the rest
    let top = RETAIL + floors as f32 * storey;
    block(
        &mut o,
        &rect_at(0.0, 0.0, hx, hy),
        RETAIL,
        top,
        wall,
        material_finish(pick, residential),
    );
    // pilasters on the front, a cornice at the top and a parapet round the roof
    let bays = ((2.0 * hx) / 4.2).round().max(2.0) as usize;
    let step = 2.0 * hx / bays as f32;
    for i in 0..=bays {
        let x = -hx + i as f32 * step;
        o.bx(
            x - 0.25,
            -hy - 0.2,
            RETAIL,
            x + 0.25,
            -hy,
            top,
            shade(wall, 0.9),
        );
    }
    o.bx(
        -hx - 0.2,
        -hy - 0.3,
        top - 0.5,
        hx + 0.2,
        hy + 0.2,
        top + 0.4,
        CONCRETE,
    );
    let mut roof = top + 0.4;
    // a third of the taller blocks step back for their top two floors, a terrace in front
    if floors >= 8 && roll > 0.62 {
        let (ix, iy) = (hx - 1.6, hy - 1.6);
        let up = 2.0 * storey;
        block(
            &mut o,
            &rect_at(0.0, 0.0, ix, iy),
            top + 0.4,
            top + 0.4 + up,
            shade(wall, 1.05),
            material_finish(pick, residential),
        );
        roof = top + 0.4 + up;
    }
    // rooftop plant: absent on some, small and offset on the rest
    let k = ((roll * 53.0) as usize) % 4;
    if k != 0 {
        let (bw, bd) = (hx * (0.14 + 0.07 * k as f32), hy * (0.13 + 0.05 * k as f32));
        let ox = (k as f32 - 2.0) * hx * 0.25;
        o.bx(
            ox - bw,
            -bd,
            roof,
            ox + bw,
            bd,
            roof + 1.6 + 0.4 * k as f32,
            ROOF,
        );
    }
    o
}

// ---- district palettes ----

/// Old-core brick, from red to brown.
const OLD_BRICK: [Rgb; 6] = [
    [158, 78, 62],
    [140, 70, 58],
    [170, 96, 72],
    [118, 82, 72],
    [150, 100, 80],
    [108, 68, 60],
];
/// Downtown walls for the low and mid-rise buildings: stone, concrete and a little blue glass.
const DOWNTOWN: [Rgb; 8] = [
    [208, 200, 180],
    [176, 173, 164],
    [196, 170, 128],
    [118, 134, 152],
    [150, 150, 146],
    [92, 94, 100],
    [190, 184, 170],
    [158, 122, 100],
];
/// Tower wall materials, picked by index: three in ten blue glass, the rest limestone,
/// sandstone, brick, concrete and dark granite.
const TOWER_WALLS: [Rgb; 10] = [
    [96, 122, 150],
    [110, 138, 168],
    [84, 108, 136],
    [208, 200, 180],
    [196, 170, 128],
    [158, 78, 62],
    [118, 82, 72],
    [176, 173, 164],
    [92, 94, 100],
    [190, 184, 170],
];
/// Industrial and warehouse district: sooty brick and concrete.
const WORKS: [Rgb; 5] = [
    [128, 96, 84],
    [150, 146, 140],
    [116, 108, 104],
    [164, 150, 130],
    [110, 118, 124],
];

/// The wall colour of a tower: material `pick` (0 to 9) of `TOWER_WALLS`, shaded a touch by plan
/// so two towers of one material are not the same tone.
pub fn tower_wall(_density: f32, plan: usize, pick: usize) -> Rgb {
    let base = TOWER_WALLS[pick % TOWER_WALLS.len()];
    shade(base, 0.94 + (plan % 4) as f32 * 0.03)
}

/// Pull a finished lot's body colours into its district's palette: brick reds and browns in the
/// old core, greys and blue glass downtown, soot and concrete for works. Houses and rowhouses are
/// left alone, they keep their paint. `roll` is a seeded value in [0, 1).
pub fn recolour(o: &mut Obj, kind: us_gen::LotKind, density: f32, downtown: bool, roll: f32) {
    use us_gen::LotKind as K;
    let palette: &[Rgb] = match kind {
        K::House => return,
        K::Row if density < 0.5 && !downtown => return,
        K::Workshop | K::Warehouse => &WORKS,
        K::Shop | K::Office | K::Apartments | K::Row | K::Strip | K::BigBox | K::FastFood => {
            if !downtown && density < 0.45 {
                return;
            }
            if density >= 0.85 || (downtown && roll > 0.55) {
                &DOWNTOWN
            } else {
                &OLD_BRICK
            }
        }
        _ => return,
    };
    // the two biggest wall colours (not glass, not roofs) become palette picks
    let mut counts: Vec<(Rgb, usize)> = Vec::new();
    for p in &o.polys {
        if p.color == GLASS || p.color == GLASS_D || p.flat || p.roof {
            continue;
        }
        match counts.iter_mut().find(|c| c.0 == p.color) {
            Some(c) => c.1 += 1,
            None => counts.push((p.color, 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let Some(&(body, _)) = counts.first() else {
        return;
    };
    let pick = palette[((roll * 997.0) as usize) % palette.len()];
    let lum = |c: Rgb| (c[0] as u32 * 3 + c[1] as u32 * 6 + c[2] as u32) / 10;
    let floor = (counts[0].1 / 3).max(6);
    let majors: Vec<Rgb> = counts
        .iter()
        .skip(1)
        .filter(|c| c.1 >= floor && saturated(c.0) && lum(c.0) < 200)
        .map(|c| c.0)
        .collect();
    for p in &mut o.polys {
        if p.color == body {
            p.color = pick;
        } else if majors.contains(&p.color) {
            p.color = shade(pick, 1.07);
        }
    }
}

fn saturated(c: Rgb) -> bool {
    let (hi, lo) = (c.iter().max().unwrap(), c.iter().min().unwrap());
    hi - lo > 28
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_skyline_family_is_finite_tall_and_distinct() {
        let mut shapes = Vec::new();
        for idx in 0..NAMES.len() {
            let o = build(idx, 3, 5, 1.0, [150, 160, 170]);
            assert!(o.polys.len() < 400, "{idx}: {} polys", o.polys.len());
            assert!(crate::levels::top_of(&o) > 88.0, "{idx} is a tower");
            for v in o.polys.iter().flat_map(|p| &p.pts) {
                assert!(v.iter().all(|c| c.is_finite()));
                assert!(v[0].hypot(v[1]) < 22.0, "{idx} stays inside a lot");
            }
            let f = o.footprint();
            shapes.push(((f[2] - f[0]) as i32, (f[3] - f[1]) as i32, o.polys.len()));
        }
        shapes.sort();
        shapes.dedup();
        assert_eq!(shapes.len(), NAMES.len(), "silhouettes differ");
    }
}
