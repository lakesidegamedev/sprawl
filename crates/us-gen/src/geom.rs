//! Plain f64 geometry: points, polylines, rings. Transcendentals go through `libm`.

use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

pub const fn pt(x: f64, y: f64) -> Pt {
    Pt { x, y }
}

impl Add for Pt {
    type Output = Pt;
    fn add(self, o: Pt) -> Pt {
        pt(self.x + o.x, self.y + o.y)
    }
}
impl Sub for Pt {
    type Output = Pt;
    fn sub(self, o: Pt) -> Pt {
        pt(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for Pt {
    type Output = Pt;
    fn mul(self, k: f64) -> Pt {
        pt(self.x * k, self.y * k)
    }
}

impl Pt {
    pub fn dot(self, o: Pt) -> f64 {
        self.x * o.x + self.y * o.y
    }
    pub fn cross(self, o: Pt) -> f64 {
        self.x * o.y - self.y * o.x
    }
    pub fn len(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn dist(self, o: Pt) -> f64 {
        (self - o).len()
    }
    pub fn lerp(self, o: Pt, t: f64) -> Pt {
        pt(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    pub fn unit(self) -> Pt {
        let l = self.len();
        if l < 1e-12 {
            pt(1.0, 0.0)
        } else {
            self * (1.0 / l)
        }
    }
}

pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

pub fn rotate(p: Pt, origin: Pt, ang: f64) -> Pt {
    let (s, c) = (sin(ang), cos(ang));
    let d = p - origin;
    pt(origin.x + c * d.x - s * d.y, origin.y + s * d.x + c * d.y)
}

pub fn rotate_all(pts: &[Pt], origin: Pt, ang: f64) -> Vec<Pt> {
    pts.iter().map(|p| rotate(*p, origin, ang)).collect()
}

// ----- segments ------------------------------------------------------------------------------

pub fn closest_on_seg(p: Pt, a: Pt, b: Pt) -> (Pt, f64) {
    let ab = b - a;
    let l2 = ab.dot(ab);
    if l2 < 1e-18 {
        return (a, 0.0);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    (a + ab * t, t)
}

pub fn dist_pt_seg(p: Pt, a: Pt, b: Pt) -> f64 {
    p.dist(closest_on_seg(p, a, b).0)
}

/// Intersection of segments a-b and c-d as parameters (t on a-b, u on c-d), when they cross or touch.
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

fn segs_cross_strict(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    seg_intersect(a, b, c, d).is_some()
}

pub fn dist_seg_seg(a: Pt, b: Pt, c: Pt, d: Pt) -> f64 {
    if segs_cross_strict(a, b, c, d) {
        return 0.0;
    }
    dist_pt_seg(a, c, d)
        .min(dist_pt_seg(b, c, d))
        .min(dist_pt_seg(c, a, b))
        .min(dist_pt_seg(d, a, b))
}

// ----- polylines -----------------------------------------------------------------------------

pub fn length(line: &[Pt]) -> f64 {
    line.windows(2).map(|w| w[0].dist(w[1])).sum()
}

/// Point at distance `d` along the polyline (clamped), and the unit tangent there.
pub fn interpolate(line: &[Pt], d: f64) -> (Pt, Pt) {
    if line.len() < 2 {
        return (line.first().copied().unwrap_or_default(), pt(1.0, 0.0));
    }
    let mut left = d.max(0.0);
    for w in line.windows(2) {
        let l = w[0].dist(w[1]);
        if left <= l || std::ptr::eq(&w[1], line.last().unwrap()) {
            let t = if l < 1e-12 { 0.0 } else { (left / l).min(1.0) };
            return (w[0].lerp(w[1], t), (w[1] - w[0]).unit());
        }
        left -= l;
    }
    let n = line.len();
    (line[n - 1], (line[n - 1] - line[n - 2]).unit())
}

/// Distance along the polyline of the point closest to `p`, plus that distance and the point.
pub fn project(line: &[Pt], p: Pt) -> (f64, f64, Pt) {
    let mut best = (0.0, f64::INFINITY, line[0]);
    let mut acc = 0.0;
    for w in line.windows(2) {
        let (q, t) = closest_on_seg(p, w[0], w[1]);
        let d = p.dist(q);
        if d < best.1 {
            best = (acc + w[0].dist(w[1]) * t, d, q);
        }
        acc += w[0].dist(w[1]);
    }
    best
}

pub fn dist_pt_line(p: Pt, line: &[Pt]) -> f64 {
    line.windows(2)
        .map(|w| dist_pt_seg(p, w[0], w[1]))
        .fold(f64::INFINITY, f64::min)
}

pub fn dist_line_line(a: &[Pt], b: &[Pt]) -> f64 {
    let mut best = f64::INFINITY;
    for wa in a.windows(2) {
        for wb in b.windows(2) {
            best = best.min(dist_seg_seg(wa[0], wa[1], wb[0], wb[1]));
        }
    }
    best
}

pub fn closed(ring: &[Pt]) -> Vec<Pt> {
    let mut v = ring.to_vec();
    if let (Some(f), Some(l)) = (v.first().copied(), v.last().copied())
        && f.dist(l) > 1e-9
    {
        v.push(f);
    }
    v
}

pub fn open_ring(ring: &[Pt]) -> Vec<Pt> {
    let mut v = ring.to_vec();
    if v.len() > 1 && v[0].dist(v[v.len() - 1]) < 1e-9 {
        v.pop();
    }
    v
}

/// Drop repeated points.
pub fn dedup(line: &[Pt]) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(line.len());
    for p in line {
        if out.last().is_none_or(|l| l.dist(*p) > 1e-9) {
            out.push(*p);
        }
    }
    out
}

pub fn bbox(pts: &[Pt]) -> (Pt, Pt) {
    let mut lo = pt(f64::INFINITY, f64::INFINITY);
    let mut hi = pt(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in pts {
        lo.x = lo.x.min(p.x);
        lo.y = lo.y.min(p.y);
        hi.x = hi.x.max(p.x);
        hi.y = hi.y.max(p.y);
    }
    (lo, hi)
}

// ----- rings ---------------------------------------------------------------------------------

/// Signed area (positive when counter-clockwise). Ring may or may not repeat its first point.
pub fn area_signed(ring: &[Pt]) -> f64 {
    let n = ring.len();
    let mut s = 0.0;
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        s += a.cross(b);
    }
    s * 0.5
}

pub fn area(ring: &[Pt]) -> f64 {
    area_signed(ring).abs()
}

pub fn centroid(ring: &[Pt]) -> Pt {
    let n = ring.len();
    let a = area_signed(ring);
    if a.abs() < 1e-9 {
        let (lo, hi) = bbox(ring);
        return pt((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
    }
    let (mut cx, mut cy) = (0.0, 0.0);
    for i in 0..n {
        let p = ring[i];
        let q = ring[(i + 1) % n];
        let w = p.cross(q);
        cx += (p.x + q.x) * w;
        cy += (p.y + q.y) * w;
    }
    pt(cx / (6.0 * a), cy / (6.0 * a))
}

pub fn point_in_ring(p: Pt, ring: &[Pt]) -> bool {
    let n = ring.len();
    let mut inside = false;
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
    let mut best = f64::INFINITY;
    for i in 0..n {
        best = best.min(dist_pt_seg(p, ring[i], ring[(i + 1) % n]));
    }
    best
}

/// Distance between a ring (as area) and a polyline: zero if the line touches or lies in it.
pub fn dist_ring_line(ring: &[Pt], line: &[Pt]) -> f64 {
    let n = ring.len();
    let mut best = f64::INFINITY;
    for w in line.windows(2) {
        for i in 0..n {
            best = best.min(dist_seg_seg(w[0], w[1], ring[i], ring[(i + 1) % n]));
            if best == 0.0 {
                return 0.0;
            }
        }
    }
    if let Some(p) = line.first()
        && point_in_ring(*p, ring)
    {
        return 0.0;
    }
    best
}

/// Pieces of `line` that lie inside (or outside) the ring.
pub fn clip_line(line: &[Pt], ring: &[Pt], inside: bool) -> Vec<Vec<Pt>> {
    let n = ring.len();
    let (rlo, rhi) = bbox(ring);
    let mut out: Vec<Vec<Pt>> = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg_len = a.dist(b);
        if seg_len < 1e-12 {
            continue;
        }
        let (slo, shi) = (
            pt(a.x.min(b.x), a.y.min(b.y)),
            pt(a.x.max(b.x), a.y.max(b.y)),
        );
        let mut ts: Vec<f64> = vec![0.0, 1.0];
        if !(shi.x < rlo.x || slo.x > rhi.x || shi.y < rlo.y || slo.y > rhi.y) {
            for i in 0..n {
                let (c, d) = (ring[i], ring[(i + 1) % n]);
                if let Some((t, _)) = seg_intersect(a, b, c, d) {
                    ts.push(t);
                }
            }
        }
        ts.sort_by(|p, q| p.total_cmp(q));
        for k in 0..ts.len() - 1 {
            let (t0, t1) = (ts[k], ts[k + 1]);
            if (t1 - t0) * seg_len < 1e-7 {
                continue;
            }
            let (p0, p1) = (a.lerp(b, t0), a.lerp(b, t1));
            let mid = a.lerp(b, (t0 + t1) * 0.5);
            let keep = point_in_ring(mid, ring) == inside;
            if keep {
                if cur.last().is_none_or(|l| l.dist(p0) > 1e-7) {
                    if !cur.is_empty() {
                        out.push(std::mem::take(&mut cur));
                    }
                    cur.push(p0);
                }
                cur.push(p1);
            } else if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter().filter(|l| l.len() >= 2).collect()
}

/// Split a polyline wherever it crosses any of `others`.
pub fn split_line_at(line: &[Pt], others: &[Vec<Pt>]) -> Vec<Vec<Pt>> {
    let mut cuts: Vec<(f64, Pt)> = Vec::new();
    let mut acc = 0.0;
    for w in line.windows(2) {
        let sl = w[0].dist(w[1]);
        for o in others {
            for v in o.windows(2) {
                if let Some((t, _)) = seg_intersect(w[0], w[1], v[0], v[1]) {
                    cuts.push((acc + t * sl, w[0].lerp(w[1], t)));
                }
            }
        }
        acc += sl;
    }
    cuts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total = acc;
    let mut out = Vec::new();
    let mut start = 0.0;
    let mut start_pt = line[0];
    for (d, p) in cuts {
        if d - start > 1e-6 && d < total - 1e-6 {
            out.push(sub_line(line, start, d, start_pt, p));
            start = d;
            start_pt = p;
        }
    }
    out.push(sub_line(line, start, total, start_pt, line[line.len() - 1]));
    out
}

fn sub_line(line: &[Pt], d0: f64, d1: f64, p0: Pt, p1: Pt) -> Vec<Pt> {
    let mut v = vec![p0];
    let mut acc = 0.0;
    for w in line.windows(2) {
        acc += w[0].dist(w[1]);
        if acc > d0 + 1e-9 && acc < d1 - 1e-9 {
            v.push(w[1]);
        }
    }
    v.push(p1);
    v
}

/// Convex hull (monotone chain), counter-clockwise, no repeated first point.
pub fn convex_hull(pts: &[Pt]) -> Vec<Pt> {
    let mut p: Vec<Pt> = pts.to_vec();
    p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let mut h: Vec<Pt> = Vec::new();
    for pass in 0..2 {
        let start = h.len();
        let iter: Box<dyn Iterator<Item = &Pt>> = if pass == 0 {
            Box::new(p.iter())
        } else {
            Box::new(p.iter().rev())
        };
        for q in iter {
            while h.len() >= start + 2
                && (h[h.len() - 1] - h[h.len() - 2]).cross(*q - h[h.len() - 2]) <= 0.0
            {
                h.pop();
            }
            h.push(*q);
        }
        h.pop();
    }
    h
}

/// Minimum-area rotated rectangle: (angle of its long side, long side length, short side length).
pub fn min_rect(ring: &[Pt]) -> (f64, f64, f64) {
    let hull = convex_hull(ring);
    let n = hull.len();
    if n < 3 {
        return (0.0, 0.0, 0.0);
    }
    let mut best = (f64::INFINITY, 0.0, 0.0, 0.0);
    for i in 0..n {
        let d = (hull[(i + 1) % n] - hull[i]).unit();
        let nrm = pt(-d.y, d.x);
        let (mut lo_a, mut hi_a, mut lo_b, mut hi_b) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for q in &hull {
            let a = q.dot(d);
            let b = q.dot(nrm);
            lo_a = lo_a.min(a);
            hi_a = hi_a.max(a);
            lo_b = lo_b.min(b);
            hi_b = hi_b.max(b);
        }
        let (wa, wb) = (hi_a - lo_a, hi_b - lo_b);
        if wa * wb < best.0 {
            let ang = atan2(d.y, d.x);
            best = if wa >= wb {
                (wa * wb, ang, wa, wb)
            } else {
                (wa * wb, ang + std::f64::consts::FRAC_PI_2, wb, wa)
            };
        }
    }
    (best.1, best.2, best.3)
}

pub fn axis_angle(ring: &[Pt]) -> f64 {
    min_rect(ring).0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> Vec<Pt> {
        vec![pt(0.0, 0.0), pt(10.0, 0.0), pt(10.0, 10.0), pt(0.0, 10.0)]
    }

    #[test]
    fn clip_splits_at_edges() {
        let l = vec![pt(-5.0, 5.0), pt(15.0, 5.0)];
        let inside = clip_line(&l, &square(), true);
        assert_eq!(inside.len(), 1);
        assert!((length(&inside[0]) - 10.0).abs() < 1e-9);
        let outside = clip_line(&l, &square(), false);
        assert_eq!(outside.len(), 2);
    }

    #[test]
    fn area_and_centroid() {
        assert!((area(&square()) - 100.0).abs() < 1e-9);
        let c = centroid(&square());
        assert!((c.x - 5.0).abs() < 1e-9 && (c.y - 5.0).abs() < 1e-9);
    }

    #[test]
    fn min_rect_of_rotated_box() {
        let ang = 0.5;
        let r: Vec<Pt> = [pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 30.0), pt(0.0, 30.0)]
            .iter()
            .map(|p| rotate(*p, pt(0.0, 0.0), ang))
            .collect();
        let (a, l, w) = min_rect(&r);
        assert!((l - 100.0).abs() < 1e-6 && (w - 30.0).abs() < 1e-6);
        let d = (a - ang).rem_euclid(std::f64::consts::PI);
        assert!(d < 1e-6 || (std::f64::consts::PI - d) < 1e-6);
    }

    #[test]
    fn split_at_crossings() {
        let l = vec![pt(0.0, 0.0), pt(10.0, 0.0)];
        let o = vec![vec![pt(5.0, -1.0), pt(5.0, 1.0)]];
        assert_eq!(split_line_at(&l, &o).len(), 2);
    }
}
