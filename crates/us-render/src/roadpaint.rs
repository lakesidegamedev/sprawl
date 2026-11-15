//! The look of roads: junctions with rounded kerbs, sidewalks, zebra crossings, lane markings by
//! class, planted medians on arterials and the island of a roundabout.
//!
//! All of it is drawing only. It reads the road lines as they are, so old saves get it too. A
//! junction is found where one road ends on another, two roads cross, or three or more meet.
//! A road that is being rebuilt keeps its roadworks look and takes no part in any of this.

use crate::ground::{Ground, layer};
use crate::live::RoadDraw;
use crate::obj::Rgb;
use crate::scene::{ASPHALT, ASPHALT_HWY, EDGE, YELLOW};
use std::collections::HashMap;
use us_gen::{Pt, RoadClass, pt};

pub const WHITE: Rgb = [236, 234, 226];
pub const SIDEWALK: Rgb = [198, 194, 186];
pub const KERB: Rgb = [166, 162, 158];
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

fn dashes(line: &[Pt], on: f64, off: f64) -> Vec<Vec<Pt>> {
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

/// A short collector that leaves a highway: a ramp.
fn is_ramp(r: &RoadDraw, all: &[&RoadDraw]) -> bool {
    r.class == RoadClass::Collector
        && r.points.len() == 15
        && all.iter().any(|h| {
            h.class == RoadClass::Highway
                && [r.points[0], r.points[14]]
                    .iter()
                    .any(|e| project(h.points, *e).1 < 1.5)
        })
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
                    arms.push(Arm {
                        road: j,
                        s0,
                        fwd,
                        dir: v.unit(),
                        hw: r.class.half_width(),
                        class: r.class,
                        avail,
                        ring,
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
        if theta < 25f64.to_radians() {
            // two roads leaving at a narrow angle: their kerbs meet in a point
            if s > 0.0 && u > 0.0 && s < 30.0 && u < 30.0 {
                left[i] = c;
                right[(i + 1) % k] = c;
            }
            continue;
        }
        if !fillets {
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
        cut.push(((right[i] - n.p).dot(ua)).max((left[i] - n.p).dot(ua)).max(0.0));
    }
    // a pad is always star shaped round its node: ordering by angle keeps it from twisting
    ring.sort_by(|a, b| {
        libm::atan2(a.y - n.p.y, a.x - n.p.x).total_cmp(&libm::atan2(b.y - n.p.y, b.x - n.p.x))
    });
    ring.dedup_by(|a, b| a.dist(*b) < 1e-6);
    Pad { ring, cut }
}

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
        g.polygon(&pad.ring, layer::ROAD + 0.005, ASPHALT_HWY);
        return;
    }
    g.polygon(&pad.ring, z::PAD, ASPHALT);
    let eligible = |a: &Arm| matches!(a.class, RoadClass::Local | RoadClass::Collector) && !a.ring;
    let streets = n
        .arms
        .iter()
        .filter(|a| a.class != RoadClass::Alley)
        .count();
    if big || streets < 3 {
        return;
    }
    for (k, a) in n.arms.iter().enumerate() {
        if !eligible(a) {
            continue;
        }
        let d = pad.cut[k] + 2.9;
        if a.avail < d + 4.0 {
            continue;
        }
        let across = pt(-a.dir.y, a.dir.x);
        let reach = a.hw - 0.9;
        let c = n.p + a.dir * d;
        let mut off = -reach;
        while off <= reach + 1e-6 {
            quad(g, c + across * off, a.dir, across, 1.5, 0.27, z::ZEBRA, WHITE);
            off += 1.1;
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
        let ramp = is_ramp(r, &all);
        let bridged = g.on_bridge(r.points);
        if matches!(r.class, RoadClass::Local | RoadClass::Collector) && !ramp && !bridged {
            for side in [-1.0f32, 1.0] {
                // a ring has its sidewalk on the outside only
                if let Some((_, ccw)) = ring
                    && (side > 0.0) == ccw
                {
                    continue;
                }
                g.ribbon(r.points, 1.0, side * (hw + 1.5), z::SIDEWALK, SIDEWALK);
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
        marks(g, i, r, ring.is_some(), ramp, &nodes);
    }
    for n in &nodes {
        draw_node(g, n, &all);
    }
}

fn marks(g: &mut Ground, i: usize, r: &RoadDraw, ring: bool, ramp: bool, nodes: &[Node]) {
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
            for side in [-1.0f32, 1.0] {
                g.ribbon(line, 0.09, side * 0.22, z::MARK, YELLOW);
                g.ribbon(line, 0.12, side * (hw - 0.7), z::MARK, WHITE);
                for d in dashes(line, 3.0, 6.0) {
                    g.ribbon(&d, 0.12, side * 5.0, z::MARK, WHITE);
                }
            }
            median(g, i, line, nodes);
        }
        RoadClass::Collector if !ring => {
            if ramp {
                // not across the highway it leaves
                g.ribbon(&sub_line(line, 16.0, len(line)), 0.1, 0.0, z::MARK, YELLOW);
            } else {
                g.ribbon(line, 0.1, 0.0, z::MARK, YELLOW);
            }
            if !ramp {
                for side in [-1.0f32, 1.0] {
                    g.ribbon(line, 0.1, side * (hw - 0.6), z::MARK, WHITE);
                }
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
                cuts.push((s - gap, s + gap));
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
                    .all(|p| p.dist(n.p) < 40.0 && p.x.is_finite() && p.y.is_finite()),
                "a pad point is far from its junction"
            );
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
