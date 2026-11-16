//! `cargo run --release -p us-render --example mountains -- <out_dir> [preset] [seed]`
//!
//! Views of the mountain towns (gorge and valley): the whole map, the town, a bridge and a low
//! angle that shows the rock faces.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{Preset, generate};
use us_render::{Camera, Renderer, Scene, Style};

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/mountains".into())
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
    for preset in [Preset::Gorge, Preset::Valley] {
        if only.as_deref().is_some_and(|o| o != preset.name()) {
            continue;
        }
        let town = generate(preset, seed);
        let scene = Scene::from_town(&town);
        let land = &scene.land;
        eprintln!(
            "{}: heights {:.0} to {:.0} m",
            preset.name(),
            land.min_height(),
            land.max_height()
        );
        let on = |x: f64, y: f64| Vec3::new(x as f32, y as f32, land.height(x as f32, y as f32));
        let mut views: Vec<(String, Vec3, f32, f32, f32)> = Vec::new();
        let mid = Vec3::new(2000.0, 2000.0, 0.0);
        views.push(("whole-a".into(), mid, 4200.0, 20.0, 38.0));
        views.push(("whole-b".into(), mid, 4200.0, 200.0, 30.0));
        views.push(("whole-c".into(), mid, 4200.0, 110.0, 24.0));
        views.push((
            "town".into(),
            on(town.hall.x, town.hall.y),
            1900.0,
            200.0,
            36.0,
        ));
        views.push((
            "town-b".into(),
            on(town.hall.x, town.hall.y),
            1900.0,
            20.0,
            36.0,
        ));
        views.push((
            "town-low".into(),
            on(town.hall.x, town.hall.y),
            1500.0,
            160.0,
            14.0,
        ));
        for road in &town.roads {
            let bed = land.bed(&road.points);
            if let Some(&(a, b)) = bed.spans().first() {
                let m = bed.pts[(a + b) / 2];
                let d = bed.pts[b] - bed.pts[a];
                let yaw = (d.x.atan2(d.y).to_degrees() as f32) + 70.0;
                let mut t = on(m.x, m.y);
                t.z = bed.deck[(a + b) / 2];
                views.push(("bridge".into(), t, 520.0, yaw, 22.0));
                break;
            }
        }
        if preset == Preset::Gorge {
            let sw = town.roads.iter().find(|r| {
                town.height_at(*r.points.last().unwrap()) < 8.0
                    && town.height_at(r.points[0]) > 150.0
            });
            if let Some(r) = sw {
                let m = r.points[r.points.len() / 2];
                views.push(("switch".into(), on(m.x, m.y), 700.0, 0.0, 40.0));
                views.push(("switch-b".into(), on(m.x, m.y), 420.0, 170.0, 24.0));
            }
        }
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
            eprintln!("  {name} done");
        }
    }
}
