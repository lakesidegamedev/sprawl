//! Extra relief for the newer styles: red rock formations and a high coast. The five 1.0 styles
//! never reach any of this code (`Relief::None`), so their ground stays exactly as it was.

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

/// One rock formation: a flat top, tiered walls and a talus slope at the foot.
#[derive(Clone, Debug)]
pub struct Formation {
    pub kind: Rock,
    pub c: Pt,
    /// Mean radius of the top, metres.
    pub r: f64,
    /// Height of the top above the flats, metres.
    pub top: f64,
    /// Horizontal width of the wall, metres.
    pub wall: f64,
    /// Width of the talus slope at the foot, metres.
    pub skirt: f64,
    lobes: [(f64, f64, f64); 3],
}

impl Formation {
    fn radius(&self, a: f64) -> f64 {
        let mut k = 1.0;
        for (i, (amp, ph, f)) in self.lobes.iter().enumerate() {
            let _ = i;
            k += amp * sin(f * a + ph);
        }
        self.r * k
    }

    /// Distance beyond the edge of the top (negative on the top).
    pub fn edge_dist(&self, x: f64, y: f64) -> f64 {
        let (dx, dy) = (x - self.c.x, y - self.c.y);
        let d = (dx * dx + dy * dy).sqrt();
        if d > self.r * 1.6 + self.wall + self.skirt + 40.0 {
            return d - self.r;
        }
        d - self.radius(atan2(dy, dx))
    }

    /// Height of the formation above the flats at a point, metres.
    pub fn rise(&self, x: f64, y: f64) -> f64 {
        let d = self.edge_dist(x, y);
        let foot = 0.3 * self.top;
        if d <= 0.0 {
            // a faint dome so the top is not a perfect plane
            return self.top + 1.6 * smooth(0.0, 1.0, -d / self.r.max(1.0));
        }
        if d <= self.wall {
            // three tiers with a ledge between each
            let u = d / self.wall * 3.0;
            let k = libm::floor(u);
            let s = (k + smooth(0.5, 1.0, u - k)) / 3.0;
            return foot + (self.top - foot) * (1.0 - s);
        }
        let u = ((d - self.wall) / self.skirt).clamp(0.0, 1.0);
        foot * (1.0 - u) * (1.0 - u)
    }

    /// Ring round the foot, `extra` metres out from where the talus is walkable.
    pub fn footprint(&self, extra: f64) -> Vec<Pt> {
        let n = 96;
        (0..=n)
            .map(|i| {
                let a = TAU * (i % n) as f64 / n as f64;
                let r = self.radius(a) + self.wall + 0.72 * self.skirt + extra;
                pt(self.c.x + r * cos(a), self.c.y + r * sin(a))
            })
            .collect()
    }
}

/// Mesas, buttes and spires, one to a block of the road grid, and a few dry washes.
#[derive(Clone, Debug)]
pub struct MesaField {
    pub formations: Vec<Formation>,
    /// Dry washes: (x at y = 0, slope dx/dy, wobble, wavelength, phase, half width, depth).
    washes: Vec<(f64, f64, f64, f64, f64, f64, f64)>,
}

impl MesaField {
    /// `half` is the distance between neighbouring roads of the grid the town is laid on.
    pub fn new(seed: u64, half: f64, river: &River) -> MesaField {
        let mut rng = Rng::new(seed, 0x3e5a);
        // block centres of the road grid that are clear of the downtown and the river
        let mut cells: Vec<Pt> = Vec::new();
        for i in -3..=3i32 {
            for j in -1..=3i32 {
                let c = pt(
                    HALL.x + (i as f64 + 0.5) * half,
                    HALL.y + (j as f64 + 0.5) * half,
                );
                if c.x < 450.0 || c.x > S - 450.0 || c.y > S - 450.0 || c.y < HWY_Y + 450.0 {
                    continue;
                }
                if c.dist(HALL) < 1000.0 || (c.y - river.y(c.x)).abs() < 580.0 {
                    continue;
                }
                cells.push(c);
            }
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
            let lobe = |rng: &mut Rng, amp: f64, f: f64| {
                (amp * rng.uniform(0.6, 1.0), rng.uniform(0.0, TAU), f)
            };
            let (r, top, wall, skirt, amp) = match kind {
                Rock::Mesa => (
                    rng.uniform(135.0, 170.0),
                    rng.uniform(170.0, 300.0),
                    34.0,
                    rng.uniform(110.0, 140.0),
                    0.1,
                ),
                Rock::Butte => (
                    rng.uniform(45.0, 80.0),
                    rng.uniform(110.0, 210.0),
                    24.0,
                    rng.uniform(70.0, 95.0),
                    0.12,
                ),
                Rock::Spire => (
                    rng.uniform(16.0, 30.0),
                    rng.uniform(130.0, 250.0),
                    14.0,
                    rng.uniform(45.0, 65.0),
                    0.1,
                ),
            };
            let lobes = [
                lobe(&mut rng, amp, 2.0),
                lobe(&mut rng, amp * 0.8, 3.0),
                lobe(&mut rng, amp * 0.6, 5.0),
            ];
            let mut f = Formation {
                kind,
                c: HALL,
                r,
                top,
                wall,
                skirt,
                lobes,
            };
            let foot = f.r * 1.25 + f.wall + 0.72 * f.skirt + 14.0;
            // candidate spots: the middle of a block, and for small rock the four quarters
            let mut spots: Vec<Pt> = Vec::new();
            for c in &cells {
                spots.push(*c);
                if kind != Rock::Mesa {
                    for ix in -2..=2 {
                        for iy in -2..=2 {
                            if ix != 0 || iy != 0 {
                                spots.push(pt(
                                    c.x + ix as f64 * half * 0.16,
                                    c.y + iy as f64 * half * 0.16,
                                ));
                            }
                        }
                    }
                }
            }
            for sp in spots {
                // inside its own block, clear of the roads that bound it
                let cell_x = ((sp.x - HALL.x) / half).floor();
                let cell_y = ((sp.y - HALL.y) / half).floor();
                let (x0, y0) = (HALL.x + cell_x * half, HALL.y + cell_y * half);
                let room = foot + 12.0;
                if sp.x - x0 < room
                    || x0 + half - sp.x < room
                    || sp.y - y0 < room
                    || y0 + half - sp.y < room
                {
                    continue;
                }
                let clash = formations.iter().any(|g| {
                    let gf = g.r * 1.25 + g.wall + 0.72 * g.skirt + 14.0;
                    g.c.dist(sp) < gf + foot + 20.0
                });
                if clash {
                    continue;
                }
                f.c = sp;
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

    /// Height to add to the plain ground at a point.
    pub fn delta(&self, x: f64, y: f64) -> f64 {
        let mut add = 0.0;
        for f in &self.formations {
            // far from a formation the sums are zero: skip the angle
            let reach = f.r * 1.6 + f.wall + f.skirt + 40.0;
            if (x - f.c.x).abs() > reach || (y - f.c.y).abs() > reach {
                continue;
            }
            add += f.rise(x, y);
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
                add -= depth * bed * keep;
            }
        }
        add
    }
}

/// A round cove cut into the coast.
#[derive(Clone, Copy, Debug)]
pub struct Cove {
    pub c: Pt,
    pub rho: f64,
    /// The sheltered harbour: its shore is a gentle beach rather than a cliff.
    pub harbour: bool,
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
    pub coves: Vec<Cove>,
}

impl CoastField {
    pub fn new(seed: u64) -> CoastField {
        let mut rng = Rng::new(seed, 0xc0a5);
        let yh = HALL.y + rng.uniform(-300.0, 450.0);
        let mut coves = vec![Cove {
            c: pt(0.0, yh),
            rho: rng.uniform(380.0, 450.0),
            harbour: true,
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
            });
            y += if y > yh { 900.0 } else { -900.0 };
        }
        let mut f = CoastField {
            base: 3040.0 + rng.uniform(-60.0, 80.0),
            a1: rng.uniform(90.0, 140.0),
            p1: rng.uniform(0.0, TAU),
            a2: rng.uniform(35.0, 60.0),
            p2: rng.uniform(0.0, TAU),
            cliff_a: rng.uniform(0.0, TAU),
            cliff_b: rng.uniform(0.0, TAU),
            coves: Vec::new(),
        };
        // coves sit on the coast line
        for c in &mut coves {
            c.c.x = f.line_x(c.c.y);
        }
        f.coves = coves;
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
            let dc = ((x - c.c.x).powi(2) + (y - c.c.y).powi(2)).sqrt() - c.rho;
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
            let r = c.rho + inland;
            let dy = y - c.c.y;
            if dy.abs() < r {
                x = x.min(c.c.x - (r * r - dy * dy).sqrt());
            }
        }
        x
    }

    /// Weight of the harbour at a point, 1 in the middle, 0 a kilometre away.
    pub fn harbour_weight(&self, x: f64, y: f64) -> f64 {
        let h = self.coves[0];
        let d = ((x - h.c.x).powi(2) + (y - h.c.y).powi(2)).sqrt();
        libm::exp(-(d / (h.rho + 520.0)).powi(4))
    }

    /// Height of a point on the coast given the plain ground there.
    pub fn height(&self, plain: f64, x: f64, y: f64) -> f64 {
        let d = self.inland(x, y);
        if d <= 0.0 {
            return 0.0;
        }
        let hw = self.harbour_weight(x, y);
        // how high the headlands stand: 60 to 150 m, falling away inland
        let swing = 0.5 + 0.5 * sin(y / 1500.0 + self.cliff_a) * cos(y / 760.0 + self.cliff_b);
        let head = 85.0 + 80.0 * swing;
        let top = plain + head * libm::exp(-d / 420.0);
        // round the harbour the land is low
        let top = top * (1.0 - hw) + (9.0 + 0.12 * top) * hw;
        // a sheer face rounded at the top edge, or a long beach in the harbour
        let w = 46.0 + 540.0 * hw;
        let t = (d / w).clamp(0.0, 1.0);
        let sheer = 1.0 - (1.0 - t) * (1.0 - t);
        let mix = smooth(0.02, 0.2, hw);
        let s = sheer * (1.0 - mix) + smooth(0.0, 1.0, t) * mix;
        top * s
    }

    /// Water polygon: everything east of the sea edge, with a flat shelf of ground between it
    /// and the foot of the cliffs. It runs on past the map edge so the sea reaches the horizon.
    pub fn sea_ring(&self) -> Vec<Pt> {
        let shelf = -32.0;
        let far = S + 1600.0;
        let mut ring = Vec::new();
        let mut y = 560.0;
        let mut started = false;
        while y <= S {
            let x = self.edge_x(y, shelf);
            if x < S + 40.0 {
                if !started {
                    ring.push(pt(far, y));
                    started = true;
                }
                ring.push(pt(x, y));
            }
            y += 12.0;
        }
        ring.push(pt(self.edge_x(S, shelf), S));
        ring.push(pt(self.edge_x(S, shelf), far));
        ring.push(pt(far, far));
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
        let line: Vec<Pt> = xs.into_iter().map(|(x, y)| pt(x, y)).collect();
        (line.clone(), line)
    }
}

/// The relief a style adds to the plain ground.
#[derive(Clone, Debug, Default)]
pub enum Relief {
    #[default]
    None,
    Mesa(MesaField),
    Coast(CoastField),
}
