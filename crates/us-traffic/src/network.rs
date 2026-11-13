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
}

#[derive(Clone, Debug)]
pub struct Network {
    pub streets: StreetGraph,
    /// Arc `2 * s` runs a to b of street `s`, arc `2 * s + 1` runs b to a.
    pub arcs: Vec<Arc>,
    /// CSR: arcs leaving node `n` are `out[off[n]..off[n + 1]]` (arc indices).
    pub off: Vec<u32>,
    pub out: Vec<u32>,
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
            });
            arcs.push(Arc {
                from: e.b,
                to: e.a,
                street: i as u32,
                t0,
                cap,
            });
        }
        let nn = streets.nodes.len();
        let mut off = vec![0u32; nn + 1];
        for a in &arcs {
            off[a.from as usize + 1] += 1;
        }
        for i in 0..nn {
            off[i + 1] += off[i];
        }
        let mut fill = off.clone();
        let mut out = vec![0u32; arcs.len()];
        for (i, a) in arcs.iter().enumerate() {
            out[fill[a.from as usize] as usize] = i as u32;
            fill[a.from as usize] += 1;
        }
        Network {
            streets,
            arcs,
            off,
            out,
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
