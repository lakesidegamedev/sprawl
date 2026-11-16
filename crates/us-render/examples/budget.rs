//! `cargo run --release -p us-render --example budget -- [seed]`
//!
//! Vertices, triangles and GPU buffer bytes of the scene of every style with each lot at the
//! level its density reaches (a town grown past its first water wall). One line per style.

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
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    println!("style seed verts tris far_tris gpu_MB");
    for preset in Preset::ALL {
        let town = generate(preset, seed);
        let mut scene = Scene::from_town(&town);
        let changes: Vec<(usize, Option<u8>, LotState)> = town
            .lots
            .iter()
            .enumerate()
            .map(|(i, l)| (i, Some(mature_level(l.kind, l.density)), LotState::Normal))
            .collect();
        scene.set_lots(&changes);
        let mut verts = scene.ground.vertices.len();
        let mut tris = scene.ground.triangles();
        let mut bytes = scene.ground.vertices.len() * 32 + (scene.ground.triangles()) * 12;
        let mut far = 0;
        for c in &scene.chunks {
            verts += c.mesh.vertices.len();
            tris += c.mesh.triangles();
            bytes += c.mesh.vertices.len() * 32 + c.mesh.triangles() * 12;
            if let Some(f) = &c.far {
                far += f.triangles();
                bytes += f.vertices.len() * 32 + f.triangles() * 12;
            }
        }
        println!(
            "{} {seed} {verts} {tris} {far} {:.1}",
            preset.name(),
            bytes as f64 / 1.048576e6
        );
    }
}
