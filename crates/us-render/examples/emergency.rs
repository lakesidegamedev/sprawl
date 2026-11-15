//! `cargo run --release -p us-render --example emergency -- <out_dir> [seed]`
//!
//! The emergency vehicles and a burning building, close up, to check the models.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{LotKind, Preset, generate};
use us_render::emergency::{EmBlaze, EmVehicle, RigKind};
use us_render::{Camera, LotState, Renderer, Scene, Style};

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args
        .next()
        .unwrap_or_else(|| "target/emergency".into())
        .into();
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    std::fs::create_dir_all(&out).expect("create output dir");
    let town = generate(Preset::Grid, seed);
    let mut scene = Scene::from_town(&town);
    let changes: Vec<(usize, Option<u8>, LotState)> = town
        .lots
        .iter()
        .enumerate()
        .map(|(i, l)| {
            (
                i,
                Some(if l.kind == LotKind::House { 2 } else { 1 }),
                LotState::Normal,
            )
        })
        .collect();
    scene.set_lots(&changes);
    let c = town.hall;
    let at = Vec3::new(c.x as f32 + 40.0, c.y as f32 + 40.0, 0.0);
    let mut vs = Vec::new();
    for (k, kind) in RigKind::ALL.iter().enumerate() {
        vs.push(EmVehicle {
            kind: *kind,
            pos: [at.x + k as f32 * 9.0, at.y],
            heading: 0.3,
            moving: k != 1,
        });
    }
    let blazes = vec![
        EmBlaze {
            pos: [at.x - 22.0, at.y + 8.0],
            stage: 1,
            top: 8.0,
        },
        EmBlaze {
            pos: [at.x - 22.0, at.y + 30.0],
            stage: 2,
            top: 11.0,
        },
        EmBlaze {
            pos: [at.x - 22.0, at.y - 14.0],
            stage: 0,
            top: 8.0,
        },
    ];
    scene.set_emergency(vs, blazes);
    scene.time = 2.1;
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1400u32, 900u32);
    let aspect = w as f32 / h as f32;
    let target = Vec3::new(at.x, at.y, scene.height_at(at.x, at.y));
    for (name, ext, yaw, pitch) in [("vehicles", 28.0, 20.0, 30.0), ("fires", 90.0, 60.0, 28.0)] {
        let cam = Camera::framing(target, ext, aspect, yaw, pitch);
        let style = Style {
            supersample: 2,
            shadow_size: 2048,
            ink_width: 1,
            ..Style::default()
        };
        let f = r.render_to_rgba(&scene, &cam, &style, w, h);
        let path = out.join(format!("{name}.png"));
        image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
            .expect("write png");
        eprintln!("{}", path.display());
    }
}
