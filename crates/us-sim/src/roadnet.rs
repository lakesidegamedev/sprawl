//! The revealed road network as a graph, for road travel distance (service coverage).

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use us_gen::{Pt, RoadClass};
use us_traffic::{Network, NodeGrid};

pub(crate) struct RoadNet {
    pub net: Network,
    grid: NodeGrid,
}

impl RoadNet {
    /// Build from the roads that exist now.
    pub fn build(roads: &[(RoadClass, Vec<Pt>)]) -> RoadNet {
        let g = us_metrics::roads_graph(roads.iter().map(|(c, p)| (*c, p.as_slice())))
            .without(&[RoadClass::Alley, RoadClass::Rail]);
        let net = Network::from_streets(g.contract());
        let grid = net.node_grid();
        RoadNet { net, grid }
    }

    /// Nearest drivable junction and the straight distance to it in metres.
    pub fn attach(&self, p: Pt) -> Option<(u32, f64)> {
        self.net
            .nearest_node(&self.grid, p)
            .map(|n| (n, self.net.streets.nodes[n as usize].dist(p)))
    }

    /// Road distance in metres from `src` to every node within `limit`, `f32::MAX` beyond.
    pub fn distances(&self, src: u32, limit: f64) -> Vec<f32> {
        let n = self.net.n_nodes();
        let mut dist = vec![u32::MAX; n];
        let cap = (limit * 10.0) as u32;
        let mut heap = BinaryHeap::new();
        dist[src as usize] = 0;
        heap.push(Reverse((0u32, src)));
        while let Some(Reverse((d, u))) = heap.pop() {
            if d > dist[u as usize] {
                continue;
            }
            let (a, b) = (
                self.net.off[u as usize] as usize,
                self.net.off[u as usize + 1] as usize,
            );
            for &ai in &self.net.out[a..b] {
                let arc = &self.net.arcs[ai as usize];
                let len = self.net.streets.edges[arc.street as usize].len;
                let nd = d.saturating_add((len * 10.0) as u32);
                if nd <= cap && nd < dist[arc.to as usize] {
                    dist[arc.to as usize] = nd;
                    heap.push(Reverse((nd, arc.to)));
                }
            }
        }
        dist.into_iter()
            .map(|d| {
                if d == u32::MAX {
                    f32::MAX
                } else {
                    d as f32 / 10.0
                }
            })
            .collect()
    }
}

/// Siren speed over the free-flow speed of the road.
const SIREN_BOOST: f64 = 1.35;

/// Fastest routes from one junction, by travel time with sirens.
pub(crate) struct Tree {
    /// Seconds from the source to each node, `f32::MAX` when out of reach.
    pub secs: Vec<f32>,
    /// The arc each node was reached by, `u32::MAX` for the source and unreached nodes.
    pred: Vec<u32>,
}

impl RoadNet {
    /// Fastest travel time in seconds from `src` to every node within `limit` seconds.
    pub fn tree(&self, src: u32, limit: f64) -> Tree {
        let n = self.net.n_nodes();
        let mut dist = vec![u64::MAX; n];
        let mut pred = vec![u32::MAX; n];
        let cap = (limit * 100.0) as u64;
        let mut heap = BinaryHeap::new();
        dist[src as usize] = 0;
        heap.push(Reverse((0u64, src)));
        while let Some(Reverse((d, u))) = heap.pop() {
            if d > dist[u as usize] {
                continue;
            }
            let (a, b) = (
                self.net.off[u as usize] as usize,
                self.net.off[u as usize + 1] as usize,
            );
            for &ai in &self.net.out[a..b] {
                let arc = &self.net.arcs[ai as usize];
                let nd = d + (arc.t0 / SIREN_BOOST * 100.0) as u64;
                if nd <= cap && nd < dist[arc.to as usize] {
                    dist[arc.to as usize] = nd;
                    pred[arc.to as usize] = ai;
                    heap.push(Reverse((nd, arc.to)));
                }
            }
        }
        Tree {
            secs: dist
                .into_iter()
                .map(|d| {
                    if d == u64::MAX {
                        f32::MAX
                    } else {
                        d as f32 / 100.0
                    }
                })
                .collect(),
            pred,
        }
    }

    /// The way from `to` back to the tree's source as a polyline (starting at `to`).
    pub fn route_to_source(&self, tree: &Tree, to: u32) -> Vec<Pt> {
        let mut out: Vec<Pt> = Vec::new();
        let mut at = to;
        let mut guard = 0;
        while tree.pred[at as usize] != u32::MAX && guard < 100_000 {
            guard += 1;
            let ai = tree.pred[at as usize] as usize;
            let arc = &self.net.arcs[ai];
            let e = &self.net.streets.edges[arc.street as usize];
            // the arc ran from `arc.from` to `at`; we travel it backwards
            if ai.is_multiple_of(2) {
                // a to b was the way the source side came: we go b to a
                out.extend(e.pts.iter().rev().copied());
            } else {
                out.extend(e.pts.iter().copied());
            }
            at = arc.from;
        }
        out
    }
}
