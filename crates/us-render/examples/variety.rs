//! `cargo run --release -p us-render --example variety -- [samples] [region]`
//!
//! Counts distinct building looks for every lot kind and level. A look is the exact geometry and
//! colours of a built model (hashed), so two models that differ in any shape, colour or detail
//! count as two looks. Each (kind, level) is built from `samples` seeds (default 4000); a count
//! equal to `samples` means the sample did not saturate and the true number is higher. Also prints
//! the triangle count of the average model.

use std::collections::HashSet;
use us_gen::LotKind;
use us_render::kit::ALL_KINDS;
use us_render::{MAX_LEVEL, Obj, Region, Rng, build_lot_level_in};

/// A look is the set of colours used, the number of faces, the footprint size to the nearest two
/// metres and the height to the nearest metre. Small shifts of where a rooftop unit or a tree
/// stands do not make a new look.
fn fingerprint(o: &Obj) -> u64 {
    let mut cols: Vec<[u8; 3]> = o.polys.iter().map(|p| p.color).collect();
    cols.sort_unstable();
    cols.dedup();
    let fp = o.footprint();
    let top = o
        .polys
        .iter()
        .flat_map(|p| p.pts.iter().map(|v| v[2]))
        .fold(0.0f32, f32::max);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut put = |v: i64| {
        h ^= v as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for c in cols {
        for k in c {
            put(k as i64);
        }
    }
    put(o.polys.len() as i64);
    for k in fp {
        put((k / 2.0).round() as i64);
    }
    put(top.round() as i64);
    h
}

fn tris(o: &Obj) -> usize {
    o.polys.iter().map(|p| p.pts.len().saturating_sub(2)).sum()
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(4000);
    println!("| kind | L1 | L2 | L3 | L4 | L5 | total | avg tris |");
    println!("|---|---|---|---|---|---|---|---|");
    let (mut grand, mut grand_tris, mut cells) = (0usize, 0usize, 0usize);
    for kind in ALL_KINDS {
        let mut row = Vec::new();
        let mut total = 0;
        let mut t = 0usize;
        for level in 1..=MAX_LEVEL {
            let mut seen = HashSet::new();
            // the samples are split across the regions, so the count is of looks anywhere
            for (r, region) in Region::ALL.iter().enumerate() {
                for i in 0..n / Region::ALL.len() {
                    let mut rng = Rng::new(0x7a11_0000 + r as u64, i as u64 * 7919 + 13);
                    let o = build_lot_level_in(*region, kind, level, &mut rng);
                    t += tris(&o);
                    seen.insert(fingerprint(&o));
                }
            }
            total += seen.len();
            row.push(seen.len());
        }
        let avg = t / (n * MAX_LEVEL as usize);
        grand += total;
        grand_tris += avg;
        cells += 1;
        let name: &str = lot_name(kind);
        println!(
            "| {name} | {} | {total} | {avg} |",
            row.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }
    println!("| all | | | | | | {grand} | {} |", grand_tris / cells);
}

fn lot_name(k: LotKind) -> &'static str {
    k.name()
}
