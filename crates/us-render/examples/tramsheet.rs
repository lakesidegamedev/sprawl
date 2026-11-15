//! `cargo run --release -p us-render --example tramsheet -- <out_dir>`
//!
//! Pictures of the trams: a street with track and wire poles in three stages of building, and a
//! line with trams running both ways.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{RoadClass, pt};
use us_render::trams::{BuildStage, TrackDraw, TramLineService, TramService, track_mesh};
use us_render::{Camera, Chunk, Land, Mesh, Renderer, RoadInfo, Scene, Style, ground};

fn main() {
    let dir: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/tramsheets".into())
        .into();
    std::fs::create_dir_all(&dir).expect("create output dir");
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 900u32);
    let size = 1200.0f32;
    let street = vec![pt(0.0, 600.0), pt(600.0, 600.0), pt(size as f64, 640.0)];
    let land = Land::flat(size);
    let style = Style {
        supersample: 2,
        shadow_size: 4096,
        ink_width: 1,
        ..Style::default()
    };
    // the street is the same in every picture; the line is built from left to right
    let views: [(&str, BuildStage, f32, f32, f32); 4] = [
        ("track", BuildStage::Track, 0.6, 300.0, 140.0),
        ("wires", BuildStage::Wires, 0.5, 300.0, 140.0),
        ("open-wide", BuildStage::Open, 1.0, 600.0, 420.0),
        ("open-close", BuildStage::Open, 1.0, 600.0, 120.0),
    ];
    for (name, stage, progress, cx, span) in views {
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
        g.ribbon(&street, 8.0, 0.0, ground::layer::ROAD, [88, 90, 96]);
        g.ribbon(&street, 0.12, 0.0, ground::layer::MARK, [226, 200, 90]);
        scene.ground = g.mesh;
        scene.chunks.push(Chunk {
            rev: 0,
            mesh: track_mesh(
                &land,
                &[TrackDraw {
                    points: &street,
                    stage,
                    progress,
                }],
            ),
            far: None,
            lo: Vec3::new(-100.0, -100.0, -5.0),
            hi: Vec3::new(size + 100.0, size + 100.0, 50.0),
        });
        scene.set_geometry(
            Vec::new(),
            vec![RoadInfo {
                class: RoadClass::Arterial,
                points: street.clone(),
                works: None,
            }],
        );
        let pts: Vec<[f32; 2]> = street.iter().map(|p| [p.x as f32, p.y as f32]).collect();
        let total: f32 = pts
            .windows(2)
            .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
            .sum();
        scene.set_trams(TramService {
            lines: vec![TramLineService {
                points: pts,
                stop_at: vec![0.0, total * 0.5, total],
                per_hour: 6.0,
                open: stage == BuildStage::Open,
            }],
        });
        // the moment with most trams near the middle
        let mut best = (0usize, 0.0f32);
        for k in 0..600 {
            let t = k as f32;
            scene.set_time(t);
            let n = scene
                .instances()
                .trains
                .sets
                .iter()
                .flatten()
                .filter(|i| (i.pos[0] - cx).abs() < span)
                .count();
            if n > best.0 {
                best = (n, t);
            }
        }
        scene.set_time(best.1);
        let cam = Camera::framing(
            Vec3::new(cx, 605.0, 0.0),
            span,
            w as f32 / h as f32,
            20.0,
            40.0,
        );
        let f = r.render_to_rgba(&scene, &cam, &style, w, h);
        let path = dir.join(format!("trams-{name}.png"));
        image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
            .expect("write png");
        eprintln!(
            "{}: {} tris, {} trams",
            path.display(),
            f.stats.triangles,
            best.0
        );
    }
}
