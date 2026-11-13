//! Assignment checks that do not trust the assignment's own bookkeeping.

use us_gen::{Preset, RoadClass, generate, pt};
use us_metrics::graph::{Street, StreetGraph};
use us_traffic::{AssignParams, Demand, DemandParams, Network, Zone, assign, class_capacity};

fn town_case(preset: Preset, seed: u64) -> (Network, Demand) {
    let t = generate(preset, seed);
    let net = Network::from_town(&t);
    let dem = Demand::from_town(&t, &net, &DemandParams::default());
    (net, dem)
}

#[test]
fn every_trip_is_routed_and_flow_is_conserved_at_every_node() {
    for (preset, seed) in [(Preset::Grid, 1), (Preset::Winding, 2)] {
        let (net, dem) = town_case(preset, seed);
        let n = dem.zones.len();
        // all produced trips are placed somewhere
        let prod: f64 = dem.zones.iter().map(|z| z.prod).sum();
        assert!(
            (dem.total - prod).abs() < 1e-6 * prod,
            "{} vs {}",
            dem.total,
            prod
        );
        assert!(dem.od.iter().sum::<f64>() > 0.0);
        let r = assign(&net, &dem, &AssignParams::default());
        // net outflow at a node must equal trips starting there minus trips ending there
        let mut balance = vec![0.0f64; net.n_nodes()];
        for (i, a) in net.arcs.iter().enumerate() {
            balance[a.from as usize] += r.arc_flow[i];
            balance[a.to as usize] -= r.arc_flow[i];
        }
        let mut want = vec![0.0f64; net.n_nodes()];
        for i in 0..n {
            for j in 0..n {
                let q = dem.od[i * n + j];
                want[dem.zones[i].node as usize] += q;
                want[dem.zones[j].node as usize] -= q;
            }
        }
        for node in 0..net.n_nodes() {
            assert!(
                (balance[node] - want[node]).abs() < 1e-6 * dem.total.max(1.0),
                "{} node {node}: flow balance {} want {}",
                preset.name(),
                balance[node],
                want[node]
            );
        }
    }
}

#[test]
fn equilibrium_gap_is_small_when_the_town_is_busy() {
    let (net, dem) = town_case(Preset::Winding, 2);
    // push the town well past capacity so the equilibrium is not trivial
    let busy = dem.scaled(2.5);
    let r = assign(&net, &busy, &AssignParams::default());
    assert!(r.edge_vc.iter().any(|&v| v > 1.0), "test needs congestion");
    assert!(
        r.gap < 0.02,
        "gap {} after {} iterations",
        r.gap,
        r.iterations
    );
    assert!(r.vht > r.vht_free);
}

#[test]
fn two_parallel_roads_reach_wardrop_equilibrium() {
    // one origin, one destination, a slow local road and a longer arterial. At equilibrium every
    // used route takes the same time, which can be checked from the flows alone.
    let a = pt(0.0, 0.0);
    let b = pt(1000.0, 0.0);
    let mid = pt(500.0, 400.0);
    let streets = StreetGraph {
        nodes: vec![a, b],
        edges: vec![
            Street {
                a: 0,
                b: 1,
                class: RoadClass::Local,
                len: 1000.0,
                pts: vec![a, b],
            },
            Street {
                a: 0,
                b: 1,
                class: RoadClass::Arterial,
                len: 1280.0,
                pts: vec![a, mid, b],
            },
        ],
    };
    let net = Network::from_streets(streets);
    let zones = vec![
        Zone {
            node: 0,
            prod: 2400.0,
            attr: 0.0,
            centroid: a,
        },
        Zone {
            node: 1,
            prod: 0.0,
            attr: 100.0,
            centroid: b,
        },
    ];
    let dem = Demand::gravity(&net, zones, &DemandParams::default());
    assert!((dem.total - 2400.0).abs() < 1e-9);
    let p = AssignParams {
        max_iter: 200,
        gap_tol: 1e-5,
        ..AssignParams::default()
    };
    let r = assign(&net, &dem, &p);
    let (fl, fa) = (r.arc_flow[0], r.arc_flow[2]);
    assert!((fl + fa - 2400.0).abs() < 1e-6);
    assert!(fl > 1.0 && fa > 1.0, "both roads used: {fl} {fa}");
    let (tl, ta) = (r.arc_time[0], r.arc_time[2]);
    assert!((tl - ta).abs() / ta < 0.01, "times {tl} vs {ta}");
    // recompute the times from the flows by hand
    let hand = |len: f64, speed: f64, delay: f64, cap: f64, x: f64| {
        (len / speed + delay) * (1.0 + 0.15 * (x / cap).powi(4))
    };
    let tl2 = hand(
        1000.0,
        35.0 / 3.6,
        3.0,
        class_capacity(RoadClass::Local),
        fl,
    );
    assert!((tl - tl2).abs() < 1e-6);
}

#[test]
fn same_result_every_time_and_for_any_thread_count() {
    let (net, dem) = town_case(Preset::Grid, 1);
    let busy = dem.scaled(2.0);
    let one = assign(&net, &busy, &AssignParams::default());
    let again = assign(&net, &busy, &AssignParams::default());
    let four = assign(
        &net,
        &busy,
        &AssignParams {
            threads: 4,
            ..AssignParams::default()
        },
    );
    assert_eq!(one.arc_flow, again.arc_flow);
    assert_eq!(one.arc_flow, four.arc_flow);
}

#[test]
fn assignment_is_fast_enough_for_a_full_town() {
    let (net, dem) = town_case(Preset::Grid, 1);
    let _ = assign(&net, &dem, &AssignParams::default());
    let t = std::time::Instant::now();
    let r = assign(&net, &dem.scaled(2.0), &AssignParams::default());
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    // the target in release mode is well under 100 ms; allow slack for debug builds and busy hosts
    let limit = if cfg!(debug_assertions) {
        3000.0
    } else {
        150.0
    };
    assert!(ms < limit, "{ms:.1} ms for {} iterations", r.iterations);
}
