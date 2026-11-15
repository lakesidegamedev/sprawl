//! `cargo run --release -p us-render --example trainsheet -- <out_dir>`
//!
//! Pictures of the trains: a passenger train standing at a depot platform, a freight train
//! passing, and a close view of each vehicle.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{RoadClass, pt};
use us_render::trains::TrainService;
use us_render::{Camera, Chunk, Mesh, Renderer, RoadInfo, Scene, Style, ground};

fn main() {
    let dir: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/trainsheets".into())
        .into();
    std::fs::create_dir_all(&dir).expect("create output dir");
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 900u32);
    let size = 1200.0f32;
    let line = vec![pt(0.0, 600.0), pt(size as f64, 600.0)];
    let mut scene = Scene::dynamic(size, Mesh::default(), Vec3::ZERO);
    let mut g = ground::Ground::default();
    g.rect(
        -40.0,
        -40.0,
        size + 80.0,
        size + 80.0,
        ground::layer::BASE,
        us_render::GRASS,
    );
    g.ribbon(&line, 2.6, 0.0, ground::layer::RAIL, [128, 104, 84]);
    g.ribbon(&line, 0.2, 0.9, ground::layer::RAIL + 0.06, [150, 154, 160]);
    g.ribbon(
        &line,
        0.2,
        -0.9,
        ground::layer::RAIL + 0.06,
        [150, 154, 160],
    );
    scene.ground = g.mesh;
    scene.chunks.push(Chunk {
        rev: 0,
        mesh: Mesh::default(),
        far: None,
        lo: Vec3::new(300.0, 500.0, 0.0),
        hi: Vec3::new(900.0, 700.0, 50.0),
    });
    scene.set_geometry(
        Vec::new(),
        vec![RoadInfo {
            class: RoadClass::Rail,
            points: line,
            works: None,
        }],
    );
    scene.set_trains(TrainService {
        passenger_per_hour: 6.0,
        freight_per_hour: 3.0,
        depots: vec![[600.0, 618.0]],
        elevators: vec![],
    });
    // find the moment with the most vehicles in the middle of the map
    let style = Style {
        supersample: 2,
        shadow_size: 4096,
        ink_width: 1,
        ..Style::default()
    };
    let mut best = (0usize, 0.0f32);
    for k in 0..1500 {
        let t = k as f32 * 1.0;
        scene.set_time(t);
        let n = scene
            .instances()
            .trains
            .sets
            .iter()
            .flatten()
            .filter(|i| (i.pos[0] - 600.0).abs() < 150.0)
            .count();
        if n > best.0 {
            best = (n, t);
        }
    }
    eprintln!(
        "busiest moment t={} with {} vehicles near the middle",
        best.1, best.0
    );
    for (name, t, span) in [("wide", best.1, 420.0f32), ("close", best.1, 140.0)] {
        scene.set_time(t);
        let cam = Camera::framing(
            Vec3::new(600.0, 600.0, 0.0),
            span,
            w as f32 / h as f32,
            20.0,
            40.0,
        );
        let f = r.render_to_rgba(&scene, &cam, &style, w, h);
        let path = dir.join(format!("trains-{name}.png"));
        image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
            .expect("write png");
        eprintln!("{}: {} tris", path.display(), f.stats.triangles);
    }
}
