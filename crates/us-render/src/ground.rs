//! Flat ground geometry: polygons, ribbons along polylines, discs. All of it is horizontal,
//! drawn in painter's order at fixed heights so layers stack without z fighting.

use crate::land::{Bed, Land};
use crate::mesh::{ID_GROUND, Mesh, Vertex};
use crate::obj::Rgb;
use us_gen::{Pt, pt};

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

/// Outline ids of bridge parts start here (above every chunk's ids, below the exempt id).
pub const BRIDGE_ID_BASE: u32 = 0x00E0_0000;
pub const CONCRETE: Rgb = [176, 172, 164];
pub const CONCRETE_DARK: Rgb = [138, 136, 134];

#[derive(Default)]
pub struct Ground {
    pub mesh: Mesh,
    /// When set, every layer is draped on the land: vertices rise to its height and flat shapes
    /// are cut along its grid so they lie exactly on the surface.
    land: Option<Land>,
    bridge_ids: u32,
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

/// Which side of a line a point is on: x / step, y / step or (x - y) / step.
fn lattice_f(family: usize, p: Pt, step: f64) -> f64 {
    match family {
        0 => p.x / step,
        1 => p.y / step,
        _ => (p.x - p.y) / step,
    }
}

/// Cut a convex polygon along the lines `f = k` for every whole `k` it spans.
fn cut_family(piece: Vec<Pt>, family: usize, step: f64, out: &mut Vec<Vec<Pt>>) {
    let f: Vec<f64> = piece.iter().map(|p| lattice_f(family, *p, step)).collect();
    let (lo, hi) = f
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
    let mut k = (lo + 1e-9).floor() + 1.0;
    if k >= hi - 1e-9 {
        out.push(piece);
        return;
    }
    let mut rest = piece;
    while k < hi - 1e-9 {
        let (below, above) = split_at(&rest, family, step, k);
        if below.len() >= 3 {
            out.push(below);
        }
        rest = above;
        if rest.len() < 3 {
            return;
        }
        k += 1.0;
    }
    out.push(rest);
}

/// Split a convex polygon by the line `f = k` into the part below and the part above.
fn split_at(poly: &[Pt], family: usize, step: f64, k: f64) -> (Vec<Pt>, Vec<Pt>) {
    let mut below = Vec::with_capacity(poly.len() + 2);
    let mut above = Vec::with_capacity(poly.len() + 2);
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let (fa, fb) = (lattice_f(family, a, step), lattice_f(family, b, step));
        if fa <= k {
            below.push(a);
        }
        if fa >= k {
            above.push(a);
        }
        if (fa < k && fb > k) || (fa > k && fb < k) {
            // computed from the same end first, so both sides of a shared edge agree to the bit
            let (p, q, fp, fq) = if (a.x, a.y) <= (b.x, b.y) {
                (a, b, fa, fb)
            } else {
                (b, a, fb, fa)
            };
            let t = (k - fp) / (fq - fp);
            let m = pt(p.x + (q.x - p.x) * t, p.y + (q.y - p.y) * t);
            below.push(m);
            above.push(m);
        }
    }
    (below, above)
}

impl Ground {
    /// A builder that drapes everything on `land`. Level land draws flat, as before.
    pub fn on(land: &Land) -> Ground {
        Ground {
            mesh: Mesh::default(),
            land: (!land.is_flat()).then(|| land.clone()),
            bridge_ids: 0,
        }
    }

    /// Ground height under a point (0 on level land).
    pub fn height_at(&self, x: f32, y: f32) -> f32 {
        self.land.as_ref().map_or(0.0, |l| l.height(x, y))
    }

    /// Place a model on the ground at (x, y), turned by `theta`, standing on the land. Outline
    /// ids come from the bridge range, so they never clash with a building's.
    pub fn place_prop(&mut self, obj: &crate::obj::Obj, x: f32, y: f32, theta: f32) {
        let mut id = self.next_bridge_id();
        let z = self.height_at(x, y);
        match self.land.clone() {
            Some(land) => obj.emit_on(
                &mut self.mesh,
                &mut id,
                x,
                y,
                theta,
                1.0,
                Some(crate::obj::Sit { land: &land, z }),
            ),
            None => obj.emit(&mut self.mesh, &mut id, x, y, theta, 1.0),
        }
        self.bridge_ids += obj.polys.len() as u32;
    }

    fn vert(&mut self, x: f32, y: f32, z: f32, color: Rgb) -> u32 {
        let (z, normal) = match &self.land {
            Some(l) => (z + l.height(x, y), l.normal(x, y)),
            None => (z, [0.0, 0.0, 1.0]),
        };
        self.raw(x, y, z, normal, color, ID_GROUND)
    }

    fn raw(&mut self, x: f32, y: f32, z: f32, normal: [f32; 3], color: Rgb, id: u32) -> u32 {
        self.mesh.vertices.push(Vertex {
            pos: [x, y, z],
            normal,
            color: [color[0], color[1], color[2], 255],
            id,
        });
        (self.mesh.vertices.len() - 1) as u32
    }

    /// The whole land as a grid of faces in one colour, reaching `apron` metres past the map.
    pub fn terrain(&mut self, land: &Land, apron: f32, color: Rgb) {
        if self.land.is_none() {
            let s = land.size();
            self.rect(-apron, -apron, s + apron, s + apron, layer::BASE, color);
            return;
        }
        let (n, step, size) = (land.n(), land.step(), land.size());
        let mut xs: Vec<f32> = vec![-apron];
        xs.extend((0..n).map(|i| i as f32 * step));
        xs.push(size + apron);
        let m = xs.len();
        let base = self.mesh.vertices.len() as u32;
        for &y in &xs {
            for &x in &xs {
                // past the map edge the land is held level, so it is shaded level too
                if x < 0.0 || y < 0.0 || x > size || y > size {
                    let z = land.height(x, y) + layer::BASE;
                    self.raw(x, y, z, [0.0, 0.0, 1.0], color, ID_GROUND);
                } else {
                    self.vert(x, y, layer::BASE, color);
                }
            }
        }
        for j in 0..m - 1 {
            for i in 0..m - 1 {
                let a = base + (j * m + i) as u32;
                let (b, c, d) = (a + 1, a + m as u32 + 1, a + m as u32);
                // the split matches Land::height: along the (x, y) diagonal
                self.mesh.plain.extend_from_slice(&[a, b, c, a, c, d]);
            }
        }
    }

    /// A flat polygon at a fixed height: water surfaces.
    pub fn polygon_level(&mut self, ring: &[Pt], z: f32, color: Rgb) {
        let (pts, tris) = triangulate(ring);
        if tris.is_empty() {
            return;
        }
        let base = self.mesh.vertices.len() as u32;
        for p in &pts {
            self.raw(p.x as f32, p.y as f32, z, [0.0, 0.0, 1.0], color, ID_GROUND);
        }
        for t in tris {
            self.mesh.plain.extend_from_slice(&[
                base + t[0] as u32,
                base + t[1] as u32,
                base + t[2] as u32,
            ]);
        }
    }

    /// A filled polygon.
    pub fn polygon(&mut self, ring: &[Pt], z: f32, color: Rgb) {
        let (pts, tris) = triangulate(ring);
        if tris.is_empty() {
            return;
        }
        if let Some(step) = self.land.as_ref().map(|l| l.step() as f64) {
            // cut along the height grid so the faces lie on the surface exactly
            for t in &tris {
                let mut pieces = vec![vec![pts[t[0]], pts[t[1]], pts[t[2]]]];
                for family in 0..3 {
                    let mut next = Vec::with_capacity(pieces.len());
                    for piece in pieces {
                        cut_family(piece, family, step, &mut next);
                    }
                    pieces = next;
                }
                for piece in pieces {
                    let ids: Vec<u32> = piece
                        .iter()
                        .map(|p| self.vert(p.x as f32, p.y as f32, z, color))
                        .collect();
                    for k in 1..ids.len().saturating_sub(1) {
                        self.mesh
                            .plain
                            .extend_from_slice(&[ids[0], ids[k], ids[k + 1]]);
                    }
                }
            }
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
        if self.land.is_some() {
            let ring = [
                pt(x0 as f64, y0 as f64),
                pt(x1 as f64, y0 as f64),
                pt(x1 as f64, y1 as f64),
                pt(x0 as f64, y1 as f64),
            ];
            self.polygon(&ring, z, color);
            return;
        }
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
        // on hills the line is cut into short steps that follow the ground (or a bridge deck)
        let bed: Option<Bed> = self.land.as_ref().map(|l| l.bed(&p));
        if let Some(b) = &bed {
            p = b.pts.clone();
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
        let place = |g: &mut Ground, i: usize, v: &[f32; 2]| -> u32 {
            match (&bed, &g.land) {
                (Some(b), Some(land)) => {
                    let land = land.clone();
                    let top = b.z(&land, i, v[0], v[1]) + z;
                    let up = b.weight[i] > 0.5;
                    let nrm = if up {
                        [0.0, 0.0, 1.0]
                    } else {
                        land.normal(v[0], v[1])
                    };
                    g.raw(v[0], v[1], top, nrm, color, ID_GROUND)
                }
                _ => g.vert(v[0], v[1], z, color),
            }
        };
        let l: Vec<u32> = left
            .iter()
            .enumerate()
            .map(|(i, v)| place(self, i, v))
            .collect();
        let r: Vec<u32> = right
            .iter()
            .enumerate()
            .map(|(i, v)| place(self, i, v))
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

    /// One face of a bridge or embankment, wound to face `outward`.
    fn face(&mut self, pts: [[f32; 3]; 4], outward: [f32; 3], color: Rgb, id: u32) {
        let n = crate::obj::newell(&pts);
        let mut v = pts.to_vec();
        if n[0] * outward[0] + n[1] * outward[1] + n[2] * outward[2] < 0.0 {
            v.reverse();
        }
        self.mesh.push_poly(&v, color, false, true, id);
    }

    fn next_bridge_id(&mut self) -> u32 {
        self.bridge_ids += 1;
        BRIDGE_ID_BASE + self.bridge_ids % 0x000F_0000
    }

    /// The structure under and beside a road that stands above the ground: embankment walls on
    /// the approaches, and on a bridge a slab edge, parapets and piers in the water. Does nothing
    /// on flat land or for a road that stays on the ground.
    pub fn bridge(&mut self, line: &[Pt], half: f32) {
        let Some(land) = self.land.clone() else {
            return;
        };
        let bed = land.bed(line);
        let n = bed.pts.len();
        if n < 2 || bed.weight.iter().all(|&w| w <= 0.0) {
            return;
        }
        let edge = half + 0.5;
        let mut since_pier = 1000.0f32;
        for i in 0..n - 1 {
            if bed.weight[i] <= 0.0 && bed.weight[i + 1] <= 0.0 {
                continue;
            }
            let (a, b) = (bed.pts[i], bed.pts[i + 1]);
            let l = a.dist(b);
            if l < 1e-6 {
                continue;
            }
            let (dx, dy) = ((b.x - a.x) / l, (b.y - a.y) / l);
            let (nx, ny) = (-dy as f32, dx as f32);
            let deck = bed.weight[i] > 0.999 && bed.weight[i + 1] > 0.999;
            let id = self.next_bridge_id();
            for side in [-1.0f32, 1.0] {
                let corner = |k: usize, off: f32| -> (f32, f32) {
                    let p = bed.pts[k];
                    (p.x as f32 + nx * off * side, p.y as f32 + ny * off * side)
                };
                let (ax, ay) = corner(i, edge);
                let (bx, by) = corner(i + 1, edge);
                let (za, zb) = (bed.z(&land, i, ax, ay), bed.z(&land, i + 1, bx, by));
                let (ga, gb) = (land.height(ax, ay), land.height(bx, by));
                let bot = |z: f32, g: f32| if z - g <= 2.6 { g - 0.2 } else { z - 1.6 };
                let (ba, bb) = (bot(za, ga), bot(zb, gb));
                if za - ba > 0.05 || zb - bb > 0.05 {
                    let out = [nx * side, ny * side, 0.0];
                    self.face(
                        [[ax, ay, ba], [bx, by, bb], [bx, by, zb], [ax, ay, za]],
                        out,
                        CONCRETE,
                        id,
                    );
                }
                if deck {
                    // parapet: a low wall along the edge
                    let (px, py) = corner(i, edge - 0.25);
                    let (qx, qy) = corner(i + 1, edge - 0.25);
                    let (ix, iy) = corner(i, edge - 0.5);
                    let (jx, jy) = corner(i + 1, edge - 0.5);
                    let h = 0.95;
                    let out = [nx * side, ny * side, 0.0];
                    self.face(
                        [
                            [px, py, za],
                            [qx, qy, zb],
                            [qx, qy, zb + h],
                            [px, py, za + h],
                        ],
                        out,
                        CONCRETE,
                        id,
                    );
                    self.face(
                        [
                            [ix, iy, za],
                            [jx, jy, zb],
                            [jx, jy, zb + h],
                            [ix, iy, za + h],
                        ],
                        [-out[0], -out[1], 0.0],
                        CONCRETE,
                        id,
                    );
                    self.face(
                        [
                            [px, py, za + h],
                            [qx, qy, zb + h],
                            [jx, jy, zb + h],
                            [ix, iy, za + h],
                        ],
                        [0.0, 0.0, 1.0],
                        CONCRETE_DARK,
                        id,
                    );
                }
            }
            if deck {
                // the slab's underside seen from the banks
                since_pier += l as f32;
                let mid = a.lerp(b, 0.5);
                if since_pier >= 26.0 && land.water_at(mid).is_some() {
                    since_pier = 0.0;
                    let z = bed.deck[i] - 1.6;
                    let (mx, my) = (mid.x as f32, mid.y as f32);
                    let g = land.height(mx, my) - 0.8;
                    let (hl, hw) = (0.9f32, half * 0.8);
                    let (ux, uy) = (dx as f32, dy as f32);
                    let c = [
                        (mx - ux * hl - nx * hw, my - uy * hl - ny * hw),
                        (mx + ux * hl - nx * hw, my + uy * hl - ny * hw),
                        (mx + ux * hl + nx * hw, my + uy * hl + ny * hw),
                        (mx - ux * hl + nx * hw, my - uy * hl + ny * hw),
                    ];
                    let id = self.next_bridge_id();
                    for k in 0..4 {
                        let (p, q) = (c[k], c[(k + 1) % 4]);
                        let out = [(p.0 + q.0) * 0.5 - mx, (p.1 + q.1) * 0.5 - my, 0.0];
                        self.face(
                            [[p.0, p.1, g], [q.0, q.1, g], [q.0, q.1, z], [p.0, p.1, z]],
                            out,
                            CONCRETE_DARK,
                            id,
                        );
                    }
                }
            } else {
                since_pier = 1000.0;
            }
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
