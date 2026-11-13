//! User equilibrium assignment: Frank-Wolfe with BPR delay on the zone to zone demand.

use crate::demand::Demand;
use crate::network::Network;
use std::collections::BinaryHeap;

#[derive(Clone, Copy, Debug)]
pub struct AssignParams {
    pub max_iter: usize,
    /// Stop when the relative gap (excess cost over the all-or-nothing cost) falls below this.
    pub gap_tol: f64,
    /// BPR: t = t0 * (1 + alpha * (v / c) ^ beta).
    pub bpr_alpha: f64,
    pub bpr_beta: f64,
    /// Origins are split into this many fixed chunks; sums are made chunk by chunk so the result
    /// does not depend on `threads`.
    pub chunks: usize,
    pub threads: usize,
}

impl Default for AssignParams {
    fn default() -> Self {
        AssignParams {
            max_iter: 24,
            gap_tol: 0.01,
            bpr_alpha: 0.15,
            bpr_beta: 4.0,
            chunks: 8,
            threads: 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Assignment {
    /// Vehicles per hour on each arc.
    pub arc_flow: Vec<f64>,
    /// Congested travel time of each arc, seconds.
    pub arc_time: Vec<f64>,
    /// Vehicles per hour on each street, both directions summed.
    pub edge_volume: Vec<f64>,
    /// Volume over capacity of each street: the busier direction.
    pub edge_vc: Vec<f64>,
    /// Vehicle hours spent in one peak hour.
    pub vht: f64,
    /// Vehicle hours the same trips would take at free flow on their free flow routes.
    pub vht_free: f64,
    pub gap: f64,
    pub iterations: usize,
}

#[derive(PartialEq)]
struct Item(f64, u32);
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

pub(crate) struct Scratch {
    dist: Vec<f64>,
    parent: Vec<u32>,
    order: Vec<u32>,
    load: Vec<f64>,
    heap: BinaryHeap<Item>,
}

impl Scratch {
    pub(crate) fn new(n: usize) -> Scratch {
        Scratch {
            dist: vec![f64::INFINITY; n],
            parent: vec![u32::MAX; n],
            order: Vec::with_capacity(n),
            load: vec![0.0; n],
            heap: BinaryHeap::new(),
        }
    }
}

/// Shortest path tree from `src`. With `dest`, stops once every marked node (count `n_dest`) is settled.
pub(crate) fn tree(
    net: &Network,
    t: &[f64],
    src: u32,
    sc: &mut Scratch,
    dest: Option<(&[bool], usize)>,
) {
    sc.dist.fill(f64::INFINITY);
    sc.parent.fill(u32::MAX);
    sc.order.clear();
    sc.heap.clear();
    sc.dist[src as usize] = 0.0;
    sc.heap.push(Item(0.0, src));
    let mut left = dest.map_or(usize::MAX, |d| d.1);
    while let Some(Item(d, u)) = sc.heap.pop() {
        if d > sc.dist[u as usize] {
            continue;
        }
        sc.order.push(u);
        if let Some((mark, _)) = dest
            && mark[u as usize]
        {
            left -= 1;
            if left == 0 {
                break;
            }
        }
        for k in net.off[u as usize]..net.off[u as usize + 1] {
            let ai = net.out[k as usize];
            let a = &net.arcs[ai as usize];
            let nd = d + t[ai as usize];
            if nd < sc.dist[a.to as usize] {
                sc.dist[a.to as usize] = nd;
                sc.parent[a.to as usize] = ai;
                sc.heap.push(Item(nd, a.to));
            }
        }
    }
}

/// All-or-nothing assignment of the whole demand on times `t`: flow per arc.
fn aon(net: &Network, t: &[f64], dem: &Demand, p: &AssignParams) -> Vec<f64> {
    let nz = dem.zones.len();
    let mut mark = vec![false; net.n_nodes()];
    for z in &dem.zones {
        mark[z.node as usize] = true;
    }
    let n_dest = mark.iter().filter(|&&m| m).count();
    let chunks = p.chunks.max(1).min(nz.max(1));
    let work = |ci: usize| -> Vec<f64> {
        let mut flow = vec![0.0; net.arcs.len()];
        let mut sc = Scratch::new(net.n_nodes());
        let (lo, hi) = (ci * nz / chunks, (ci + 1) * nz / chunks);
        for i in lo..hi {
            let any = (0..nz).any(|j| dem.od[i * nz + j] > 0.0);
            if !any {
                continue;
            }
            tree(net, t, dem.zones[i].node, &mut sc, Some((&mark, n_dest)));
            for l in sc.load.iter_mut() {
                *l = 0.0;
            }
            for j in 0..nz {
                let q = dem.od[i * nz + j];
                if q > 0.0 && sc.dist[dem.zones[j].node as usize].is_finite() {
                    sc.load[dem.zones[j].node as usize] += q;
                }
            }
            for k in (0..sc.order.len()).rev() {
                let n = sc.order[k] as usize;
                let pa = sc.parent[n];
                if pa == u32::MAX {
                    continue;
                }
                let l = sc.load[n];
                if l > 0.0 {
                    flow[pa as usize] += l;
                    sc.load[net.arcs[pa as usize].from as usize] += l;
                }
            }
        }
        flow
    };
    let parts: Vec<Vec<f64>> = if p.threads <= 1 || chunks == 1 {
        (0..chunks).map(work).collect()
    } else {
        let mut parts: Vec<Option<Vec<f64>>> = vec![None; chunks];
        let nt = p.threads.min(chunks);
        std::thread::scope(|s| {
            let work = &work;
            let hs: Vec<_> = (0..nt)
                .map(|ti| {
                    s.spawn(move || {
                        (ti..chunks)
                            .step_by(nt)
                            .map(|ci| (ci, work(ci)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for h in hs {
                for (ci, f) in h.join().expect("assignment worker") {
                    parts[ci] = Some(f);
                }
            }
        });
        parts.into_iter().map(|x| x.expect("chunk")).collect()
    };
    let mut flow = vec![0.0; net.arcs.len()];
    for part in parts {
        for (f, x) in flow.iter_mut().zip(part) {
            *f += x;
        }
    }
    flow
}

fn bpr(p: &AssignParams, t0: f64, cap: f64, x: f64) -> f64 {
    let r = x / cap;
    t0 * (1.0 + p.bpr_alpha * r * r * r * r * if p.bpr_beta == 4.0 { 1.0 } else { 0.0 })
}

fn time_of(net: &Network, p: &AssignParams, x: &[f64]) -> Vec<f64> {
    net.arcs
        .iter()
        .zip(x)
        .map(|(a, &f)| {
            if p.bpr_beta == 4.0 {
                bpr(p, a.t0, a.cap, f)
            } else {
                a.t0 * (1.0 + p.bpr_alpha * libm::pow(f / a.cap, p.bpr_beta))
            }
        })
        .collect()
}

/// Frank-Wolfe user equilibrium. Deterministic.
pub fn assign(net: &Network, dem: &Demand, p: &AssignParams) -> Assignment {
    let t0: Vec<f64> = net.arcs.iter().map(|a| a.t0).collect();
    let mut x = aon(net, &t0, dem, p);
    let vht_free = x.iter().zip(&t0).map(|(f, t)| f * t).sum::<f64>() / 3600.0;
    let mut gap = f64::INFINITY;
    let mut it = 0;
    while it < p.max_iter {
        it += 1;
        let t = time_of(net, p, &x);
        let y = aon(net, &t, dem, p);
        let cx: f64 = x.iter().zip(&t).map(|(f, t)| f * t).sum();
        let cy: f64 = y.iter().zip(&t).map(|(f, t)| f * t).sum();
        gap = if cx > 0.0 {
            ((cx - cy) / cx).max(0.0)
        } else {
            0.0
        };
        if gap < p.gap_tol {
            break;
        }
        // line search on the Beckmann objective: bisect the directional derivative
        let deriv = |a: f64| -> f64 {
            net.arcs
                .iter()
                .enumerate()
                .map(|(i, arc)| {
                    let d = y[i] - x[i];
                    let f = x[i] + a * d;
                    let tt = if p.bpr_beta == 4.0 {
                        bpr(p, arc.t0, arc.cap, f)
                    } else {
                        arc.t0 * (1.0 + p.bpr_alpha * libm::pow(f / arc.cap, p.bpr_beta))
                    };
                    tt * d
                })
                .sum()
        };
        let alpha = if deriv(1.0) <= 0.0 {
            1.0
        } else if deriv(0.0) >= 0.0 {
            0.0
        } else {
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..24 {
                let mid = 0.5 * (lo + hi);
                if deriv(mid) > 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            0.5 * (lo + hi)
        };
        if alpha == 0.0 {
            break;
        }
        for (xi, yi) in x.iter_mut().zip(&y) {
            *xi += alpha * (yi - *xi);
        }
    }
    let t = time_of(net, p, &x);
    let vht = x.iter().zip(&t).map(|(f, t)| f * t).sum::<f64>() / 3600.0;
    let ns = net.streets.edges.len();
    let mut edge_volume = vec![0.0; ns];
    let mut edge_vc = vec![0.0; ns];
    for (i, a) in net.arcs.iter().enumerate() {
        edge_volume[a.street as usize] += x[i];
        let r = x[i] / a.cap;
        if r > edge_vc[a.street as usize] {
            edge_vc[a.street as usize] = r;
        }
    }
    Assignment {
        arc_flow: x,
        arc_time: t,
        edge_volume,
        edge_vc,
        vht,
        vht_free,
        gap,
        iterations: it,
    }
}

/// Travel times (seconds) from each zone node to every node, on the given arc times.
#[derive(Clone, Debug)]
pub struct Skim {
    pub times: Vec<Vec<f32>>,
}

pub fn skim(net: &Network, t: &[f64], sources: &[u32]) -> Skim {
    let mut sc = Scratch::new(net.n_nodes());
    let times = sources
        .iter()
        .map(|&s| {
            tree(net, t, s, &mut sc, None);
            sc.dist.iter().map(|&d| d as f32).collect()
        })
        .collect();
    Skim { times }
}

/// Shortest times between all zone nodes at the given arc times, as a dense matrix.
pub(crate) fn zone_times(net: &Network, t: &[f64], nodes: &[u32]) -> Vec<f64> {
    let n = nodes.len();
    let mut mark = vec![false; net.n_nodes()];
    for &z in nodes {
        mark[z as usize] = true;
    }
    let n_dest = mark.iter().filter(|&&m| m).count();
    let mut sc = Scratch::new(net.n_nodes());
    let mut out = vec![f64::INFINITY; n * n];
    for i in 0..n {
        tree(net, t, nodes[i], &mut sc, Some((&mark, n_dest)));
        for j in 0..n {
            out[i * n + j] = sc.dist[nodes[j] as usize];
        }
    }
    out
}
