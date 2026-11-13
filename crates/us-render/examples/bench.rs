//! `cargo run --release -p us-render --example bench -- [out_dir]`
//!
//! Frame time of the whole 4 km town at 1920x1080 (no supersampling), with the level of detail
//! off and on, from a few camera heights. Also writes the pictures.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{Preset, generate};
use us_render::{Camera, Renderer, Scene, Style};

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/bench".into())
        .into();
    std::fs::create_dir_all(&out).expect("create output dir");
    let mut r = Renderer::new().expect("adapter");
    eprintln!("adapter: {:?}", r.adapter());
    let town = generate(Preset::Grid, 1);
    let scene = Scene::from_town(&town);
    let hall = Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0);
    let (w, h) = (1920u32, 1080u32);
    let aspect = w as f32 / h as f32;
    let views = [
        (
            "whole",
            Camera::framing(Vec3::new(2048.0, 1900.0, 0.0), 4200.0, aspect, 30.0, 48.0),
        ),
        ("wide", Camera::framing(hall, 1500.0, aspect, 30.0, 45.0)),
        ("close", Camera::framing(hall, 300.0, aspect, 45.0, 40.0)),
    ];
    for (name, cam) in views {
        for lod in [false, true] {
            let style = Style {
                supersample: 1,
                shadow_size: 4096,
                ink_width: 1,
                lod,
                ..Style::default()
            };
            let mut times = Vec::new();
            let mut last = None;
            for _ in 0..9 {
                let f = r.render_to_rgba(&scene, &cam, &style, w, h);
                times.push(f.stats.total.as_secs_f32());
                last = Some(f);
            }
            let f = last.unwrap();
            times.remove(0); // first frame uploads the scene
            times.sort_by(|a, b| a.partial_cmp(b).unwrap());
            eprintln!(
                "{name} lod={lod}: min {:.3}s median {:.3}s, {} tris, {}/{} chunks drawn, {} far",
                times[0],
                times[times.len() / 2],
                f.stats.triangles,
                f.stats.chunks_drawn,
                f.stats.chunks_total,
                f.stats.chunks_far
            );
            image::save_buffer(
                out.join(format!("{name}-{}.png", if lod { "lod" } else { "full" })),
                &f.rgba,
                f.width,
                f.height,
                image::ColorType::Rgba8,
            )
            .expect("write png");
        }
    }
}
