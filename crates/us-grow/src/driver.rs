//! The growth loop: assign traffic, propose, score, apply the best changes, repeat.

use crate::geom::{dist_pt_seg, seg_intersect};
use crate::{Change, Context, End, GrowParams, Index, Proposer, Scorer};
use std::collections::BTreeSet;
use us_gen::{Pt, Road, RoadClass, Town};
use us_traffic::{Assignment, Demand, Network, Skim, assign, skim};

pub struct Evaluation {
    pub net: Network,
    pub demand: Demand,
    pub assignment: Assignment,
    pub skim: Skim,
}

/// Traffic on a town at `scale` times its base peak demand.
pub fn evaluate(town: &Town, p: &GrowParams, scale: f64) -> Evaluation {
    let net = Network::from_town(town);
    let demand = Demand::from_town(town, &net, &p.demand).scaled(scale);
    let assignment = assign(&net, &demand, &p.assign);
    let nodes: Vec<u32> = demand.zones.iter().map(|z| z.node).collect();
    let skim = skim(&net, &assignment.arc_time, &nodes);
    Evaluation {
        net,
        demand,
        assignment,
        skim,
    }
}

#[derive(Clone, Debug)]
pub struct RoundLog {
    pub round: usize,
    pub scale: f64,
    pub vht: f64,
    pub vht_free: f64,
    pub gap: f64,
    pub over_capacity_streets: usize,
    pub over_capacity_m: f64,
    pub upgrades: usize,
    pub upgraded_m: f64,
    pub connectors: usize,
    pub connector_m: f64,
    pub lots_removed: usize,
    /// Straight connectors added this round.
    pub links: Vec<(Pt, Pt)>,
    /// Vehicle hours per peak hour each added connector was expected to save.
    pub link_gains: Vec<f64>,
}

struct Chosen {
    change: Change,
    ratio: f64,
    gain: f64,
}

/// Run `p.rounds` rounds of growth. Returns the grown town and one log line per round.
pub fn grow(
    town: &Town,
    p: &GrowParams,
    proposer: &dyn Proposer,
    scorer: &dyn Scorer,
) -> (Town, Vec<RoundLog>) {
    let mut town = town.clone();
    let mut logs = Vec::new();
    for round in 0..p.rounds {
        let scale = (1.0 + p.demand_growth).powi(round as i32);
        let ev = evaluate(&town, p, scale);
        let index = Index::new(&town, &ev.net);
        let ctx = Context {
            town: &town,
            net: &ev.net,
            demand: &ev.demand,
            assignment: &ev.assignment,
            skim: &ev.skim,
            index: &index,
            params: p,
        };
        let mut log = RoundLog {
            round,
            scale,
            vht: ev.assignment.vht,
            vht_free: ev.assignment.vht_free,
            gap: ev.assignment.gap,
            over_capacity_streets: 0,
            over_capacity_m: 0.0,
            upgrades: 0,
            upgraded_m: 0.0,
            connectors: 0,
            connector_m: 0.0,
            lots_removed: 0,
            links: Vec::new(),
            link_gains: Vec::new(),
        };
        for (e, &vc) in ev.net.streets.edges.iter().zip(&ev.assignment.edge_vc) {
            if vc > 1.0 {
                log.over_capacity_streets += 1;
                log.over_capacity_m += e.len;
            }
        }
        let mut ups: Vec<Chosen> = Vec::new();
        let mut cons: Vec<Chosen> = Vec::new();
        for c in proposer.propose(&ctx) {
            let Some(s) = scorer.score(&ctx, &c) else {
                continue;
            };
            if !(s.gain.is_finite() && s.cost > 0.0) || s.gain <= 0.0 {
                continue;
            }
            let ratio = s.gain / s.cost;
            match &c {
                Change::Upgrade { .. } => ups.push(Chosen {
                    change: c,
                    ratio,
                    gain: s.gain,
                }),
                Change::Connector { .. } => {
                    if s.gain >= p.min_gain && ratio * 1000.0 >= p.min_gain_per_km {
                        cons.push(Chosen {
                            change: c,
                            ratio,
                            gain: s.gain,
                        });
                    }
                }
            }
        }
        let by_ratio = |a: &Chosen, b: &Chosen| b.ratio.total_cmp(&a.ratio);
        ups.sort_by(by_ratio);
        cons.sort_by(by_ratio);
        let mut chosen: Vec<Change> = Vec::new();
        for u in ups {
            let Change::Upgrade { street, .. } = &u.change else {
                continue;
            };
            let len = ev.net.streets.edges[*street as usize].len;
            if log.upgraded_m + len <= p.upgrade_budget_m || log.upgrades == 0 {
                log.upgraded_m += len;
                log.upgrades += 1;
                chosen.push(u.change);
            }
        }
        let mut taken: Vec<(Pt, Pt)> = Vec::new();
        let mut taken_ends: Vec<Pt> = Vec::new();
        for c in cons {
            if log.connectors >= p.max_connectors {
                break;
            }
            let Change::Connector { pa, pb, .. } = &c.change else {
                continue;
            };
            let clash = taken.iter().any(|(a, b)| seg_dist(*pa, *pb, *a, *b) < 40.0)
                || taken_ends
                    .iter()
                    .any(|e| e.dist(*pa) < 40.0 || e.dist(*pb) < 40.0);
            if clash {
                continue;
            }
            taken.push((*pa, *pb));
            taken_ends.push(*pa);
            taken_ends.push(*pb);
            log.connectors += 1;
            log.connector_m += pa.dist(*pb);
            log.links.push((*pa, *pb));
            log.link_gains.push(c.gain);
            chosen.push(c.change);
        }
        log.lots_removed = apply(&mut town, &ev.net, &chosen);
        logs.push(log);
    }
    (town, logs)
}

fn seg_dist(a: Pt, b: Pt, c: Pt, d: Pt) -> f64 {
    if seg_intersect(a, b, c, d).is_some() {
        return 0.0;
    }
    dist_pt_seg(a, c, d)
        .min(dist_pt_seg(b, c, d))
        .min(dist_pt_seg(c, a, b))
        .min(dist_pt_seg(d, a, b))
}

/// Apply changes to the town: drivable roads are rewritten from the street graph (one road per
/// street, split where a connector lands), upgraded streets change class, lots in the way go.
/// Returns the number of lots removed.
fn apply(town: &mut Town, net: &Network, changes: &[Change]) -> usize {
    let streets = &net.streets.edges;
    let mut class: Vec<RoadClass> = streets.iter().map(|e| e.class).collect();
    // inserts per street: (segment, point)
    let mut inserts: Vec<Vec<(u32, Pt)>> = vec![Vec::new(); streets.len()];
    let mut new_roads: Vec<Road> = Vec::new();
    let mut gone: BTreeSet<u32> = BTreeSet::new();
    let mut bulb_spots: Vec<Pt> = Vec::new();
    for c in changes {
        match c {
            Change::Upgrade {
                street,
                to,
                lots_removed,
            } => {
                class[*street as usize] = *to;
                gone.extend(lots_removed.iter().copied());
            }
            Change::Connector {
                a,
                b,
                pa,
                pb,
                class: cl,
                lots_removed,
            } => {
                for (end, p) in [(a, *pa), (b, *pb)] {
                    match end {
                        End::Street { street, seg, .. } => {
                            inserts[*street as usize].push((*seg, p));
                        }
                        End::Node(_) => bulb_spots.push(p),
                    }
                }
                new_roads.push(Road {
                    class: *cl,
                    points: vec![*pa, *pb],
                });
                gone.extend(lots_removed.iter().copied());
            }
        }
    }
    let mut roads: Vec<Road> = Vec::with_capacity(town.roads.len() + new_roads.len());
    for (i, e) in streets.iter().enumerate() {
        let mut pts = e.pts.clone();
        let ins = &mut inserts[i];
        // later segments first so earlier indices stay valid; within a segment far points first
        ins.sort_by(|x, y| {
            y.0.cmp(&x.0).then_with(|| {
                let d0 = pts[x.0 as usize];
                d0.dist(y.1).total_cmp(&d0.dist(x.1))
            })
        });
        for (seg, p) in ins.iter() {
            pts.insert(*seg as usize + 1, *p);
        }
        roads.push(Road {
            class: class[i],
            points: pts,
        });
    }
    // roads outside the drivable network stay as they were
    for r in &town.roads {
        if matches!(r.class, RoadClass::Alley | RoadClass::Rail) {
            roads.push(r.clone());
        }
    }
    roads.extend(new_roads);
    town.roads = roads;
    // a dead end that now continues is no longer a turning bulb
    town.cul_de_sacs
        .retain(|c| !bulb_spots.iter().any(|s| s.dist(c.pos) < 12.0));
    let n = gone.len();
    let mut idx = 0u32;
    town.lots.retain(|_| {
        let keep = !gone.contains(&idx);
        idx += 1;
        keep
    });
    n
}
