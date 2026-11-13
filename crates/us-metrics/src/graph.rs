//! Road graph builder shared by generated towns and real towns.
//!
//! `build` nodes a set of classed polylines (snapping vertices within a tolerance and, for
//! generated towns, splitting segments where they cross or where an end touches another road)
//! into a fine graph with one edge per straight piece. `Graph::contract` merges the degree 2
//! chains into street edges that keep their polyline.

use std::collections::{BTreeMap, HashMap};
use us_gen::{Pt, RoadClass};

#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub a: u32,
    pub b: u32,
    pub class: RoadClass,
    pub len: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Pt>,
    pub edges: Vec<Edge>,
}

/// A street: a chain of fine edges between two junctions or dead ends, all of one class.
#[derive(Clone, Debug)]
pub struct Street {
    pub a: u32,
    pub b: u32,
    pub class: RoadClass,
    pub len: f64,
    /// Polyline from node `a` to node `b`.
    pub pts: Vec<Pt>,
}

#[derive(Clone, Debug, Default)]
pub struct StreetGraph {
    pub nodes: Vec<Pt>,
    pub edges: Vec<Street>,
}

/// Higher is bigger. Rail is not a road and has no rank among streets.
pub fn rank(c: RoadClass) -> u8 {
    match c {
        RoadClass::Highway => 5,
        RoadClass::Arterial => 4,
        RoadClass::Collector => 3,
        RoadClass::Local => 2,
        RoadClass::Alley => 1,
        RoadClass::Rail => 0,
    }
}

struct Snap {
    tol: f64,
    pts: Vec<Pt>,
    map: HashMap<(i64, i64), Vec<u32>>,
}

impl Snap {
    fn key(&self, p: Pt) -> (i64, i64) {
        (
            (p.x / self.tol).floor() as i64,
            (p.y / self.tol).floor() as i64,
        )
    }
    fn get(&mut self, p: Pt) -> u32 {
        let (kx, ky) = self.key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(v) = self.map.get(&(kx + dx, ky + dy)) {
                    for &i in v {
                        if self.pts[i as usize].dist(p) <= self.tol {
                            return i;
                        }
                    }
                }
            }
        }
        let id = self.pts.len() as u32;
        self.pts.push(p);
        self.map.entry((kx, ky)).or_default().push(id);
        id
    }
}

struct Seg {
    a: Pt,
    b: Pt,
    class: RoadClass,
}

fn closest_param(p: Pt, a: Pt, b: Pt) -> (f64, f64) {
    let ab = b - a;
    let l2 = ab.dot(ab);
    if l2 < 1e-18 {
        return (0.0, p.dist(a));
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    (t, p.dist(a + ab * t))
}

/// Build the fine graph. With `intersect`, crossings and T touches are noded; without it only
/// shared (snapped) vertices connect, which is right for OpenStreetMap ways.
pub fn build(lines: &[(RoadClass, Vec<Pt>)], tol: f64, intersect: bool) -> Graph {
    let mut segs: Vec<Seg> = Vec::new();
    for (class, pts) in lines {
        if *class == RoadClass::Rail {
            continue;
        }
        for w in pts.windows(2) {
            if w[0].dist(w[1]) > 1e-6 {
                segs.push(Seg {
                    a: w[0],
                    b: w[1],
                    class: *class,
                });
            }
        }
    }
    let n = segs.len();
    let mut splits: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; n];
    if intersect && n > 0 {
        let cell = 100.0;
        let mut grid: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
        let cells = |s: &Seg| {
            let x0 = ((s.a.x.min(s.b.x) - tol) / cell).floor() as i64;
            let x1 = ((s.a.x.max(s.b.x) + tol) / cell).floor() as i64;
            let y0 = ((s.a.y.min(s.b.y) - tol) / cell).floor() as i64;
            let y1 = ((s.a.y.max(s.b.y) + tol) / cell).floor() as i64;
            (x0, x1, y0, y1)
        };
        for (i, s) in segs.iter().enumerate() {
            let (x0, x1, y0, y1) = cells(s);
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    grid.entry((cx, cy)).or_default().push(i as u32);
                }
            }
        }
        for i in 0..n {
            let (x0, x1, y0, y1) = cells(&segs[i]);
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    let Some(v) = grid.get(&(cx, cy)) else {
                        continue;
                    };
                    for &j in v {
                        let j = j as usize;
                        if j <= i {
                            continue;
                        }
                        node_pair(&segs, i, j, tol, &mut splits);
                    }
                }
            }
        }
    }
    let mut snap = Snap {
        tol,
        pts: Vec::new(),
        map: HashMap::new(),
    };
    let mut emap: BTreeMap<(u32, u32), RoadClass> = BTreeMap::new();
    for (i, s) in segs.iter().enumerate() {
        let sp = &mut splits[i];
        sp.sort_by(|a, b| a.total_cmp(b));
        let mut prev: Option<u32> = None;
        let mut last_t = -1.0;
        for &t in sp.iter() {
            if t - last_t < 1e-9 && prev.is_some() {
                continue;
            }
            last_t = t;
            let p = s.a.lerp(s.b, t);
            let id = snap.get(p);
            if let Some(pv) = prev
                && pv != id
            {
                let key = (pv.min(id), pv.max(id));
                let e = emap.entry(key).or_insert(s.class);
                if rank(s.class) > rank(*e) {
                    *e = s.class;
                }
            }
            prev = Some(id);
        }
    }
    let edges = emap
        .into_iter()
        .map(|((a, b), class)| Edge {
            a,
            b,
            class,
            len: snap.pts[a as usize].dist(snap.pts[b as usize]),
        })
        .collect();
    Graph {
        nodes: snap.pts,
        edges,
    }
}

fn node_pair(segs: &[Seg], i: usize, j: usize, tol: f64, splits: &mut [Vec<f64>]) {
    let (a, b) = (segs[i].a, segs[i].b);
    let (c, d) = (segs[j].a, segs[j].b);
    // crossing
    let r = b - a;
    let s = d - c;
    let den = r.cross(s);
    if den.abs() > 1e-12 {
        let qp = c - a;
        let t = qp.cross(s) / den;
        let u = qp.cross(r) / den;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            splits[i].push(t);
            splits[j].push(u);
        }
    }
    // ends touching the other segment within tolerance
    for (p, t_self) in [(a, 0.0), (b, 1.0)] {
        let (u, dist) = closest_param(p, c, d);
        if dist <= tol {
            splits[j].push(u);
            splits[i].push(t_self);
        }
    }
    for (p, u_self) in [(c, 0.0), (d, 1.0)] {
        let (t, dist) = closest_param(p, a, b);
        if dist <= tol {
            splits[i].push(t);
            splits[j].push(u_self);
        }
    }
}

impl Graph {
    pub fn degrees(&self) -> Vec<u32> {
        let mut d = vec![0u32; self.nodes.len()];
        for e in &self.edges {
            d[e.a as usize] += 1;
            d[e.b as usize] += 1;
        }
        d
    }

    /// Edges without the given classes, nodes untouched.
    pub fn without(&self, drop: &[RoadClass]) -> Graph {
        Graph {
            nodes: self.nodes.clone(),
            edges: self
                .edges
                .iter()
                .filter(|e| !drop.contains(&e.class))
                .copied()
                .collect(),
        }
    }

    /// CSR adjacency: for node `n`, entries `off[n]..off[n+1]` of `(neighbour, edge)`.
    pub fn csr(&self) -> (Vec<u32>, Vec<(u32, u32)>) {
        let nn = self.nodes.len();
        let mut off = vec![0u32; nn + 1];
        for e in &self.edges {
            off[e.a as usize + 1] += 1;
            off[e.b as usize + 1] += 1;
        }
        for i in 0..nn {
            off[i + 1] += off[i];
        }
        let mut fill = off.clone();
        let mut adj = vec![(0u32, 0u32); self.edges.len() * 2];
        for (ei, e) in self.edges.iter().enumerate() {
            adj[fill[e.a as usize] as usize] = (e.b, ei as u32);
            fill[e.a as usize] += 1;
            adj[fill[e.b as usize] as usize] = (e.a, ei as u32);
            fill[e.b as usize] += 1;
        }
        (off, adj)
    }

    /// Merge degree 2 chains of one class into streets. Nodes that remain are junctions, dead
    /// ends and class changes. Output node and edge order is deterministic.
    pub fn contract(&self) -> StreetGraph {
        let (off, adj) = self.csr();
        let nn = self.nodes.len();
        let keep: Vec<bool> = (0..nn)
            .map(|n| {
                let s = &adj[off[n] as usize..off[n + 1] as usize];
                match s.len() {
                    0 => false,
                    2 => self.edges[s[0].1 as usize].class != self.edges[s[1].1 as usize].class,
                    _ => true,
                }
            })
            .collect();
        let mut keep = keep;
        let mut used = vec![false; self.edges.len()];
        let mut out_edges: Vec<(u32, u32, RoadClass, Vec<Pt>)> = Vec::new();
        let walk = |start: u32, first: u32, keep: &[bool], used: &mut Vec<bool>| {
            let mut pts = vec![self.nodes[start as usize]];
            let mut cur = start;
            let mut e = first;
            let class = self.edges[e as usize].class;
            loop {
                used[e as usize] = true;
                let ed = &self.edges[e as usize];
                let nxt = if ed.a == cur { ed.b } else { ed.a };
                pts.push(self.nodes[nxt as usize]);
                cur = nxt;
                if keep[cur as usize] {
                    break;
                }
                let s = &adj[off[cur as usize] as usize..off[cur as usize + 1] as usize];
                let Some(&(_, ne)) = s.iter().find(|&&(_, x)| x != e) else {
                    break;
                };
                if used[ne as usize] {
                    break;
                }
                e = ne;
            }
            (start, cur, class, pts)
        };
        for n in 0..nn {
            if !keep[n] {
                continue;
            }
            for k in off[n]..off[n + 1] {
                let e = adj[k as usize].1;
                if !used[e as usize] {
                    out_edges.push(walk(n as u32, e, &keep, &mut used));
                }
            }
        }
        // pure cycles of degree 2 nodes
        for e in 0..self.edges.len() {
            if !used[e] {
                let start = self.edges[e].a;
                keep[start as usize] = true;
                out_edges.push(walk(start, e as u32, &keep, &mut used));
            }
        }
        let mut remap: Vec<i64> = vec![-1; nn];
        let mut nodes: Vec<Pt> = Vec::new();
        let mut edges: Vec<Street> = Vec::new();
        for (a, b, class, pts) in out_edges {
            let mut id = |n: u32| -> u32 {
                if remap[n as usize] < 0 {
                    remap[n as usize] = nodes.len() as i64;
                    nodes.push(self.nodes[n as usize]);
                }
                remap[n as usize] as u32
            };
            let (a2, b2) = (id(a), id(b));
            let len = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
            edges.push(Street {
                a: a2,
                b: b2,
                class,
                len,
                pts,
            });
        }
        StreetGraph { nodes, edges }
    }
}

impl StreetGraph {
    pub fn csr(&self) -> (Vec<u32>, Vec<(u32, u32)>) {
        let nn = self.nodes.len();
        let mut off = vec![0u32; nn + 1];
        for e in &self.edges {
            off[e.a as usize + 1] += 1;
            off[e.b as usize + 1] += 1;
        }
        for i in 0..nn {
            off[i + 1] += off[i];
        }
        let mut fill = off.clone();
        let mut adj = vec![(0u32, 0u32); self.edges.len() * 2];
        for (ei, e) in self.edges.iter().enumerate() {
            adj[fill[e.a as usize] as usize] = (e.b, ei as u32);
            fill[e.a as usize] += 1;
            adj[fill[e.b as usize] as usize] = (e.a, ei as u32);
            fill[e.b as usize] += 1;
        }
        (off, adj)
    }

    /// Component id per node, and the size of each component.
    pub fn components(&self) -> (Vec<u32>, Vec<u32>) {
        let (off, adj) = self.csr();
        let nn = self.nodes.len();
        let mut comp = vec![u32::MAX; nn];
        let mut sizes: Vec<u32> = Vec::new();
        let mut stack: Vec<u32> = Vec::new();
        for s in 0..nn {
            if comp[s] != u32::MAX {
                continue;
            }
            let id = sizes.len() as u32;
            sizes.push(0);
            comp[s] = id;
            stack.push(s as u32);
            while let Some(u) = stack.pop() {
                sizes[id as usize] += 1;
                for k in off[u as usize]..off[u as usize + 1] {
                    let v = adj[k as usize].0;
                    if comp[v as usize] == u32::MAX {
                        comp[v as usize] = id;
                        stack.push(v);
                    }
                }
            }
        }
        (comp, sizes)
    }
}

/// A bounded face of the planar graph.
#[derive(Clone, Debug)]
pub struct Face {
    pub area: f64,
    pub perimeter: f64,
    pub ring: Vec<Pt>,
}

/// Bounded faces of the fine graph, after pruning dead ends. Counter-clockwise rings.
pub fn faces(g: &Graph) -> Vec<Face> {
    let nn = g.nodes.len();
    let ne = g.edges.len();
    let mut alive = vec![true; ne];
    let mut deg = g.degrees();
    let (off, adj) = g.csr();
    let mut stack: Vec<u32> = (0..nn as u32).filter(|&n| deg[n as usize] == 1).collect();
    while let Some(n) = stack.pop() {
        if deg[n as usize] != 1 {
            continue;
        }
        for k in off[n as usize]..off[n as usize + 1] {
            let (m, e) = adj[k as usize];
            if alive[e as usize] {
                alive[e as usize] = false;
                deg[n as usize] -= 1;
                deg[m as usize] -= 1;
                if deg[m as usize] == 1 {
                    stack.push(m);
                }
            }
        }
    }
    // per node: alive edges sorted by outgoing angle
    let mut order: Vec<Vec<u32>> = vec![Vec::new(); nn];
    for (ei, e) in g.edges.iter().enumerate() {
        if alive[ei] {
            order[e.a as usize].push(ei as u32);
            order[e.b as usize].push(ei as u32);
        }
    }
    let ang = |from: u32, e: u32| -> f64 {
        let ed = &g.edges[e as usize];
        let to = if ed.a == from { ed.b } else { ed.a };
        let d = g.nodes[to as usize] - g.nodes[from as usize];
        libm::atan2(d.y, d.x)
    };
    let mut pos: Vec<[u32; 2]> = vec![[0, 0]; ne];
    for (n, slot) in order.iter_mut().enumerate() {
        let mut v = std::mem::take(slot);
        v.sort_by(|&x, &y| {
            ang(n as u32, x)
                .total_cmp(&ang(n as u32, y))
                .then(x.cmp(&y))
        });
        for (i, &e) in v.iter().enumerate() {
            let side = if g.edges[e as usize].a == n as u32 {
                0
            } else {
                1
            };
            pos[e as usize][side] = i as u32;
        }
        *slot = v;
    }
    let mut seen = vec![[false; 2]; ne];
    let mut out = Vec::new();
    for e0 in 0..ne {
        if !alive[e0] {
            continue;
        }
        for s0 in 0..2usize {
            if seen[e0][s0] {
                continue;
            }
            // half-edge (e, side): leaves the node `a` if side 0, else `b`
            let mut ring: Vec<Pt> = Vec::new();
            let (mut e, mut s) = (e0 as u32, s0);
            let mut perim = 0.0;
            let mut guard = 0usize;
            loop {
                seen[e as usize][s] = true;
                let ed = &g.edges[e as usize];
                let (from, to) = if s == 0 { (ed.a, ed.b) } else { (ed.b, ed.a) };
                ring.push(g.nodes[from as usize]);
                perim += ed.len;
                // at `to`, take the previous edge in angular order from the reverse edge
                let ts = if g.edges[e as usize].a == to { 0 } else { 1 };
                let list = &order[to as usize];
                let p = pos[e as usize][ts] as usize;
                let ne2 = list[(p + list.len() - 1) % list.len()];
                let ns = if g.edges[ne2 as usize].a == to { 0 } else { 1 };
                e = ne2;
                s = ns;
                guard += 1;
                if (e == e0 as u32 && s == s0) || guard > ne * 2 + 4 {
                    break;
                }
            }
            let area = shoelace(&ring);
            if area > 1e-6 {
                out.push(Face {
                    area,
                    perimeter: perim,
                    ring,
                });
            }
        }
    }
    out
}

pub fn shoelace(ring: &[Pt]) -> f64 {
    let mut a = 0.0;
    for i in 0..ring.len() {
        let p = ring[i];
        let q = ring[(i + 1) % ring.len()];
        a += p.x * q.y - q.x * p.y;
    }
    a * 0.5
}
