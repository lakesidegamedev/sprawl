//! Extra relief for the newer styles: red rock formations and a high coast. The five 1.0 styles
//! never reach any of this code (`RockCoast::None`), so their ground stays exactly as it was.

use crate::geom::{Pt, atan2, cos, pt, sin};
use crate::rng::Rng;
use crate::world::{HALL, HWY_Y, River, S, TAU};

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// What a formation is: it sets the proportions of its walls and its foot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rock {
    /// A flat-topped mesa with a wide cap.
    Mesa,
    /// A narrower table with sheer sides.
    Butte,
    /// A thin tower.
    Spire,
}

/// One rock formation: a flat top, a sheer wall and a talus apron of broken rock at the foot.
#[derive(Clone, Debug)]
pub struct Formation {
    pub kind: Rock,
    pub c: Pt,
    /// Mean radius of the top, metres.
    pub r: f64,
    /// Height of the top above the flats, metres.
    pub top: f64,
    /// Horizontal width of the wall, metres (a cell or so: the wall is sheer).
    pub wall: f64,
    /// Width of the talus slope at the foot, metres.
    pub skirt: f64,
    /// Height of the talus where it meets the wall, metres.
    pub foot: f64,
    /// Sine lobes of the outline: (amplitude, phase, frequency).
    lobes: [(f64, f64, f64); 5],
    /// A notch cut into the outline: (amplitude, direction).
    notch: (f64, f64),
    /// Phases of the lumps in the talus.
    lump: (f64, f64),
}

impl Formation {
    fn radius(&self, a: f64) -> f64 {
        let mut k = 1.0;
        for (amp, ph, f) in self.lobes.iter() {
            k += amp * sin(f * a + ph);
        }
        // a notch: a gully cut into the side of the rock
        let n = cos(a - self.notch.1);
        if n > 0.0 {
            k -= self.notch.0 * n.powi(5);
        }
        self.r * k
    }

    /// Distance beyond the edge of the top (negative on the top).
    pub fn edge_dist(&self, x: f64, y: f64) -> f64 {
        let (dx, dy) = (x - self.c.x, y - self.c.y);
        let d = (dx * dx + dy * dy).sqrt();
        if d > self.r * 1.8 + self.wall + self.skirt + 40.0 {
            return d - self.r;
        }
        d - self.radius(atan2(dy, dx))
    }

    /// Height of the formation above the flats at a point, metres.
    pub fn rise(&self, x: f64, y: f64) -> f64 {
        let d = self.edge_dist(x, y);
        if d <= 0.0 {
            // the top is flat
            return self.top;
        }
        if d <= self.wall {
            // sheer: a straight fall from a crisp edge to the head of the talus
            return self.foot + (self.top - self.foot) * (1.0 - d / self.wall);
        }
        let u = ((d - self.wall) / self.skirt).clamp(0.0, 1.0);
        let base = self.foot * (1.0 - u).powf(1.6);
        // broken rock: lumps that vanish at both ends of the slope
        let lump = sin(x / 17.0 + self.lump.0) * sin(y / 13.0 + self.lump.1);
        let lumps = 0.22 * self.foot * lump * 4.0 * u * (1.0 - u) * (1.0 - u);
        (base + lumps).max(0.0)
    }

    /// How far the flats under the formation are held level to its base, 1 under the rock.
    fn hold(&self, x: f64, y: f64) -> f64 {
        let d = self.edge_dist(x, y);
        1.0 - smooth(self.wall + 0.25 * self.skirt, self.wall + self.skirt, d)
    }

    /// Ring round the foot, `extra` metres out from where the talus is walkable.
    pub fn footprint(&self, extra: f64) -> Vec<Pt> {
        let n = 96;
        let mut rad: Vec<f64> = (0..n)
            .map(|i| self.radius(TAU * i as f64 / n as f64) + self.wall + 0.72 * self.skirt)
            .collect();
        // rounded: a road that follows the foot cannot turn on a point
        for _ in 0..9 {
            let src = rad.clone();
            for i in 0..n {
                rad[i] = 0.25 * src[(i + n - 1) % n] + 0.5 * src[i] + 0.25 * src[(i + 1) % n];
            }
        }
        (0..=n)
            .map(|i| {
                let a = TAU * (i % n) as f64 / n as f64;
                let r = rad[i % n] + extra;
                pt(self.c.x + r * cos(a), self.c.y + r * sin(a))
            })
            .collect()
    }

    /// Furthest the foot reaches from the centre, metres.
    fn reach(&self) -> f64 {
        self.r * 1.8 + self.wall + self.skirt + 40.0
    }
}

/// Mesas, buttes and spires scattered over the flats, and a few dry washes.
#[derive(Clone, Debug)]
pub struct MesaField {
    pub formations: Vec<Formation>,
    /// Dry washes: (x at y = 0, slope dx/dy, wobble, wavelength, phase, half width, depth).
    washes: Vec<(f64, f64, f64, f64, f64, f64, f64)>,
}

impl MesaField {
    /// `half` is the distance between neighbouring roads of the grid the town is laid on.
    pub fn new(seed: u64, half: f64, river: &River, ends: &[Pt]) -> MesaField {
        let _ = half;
        let mut rng = Rng::new(seed, 0x3e5a);
        // candidate centres over the flats, clear of the downtown and the river
        let mut cells: Vec<Pt> = Vec::new();
        let mut y = HWY_Y + 480.0;
        while y < S - 450.0 {
            let mut x = 450.0;
            while x < S - 450.0 {
                let c = pt(x, y);
                if c.dist(HALL) > 1000.0 && (c.y - river.y(c.x)).abs() > 580.0 {
                    cells.push(c);
                }
                x += 110.0;
            }
            y += 110.0;
        }
        for k in (1..cells.len()).rev() {
            let j = rng.below(k + 1);
            cells.swap(k, j);
        }
        let plan = [
            Rock::Mesa,
            Rock::Mesa,
            Rock::Mesa,
            Rock::Butte,
            Rock::Butte,
            Rock::Butte,
            Rock::Butte,
            Rock::Butte,
            Rock::Spire,
            Rock::Spire,
            Rock::Spire,
            Rock::Spire,
            Rock::Spire,
            Rock::Spire,
        ];
        let mut formations: Vec<Formation> = Vec::new();
        for kind in plan {
            let (r, top, wall, skirt_k, amp) = match kind {
                Rock::Mesa => (
                    rng.uniform(135.0, 175.0),
                    rng.uniform(170.0, 300.0),
                    16.0,
                    rng.uniform(1.8, 2.4),
                    0.2,
                ),
                Rock::Butte => (
                    rng.uniform(48.0, 85.0),
                    rng.uniform(125.0, 210.0),
                    14.0,
                    rng.uniform(1.8, 2.4),
                    0.22,
                ),
                Rock::Spire => (
                    rng.uniform(24.0, 38.0),
                    rng.uniform(130.0, 250.0),
                    12.0,
                    rng.uniform(1.6, 2.0),
                    0.16,
                ),
            };
            let foot = (0.2 * top).clamp(24.0, 56.0);
            let lobes = [
                (amp * rng.uniform(0.6, 1.0), rng.uniform(0.0, TAU), 2.0),
                (amp * rng.uniform(0.5, 0.9), rng.uniform(0.0, TAU), 3.0),
                (amp * rng.uniform(0.4, 0.7), rng.uniform(0.0, TAU), 4.0),
                (amp * rng.uniform(0.2, 0.4), rng.uniform(0.0, TAU), 7.0),
                (amp * rng.uniform(0.1, 0.25), rng.uniform(0.0, TAU), 11.0),
            ];
            let notch = if kind == Rock::Spire {
                (0.0, 0.0)
            } else {
                (rng.uniform(0.15, 0.3), rng.uniform(0.0, TAU))
            };
            let lump = (rng.uniform(0.0, TAU), rng.uniform(0.0, TAU));
            let mut f = Formation {
                kind,
                c: HALL,
                r,
                top,
                wall,
                skirt: foot * skirt_k,
                foot,
                lobes,
                notch,
                lump,
            };
            let reach = f.r * 1.3 + f.wall + 0.72 * f.skirt + 14.0;
            let clear = reach + 40.0 + 30.0;
            for sp in &cells {
                if ends.iter().any(|e| e.dist(*sp) < clear) {
                    continue;
                }
                if formations.iter().any(|g| {
                    let gr = g.r * 1.3 + g.wall + 0.72 * g.skirt + 14.0;
                    g.c.dist(*sp) < gr + reach + 150.0
                }) {
                    continue;
                }
                f.c = *sp;
                formations.push(f.clone());
                break;
            }
        }
        let mut washes = Vec::new();
        for i in 0..3 {
            washes.push((
                rng.uniform(300.0, S - 300.0),
                rng.uniform(-0.25, 0.25),
                rng.uniform(40.0, 90.0),
                rng.uniform(380.0, 700.0),
                rng.uniform(0.0, TAU),
                rng.uniform(22.0, 34.0),
                rng.uniform(3.0, 4.5) - 0.3 * i as f64,
            ));
        }
        MesaField { formations, washes }
    }

    /// Ground height at a point given the plain ground there and `plain_at`, the plain ground
    /// anywhere (each formation stands level on the ground at its own middle).
    pub fn height(&self, plain: f64, x: f64, y: f64, plain_at: &dyn Fn(f64, f64) -> f64) -> f64 {
        let mut h = plain;
        for f in &self.formations {
            let reach = f.reach();
            if (x - f.c.x).abs() > reach || (y - f.c.y).abs() > reach {
                continue;
            }
            let hold = f.hold(x, y);
            if hold <= 0.0 && f.edge_dist(x, y) > f.wall + f.skirt {
                continue;
            }
            // the flats under and round the rock are held level to its base
            let level = plain_at(f.c.x, f.c.y);
            h += (level - plain) * hold + f.rise(x, y);
        }
        // washes: a shallow bed with a flat sandy floor
        for (x0, sl, amp, wl, ph, hw, depth) in &self.washes {
            if y < HWY_Y + 300.0 {
                continue;
            }
            let cx = x0 + sl * y + amp * sin(y / wl + ph);
            let d = (x - cx).abs();
            if d < hw * 3.0 {
                let bed = 1.0 - smooth(hw * 0.45, hw * 1.2, d);
                // a wash fades out near the downtown so the hall stays level
                let keep = smooth(
                    500.0,
                    1100.0,
                    ((x - HALL.x).powi(2) + (y - HALL.y).powi(2)).sqrt(),
                );
                h -= depth * bed * keep;
            }
        }
        h
    }
}

/// A round cove cut into the coast.
#[derive(Clone, Copy, Debug)]
pub struct Cove {
    pub c: Pt,
    pub rho: f64,
    /// The sheltered harbour: a beach on its inner shore, cliffs and headlands round its mouth.
    pub harbour: bool,
    /// Phase of the bumps of an irregular shore.
    pub phase: f64,
}

impl Cove {
    /// Distance from the centre to the shore in direction `a` (radians from +x). The small coves
    /// are round; the harbour has a wandering shore with small rock points on its two sides and a
    /// smooth sandy arc on the inland side.
    pub fn radius(&self, a: f64) -> f64 {
        if !self.harbour {
            return self.rho;
        }
        let p = self.phase;
        let mut a = a;
        while a > std::f64::consts::PI {
            a -= TAU;
        }
        while a < -std::f64::consts::PI {
            a += TAU;
        }
        // the mouth side stays round, so the horns (where the cove meets the open coast) are a
        // single crossing; the sides and the back wander
        let wander = smooth(1.15, 1.5, a.abs());
        let rough = wander * (0.4 + 0.6 * sin(a) * sin(a));
        let mut k = 1.0
            + rough
                * (0.10 * sin(2.0 * a + p)
                    + 0.06 * sin(3.0 * a + 1.7 * p)
                    + 0.035 * sin(6.0 * a + 2.3 * p));
        // small points of rock that run out into the water
        for (at, w, h) in [
            (1.6, 0.13, 0.16),
            (2.1, 0.10, 0.12),
            (-1.65, 0.12, 0.17),
            (-2.15, 0.09, 0.11),
        ] {
            let d = a - (at + 0.1 * sin(p * 2.0 + at * 3.0));
            k -= h * libm::exp(-(d / w) * (d / w));
        }
        self.rho * k
    }

    /// Signed distance of a point from the shore of the cove: negative inside the water.
    pub fn dist(&self, x: f64, y: f64) -> f64 {
        let (dx, dy) = (x - self.c.x, y - self.c.y);
        (dx * dx + dy * dy).sqrt() - self.radius(atan2(dy, dx))
    }

    /// x of the west (`east` false) or east edge of the cove, grown by `inland` metres, along the
    /// row `y`.
    pub fn edge_at(&self, y: f64, inland: f64, east: bool) -> Option<f64> {
        let dy = y - self.c.y;
        if !self.harbour {
            let r = self.rho + inland;
            if dy.abs() >= r {
                return None;
            }
            let w = (r * r - dy * dy).sqrt();
            return Some(if east { self.c.x + w } else { self.c.x - w });
        }
        let reach = self.rho * 1.4 + inland;
        if dy.abs() >= reach {
            return None;
        }
        let sgn = if east { 1.0 } else { -1.0 };
        let f = |x: f64| self.dist(x, y) - inland;
        let (mut out, mut inn) = (self.c.x + sgn * reach, self.c.x);
        // walk in from outside until the shore is crossed, then bisect
        let mut x = out;
        let step = -sgn * 6.0;
        let mut found = false;
        while (x - self.c.x) * sgn > 0.0 {
            let nx = x + step;
            if f(nx) < 0.0 {
                inn = nx;
                out = x;
                found = true;
                break;
            }
            x = nx;
        }
        if !found {
            return None;
        }
        for _ in 0..28 {
            let m = 0.5 * (out + inn);
            if f(m) < 0.0 {
                inn = m;
            } else {
                out = m;
            }
        }
        Some(0.5 * (out + inn))
    }
}

/// A stack of rock standing in the sea off the cliffs.
#[derive(Clone, Copy, Debug)]
pub struct Stack {
    pub c: Pt,
    pub r: f64,
    pub h: f64,
    /// A stack with an arch through its foot.
    pub arch: bool,
}

/// A boat at its mooring: place, heading in radians, and size (0 small, 1 medium, 2 large).
#[derive(Clone, Copy, Debug)]
pub struct Mooring {
    pub at: Pt,
    pub heading: f64,
    pub size: u8,
}

/// What stands in and round the harbour cove: breakwaters, piers, boats and the lighthouse.
#[derive(Clone, Debug, Default)]
pub struct Harbour {
    /// Stone breakwaters: the line of each, from the shore out to sea.
    pub breakwaters: Vec<Vec<Pt>>,
    /// Piers: root on the shore and tip in the water.
    pub piers: Vec<(Pt, Pt)>,
    /// Foot of the lighthouse, on the high ground of a headland.
    pub lighthouse: Pt,
    pub boats: Vec<Mooring>,
}

/// The sea along the east edge: cliffs, coves and a harbour.
#[derive(Clone, Debug)]
pub struct CoastField {
    base: f64,
    a1: f64,
    p1: f64,
    a2: f64,
    p2: f64,
    cliff_a: f64,
    cliff_b: f64,
    face: f64,
    pub coves: Vec<Cove>,
    pub stacks: Vec<Stack>,
    inland_approach: bool,
    seed: u64,
}

impl CoastField {
    pub fn new(seed: u64) -> CoastField {
        Self::for_version(seed, crate::GEN_VERSION)
    }

    pub(crate) fn for_version(seed: u64, version: u32) -> CoastField {
        let mut rng = Rng::new(seed, 0xc0a5);
        let yh = HALL.y + rng.uniform(-300.0, 450.0);
        let rho_h = rng.uniform(380.0, 450.0);
        let mut coves = vec![Cove {
            c: pt(0.0, yh),
            rho: rho_h,
            harbour: true,
            phase: rng.uniform(0.0, TAU),
        }];
        let mut y = if rng.chance(0.5) {
            yh + 1100.0
        } else {
            yh - 1000.0
        };
        for k in 0..2 {
            let rho = rng.uniform(130.0, 210.0);
            coves.push(Cove {
                c: pt(0.0, y.clamp(1500.0 - 700.0 * (k as f64), S - 400.0)),
                rho,
                harbour: false,
                phase: 0.0,
            });
            y += if y > yh { 900.0 } else { -900.0 };
        }
        let mut f = CoastField {
            base: 3040.0 + rng.uniform(-60.0, 80.0),
            a1: rng.uniform(70.0, 110.0),
            p1: rng.uniform(0.0, TAU),
            a2: rng.uniform(18.0, 34.0),
            p2: rng.uniform(0.0, TAU),
            cliff_a: rng.uniform(0.0, TAU),
            cliff_b: rng.uniform(0.0, TAU),
            face: rng.uniform(0.0, TAU),
            coves: Vec::new(),
            stacks: Vec::new(),
            inland_approach: version >= 4,
            seed,
        };
        // coves sit on the coast line; the harbour is a keyhole with a narrower mouth, the
        // others are round bites out of the cliffs
        for c in &mut coves {
            let off = if c.harbour { 0.6 * c.rho } else { 0.0 };
            c.c.x = f.line_x(c.c.y) - off;
        }
        f.coves = coves;
        // sea stacks off the cliffs, clear of the harbour
        let mut srng = Rng::new(seed, 0xc0a6);
        let mut tries = 0;
        let mut y = 900.0 + srng.uniform(0.0, 200.0);
        while f.stacks.len() < 8 && tries < 120 && y < S - 200.0 {
            tries += 1;
            let r = srng.uniform(9.0, 32.0);
            let out = srng.uniform(70.0, 190.0);
            let c = pt(f.edge_x(y, -32.0) + out, y);
            let ok = f.inland(c.x, c.y) < -(60.0 + r)
                && (c.y - yh).abs() > rho_h + 260.0
                && f.stacks.iter().all(|o| o.c.dist(c) > 140.0);
            if ok {
                let arch = f.stacks.len() % 3 == 1;
                f.stacks.push(Stack {
                    c,
                    r: if arch { r + 12.0 } else { r },
                    h: srng.uniform(26.0, 110.0),
                    arch,
                });
            }
            y += srng.uniform(120.0, 300.0);
        }
        f
    }

    /// x of the straight run of coast at height y (before coves), swinging off the map in the
    /// south so the highway reaches the east edge.
    fn line_x(&self, y: f64) -> f64 {
        self.base
            + self.a1 * sin(y / 520.0 + self.p1)
            + self.a2 * sin(y / 190.0 + self.p2)
            + 1500.0 * (1.0 - smooth(600.0, 1200.0, y))
    }

    /// Distance inland from the sea at a point: negative at sea.
    pub fn inland(&self, x: f64, y: f64) -> f64 {
        let mut d = self.line_x(y) - x;
        for c in &self.coves {
            let dc = c.dist(x, y);
            if dc < d {
                d = dc;
            }
        }
        d
    }

    /// x of the sea edge `inland` metres in from the water, at height y (the sea is east of it).
    pub fn edge_x(&self, y: f64, inland: f64) -> f64 {
        let mut x = self.line_x(y) - inland;
        for c in &self.coves {
            if let Some(w) = c.edge_at(y, inland, false) {
                x = x.min(w);
            }
        }
        x
    }

    /// Weight of the harbour at a point, 1 in the cove, 0 some way off.
    pub fn harbour_weight(&self, x: f64, y: f64) -> f64 {
        let h = self.coves[0];
        let d = ((x - h.c.x).powi(2) + (y - h.c.y).powi(2)).sqrt();
        let near = 1.0 - smooth(0.85 * h.rho, h.rho + 700.0, d);
        // the downtown keeps its own level ground
        let keep = smooth(
            120.0,
            320.0,
            ((x - HALL.x).powi(2) + (y - HALL.y).powi(2)).sqrt(),
        );
        // only the inner (inland) side of the cove is low: its mouth stays cliff
        near * keep * smooth(-0.05, 0.45, (h.c.x - x) / h.rho)
    }

    /// How much of a point is beach: the inner shore of the harbour cove only.
    pub fn beach_weight(&self, x: f64, y: f64) -> f64 {
        let h = self.coves[0];
        let d = self.inland(x, y);
        let u = (h.c.x - x) / h.rho;
        let along = 1.0 - smooth(0.75, 1.05, ((y - h.c.y) / h.rho).abs());
        let fade = if d > 0.0 {
            1.0 - smooth(55.0, 100.0, d)
        } else {
            1.0
        };
        smooth(0.3, 0.7, u) * along * fade
    }

    /// Height of a point on the coast given the plain ground there.
    pub fn height(&self, plain: f64, x: f64, y: f64) -> f64 {
        let d = self.inland(x, y);
        if d <= 0.0 {
            return 0.0;
        }
        let hv = self.coves[0];
        let hw = self.harbour_weight(x, y);
        // how high the headlands stand: 60 to 150 m, higher still at the horns of the harbour
        let swing = 0.5 + 0.5 * sin(y / 1500.0 + self.cliff_a) * cos(y / 760.0 + self.cliff_b);
        let mut head = 75.0 + 62.0 * swing;
        for k in [-1.0, 1.0] {
            let dy = y - (hv.c.y + k * 0.8 * hv.rho);
            head += 12.0 * libm::exp(-(dy / 260.0).powi(2));
        }
        // the cliff top is a plateau at the lip, falling away inland
        // the downtown keeps its level ground
        let keep = smooth(
            500.0,
            1200.0,
            ((x - HALL.x).powi(2) + (y - HALL.y).powi(2)).sqrt(),
        );
        let top = plain.max(0.0) + keep * head * libm::exp(-(d - 150.0).max(0.0) / 380.0);
        // round the inner harbour the land is low
        let top = top * (1.0 - hw) + (5.0 + 0.03 * plain.max(0.0)) * hw;
        // a sheer face, its width varying so the wall has buttresses and gullies; or a beach
        let wc = 40.0 + 12.0 * sin(y / 31.0 + self.face) + 7.0 * sin(y / 11.0 + 2.0 * self.face);
        let mix = smooth(0.04, 0.5, hw);
        let w = wc * (1.0 - mix) + 280.0 * mix;
        let t = (d / w).clamp(0.0, 1.0);
        let sheer = 1.0 - (1.0 - t).powf(2.4);
        let s = sheer * (1.0 - mix) + smooth(0.0, 1.0, t) * mix;
        top * s
    }

    /// x of the open coast at height y (the straight run and the small coves, not the harbour),
    /// `inland` metres in from the water.
    fn open_edge_x(&self, y: f64, inland: f64) -> f64 {
        let mut x = self.line_x(y) - inland;
        for c in self.coves.iter().filter(|c| !c.harbour) {
            if let Some(w) = c.edge_at(y, inland, false) {
                x = x.min(w);
            }
        }
        x
    }

    /// The two horns of the harbour at the sea edge: where the round cove meets the open coast.
    /// Returns (south, north).
    pub fn horns(&self, inland: f64) -> (Pt, Pt) {
        let hv = self.coves[0];
        let r = hv.rho + inland;
        let ex = |y: f64| hv.edge_at(y, inland, true).unwrap_or(hv.c.x);
        let find = |sign: f64| -> Pt {
            let mut dy = 0.0;
            while dy < r - 1.0 {
                let y = hv.c.y + sign * dy;
                // the cove's east side has left the open coast behind
                if ex(y) < self.open_edge_x(y, inland) {
                    break;
                }
                dy += 1.0;
            }
            let y = hv.c.y + sign * dy;
            pt(ex(y), y)
        };
        (find(-1.0), find(1.0))
    }

    /// Water polygon: everything east of the sea edge, with a flat shelf of ground between it
    /// and the foot of the cliffs. It follows the coast on past the map edge and runs out to a
    /// far shore so the sea reaches the horizon. The harbour is a round bite out of the land.
    pub fn sea_ring(&self) -> Vec<Pt> {
        let shelf = -30.0;
        let far = S + 1600.0;
        let hv = self.coves[0];
        let (hs, hn) = self.horns(shelf);
        let mut ring = Vec::new();
        let (y0, y1) = (-500.0, S + 500.0);
        ring.push(pt(far, y0));
        let mut y = y0;
        while y < hs.y {
            ring.push(pt(self.open_edge_x(y, shelf).min(far), y));
            y += 12.0;
        }
        // round the cove, from the south horn through the west to the north horn
        let a0 = atan2(hs.y - hv.c.y, hs.x - hv.c.x);
        let a1 = atan2(hn.y - hv.c.y, hn.x - hv.c.x) - TAU;
        let steps = 120;
        for i in 0..=steps {
            let a = a0 + (a1 - a0) * i as f64 / steps as f64;
            let r = hv.radius(a) + shelf;
            ring.push(pt(hv.c.x + r * cos(a), hv.c.y + r * sin(a)));
        }
        let mut y = hn.y;
        while y <= y1 {
            ring.push(pt(self.open_edge_x(y, shelf).min(far), y));
            y += 12.0;
        }
        ring.push(pt(far, y1));
        ring.dedup_by(|a, b| a.dist(*b) < 1e-6);
        ring
    }

    /// Keep-out ring for roads: the sea and a margin `m` metres in from its edge, reaching a
    /// little past the map. Its edge is the coast road.
    pub fn road_zone(&self, m: f64) -> Vec<Pt> {
        let (line, _) = self.coast_line(m);
        let mut ring = line.clone();
        ring.push(pt(S + 60.0, S));
        ring.push(pt(S + 60.0, line[0].y));
        ring
    }

    /// The line `m` metres in from the sea, from the highway to the north edge: the coast road.
    /// Corners where a cove meets the coast are rounded.
    pub fn coast_line(&self, m: f64) -> (Vec<Pt>, Vec<Pt>) {
        let mut xs: Vec<(f64, f64)> = Vec::new();
        let mut y = HWY_Y;
        while y < S {
            xs.push((self.edge_x(y, m).min(S - 250.0), y));
            y += 10.0;
        }
        xs.push((self.edge_x(S, m).min(S - 250.0), S));
        for _ in 0..3 {
            let src = xs.clone();
            for i in 2..src.len() - 2 {
                xs[i].0 = (src[i - 2].0
                    + 2.0 * src[i - 1].0
                    + 3.0 * src[i].0
                    + 2.0 * src[i + 1].0
                    + src[i + 2].0)
                    / 9.0;
            }
        }
        if self.inland_approach {
            // Stay inland of every sampled headland. Start the turn before the cliff and
            // ease back toward the sea on the other side. Road and keep-out rings share this line.
            const EASTING_PER_METRE: f64 = 0.18;
            for i in 1..xs.len() {
                let (x, y) = xs[i - 1];
                xs[i].0 = xs[i].0.min(x + EASTING_PER_METRE * (xs[i].1 - y));
            }
            for i in (0..xs.len() - 1).rev() {
                let (x, y) = xs[i + 1];
                xs[i].0 = xs[i].0.min(x + EASTING_PER_METRE * (y - xs[i].1));
            }
        }
        let line: Vec<Pt> = xs.into_iter().map(|(x, y)| pt(x, y)).collect();
        (line.clone(), line)
    }

    /// Breakwaters, piers, boats and the lighthouse of the harbour.
    pub fn harbour(&self) -> Harbour {
        let hv = self.coves[0];
        let mut rng = Rng::new(self.seed, 0xc0a7);
        // the horns: where the cove meets the open coast
        let (hs, hn) = self.horns(-30.0);
        // two arms of stone leave the horns and leave a gap between them for boats
        let arm = |from: Pt, toward: f64, reach: f64, bend: f64| -> Vec<Pt> {
            let mid = pt(from.x + 0.45 * reach, from.y + toward * 0.3 * reach);
            let tip = pt(
                from.x + 0.75 * reach,
                from.y + toward * (0.62 * reach + bend),
            );
            vec![
                pt(from.x - 10.0, from.y),
                from,
                pt(from.x + 25.0, from.y + toward * 12.0),
                mid,
                tip,
            ]
        };
        let south = arm(hs, 1.0, rng.uniform(330.0, 370.0), 0.0);
        let north = arm(hn, -1.0, rng.uniform(270.0, 310.0), 0.0);
        let breakwaters = vec![south, north];
        // piers run out from the beach on the inner shore
        let mut piers = Vec::new();
        let mut boats = Vec::new();
        for (i, deg) in [-34.0f64, 4.0, 38.0].into_iter().enumerate() {
            let a = (deg + rng.uniform(-5.0, 5.0)).to_radians();
            let th = std::f64::consts::PI - a;
            let rr = hv.radius(th) + 14.0;
            let root = pt(hv.c.x + rr * cos(th), hv.c.y + rr * sin(th));
            let len = rng.uniform(70.0, 120.0) - 10.0 * i as f64;
            let tip = pt(root.x + len * cos(a), root.y - len * sin(a));
            piers.push((root, tip));
            // small boats tied up along both sides
            let (dx, dy) = ((tip.x - root.x) / len, (tip.y - root.y) / len);
            let n = 2 + (i % 2);
            for k in 0..n {
                let t = 0.7 + 0.25 * k as f64 / n as f64;
                for side in [-1.0, 1.0] {
                    if rng.chance(0.7)
                        && self.inland(root.x + dx * len * t, root.y + dy * len * t) < -40.0
                    {
                        boats.push(Mooring {
                            at: pt(
                                root.x + dx * len * t - dy * 5.5 * side,
                                root.y + dy * len * t + dx * 5.5 * side,
                            ),
                            heading: atan2(dy, dx),
                            size: (rng.below(2)) as u8,
                        });
                    }
                }
            }
        }
        // boats at anchor in the bay and a few larger ones in the lee of the south arm
        let mut tries = 0;
        while boats.len() < 22 && tries < 200 {
            tries += 1;
            let a = rng.uniform(0.0, TAU);
            let r = rng.uniform(0.15, 0.7) * hv.rho;
            let at = pt(hv.c.x + r * cos(a), hv.c.y + r * sin(a));
            if self.inland(at.x, at.y) > -45.0 || boats.iter().any(|b| b.at.dist(at) < 22.0) {
                continue;
            }
            boats.push(Mooring {
                at,
                heading: rng.uniform(0.0, TAU),
                size: rng.below(3) as u8,
            });
        }
        // the lighthouse stands on the north headland, a little inland of the cliff edge
        let ly = hn.y + 150.0;
        let lighthouse = pt(self.open_edge_x(ly, 70.0), ly);
        Harbour {
            breakwaters,
            piers,
            lighthouse,
            boats,
        }
    }
}

/// The relief a style adds to the plain ground.
#[derive(Clone, Debug, Default)]
pub enum RockCoast {
    #[default]
    None,
    Mesa(MesaField),
    Coast(CoastField),
}
