//! `cargo run --release -p us-gen --example density`
//!
//! Prints built lots per square kilometre of developed land and the share of that land covered by
//! building footprints, for every preset (seeds 1 to 3). Developed land is every addition that is
//! not farm or park. Footprints are estimated as 70 per cent of each lot's collision disk.

use us_gen::{LotKind, Preset, Zone, generate};

fn area(r: &[us_gen::Pt]) -> f64 {
    let mut s = 0.0;
    for i in 0..r.len() {
        let (a, b) = (r[i], r[(i + 1) % r.len()]);
        s += a.x * b.y - b.x * a.y;
    }
    s.abs() / 2.0
}

fn main() {
    println!("preset     seed  land_km2  buildings  per_km2  coverage%");
    for p in Preset::ALL {
        let (mut tl, mut tb, mut tc) = (0.0, 0.0, 0.0);
        for seed in 1..=3 {
            let t = generate(p, seed);
            let land: f64 = t
                .additions
                .iter()
                .filter(|a| !matches!(a.zone, Zone::Farm | Zone::Park))
                .map(|a| area(&a.polygon))
                .sum();
            let built: Vec<_> = t
                .lots
                .iter()
                .filter(|l| !matches!(l.kind, LotKind::Farm | LotKind::Park))
                .filter(|l| l.zone != Zone::Farm)
                .collect();
            let foot: f64 = built
                .iter()
                .map(|l| 0.7 * std::f64::consts::PI * l.radius * l.radius)
                .sum();
            println!(
                "{:<10} {:>4}  {:>8.2}  {:>9}  {:>7.0}  {:>8.1}",
                p.name(),
                seed,
                land / 1e6,
                built.len(),
                built.len() as f64 / (land / 1e6),
                100.0 * foot / land
            );
            tl += land;
            tb += built.len() as f64;
            tc += foot;
        }
        println!(
            "{:<10}  all  {:>8.2}  {:>9.0}  {:>7.0}  {:>8.1}\n",
            p.name(),
            tl / 1e6,
            tb,
            tb / (tl / 1e6),
            100.0 * tc / tl
        );
    }
}
