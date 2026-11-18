//! The drivable network: street graph plus directed arcs with free-flow time and capacity.

use us_gen::{Pt, RoadClass, Town};
use us_metrics::graph::StreetGraph;

/// Free-flow speed in metres per second.
///
/// Highway 100 km/h, Arterial 55 km/h, Collector 45 km/h, Local 35 km/h, Alley 15 km/h.
pub fn class_speed(c: RoadClass) -> f64 {
    let kmh = match c {
        RoadClass::Highway => 100.0,
        RoadClass::Arterial => 55.0,
        RoadClass::Collector => 45.0,
        RoadClass::Local => 35.0,
        RoadClass::Alley => 15.0,
        RoadClass::Rail => 1.0,
    };
    kmh / 3.6
}

/// Capacity in vehicles per hour per direction.
///
/// Local 600 (one lane each way, parked cars, stop signs), Collector 900, Arterial 1,800
/// (4 lanes, two each way), Highway 4,000 (two lanes each way, free flow), Alley 300.
pub fn class_capacity(c: RoadClass) -> f64 {
    match c {
        RoadClass::Highway => 4000.0,
        RoadClass::Arterial => 1800.0,
        RoadClass::Collector => 900.0,
        RoadClass::Local => 600.0,
        RoadClass::Alley => 300.0,
        RoadClass::Rail => 1.0,
    }
}

/// Seconds lost at the junction a street ends on (stop sign, signal, turn): Local 3, Collector 5,
/// Arterial 8, Highway 0, Alley 3. Added to each street's free-flow time.
pub fn class_node_delay(c: RoadClass) -> f64 {
    match c {
        RoadClass::Highway => 0.0,
        RoadClass::Arterial => 8.0,
        RoadClass::Collector => 5.0,
        RoadClass::Local | RoadClass::Alley => 3.0,
        RoadClass::Rail => 0.0,
    }
}

/// One direction of a street.
#[derive(Clone, Copy, Debug)]
pub struct Arc {
    pub from: u32,
    pub to: u32,
    /// Index into `Network::streets.edges`.
    pub street: u32,
    /// Free-flow travel time in seconds.
    pub t0: f64,
    /// Vehicles per hour.
    pub cap: f64,
    /// Whether this direction is open to routing.
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub struct Network {
    pub streets: StreetGraph,
    /// Arc `2 * s` runs a to b of street `s`, arc `2 * s + 1` runs b to a.
    pub arcs: Vec<Arc>,
    /// CSR: arcs leaving node `n` are `out[off[n]..off[n + 1]]` (arc indices).
    pub off: Vec<u32>,
    pub out: Vec<u32>,
    /// CSR of incoming arcs, used by dispatch to find routes toward an incident.
    pub in_off: Vec<u32>,
    pub incoming: Vec<u32>,
}

const CELL: f64 = 200.0;

impl Network {
    /// Drivable network of a town: every road class except rail and alleys.
    pub fn from_town(t: &Town) -> Network {
        let g = us_metrics::town_graph(t).without(&[RoadClass::Alley, RoadClass::Rail]);
        Network::from_streets(g.contract())
    }

    pub fn from_streets(streets: StreetGraph) -> Network {
        let mut arcs = Vec::with_capacity(streets.edges.len() * 2);
        for (i, e) in streets.edges.iter().enumerate() {
            let t0 = e.len / class_speed(e.class) + class_node_delay(e.class);
            let cap = class_capacity(e.class);
            arcs.push(Arc {
                from: e.a,
                to: e.b,
                street: i as u32,
                t0,
                cap,
                enabled: e.travel.allows_forward(),
            });
            arcs.push(Arc {
                from: e.b,
                to: e.a,
                street: i as u32,
                t0,
                cap,
                enabled: e.travel.reversed().allows_forward(),
            });
        }
        let adjacency = |node: fn(&Arc) -> u32| {
            let mut off = vec![0u32; streets.nodes.len() + 1];
            for a in arcs.iter().filter(|a| a.enabled) {
                off[node(a) as usize + 1] += 1;
            }
            for i in 0..streets.nodes.len() {
                off[i + 1] += off[i];
            }
            let mut fill = off.clone();
            let mut edges = vec![0u32; off.last().copied().unwrap_or_default() as usize];
            for (i, a) in arcs.iter().enumerate().filter(|(_, a)| a.enabled) {
                let from = node(a) as usize;
                edges[fill[from] as usize] = i as u32;
                fill[from] += 1;
            }
            (off, edges)
        };
        let (off, out) = adjacency(|a| a.from);
        let (in_off, incoming) = adjacency(|a| a.to);
        Network {
            streets,
            arcs,
            off,
            out,
            in_off,
            incoming,
        }
    }

    pub fn n_nodes(&self) -> usize {
        self.streets.nodes.len()
    }

    /// Nearest node that a trip may start or end on: not a node whose streets are all highway.
    pub fn nearest_node(&self, grid: &NodeGrid, p: Pt) -> Option<u32> {
        grid.nearest(&self.streets, p)
    }

    pub fn node_grid(&self) -> NodeGrid {
        let mut cells: std::collections::HashMap<(i64, i64), Vec<u32>> =
            std::collections::HashMap::new();
        let mut hw_only = vec![true; self.n_nodes()];
        let mut touched = vec![false; self.n_nodes()];
        for e in &self.streets.edges {
            for n in [e.a, e.b] {
                touched[n as usize] = true;
                if e.class != RoadClass::Highway {
                    hw_only[n as usize] = false;
                }
            }
        }
        for (i, p) in self.streets.nodes.iter().enumerate() {
            if touched[i] && !hw_only[i] {
                cells
                    .entry(((p.x / CELL).floor() as i64, (p.y / CELL).floor() as i64))
                    .or_default()
                    .push(i as u32);
            }
        }
        NodeGrid { cells }
    }
}

pub struct NodeGrid {
    cells: std::collections::HashMap<(i64, i64), Vec<u32>>,
}

impl NodeGrid {
    fn nearest(&self, sg: &StreetGraph, p: Pt) -> Option<u32> {
        let (cx, cy) = ((p.x / CELL).floor() as i64, (p.y / CELL).floor() as i64);
        let mut best: Option<(f64, u32)> = None;
        for ring in 0..=30i64 {
            for dx in -ring..=ring {
                for dy in -ring..=ring {
                    if dx.abs().max(dy.abs()) != ring {
                        continue;
                    }
                    if let Some(v) = self.cells.get(&(cx + dx, cy + dy)) {
                        for &n in v {
                            let d = sg.nodes[n as usize].dist(p);
                            if best.is_none_or(|(bd, bn)| d < bd || (d == bd && n < bn)) {
                                best = Some((d, n));
                            }
                        }
                    }
                }
            }
            // anything in a farther ring is at least (ring) * CELL away
            if let Some((bd, _)) = best
                && bd <= ring as f64 * CELL
            {
                break;
            }
        }
        best.map(|(_, n)| n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_slip_lanes_disable_the_wrong_way_arc() {
        let town = us_gen::generate(us_gen::Preset::Grid, 1);
        let network = Network::from_town(&town);
        let disabled: Vec<_> = network
            .arcs
            .iter()
            .enumerate()
            .filter(|(_, arc)| !arc.enabled)
            .collect();
        assert!(!disabled.is_empty());
        for (index, arc) in disabled {
            assert!(network.arcs[index ^ 1].enabled);
            assert!(
                !network.out[network.off[arc.from as usize] as usize
                    ..network.off[arc.from as usize + 1] as usize]
                    .contains(&(index as u32))
            );
            assert!(
                !network.incoming[network.in_off[arc.to as usize] as usize
                    ..network.in_off[arc.to as usize + 1] as usize]
                    .contains(&(index as u32))
            );
        }
    }
}
