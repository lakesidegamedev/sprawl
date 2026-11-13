//! `us-metrics corpus --dir D --out corpus-stats.json`
//! `us-metrics score --preset grid|winding --seed N [--seeds K] [--corpus corpus-stats.json]`

use std::path::PathBuf;
use us_gen::{Preset, generate};
use us_metrics::corpus;
use us_metrics::metrics::{Metrics, NAMES};
use us_metrics::score::{CorpusStats, score};

fn usage() -> ! {
    eprintln!(
        "usage:\n  us-metrics corpus --dir DIR --out FILE [--towns-out FILE]\n  us-metrics score --preset grid|winding --seed N [--seeds K] [--corpus FILE]"
    );
    std::process::exit(2);
}

fn fmt(v: f64) -> String {
    if !v.is_finite() {
        "nan".into()
    } else if v.abs() >= 1000.0 {
        format!("{v:.0}")
    } else if v.abs() >= 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.3}")
    }
}

fn run_corpus(args: &[String]) {
    let (mut dir, mut out, mut towns_out) = (None, None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--dir" => dir = it.next().map(PathBuf::from),
            "--out" => out = it.next().map(PathBuf::from),
            "--towns-out" => towns_out = it.next().map(PathBuf::from),
            _ => usage(),
        }
    }
    let (Some(dir), Some(out)) = (dir, out) else {
        usage()
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| {
            eprintln!("{}: {e}", dir.display());
            std::process::exit(1)
        })
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".json.gz"))
        .collect();
    files.sort();
    let mut all: Vec<Metrics> = Vec::new();
    let mut rows = Vec::new();
    for f in &files {
        match corpus::load(f) {
            Ok(t) => {
                let m = corpus::metrics_of(&t);
                if m.n_blocks < 20 {
                    eprintln!("skip {} ({} blocks)", t.name, m.n_blocks);
                    continue;
                }
                eprintln!("{:<24} pop {:>7} blocks {:>5}", t.name, t.pop, m.n_blocks);
                rows.push(serde_json::json!({
                    "name": t.name, "pop": t.pop,
                    "values": NAMES.iter().zip(m.values).map(|(n, v)| (n.to_string(), serde_json::json!(v))).collect::<serde_json::Map<_, _>>()
                }));
                all.push(m);
            }
            Err(e) => eprintln!("skip: {e}"),
        }
    }
    let stats = CorpusStats::from_towns(&all);
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&stats.to_json()).unwrap() + "\n",
    )
    .expect("write stats");
    if let Some(p) = towns_out {
        std::fs::write(p, serde_json::to_string_pretty(&rows).unwrap() + "\n")
            .expect("write towns");
    }
    eprintln!("{} towns used of {} files", stats.n_towns, files.len());
}

fn run_score(args: &[String]) {
    let (mut preset, mut seed, mut seeds, mut corpus_path) = (None, 1u64, 1u64, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--preset" => {
                preset = Some(match it.next().map(|s| s.as_str()) {
                    Some("grid") => Preset::Grid,
                    Some("winding") => Preset::Winding,
                    _ => usage(),
                })
            }
            "--seed" => {
                seed = it
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            "--seeds" => {
                seeds = it
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            "--corpus" => corpus_path = it.next().map(PathBuf::from),
            _ => usage(),
        }
    }
    let Some(preset) = preset else { usage() };
    let path = corpus_path.unwrap_or_else(|| PathBuf::from("corpus-stats.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("{}: {e}", path.display());
        std::process::exit(1)
    });
    let stats = CorpusStats::from_json(&serde_json::from_str(&text).expect("corpus json"))
        .expect("corpus stats shape");
    let mut dist = Vec::new();
    for s in seed..seed + seeds {
        let town = generate(preset, s);
        let m = us_metrics::town_metrics(&town);
        let sc = score(&m, &stats);
        println!("{} seed {s}: distance {:.3}", preset.name(), sc.distance);
        if seeds == 1 {
            println!(
                "{:<24}{:>12}{:>12}{:>8}  in p05-p95",
                "metric", "value", "corpus p50", "z"
            );
            for r in &sc.per_metric {
                println!(
                    "{:<24}{:>12}{:>12}{:>8.2}  {}",
                    r.name,
                    fmt(r.value),
                    fmt(r.median),
                    r.z,
                    if r.inside_p05_p95 { "yes" } else { "NO" }
                );
            }
        }
        dist.push(sc.distance);
    }
    if seeds > 1 {
        let mean = dist.iter().sum::<f64>() / dist.len() as f64;
        println!(
            "{} mean distance over {} seeds: {:.3}",
            preset.name(),
            seeds,
            mean
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("corpus") => run_corpus(&args[1..]),
        Some("score") => run_score(&args[1..]),
        _ => usage(),
    }
}
