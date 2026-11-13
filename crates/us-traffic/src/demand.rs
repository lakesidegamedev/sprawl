//! Travel demand: zones from lots, a gravity model with travel time deterrence, peak hour trips.

use crate::assign::zone_times;
use crate::network::{Network, class_capacity};
use std::collections::BTreeMap;
use us_gen::{LotKind, Pt, Town, pt};

#[derive(Clone, Copy, Debug)]
pub struct DemandParams {
    /// Side of the square cells lots are grouped into before attaching to the network, metres.
    pub zone_cell: f64,
    /// Peak hour vehicle trips leaving each dwelling unit.
    pub trips_per_unit: f64,
    /// Deterrence f(t) = exp(-beta * t) with t in minutes at free flow.
    pub beta: f64,
    /// Share of all trips that start or end outside the town, entering by the highway and the
    /// roads that reach the map edge.
    pub external_share: f64,
}

impl Default for DemandParams {
    fn default() -> Self {
        DemandParams {
            zone_cell: 400.0,
            trips_per_unit: 1.0,
            beta: 0.12,
            external_share: 0.15,
        }
    }
}

/// Dwelling units a lot holds. Parks, parking and civic lots hold none.
pub fn production_homes(k: LotKind) -> f64 {
    match k {
        LotKind::House | LotKind::Row => 1.0,
        LotKind::Apartments => 10.0,
        LotKind::Farm => 1.0,
        _ => 0.0,
    }
}

/// Jobs and customers a lot draws trips to: shops, offices, industry, schools and the like.
pub fn attraction_jobs(k: LotKind) -> f64 {
    match k {
        LotKind::Shop => 5.0,
        LotKind::Office => 30.0,
        LotKind::Workshop => 20.0,
        LotKind::Warehouse => 15.0,
        LotKind::BigBox => 80.0,
        LotKind::Strip => 25.0,
        LotKind::FastFood => 8.0,
        LotKind::Gas => 4.0,
        LotKind::School => 25.0,
        LotKind::Church => 2.0,
        LotKind::Fire => 5.0,
        LotKind::Sports => 2.0,
        LotKind::Farm => 2.0,
        _ => 0.0,
    }
}

/// (homes, jobs, weighted x, weighted y, weight) of the lots in one cell.
type CellSum = (f64, f64, f64, f64, f64);

#[derive(Clone, Debug)]
pub struct Zone {
    /// Network node trips start and end on.
    pub node: u32,
    /// Peak hour trips produced (homes).
    pub prod: f64,
    /// Jobs attracting trips.
    pub attr: f64,
    pub centroid: Pt,
}

#[derive(Clone, Debug)]
pub struct Demand {
    pub zones: Vec<Zone>,
    /// Trips per peak hour from zone i to zone j, row major, `zones.len()` squared. Zero diagonal.
    pub od: Vec<f64>,
    pub total: f64,
}

impl Demand {
    /// Morning peak: homes send trips to jobs. Trips that cannot reach any job are dropped.
    pub fn from_town(t: &Town, net: &Network, p: &DemandParams) -> Demand {
        let grid = net.node_grid();
        // cell -> (prod, attr, sum x, sum y, weight)
        let mut cells: BTreeMap<(i64, i64), CellSum> = BTreeMap::new();
        for l in &t.lots {
            let (h, j) = (production_homes(l.kind), attraction_jobs(l.kind));
            if h == 0.0 && j == 0.0 {
                continue;
            }
            let c = cells
                .entry((
                    (l.centre.x / p.zone_cell).floor() as i64,
                    (l.centre.y / p.zone_cell).floor() as i64,
                ))
                .or_default();
            let w = h + j;
            c.0 += h;
            c.1 += j;
            c.2 += l.centre.x * w;
            c.3 += l.centre.y * w;
            c.4 += w;
        }
        let mut by_node: BTreeMap<u32, Zone> = BTreeMap::new();
        for (_, c) in cells {
            let centroid = pt(c.2 / c.4, c.3 / c.4);
            let Some(node) = net.nearest_node(&grid, centroid) else {
                continue;
            };
            let z = by_node.entry(node).or_insert(Zone {
                node,
                prod: 0.0,
                attr: 0.0,
                centroid,
            });
            z.prod += c.0 * p.trips_per_unit;
            z.attr += c.1;
        }
        let mut zones: Vec<Zone> = by_node.into_values().collect();
        // gateways: dead ends of collectors and bigger at the map edge
        let (tp, ta): (f64, f64) = zones
            .iter()
            .fold((0.0, 0.0), |a, z| (a.0 + z.prod, a.1 + z.attr));
        let mut deg = vec![0u32; net.n_nodes()];
        let mut best = vec![0.0f64; net.n_nodes()];
        for e in &net.streets.edges {
            deg[e.a as usize] += 1;
            deg[e.b as usize] += 1;
            let w = class_capacity(e.class);
            best[e.a as usize] = best[e.a as usize].max(w);
            best[e.b as usize] = best[e.b as usize].max(w);
        }
        let size = t.size;
        let gates: Vec<(u32, f64)> = (0..net.n_nodes())
            .filter(|&n| {
                let q = net.streets.nodes[n];
                deg[n] == 1
                    && best[n] >= 900.0
                    && (q.x < 50.0 || q.y < 50.0 || q.x > size - 50.0 || q.y > size - 50.0)
            })
            .map(|n| (n as u32, best[n]))
            .collect();
        let gw: f64 = gates.iter().map(|g| g.1).sum();
        if gw > 0.0 && p.external_share > 0.0 {
            let k = p.external_share / (1.0 - p.external_share);
            for (n, w) in gates {
                let share = w / gw;
                let z = Zone {
                    node: n,
                    prod: tp * k * share,
                    attr: ta * k * share,
                    centroid: net.streets.nodes[n as usize],
                };
                match zones.iter_mut().find(|z2| z2.node == n) {
                    Some(z2) => {
                        z2.prod += z.prod;
                        z2.attr += z.attr;
                    }
                    None => zones.push(z),
                }
            }
            zones.sort_by_key(|z| z.node);
        }
        Demand::gravity(net, zones, p)
    }

    /// Gravity model on free flow times.
    pub fn gravity(net: &Network, zones: Vec<Zone>, p: &DemandParams) -> Demand {
        let n = zones.len();
        let t0: Vec<f64> = net.arcs.iter().map(|a| a.t0).collect();
        let nodes: Vec<u32> = zones.iter().map(|z| z.node).collect();
        let tt = zone_times(net, &t0, &nodes);
        let mut od = vec![0.0; n * n];
        let mut total = 0.0;
        for i in 0..n {
            if zones[i].prod <= 0.0 {
                continue;
            }
            let mut w = vec![0.0; n];
            let mut sum = 0.0;
            for j in 0..n {
                if i == j || !tt[i * n + j].is_finite() {
                    continue;
                }
                w[j] = zones[j].attr * libm::exp(-p.beta * tt[i * n + j] / 60.0);
                sum += w[j];
            }
            if sum <= 0.0 {
                continue;
            }
            for j in 0..n {
                let q = zones[i].prod * w[j] / sum;
                od[i * n + j] = q;
                total += q;
            }
        }
        Demand { zones, od, total }
    }

    pub fn scaled(&self, k: f64) -> Demand {
        Demand {
            zones: self.zones.clone(),
            od: self.od.iter().map(|q| q * k).collect(),
            total: self.total * k,
        }
    }
}
