//! `us-gen-cli --preset grid|winding --seed N --png out.png --stats out.json`

mod font;
mod render;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use us_gen::{Preset, RoadClass, Town, generate};

fn usage() -> ! {
    eprintln!(
        "usage: us-gen-cli --preset grid|winding --seed N [--png out.png] [--stats out.json]"
    );
    std::process::exit(2);
}

fn stats_json(t: &Town) -> String {
    let mut road_count: BTreeMap<&str, usize> = BTreeMap::new();
    let mut road_len: BTreeMap<&str, f64> = BTreeMap::new();
    for r in &t.roads {
        *road_count.entry(r.class.name()).or_default() += 1;
        let len: f64 = r.points.windows(2).map(|w| w[0].dist(w[1])).sum();
        *road_len.entry(r.class.name()).or_default() += len;
    }
    let mut lot_kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for l in &t.lots {
        *lot_kinds.entry(l.kind.name()).or_default() += 1;
    }
    let mut patterns: BTreeMap<&str, usize> = BTreeMap::new();
    for a in &t.additions {
        *patterns.entry(a.pattern.name()).or_default() += 1;
    }
    let obj_u = |m: &BTreeMap<&str, usize>| {
        m.iter()
            .map(|(k, v)| format!("\"{k}\": {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let obj_f = |m: &BTreeMap<&str, f64>| {
        m.iter()
            .map(|(k, v)| format!("\"{k}\": {:.1}", v))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut s = String::new();
    let _ = writeln!(s, "{{");
    let _ = writeln!(s, "  \"preset\": \"{}\",", t.preset.name());
    let _ = writeln!(s, "  \"seed\": {},", t.seed);
    let _ = writeln!(s, "  \"fingerprint\": \"{:016x}\",", t.fingerprint());
    let _ = writeln!(s, "  \"roads\": {{ {} }},", obj_u(&road_count));
    let _ = writeln!(s, "  \"road_length_m\": {{ {} }},", obj_f(&road_len));
    let _ = writeln!(s, "  \"additions\": {},", t.additions.len());
    let _ = writeln!(s, "  \"patterns\": {{ {} }},", obj_u(&patterns));
    let _ = writeln!(s, "  \"cul_de_sacs\": {},", t.cul_de_sacs.len());
    let _ = writeln!(s, "  \"lots\": {},", t.lots.len());
    let _ = writeln!(s, "  \"lot_kinds\": {{ {} }},", obj_u(&lot_kinds));
    let _ = writeln!(s, "  \"hall\": [{:.1}, {:.1}]", t.hall.x, t.hall.y);
    s.push('}');
    s.push('\n');
    s
}

fn main() {
    let mut preset = None;
    let mut seed = None;
    let mut png = None;
    let mut stats = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--preset" => {
                preset = Some(match args.next().as_deref() {
                    Some("grid") => Preset::Grid,
                    Some("winding") => Preset::Winding,
                    _ => usage(),
                })
            }
            "--seed" => seed = args.next().and_then(|s| s.parse::<u64>().ok()),
            "--png" => png = args.next(),
            "--stats" => stats = args.next(),
            _ => usage(),
        }
    }
    let (Some(preset), Some(seed)) = (preset, seed) else {
        usage()
    };
    let start = std::time::Instant::now();
    let town = generate(preset, seed);
    eprintln!("generated in {:.2}s", start.elapsed().as_secs_f64());
    if let Some(path) = png {
        let caption = match preset {
            Preset::Grid => format!("Grid town, seed {seed}"),
            Preset::Winding => format!("Winding town, seed {seed}"),
        };
        let sub = format!(
            "{} additions, {} lots, {} cul-de-sacs, {} roads",
            town.additions.len(),
            town.lots.len(),
            town.cul_de_sacs.len(),
            town.roads.len()
        );
        let pm = render::plan_png(&town, &caption, &sub);
        if let Err(e) = pm.save_png(&path) {
            eprintln!("could not write {path}: {e}");
            std::process::exit(1);
        }
    }
    if let Some(path) = stats
        && let Err(e) = std::fs::write(&path, stats_json(&town))
    {
        eprintln!("could not write {path}: {e}");
        std::process::exit(1);
    }
    let _ = RoadClass::Highway;
}
