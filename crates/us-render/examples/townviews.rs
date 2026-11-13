//! `cargo run --release -p us-render --example townviews -- <out_dir>`
//!
//! One close view of the thickest housing cluster and one downtown view for every town preset.

use glam::Vec3;
use std::path::PathBuf;
use std::time::Instant;
use us_gen::{LotKind, Preset, generate};
use us_render::{Camera, Renderer, Scene, Style};

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/townviews".into())
        .into();
    std::fs::create_dir_all(&out).expect("create output dir");
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 1000u32);
    let aspect = w as f32 / h as f32;
    for preset in Preset::ALL {
        let town = generate(preset, 1);
        let t = Instant::now();
        let scene = Scene::from_town(&town);
        eprintln!(
            "{}: scene build {:.2}s, {} triangles",
            preset.name(),
            t.elapsed().as_secs_f32(),
            scene.triangles()
        );
        let houses: Vec<_> = town
            .lots
            .iter()
            .filter(|l| l.kind == LotKind::House)
            .collect();
        let mut best = (0usize, 0usize);
        for (i, a) in houses.iter().enumerate() {
            let d = a.centre.dist(town.hall);
            if d < 300.0 {
                continue;
            }
            let n = houses
                .iter()
                .filter(|o| o.centre.dist(a.centre) < 60.0)
                .count();
            if n > best.0 {
                best = (n, i);
            }
        }
        let c = houses[best.1].centre;
        let hall = Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0);
        let views = [
            (
                "suburb",
                Vec3::new(c.x as f32, c.y as f32, 0.0),
                170.0,
                25.0,
                42.0,
            ),
            ("downtown", hall, 150.0, 35.0, 40.0),
        ];
        for (name, target, ext, yaw, pitch) in views {
            let cam = Camera::framing(target, ext, aspect, yaw, pitch);
            let style = Style {
                supersample: 2,
                shadow_size: 4096,
                ink_width: 1,
                ..Style::default()
            };
            let f = r.render_to_rgba(&scene, &cam, &style, w, h);
            image::save_buffer(
                out.join(format!("{}-{name}.png", preset.name())),
                &f.rgba,
                f.width,
                f.height,
                image::ColorType::Rgba8,
            )
            .expect("write png");
        }
    }
}
