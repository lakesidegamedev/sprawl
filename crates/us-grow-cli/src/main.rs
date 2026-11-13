//! `us-grow-cli run --preset grid|winding --seed N [--rounds 20] --out DIR [--corpus FILE]`
//! `us-grow-cli timing --preset grid|winding --seed N`

mod draw;
#[path = "../../us-gen-cli/src/font.rs"]
mod font;

use std::path::PathBuf;
use std::time::Instant;
use us_gen::{Preset, Town, generate};
use us_grow::{GrowParams, RuleProposer, TrafficScorer, evaluate, grow};
use us_metrics::metrics::NAMES;
use us_metrics::score::{CorpusStats, score};

fn usage() -> ! {
    eprintln!(
        "usage:\n  us-grow-cli run --preset grid|winding --seed N [--rounds 20] --out DIR [--corpus FILE]\n  us-grow-cli timing --preset grid|winding --seed N"
    );
    std::process::exit(2);
}

struct Args {
    preset: Preset,
    seed: u64,
    rounds: usize,
    out: Option<PathBuf>,
    corpus: Option<PathBuf>,
}

fn parse(args: &[String]) -> Args {
    let mut a = Args {
        preset: Preset::Grid,
        seed: 1,
        rounds: 20,
        out: None,
        corpus: None,
    };
    let mut it = args.iter();
    while let Some(k) = it.next() {
        match k.as_str() {
            "--preset" => {
                a.preset = match it.next().map(|s| s.as_str()) {
                    Some("grid") => Preset::Grid,
                    Some("winding") => Preset::Winding,
                    _ => usage(),
                }
            }
            "--seed" => {
                a.seed = it
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            "--rounds" => {
                a.rounds = it
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            "--out" => a.out = it.next().map(PathBuf::from),
            "--corpus" => a.corpus = it.next().map(PathBuf::from),
            _ => usage(),
        }
    }
    a
}

type Realism = (f64, Vec<(&'static str, f64, f64)>);

fn realism(t: &Town, stats: &Option<CorpusStats>) -> Option<Realism> {
    let st = stats.as_ref()?;
    let m = us_metrics::town_metrics(t);
    let s = score(&m, st);
    Some((
        s.distance,
        s.per_metric
            .iter()
            .map(|r| (r.name, r.value, r.z))
            .collect(),
    ))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else { usage() };
    let a = parse(&args[1..]);
    let town = generate(a.preset, a.seed);
    let p = GrowParams {
        rounds: a.rounds,
        ..GrowParams::default()
    };
    match cmd.as_str() {
        "timing" => {
            let ev = evaluate(&town, &p, 1.0);
            for scale in [1.0, 2.1] {
                let dem = ev.demand.scaled(scale);
                let mut ms = Vec::new();
                let mut last = None;
                for _ in 0..9 {
                    let t = Instant::now();
                    last = Some(us_traffic::assign(&ev.net, &dem, &p.assign));
                    ms.push(t.elapsed().as_secs_f64() * 1000.0);
                }
                ms.sort_by(|x, y| x.total_cmp(y));
                let r = last.expect("ran");
                println!(
                    "{} seed {} demand x{scale}: {} nodes, {} streets, {} zones, {:.0} trips/h, assignment median {:.1} ms (min {:.1}, max {:.1}), {} iterations, gap {:.4}, {} streets over capacity",
                    a.preset.name(),
                    a.seed,
                    ev.net.n_nodes(),
                    ev.net.streets.edges.len(),
                    ev.demand.zones.len(),
                    dem.total,
                    ms[4],
                    ms[0],
                    ms[8],
                    r.iterations,
                    r.gap,
                    r.edge_vc.iter().filter(|&&v| v > 1.0).count()
                );
            }
        }
        "run" => {
            let Some(out) = a.out.clone() else { usage() };
            std::fs::create_dir_all(&out).expect("out dir");
            let stats = a.corpus.as_ref().map(|p| {
                let text = std::fs::read_to_string(p).expect("corpus file");
                CorpusStats::from_json(&serde_json::from_str(&text).expect("json")).expect("shape")
            });
            let name = format!("{}-{}", a.preset.name(), a.seed);
            let t0 = Instant::now();
            let (after, logs) = grow(&town, &p, &RuleProposer, &TrafficScorer);
            eprintln!(
                "grew {} rounds in {:.2}s",
                logs.len(),
                t0.elapsed().as_secs_f64()
            );
            let final_scale = (1.0 + p.demand_growth).powi(a.rounds as i32);
            let ev0 = evaluate(&town, &p, 1.0);
            let ev0n = evaluate(&town, &p, final_scale);
            let ev1 = evaluate(&after, &p, final_scale);
            let all_links: Vec<_> = logs.iter().flat_map(|l| l.links.clone()).collect();
            let shots = [
                ("before", &town, &ev0, "year 0 traffic", vec![]),
                (
                    "before-y20",
                    &town,
                    &ev0n,
                    "year 20 traffic on the unchanged town",
                    vec![],
                ),
                (
                    "after",
                    &after,
                    &ev1,
                    "year 20 traffic after 20 rounds of growth",
                    all_links,
                ),
            ];
            for (tag, t, ev, label, links) in shots {
                let over = ev.assignment.edge_vc.iter().filter(|&&v| v > 1.0).count();
                let sub = format!(
                    "{label}: {:.0} trips/h, {:.0} vehicle hours, {} streets over capacity",
                    ev.demand.total, ev.assignment.vht, over
                );
                let cap = format!("{} town, seed {} ({tag})", a.preset.name(), a.seed);
                let pm = draw::traffic_png(t, ev, &cap, &sub, &links);
                let path = out.join(format!("{name}-{tag}.png"));
                pm.save_png(&path).expect("png");
                println!("wrote {}", path.display());
            }
            println!("round  scale   vht  free-vht  over(st) over(m) up(m)  links(m)  lots");
            for l in &logs {
                println!(
                    "{:>5} {:>6.2} {:>6.0} {:>8.0} {:>8} {:>7.0} {:>6.0} {:>3}({:>4.0}) {:>4}",
                    l.round,
                    l.scale,
                    l.vht,
                    l.vht_free,
                    l.over_capacity_streets,
                    l.over_capacity_m,
                    l.upgraded_m,
                    l.connectors,
                    l.connector_m,
                    l.lots_removed
                );
            }
            let gains: Vec<String> = logs
                .iter()
                .flat_map(|l| l.link_gains.iter().map(|g| format!("{g:.1}")))
                .collect();
            println!(
                "connector gains (vehicle hours per peak hour): {}",
                gains.join(" ")
            );
            println!(
                "lots {} -> {}; roads {} -> {}",
                town.lots.len(),
                after.lots.len(),
                town.roads.len(),
                after.roads.len()
            );
            let (b, e) = (realism(&town, &stats), realism(&after, &stats));
            if let (Some(b), Some(e)) = (b, e) {
                println!("realism distance: before {:.3} after {:.3}", b.0, e.0);
                println!(
                    "{:<24}{:>12}{:>8}{:>12}{:>8}",
                    "metric", "before", "z", "after", "z"
                );
                for (i, n) in NAMES.iter().enumerate() {
                    println!(
                        "{:<24}{:>12.3}{:>8.2}{:>12.3}{:>8.2}",
                        n, b.1[i].1, b.1[i].2, e.1[i].1, e.1[i].2
                    );
                }
            }
        }
        _ => usage(),
    }
}
