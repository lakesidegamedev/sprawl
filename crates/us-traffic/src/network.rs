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

fn approach_cost(
    class: RoadClass,
    length: f64,
    arms: impl ExactSizeIterator<Item = RoadClass>,
) -> (f64, f64) {
    let count = arms.len();
    let travel = length / class_speed(class);
    let cap = class_capacity(class);
    if !us_gen::signalized_junction(arms) {
        return (travel + class_node_delay(class), cap);
    }
    let cycle = count as f64 * us_gen::SIGNAL_APPROACH_SECONDS;
    let green = us_gen::SIGNAL_GREEN_SECONDS;
    let red = cycle - green;
    // For uniformly arriving cars, only arrivals during red wait.
    let delay = red * red / (2.0 * cycle);
    let lanes = if class == RoadClass::Arterial {
        2.0
    } else {
        1.0
    };
    (travel + delay, cap.min(1800.0 * lanes * green / cycle))
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
    junction_streets: Vec<Vec<usize>>,
}

const CELL: f64 = 200.0;

impl Network {
    /// Drivable network of a town: every road class except rail and alleys.
    pub fn from_town(t: &Town) -> Network {
        let g = us_metrics::town_graph(t).without(&[RoadClass::Alley, RoadClass::Rail]);
        Network::from_streets(g.contract())
    }

    pub fn from_streets(streets: StreetGraph) -> Network {
        let mut junction_streets = vec![Vec::new(); streets.nodes.len()];
        for (index, edge) in streets.edges.iter().enumerate() {
            for node in [edge.a, edge.b] {
                junction_streets[node as usize].push(index);
            }
        }
        let mut arcs = Vec::with_capacity(streets.edges.len() * 2);
        for (i, e) in streets.edges.iter().enumerate() {
            for (from, to, enabled) in [
                (e.a, e.b, e.travel.allows_forward()),
                (e.b, e.a, e.travel.reversed().allows_forward()),
            ] {
                let (t0, cap) = approach_cost(
                    e.class,
                    e.len,
                    junction_streets[to as usize]
                        .iter()
                        .map(|&street| streets.edges[street].class),
                );
                arcs.push(Arc {
                    from,
                    to,
                    street: i as u32,
                    t0,
                    cap,
                    enabled,
                });
            }
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
            junction_streets,
        }
    }

    /// Travel time and capacity if the arc's street were widened to this class.
    pub fn widened_cost(&self, arc: usize, class: RoadClass) -> (f64, f64) {
        let arc = self.arcs[arc];
        let widened = arc.street as usize;
        let length = self.streets.edges[widened].len;
        approach_cost(
            class,
            length,
            self.junction_streets[arc.to as usize]
                .iter()
                .map(|&street| {
                    if street == widened {
                        class
                    } else {
                        self.streets.edges[street].class
                    }
                }),
        )
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
    fn signal_delay_and_capacity_apply_only_to_arriving_arcs() {
        let east_west = [us_gen::pt(-100., 0.), us_gen::pt(100., 0.)];
        let north_south = [us_gen::pt(0., -100.), us_gen::pt(0., 100.)];
        let graph = us_metrics::roads_graph(
            [
                (RoadClass::Arterial, east_west.as_slice()),
                (RoadClass::Local, north_south.as_slice()),
            ]
            .into_iter(),
        );
        let net = Network::from_streets(graph.contract());
        let centre = us_gen::pt(0., 0.);
        assert_eq!(net.arcs.len(), 8);
        for (index, incoming) in net
            .arcs
            .iter()
            .enumerate()
            .filter(|(_, a)| net.streets.nodes[a.to as usize] == centre)
        {
            let outgoing = net.arcs[index ^ 1];
            assert!(incoming.cap < outgoing.cap);
            assert!(incoming.t0 > outgoing.t0);
        }
    }

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
