//! Realism metrics of a road network, for generated and real towns alike.
//!
//! Everything except the alley share is measured on the street graph without alleys: blocks are
//! bounded by streets, and OpenStreetMap maps alleys in a patchy way.

use crate::graph::{Graph, StreetGraph, faces, rank};
use std::collections::BinaryHeap;
use us_gen::{Pt, RoadClass};

/// Side of the analysis window in metres. Generated towns are 4096 m; real towns are cut to the same.
pub const WINDOW: f64 = 4096.0;
/// Nodes closer than this to the window edge are ignored for junction statistics.
pub const EDGE_MARGIN: f64 = 40.0;
/// Faces outside this area range (square metres) are not blocks: slivers and open land.
pub const BLOCK_MIN: f64 = 500.0;
pub const BLOCK_MAX: f64 = 250_000.0;

/// Metric names in report order.
pub const NAMES: [&str; 17] = [
    "block_area_p25",
    "block_area_median",
    "block_area_p90",
    "block_perimeter_mean",
    "share_3way",
    "share_4way",
    "share_dead_end",
    "orientation_entropy",
    "circuity",
    "intersection_density",
    "street_density",
    "share_highway",
    "share_arterial",
    "share_collector",
    "share_local",
    "share_alley",
    "giant_component_share",
];

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    pub values: [f64; 17],
    pub n_blocks: usize,
    pub n_circuity_pairs: usize,
}

impl Metrics {
    pub fn get(&self, name: &str) -> f64 {
        NAMES
            .iter()
            .position(|n| *n == name)
            .map(|i| self.values[i])
            .unwrap_or(f64::NAN)
    }
}

fn pct(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let pos = q * (sorted.len() - 1) as f64;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

/// Linear interpolated percentile of the finite values; `q` in [0, 1].
pub fn percentile(values: &[f64], q: f64) -> f64 {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    pct(&v, q)
}

struct Mix(u64);
impl Mix {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// Boeing style orientation entropy: 36 bins of 10 degrees, every street segment counted in both
/// directions, weighted by length, bins shifted so north is a bin centre. Segments of the street
/// polylines are used rather than junction to junction chords, so a bend is not smeared into a diagonal.
pub fn orientation_entropy(sg: &StreetGraph) -> f64 {
    let mut bins = [0.0f64; 36];
    for e in &sg.edges {
        for w in e.pts.windows(2) {
            let d = w[1] - w[0];
            let len = d.len();
            if len < 3.0 {
                continue;
            }
            // bearing clockwise from north, degrees
            let mut b = libm::atan2(d.x, d.y).to_degrees();
            if b < 0.0 {
                b += 360.0;
            }
            for bearing in [b, (b + 180.0) % 360.0] {
                let idx = (((bearing + 5.0) % 360.0) / 10.0).floor() as usize % 36;
                bins[idx] += len;
            }
        }
    }
    let total: f64 = bins.iter().sum();
    if total <= 0.0 {
        return 0.0;
    }
    -bins
        .iter()
        .filter(|&&w| w > 0.0)
        .map(|&w| {
            let p = w / total;
            p * libm::log(p)
        })
        .sum::<f64>()
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

/// Network distance from `src` to every node by street length.
pub fn dijkstra_len(sg: &StreetGraph, off: &[u32], adj: &[(u32, u32)], src: u32) -> Vec<f64> {
    let mut dist = vec![f64::INFINITY; sg.nodes.len()];
    let mut h = BinaryHeap::new();
    dist[src as usize] = 0.0;
    h.push(Item(0.0, src));
    while let Some(Item(d, u)) = h.pop() {
        if d > dist[u as usize] {
            continue;
        }
        for k in off[u as usize]..off[u as usize + 1] {
            let (v, e) = adj[k as usize];
            let nd = d + sg.edges[e as usize].len;
            if nd < dist[v as usize] {
                dist[v as usize] = nd;
                h.push(Item(nd, v));
            }
        }
    }
    dist
}

/// Mean of network distance over straight-line distance for sampled node pairs at least
/// `min_straight` metres apart, inside the largest connected piece. Returns (mean, pairs).
pub fn circuity(sg: &StreetGraph, min_straight: f64) -> (f64, usize) {
    let (comp, sizes) = sg.components();
    let Some((big, _)) = sizes
        .iter()
        .enumerate()
        .max_by_key(|&(i, &s)| (s, usize::MAX - i))
    else {
        return (f64::NAN, 0);
    };
    let (off, adj) = sg.csr();
    let nodes: Vec<u32> = (0..sg.nodes.len() as u32)
        .filter(|&n| comp[n as usize] == big as u32)
        .collect();
    if nodes.len() < 20 {
        return (f64::NAN, 0);
    }
    let mut rng = Mix(0xC1_2C_17_u64);
    let (mut sum, mut cnt) = (0.0, 0usize);
    for _ in 0..40 {
        let s = nodes[(rng.next() % nodes.len() as u64) as usize];
        let dist = dijkstra_len(sg, &off, &adj, s);
        let mut got = 0;
        let mut tries = 0;
        while got < 25 && tries < 200 {
            tries += 1;
            let t = nodes[(rng.next() % nodes.len() as u64) as usize];
            let straight = sg.nodes[s as usize].dist(sg.nodes[t as usize]);
            if straight < min_straight || !dist[t as usize].is_finite() {
                continue;
            }
            sum += dist[t as usize] / straight;
            cnt += 1;
            got += 1;
        }
    }
    if cnt == 0 {
        (f64::NAN, 0)
    } else {
        (sum / cnt as f64, cnt)
    }
}

fn near_edge(p: Pt) -> bool {
    p.x < EDGE_MARGIN
        || p.y < EDGE_MARGIN
        || p.x > WINDOW - EDGE_MARGIN
        || p.y > WINDOW - EDGE_MARGIN
}

/// Compute all metrics for a fine graph.
pub fn compute(full: &Graph) -> Metrics {
    let mut m = Metrics::default();
    let km2 = WINDOW * WINDOW / 1e6;
    // class shares include alleys; everything else is on the street graph
    let mut class_len = [0.0f64; 6];
    for e in &full.edges {
        class_len[rank(e.class) as usize] += e.len;
    }
    let total: f64 = class_len[1..].iter().sum();
    let g = full.without(&[RoadClass::Alley, RoadClass::Rail]);
    let street_len: f64 = g.edges.iter().map(|e| e.len).sum();

    let fs = faces(&g);
    let mut areas: Vec<f64> = Vec::new();
    let mut perims: Vec<f64> = Vec::new();
    for f in &fs {
        if (BLOCK_MIN..=BLOCK_MAX).contains(&f.area) {
            areas.push(f.area);
            perims.push(f.perimeter);
        }
    }
    areas.sort_by(|a, b| a.total_cmp(b));
    m.n_blocks = areas.len();
    let deg = g.degrees();
    let (mut d1, mut d3, mut d4) = (0usize, 0usize, 0usize);
    for (i, &d) in deg.iter().enumerate() {
        if near_edge(g.nodes[i]) {
            continue;
        }
        match d {
            1 => d1 += 1,
            3 => d3 += 1,
            x if x >= 4 => d4 += 1,
            _ => {}
        }
    }
    let nj = (d1 + d3 + d4).max(1) as f64;
    let sg = g.contract();
    let (circ, pairs) = circuity(&sg, 300.0);
    m.n_circuity_pairs = pairs;
    let (_, sizes) = sg.components();
    let big = sizes.iter().copied().max().unwrap_or(0) as f64;
    let all_nodes = sg.nodes.len().max(1) as f64;
    let v = &mut m.values;
    v[0] = pct(&areas, 0.25);
    v[1] = pct(&areas, 0.5);
    v[2] = pct(&areas, 0.9);
    v[3] = if perims.is_empty() {
        f64::NAN
    } else {
        perims.iter().sum::<f64>() / perims.len() as f64
    };
    v[4] = d3 as f64 / nj;
    v[5] = d4 as f64 / nj;
    v[6] = d1 as f64 / nj;
    v[7] = orientation_entropy(&sg);
    v[8] = circ;
    v[9] = (d3 + d4) as f64 / km2;
    v[10] = street_len / 1000.0 / km2;
    let t = total.max(1e-9);
    v[11] = class_len[5] / t;
    v[12] = class_len[4] / t;
    v[13] = class_len[3] / t;
    v[14] = class_len[2] / t;
    v[15] = class_len[1] / t;
    v[16] = big / all_nodes;
    m
}
