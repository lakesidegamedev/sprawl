//! `us-metrics corpus --dir D --out corpus-stats.json`
//! `us-metrics tune --preset P [--corpus FILE] [--rounds N]` searches generator settings.
//! `us-metrics score --preset grid|winding|sunbelt|crossroads|lake --seed N [--seeds K] [--corpus corpus-stats.json]`

use std::path::PathBuf;
use us_gen::{Preset, Settings, generate, generate_with};
use us_metrics::corpus;
use us_metrics::metrics::{Metrics, NAMES};
use us_metrics::score::{CorpusStats, score};

fn usage() -> ! {
    eprintln!(
        "usage:\n  us-metrics corpus --dir DIR --out FILE [--towns-out FILE]\n  us-metrics score --preset grid|winding|sunbelt|crossroads|lake --seed N [--seeds K] [--corpus FILE]"
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
                preset = Some(
                    it.next()
                        .and_then(|n| Preset::from_name(n))
                        .unwrap_or_else(|| usage()),
                )
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

struct Lcg(u64);
impl Lcg {
    fn f(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.f()
    }
}

fn mean_distance(preset: Preset, st: &Settings, seeds: &[u64], stats: &CorpusStats) -> f64 {
    let handles: Vec<_> = seeds
        .iter()
        .map(|&s| {
            let st = st.clone();
            let stats = stats.clone();
            std::thread::spawn(move || {
                let town = generate_with(preset, s, &st);
                score(&us_metrics::town_metrics(&town), &stats).distance
            })
        })
        .collect();
    let v: Vec<f64> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    v.iter().sum::<f64>() / v.len() as f64
}

/// Offline random search followed by local refinement over the generator settings.
fn run_tune(args: &[String]) {
    let (mut preset, mut corpus_path, mut rounds) = (None, None, 40usize);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--preset" => preset = it.next().and_then(|n| Preset::from_name(n)),
            "--corpus" => corpus_path = it.next().map(PathBuf::from),
            "--rounds" => rounds = it.next().and_then(|s| s.parse().ok()).unwrap_or(rounds),
            _ => usage(),
        }
    }
    let Some(preset) = preset else { usage() };
    let path = corpus_path.unwrap_or_else(|| PathBuf::from("corpus-stats.json"));
    let stats = CorpusStats::from_json(
        &serde_json::from_str(&std::fs::read_to_string(&path).expect("corpus file")).expect("json"),
    )
    .expect("corpus shape");
    let train: Vec<u64> = (1..=4).collect();
    let test: Vec<u64> = (101..=108).collect();
    let base = preset.settings();
    println!(
        "start: train {:.3} holdout {:.3}",
        mean_distance(preset, &base, &train, &stats),
        mean_distance(preset, &base, &test, &stats)
    );
    let mut rng = Lcg(0x5eed_0001 ^ preset as u64);
    let grid_like = matches!(preset, Preset::Grid | Preset::Crossroads | Preset::LakeTown);
    let sample = |rng: &mut Lcg, near: Option<&Settings>| -> Settings {
        let pick = |rng: &mut Lcg, lo: f64, hi: f64, cur: Option<f64>| match cur {
            Some(c) => (c + rng.range(-0.2, 0.2) * (hi - lo)).clamp(lo, hi),
            None => rng.range(lo, hi),
        };
        let n = near;
        let mut s = base.clone();
        if preset == Preset::Grid {
            s.arterial_spacing = pick(rng, 805.0, 1400.0, n.map(|x| x.arterial_spacing));
        }
        s.block_scale = pick(rng, 0.9, 2.2, n.map(|x| x.block_scale));
        if grid_like {
            s.core_depth = pick(rng, 80.0, 140.0, n.map(|x| x.core_depth));
            s.core_len = pick(rng, 100.0, 200.0, n.map(|x| x.core_len));
            s.core_alley = pick(rng, 0.0, 1.0, n.map(|x| x.core_alley));
            s.cat_alley = pick(rng, 0.0, 1.0, n.map(|x| x.cat_alley));
            s.jog_prob = pick(rng, 0.0, 1.0, n.map(|x| x.jog_prob));
            s.jog = pick(rng, 18.0, 40.0, n.map(|x| x.jog));
        }
        s.pod_share = pick(rng, 0.2, 1.0, n.map(|x| x.pod_share));
        s.collector_len = pick(rng, 300.0, 1000.0, n.map(|x| x.collector_len));
        s.street_spacing = pick(rng, 0.9, 2.0, n.map(|x| x.street_spacing));
        s.cul_share = pick(rng, 0.3, 1.0, n.map(|x| x.cul_share));
        s
    };
    let mut best = base.clone();
    let mut best_d = mean_distance(preset, &best, &train, &stats);
    for r in 0..rounds * 2 {
        let cand = if r < rounds {
            sample(&mut rng, None)
        } else {
            sample(&mut rng, Some(&best))
        };
        let d = mean_distance(preset, &cand, &train, &stats);
        if d < best_d {
            best_d = d;
            best = cand;
            println!("round {r}: train {d:.3}");
        }
    }
    println!(
        "best: train {:.3} holdout {:.3}\n{:#?}",
        best_d,
        mean_distance(preset, &best, &test, &stats),
        best
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(|s| s.as_str()) {
        Some("corpus") => run_corpus(&args[1..]),
        Some("score") => run_score(&args[1..]),
        Some("tune") => run_tune(&args[1..]),
        _ => usage(),
    }
}
