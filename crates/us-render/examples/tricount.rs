//! `cargo run --release -p us-render --example tricount -- [preset] [seed]`
//!
//! Triangles in the meshes of a grown-up town (every lot at the level its density reaches),
//! and the time to build the scene.

use std::time::Instant;
use us_gen::{LotKind, Preset, generate};
use us_render::{LotState, Scene};

fn mature_level(kind: LotKind, density: f64) -> u8 {
    let top = match kind {
        LotKind::House => 3,
        LotKind::Row | LotKind::Workshop => 4,
        LotKind::Apartments | LotKind::Office => 5,
        LotKind::Shop | LotKind::Strip | LotKind::BigBox | LotKind::Warehouse => 3,
        LotKind::Gas | LotKind::FastFood => 2,
        _ => 1,
    };
    (1 + (density * 4.4) as u8).min(top)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let preset = args
        .next()
        .and_then(|n| Preset::from_name(&n))
        .unwrap_or(Preset::Grid);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(3);
    let town = generate(preset, seed);
    let mut scene = Scene::from_town(&town);
    let changes: Vec<(usize, Option<u8>, LotState)> = town
        .lots
        .iter()
        .enumerate()
        .map(|(i, l)| (i, Some(mature_level(l.kind, l.density)), LotState::Normal))
        .collect();
    let t = Instant::now();
    scene.set_lots(&changes);
    let dt = t.elapsed();
    let full: usize = scene.chunks.iter().map(|c| c.mesh.triangles()).sum();
    let far: usize = scene
        .chunks
        .iter()
        .filter_map(|c| c.far.as_ref())
        .map(|m| m.triangles())
        .sum();
    println!(
        "full tris {full}, far tris {far}, set_lots {:.2}s",
        dt.as_secs_f32()
    );
}
