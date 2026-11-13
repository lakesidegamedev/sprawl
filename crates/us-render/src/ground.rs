//! Flat ground geometry: polygons, ribbons along polylines, discs. All of it is horizontal,
//! drawn in painter's order at fixed heights so layers stack without z fighting.

use crate::mesh::{ID_GROUND, Mesh, Vertex};
use crate::obj::Rgb;
use us_gen::Pt;

/// Heights of the ground layers in metres, bottom first.
pub mod layer {
    pub const BASE: f32 = 0.0;
    pub const ZONE: f32 = 0.06;
    pub const SAND: f32 = 0.12;
    pub const WATER: f32 = 0.18;
    pub const EDGE: f32 = 0.24;
    pub const ROAD: f32 = 0.30;
    pub const MARK: f32 = 0.36;
    pub const RAIL: f32 = 0.42;
}

#[derive(Default)]
pub struct Ground {
    pub mesh: Mesh,
}

fn cross(o: Pt, a: Pt, b: Pt) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

pub fn signed_area(ring: &[Pt]) -> f64 {
    let mut a = 0.0;
    for i in 0..ring.len() {
        let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
        a += p.x * q.y - q.x * p.y;
    }
    a / 2.0
}

fn in_tri(p: Pt, a: Pt, b: Pt, c: Pt) -> bool {
    let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
    d1 >= -1e-9 && d2 >= -1e-9 && d3 >= -1e-9
}

/// Ear clipping. Returns triangles as index triples into the cleaned ring, counter-clockwise.
/// The ring may be open or closed, either orientation.
pub fn triangulate(ring: &[Pt]) -> (Vec<Pt>, Vec<[usize; 3]>) {
    let mut pts: Vec<Pt> = Vec::with_capacity(ring.len());
    for p in ring {
        if pts.last().is_none_or(|l| l.dist(*p) > 1e-6) {
            pts.push(*p);
        }
    }
    while pts.len() > 1 && pts[0].dist(*pts.last().unwrap()) <= 1e-6 {
        pts.pop();
    }
    if pts.len() < 3 {
        return (pts, vec![]);
    }
    if signed_area(&pts) < 0.0 {
        pts.reverse();
    }
    let mut idx: Vec<usize> = (0..pts.len()).collect();
    let mut tris = Vec::with_capacity(pts.len() - 2);
    let mut guard = 0usize;
    while idx.len() > 3 && guard < pts.len() * pts.len() + 16 {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for k in 0..n {
            let (ia, ib, ic) = (idx[(k + n - 1) % n], idx[k], idx[(k + 1) % n]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross(a, b, c) <= 1e-9 {
                continue;
            }
            let blocked = idx.iter().any(|&j| {
                j != ia && j != ib && j != ic && {
                    let p = pts[j];
                    p.dist(a) > 1e-9 && p.dist(b) > 1e-9 && p.dist(c) > 1e-9 && in_tri(p, a, b, c)
                }
            });
            if !blocked {
                tris.push([ia, ib, ic]);
                idx.remove(k);
                clipped = true;
                break;
            }
        }
        if !clipped {
            // a degenerate sliver is left: drop a collinear vertex or give up on the rest
            let n = idx.len();
            let k = (0..n).find(|&k| {
                cross(
                    pts[idx[(k + n - 1) % n]],
                    pts[idx[k]],
                    pts[idx[(k + 1) % n]],
                )
                .abs()
                    <= 1e-9
            });
            match k {
                Some(k) => {
                    idx.remove(k);
                }
                None => break,
            }
        }
    }
    if idx.len() == 3 {
        tris.push([idx[0], idx[1], idx[2]]);
    }
    (pts, tris)
}

impl Ground {
    fn vert(&mut self, x: f32, y: f32, z: f32, color: Rgb) -> u32 {
        self.mesh.vertices.push(Vertex {
            pos: [x, y, z],
            normal: [0.0, 0.0, 1.0],
            color: [color[0], color[1], color[2], 255],
            id: ID_GROUND,
        });
        (self.mesh.vertices.len() - 1) as u32
    }

    /// A filled polygon.
    pub fn polygon(&mut self, ring: &[Pt], z: f32, color: Rgb) {
        let (pts, tris) = triangulate(ring);
        if tris.is_empty() {
            return;
        }
        let base = self.mesh.vertices.len() as u32;
        for p in &pts {
            self.vert(p.x as f32, p.y as f32, z, color);
        }
        for t in tris {
            self.mesh.plain.extend_from_slice(&[
                base + t[0] as u32,
                base + t[1] as u32,
                base + t[2] as u32,
            ]);
        }
    }

    pub fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, z: f32, color: Rgb) {
        let a = self.vert(x0, y0, z, color);
        let b = self.vert(x1, y0, z, color);
        let c = self.vert(x1, y1, z, color);
        let d = self.vert(x0, y1, z, color);
        self.mesh.plain.extend_from_slice(&[a, b, c, a, c, d]);
    }

    pub fn disc(&mut self, c: Pt, r: f32, z: f32, color: Rgb) {
        let n = 20;
        let centre = self.vert(c.x as f32, c.y as f32, z, color);
        let ring: Vec<u32> = (0..n)
            .map(|i| {
                let a = std::f32::consts::TAU * i as f32 / n as f32;
                self.vert(
                    c.x as f32 + r * libm::cosf(a),
                    c.y as f32 + r * libm::sinf(a),
                    z,
                    color,
                )
            })
            .collect();
        for i in 0..n {
            self.mesh
                .plain
                .extend_from_slice(&[centre, ring[i], ring[(i + 1) % n]]);
        }
    }

    /// A strip of constant half width along a polyline, mitred at the joints. `offset` shifts it
    /// sideways (positive is to the left of travel).
    pub fn ribbon(&mut self, line: &[Pt], half: f32, offset: f32, z: f32, color: Rgb) {
        let mut p: Vec<Pt> = Vec::with_capacity(line.len());
        for q in line {
            if p.last().is_none_or(|l| l.dist(*q) > 1e-6) {
                p.push(*q);
            }
        }
        if p.len() < 2 {
            return;
        }
        let closed = p.len() > 2 && p[0].dist(*p.last().unwrap()) < 1e-6;
        if closed {
            p.pop();
        }
        let n = p.len();
        let seg_dir = |i: usize, j: usize| {
            let (a, b) = (p[i], p[j]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let l = dx.hypot(dy).max(1e-12);
            (dx / l, dy / l)
        };
        let mut left: Vec<[f32; 2]> = Vec::with_capacity(n);
        let mut right: Vec<[f32; 2]> = Vec::with_capacity(n);
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            let prev = if i > 0 {
                Some(seg_dir(i - 1, i))
            } else if closed {
                Some(seg_dir(n - 1, 0))
            } else {
                None
            };
            let next = if i + 1 < n {
                Some(seg_dir(i, i + 1))
            } else if closed {
                Some(seg_dir(n - 1, 0))
            } else {
                None
            };
            let d = match (prev, next) {
                (Some(a), Some(b)) => (a.0 + b.0, a.1 + b.1),
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => (1.0, 0.0),
            };
            let l = d.0.hypot(d.1);
            let nrm = if l < 1e-6 {
                // a hairpin: fall back to the incoming edge's normal
                let a = prev.or(next).unwrap_or((1.0, 0.0));
                (-a.1, a.0)
            } else {
                (-d.1 / l, d.0 / l)
            };
            // widen at a bend so the edges keep their width, within a limit
            let base = prev.or(next).unwrap_or((1.0, 0.0));
            let cosw = ((-base.1) * nrm.0 + base.0 * nrm.1).clamp(0.5, 1.0);
            let w = half as f64 / cosw;
            let (cx, cy) = (
                p[i].x + nrm.0 * offset as f64,
                p[i].y + nrm.1 * offset as f64,
            );
            left.push([(cx + nrm.0 * w) as f32, (cy + nrm.1 * w) as f32]);
            right.push([(cx - nrm.0 * w) as f32, (cy - nrm.1 * w) as f32]);
        }
        let l: Vec<u32> = left
            .iter()
            .map(|v| self.vert(v[0], v[1], z, color))
            .collect();
        let r: Vec<u32> = right
            .iter()
            .map(|v| self.vert(v[0], v[1], z, color))
            .collect();
        let segs = if closed { n } else { n - 1 };
        for i in 0..segs {
            let j = (i + 1) % n;
            // right[i], right[j], left[j], left[i] is counter-clockwise from above
            self.mesh
                .plain
                .extend_from_slice(&[r[i], r[j], l[j], r[i], l[j], l[i]]);
        }
    }

    /// Offset a closed ring outward by `d` metres along vertex normals.
    pub fn grow(ring: &[Pt], d: f64) -> Vec<Pt> {
        let mut pts: Vec<Pt> = Vec::new();
        for p in ring {
            if pts.last().is_none_or(|l| l.dist(*p) > 1e-6) {
                pts.push(*p);
            }
        }
        while pts.len() > 1 && pts[0].dist(*pts.last().unwrap()) <= 1e-6 {
            pts.pop();
        }
        let sign = if signed_area(&pts) >= 0.0 { 1.0 } else { -1.0 };
        let n = pts.len();
        (0..n)
            .map(|i| {
                let (a, b, c) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
                let e1 = ((b.x - a.x), (b.y - a.y));
                let e2 = ((c.x - b.x), (c.y - b.y));
                let (l1, l2) = (e1.0.hypot(e1.1).max(1e-9), e2.0.hypot(e2.1).max(1e-9));
                // outward normals of a counter-clockwise ring point to the right of travel
                let n1 = (sign * e1.1 / l1, -sign * e1.0 / l1);
                let n2 = (sign * e2.1 / l2, -sign * e2.0 / l2);
                let m = (n1.0 + n2.0, n1.1 + n2.1);
                let ml = m.0.hypot(m.1).max(1e-9);
                let k = (1.0 / (m.0 / ml * n1.0 + m.1 / ml * n1.1)).clamp(1.0, 3.0);
                Pt {
                    x: b.x + m.0 / ml * d * k,
                    y: b.y + m.1 / ml * d * k,
                }
            })
            .collect()
    }
}
