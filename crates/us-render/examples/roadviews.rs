//! `cargo run --release -p us-render --example roadviews -- <out_dir>`
//!
//! Close views of roads: a roundabout, a highway ramp, an arterial with its median, and a
//! suburban junction, for towns made with the newest generator.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{Preset, RoadClass, Town, generate};
use us_render::{Camera, Renderer, Scene, Style};

fn is_ring(r: &us_gen::Road) -> bool {
    r.class == RoadClass::Collector
        && r.points.len() == 33
        && r.points[0].dist(*r.points.last().unwrap()) < 1e-6
}

fn centre(pts: &[us_gen::Pt]) -> (f32, f32) {
    let n = pts.len() as f64;
    (
        (pts.iter().map(|p| p.x).sum::<f64>() / n) as f32,
        (pts.iter().map(|p| p.y).sum::<f64>() / n) as f32,
    )
}

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/roadviews".into())
        .into();
    std::fs::create_dir_all(&out).expect("create output dir");
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1400u32, 900u32);
    let aspect = w as f32 / h as f32;
    // the first town with a roundabout and a ramp, searching presets and seeds in order
    let mut found: Option<Town> = None;
    'search: for seed in 1..=8u64 {
        for p in Preset::ALL {
            let t = generate(p, seed);
            let ramps = t
                .roads
                .iter()
                .any(|r| r.class == RoadClass::Collector && r.points.len() == 15);
            if ramps && t.roads.iter().any(is_ring) {
                eprintln!("using {} seed {seed}", p.name());
                found = Some(t);
                break 'search;
            }
        }
    }
    let town = found.expect("a town with both");
    let scene = Scene::from_town(&town);
    let mut views: Vec<(String, Vec3, f32, f32, f32)> = Vec::new();
    if let Some(ring) = town.roads.iter().find(|r| is_ring(r)) {
        let (x, y) = centre(&ring.points);
        views.push(("ring".into(), Vec3::new(x, y, 0.0), 90.0, 25.0, 50.0));
        views.push(("ring-high".into(), Vec3::new(x, y, 0.0), 220.0, 25.0, 60.0));
    }
    if let Some(ramp) = town
        .roads
        .iter()
        .find(|r| r.class == RoadClass::Collector && r.points.len() == 15)
    {
        let (x, y) = centre(&ramp.points);
        views.push(("ramp".into(), Vec3::new(x, y, 0.0), 180.0, 20.0, 55.0));
    }
    if let Some(a) = town
        .roads
        .iter()
        .filter(|r| r.class == RoadClass::Arterial)
        .max_by(|a, b| a.points.len().cmp(&b.points.len()))
    {
        let p = a.points[a.points.len() / 2];
        views.push((
            "arterial".into(),
            Vec3::new(p.x as f32, p.y as f32, 0.0),
            110.0,
            30.0,
            45.0,
        ));
    }
    // a place where several local streets meet, well out of the old town
    let mut best: Option<(usize, us_gen::Pt)> = None;
    for road in town.roads.iter().filter(|r| r.class == RoadClass::Local) {
        for p in [road.points[0], *road.points.last().unwrap()] {
            if p.dist(town.hall) < 500.0 {
                continue;
            }
            let n = town
                .roads
                .iter()
                .filter(|o| o.class != RoadClass::Rail)
                .filter(|o| {
                    o.points.iter().any(|q| q.dist(p) < 3.0)
                        || o.points.windows(2).any(|w| {
                            let ab = w[1] - w[0];
                            let t = ((p - w[0]).dot(ab) / ab.dot(ab).max(1e-9)).clamp(0.0, 1.0);
                            p.dist(w[0] + ab * t) < 2.0
                        })
                })
                .count();
            if best.is_none_or(|b| n > b.0) {
                best = Some((n, p));
            }
        }
    }
    if let Some((_, p)) = best {
        views.push((
            "junction".into(),
            Vec3::new(p.x as f32, p.y as f32, 0.0),
            70.0,
            30.0,
            50.0,
        ));
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
            out.join(format!("{name}.png")),
            &f.rgba,
            f.width,
            f.height,
            image::ColorType::Rgba8,
        )
        .expect("write png");
        eprintln!("wrote {name}");
    }
}
