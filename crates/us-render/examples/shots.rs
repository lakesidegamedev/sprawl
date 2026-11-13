//! `cargo run --release -p us-render --example shots -- [out_dir]`
//!
//! Generates grid seed 1 and winding seed 2 and renders three views of each at 1600 x 1000:
//! the whole town, a suburb close-up and the downtown around the town hall.

use glam::Vec3;
use std::path::PathBuf;
use std::time::Instant;
use us_gen::{LotKind, Preset, Town, generate};
use us_render::{Camera, Renderer, Scene, Style};

const W: u32 = 1600;
const H: u32 = 1000;

/// The middle of the thickest cluster of houses about a kilometre from the hall.
fn suburb_target(t: &Town) -> Vec3 {
    let houses: Vec<_> = t.lots.iter().filter(|l| l.kind == LotKind::House).collect();
    let mut best: Option<(f64, usize)> = None;
    for (i, h) in houses.iter().enumerate() {
        let d = h.centre.dist(t.hall);
        if !(800.0..1500.0).contains(&d) {
            continue;
        }
        let near = houses
            .iter()
            .filter(|o| o.centre.dist(h.centre) < 70.0)
            .count();
        let score = near as f64 * 10.0 - (d - 1100.0).abs() * 0.02;
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, i));
        }
    }
    let h = houses[best.map_or(0, |b| b.1)];
    Vec3::new(h.centre.x as f32, h.centre.y as f32, 0.0)
}

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/shots".into())
        .into();
    std::fs::create_dir_all(&out).expect("create output dir");
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    eprintln!("adapter: {:?}", r.adapter());
    let aspect = W as f32 / H as f32;
    for (preset, seed) in [(Preset::Grid, 1u64), (Preset::Winding, 2)] {
        let t0 = Instant::now();
        let town = generate(preset, seed);
        let t1 = Instant::now();
        let scene = Scene::from_town(&town);
        eprintln!(
            "{} seed {seed}: generate {:.2}s, build scene {:.2}s, {} lots, {} triangles",
            preset.name(),
            (t1 - t0).as_secs_f32(),
            t1.elapsed().as_secs_f32(),
            town.lots.len(),
            scene.triangles()
        );
        let hall = Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0);
        let views = [
            (
                "town",
                Camera::framing(
                    hall + Vec3::new(0.0, -150.0, 0.0),
                    2000.0,
                    aspect,
                    30.0,
                    48.0,
                ),
                (8192, 1, 3),
            ),
            (
                "suburb",
                Camera::framing(suburb_target(&town), 250.0, aspect, 45.0, 40.0),
                (4096, 2, 2),
            ),
            (
                "downtown",
                Camera::framing(hall, 250.0, aspect, 45.0, 40.0),
                (4096, 2, 2),
            ),
        ];
        for (name, cam, (shadow_size, ink_width, supersample)) in views {
            let style = Style {
                shadow_size,
                ink_width,
                supersample,
                ..Style::default()
            };
            let f = r.render_to_rgba(&scene, &cam, &style, W, H);
            let path = out.join(format!("{}-seed{seed}-{name}.png", preset.name()));
            image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
                .expect("write png");
            eprintln!(
                "  {name}: {:.2}s total, upload {:.2}s, gpu {:.2}s, {}/{} chunks, {} tris -> {}",
                f.stats.total.as_secs_f32(),
                f.stats.upload.as_secs_f32(),
                f.stats.gpu_wait.as_secs_f32(),
                f.stats.chunks_drawn,
                f.stats.chunks_total,
                f.stats.triangles,
                path.display()
            );
        }
    }
}
