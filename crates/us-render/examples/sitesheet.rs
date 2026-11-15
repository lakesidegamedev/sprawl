//! `cargo run --release -p us-render --example sitesheet -- <out_dir> [kind] [level]`
//!
//! A building site at every construction step, left to right and top to bottom, for one lot kind
//! and level. A second sheet shows the same job as a rebuild, with the old building coming down.

use glam::Vec3;
use std::path::PathBuf;
use us_render::kit::ALL_KINDS;
use us_render::levels::building_site_for;
use us_render::site::{STEPS, step_progress};
use us_render::{
    Camera, Chunk, Mesh, Region, Renderer, Rng, Scene, Style, build_lot_level_in, ground,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir: PathBuf = args
        .first()
        .cloned()
        .unwrap_or_else(|| "target/sitesheets".into())
        .into();
    std::fs::create_dir_all(&dir).expect("create output dir");
    let which = args.get(1).cloned().unwrap_or_else(|| "office".into());
    let level: u8 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(5);
    let kind = ALL_KINDS
        .iter()
        .copied()
        .find(|k| k.name() == which)
        .expect("lot kind");
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1800u32, 1100u32);
    for rebuild in [false, true] {
        let cols = 4usize;
        let rows = (STEPS as usize).div_ceil(cols);
        let cell = kind.nominal_radius() as f32 * 2.6 + 60.0;
        let mut mesh = Mesh::default();
        let mut id = 1u32;
        let mut rng = Rng::new(0x5ee7, 3);
        let new = build_lot_level_in(Region::Mixed, kind, level, &mut rng);
        let old = build_lot_level_in(Region::Mixed, kind, level.saturating_sub(2).max(1), &mut Rng::new(0x5ee8, 3));
        for s in 0..STEPS as usize {
            let (cx, cy) = ((s % cols) as f32, (s / cols) as f32);
            let o = building_site_for(
                &new,
                rebuild.then_some(&old),
                level >= 4,
                step_progress(s as u8),
            );
            o.emit(&mut mesh, &mut id, cell * (cx + 0.5), cell * (cy + 0.5), 0.0, 1.0);
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
        let size = cell * (cols as f32 + 1.0);
        let mut scene = Scene::dynamic(size, Mesh::default(), Vec3::ZERO);
        scene.ground = g.mesh;
        scene.chunks.push(Chunk {
            rev: 0,
            mesh,
            far: None,
            lo: Vec3::new(-60.0, -60.0, 0.0),
            hi: Vec3::new(cell * cols as f32 + 60.0, cell * rows as f32 + 60.0, 160.0),
        });
        let centre = Vec3::new(cell * cols as f32 / 2.0, cell * rows as f32 / 2.0, 0.0);
        let cam = Camera::framing(centre, cell * cols as f32 * 1.1, w as f32 / h as f32, 0.0, 52.0);
        let style = Style {
            supersample: 2,
            shadow_size: 4096,
            ink_width: 1,
            ..Style::default()
        };
        let f = r.render_to_rgba(&scene, &cam, &style, w, h);
        let path = dir.join(format!(
            "{}-L{}-{}.png",
            kind.name(),
            level,
            if rebuild { "rebuild" } else { "new" }
        ));
        image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
            .expect("write png");
        eprintln!("{}: {} tris", path.display(), f.stats.triangles);
    }
}
