//! A deterministic heightfield: flat ground round the town hall and along the highway, a gentle
//! valley with flat floor for the river, hills further out. Pure function of the seed.

use crate::Preset;
use crate::geom::{Pt, atan2, cos, pt, sin};
use crate::rng::Rng;
use crate::world::{HALL, HWY_Y, River, S};

const WAVES: usize = 6;

/// Height of the land in metres at any point of the map. Cheap to evaluate, so renderers can
/// sample it per vertex rather than storing a grid.
#[derive(Clone, Debug)]
pub struct Terrain {
    river: River,
    /// Lake basin (centre, radius) that pulls the ground down and keeps it flat.
    basin: Option<(Pt, f64)>,
    /// (kx, ky, phase, amplitude) of each wave.
    waves: [(f64, f64, f64, f64); WAVES],
    /// The per-style shaping of the newer towns; `Plain` for the five 1.0 styles.
    shape: Shape,
}

#[derive(Clone, Debug)]
enum Shape {
    Plain,
    Gorge(Gorge),
    Valley(Valley),
}

/// A side ravine, the long tributary, or the wide descent gully of a gorge town. They run north
/// or south from the river; `x0` is where they leave it.
#[derive(Clone, Debug)]
pub(crate) struct Ravine {
    pub x0: f64,
    /// +1 runs north of the river, -1 south.
    pub side: f64,
    /// Length along y from the bench edge to the head, metres.
    pub len: f64,
    /// Half width of the top of the walls at the mouth, metres.
    pub hw: f64,
    /// Half width of the flat floor, metres.
    pub fhw: f64,
    /// Wobble of the centre line: amplitude, wavelength, phase.
    pub wob: (f64, f64, f64),
    /// Floor rises linearly (the descent gully) instead of in an S curve.
    pub linear: bool,
    /// Share of the plateau height the floor reaches at the head (the tributary stays low).
    pub reach: f64,
    /// How far the flat bench in front of the mouth extends from the river's floor, metres.
    pub bench: f64,
    /// The descent gully carries a switchback road: the number of legs.
    pub legs: usize,
    /// The switchback's corners in the world and the ground height the road has at each: the
    /// road falls at an even grade from the head to the foot.
    pub path: Vec<(f64, f64, f64)>,
}

impl Ravine {
    /// Centre line x at distance `t` along the ravine.
    pub(crate) fn cx(&self, t: f64) -> f64 {
        self.x0 + self.wob.0 * sin(t / self.wob.1 + self.wob.2)
    }

    /// Where the legs of the switchback turn: (x, distance t from the foot), from the head
    /// down. The first is the head, the last is the foot (not a turn).
    pub(crate) fn corners(&self) -> Vec<(f64, f64)> {
        let top = self.len;
        let half = self.fhw * 0.8;
        (0..=self.legs)
            .map(|k| {
                let t = top * (1.0 - k as f64 / self.legs as f64);
                let x = if k == 0 {
                    self.x0
                } else if k % 2 == 1 {
                    self.x0 - half
                } else {
                    self.x0 + half
                };
                (x, t)
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Gorge {
    /// Floor to rim, metres.
    pub depth: f64,
    /// Half width of the gorge at the rim, metres (across the slope).
    pub rim: f64,
    pub ph: [f64; 4],
    pub ravines: Vec<Ravine>,
    /// Seed of the rock noise (buttresses, fans).
    pub noise: u64,
    /// Shallow drainage swales that run to the rim over the plains: (x, wobble phase, depth,
    /// half width).
    pub swales: Vec<(f64, f64, f64, f64)>,
}

#[derive(Clone, Debug)]
pub(crate) struct Valley {
    /// Half width of the flat floor, metres.
    pub floor: f64,
    pub peak: f64,
    pub ph: [f64; 4],
    /// Side valleys running south and north: (x of centre line, wobble amplitude, wavelength,
    /// phase, half width of the floor).
    pub south: (f64, f64, f64, f64, f64),
    pub north: (f64, f64, f64, f64, f64),
    /// Seed of the rock noise (spurs and gullies).
    pub noise: u64,
    /// Individual summits: (x, side times share of the crest distance, height added, radius).
    pub peaks: Vec<(f64, f64, f64, f64)>,
}

/// Height of the lowland round the mountains.
const LOWLAND: f64 = 55.0;
/// Floor of the river to the top of the plateau in a gorge town, plus the lip.
const FLOOR_HW: f64 = 62.0;

/// The corner `b` of the polyline a-b-c rounded with an arc of radius up to `r` (less where the
/// legs are short): the points of the arc, in order from the a side to the c side.
fn fillet(a: (f64, f64), b: (f64, f64), c: (f64, f64), r: f64) -> Vec<(f64, f64)> {
    let unit = |p: (f64, f64), q: (f64, f64)| {
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        let l = (dx * dx + dy * dy).sqrt().max(1e-9);
        ((dx / l, dy / l), l)
    };
    let (u1, l1) = unit(b, a);
    let (u2, l2) = unit(b, c);
    let cosw = (u1.0 * u2.0 + u1.1 * u2.1).clamp(-1.0, 1.0);
    let theta = libm::acos(cosw);
    if !(0.05..=3.1).contains(&theta) {
        return vec![b];
    }
    let half = theta * 0.5;
    let t = libm::tan(half);
    let mut radius = r;
    let mut tl = radius / t;
    let cap = 0.45 * l1.min(l2);
    if tl > cap {
        tl = cap;
        radius = tl * t;
    }
    let t1 = (b.0 + u1.0 * tl, b.1 + u1.1 * tl);
    let t2 = (b.0 + u2.0 * tl, b.1 + u2.1 * tl);
    let bis = (u1.0 + u2.0, u1.1 + u2.1);
    let bl = (bis.0 * bis.0 + bis.1 * bis.1).sqrt().max(1e-9);
    let cd = radius / libm::sin(half);
    let cen = (b.0 + bis.0 / bl * cd, b.1 + bis.1 / bl * cd);
    let a1 = libm::atan2(t1.1 - cen.1, t1.0 - cen.0);
    let mut a2 = libm::atan2(t2.1 - cen.1, t2.0 - cen.0);
    // take the short way round
    while a2 - a1 > std::f64::consts::PI {
        a2 -= std::f64::consts::TAU;
    }
    while a2 - a1 < -std::f64::consts::PI {
        a2 += std::f64::consts::TAU;
    }
    let n = (((a2 - a1).abs() * radius / 5.0).ceil() as usize).max(3);
    (0..=n)
        .map(|i| {
            let ang = a1 + (a2 - a1) * i as f64 / n as f64;
            (cen.0 + radius * cos(ang), cen.1 + radius * sin(ang))
        })
        .collect()
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Terrain {
    pub(crate) fn new(seed: u64, river: &River, basin: Option<(Pt, f64)>) -> Terrain {
        let mut rng = Rng::new(seed, 0x7e44);
        let mut waves = [(0.0, 0.0, 0.0, 0.0); WAVES];
        for (i, w) in waves.iter_mut().enumerate() {
            // three scales: broad hills, ridges, knolls
            let (len, amp) = match i / 2 {
                0 => (rng.uniform(1400.0, 2200.0), 20.0),
                1 => (rng.uniform(700.0, 1000.0), 6.0),
                _ => (rng.uniform(300.0, 420.0), 2.0),
            };
            let dir = rng.uniform(0.0, std::f64::consts::TAU);
            let k = std::f64::consts::TAU / len;
            *w = (
                k * cos(dir),
                k * sin(dir),
                rng.uniform(0.0, std::f64::consts::TAU),
                amp * rng.uniform(0.8, 1.2),
            );
        }
        Terrain {
            river: river.clone(),
            basin,
            waves,
            shape: Shape::Plain,
        }
    }

    /// The ground of a town style. The five 1.0 styles are `new`; the newer styles add a
    /// shaping that the old ones never reach.
    pub(crate) fn styled(seed: u64, river: &River, style: Preset) -> Terrain {
        let mut t = Terrain::new(seed, river, None);
        let mut rng = Rng::new(seed, 0x6074);
        match style {
            Preset::Gorge => {
                let depth = rng.uniform(300.0, 370.0);
                let rim = rng.uniform(165.0, 200.0);
                let ph = [
                    rng.uniform(0.0, 6.3),
                    rng.uniform(0.0, 6.3),
                    rng.uniform(0.0, 6.3),
                    rng.uniform(0.0, 6.3),
                ];
                let ravine = |x0: f64, side: f64, len: f64, hw: f64, rng: &mut Rng| Ravine {
                    x0,
                    side,
                    len,
                    hw,
                    fhw: 30.0,
                    wob: (
                        rng.uniform(25.0, 70.0),
                        rng.uniform(260.0, 420.0),
                        rng.uniform(0.0, 6.3),
                    ),
                    linear: false,
                    reach: 1.0,
                    bench: 0.0,
                    legs: 0,
                    path: Vec::new(),
                };
                let mut ravines = Vec::new();
                // the descent gully: a long ramp into the gorge with a flat bench at its foot
                ravines.push(Ravine {
                    x0: 1420.0,
                    side: -1.0,
                    len: 860.0,
                    hw: 500.0,
                    fhw: 300.0,
                    wob: (0.0, 300.0, 0.0),
                    linear: true,
                    reach: 1.0,
                    bench: 170.0,
                    legs: 10,
                    path: Vec::new(),
                });
                // the long tributary that the highway bridges
                ravines.push(Ravine {
                    x0: 3250.0 + rng.uniform(-120.0, 120.0),
                    side: -1.0,
                    len: 2700.0,
                    hw: 190.0,
                    fhw: 46.0,
                    wob: (
                        rng.uniform(60.0, 110.0),
                        rng.uniform(520.0, 700.0),
                        rng.uniform(0.0, 6.3),
                    ),
                    linear: false,
                    reach: 0.4,
                    bench: 0.0,
                    legs: 0,
                    path: Vec::new(),
                });
                let jit = |rng: &mut Rng, v: f64| v + rng.uniform(-90.0, 90.0);
                let a = jit(&mut rng, 560.0);
                ravines.push(ravine(a, -1.0, 760.0, 140.0, &mut rng));
                let b = jit(&mut rng, 2560.0);
                ravines.push(ravine(b, -1.0, 820.0, 150.0, &mut rng));
                let c = jit(&mut rng, 900.0);
                ravines.push(ravine(c, 1.0, 900.0, 150.0, &mut rng));
                let d = jit(&mut rng, 2750.0);
                ravines.push(ravine(d, 1.0, 780.0, 140.0, &mut rng));
                let e = jit(&mut rng, 3650.0);
                ravines.push(ravine(e, 1.0, 700.0, 135.0, &mut rng));
                for r in ravines.iter_mut().filter(|r| r.legs > 0) {
                    let level = depth + 12.0;
                    let cs = r.corners();
                    let world = |x: f64, tt: f64| {
                        let m = river.dy(x);
                        let k = (1.0 + m * m).sqrt();
                        let ext = r.bench * smooth(0.0, 1.0, 1.0 - (x - r.x0).abs() / 420.0);
                        (x, river.y(x) - (FLOOR_HW + ext + tt) * k)
                    };
                    // the legs are straight in the gully's own measure (x, distance from the
                    // bench edge), with each turn a round hairpin, so the road keeps one even
                    // grade and the same height above the bench wherever the river bends
                    let mut flat: Vec<(f64, f64)> = vec![cs[0]];
                    for i in 1..cs.len() - 1 {
                        flat.extend(fillet(cs[i - 1], cs[i], cs[i + 1], 11.0));
                    }
                    flat.push(cs[cs.len() - 1]);
                    // resample every 14 m
                    let mut pts: Vec<(f64, f64)> = Vec::new();
                    for w in flat.windows(2) {
                        let l = ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt();
                        let n = ((l / 14.0).ceil() as usize).max(1);
                        for j in 0..n {
                            let u = j as f64 / n as f64;
                            let (x, tt) = (
                                w[0].0 + (w[1].0 - w[0].0) * u,
                                w[0].1 + (w[1].1 - w[0].1) * u,
                            );
                            pts.push(world(x, tt));
                        }
                    }
                    let last = cs[cs.len() - 1];
                    // the road ends a little way out on the flat bench
                    pts.push(world(last.0, last.1));
                    pts.push(world(last.0, last.1 - 14.0));
                    let seg = |w: &[(f64, f64)]| {
                        ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt()
                    };
                    // the road reaches the bench level where the last leg meets it; the short
                    // run beyond is level
                    let total: f64 = pts[..pts.len() - 1].windows(2).map(seg).sum();
                    let top_h = level * r.reach * (r.corners()[0].1 / r.len);
                    let mut acc = 0.0;
                    let mut path = Vec::new();
                    for (i, &(x, y)) in pts.iter().enumerate() {
                        if i > 0 && i + 1 < pts.len() {
                            acc += seg(&pts[i - 1..=i]);
                        }
                        path.push((x, y, top_h * (1.0 - acc / total).max(0.0)));
                    }
                    r.path = path;
                }
                let mut swales = Vec::new();
                for k in 0..10 {
                    swales.push((
                        150.0 + 410.0 * k as f64 + rng.uniform(-90.0, 90.0),
                        rng.uniform(0.0, 6.3),
                        rng.uniform(6.0, 12.0),
                        rng.uniform(150.0, 260.0),
                    ));
                }
                t.shape = Shape::Gorge(Gorge {
                    depth,
                    rim,
                    ph,
                    ravines,
                    noise: rng.next_u64(),
                    swales,
                });
            }
            Preset::Valley => {
                let floor = rng.uniform(560.0, 640.0);
                let peak = rng.uniform(780.0, 960.0);
                let ph = [
                    rng.uniform(0.0, 6.3),
                    rng.uniform(0.0, 6.3),
                    rng.uniform(0.0, 6.3),
                    rng.uniform(0.0, 6.3),
                ];
                let south = (
                    2750.0 + rng.uniform(-150.0, 150.0),
                    rng.uniform(40.0, 90.0),
                    rng.uniform(380.0, 520.0),
                    rng.uniform(0.0, 6.3),
                    120.0,
                );
                let north = (
                    1050.0 + rng.uniform(-150.0, 150.0),
                    rng.uniform(40.0, 90.0),
                    rng.uniform(380.0, 520.0),
                    rng.uniform(0.0, 6.3),
                    130.0,
                );
                let mut peaks = Vec::new();
                for k in 0..16 {
                    let side = if k % 2 == 0 { -1.0 } else { 1.0 };
                    let x = 250.0 + 3600.0 * (k as f64 + rng.uniform(0.1, 0.9)) / 16.0;
                    let d = rng.uniform(0.7, 1.0);
                    let h = rng.uniform(0.04, 0.26) * peak;
                    peaks.push((x, side * d, h, rng.uniform(150.0, 320.0)));
                }
                t.shape = Shape::Valley(Valley {
                    floor,
                    peak,
                    ph,
                    south,
                    north,
                    noise: rng.next_u64(),
                    peaks,
                });
            }
            _ => {}
        }
        t
    }

    pub(crate) fn gorge(&self) -> Option<&Gorge> {
        match &self.shape {
            Shape::Gorge(g) => Some(g),
            _ => None,
        }
    }

    pub(crate) fn valley(&self) -> Option<&Valley> {
        match &self.shape {
            Shape::Valley(v) => Some(v),
            _ => None,
        }
    }

    /// Hills of the old terrain, 0 to about 30 metres either way.
    fn hills(&self, x: f64, y: f64) -> f64 {
        let mut hills = 0.0;
        for (kx, ky, ph, a) in &self.waves {
            hills += a * sin(kx * x + ky * y + ph);
        }
        hills
    }

    /// Distance across the slope from the river, and which side (+1 north, -1 south).
    fn river_dist(&self, x: f64, y: f64) -> (f64, f64) {
        let xc = x.clamp(0.0, S);
        let dy = y - self.river.y(xc);
        let m = self.river.dy(xc);
        (
            dy.abs() / (1.0 + m * m).sqrt(),
            if dy < 0.0 { -1.0 } else { 1.0 },
        )
    }

    fn gorge_height(&self, g: &Gorge, x: f64, y: f64) -> f64 {
        let (d, side) = self.river_dist(x, y);
        let d_hall = ((x - HALL.x).powi(2) + (y - HALL.y).powi(2)).sqrt();
        let rimw =
            g.rim * (1.0 + 0.24 * sin(x / 430.0 + g.ph[0]) + 0.12 * sin(x / 190.0 + g.ph[1]));
        // the plateau: level near the lip, gentle hills further back, a rolled shoulder at the
        // lip, and shallow swales that run to the rim and notch it
        let mask = smooth(300.0, 900.0, d_hall)
            * smooth(HWY_Y + 60.0, HWY_Y + 420.0, y)
            * smooth(rimw, rimw + 260.0, d);
        let calm = smooth(250.0, 650.0, d_hall) * smooth(HWY_Y + 40.0, HWY_Y + 300.0, y);
        let mut swale = 0.0;
        for &(sx, ph, amp, hw) in &g.swales {
            let c = sx + 70.0 * sin(d / 260.0 + ph);
            let q = (x - c) / hw;
            if q.abs() < 2.6 {
                swale += amp * exp_neg(q * q) * (1.0 - smooth(rimw + 80.0, rimw + 900.0, d));
            }
        }
        let shoulder = 8.0 * (1.0 - smooth(rimw, rimw + 170.0, d));
        let plateau = g.depth + 12.0 + 0.7 * self.hills(x, y) * mask
            - (swale * calm).min(18.0)
            - shoulder * calm;
        let mut h = plateau;
        // the gorge itself: a flat floor and strata walls with ledges
        let ledge = 0.34 + 0.16 * sin(x / 310.0 + g.ph[2]);
        let (steps, tilt) = if side < 0.0 { (3.0, 0.1) } else { (2.0, 0.14) };
        // the flat bench at the foot of the descent gully widens the floor on the south bank
        let bench = if side < 0.0 {
            g.ravines
                .iter()
                .filter(|r| r.bench > 0.0)
                .map(|r| r.bench * smooth(0.0, 1.0, 1.0 - (x - r.x0).abs() / 420.0))
                .fold(0.0, f64::max)
        } else {
            0.0
        };
        let floor_hw = FLOOR_HW + bench;
        let top = rimw.max(floor_hw + 90.0);
        // buttresses and alcoves: the wall wanders in and out along the gorge
        let wob = 24.0 * (2.0 * fbm(g.noise, x / 120.0, side * 4.0, 2) - 1.0)
            + 10.0 * (2.0 * vnoise(g.noise ^ 5, x / 38.0, side * 9.0) - 1.0);
        let u = ((d + wob * smooth(floor_hw, floor_hw + 40.0, d) - floor_hw) / (top - floor_hw))
            .clamp(0.0, 1.0);
        // the descent gully takes the wall's place: no wall foot is cut across it
        let mut cap_mix = 1.0f64;
        if side < 0.0 {
            for r in g.ravines.iter().filter(|r| r.linear) {
                let t = d - (FLOOR_HW + bench);
                if t > -110.0 && t < r.len {
                    let s = t.max(0.0) / r.len;
                    let hw = r.hw * (1.0 - 0.45 * s);
                    let c = smooth(hw * 0.9, hw * 1.1, (x - r.cx(t.max(0.0))).abs());
                    let c = c.max(1.0 - smooth(0.0, 36.0, t));
                    cap_mix = f64::min(cap_mix, c);
                }
            }
        }
        if u < 1.0 {
            let wall = plateau * strata(u, steps, ledge, tilt);
            h = h.min(plateau + (wall - plateau) * cap_mix);
        }
        // talus fans at the foot of the walls
        if d < floor_hw + 150.0 && bench < 1.0 && h < plateau - 20.0 {
            let n = smooth(0.5, 0.8, fbm(g.noise ^ 9, x / 150.0, side * 3.0, 2));
            let fan = 30.0 * n * smooth(floor_hw + 150.0, floor_hw + 8.0, d);
            h += fan;
        }
        // side ravines
        for r in &g.ravines {
            let t = if r.linear {
                // the descent gully is measured from the bench edge wherever the river bends
                let bench_here = r.bench * smooth(0.0, 1.0, 1.0 - (x - r.x0).abs() / 420.0);
                if side == r.side {
                    d - (FLOOR_HW + bench_here)
                } else {
                    -1e9
                }
            } else {
                r.side * (y - self.river.y(r.x0)) - (FLOOR_HW + r.bench)
            };
            // a short lead-in before the foot keeps the floor joined to the bench where the river bends
            if t <= -(if r.bench > 0.0 { 110.0 } else { 0.0 }) || t >= r.len {
                continue;
            }
            let t = t.max(0.0);
            let s = t / r.len;
            let xc = r.cx(t);
            let hw = r.hw * (1.0 - 0.45 * s) + r.fhw * 0.0;
            let fhw = r.fhw.min(hw * 0.9);
            let dd = (x - xc).abs();
            let g_s = if r.linear { s } else { smooth(0.0, 1.0, s) };
            let mut floor = plateau * (r.reach * g_s);
            if r.legs > 0 && dd < hw + 80.0 {
                // the road down the gully has an even grade: the floor follows it near the road
                let (mut wsum, mut dsum) = (0.0, 0.0);
                for w in r.path.windows(2) {
                    let (ax, ay, ah) = w[0];
                    let (bx, by, bh) = w[1];
                    let (vx, vy) = (bx - ax, by - ay);
                    let l2 = vx * vx + vy * vy;
                    let u = (((x - ax) * vx + (y - ay) * vy) / l2).clamp(0.0, 1.0);
                    let (px, py) = (ax + vx * u, ay + vy * u);
                    let dist = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
                    if dist < 40.0 {
                        let wt = (1.0 - smooth(10.0, 40.0, dist)) * (1.0 - smooth(0.9, 1.0, s));
                        wsum += wt;
                        dsum += wt * (ah + (bh - ah) * u - floor);
                    }
                }
                if wsum > 0.0 {
                    floor += dsum / wsum.max(1.0);
                }
            }
            let uu = ((dd - fhw) / (hw - fhw)).clamp(0.0, 1.0);
            let wall = strata(uu, 2.0, 0.3, 0.24);
            let carve = floor + wall * (plateau - floor);
            if carve < h {
                h = carve;
            }
        }
        h
    }

    fn valley_height(&self, v: &Valley, x: f64, y: f64) -> f64 {
        let (d, side) = self.river_dist(x, y);
        let wob = 1.0 + 0.12 * sin(x / 610.0 + v.ph[0]) + 0.07 * sin(x / 230.0 + v.ph[1]);
        let fl = v.floor * wob;
        let hills = self.hills(x, y);
        let d_hall = ((x - HALL.x).powi(2) + (y - HALL.y).powi(2)).sqrt();
        // lowland round the mountains, flat enough for the highway
        let low = LOWLAND * smooth(fl, fl + 760.0, d);
        let mask_out = smooth(HWY_Y + 40.0, HWY_Y + 260.0, y) * smooth(fl + 600.0, fl + 1100.0, d);
        // the valley floor is level and gently rolling away from the river
        let mask_floor = 0.22 * smooth(110.0, 420.0, d) * smooth(300.0, 800.0, d_hall);
        let mut ground =
            low + hills * (mask_out * 0.6 + mask_floor * (1.0 - smooth(fl, fl + 80.0, d)));
        // the ridges: crest distance differs by side
        let (crest, back) = if side < 0.0 {
            (620.0, 380.0)
        } else {
            (780.0, 560.0)
        };
        let crest = crest * (1.0 + 0.12 * sin(x / 700.0 + v.ph[2]));
        let along = 0.62 + 0.38 * sin(x / 410.0 + v.ph[3]) * sin(x / 1130.0 + v.ph[0]);
        let u = ((d - fl) / crest).clamp(0.0, 1.0);
        // forested foot slopes steepen to bare upper faces
        let wall = u.powf(1.2) * (0.5 + 0.5 * smooth(0.0, 1.0, u));
        // spurs and gullies run down the slope: noise stretched along it, ridged so the crests
        // between gullies are sharp
        let warp = 0.7 * fbm(v.noise ^ 11, x / 260.0, y / 260.0, 2);
        let sp = fbm(v.noise, x / 150.0 + warp, y / 560.0, 3);
        let ridge = 1.0 - (2.0 * sp - 1.0).abs();
        let gully = 0.4 * smooth(0.12, 0.85, u);
        let relief = 1.0 - gully * (1.0 - ridge.powf(0.7));
        // crags: fine ridged noise on the upper faces
        let cr = fbm(v.noise ^ 77, x / 52.0, y / 52.0, 2);
        let crags = 30.0 * smooth(0.45, 1.0, u) * (1.0 - (2.0 * cr - 1.0).abs() - 0.45);
        let hpeak = v.peak * along * relief;
        let down = 1.0 - smooth(fl + crest, fl + crest + back, d);
        let mut summit = 0.0;
        for &(px, pd, ph, pr) in &v.peaks {
            let (qx, qy) = (x - px, d - (fl + pd.abs() * crest));
            if (pd < 0.0) != (side < 0.0) {
                continue;
            }
            let r = (qx * qx + qy * qy).sqrt() / pr;
            if r < 1.0 {
                summit += ph * (1.0 - r).powf(1.6);
            }
        }
        let mut mtn = (hpeak * wall + crags * wall + summit * wall) * down;
        // side valleys cut notches through the ridges
        let corr = |c: &(f64, f64, f64, f64, f64), yy: f64, xx: f64| -> f64 {
            let xc = c.0 + c.1 * sin(yy / c.2 + c.3);
            1.0 - smooth(c.4, c.4 + 300.0, (xx - xc).abs())
        };
        if side < 0.0 {
            mtn *= 1.0 - corr(&v.south, y, x);
        } else {
            mtn *= 1.0 - corr(&v.north, y, x);
        }
        // the highway runs on the lowland at the foot of the ridge
        mtn *= smooth(HWY_Y + 20.0, HWY_Y + 340.0, y);
        ground += mtn;
        ground
    }

    /// Ground height in metres (0 is roughly the river's floor).
    pub fn height(&self, x: f64, y: f64) -> f64 {
        match &self.shape {
            Shape::Plain => {}
            Shape::Gorge(g) => return self.gorge_height(g, x, y),
            Shape::Valley(v) => return self.valley_height(v, x, y),
        }
        let mut hills = 0.0;
        for (kx, ky, ph, a) in &self.waves {
            hills += a * sin(kx * x + ky * y + ph);
        }
        let d_hall = ((x - HALL.x).powi(2) + (y - HALL.y).powi(2)).sqrt();
        let dy = (y - self.river.y(x.clamp(0.0, S))).abs();
        let mut mask = smooth(450.0, 1500.0, d_hall)
            * smooth(HWY_Y + 120.0, HWY_Y + 700.0, y)
            * smooth(160.0, 520.0, dy);
        let mut base = 30.0 + 0.0 * x;
        // the valley: the land falls to the river over about half a kilometre each side
        base -= 28.0 * exp_neg((dy / 420.0).powi(2));
        if let Some((c, r)) = self.basin {
            let d = ((x - c.x).powi(2) + (y - c.y).powi(2)).sqrt();
            mask *= smooth(r * 1.4, r * 2.6, d);
            base -= 14.0 * exp_neg((d / (r * 2.2)).powi(2));
        }
        base + hills * mask
    }

    /// Height at a point.
    pub fn height_at(&self, p: Pt) -> f64 {
        self.height(p.x, p.y)
    }

    /// Gradient (rise per metre east, rise per metre north) by central differences.
    pub fn gradient(&self, p: Pt) -> Pt {
        let e = 6.0;
        pt(
            (self.height(p.x + e, p.y) - self.height(p.x - e, p.y)) / (2.0 * e),
            (self.height(p.x, p.y + e) - self.height(p.x, p.y - e)) / (2.0 * e),
        )
    }

    /// Steepness at a point: rise over run, so 0.1 is a ten percent grade.
    pub fn slope(&self, p: Pt) -> f64 {
        self.gradient(p).len()
    }

    /// Direction (radians, in [0, pi)) of the contour line through a point: along the hillside
    /// rather than up or down it. `None` on ground flatter than `min_slope`.
    pub fn contour_angle(&self, p: Pt, min_slope: f64) -> Option<f64> {
        let g = self.gradient(p);
        if g.len() < min_slope {
            return None;
        }
        let a = atan2(g.x, -g.y);
        Some(a.rem_euclid(std::f64::consts::PI))
    }

    /// Heights on an `n` by `n` grid over the whole map, row by row from y = 0, as f32.
    /// Sample (i, j) sits at x = i * size / (n - 1), y = j * size / (n - 1).
    pub fn heightfield(&self, n: usize) -> Vec<f32> {
        let n = n.max(2);
        let step = S / (n - 1) as f64;
        let mut out = Vec::with_capacity(n * n);
        for j in 0..n {
            for i in 0..n {
                out.push(self.height(i as f64 * step, j as f64 * step) as f32);
            }
        }
        out
    }
}

/// A stepped rise from 0 to 1: level ledges joined by steep risers, `steps` of them. `ledge` is
/// the share of each step that is level; `tilt` mixes in a little plain slope so ledges lean.
fn strata(u: f64, steps: f64, ledge: f64, tilt: f64) -> f64 {
    let u = u.clamp(0.0, 1.0);
    let s = u * steps;
    let i = s.floor().min(steps - 1.0);
    let f = s - i;
    let r = smooth(ledge, 1.0, f);
    (1.0 - tilt) * (i + r) / steps + tilt * u
}

/// Lattice value in [0, 1) at an integer point, from a seed alone.
fn lattice(seed: u64, ix: i64, iy: i64) -> f64 {
    let mut z = seed
        ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
}

/// Smooth value noise in [0, 1).
pub(crate) fn vnoise(seed: u64, x: f64, y: f64) -> f64 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i64, fy as i64);
    let (tx, ty) = (x - fx, y - fy);
    let (ux, uy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let a = lattice(seed, ix, iy);
    let b = lattice(seed, ix + 1, iy);
    let c = lattice(seed, ix, iy + 1);
    let d = lattice(seed, ix + 1, iy + 1);
    (a + (b - a) * ux) + ((c + (d - c) * ux) - (a + (b - a) * ux)) * uy
}

/// Value noise with `oct` octaves, back in [0, 1).
pub(crate) fn fbm(seed: u64, x: f64, y: f64, oct: usize) -> f64 {
    let (mut sum, mut amp, mut norm, mut f) = (0.0, 1.0, 0.0, 1.0);
    for o in 0..oct {
        sum += amp * vnoise(seed.wrapping_add(o as u64 * 0x51), x * f, y * f);
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    sum / norm
}

fn exp_neg(v: f64) -> f64 {
    libm::exp(-v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_at_hall_and_hilly_far_out() {
        let river = River::new(&mut Rng::new(1, 0x71e7));
        let t = Terrain::new(1, &river, None);
        assert!(t.slope(HALL) < 0.03);
        let mut steep = 0;
        for i in 0..40 {
            for j in 0..40 {
                if t.slope(pt(100.0 + i as f64 * 100.0, 400.0 + j as f64 * 60.0)) > 0.1 {
                    steep += 1;
                }
            }
        }
        assert!(steep > 10, "no steep ground at all: {steep}");
        assert!(steep < 800, "far too much steep ground: {steep}");
    }
}
