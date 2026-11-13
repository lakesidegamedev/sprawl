//! Faces from a set of lines: noding, then a planar half-edge walk.

use crate::geom::{Pt, atan2, pt};
use std::collections::{BTreeSet, HashMap};

const SNAP: f64 = 1e-4;

struct Nodes {
    pts: Vec<Pt>,
    map: HashMap<(i64, i64), Vec<usize>>,
}

impl Nodes {
    fn key(p: Pt) -> (i64, i64) {
        ((p.x / 1e-3).floor() as i64, (p.y / 1e-3).floor() as i64)
    }

    fn get(&mut self, p: Pt) -> usize {
        let (kx, ky) = Self::key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = self.map.get(&(kx + dx, ky + dy)) {
                    for &i in v {
                        if self.pts[i].dist(p) <= SNAP {
                            return i;
                        }
                    }
                }
            }
        }
        let id = self.pts.len();
        self.pts.push(p);
        self.map.entry((kx, ky)).or_default().push(id);
        id
    }
}

/// Bounded faces of the planar graph made by `lines` (dangling edges ignored).
/// Rings are counter-clockwise, without a repeated first point.
pub fn polygonize(lines: &[Vec<Pt>]) -> Vec<Vec<Pt>> {
    let mut segs: Vec<(Pt, Pt)> = Vec::new();
    for l in lines {
        for w in l.windows(2) {
            if w[0].dist(w[1]) > SNAP {
                segs.push((w[0], w[1]));
            }
        }
    }
    let n = segs.len();
    if n == 0 {
        return Vec::new();
    }
    // sweep and prune on x
    let mut order: Vec<usize> = (0..n).collect();
    let minx = |i: usize| segs[i].0.x.min(segs[i].1.x);
    let maxx = |i: usize| segs[i].0.x.max(segs[i].1.x);
    let miny = |i: usize| segs[i].0.y.min(segs[i].1.y);
    let maxy = |i: usize| segs[i].0.y.max(segs[i].1.y);
    order.sort_by(|&a, &b| minx(a).total_cmp(&minx(b)).then(a.cmp(&b)));
    let mut splits: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; n];
    for oi in 0..n {
        let i = order[oi];
        let hi = maxx(i) + SNAP;
        for &j in &order[oi + 1..] {
            if minx(j) > hi {
                break;
            }
            if miny(j) > maxy(i) + SNAP || maxy(j) < miny(i) - SNAP {
                continue;
            }
            node_pair(&segs, i, j, &mut splits);
        }
    }
    // build nodes and edges
    let mut nodes = Nodes {
        pts: Vec::new(),
        map: HashMap::new(),
    };
    let mut edges: BTreeSet<(usize, usize)> = BTreeSet::new();
    for i in 0..n {
        let (a, b) = segs[i];
        let mut ts = std::mem::take(&mut splits[i]);
        ts.sort_by(|p, q| p.total_cmp(q));
        let mut prev = nodes.get(a);
        for &t in ts.iter().skip(1) {
            let p = if t >= 1.0 { b } else { a.lerp(b, t) };
            let cur = nodes.get(p);
            if cur != prev {
                edges.insert((prev.min(cur), prev.max(cur)));
            }
            prev = cur;
        }
    }
    let nn = nodes.pts.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nn];
    for &(a, b) in &edges {
        adj[a].push(b);
        adj[b].push(a);
    }
    // prune dangling
    let mut stack: Vec<usize> = (0..nn).filter(|&i| adj[i].len() == 1).collect();
    while let Some(v) = stack.pop() {
        if adj[v].len() != 1 {
            continue;
        }
        let u = adj[v][0];
        adj[v].clear();
        adj[u].retain(|&x| x != v);
        if adj[u].len() == 1 {
            stack.push(u);
        }
    }
    // order neighbours by angle
    for (v, list) in adj.iter_mut().enumerate() {
        let p = nodes.pts[v];
        let mut keyed: Vec<(f64, usize)> = list
            .iter()
            .map(|&u| (atan2(nodes.pts[u].y - p.y, nodes.pts[u].x - p.x), u))
            .collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        *list = keyed.into_iter().map(|k| k.1).collect();
    }
    let mut visited: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut faces = Vec::new();
    for u in 0..nn {
        for k in 0..adj[u].len() {
            let v0 = adj[u][k];
            if visited.contains(&(u, v0)) {
                continue;
            }
            let mut ring = Vec::new();
            let (mut a, mut b) = (u, v0);
            let mut guard = 0usize;
            loop {
                visited.insert((a, b));
                ring.push(nodes.pts[a]);
                let list = &adj[b];
                let pos = list.iter().position(|&x| x == a).unwrap_or(0);
                let next = list[(pos + list.len() - 1) % list.len()];
                a = b;
                b = next;
                guard += 1;
                if (a == u && b == v0) || guard > nn * 4 + 16 {
                    break;
                }
            }
            if guard <= nn * 4 && crate::geom::area_signed(&ring) > 1e-6 {
                faces.push(ring);
            }
        }
    }
    let _ = pt;
    faces
}

fn node_pair(segs: &[(Pt, Pt)], i: usize, j: usize, splits: &mut [Vec<f64>]) {
    let (a, b) = segs[i];
    let (c, d) = segs[j];
    let li = a.dist(b);
    let lj = c.dist(d);
    // endpoints lying on the other segment
    let mut hit = false;
    for (p, on_j) in [(a, true), (b, true), (c, false), (d, false)] {
        let (seg_a, seg_b, len) = if on_j { (c, d, lj) } else { (a, b, li) };
        let (q, t) = crate::geom::closest_on_seg(p, seg_a, seg_b);
        if p.dist(q) <= SNAP {
            hit = true;
            let _ = len;
            if on_j {
                splits[j].push(t);
            } else {
                splits[i].push(t);
            }
        }
    }
    let _ = hit;
    // proper crossings
    let r = b - a;
    let s = d - c;
    let den = r.cross(s);
    if den.abs() > 1e-12 {
        let qp = c - a;
        let t = qp.cross(s) / den;
        let u = qp.cross(r) / den;
        if t > 0.0 && t < 1.0 && u > 0.0 && u < 1.0 {
            splits[i].push(t);
            splits[j].push(u);
        }
    }
}

/// Split a polygon ring along cut lines. Pieces come back counter-clockwise.
pub fn split_polygon(ring: &[Pt], cuts: &[Vec<Pt>]) -> Vec<Vec<Pt>> {
    let mut lines = vec![crate::geom::closed(ring)];
    for c in cuts {
        lines.extend(crate::geom::clip_line(c, ring, true));
    }
    polygonize(&lines)
        .into_iter()
        .filter(|f| crate::geom::area(f) > 1.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_makes_four_faces() {
        let lines = vec![
            vec![
                pt(0.0, 0.0),
                pt(20.0, 0.0),
                pt(20.0, 20.0),
                pt(0.0, 20.0),
                pt(0.0, 0.0),
            ],
            vec![pt(10.0, -5.0), pt(10.0, 25.0)],
            vec![pt(-5.0, 10.0), pt(25.0, 10.0)],
        ];
        let faces = polygonize(&lines);
        assert_eq!(faces.len(), 4);
        for f in faces {
            assert!((crate::geom::area(&f) - 100.0).abs() < 1e-6);
        }
    }

    #[test]
    fn t_junction_splits() {
        let lines = vec![
            vec![
                pt(0.0, 0.0),
                pt(10.0, 0.0),
                pt(10.0, 10.0),
                pt(0.0, 10.0),
                pt(0.0, 0.0),
            ],
            vec![pt(5.0, 0.0), pt(5.0, 10.0)],
        ];
        assert_eq!(polygonize(&lines).len(), 2);
    }

    #[test]
    fn overlapping_duplicate_edges_are_fine() {
        let lines = vec![
            vec![
                pt(0.0, 0.0),
                pt(10.0, 0.0),
                pt(10.0, 10.0),
                pt(0.0, 10.0),
                pt(0.0, 0.0),
            ],
            vec![pt(0.0, 0.0), pt(10.0, 0.0)],
        ];
        assert_eq!(polygonize(&lines).len(), 1);
    }
}
