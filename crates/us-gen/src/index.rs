//! Uniform-grid index over line segments, for nearest-road and coverage queries.

use crate::geom::{Pt, closest_on_seg, pt};

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub dist: f64,
    pub pt: Pt,
    pub owner: u32,
}

pub struct SegIndex {
    cell: f64,
    nx: i64,
    ny: i64,
    cells: Vec<Vec<u32>>,
    segs: Vec<(Pt, Pt, u32)>,
}

const ORIGIN: f64 = -200.0;

impl SegIndex {
    pub fn new(extent: f64, cell: f64) -> SegIndex {
        let n = ((extent + 400.0) / cell).ceil() as i64 + 1;
        SegIndex {
            cell,
            nx: n,
            ny: n,
            cells: vec![Vec::new(); (n * n) as usize],
            segs: Vec::new(),
        }
    }

    fn cidx(&self, v: f64) -> i64 {
        (((v - ORIGIN) / self.cell).floor() as i64).clamp(0, self.nx - 1)
    }

    pub fn add_segment(&mut self, a: Pt, b: Pt, owner: u32) {
        let id = self.segs.len() as u32;
        self.segs.push((a, b, owner));
        let len = a.dist(b);
        let steps = ((len / (self.cell * 0.25)).ceil() as usize).max(1);
        let mut last = (i64::MIN, i64::MIN);
        for s in 0..=steps {
            let p = a.lerp(b, s as f64 / steps as f64);
            let c = (self.cidx(p.x), self.cidx(p.y));
            if c != last {
                self.cells[(c.1 * self.nx + c.0) as usize].push(id);
                last = c;
            }
        }
    }

    pub fn add_line(&mut self, line: &[Pt], owner: u32) {
        for w in line.windows(2) {
            if w[0].dist(w[1]) > 1e-9 {
                self.add_segment(w[0], w[1], owner);
            }
        }
    }

    /// Nearest segment within `r` of `p`, ignoring the owner `skip`.
    pub fn nearest(&self, p: Pt, r: f64, skip: Option<u32>) -> Option<Hit> {
        let (x0, x1) = (self.cidx(p.x - r) - 1, self.cidx(p.x + r) + 1);
        let (y0, y1) = (self.cidx(p.y - r) - 1, self.cidx(p.y + r) + 1);
        let mut best: Option<Hit> = None;
        for cy in y0.max(0)..=y1.min(self.ny - 1) {
            for cx in x0.max(0)..=x1.min(self.nx - 1) {
                for &id in &self.cells[(cy * self.nx + cx) as usize] {
                    let (a, b, o) = self.segs[id as usize];
                    if Some(o) == skip {
                        continue;
                    }
                    let (q, _) = closest_on_seg(p, a, b);
                    let d = p.dist(q);
                    if d <= r && best.is_none_or(|h| d < h.dist || (d == h.dist && o < h.owner)) {
                        best = Some(Hit {
                            dist: d,
                            pt: q,
                            owner: o,
                        });
                    }
                }
            }
        }
        best
    }

    /// True when some segment within `r` of `p` runs within `max_sin` (sine of the angle) of `dir`.
    pub fn any_parallel(&self, p: Pt, r: f64, dir: Pt, max_sin: f64) -> bool {
        let (x0, x1) = (self.cidx(p.x - r) - 1, self.cidx(p.x + r) + 1);
        let (y0, y1) = (self.cidx(p.y - r) - 1, self.cidx(p.y + r) + 1);
        for cy in y0.max(0)..=y1.min(self.ny - 1) {
            for cx in x0.max(0)..=x1.min(self.nx - 1) {
                for &id in &self.cells[(cy * self.nx + cx) as usize] {
                    let (a, b, _) = self.segs[id as usize];
                    if p.dist(closest_on_seg(p, a, b).0) <= r
                        && (b - a).unit().cross(dir).abs() <= max_sin
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Call `f(distance, owner)` for every segment within `r` of `p`.
    pub fn for_each_within(&self, p: Pt, r: f64, mut f: impl FnMut(f64, u32)) {
        let (x0, x1) = (self.cidx(p.x - r) - 1, self.cidx(p.x + r) + 1);
        let (y0, y1) = (self.cidx(p.y - r) - 1, self.cidx(p.y + r) + 1);
        for cy in y0.max(0)..=y1.min(self.ny - 1) {
            for cx in x0.max(0)..=x1.min(self.nx - 1) {
                for &id in &self.cells[(cy * self.nx + cx) as usize] {
                    let (a, b, o) = self.segs[id as usize];
                    let d = p.dist(closest_on_seg(p, a, b).0);
                    if d <= r {
                        f(d, o);
                    }
                }
            }
        }
    }
}

/// Pieces of `line` that are farther than `tol` from everything in the index.
/// Sampled every `step` metres; piece ends are placed on the sampling grid.
#[allow(clippy::needless_range_loop)]
pub fn uncovered_pieces(line: &[Pt], idx: &SegIndex, tol: f64, step: f64) -> Vec<Vec<Pt>> {
    let total = crate::geom::length(line);
    if total < 1e-6 {
        return Vec::new();
    }
    let n = ((total / step).ceil() as usize).max(1);
    let mut out: Vec<Vec<Pt>> = Vec::new();
    let mut run_start: Option<usize> = None;
    let sample = |k: usize| crate::geom::interpolate(line, total * k as f64 / n as f64).0;
    let mut flags = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let (p, dir) = crate::geom::interpolate(line, total * k as f64 / n as f64);
        // only a road running the same way covers: crossings and corners keep their ends
        flags.push(!idx.any_parallel(p, tol, dir, 0.34));
    }
    // a brief brush against another road (a corner, a tangent entrance) is not an overlap
    let min_run = (6.0 / (total / n as f64)).ceil() as usize;
    let mut k = 0;
    while k <= n {
        if !flags[k] {
            let s0 = k;
            while k <= n && !flags[k] {
                k += 1;
            }
            if k - s0 < min_run {
                for f in &mut flags[s0..k] {
                    *f = true;
                }
            }
        } else {
            k += 1;
        }
    }
    if flags.iter().all(|f| *f) {
        return vec![line.to_vec()];
    }
    for k in 0..=n + 1 {
        let free = k <= n && flags[k];
        match (free, run_start) {
            (true, None) => run_start = Some(k),
            (false, Some(s)) => {
                let e = k - 1;
                if e > s {
                    // widen the run by one sample on each side so pieces reach the covering road
                    let s2 = s.saturating_sub(1);
                    let e2 = (e + 1).min(n);
                    let mut v = Vec::new();
                    let (d0, d1) = (total * s2 as f64 / n as f64, total * e2 as f64 / n as f64);
                    v.push(sample(s2));
                    let mut acc = 0.0;
                    for w in line.windows(2) {
                        acc += w[0].dist(w[1]);
                        if acc > d0 + 1e-9 && acc < d1 - 1e-9 {
                            v.push(w[1]);
                        }
                    }
                    v.push(sample(e2));
                    out.push(v);
                }
                run_start = None;
            }
            _ => {}
        }
    }
    let _ = pt;
    out
}
