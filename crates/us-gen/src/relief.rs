//! The land and roads of the two mountain styles, the river gorge and the mountain valley:
//! where roads may run, which ground patches may be laid out on, and the switchback down the
//! gorge wall. Only these styles reach this module, so the older styles are untouched.

use crate::geom::{Pt, clip_line, cos, dedup, dist_pt_ring, length, point_in_ring, pt, sin};
use crate::rng::Rng;
use crate::terrain::{Ravine, Terrain};
use crate::world::{HALL, HWY_Y, River, S, wander_line};
use crate::{Preset, RoadClass};

/// What the generator needs to know about a mountain town besides its roads.
pub struct Relief {
    pub roads: Vec<(Vec<Pt>, RoadClass)>,
    pub rails: Vec<Vec<Pt>>,
    pub rail_angle: f64,
    /// Patches of land are cut to these (their union) ...
    pub allowed: Vec<Vec<Pt>>,
    /// ... and these are cut out of them.
    pub banned: Vec<Vec<Pt>>,
    /// The last this many `allowed` rings are not cut by `banned` (a gorge's riverside flat).
    pub exempt: usize,
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A ring from two curves over x: `lo(x)` below `hi(x)`.
fn ring_between(lo: impl Fn(f64) -> f64, hi: impl Fn(f64) -> f64, x0: f64, x1: f64) -> Vec<Pt> {
    let n = (((x1 - x0) / 20.0).ceil() as usize).max(2);
    let xs: Vec<f64> = (0..=n)
        .map(|i| x0 + (x1 - x0) * i as f64 / n as f64)
        .collect();
    let mut ring: Vec<Pt> = xs.iter().map(|&x| pt(x, lo(x))).collect();
    ring.extend(xs.iter().rev().map(|&x| pt(x, hi(x))));
    ring
}

/// A strip along a curve x = f(y) between y0 and y1, `half` metres each side.
fn strip_along_y(f: impl Fn(f64) -> f64, y0: f64, y1: f64, half: impl Fn(f64) -> f64) -> Vec<Pt> {
    let n = (((y1 - y0).abs() / 20.0).ceil() as usize).max(2);
    let ys: Vec<f64> = (0..=n)
        .map(|i| y0 + (y1 - y0) * i as f64 / n as f64)
        .collect();
    let mut ring: Vec<Pt> = ys.iter().map(|&y| pt(f(y) - half(y), y)).collect();
    ring.extend(ys.iter().rev().map(|&y| pt(f(y) + half(y), y)));
    ring
}

/// Bench width ahead of the descent gully at x: how far the flat floor reaches south of the river.
fn bench_ext(g: &Ravine, x: f64) -> f64 {
    g.bench * smooth(0.0, 1.0, 1.0 - (x - g.x0).abs() / 420.0)
}

fn within_map(line: &[Pt]) -> Vec<Vec<Pt>> {
    clip_line(line, &crate::world::map_ring(), true)
        .into_iter()
        .map(|l| dedup(&l))
        .filter(|l| l.len() >= 2)
        .collect()
}

/// Pieces of `line` that stay out of every ring in `zones`.
fn outside(line: &[Pt], zones: &[Vec<Pt>]) -> Vec<Vec<Pt>> {
    let mut pieces = vec![line.to_vec()];
    for z in zones {
        pieces = pieces
            .into_iter()
            .flat_map(|l| clip_line(&l, z, false))
            .collect();
    }
    pieces.into_iter().filter(|p| p.len() >= 2).collect()
}

/// Round the hairpins of a densely sampled zigzag: two Chaikin passes.
fn rounded(pts: &[Pt]) -> Vec<Pt> {
    let mut cur = pts.to_vec();
    for _ in 0..2 {
        let mut next = vec![cur[0]];
        for w in cur.windows(2) {
            next.push(w[0].lerp(w[1], 0.25));
            next.push(w[0].lerp(w[1], 0.75));
        }
        next.push(*cur.last().unwrap());
        cur = next;
    }
    cur
}

pub fn build(
    style: Preset,
    seed: u64,
    river: &River,
    terrain: &Terrain,
    pad: &mut f64,
) -> Option<Relief> {
    match style {
        Preset::Gorge => Some(gorge(seed, river, terrain, pad)),
        Preset::Valley => Some(valley(seed, river, terrain)),
        _ => None,
    }
}

fn gorge(seed: u64, river: &River, terrain: &Terrain, pad: &mut f64) -> Relief {
    let g = terrain.gorge().expect("a gorge town has a gorge");
    let gully = g
        .ravines
        .iter()
        .find(|r| r.linear)
        .expect("a gorge has a descent gully");
    let cut = g.rim * 1.3 + 48.0;
    *pad = cut - 60.0;
    // the south rim road swings round the head of the descent gully; the patches' edge only
    // makes room for the bench
    let south_cut = |x: f64| cut + bench_ext(gully, x) + 24.0;
    let head_off = |x: f64| {
        (62.0 + bench_ext(gully, x) + gully.len) * (1.0 + river.dy(x).powi(2)).sqrt() + 70.0
    };
    let south_off = |x: f64| {
        let around = head_off(x) * smooth(0.0, 1.0, (700.0 - (x - gully.x0).abs()) / 340.0);
        south_cut(x).max(around)
    };
    let north_off = |_x: f64| cut + 24.0;
    let mut roads: Vec<(Vec<Pt>, RoadClass)> = Vec::new();

    // the main road crosses the gorge on a high bridge at the town hall's x
    roads.push((vec![pt(HALL.x, HWY_Y), pt(HALL.x, S)], RoadClass::Arterial));
    // the rim roads
    let rim_line = |off: &dyn Fn(f64) -> f64, sign: f64| -> Vec<Pt> {
        (0..=204)
            .map(|i| {
                let x = S * i as f64 / 204.0;
                pt(x, river.y(x) + sign * off(x))
            })
            .collect()
    };
    for p in within_map(&rim_line(&south_off, -1.0)) {
        roads.push((p, RoadClass::Arterial));
    }
    for p in within_map(&rim_line(&north_off, 1.0)) {
        roads.push((p, RoadClass::Arterial));
    }

    // the switchback: zigzag legs across the descent gully, from the head down to the bench
    let zig: Vec<Pt> = gully.path.iter().map(|&(x, y, _)| pt(x, y)).collect();
    let mut descent = rounded(&zig);
    // down to the bench drive: a loop road on the flat, so it has no loose ends
    let o_low = |x: f64| 62.0 + 0.8 * bench_ext(gully, x) + 4.0;
    let o_up = |x: f64| 62.0 + 0.2 * bench_ext(gully, x) + 14.0;
    let foot_x = gully.path.last().map_or(gully.x0, |c| c.0);
    descent.push(pt(foot_x, river.y(foot_x) - o_low(foot_x)));
    let head = descent[0];
    roads.push((descent, RoadClass::Arterial));
    // the head joins the south rim road, which passes just beyond it
    roads.push((
        vec![head, pt(head.x, river.y(head.x) - south_off(head.x))],
        RoadClass::Arterial,
    ));
    let n = 24usize;
    let reach = gully.fhw * 0.8 + 14.0;
    let xs: Vec<f64> = (0..=n)
        .map(|i| gully.x0 + reach * (2.0 * i as f64 / n as f64 - 1.0))
        .collect();
    let mut ring: Vec<Pt> = xs.iter().map(|&x| pt(x, river.y(x) - o_low(x))).collect();
    ring.extend(xs.iter().rev().map(|&x| pt(x, river.y(x) - o_up(x))));
    ring.push(ring[0]);
    // the middle vertex of the lower side is where the switchback arrives
    roads.push((ring, RoadClass::Collector));

    // the rail stays on the south plateau, west of the tributary
    let trib = g
        .ravines
        .iter()
        .find(|r| r.len > 2000.0)
        .expect("a gorge has a tributary");
    let mut bans: Vec<Vec<Pt>> = Vec::new();
    for r in &g.ravines {
        bans.push(ravine_ring(r, river, if r.linear { 90.0 } else { 36.0 }));
    }
    let rail_angle = (3.0 + Rng::new(seed, 0x7a11).uniform(-2.0, 2.0)).to_radians();
    let mut rails = Vec::new();
    let line = wander_line(
        pt(0.0, 820.0 + Rng::new(seed, 0x7a12).uniform(-60.0, 60.0)),
        rail_angle,
        &mut Rng::new(seed, 2),
        0.05,
        Some(terrain),
    );
    let stop_x = trib.cx(1000.0) - 330.0;
    for piece in within_map(&line) {
        for p in outside(&piece, &bans) {
            let kept: Vec<Pt> = p.into_iter().filter(|q| q.x < stop_x).collect();
            if kept.len() >= 2 && length(&kept) > 400.0 {
                rails.push(kept);
            }
        }
    }

    // land that may carry a town: the two rims, and the flat bench at the foot of the gully
    let lo_far = |x: f64| river.y(x) - 4200.0;
    let south = ring_between(
        lo_far,
        |x| river.y(x) - south_cut(x) + 14.0,
        -10.0,
        S + 10.0,
    );
    let north = ring_between(
        |x| river.y(x) + north_off(x) - 14.0,
        |x| river.y(x) + 4200.0,
        -10.0,
        S + 10.0,
    );
    let bench = ring_between(
        |x| river.y(x) - 62.0 - bench_ext(gully, x) + 8.0,
        |x| river.y(x) - 56.0,
        gully.x0 - 420.0,
        gully.x0 + 420.0,
    );
    Relief {
        roads,
        rails,
        rail_angle,
        allowed: vec![south, north, bench],
        banned: bans,
        exempt: 1,
    }
}

/// The ground a ravine takes, with a margin.
fn ravine_ring(r: &Ravine, river: &River, margin: f64) -> Vec<Pt> {
    if r.linear {
        // the descent gully is measured from the river wherever it bends
        let steps = 44usize;
        let side_of = |sign: f64| -> Vec<Pt> {
            (0..=steps)
                .map(|i| {
                    let t = -125.0 + (r.len + 215.0) * i as f64 / steps as f64;
                    let s = (t / r.len).clamp(0.0, 1.0);
                    let x = r.cx(t.max(0.0)) + sign * (r.hw * (1.0 - 0.45 * s) + margin);
                    let m = river.dy(x);
                    let k = (1.0 + m * m).sqrt();
                    pt(x, river.y(x) + r.side * (62.0 + bench_ext(r, x) + t) * k)
                })
                .collect()
        };
        let at = |x: f64, t: f64| {
            let m = river.dy(x);
            let k = (1.0 + m * m).sqrt();
            pt(x, river.y(x) + r.side * (62.0 + bench_ext(r, x) + t) * k)
        };
        // the head and the mouth follow the river too, not a straight chord
        let across = |t: f64| -> Vec<Pt> {
            let s = (t / r.len).clamp(0.0, 1.0);
            let w = r.hw * (1.0 - 0.45 * s) + margin;
            (1..12)
                .map(|i| at(r.cx(t.max(0.0)) + w * (2.0 * i as f64 / 12.0 - 1.0), t))
                .collect()
        };
        let mut ring = side_of(-1.0);
        ring.extend(across(r.len + 90.0));
        ring.extend(side_of(1.0).into_iter().rev());
        ring.extend(across(-125.0).into_iter().rev());
        return ring;
    }
    let y_ref = river.y(r.x0);
    let foot = 62.0 + r.bench;
    let (y0, y1) = (
        y_ref + r.side * (foot - 125.0),
        y_ref + r.side * (foot + r.len + 90.0),
    );
    let t_of = |y: f64| (r.side * (y - y_ref) - foot).max(0.0);
    strip_along_y(
        |y| r.cx(t_of(y)),
        y0,
        y1,
        |y| {
            let s = (t_of(y) / r.len).clamp(0.0, 1.0);
            r.hw * (1.0 - 0.45 * s) + margin
        },
    )
}

fn valley(seed: u64, river: &River, terrain: &Terrain) -> Relief {
    let v = terrain.valley().expect("a valley town has a valley");
    let floor_at = |x: f64| {
        v.floor * (1.0 + 0.12 * sin(x / 610.0 + v.ph[0]) + 0.07 * sin(x / 230.0 + v.ph[1]))
    };
    let (sc, nc) = (v.south, v.north);
    let south_x = move |y: f64| sc.0 + sc.1 * sin(y / sc.2 + sc.3);
    let north_x = move |y: f64| nc.0 + nc.1 * sin(y / nc.2 + nc.3);
    let mut roads: Vec<(Vec<Pt>, RoadClass)> = Vec::new();
    // roads along both banks of the river, kept clear of its edge
    for sign in [-1.0, 1.0] {
        let line: Vec<Pt> = (0..=204)
            .map(|i| {
                let x = S * i as f64 / 204.0;
                pt(x, river.y(x) + sign * 205.0)
            })
            .collect();
        for p in within_map(&line) {
            roads.push((p, RoadClass::Arterial));
        }
    }
    // crossings of the river
    for x in [HALL.x, 640.0, 3350.0] {
        roads.push((
            vec![pt(x, river.y(x) - 205.0), pt(x, river.y(x) + 205.0)],
            RoadClass::Arterial,
        ));
    }
    // the side valleys: south to the highway, north to the edge of the map
    let mut y_s = river.y(sc.0) - 205.0;
    for _ in 0..8 {
        y_s = river.y(south_x(y_s)) - 205.0;
    }
    let south: Vec<Pt> = (0..=60)
        .map(|i| {
            let y = HWY_Y + (y_s - HWY_Y) * i as f64 / 60.0;
            pt(south_x(y), y)
        })
        .collect();
    roads.push((south, RoadClass::Arterial));
    let mut y_n = river.y(nc.0) + 205.0;
    for _ in 0..8 {
        y_n = river.y(north_x(y_n)) + 205.0;
    }
    let north: Vec<Pt> = (0..=120)
        .map(|i| {
            let y = y_n + (S - y_n) * i as f64 / 120.0;
            pt(north_x(y), y)
        })
        .collect();
    roads.push((north, RoadClass::Arterial));

    // the railway runs down the valley floor on the south side
    let rail_angle = 0.0;
    let mut rails = Vec::new();
    let line: Vec<Pt> = (0..=204)
        .map(|i| {
            let x = S * i as f64 / 204.0;
            pt(x, river.y(x) - 400.0)
        })
        .collect();
    for p in within_map(&line) {
        rails.push(p);
    }
    let _ = (seed, cos(0.0));

    // land that may carry a town: the floor, and the floors of the side valleys
    let floor = ring_between(
        |x| river.y(x) - (floor_at(x) - 36.0),
        |x| river.y(x) + (floor_at(x) - 36.0),
        -10.0,
        S + 10.0,
    );
    let south_valley = strip_along_y(south_x, HWY_Y - 10.0, y_s + 100.0, |_| sc.4 + 20.0);
    let north_valley = strip_along_y(north_x, y_n - 100.0, S + 10.0, |_| nc.4 + 20.0);
    let allowed = union_all(vec![floor, south_valley, north_valley]);
    Relief {
        roads,
        rails,
        rail_angle,
        allowed,
        banned: Vec::new(),
        exempt: 0,
    }
}

/// Union of rings, as rings.
fn union_all(rings: Vec<Vec<Pt>>) -> Vec<Vec<Pt>> {
    crate::plan::union_rings(&rings)
}

impl Relief {
    /// Ground that no street may be laid across: a ravine, or land well outside what the town
    /// may take (a gorge wall, a mountainside).
    pub fn forbids(&self, q: Pt) -> bool {
        if self.banned.iter().any(|z| point_in_ring(q, z)) {
            return true;
        }
        !self
            .allowed
            .iter()
            .any(|z| point_in_ring(q, z) || dist_pt_ring(q, z) < 14.0)
    }

    /// The part of a face of the road network that a patch may take.
    pub fn dry_parts(&self, face: &[Pt], river_band: &[Pt]) -> Vec<Vec<Pt>> {
        let mut out: Vec<Vec<Pt>> = Vec::new();
        let keep = self.allowed.len() - self.exempt;
        for (k, z) in self.allowed.iter().enumerate() {
            let mut part: Vec<Vec<Pt>> = Vec::new();
            for piece in crate::plan::intersect_rings(face, z) {
                part.extend(crate::plan::difference_rings(&piece, river_band));
            }
            if k < keep {
                for b in &self.banned {
                    part = part
                        .into_iter()
                        .flat_map(|q| crate::plan::difference_rings(&q, b))
                        .collect();
                }
            }
            out.extend(part);
        }
        out
    }
}
