//! Downtown buildings built to the lot line: street-wall blocks with varied podiums, setbacks,
//! storefront bays and roofs, three new tower profiles (straight slab, tapered, setback) whose
//! height follows the floor count the town's rule gives them, and the interim uses of vacant
//! downtown land. Everything is plain numbers and a seeded roll in [0, 1), so a lot always looks
//! the same. Local metres, front facing south (-y).
use crate::obj::{Finish, Obj, Rgb};
use crate::parts::{GLASS, GLASS_D, shade};
use crate::skyline::{RESIDENTIAL, RETAIL, material_finish, rect_at, tower_wall};

const STOREY: f32 = 3.5;
const CONCRETE: Rgb = [176, 173, 164];
const ROOF: Rgb = [79, 91, 103];
const DOOR_DARK: Rgb = [52, 58, 70];
const SLATE: Rgb = [66, 74, 88];
const PLANT: Rgb = [150, 148, 144];
const LEAF: Rgb = [88, 140, 72];
const PLINTHS: [Rgb; 6] = [
    CONCRETE,
    [145, 113, 99],
    [142, 152, 158],
    [120, 112, 104],
    [186, 168, 140],
    [96, 98, 104],
];
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
/// Shop signs: mostly dark boards, a few cream ones. No bright bands.
const FASCIAS: [Rgb; 7] = [
    [34, 44, 60],
    [96, 34, 40],
    [28, 66, 52],
    [52, 40, 32],
    [224, 216, 194],
    [60, 60, 66],
    [118, 84, 40],
];

/// A seeded value in [0, 1) from the roll and a slot number.
pub fn hv(roll: f32, k: u32) -> f32 {
    let x = libm::sinf(roll * 127.1 + k as f32 * 311.7) * 43_758.547;
    x - libm::floorf(x)
}

/// Storeys of the podium (the full-lot base, ground floor included) a tower stands on: 1 is a
/// tower that rises straight from a single shopfront floor on the sidewalk, up to 8.
pub fn podium_storeys(roll: f32) -> usize {
    const P: [usize; 10] = [1, 2, 2, 3, 3, 4, 5, 6, 6, 8];
    P[(hv(roll, 7) * P.len() as f32) as usize % P.len()]
}

/// Height of the roof of a podium of `storeys` storeys (ground floor included), as `street_block`
/// builds it.
pub fn podium_top(storeys: usize, residential: bool) -> f32 {
    let (g, storey) = if residential {
        (3.6, RESIDENTIAL)
    } else {
        (RETAIL, STOREY)
    };
    if storeys <= 1 {
        g + 0.1
    } else {
        g + (storeys - 1) as f32 * storey + 0.4
    }
}

fn block(o: &mut Obj, ring: &[[f32; 2]], z0: f32, z1: f32, wall: Rgb, finish: Finish) {
    crate::skyline::block(o, ring, z0, z1, wall, finish);
}

/// One side of a lot, as an edge `a` to `b` with the outside on its right: positions along it and
/// out from it become plan coordinates.
struct Edge {
    a: [f32; 2],
    u: [f32; 2],
    n: [f32; 2],
}

impl Edge {
    fn new(a: [f32; 2], b: [f32; 2]) -> Edge {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l = dx.hypot(dy).max(1e-6);
        Edge {
            a,
            u: [dx / l, dy / l],
            n: [dy / l, -dx / l],
        }
    }
    fn at(&self, t: f32, d: f32) -> [f32; 2] {
        [
            self.a[0] + self.u[0] * t + self.n[0] * d,
            self.a[1] + self.u[1] * t + self.n[1] * d,
        ]
    }
    /// An axis-aligned box from `t0` to `t1` along the edge and `d0` to `d1` out from it.
    #[allow(clippy::too_many_arguments)]
    fn bx(&self, o: &mut Obj, t: [f32; 2], d: [f32; 2], z: [f32; 2], col: Rgb) {
        let (p, q) = (self.at(t[0], d[0]), self.at(t[1], d[1]));
        o.bx(
            p[0].min(q[0]),
            p[1].min(q[1]),
            z[0],
            p[0].max(q[0]),
            p[1].max(q[1]),
            z[1],
            col,
        );
    }
    fn quad(&self, o: &mut Obj, t: [f32; 2], z: [f32; 2], col: Rgb) {
        o.wall_quad(self.at(t[0], 0.0), self.at(t[1], 0.0), z[0], z[1], col);
    }
}

/// The ground floor along one edge from `t0` to `t1`: separate bays, each its own shop (window,
/// awning, fascia board, blade sign) or an entrance with a canopy, divided by piers. Homes get
/// small windows, doors with a stoop and a canopy instead, and no signs.
#[allow(clippy::too_many_arguments)]
fn ground_floor(
    o: &mut Obj,
    e: &Edge,
    span: [f32; 2],
    g: f32,
    plinth: Rgb,
    roll: f32,
    seed: u32,
    homes: bool,
) {
    let pier = shade(plinth, 0.86);
    let mut t = span[0];
    let mut i = 0u32;
    while t < span[1] - 0.8 {
        let mut w = if homes {
            3.6 + 1.6 * hv(roll, seed + i)
        } else {
            3.0 + 3.4 * hv(roll, seed + i)
        };
        if span[1] - (t + w) < 2.4 {
            w = span[1] - t;
        }
        let (t0, t1) = (t, t + w);
        let mid = (t0 + t1) / 2.0;
        let pick = hv(roll, seed + 40 + i);
        e.bx(o, [t0 - 0.22, t0 + 0.22], [0.0, 0.2], [0.0, g], pier);
        if homes {
            e.quad(
                o,
                [mid - 0.7, mid + 0.7],
                [1.1, 2.6],
                if i.is_multiple_of(2) {
                    GLASS_D
                } else {
                    DOOR_DARK
                },
            );
            if pick > 0.5 {
                // a stoop and a canopy over the door
                e.quad(o, [mid - 0.55, mid + 0.55], [0.4, 2.3], DOOR_DARK);
                e.bx(o, [mid - 1.0, mid + 1.0], [0.0, 0.9], [0.0, 0.4], CONCRETE);
                e.bx(o, [mid - 1.0, mid + 1.0], [0.0, 0.7], [2.5, 2.65], pier);
            }
        } else if pick < 0.16 {
            // an entrance: a dark door set in a deep frame under a canopy
            e.quad(o, [mid - 1.0, mid + 1.0], [0.0, g * 0.66], DOOR_DARK);
            e.bx(
                o,
                [mid - 1.25, mid - 1.0],
                [0.0, 0.45],
                [0.0, g * 0.7],
                pier,
            );
            e.bx(
                o,
                [mid + 1.0, mid + 1.25],
                [0.0, 0.45],
                [0.0, g * 0.7],
                pier,
            );
            e.bx(
                o,
                [mid - 1.4, mid + 1.4],
                [0.0, 1.1],
                [g * 0.7, g * 0.77],
                shade(plinth, 0.7),
            );
        } else {
            e.quad(o, [t0 + 0.35, t1 - 0.35], [0.55, g * 0.66], GLASS_D);
            if pick < 0.6 {
                let col = AWNINGS[(hv(roll, seed + 80 + i) * 8.0) as usize % 8];
                let depth = 0.9 + 0.7 * hv(roll, seed + 120 + i);
                e.bx(
                    o,
                    [t0 + 0.2, t1 - 0.2],
                    [0.0, depth],
                    [g * 0.64, g * 0.7],
                    col,
                );
            } else if pick < 0.82 {
                let col = FASCIAS[(hv(roll, seed + 160 + i) * 7.0) as usize % 7];
                let half = (w * 0.5 - 0.5) * (0.55 + 0.4 * hv(roll, seed + 200 + i));
                e.bx(
                    o,
                    [mid - half, mid + half],
                    [0.0, 0.14],
                    [g * 0.76, g * 0.9],
                    col,
                );
            } else {
                // a blade sign hung from the pier
                let col = FASCIAS[(hv(roll, seed + 240 + i) * 7.0) as usize % 7];
                e.bx(
                    o,
                    [t1 - 0.5, t1 - 0.36],
                    [0.1, 1.2],
                    [g * 0.7, g * 0.97],
                    col,
                );
            }
        }
        t = t1;
        i += 1;
    }
    e.bx(
        o,
        [span[1] - 0.22, span[1] + 0.22],
        [0.0, 0.2],
        [0.0, g],
        pier,
    );
}

/// What sits on a roof of `r` (x0, y0, x1, y1) at height `z`, chosen by the roll: a plant room,
/// stepped parapet, rooftop tank, terrace, skylights or a pitched or mansard roof on a low
/// building.
fn roof_top(
    o: &mut Obj,
    r: [f32; 4],
    z: f32,
    floors: usize,
    masonry: bool,
    brick: bool,
    roll: f32,
) {
    let (x0, y0, x1, y1) = (r[0], r[1], r[2], r[3]);
    let (w, d) = (x1 - x0, y1 - y0);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let k = hv(roll, 9);
    if floors <= 3 && masonry && k < 0.34 {
        if k < 0.17 {
            o.gable(
                x0,
                y0,
                x1,
                y1,
                z,
                2.6 + 0.5 * hv(roll, 10),
                SLATE,
                shade(SLATE, 1.1),
                true,
                0.3,
            );
        } else {
            o.mansard(x0, y0, x1, y1, z, 3.0, 1.6, SLATE, shade(SLATE, 0.9));
            // dormers
            let n = ((w / 5.0) as usize).clamp(1, 4);
            for i in 0..n {
                let x = x0 + w * (i as f32 + 0.5) / n as f32;
                o.bx_top(
                    x - 0.7,
                    y0 + 0.5,
                    z + 0.4,
                    x + 0.7,
                    y0 + 1.3,
                    z + 1.9,
                    shade(SLATE, 1.15),
                    SLATE,
                );
                o.wall_quad(
                    [x - 0.5, y0 + 0.44],
                    [x + 0.5, y0 + 0.44],
                    z + 0.7,
                    z + 1.6,
                    GLASS_D,
                );
            }
        }
        return;
    }
    let mut kind = (k * 6.0) as usize;
    if kind == 1 && !masonry {
        kind = 3;
    }
    if kind == 2 && !(brick && floors <= 8) {
        kind = if hv(roll, 16) > 0.5 { 3 } else { 4 };
    }
    match kind {
        0 => {
            // a plant room set to one side, louvred, with a smaller box beside it
            let (bw, bd) = (
                w * (0.16 + 0.1 * hv(roll, 11)),
                d * (0.18 + 0.1 * hv(roll, 12)),
            );
            let ox = cx + (hv(roll, 13) - 0.5) * w * 0.4;
            o.bx_top(ox - bw, cy - bd, z, ox + bw, cy + bd, z + 2.6, PLANT, ROOF);
            o.wall_quad(
                [ox - bw * 0.8, cy - bd - 0.0],
                [ox + bw * 0.8, cy - bd],
                z + 0.6,
                z + 1.8,
                shade(PLANT, 0.55),
            );
            o.bx(
                x0 + w * 0.12,
                y1 - 2.2,
                z,
                x0 + w * 0.12 + 1.8,
                y1 - 0.6,
                z + 1.4,
                PLANT,
            );
        }
        1 => {
            // a stepped parapet along the front
            let steps = [(0.5, 0.7), (0.34, 1.3), (0.18, 1.9)];
            for (f, h) in steps {
                o.bx(
                    cx - w * f / 2.0 * 1.2,
                    y0,
                    z,
                    cx + w * f / 2.0 * 1.2,
                    y0 + 0.4,
                    z + h,
                    shade(CONCRETE, 1.02),
                );
            }
            o.bx(
                x0 + w * 0.2,
                cy,
                z,
                x0 + w * 0.2 + 2.0,
                cy + 1.6,
                z + 1.5,
                PLANT,
            );
        }
        2 => {
            // a wooden tank on legs, capped
            let (tx, ty) = (cx + (hv(roll, 14) - 0.5) * w * 0.5, cy + d * 0.1);
            for (lx, ly) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                o.bx(
                    tx + lx - 0.1,
                    ty + ly - 0.1,
                    z,
                    tx + lx + 0.1,
                    ty + ly + 0.1,
                    z + 1.4,
                    [90, 92, 96],
                );
            }
            o.prism(tx, ty, z + 1.4, z + 4.2, 1.7, [126, 98, 70], 10, None);
            o.pyramid(tx, ty, z + 4.2, 1.9, 1.1, [86, 70, 56], 10, 0.0);
        }
        3 => {
            // a roof terrace: a rail round the front half, planters and a pergola
            let (rx0, rx1, ry0, ry1) = (x0 + 0.8, x1 - 0.8, y0 + 0.5, cy);
            o.slab(rx0, ry0, z, rx1, ry1, z + 0.08, [150, 132, 108]);
            let n = ((rx1 - rx0) / 2.2) as usize;
            for i in 0..=n {
                let x = rx0 + (rx1 - rx0) * i as f32 / n.max(1) as f32;
                o.bx(
                    x - 0.04,
                    ry0,
                    z,
                    x + 0.04,
                    ry0 + 0.08,
                    z + 1.05,
                    [90, 94, 100],
                );
            }
            o.bx(rx0, ry0, z + 1.0, rx1, ry0 + 0.08, z + 1.08, [90, 94, 100]);
            for f in [0.18f32, 0.5, 0.82] {
                let x = rx0 + (rx1 - rx0) * f;
                o.bx(
                    x - 0.6,
                    ry0 + 0.4,
                    z,
                    x + 0.6,
                    ry0 + 1.0,
                    z + 0.55,
                    [120, 96, 80],
                );
                o.bx(
                    x - 0.5,
                    ry0 + 0.5,
                    z + 0.55,
                    x + 0.5,
                    ry0 + 0.9,
                    z + 0.95,
                    LEAF,
                );
            }
            let px = rx0 + (rx1 - rx0) * 0.62;
            o.bx(
                px,
                ry1 - 2.6,
                z + 2.3,
                px + 3.4,
                ry1 - 0.4,
                z + 2.4,
                [140, 112, 84],
            );
            for (a, b) in [(0.0, 0.0), (3.3, 0.0), (0.0, 2.1), (3.3, 2.1)] {
                o.bx(
                    px + a,
                    ry1 - 2.6 + b,
                    z,
                    px + a + 0.1,
                    ry1 - 2.5 + b,
                    z + 2.3,
                    [140, 112, 84],
                );
            }
        }
        4 => {
            // two or three skylights
            let n = 2 + (hv(roll, 15) * 2.0) as usize;
            for i in 0..n {
                let x = x0 + w * (i as f32 + 0.5) / n as f32;
                let s = (w / n as f32 * 0.28).clamp(0.8, 2.0);
                o.bx_top(
                    x - s,
                    cy - s * 0.8,
                    z,
                    x + s,
                    cy + s * 0.8,
                    z + 0.7,
                    shade(PLANT, 0.9),
                    GLASS,
                );
            }
            o.bx(x1 - 3.4, y1 - 2.8, z, x1 - 1.4, y1 - 0.8, z + 1.6, PLANT);
        }
        _ => {
            // a stair bulkhead and a pair of cooling units
            o.bx_top(
                x0 + w * 0.15,
                y1 - 3.2,
                z,
                x0 + w * 0.15 + 3.0,
                y1 - 0.6,
                z + 2.7,
                PLANT,
                ROOF,
            );
            for i in 0..2 {
                let x = cx + 1.0 + i as f32 * 2.6;
                o.bx(
                    x,
                    cy - 0.9,
                    z,
                    x + 1.9,
                    cy + 0.9,
                    z + 1.2,
                    shade(PLANT, 0.8),
                );
            }
        }
    }
}

/// A street-wall block that fills the lot to its line, `floors` storeys above a ground floor of
/// shopfront bays (homes get doors and stoops). Offices and shops are 3.5 m a floor, homes 3.0 m.
/// A tall block may step back once or twice, on the front or on several sides, at floors picked
/// by the roll, never at a fixed interval; a stone or brick wall gets string courses and
/// pilasters and the roof is one of several kinds. `hx` and `hy` are half the frontage and depth.
/// With `tower` the roof is left plain for the shaft that stands on it.
#[allow(clippy::too_many_arguments)]
pub fn street_block(
    hx: f32,
    hy: f32,
    floors: usize,
    residential: bool,
    pick: usize,
    plan: usize,
    roll: f32,
    tower: bool,
) -> Obj {
    let mut o = Obj::new();
    let storey = if residential { RESIDENTIAL } else { STOREY };
    let g = if residential { 3.6 } else { RETAIL };
    let wall = tower_wall(1.0, plan, pick);
    let finish = material_finish(pick, residential);
    let glass = finish == Finish::CurtainWall;
    let brick = matches!(pick % 10, 5 | 6);
    let plinth = PLINTHS[(plan + (hv(roll, 3) * 6.0) as usize) % PLINTHS.len()];
    block(
        &mut o,
        &rect_at(0.0, 0.0, hx, hy),
        0.0,
        g,
        plinth,
        Finish::Plain,
    );
    // the ground floor along the front, and a bay or two round each front corner
    let len = 2.0 * hx;
    let front = Edge::new([-hx, -hy], [hx, -hy]);
    ground_floor(
        &mut o,
        &front,
        [0.0, len],
        g,
        plinth,
        roll,
        100,
        residential,
    );
    let side = (2.0 * hy).min(8.0);
    let east = Edge::new([hx, -hy], [hx, hy]);
    ground_floor(
        &mut o,
        &east,
        [0.0, side],
        g,
        plinth,
        roll,
        300,
        residential,
    );
    let west = Edge::new([-hx, hy], [-hx, -hy]);
    ground_floor(
        &mut o,
        &west,
        [2.0 * hy - side, 2.0 * hy],
        g,
        plinth,
        roll,
        500,
        residential,
    );
    let mut z = g;
    let mut rect = [-hx, -hy, hx, hy];
    if floors == 0 {
        o.bx(
            -hx - 0.2,
            -hy - 0.3,
            g - 0.5,
            hx + 0.2,
            hy + 0.2,
            g + 0.1,
            CONCRETE,
        );
        return o;
    }
    // setbacks: none (a straight slab), one or two, at heights picked by the roll
    let want = if floors >= 9 && hv(roll, 1) > 0.6 {
        2
    } else if floors >= 6 && hv(roll, 1) > 0.3 {
        1
    } else {
        0
    };
    let mut cuts: Vec<usize> = Vec::new();
    if want >= 1 {
        let f1 = (floors as f32 * (0.4 + 0.42 * hv(roll, 2))).round() as usize;
        cuts.push(f1.clamp(2, floors - 2));
    }
    if want >= 2 {
        let f2 = cuts[0] + ((floors - cuts[0]) as f32 * (0.4 + 0.3 * hv(roll, 4))).round() as usize;
        if f2 >= cuts[0] + 2 && f2 < floors {
            cuts.push(f2);
        }
    }
    let masonry = !glass;
    let mut done = 0;
    let mut stage = 0;
    for &cut in cuts.iter().chain(std::iter::once(&floors)) {
        let top = z + (cut - done) as f32 * storey;
        block(
            &mut o,
            &rect_at(
                (rect[0] + rect[2]) / 2.0,
                (rect[1] + rect[3]) / 2.0,
                (rect[2] - rect[0]) / 2.0,
                (rect[3] - rect[1]) / 2.0,
            ),
            z,
            top,
            wall,
            finish,
        );
        let (w, d) = (rect[2] - rect[0], rect[3] - rect[1]);
        // string courses and pilasters on a masonry wall, not a glass one
        if masonry {
            let every = 2 + (hv(roll, 20 + stage) * 3.0) as usize;
            let mut f = done + every;
            while f < cut {
                let zz = z + (f - done) as f32 * storey - 0.2;
                o.bx(
                    rect[0] - 0.12,
                    rect[1] - 0.14,
                    zz,
                    rect[2] + 0.12,
                    rect[3] + 0.12,
                    zz + 0.4,
                    shade(CONCRETE, 1.04),
                );
                f += every;
            }
            if hv(roll, 30 + stage) > 0.45 {
                let bays = ((w / 4.4).round() as usize).max(2);
                for i in 0..=bays {
                    let x = rect[0] + w * i as f32 / bays as f32;
                    o.bx(
                        x - 0.24,
                        rect[1] - 0.18,
                        z,
                        x + 0.24,
                        rect[1],
                        top,
                        shade(wall, 0.9),
                    );
                }
            }
        }
        // cornice
        o.bx(
            rect[0] - 0.2,
            rect[1] - 0.3,
            top - 0.5,
            rect[2] + 0.2,
            rect[3] + 0.2,
            top + 0.4,
            CONCRETE,
        );
        z = top + 0.4;
        done = cut;
        stage += 1;
        if cut < floors {
            // step back: always on the front, on the sides by roll, by a depth of its own
            let dep = (1.3 + 2.6 * hv(roll, 40 + stage)).min(w.min(d) * 0.3);
            let sides = (hv(roll, 50 + stage) * 4.0) as usize;
            rect[1] += dep;
            if sides >= 1 {
                rect[0] += dep * 0.8;
            }
            if sides >= 2 {
                rect[2] -= dep * 0.8;
            }
            if sides >= 3 {
                rect[3] -= dep * 0.6;
            }
            // the roof of the lower stage is a terrace: a rail along the step
            o.bx(
                rect[0],
                rect[1] - 0.1,
                z,
                rect[2],
                rect[1],
                z + 0.9,
                [96, 100, 106],
            );
        }
    }
    if !tower {
        roof_top(&mut o, rect, z, floors, masonry, brick, roll);
    }
    o
}

/// A shaft profile for the three new tower classes. `kind` 0 is a straight slab with no setback,
/// 1 a tapered tower, 2 a tower with one to three setbacks of different depths at different
/// heights. `plan` (0 to 11) picks the proportions. The floor count is `scale` times a nominal 40
/// unless the town rule has set it with `skyline::with_tower`.
pub fn profile_tower(kind: usize, plan: usize, scale: f32, wall: Rgb) -> Obj {
    let (storey, forced) = crate::skyline::tower_rule();
    let n = forced
        .unwrap_or(((40.0 * scale).round() as usize).max(8))
        .max(8);
    let hw = [8.0, 9.5, 11.0, 13.0][plan % 4];
    let hd = [6.5, 9.0, 11.0][(plan / 4) % 3];
    let h = n as f32 * storey;
    let mut o = Obj::new();
    let glass = Finish::CurtainWall;
    let mut crown = h;
    match kind {
        0 => {
            let ring: Vec<[f32; 2]> = if plan % 3 == 1 {
                chamfer(hw, hd, 1.6)
            } else {
                rect_at(0.0, 0.0, hw, hd).to_vec()
            };
            block(&mut o, &ring, 0.0, h, wall, glass);
        }
        1 => {
            let straight = [0.0, 0.3, 0.45][plan % 3];
            let k = [0.52, 0.68, 0.82][(plan / 3) % 3];
            let z1 = h * straight;
            if z1 > 0.0 {
                block(&mut o, &rect_at(0.0, 0.0, hw, hd), 0.0, z1, wall, glass);
            }
            let lo = rect_at(0.0, 0.0, hw, hd);
            let hi = rect_at(0.0, 0.0, hw * k, hd * k);
            crate::skyline::taper(&mut o, &lo, &hi, z1, h, wall, glass);
        }
        _ => {
            let steps = (1 + plan % 3).min(n / 5);
            let mut rect = [-hw, -hd, hw, hd];
            let mut z = 0.0;
            for s in 0..steps {
                let f = [0.34, 0.58, 0.78][s] + 0.1 * hv(plan as f32 * 0.37 + 0.11, 60 + s as u32)
                    - 0.05;
                let zt = ((n as f32 * f).round() * storey)
                    .max(z + storey * 3.0)
                    .min(h - storey * 2.0);
                block(
                    &mut o,
                    &rect_at(
                        (rect[0] + rect[2]) / 2.0,
                        (rect[1] + rect[3]) / 2.0,
                        (rect[2] - rect[0]) / 2.0,
                        (rect[3] - rect[1]) / 2.0,
                    ),
                    z,
                    zt,
                    wall,
                    glass,
                );
                o.bx(
                    rect[0] - 0.2,
                    rect[1] - 0.2,
                    zt - 0.4,
                    rect[2] + 0.2,
                    rect[3] + 0.2,
                    zt,
                    CONCRETE,
                );
                z = zt;
                // each step in is a different depth, and on one side or several
                let dep = 1.4 + 0.9 * ((plan * 5 + s * 3) % 5) as f32;
                match (plan / 3 + s) % 3 {
                    0 => {
                        rect[0] += dep;
                        rect[2] -= dep;
                        rect[1] += dep * 0.6;
                        rect[3] -= dep * 0.6;
                    }
                    1 => {
                        rect[0] += dep * 1.4;
                    }
                    _ => {
                        rect[1] += dep;
                        rect[2] -= dep * 0.7;
                    }
                }
            }
            block(
                &mut o,
                &rect_at(
                    (rect[0] + rect[2]) / 2.0,
                    (rect[1] + rect[3]) / 2.0,
                    (rect[2] - rect[0]) / 2.0,
                    (rect[3] - rect[1]) / 2.0,
                ),
                z,
                h,
                wall,
                glass,
            );
            crown = h;
        }
    }
    // a crown parapet and a small plant box, never a spire
    let f = o.footprint();
    let (cx, cy) = ((f[0] + f[2]) / 2.0, (f[1] + f[3]) / 2.0);
    let _ = crown;
    let top = crate::levels::top_of(&o);
    o.bx(
        cx - 2.6,
        cy - 1.8,
        top,
        cx + 2.6,
        cy + 1.8,
        top + 2.2 + 0.6 * (plan % 3) as f32,
        ROOF,
    );
    o
}

fn chamfer(hw: f32, hd: f32, c: f32) -> Vec<[f32; 2]> {
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

/// What stands on vacant downtown land until it is built: a surface car park with marked bays, a
/// fenced empty lot or a small planned square, chosen by the roll. `r` is the lot radius.
pub fn vacant_lot(r: f32, roll: f32, rng: &mut crate::rng::Rng) -> Obj {
    let h = r * 0.88;
    let mut o = Obj::new();
    let k = hv(roll, 70);
    if k < 0.45 {
        // a car park: bays in rows with a few cars, kerb round it
        o = crate::kit_misc::parking(h * 2.0, h * 2.0, rng);
        return o;
    }
    if k < 0.8 {
        // a fenced empty lot: a post and rail fence on every side, a gate, a heap of spoil, a
        // few pallets and a site sign
        let post = [96, 100, 106];
        let rail = [140, 144, 150];
        let n = ((h * 2.0) / 3.0).round().max(2.0) as usize;
        for side in 0..4 {
            for i in 0..=n {
                let t = -h + 2.0 * h * i as f32 / n as f32;
                let (x, y) = match side {
                    0 => (t, -h),
                    1 => (h, t),
                    2 => (t, h),
                    _ => (-h, t),
                };
                o.bx(x - 0.06, y - 0.06, 0.0, x + 0.06, y + 0.06, 2.0, post);
            }
        }
        o.bx(-h, -h - 0.03, 1.8, h, -h + 0.03, 1.9, rail);
        o.bx(-h, h - 0.03, 1.8, h, h + 0.03, 1.9, rail);
        o.bx(-h - 0.03, -h, 1.8, -h + 0.03, h, 1.9, rail);
        o.bx(h - 0.03, -h, 1.8, h + 0.03, h, 1.9, rail);
        o.bx(-h, -h - 0.03, 0.3, h, -h + 0.03, 0.4, rail);
        let (mx, my) = (h * 0.3 * (hv(roll, 71) - 0.5) * 2.0, h * 0.2);
        o.pyramid(mx, my, 0.0, h * 0.35, 1.6, [152, 134, 108], 6, 0.0);
        for i in 0..3 {
            let px = -h * 0.5 + i as f32 * 1.7;
            o.bx(
                px,
                -h * 0.4,
                0.0,
                px + 1.2,
                -h * 0.4 + 1.0,
                0.45,
                [150, 112, 78],
            );
        }
        o.bx(-1.6, -h - 0.4, 0.0, 1.6, -h - 0.3, 0.2, [90, 94, 100]);
        o.bx(-1.4, -h - 0.45, 1.0, 1.4, -h - 0.35, 2.0, [210, 200, 170]);
        return o;
    }
    // a small planned square: a kerb, planted beds, benches, trees and a fountain
    let kerb = crate::roadpaint::KERB;
    o.bx(-h, -h, 0.0, h, -h + 0.5, 0.25, kerb);
    o.bx(-h, h - 0.5, 0.0, h, h, 0.25, kerb);
    o.bx(-h, -h, 0.0, -h + 0.5, h, 0.25, kerb);
    o.bx(h - 0.5, -h, 0.0, h, h, 0.25, kerb);
    o.prism(0.0, 0.0, 0.0, 0.7, (h * 0.26).max(1.6), kerb, 12, None);
    o.prism(
        0.0,
        0.0,
        0.7,
        0.78,
        (h * 0.22).max(1.4),
        [120, 168, 196],
        12,
        None,
    );
    o.prism(0.0, 0.0, 0.78, 2.0, 0.3, kerb, 8, None);
    for (sx, sy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let (bx, by) = (sx * h * 0.55, sy * h * 0.55);
        o.bx(
            bx - 1.6,
            by - 1.6,
            0.0,
            bx + 1.6,
            by + 1.6,
            0.4,
            [120, 96, 80],
        );
        o.bx(bx - 1.4, by - 1.4, 0.4, bx + 1.4, by + 1.4, 0.7, LEAF);
        o.bx(
            sx * h * 0.35 - 0.8,
            sy * h * 0.72 - 0.25,
            0.0,
            sx * h * 0.35 + 0.8,
            sy * h * 0.72 + 0.25,
            0.5,
            [140, 112, 84],
        );
    }
    let t = crate::kit::tree(2.0, 6.0, [88, 150, 76]);
    for (sx, sy) in [(-1.0f32, 0.0f32), (1.0, 0.0)] {
        o.add_placed(&t, sx * h * 0.8, sy, 0.0);
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_have_the_height_they_were_given() {
        for floors in [2usize, 5, 8, 12] {
            for roll in [0.07f32, 0.41, 0.66, 0.93] {
                let o = street_block(9.0, 9.0, floors, false, 5, 1, roll, false);
                let h = crate::levels::top_of(&o);
                let lo = RETAIL + floors as f32 * STOREY;
                assert!(
                    h >= lo && h < lo + 8.5,
                    "{floors} floors: {h} m, wanted {lo}+"
                );
                for v in o.polys.iter().flat_map(|p| &p.pts) {
                    assert!(v.iter().all(|c| c.is_finite()));
                    assert!(v[0].abs() <= 11.5 && v[1].abs() <= 11.5, "stays on the lot");
                }
            }
        }
    }

    #[test]
    fn profile_towers_are_finite_and_follow_the_floor_count() {
        for kind in 0..3 {
            for plan in 0..12 {
                let o = crate::skyline::with_tower(STOREY, Some(30), || {
                    profile_tower(kind, plan, 1.0, [150, 160, 170])
                });
                let h = crate::levels::top_of(&o);
                assert!(
                    h > 30.0 * STOREY && h < 30.0 * STOREY + 4.0,
                    "{kind}/{plan}: {h}"
                );
                assert!(o.polys.len() < 160);
            }
        }
    }

    #[test]
    fn podiums_vary_from_one_to_eight_storeys() {
        let mut seen = std::collections::BTreeSet::new();
        for i in 0..400 {
            seen.insert(podium_storeys(i as f32 / 400.0));
        }
        assert!(
            seen.contains(&1) && seen.contains(&8) && seen.len() >= 6,
            "{seen:?}"
        );
    }

    #[test]
    fn every_vacant_use_is_a_real_object() {
        for roll in [0.1f32, 0.5, 0.9] {
            let mut rng = crate::rng::Rng::new(3, 4);
            let o = vacant_lot(9.0, roll, &mut rng);
            assert!(o.polys.len() > 10);
            assert!(
                o.polys
                    .iter()
                    .flat_map(|p| &p.pts)
                    .all(|v| v.iter().all(|c| c.is_finite()))
            );
        }
    }
}
