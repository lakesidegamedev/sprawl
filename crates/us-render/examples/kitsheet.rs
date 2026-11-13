//! `cargo run --release -p us-render --example kitsheet -- <out_dir> [kind|all] [region] [levels]`
//!
//! Kit sheets: a grid of different models of one lot kind and level, standing on grass, seen
//! from the street side. Region is one of mixed, prairie, river, sunbelt, crossroads, lake.
//! Levels is a list such as `1,2,3` (default all five).

use glam::Vec3;
use std::path::PathBuf;
use us_render::kit::ALL_KINDS;
use us_render::{
    Camera, Chunk, Mesh, Region, Renderer, Rng, Scene, Style, build_lot_level_in, ground,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir: PathBuf = args
        .first()
        .cloned()
        .unwrap_or_else(|| "target/kitsheets".into())
        .into();
    std::fs::create_dir_all(&dir).expect("create output dir");
    let which = args.get(1).cloned().unwrap_or_else(|| "all".into());
    let region = Region::ALL
        .iter()
        .copied()
        .find(|r| Some(r.name()) == args.get(2).map(|s| s.as_str()))
        .unwrap_or(Region::Mixed);
    let levels: Vec<u8> = args
        .get(3)
        .map(|s| s.split(',').filter_map(|v| v.parse().ok()).collect())
        .unwrap_or_else(|| (1..=5).collect());
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 1000u32);
    let aspect = w as f32 / h as f32;
    for kind in ALL_KINDS {
        if which != "all" && which != kind.name() {
            continue;
        }
        let radius = kind.nominal_radius() as f32;
        let big = radius > 20.0;
        let (cols, rows) = if big {
            (2usize, 2usize)
        } else if radius > 11.0 {
            (4, 3)
        } else {
            (5, 3)
        };
        let cell = radius * 2.3 + if big { 30.0 } else { 3.0 };
        for &level in &levels {
            let size = cell * (cols.max(rows) as f32 + 1.0);
            let mut scene = Scene::dynamic(size, Mesh::default(), Vec3::ZERO);
            let mut mesh = Mesh::default();
            let mut id = 1u32;
            for i in 0..cols * rows {
                let (cx, cy) = ((i % cols) as f32, (i / cols) as f32);
                let mut rng = Rng::new(0x5ee7 + i as u64 * 17 + level as u64, kind as u64 + 3);
                let o = build_lot_level_in(region, kind, level, &mut rng);
                o.emit(
                    &mut mesh,
                    &mut id,
                    cell * (cx + 0.5),
                    cell * (cy + 0.5),
                    0.0,
                    1.0,
                );
            }
            let mut g = ground::Ground::default();
            g.rect(
                -40.0,
                -40.0,
                cell * cols as f32 + 80.0,
                cell * rows as f32 + 80.0,
                ground::layer::BASE,
                us_render::GRASS,
            );
            scene.ground = g.mesh;
            scene.chunks.push(Chunk {
                rev: 0,
                mesh,
                far: None,
                lo: Vec3::new(-60.0, -60.0, 0.0),
                hi: Vec3::new(cell * cols as f32 + 60.0, cell * rows as f32 + 60.0, 160.0),
            });
            let centre = Vec3::new(cell * cols as f32 / 2.0, cell * rows as f32 / 2.0, 0.0);
            let cam = Camera::framing(centre, cell * cols as f32 * 1.02, aspect, 0.0, 52.0);
            let style = Style {
                supersample: 2,
                shadow_size: 4096,
                ink_width: 1,
                ..Style::default()
            };
            let f = r.render_to_rgba(&scene, &cam, &style, w, h);
            let path = dir.join(format!("{}-L{}-{}.png", kind.name(), level, region.name()));
            image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
                .expect("write png");
            eprintln!("{}: {} tris", path.display(), f.stats.triangles);
        }
    }
}
