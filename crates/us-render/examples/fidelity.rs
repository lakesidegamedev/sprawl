//! `cargo run --release -p us-render --example fidelity -- [out_dir]`
//!
//! Pictures for the fidelity pass: a close, a medium and a whole-town view, streets at each stage
//! of being built, and a dressed park. Times each view and counts triangles.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{Pattern, Preset, RoadClass, Zone, generate};
use us_render::{Camera, Chunk, Renderer, Scene, Style, live};

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/fidelity".into())
        .into();
    std::fs::create_dir_all(&out).expect("create output dir");
    let mut r = Renderer::new().expect("adapter");
    eprintln!("adapter: {:?}", r.adapter());
    let town = generate(Preset::Grid, 1);
    let mut scene = Scene::from_town(&town);
    let hall = Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0);
    let (w, h) = (1600u32, 1000u32);
    let aspect = w as f32 / h as f32;
    let style = Style {
        supersample: 1,
        ink_width: 1,
        ..Style::default()
    };
    let mut shoot = |name: &str, scene: &Scene, cam: &Camera| {
        let mut times = Vec::new();
        let mut last = None;
        for _ in 0..4 {
            let f = r.render_to_rgba(scene, cam, &style, w, h);
            times.push(f.stats.total.as_secs_f32());
            last = Some(f);
        }
        let f = last.unwrap();
        times.remove(0);
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        eprintln!(
            "{name}: median {:.3}s, {} tris, {} draw calls",
            times[times.len() / 2],
            f.stats.triangles,
            f.stats.draw_calls
        );
        image::save_buffer(
            out.join(format!("{name}.png")),
            &f.rgba,
            f.width,
            f.height,
            image::ColorType::Rgba8,
        )
        .expect("write png");
    };
    shoot(
        "close",
        &scene,
        &Camera::framing(
            hall + Vec3::new(120.0, -60.0, 0.0),
            110.0,
            aspect,
            30.0,
            38.0,
        ),
    );
    shoot(
        "medium",
        &scene,
        &Camera::framing(hall, 520.0, aspect, 35.0, 45.0),
    );
    shoot(
        "far",
        &scene,
        &Camera::framing(Vec3::new(2048.0, 1900.0, 0.0), 4200.0, aspect, 30.0, 48.0),
    );
    shoot(
        "wide",
        &scene,
        &Camera::framing(hall, 1800.0, aspect, 30.0, 45.0),
    );

    // streets at each stage of being built, nearest the hall first
    let mut near: Vec<usize> = (0..town.roads.len())
        .filter(|&i| matches!(town.roads[i].class, RoadClass::Local | RoadClass::Collector))
        .filter(|&i| {
            let p = town.roads[i].points[town.roads[i].points.len() / 2];
            p.dist(town.hall) < 260.0
        })
        .collect();
    near.sort_by_key(|&i| {
        let p = town.roads[i].points[0];
        (p.dist(town.hall) * 10.0) as i64
    });
    let stages = [0.1f32, 0.2, 0.4, 0.5, 0.7, 0.75, 0.9, 0.95];
    let changes: Vec<_> = near
        .iter()
        .enumerate()
        .map(|(k, &i)| (i, town.roads[i].class, Some(stages[k % stages.len()])))
        .collect();
    scene.set_roads(&changes);
    shoot(
        "roads-building",
        &scene,
        &Camera::framing(hall, 300.0, aspect, 30.0, 50.0),
    );
    let longest = near.iter().copied().max_by_key(|&i| {
        town.roads[i]
            .points
            .windows(2)
            .map(|w| w[0].dist(w[1]) as i64)
            .sum::<i64>()
    });
    if let Some(i) = longest {
        let pts = &town.roads[i].points;
        let total: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
        let (mut left, mut p) = (total / 2.0, pts[0]);
        for w in pts.windows(2) {
            let l = w[0].dist(w[1]);
            if left <= l {
                p = w[0].lerp(w[1], left / l.max(1e-9));
                break;
            }
            left -= l;
        }
        let t = Vec3::new(
            p.x as f32,
            p.y as f32,
            scene.land.height(p.x as f32, p.y as f32),
        );
        for (name, prog) in [
            ("roadwork-dirt", 0.15f32),
            ("roadwork-gravel", 0.45),
            ("roadwork-asphalt", 0.7),
            ("roadwork-lines", 0.9),
        ] {
            scene.set_roads(&[(i, town.roads[i].class, Some(prog))]);
            shoot(name, &scene, &Camera::framing(t, 60.0, aspect, 20.0, 50.0));
        }
    }

    // a dressed park, from the first park addition in a few presets
    for preset in [
        Preset::Grid,
        Preset::Winding,
        Preset::Sunbelt,
        Preset::Crossroads,
        Preset::LakeTown,
    ] {
        let t = generate(preset, 1);
        let Some(ai) = t
            .additions
            .iter()
            .position(|a| a.zone == Zone::Park && a.pattern == Pattern::Park)
        else {
            continue;
        };
        let land = us_render::Land::flat(t.size as f32);
        let mut s = Scene::dynamic(
            t.size as f32,
            live::base_ground_on(&land, &t, &[]),
            Vec3::new(t.hall.x as f32, t.hall.y as f32, 0.0),
        );
        let mesh = live::zone_ground_on(&land, &t, ai);
        s.chunks.push(Chunk {
            rev: 1,
            mesh,
            far: None,
            lo: Vec3::new(-1e4, -1e4, -50.0),
            hi: Vec3::new(1e4, 1e4, 200.0),
        });
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in &t.additions[ai].polygon {
            lo = lo.min(Vec3::new(p.x as f32, p.y as f32, 0.0));
            hi = hi.max(Vec3::new(p.x as f32, p.y as f32, 0.0));
        }
        let mid = (lo + hi) * 0.5;
        let ext = (hi - lo).max_element().clamp(80.0, 400.0);
        shoot(
            &format!("park-{}", preset.name()),
            &s,
            &Camera::framing(mid, ext * 1.05, aspect, 25.0, 52.0),
        );
    }
}
