//! `cargo run --release -p us-render --example rockviews -- <out_dir> <mesa|coast> [seed] [view name filter]`
//!
//! Views of the red rock and coast towns: the whole map, the town, and close views of the rock,
//! the harbour, the cliffs and the sea stacks.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{Preset, RockCoast, generate};
use us_render::{Camera, Renderer, Scene, Style};

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args
        .next()
        .unwrap_or_else(|| "target/rockviews".into())
        .into();
    let preset = args
        .next()
        .and_then(|n| Preset::from_name(&n))
        .unwrap_or(Preset::Coast);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    let only = args.next();
    std::fs::create_dir_all(&out).expect("create output dir");
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 1000u32);
    let aspect = w as f32 / h as f32;
    let town = generate(preset, seed);
    let scene = Scene::from_town(&town);
    let land = &scene.land;
    let on = |x: f64, y: f64| Vec3::new(x as f32, y as f32, land.height(x as f32, y as f32));
    let mid = Vec3::new(2000.0, 2000.0, 0.0);
    let mut views: Vec<(String, Vec3, f32, f32, f32)> = vec![
        ("whole".into(), mid, 4200.0, 20.0, 38.0),
        ("whole-b".into(), mid, 4200.0, 200.0, 30.0),
        (
            "town".into(),
            on(town.hall.x, town.hall.y),
            1700.0,
            200.0,
            36.0,
        ),
    ];
    match town.terrain.rock_coast() {
        RockCoast::Mesa(m) => {
            let mut fs: Vec<_> = m.formations.iter().collect();
            fs.sort_by(|a, b| b.top.total_cmp(&a.top));
            for (i, f) in fs.iter().take(3).enumerate() {
                views.push((
                    format!("rock{i}"),
                    on(f.c.x, f.c.y),
                    (f.r * 4.0).max(500.0) as f32,
                    30.0 + 80.0 * i as f32,
                    24.0,
                ));
            }
            if let Some(f) = fs.first() {
                views.push(("rock-wide".into(), on(f.c.x, f.c.y), 1800.0, 200.0, 22.0));
                views.push(("rock-top".into(), on(f.c.x, f.c.y), 900.0, 140.0, 55.0));
            }
            views.push(("horizon".into(), on(2000.0, 3900.0), 1200.0, 0.0, 12.0));
        }
        RockCoast::Coast(c) => {
            let hv = c.coves[0];
            views.push((
                "harbour".into(),
                on(hv.c.x - 0.3 * hv.rho, hv.c.y),
                1100.0,
                250.0,
                38.0,
            ));
            views.push(("harbour-b".into(), on(hv.c.x, hv.c.y), 700.0, 110.0, 26.0));
            views.push(("harbour-top".into(), on(hv.c.x, hv.c.y), 1100.0, 90.0, 70.0));
            views.push((
                "cliffs".into(),
                on(c.edge_x(2600.0, 0.0), 2600.0),
                900.0,
                285.0,
                20.0,
            ));
            for (k, ext, yaw, tilt) in [
                (2usize, 160.0f32, 250.0f32, 12.0f32),
                (1, 260.0, 250.0, 25.0),
                (1, 260.0, 70.0, 18.0),
                (1, 260.0, 160.0, 12.0),
                (4, 300.0, 250.0, 25.0),
                (7, 300.0, 250.0, 25.0),
                (7, 300.0, 70.0, 25.0),
                (7, 300.0, 160.0, 60.0),
                (0, 260.0, 300.0, 10.0),
                (3, 240.0, 230.0, 10.0),
            ] {
                if let Some(s) = c.stacks.get(k) {
                    views.push((
                        format!("stack{k}-{yaw}"),
                        Vec3::new(s.c.x as f32, s.c.y as f32, s.h as f32 * 0.5),
                        ext.max(s.h as f32 * 3.4),
                        yaw,
                        tilt,
                    ));
                }
            }
        }
        RockCoast::None => {}
    }
    for (name, target, ext, yaw, pitch) in views {
        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }
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
