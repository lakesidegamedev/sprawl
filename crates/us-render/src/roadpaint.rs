//! The look of roads: junctions with rounded kerbs, sidewalks, zebra crossings, lane markings by
//! class, planted medians on arterials and the island of a roundabout.
//!
//! All of it is drawing only. It reads the road lines as they are, so old saves get it too. A
//! junction is found where one road ends on another, two roads cross, or three or more meet.
//! A road that is being rebuilt keeps its roadworks look and takes no part in any of this.

use crate::ground::{Ground, layer};
use crate::live::RoadDraw;
use crate::obj::Rgb;
use crate::scene::{EDGE, YELLOW};
use std::collections::HashMap;
use us_gen::{Pt, RoadClass, pt};

pub const WHITE: Rgb = [236, 234, 226];
/// Junction paving: asphalt with one more blue, so the winter shader can lay slush on it. It is
/// drawn exactly like the road surface the rest of the year.
pub const PAD: Rgb = [108, 108, 121];
pub const SIDEWALK: Rgb = [198, 194, 186];
pub const KERB: Rgb = [166, 162, 158];
pub const SHOULDER: Rgb = [150, 141, 118];
pub const BARRIER: Rgb = [170, 168, 164];
pub const PLANT: Rgb = [100, 158, 82];
pub const SHRUB: Rgb = [66, 122, 64];

/// Where a road's lines may be drawn: layers above the road surface, bottom first.
mod z {
    pub const SIDEWALK: f32 = 0.25;
    pub const KERB: f32 = 0.26;
    pub const CORNER_EDGE: f32 = 0.27;
    pub const MARK: f32 = 0.36;
    pub const MEDIAN_KERB: f32 = 0.37;
    pub const MEDIAN: f32 = 0.38;
    pub const PAD: f32 = 0.39;
    pub const ZEBRA: f32 = 0.40;
}

fn len(line: &[Pt]) -> f64 {
    line.windows(2).map(|w| w[0].dist(w[1])).sum()
}

fn is_closed(line: &[Pt]) -> bool {
    line.len() > 3 && line[0].dist(*line.last().unwrap()) < 1e-3
}

/// Point at arclength `s` along the line, clamped to its ends.
fn at(line: &[Pt], s: f64) -> Pt {
    let mut left = s.max(0.0);
    for w in line.windows(2) {
        let l = w[0].dist(w[1]);
        if left <= l {
            return if l < 1e-12 {
                w[0]
            } else {
                w[0].lerp(w[1], left / l)
            };
        }
        left -= l;
    }
    *line.last().unwrap()
}

/// Closest point of the line to `p`: (arclength, distance, point).
fn project(line: &[Pt], p: Pt) -> (f64, f64, Pt) {
    let mut best = (0.0, f64::INFINITY, line[0]);
    let mut acc = 0.0;
    for w in line.windows(2) {
        let ab = w[1] - w[0];
        let l2 = ab.dot(ab);
        let t = if l2 < 1e-18 {
            0.0
        } else {
            ((p - w[0]).dot(ab) / l2).clamp(0.0, 1.0)
        };
        let q = w[0] + ab * t;
        let d = p.dist(q);
        if d < best.1 {
            best = (acc + l2.sqrt() * t, d, q);
        }
        acc += l2.sqrt();
    }
    best
}

/// The part of the line between arclengths `s0` and `s1`.
fn sub_line(line: &[Pt], s0: f64, s1: f64) -> Vec<Pt> {
    let mut out = vec![at(line, s0)];
    let mut acc = 0.0;
    for w in line.windows(2) {
        let l = w[0].dist(w[1]);
        if acc + l > s0 + 1e-9 && acc + l < s1 - 1e-9 {
            out.push(w[1]);
        }
        acc += l;
    }
    out.push(at(line, s1));
    out
}

pub(crate) fn dashes(line: &[Pt], on: f64, off: f64) -> Vec<Vec<Pt>> {
    let total = len(line);
    let mut out = Vec::new();
    let mut s = 0.0;
    while s + on * 0.5 < total {
        out.push(sub_line(line, s, (s + on).min(total)));
        s += on + off;
    }
    out
}

/// A closed road that is (an arc of) a roundabout ring: its centre and whether it runs
/// counter-clockwise.
pub fn ring_arc(r: &RoadDraw) -> Option<(Pt, bool)> {
    let p = r.points;
    let n = p.len();
    if r.class != RoadClass::Collector || n < 4 {
        return None;
    }
    let (a, b, c) = (p[0], p[n / 3], p[2 * n / 3]);
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-6 {
        return None;
    }
    let (a2, b2, c2) = (
        a.x * a.x + a.y * a.y,
        b.x * b.x + b.y * b.y,
        c.x * c.x + c.y * c.y,
    );
    let o = pt(
        (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    );
    let rad = o.dist(a);
    if !(15.0..=17.5).contains(&rad) || p.iter().any(|q| (q.dist(o) - rad).abs() > 0.4) {
        return None;
    }
    let ccw = (b - a).cross(c - b) > 0.0;
    Some((o, ccw))
}

/// A short collector that leaves a highway: a ramp. Its drawn line starts at the highway's
/// edge (see `ramp_cut`), so an end within the highway's width counts.
fn is_ramp(r: &RoadDraw, all: &[&RoadDraw]) -> bool {
    r.class == RoadClass::Collector
        && r.points.len() <= 15
        && r.points.len() >= 2
        && all.iter().any(|h| {
            h.class == RoadClass::Highway
                && [r.points[0], r.points[r.points.len() - 1]]
                    .iter()
                    .any(|e| project(h.points, *e).1 < RAMP_CUT + 1.5)
        })
}

/// Distance from a ramp's centre line to the highway's where the drawn slip lane starts: its
/// surface then meets the highway's outer lane in a taper, with no junction patch.
const RAMP_CUT: f64 = 16.0;

/// Half width of a slip lane as drawn: one lane, one way.
pub const RAMP_HW: f64 = 4.0;

/// Height of the patch that hides the highway's edge line along a slip lane's merge.
pub const GAP_Z: f32 = z::MARK + 0.01;

/// Length of the merge taper along the highway's edge, metres.
const TAPER: f64 = 75.0;

/// How a ramp meets its highway, for drawing.
pub struct RampCut {
    /// The line to draw: from where the slip lane's inner edge meets the highway's edge.
    pub line: Vec<Pt>,
    /// The merge wedge: a thin triangle along the highway's edge widening to the slip lane.
    pub wedge: [Pt; 3],
    /// The strip of the highway's edge line to paint over where the slip lane joins.
    pub gap: [Pt; 4],
}

/// Whether a (cut) ramp is an exit, traffic leaving the highway, or an entrance. Traffic keeps
/// right: on the side of the highway the ramp sits, it runs along the direction whose right is
/// that side; a ramp leaving in that direction is an exit. Also returns which end is the
/// highway's.
fn ramp_flow(r: &RoadDraw, all: &[&RoadDraw]) -> Option<(bool, bool)> {
    let p = r.points;
    for h in all.iter().filter(|h| h.class == RoadClass::Highway) {
        for front in [true, false] {
            let (e, nx) = if front {
                (p[0], p[1])
            } else {
                (p[p.len() - 1], p[p.len() - 2])
            };
            let (along, dist, q) = project(h.points, e);
            if dist > RAMP_CUT + 1.5 {
                continue;
            }
            let ht = tangent_at(h.points, along);
            let n = (e - q).unit();
            let v = if pt(ht.y, -ht.x).dot(n) > 0.0 {
                ht
            } else {
                ht * -1.0
            };
            let d = (nx - e).unit();
            return Some((d.dot(v) > 0.0, front));
        }
    }
    None
}

/// Unit direction of a polyline at a distance along it.
fn tangent_at(line: &[Pt], along: f64) -> Pt {
    let mut acc = 0.0;
    for w in line.windows(2) {
        let l = w[0].dist(w[1]);
        if acc + l >= along {
            return (w[1] - w[0]).unit();
        }
        acc += l;
    }
    let n = line.len();
    (line[n - 1] - line[n - 2]).unit()
}

/// The part of a ramp to draw: the line from where its inner edge meets the highway's edge, and
/// the merge wedge (a thin triangle along the highway's edge that widens to the ramp). `None`
/// for any road that is not a ramp (15 points, a collector, one end on a highway's centre line).
pub fn ramp_cut(r: &RoadDraw, highways: &[&[Pt]]) -> Option<RampCut> {
    if r.class != RoadClass::Collector || r.points.len() != 15 {
        return None;
    }
    let p = r.points;
    let (front, h) = highways.iter().find_map(|h| {
        if project(h, p[0]).1 < 1.5 {
            Some((true, *h))
        } else if project(h, p[p.len() - 1]).1 < 1.5 {
            Some((false, *h))
        } else {
            None
        }
    })?;
    let line: Vec<Pt> = if front {
        p.to_vec()
    } else {
        p.iter().rev().copied().collect()
    };
    // walk from the highway until the centre line is RAMP_CUT away from the highway's
    for k in 1..line.len() {
        let (a, b) = (line[k - 1], line[k]);
        let (da, db) = (project(h, a).1, project(h, b).1);
        if db >= RAMP_CUT {
            let t = if db > da {
                ((RAMP_CUT - da) / (db - da)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let start = a.lerp(b, t);
            let mut out = vec![start];
            out.extend_from_slice(&line[k..]);
            // the wedge: from the highway's edge some way back, out to both ramp edges
            let (along, _, q) = project(h, start);
            let ht = tangent_at(h, along);
            let n = (start - q).unit();
            let d = (b - a).unit();
            let mut rn = pt(-d.y, d.x);
            if rn.dot(n) > 0.0 {
                rn = rn * -1.0;
            }
            let hw_h = RoadClass::Highway.half_width();
            let back = if d.dot(ht) >= 0.0 { ht * -1.0 } else { ht };
            let edge0 = q + n * hw_h;
            let tip = edge0 + back * TAPER;
            let wedge = [tip, start + rn * RAMP_HW, start - rn * RAMP_HW];
            // the highway's edge line is broken along the taper, from its tip to the ramp
            let inner = n * -1.6;
            let gap = [
                tip + inner,
                tip,
                edge0 - back * 2.0,
                edge0 - back * 2.0 + inner,
            ];
            if !front {
                out.reverse();
            }
            return Some(RampCut {
                line: out,
                wedge,
                gap,
            });
        }
    }
    None
}

struct Arm {
    road: usize,
    /// Arclength of the node along its road, and whether the arm runs toward the road's end.
    s0: f64,
    fwd: bool,
    dir: Pt,
    hw: f64,
    class: RoadClass,
    avail: f64,
    ring: bool,
    /// How far from the node the road keeps to the arm's heading (within two metres), metres.
    /// Paving and markings worked out from the heading are only right this far.
    straight: f64,
}

struct Node {
    p: Pt,
    arms: Vec<Arm>,
}

struct Grid {
    cells: HashMap<(i32, i32), Vec<(u32, u32)>>,
}

const CELL: f64 = 64.0;

impl Grid {
    fn new(all: &[&RoadDraw]) -> Grid {
        let mut cells: HashMap<(i32, i32), Vec<(u32, u32)>> = HashMap::new();
        for (ri, r) in all.iter().enumerate() {
            for (si, w) in r.points.windows(2).enumerate() {
                let (x0, x1) = (w[0].x.min(w[1].x), w[0].x.max(w[1].x));
                let (y0, y1) = (w[0].y.min(w[1].y), w[0].y.max(w[1].y));
                for cx in ((x0 - 2.0) / CELL).floor() as i32..=((x1 + 2.0) / CELL).floor() as i32 {
                    for cy in
                        ((y0 - 2.0) / CELL).floor() as i32..=((y1 + 2.0) / CELL).floor() as i32
                    {
                        cells
                            .entry((cx, cy))
                            .or_default()
                            .push((ri as u32, si as u32));
                    }
                }
            }
        }
        Grid { cells }
    }

    fn near(&self, p: Pt) -> &[(u32, u32)] {
        self.cells
            .get(&((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32))
            .map_or(&[], |v| v.as_slice())
    }

    /// Segments registered in any cell the segment a-b touches.
    fn along(&self, a: Pt, b: Pt) -> Vec<(u32, u32)> {
        let mut out: Vec<(u32, u32)> = Vec::new();
        let n = ((a.dist(b) / (CELL * 0.5)).ceil() as usize).max(1);
        let mut last = (i32::MIN, i32::MIN);
        for k in 0..=n {
            let p = a.lerp(b, k as f64 / n as f64);
            let c = ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32);
            if c != last {
                last = c;
                out.extend_from_slice(self.near(p));
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

fn seg_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<Pt> {
    let r = b - a;
    let s = d - c;
    let den = r.cross(s);
    if den.abs() < 1e-9 {
        return None;
    }
    let qp = c - a;
    let t = qp.cross(s) / den;
    let u = qp.cross(r) / den;
    ((0.02..0.98).contains(&t) && (0.02..0.98).contains(&u)).then(|| a + r * t)
}

/// Junctions of three or more roads that involve at least one of the first `n_mine` roads.
fn find_nodes(all: &[&RoadDraw], n_mine: usize) -> Vec<Node> {
    let grid = Grid::new(all);
    let mut raw: Vec<Pt> = Vec::new();
    for (i, r) in all.iter().enumerate() {
        let closed = is_closed(r.points);
        let ends = [r.points[0], *r.points.last().unwrap()];
        if !closed {
            for e in ends {
                let mut hit = false;
                for &(j, s) in grid.near(e) {
                    let j = j as usize;
                    if j == i || (i >= n_mine && j >= n_mine) {
                        continue;
                    }
                    let w = &all[j].points[s as usize..s as usize + 2];
                    let ab = w[1] - w[0];
                    let l2 = ab.dot(ab);
                    let t = if l2 < 1e-18 {
                        0.0
                    } else {
                        ((e - w[0]).dot(ab) / l2).clamp(0.0, 1.0)
                    };
                    if e.dist(w[0] + ab * t) < 1.6 {
                        hit = true;
                    }
                }
                if hit {
                    raw.push(e);
                }
            }
        }
        if i >= n_mine {
            continue;
        }
        for w in r.points.windows(2) {
            for (j, s) in grid.along(w[0], w[1]) {
                let j = j as usize;
                if j == i {
                    continue;
                }
                let v = &all[j].points[s as usize..s as usize + 2];
                if let Some(x) = seg_cross(w[0], w[1], v[0], v[1]) {
                    raw.push(x);
                }
            }
        }
    }
    let mut centres: Vec<Pt> = Vec::new();
    for p in raw {
        if centres.iter().all(|c| c.dist(p) > 4.0) {
            centres.push(p);
        }
    }
    let mut nodes: Vec<Node> = Vec::new();
    for p in centres {
        let mut arms: Vec<Arm> = Vec::new();
        let mut mine = false;
        // roads near this point, found through the grid cells around it
        let mut seen: Vec<usize> = Vec::new();
        for dx in [-1.0, 0.0, 1.0] {
            for dy in [-1.0, 0.0, 1.0] {
                for &(j, _) in grid.near(pt(p.x + dx * CELL, p.y + dy * CELL)) {
                    if !seen.contains(&(j as usize)) {
                        seen.push(j as usize);
                    }
                }
            }
        }
        seen.sort_unstable();
        for j in seen {
            let r = all[j];
            let (s0, d, _) = project(r.points, p);
            if d > 3.0 {
                continue;
            }
            let total = len(r.points);
            let closed = is_closed(r.points);
            let ring = ring_arc(r).is_some();
            let look = 9.0;
            let mut push = |fwd: bool, avail: f64| {
                let s = if fwd { s0 + look } else { s0 - look };
                let q = if closed {
                    at(r.points, s.rem_euclid(total))
                } else {
                    at(r.points, s.clamp(0.0, total))
                };
                let v = q - p;
                if v.len() > 1.0 {
                    let dir = v.unit();
                    let mut straight = look;
                    while straight < 180.0 {
                        let t = straight + 6.0;
                        let st = if fwd { s0 + t } else { s0 - t };
                        if !closed && !(0.0..=total).contains(&st) {
                            break;
                        }
                        let on_road = if closed {
                            at(r.points, st.rem_euclid(total))
                        } else {
                            at(r.points, st)
                        };
                        if on_road.dist(p + dir * t) > 2.0 {
                            break;
                        }
                        straight = t;
                    }
                    arms.push(Arm {
                        road: j,
                        s0,
                        fwd,
                        dir: v.unit(),
                        hw: r.class.half_width(),
                        class: r.class,
                        avail,
                        ring,
                        straight,
                    });
                }
            };
            if closed {
                push(true, total / 4.0);
                push(false, total / 4.0);
            } else {
                if total - s0 > 1.5 {
                    push(true, total - s0);
                }
                if s0 > 1.5 {
                    push(false, s0);
                }
            }
            if j < n_mine {
                mine = true;
            }
        }
        if arms.len() >= 3 && mine {
            arms.sort_by(|a, b| {
                libm::atan2(a.dir.y, a.dir.x).total_cmp(&libm::atan2(b.dir.y, b.dir.x))
            });
            nodes.push(Node { p, arms });
        }
    }
    // an arm is only as long as the way to the next junction on its road
    let mut stops: Vec<Vec<f64>> = vec![Vec::new(); all.len()];
    for n in &nodes {
        for a in &n.arms {
            stops[a.road].push(a.s0);
        }
    }
    for v in &mut stops {
        v.sort_by(|a, b| a.total_cmp(b));
    }
    for n in &mut nodes {
        for a in &mut n.arms {
            let v = &stops[a.road];
            let next = if a.fwd {
                v.iter().find(|&&s| s > a.s0 + 4.0).map(|s| s - a.s0)
            } else {
                v.iter().rev().find(|&&s| s < a.s0 - 4.0).map(|s| a.s0 - s)
            };
            if let Some(d) = next {
                a.avail = a.avail.min(d);
            }
        }
    }
    nodes
}

struct Pad {
    ring: Vec<Pt>,
    /// Distance from the node at which each arm's kerbs leave the pad.
    cut: Vec<f64>,
}

fn pad_of(n: &Node, fillets: bool) -> Pad {
    let k = n.arms.len();
    let tau = std::f64::consts::TAU;
    let ang = |a: &Arm| libm::atan2(a.dir.y, a.dir.x);
    // per arm: right edge point and left edge point; per corner: the arc between them
    let mut right: Vec<Pt> = vec![n.p; k];
    let mut left: Vec<Pt> = vec![n.p; k];
    let mut arcs: Vec<Vec<Pt>> = vec![Vec::new(); k];
    for i in 0..k {
        let (a, b) = (&n.arms[i], &n.arms[(i + 1) % k]);
        let theta = (ang(b) - ang(a)).rem_euclid(tau);
        let (ua, ub) = (a.dir, b.dir);
        let na = pt(-ua.y, ua.x);
        let nb = pt(ub.y, -ub.x);
        let la = n.p + na * a.hw;
        let rb = n.p + nb * b.hw;
        left[i] = la;
        right[(i + 1) % k] = rb;
        if theta > 165f64.to_radians() {
            continue;
        }
        let rhs = nb * b.hw - na * a.hw;
        let den = ua.cross(ub);
        if den.abs() < 1e-6 {
            continue;
        }
        let s = rhs.cross(ub) / den;
        let u = rhs.cross(ua) / den;
        let c = la + ua * s;
        // Two roads leaving at a narrow angle run over each other until their kerbs part. The
        // pad reaches to that point, however far, so one road's lines, median and kerb are
        // never drawn across the other; only the room the arms have limits it.
        let meet = s > 0.0
            && u > 0.0
            && s < a.avail.min(a.straight).min(170.0)
            && u < b.avail.min(b.straight).min(170.0);
        if theta < 25f64.to_radians() {
            if meet {
                left[i] = c;
                right[(i + 1) % k] = c;
            }
            continue;
        }
        if !fillets {
            if meet && theta < 60f64.to_radians() {
                left[i] = c;
                right[(i + 1) % k] = c;
            }
            continue;
        }
        let half = theta / 2.0;
        let mut rad = (0.85 * a.hw.max(b.hw) + 1.0).min(16.0 * libm::tan(half));
        let mut t = rad / libm::tan(half);
        // keep the fillet inside what the arms have room for
        let room = (0.6 * a.avail - s).min(0.6 * b.avail - u);
        if t > room {
            t = room;
            rad = t * libm::tan(half);
        }
        if t < 1.0 || s + t < 0.0 || u + t < 0.0 || s + t > 30.0 || u + t > 30.0 {
            if meet && theta < 60f64.to_radians() {
                // no room for a rounded corner: at least close the wedge between the kerbs
                left[i] = c;
                right[(i + 1) % k] = c;
            }
            continue;
        }
        let ta = c + ua * t;
        let tb = c + ub * t;
        let o = c + (ua + ub).unit() * (rad / libm::sin(half));
        let (a0, a1) = (
            libm::atan2(ta.y - o.y, ta.x - o.x),
            libm::atan2(tb.y - o.y, tb.x - o.x),
        );
        let mut d = a1 - a0;
        while d > std::f64::consts::PI {
            d -= tau;
        }
        while d < -std::f64::consts::PI {
            d += tau;
        }
        left[i] = ta;
        right[(i + 1) % k] = tb;
        let steps = ((d.abs() / 0.3).ceil() as usize).max(2);
        for m in 1..steps {
            let q = a0 + d * m as f64 / steps as f64;
            arcs[i].push(pt(o.x + rad * libm::cos(q), o.y + rad * libm::sin(q)));
        }
    }
    let mut ring = Vec::new();
    let mut cut = Vec::new();
    for i in 0..k {
        ring.push(right[i]);
        ring.push(left[i]);
        ring.extend(arcs[i].iter().copied());
        let ua = n.arms[i].dir;
        cut.push(
            ((right[i] - n.p).dot(ua))
                .max((left[i] - n.p).dot(ua))
                .max(0.0),
        );
    }
    // a pad is always star shaped round its node: ordering by angle keeps it from twisting
    ring.sort_by(|a, b| {
        libm::atan2(a.y - n.p.y, a.x - n.p.x).total_cmp(&libm::atan2(b.y - n.p.y, b.x - n.p.x))
    });
    ring.dedup_by(|a, b| a.dist(*b) < 1e-6);
    Pad { ring, cut }
}

#[allow(clippy::too_many_arguments)]
fn quad(g: &mut Ground, c: Pt, along: Pt, across: Pt, la: f64, lc: f64, z: f32, col: Rgb) {
    let (a, b) = (along * la, across * lc);
    g.polygon(&[c - a - b, c + a - b, c + a + b, c - a + b], z, col);
}

fn near_water(g: &Ground, p: Pt, r: f64) -> bool {
    [(0.0, 0.0), (r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)]
        .iter()
        .any(|(dx, dy)| g.water_at(pt(p.x + dx, p.y + dy)))
}

/// Junctions: kerb corners, the pad that hides lines across them, zebra crossings.
fn draw_node(g: &mut Ground, n: &Node, all: &[&RoadDraw]) {
    if n.arms.iter().any(|a| all[a.road].works.is_some()) || near_water(g, n.p, 35.0) {
        return;
    }
    let mut pad = pad_of(n, true);
    if crate::ground::signed_area(&pad.ring) <= 0.0 {
        // crowded arms: square corners rather than a twisted shape
        pad = pad_of(n, false);
        if crate::ground::signed_area(&pad.ring) <= 0.0 {
            return;
        }
    }
    let big = n
        .arms
        .iter()
        .any(|a| matches!(a.class, RoadClass::Highway | RoadClass::Arterial));
    let highway = n.arms.iter().any(|a| a.class == RoadClass::Highway);
    g.polygon(&Ground::grow(&pad.ring, 0.5), z::CORNER_EDGE, EDGE);
    if highway {
        // the highway keeps its lines through a ramp junction
        g.polygon(&pad.ring, layer::ROAD + 0.005, PAD);
        return;
    }
    g.polygon(&pad.ring, z::PAD, PAD);
    let eligible = |a: &Arm| {
        matches!(
            a.class,
            RoadClass::Local | RoadClass::Collector | RoadClass::Arterial
        ) && !a.ring
    };
    let streets = n
        .arms
        .iter()
        .filter(|a| a.class != RoadClass::Alley)
        .count();
    if streets < 3 {
        return;
    }
    for (k, a) in n.arms.iter().enumerate() {
        if !eligible(a) {
            continue;
        }
        let d = pad.cut[k] + 2.9;
        // room for a crossing at each end of the block, not on top of the next one
        if a.avail < 2.0 * d + 3.0 {
            continue;
        }
        // a crossing belongs at the mouth of a compact junction. Where the paving runs far
        // along an arm (roads parting at a narrow angle) it would sit out on its own.
        if pad.cut[k] > a.hw * 1.5 + 13.0 {
            continue;
        }
        // and only where the road is still on the heading the marking is laid out along
        if d + 4.5 > a.straight {
            continue;
        }
        let across = pt(-a.dir.y, a.dir.x);
        let reach = a.hw - 0.9;
        let c = n.p + a.dir * d;
        // a crossing never lies on another arm of the junction
        let on_other_arm = n.arms.iter().enumerate().any(|(j, o)| {
            j != k
                && [c - across * reach, c, c + across * reach].iter().any(|q| {
                    let rel = *q - n.p;
                    let t = rel.dot(o.dir).max(0.0);
                    (rel - o.dir * t).len() < o.hw + 1.2
                })
        });
        if on_other_arm {
            continue;
        }
        let mut off = -reach;
        while off <= reach + 1e-6 {
            quad(
                g,
                c + across * off,
                a.dir,
                across,
                1.5,
                0.27,
                z::ZEBRA,
                WHITE,
            );
            off += 1.1;
        }
        if big && a.avail > 2.0 * d + 12.0 {
            // a stop line across the lanes that come in to a main junction
            let half = a.hw * 0.5;
            quad(
                g,
                n.p + a.dir * (d + 2.6) + across * half,
                a.dir,
                across,
                0.22,
                half - 0.6,
                z::ZEBRA,
                WHITE,
            );
        }
    }
}

fn circle(c: Pt, r: f64) -> Vec<Pt> {
    (0..28)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / 28.0;
            pt(c.x + r * libm::cos(a), c.y + r * libm::sin(a))
        })
        .collect()
}

/// Everything that goes on top of the road surface, for the roads of one chunk. `context`
/// are roads of other chunks: they are never drawn here, they only take part in junctions.
pub fn paint(g: &mut Ground, roads: &[RoadDraw], context: &[RoadDraw]) {
    let all: Vec<&RoadDraw> = roads
        .iter()
        .filter(|r| r.class != RoadClass::Rail && r.points.len() >= 2)
        .chain(
            context
                .iter()
                .filter(|r| r.class != RoadClass::Rail && r.points.len() >= 2),
        )
        .collect();
    let n_mine = roads
        .iter()
        .filter(|r| r.class != RoadClass::Rail && r.points.len() >= 2)
        .count();
    let nodes = find_nodes(&all, n_mine);
    for (i, r) in all.iter().take(n_mine).enumerate() {
        if r.works.is_some() {
            continue;
        }
        let hw = r.class.half_width() as f32;
        let ring = ring_arc(r);
        let ramp_at = if is_ramp(r, &all) {
            ramp_flow(r, &all)
        } else {
            None
        };
        let ramp = ramp_at.is_some();
        let bridged = g.on_bridge(r.points);
        if matches!(r.class, RoadClass::Arterial | RoadClass::Highway) && !bridged && ring.is_none()
        {
            // a gravel shoulder beside a main road, so the carriageway does not end in a clean
            // line on grass
            for side in [-1.0f32, 1.0] {
                g.ribbon(
                    r.points,
                    0.9,
                    side * (hw + 1.2),
                    z::SIDEWALK - 0.03,
                    SHOULDER,
                );
            }
        }
        if matches!(r.class, RoadClass::Local | RoadClass::Collector) && !ramp && !bridged {
            for side in [-1.0f32, 1.0] {
                // a ring has its sidewalk on the outside only
                if let Some((_, ccw)) = ring
                    && (side > 0.0) == ccw
                {
                    continue;
                }
                // the blue runs 181 (road side) to 191 (outer side) across the strip so the winter
                // shader can wear a footpath along it; the colour is the sidewalk colour to the eye
                let (right, left) = if side > 0.0 {
                    (
                        [SIDEWALK[0], SIDEWALK[1], 181],
                        [SIDEWALK[0], SIDEWALK[1], 191],
                    )
                } else {
                    (
                        [SIDEWALK[0], SIDEWALK[1], 191],
                        [SIDEWALK[0], SIDEWALK[1], 181],
                    )
                };
                g.ribbon_across(r.points, 1.0, side * (hw + 1.5), z::SIDEWALK, right, left);
                g.ribbon(r.points, 0.18, side * (hw + 0.4), z::KERB, KERB);
            }
        }
        if let Some((o, _)) = ring {
            g.polygon(&circle(o, 9.9), z::MEDIAN_KERB, KERB);
            g.polygon(&circle(o, 9.0), z::MEDIAN, PLANT);
            for k in 0..5 {
                let a = std::f64::consts::TAU * k as f64 / 5.0 + 0.4;
                let c = pt(o.x + 5.2 * libm::cos(a), o.y + 5.2 * libm::sin(a));
                g.polygon(&circle(c, 1.5), z::MEDIAN + 0.01, SHRUB);
            }
            g.polygon(&circle(o, 2.2), z::MEDIAN + 0.01, SHRUB);
        }
        marks(g, i, r, ring.is_some(), ramp_at, &nodes);
    }
    for n in &nodes {
        draw_node(g, n, &all);
    }
}

/// A white lane arrow at `p` pointing along `dir`, painted on the road.
fn arrow(g: &mut Ground, p: Pt, dir: Pt) {
    let side = pt(-dir.y, dir.x);
    let shaft = [
        p - dir * 2.6 + side * 0.18,
        p - dir * 2.6 - side * 0.18,
        p + dir * 0.6 - side * 0.18,
        p + dir * 0.6 + side * 0.18,
    ];
    let head = [
        p + dir * 2.6,
        p + dir * 0.4 - side * 0.8,
        p + dir * 0.4 + side * 0.8,
    ];
    g.polygon(&shaft, z::MARK, WHITE);
    g.polygon(&head, z::MARK, WHITE);
}

/// Point at a distance along a polyline.
fn point_at(line: &[Pt], along: f64) -> Pt {
    let mut acc = 0.0;
    for w in line.windows(2) {
        let l = w[0].dist(w[1]);
        if acc + l >= along {
            return w[0].lerp(w[1], ((along - acc) / l.max(1e-9)).clamp(0.0, 1.0));
        }
        acc += l;
    }
    line[line.len() - 1]
}

/// Keep arterial striping clear of other arterial approaches, even on skew junctions.
fn arterial_marking_segments(road: usize, line: &[Pt], nodes: &[Node]) -> Vec<Vec<Pt>> {
    let total = len(line);
    let mut cuts = Vec::new();
    for n in nodes {
        for a in n.arms.iter().filter(|a| a.road == road) {
            let reach = n
                .arms
                .iter()
                .filter(|b| b.road != road && b.class == RoadClass::Arterial)
                .map(|b| b.hw / a.dir.cross(b.dir).abs().max(0.48) + 1.2)
                .fold(0.0, f64::max);
            if reach > 0.0 {
                let (lo, hi) = if a.fwd {
                    (a.s0, a.s0 + reach)
                } else {
                    (a.s0 - reach, a.s0)
                };
                cuts.push((lo.clamp(0.0, total), hi.clamp(0.0, total)));
            }
        }
    }
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut pieces = Vec::new();
    let mut start = 0.0;
    for (lo, hi) in cuts {
        if lo - start > 0.3 {
            pieces.push(sub_line(line, start, lo));
        }
        start = start.max(hi);
    }
    if total - start > 0.3 {
        pieces.push(sub_line(line, start, total));
    }
    pieces
}

fn marks(
    g: &mut Ground,
    i: usize,
    r: &RoadDraw,
    ring: bool,
    ramp: Option<(bool, bool)>,
    nodes: &[Node],
) {
    let hw = r.class.half_width() as f32;
    let line = r.points;
    match r.class {
        RoadClass::Highway => {
            g.ribbon(line, 0.6, 0.0, z::MARK + 0.02, BARRIER);
            for side in [-1.0f32, 1.0] {
                g.ribbon(line, 0.14, side * (hw - 0.8), z::MARK, WHITE);
                for d in dashes(line, 3.0, 9.0) {
                    g.ribbon(&d, 0.12, side * hw * 0.5, z::MARK, WHITE);
                }
            }
        }
        RoadClass::Arterial => {
            for segment in arterial_marking_segments(i, line, nodes) {
                let stripes = dashes(&segment, 3.0, 6.0);
                for side in [-1.0f32, 1.0] {
                    g.ribbon(&segment, 0.09, side * 0.22, z::MARK, YELLOW);
                    g.ribbon(&segment, 0.12, side * (hw - 0.7), z::MARK, WHITE);
                    for d in &stripes {
                        g.ribbon(d, 0.12, side * 5.0, z::MARK, WHITE);
                    }
                }
            }
            median(g, i, line, nodes);
        }
        RoadClass::Collector if ramp.is_some() => {
            // a one-way slip lane: white edge lines and arrows the way traffic runs
            let (exit, front) = ramp.unwrap();
            let rw = RAMP_HW as f32;
            for side in [-1.0f32, 1.0] {
                g.ribbon(line, 0.12, side * (rw - 0.5), z::MARK, WHITE);
            }
            let total = len(line);
            for at in [22.0, 70.0] {
                if at + 6.0 > total {
                    continue;
                }
                let s = if front { at } else { total - at };
                let p = point_at(line, s);
                let mut dir = tangent_at(line, s);
                // from the highway's end outward: an exit points that way, an entrance back
                if !front {
                    dir = dir * -1.0;
                }
                if !exit {
                    dir = dir * -1.0;
                }
                arrow(g, p, dir);
            }
        }
        RoadClass::Collector if !ring => {
            g.ribbon(line, 0.1, 0.0, z::MARK, YELLOW);
            for side in [-1.0f32, 1.0] {
                g.ribbon(line, 0.1, side * (hw - 0.6), z::MARK, WHITE);
            }
        }
        RoadClass::Collector => {
            for side in [-1.0f32, 1.0] {
                g.ribbon(line, 0.1, side * (hw - 0.6), z::MARK, WHITE);
            }
        }
        _ => {}
    }
}

/// A planted median down the middle of an arterial, left open where roads join it.
fn median(g: &mut Ground, road: usize, line: &[Pt], nodes: &[Node]) {
    let total = len(line);
    let mut cuts: Vec<(f64, f64)> = vec![(0.0, 16.0), (total - 16.0, total)];
    for n in nodes {
        if n.arms.iter().any(|a| a.road == road) {
            let (s, d, _) = project(line, n.p);
            if d < 3.0 {
                let gap = n
                    .arms
                    .iter()
                    .filter(|a| a.road != road)
                    .map(|a| a.hw)
                    .fold(0.0, f64::max)
                    + 16.0;
                // the median stops short of the whole junction, however far its paving
                // reaches along this road (a road joining at a narrow angle makes it long)
                let mut pad = pad_of(n, true);
                if crate::ground::signed_area(&pad.ring) <= 0.0 {
                    pad = pad_of(n, false);
                }
                let (mut back, mut ahead) = (gap, gap);
                for (k, a) in n.arms.iter().enumerate() {
                    if a.road == road {
                        let reach = pad.cut[k] + 9.0;
                        if a.fwd {
                            ahead = ahead.max(reach);
                        } else {
                            back = back.max(reach);
                        }
                    }
                }
                cuts.push((s - back, s + ahead));
            }
        }
    }
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut s = 0.0;
    let mut pieces: Vec<(f64, f64)> = Vec::new();
    for (a, b) in cuts {
        if a > s {
            pieces.push((s, a));
        }
        s = s.max(b);
    }
    if s < total {
        pieces.push((s, total));
    }
    for (a, b) in pieces {
        if b - a < 40.0 {
            continue;
        }
        let p = sub_line(line, a, b);
        g.ribbon(&p, 1.75, 0.0, z::MEDIAN_KERB, KERB);
        g.ribbon(&p, 1.5, 0.0, z::MEDIAN, PLANT);
        // a rounded nose at each end instead of a square cut
        for end in [p[0], p[p.len() - 1]] {
            g.polygon(&circle(end, 1.75), z::MEDIAN_KERB, KERB);
            g.polygon(&circle(end, 1.5), z::MEDIAN, PLANT);
        }
        for d in dashes(&p, 1.6, 5.0) {
            g.ribbon(&d, 0.6, 0.0, z::MEDIAN + 0.01, SHRUB);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::{Preset, generate};

    fn draws(t: &us_gen::Town) -> Vec<RoadDraw<'_>> {
        t.roads
            .iter()
            .map(|r| RoadDraw {
                class: r.class,
                points: &r.points,
                works: None,
            })
            .collect()
    }

    #[test]
    fn a_roundabout_is_found_with_its_arms_and_no_junction_twice() {
        let t = generate(Preset::Grid, 1);
        let d = draws(&t);
        let all: Vec<&RoadDraw> = d.iter().filter(|r| r.class != RoadClass::Rail).collect();
        let nodes = find_nodes(&all, all.len());
        let (o, _) = all.iter().find_map(|r| ring_arc(r)).expect("a roundabout");
        let near: Vec<&Node> = nodes.iter().filter(|n| n.p.dist(o) < 20.0).collect();
        assert!(near.len() >= 3, "{} junctions on the ring", near.len());
        for n in &near {
            assert!(n.arms.len() >= 3);
            assert_eq!(n.arms.iter().filter(|a| a.ring).count(), 2);
        }
        for (i, a) in nodes.iter().enumerate() {
            for b in &nodes[i + 1..] {
                assert!(a.p.dist(b.p) > 4.0, "two junctions on one spot");
            }
        }
    }

    #[test]
    fn pads_are_simple_and_cover_the_node() {
        let t = generate(Preset::Grid, 1);
        let d = draws(&t);
        let all: Vec<&RoadDraw> = d.iter().filter(|r| r.class != RoadClass::Rail).collect();
        let (mut bad, mut worse, mut total) = (0, 0, 0);
        for n in find_nodes(&all, all.len()) {
            let pad = pad_of(&n, true);
            assert!(pad.ring.len() >= 4);
            assert!(
                pad.ring
                    .iter()
                    .all(|p| p.dist(n.p) < 175.0 && p.x.is_finite() && p.y.is_finite()),
                "a pad point is far from its junction"
            );
            // only roads that part at a narrow angle share a long wedge of pad, and it never
            // reaches past where either road leaves its heading
            let reach = pad.ring.iter().map(|p| p.dist(n.p)).fold(0.0, f64::max);
            if reach >= 40.0 {
                let narrow = n.arms.iter().enumerate().any(|(i, a)| {
                    n.arms
                        .iter()
                        .skip(i + 1)
                        .any(|b| a.dir.dot(b.dir) > 25f64.to_radians().cos())
                });
                assert!(
                    narrow,
                    "a long pad at a junction with no narrow pair of arms"
                );
                let limit = n.arms.iter().map(|a| a.straight).fold(0.0, f64::max);
                assert!(
                    reach <= limit + 25.0,
                    "a pad runs past the straight of its arms"
                );
            }
            let square = pad_of(&n, false);
            if crate::ground::signed_area(&pad.ring) <= 0.0 {
                bad += 1;
                if crate::ground::signed_area(&square.ring) <= 0.0 {
                    worse += 1;
                }
            }
            total += 1;
        }
        eprintln!("{bad} of {total} pads fall back, {worse} have no pad");
        assert_eq!(bad, 0, "{bad} pads twist");
        assert_eq!(worse, 0);
    }
}
