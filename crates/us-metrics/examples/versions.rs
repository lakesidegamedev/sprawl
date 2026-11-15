//! Compare road statistics of generator versions: `cargo run --release -p us-metrics --example versions -- 2 3`.
use us_gen::{Preset, generate_version};
use us_metrics::metrics::NAMES;
use us_metrics::town_metrics;

fn main() {
    let versions: Vec<u32> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let seeds = 6u64;
    for p in Preset::ALL {
        for &v in &versions {
            let mut sum = vec![0.0f64; NAMES.len()];
            let (mut rings, mut ramps) = (0, 0);
            for s in 1..=seeds {
                let t = generate_version(p, s, v);
                rings += t
                    .roads
                    .iter()
                    .filter(|r| {
                        r.class == us_gen::RoadClass::Collector
                            && r.points.len() == 33
                            && r.points[0].dist(*r.points.last().unwrap()) < 1e-6
                    })
                    .count();
                ramps += t
                    .roads
                    .iter()
                    .filter(|r| r.class == us_gen::RoadClass::Collector && r.points.len() == 15)
                    .count();
                let m = town_metrics(&t);
                for (k, n) in NAMES.iter().enumerate() {
                    sum[k] += m.get(n);
                }
            }
            let cols: Vec<String> = NAMES
                .iter()
                .enumerate()
                .filter(|(_, n)| {
                    matches!(
                        **n,
                        "share_3way"
                            | "share_4way"
                            | "share_dead_end"
                            | "circuity"
                            | "intersection_density"
                            | "street_density"
                            | "share_collector"
                            | "giant_component_share"
                    )
                })
                .map(|(k, n)| format!("{n}={:.3}", sum[k] / seeds as f64))
                .collect();
            println!(
                "{:10} v{v} rings={rings} ramps={ramps} {}",
                p.name(),
                cols.join(" ")
            );
        }
    }
}
