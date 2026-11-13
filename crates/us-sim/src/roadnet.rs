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
