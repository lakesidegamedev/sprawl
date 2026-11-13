//! `cargo run --release -p us-traffic --example bench -- grid 1` prints demand, timing and load.
use std::time::Instant;
use us_gen::{Preset, generate};
use us_traffic::{AssignParams, Demand, DemandParams, Network, assign};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let preset = if a.get(1).map(|s| s.as_str()) == Some("winding") {
        Preset::Winding
    } else {
        Preset::Grid
    };
    let seed: u64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let threads: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let town = generate(preset, seed);
    let t = Instant::now();
    let net = Network::from_town(&town);
    println!(
        "network: {} nodes {} streets in {:?}",
        net.n_nodes(),
        net.streets.edges.len(),
        t.elapsed()
    );
    let t = Instant::now();
    let dem = Demand::from_town(&town, &net, &DemandParams::default());
    println!(
        "demand: {} zones {:.0} trips in {:?}",
        dem.zones.len(),
        dem.total,
        t.elapsed()
    );
    let p = AssignParams {
        threads,
        ..AssignParams::default()
    };
    for _ in 0..3 {
        let t = Instant::now();
        let r = assign(&net, &dem, &p);
        let el = t.elapsed();
        let over: usize = r.edge_vc.iter().filter(|&&v| v > 1.0).count();
        let over_len: f64 = net
            .streets
            .edges
            .iter()
            .zip(&r.edge_vc)
            .filter(|(_, v)| **v > 1.0)
            .map(|(e, _)| e.len)
            .sum();
        println!(
            "assign: {:?} iters {} gap {:.4} vht {:.0} free {:.0} over-capacity streets {} ({:.0} m) max v/c {:.2}",
            el,
            r.iterations,
            r.gap,
            r.vht,
            r.vht_free,
            over,
            over_len,
            r.edge_vc.iter().cloned().fold(0.0, f64::max)
        );
    }
}
