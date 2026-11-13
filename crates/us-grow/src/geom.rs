//! Small geometry helpers and spatial grids for candidate checks.

use std::collections::HashMap;
use us_gen::{Pt, Town};

pub fn dist_pt_seg(p: Pt, a: Pt, b: Pt) -> f64 {
    closest(p, a, b).0.dist(p)
}

/// Closest point on segment a-b and its parameter.
pub fn closest(p: Pt, a: Pt, b: Pt) -> (Pt, f64) {
    let ab = b - a;
    let l2 = ab.dot(ab);
    if l2 < 1e-12 {
        return (a, 0.0);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    (a + ab * t, t)
}

/// Intersection parameters (t on a-b, u on c-d) of two segments, if they cross or touch.
pub fn seg_intersect(a: Pt, b: Pt, c: Pt, d: Pt) -> Option<(f64, f64)> {
    let r = b - a;
    let s = d - c;
    let den = r.cross(s);
    if den.abs() < 1e-12 {
        return None;
    }
    let qp = c - a;
    let t = qp.cross(s) / den;
    let u = qp.cross(r) / den;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some((t, u))
    } else {
        None
    }
}

pub fn point_in_ring(p: Pt, ring: &[Pt]) -> bool {
    let mut inside = false;
    let n = ring.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub fn dist_pt_ring(p: Pt, ring: &[Pt]) -> f64 {
    let n = ring.len();
    (0..n)
        .map(|i| dist_pt_seg(p, ring[i], ring[(i + 1) % n]))
        .fold(f64::INFINITY, f64::min)
}

pub fn polyline_len(pts: &[Pt]) -> f64 {
    pts.windows(2).map(|w| w[0].dist(w[1])).sum()
}

const CELL: f64 = 100.0;

fn key(p: Pt) -> (i64, i64) {
    ((p.x / CELL).floor() as i64, (p.y / CELL).floor() as i64)
}

/// Road segments of a town in a uniform grid. Entries are (road index, segment index).
pub struct SegGrid {
    map: HashMap<(i64, i64), Vec<(u32, u32)>>,
}

impl SegGrid {
    pub fn new(t: &Town) -> SegGrid {
        SegGrid::from_lines(t.roads.iter().map(|r| r.points.as_slice()))
    }

    pub fn from_lines<'a>(lines: impl Iterator<Item = &'a [Pt]>) -> SegGrid {
        let mut map: HashMap<(i64, i64), Vec<(u32, u32)>> = HashMap::new();
        for (ri, pts) in lines.enumerate() {
            for (si, w) in pts.windows(2).enumerate() {
                let (x0, y0) = key(pt_min(w[0], w[1]));
                let (x1, y1) = key(pt_max(w[0], w[1]));
                for cx in x0..=x1 {
                    for cy in y0..=y1 {
                        map.entry((cx, cy))
                            .or_default()
                            .push((ri as u32, si as u32));
                    }
                }
            }
        }
        SegGrid { map }
    }

    /// Candidate segments near segment p-q within `pad` metres (may repeat across cells).
    pub fn near(&self, p: Pt, q: Pt, pad: f64) -> Vec<(u32, u32)> {
        let lo = us_gen::pt(p.x.min(q.x) - pad, p.y.min(q.y) - pad);
        let hi = us_gen::pt(p.x.max(q.x) + pad, p.y.max(q.y) + pad);
        let (x0, y0) = key(lo);
        let (x1, y1) = key(hi);
        let mut out = Vec::new();
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                if let Some(v) = self.map.get(&(cx, cy)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

fn pt_min(a: Pt, b: Pt) -> Pt {
    us_gen::pt(a.x.min(b.x), a.y.min(b.y))
}
fn pt_max(a: Pt, b: Pt) -> Pt {
    us_gen::pt(a.x.max(b.x), a.y.max(b.y))
}

/// Point grid of ids.
pub struct PointGrid {
    map: HashMap<(i64, i64), Vec<u32>>,
}

impl PointGrid {
    pub fn new(pts: impl Iterator<Item = Pt>) -> PointGrid {
        let mut map: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
        for (i, p) in pts.enumerate() {
            map.entry(key(p)).or_default().push(i as u32);
        }
        PointGrid { map }
    }

    /// Ids in cells overlapping the disc of radius `r` around `p`, sorted.
    pub fn near(&self, p: Pt, r: f64) -> Vec<u32> {
        let (x0, y0) = key(us_gen::pt(p.x - r, p.y - r));
        let (x1, y1) = key(us_gen::pt(p.x + r, p.y + r));
        let mut out = Vec::new();
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                if let Some(v) = self.map.get(&(cx, cy)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_unstable();
        out
    }
}
