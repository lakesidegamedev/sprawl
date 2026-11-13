//! The traffic scorer: vehicle hours saved per peak hour by a change, and what it costs to build.

use crate::{Change, Context, End, Score, Scorer};
use us_traffic::{class_capacity, class_node_delay, class_speed};

pub struct TrafficScorer;

/// Metres of road counted for one lot lost.
pub const LOT_COST_M: f64 = 80.0;

fn class_cost_factor(c: us_gen::RoadClass) -> f64 {
    match c {
        us_gen::RoadClass::Collector => 1.0,
        us_gen::RoadClass::Arterial => 2.0,
        _ => 1.0,
    }
}

fn bpr(t0: f64, cap: f64, x: f64, alpha: f64) -> f64 {
    let r = x / cap;
    t0 * (1.0 + alpha * r * r * r * r)
}

impl Scorer for TrafficScorer {
    fn score(&self, ctx: &Context, change: &Change) -> Option<Score> {
        match change {
            Change::Upgrade {
                street,
                to,
                lots_removed,
            } => {
                let s = *street as usize;
                let e = &ctx.net.streets.edges[s];
                let t0 = e.len / class_speed(*to) + class_node_delay(*to);
                let cap = class_capacity(*to);
                let alpha = ctx.params.assign.bpr_alpha;
                let mut gain = 0.0;
                for k in 0..2 {
                    let ai = 2 * s + k;
                    let f = ctx.assignment.arc_flow[ai];
                    gain += f * (ctx.assignment.arc_time[ai] - bpr(t0, cap, f, alpha));
                }
                Some(Score {
                    gain: gain / 3600.0,
                    cost: e.len * class_cost_factor(*to) + LOT_COST_M * lots_removed.len() as f64,
                })
            }
            Change::Connector {
                a,
                b,
                pa,
                pb,
                class,
                lots_removed,
            } => {
                let sg = &ctx.net.streets;
                let l = pa.dist(*pb);
                let tau = l / class_speed(*class) + class_node_delay(*class);
                let dem = ctx.demand;
                let nz = dem.zones.len();
                // time from zone z to the end, from the skim (reverse times taken as equal)
                let at = |z: usize, end: &End| -> f32 {
                    let row = &ctx.skim.times[z];
                    match end {
                        End::Node(n) => row[*n as usize],
                        End::Street { street, frac, .. } => {
                            let e = &sg.edges[*street as usize];
                            let s = *street as usize;
                            let ta = (ctx.assignment.arc_time[2 * s]
                                + ctx.assignment.arc_time[2 * s + 1])
                                as f32
                                * 0.5;
                            let f = *frac as f32;
                            (row[e.a as usize] + f * ta).min(row[e.b as usize] + (1.0 - f) * ta)
                        }
                    }
                };
                let ta: Vec<f32> = (0..nz).map(|z| at(z, a)).collect();
                let tb: Vec<f32> = (0..nz).map(|z| at(z, b)).collect();
                let tau = tau as f32;
                let mut saved = 0.0f64;
                for i in 0..nz {
                    for j in 0..nz {
                        let q = dem.od[i * nz + j];
                        if q <= 0.0 {
                            continue;
                        }
                        let base = ctx.skim.times[i][dem.zones[j].node as usize];
                        let via = (ta[i] + tau + tb[j]).min(tb[i] + tau + ta[j]);
                        if via < base {
                            saved += q * (base - via) as f64;
                        }
                    }
                }
                Some(Score {
                    gain: saved / 3600.0,
                    cost: l + LOT_COST_M * lots_removed.len() as f64,
                })
            }
        }
    }
}
