//! `cargo run --release -p us-render --example hills -- <out_dir> [preset]`
//!
//! Views of the hills for every town preset: an overview, a bridge over the river valley, wooded
//! slopes and the densest housing block.

use glam::Vec3;
use std::path::PathBuf;
use std::time::Instant;
use us_gen::{LotKind, Pattern, Preset, Zone, generate, pt};
use us_render::{Camera, Renderer, Scene, Style};

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/hills".into())
        .into();
    let only = std::env::args().nth(2);
    std::fs::create_dir_all(&out).expect("create output dir");
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 1000u32);
    let aspect = w as f32 / h as f32;
    for preset in Preset::ALL {
        if only.as_deref().is_some_and(|o| o != preset.name()) {
            continue;
        }
        let town = generate(preset, 1);
        let t = Instant::now();
        let scene = Scene::from_town(&town);
        eprintln!(
            "{}: scene build {:.2}s, {} triangles (ground {})",
            preset.name(),
            t.elapsed().as_secs_f32(),
            scene.triangles(),
            scene.ground.triangles()
        );
        let land = &scene.land;
        let on = |x: f64, y: f64| Vec3::new(x as f32, y as f32, land.height(x as f32, y as f32));
        let mut views: Vec<(&str, Vec3, f32, f32, f32)> = Vec::new();

        // overview: the built-up window with the most relief
        let mut hilly: Option<(f32, f64, f64)> = None;
        let mut gy = 700.0;
        while gy < 3500.0 {
            let mut gx = 700.0;
            while gx < 3500.0 {
                let c = pt(gx, gy);
                let n = town
                    .lots
                    .iter()
                    .filter(|l| l.centre.dist(c) < 350.0)
                    .count();
                if n >= 40 {
                    let (lo, hi) = land.range_over(
                        gx as f32 - 450.0,
                        gy as f32 - 450.0,
                        gx as f32 + 450.0,
                        gy as f32 + 450.0,
                    );
                    if hilly.is_none_or(|b| hi - lo > b.0) {
                        hilly = Some((hi - lo, gx, gy));
                    }
                }
                gx += 200.0;
            }
            gy += 200.0;
        }
        let (relief, hx, hy) = hilly.unwrap_or((0.0, town.hall.x, town.hall.y));
        eprintln!("  most relief near built land: {relief:.1} m");
        views.push(("overview", on(hx, hy), 1300.0, 20.0, 30.0));

        // a bridge: the first road that gets a deck
        'bridge: for road in &town.roads {
            let bed = land.bed(&road.points);
            if let Some(&(a, b)) = bed.spans().first() {
                let m = bed.pts[(a + b) / 2];
                let d = bed.pts[b] - bed.pts[a];
                let yaw = (d.x.atan2(d.y).to_degrees() as f32) + 70.0;
                let mut t = on(m.x, m.y);
                t.z = bed.deck[(a + b) / 2];
                views.push(("bridge", t, 260.0, yaw, 30.0));
                break 'bridge;
            }
        }

        // wooded slopes: the park patch with the most slope
        let mut best: Option<(f32, usize)> = None;
        for (ai, a) in town.additions.iter().enumerate() {
            if a.zone != Zone::Park || a.pattern != Pattern::Park {
                continue;
            }
            let n = a.polygon.len() as f64;
            let c = pt(
                a.polygon.iter().map(|p| p.x).sum::<f64>() / n,
                a.polygon.iter().map(|p| p.y).sum::<f64>() / n,
            );
            let s = land.slope(c.x as f32, c.y as f32);
            if best.is_none_or(|b| s > b.0) {
                best = Some((s, ai));
            }
        }
        if let Some((_, ai)) = best {
            let a = &town.additions[ai];
            let n = a.polygon.len() as f64;
            let c = pt(
                a.polygon.iter().map(|p| p.x).sum::<f64>() / n,
                a.polygon.iter().map(|p| p.y).sum::<f64>() / n,
            );
            views.push(("woods", on(c.x, c.y), 420.0, 200.0, 30.0));
        }

        // the densest block of houses
        let houses: Vec<_> = town
            .lots
            .iter()
            .filter(|l| l.kind == LotKind::House)
            .collect();
        let mut bestn = (0usize, 0usize);
        for (i, a) in houses.iter().enumerate() {
            if a.centre.dist(town.hall) < 300.0 {
                continue;
            }
            let n = houses
                .iter()
                .filter(|o| o.centre.dist(a.centre) < 60.0)
                .count();
            if n > bestn.0 {
                bestn = (n, i);
            }
        }
        if let Some(a) = houses.get(bestn.1) {
            views.push(("dense", on(a.centre.x, a.centre.y), 170.0, 25.0, 40.0));
        }
        // a hillside with houses on it
        let mut steep: Option<(f32, usize)> = None;
        for (i, l) in town.lots.iter().enumerate() {
            let s = land.slope(l.centre.x as f32, l.centre.y as f32);
            let near = town
                .lots
                .iter()
                .filter(|o| o.centre.dist(l.centre) < 90.0)
                .count();
            if near > 10 && steep.is_none_or(|b| s > b.0) {
                steep = Some((s, i));
            }
        }
        if let Some((_, i)) = steep {
            let c = town.lots[i].centre;
            views.push(("hillside", on(c.x, c.y), 260.0, 300.0, 28.0));
        }

        for (name, target, ext, yaw, pitch) in views {
            let cam = Camera::framing(target, ext, aspect, yaw, pitch);
            let style = Style {
                supersample: 2,
                shadow_size: 4096,
                ink_width: 1,
                ..Style::default()
            };
            let t = Instant::now();
            let f = r.render_to_rgba(&scene, &cam, &style, w, h);
            eprintln!("  {name}: {:.2}s", t.elapsed().as_secs_f32());
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
