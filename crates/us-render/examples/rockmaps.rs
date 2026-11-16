//! `cargo run --release -p us-render --example rockmaps -- <out_dir> [preset] [seed]`
//!
//! Views of the red rock and coastal styles: a wide look at the whole land, the rock or the cliffs
//! close up, and the town beside them.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{Preset, Relief, generate};
use us_render::{Camera, Renderer, Scene, Style};

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/maps".into())
        .into();
    let only = std::env::args().nth(2);
    let seed: u64 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    std::fs::create_dir_all(&out).expect("create output dir");
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 1000u32);
    let aspect = w as f32 / h as f32;
    for preset in [Preset::Mesa, Preset::Coast] {
        if only.as_deref().is_some_and(|o| o != preset.name()) {
            continue;
        }
        let town = generate(preset, seed);
        let scene = Scene::from_town(&town);
        let land = &scene.land;
        let on = |x: f64, y: f64| Vec3::new(x as f32, y as f32, land.height(x as f32, y as f32));
        let mut views: Vec<(String, Vec3, f32, f32, f32)> = Vec::new();
        match town.terrain.relief() {
            Relief::Mesa(m) => {
                views.push(("wide".into(), on(2000.0, 1900.0), 3600.0, 200.0, 32.0));
                views.push(("edge".into(), on(3800.0, 1900.0), 1400.0, 160.0, 55.0));
                let mut fs: Vec<_> = m.formations.iter().collect();
                fs.sort_by(|a, b| b.top.total_cmp(&a.top));
                if let Some(f) = fs.first() {
                    let c = on(f.c.x, f.c.y);
                    views.push(("mesa".into(), c, 1100.0, 215.0, 14.0));
                    views.push(("mesa-high".into(), c, 1000.0, 120.0, 28.0));
                }
                if let Some(f) = m.formations.iter().find(|f| f.kind == us_gen::Rock::Butte) {
                    views.push(("butte".into(), on(f.c.x, f.c.y), 700.0, 160.0, 18.0));
                }
            }
            Relief::Coast(c) => {
                views.push(("wide".into(), on(2350.0, 2000.0), 2700.0, 75.0, 34.0));
                let hv = c.coves[0].c;
                views.push(("harbour".into(), on(hv.x - 250.0, hv.y), 1000.0, 75.0, 22.0));
                let shore = |y: f64| on(c.edge_x(y, 40.0), y);
                views.push(("cliffs".into(), shore(hv.y + 1300.0), 800.0, 255.0, 16.0));
                views.push((
                    "cliffs-top".into(),
                    shore(hv.y - 1100.0),
                    1200.0,
                    300.0,
                    28.0,
                ));
            }
            Relief::None => {}
        }
        views.push((
            "town".into(),
            on(town.hall.x, town.hall.y),
            900.0,
            215.0,
            30.0,
        ));
        for (name, target, ext, yaw, pitch) in views {
            let cam = Camera::framing(target, ext, aspect, yaw, pitch);
            let style = Style {
                supersample: 2,
                shadow_size: 4096,
                ink_width: 1,
                shadow_tone: std::env::var("NOSHADOW")
                    .map_or(Style::default().shadow_tone, |_| 1.0),
                ..Style::default()
            };
            let f = r.render_to_rgba(&scene, &cam, &style, w, h);
            image::save_buffer(
                out.join(format!("{}{}-{name}.png", preset.name(), seed)),
                &f.rgba,
                f.width,
                f.height,
                image::ColorType::Rgba8,
            )
            .expect("write png");
            eprintln!("{} {name}", preset.name());
        }
    }
}
