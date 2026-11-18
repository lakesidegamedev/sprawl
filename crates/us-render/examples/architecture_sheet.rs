//! Render the twelve plans in each architecture family for review.
use glam::Vec3;
use std::path::PathBuf;
use us_render::{Camera, Chunk, Mesh, Renderer, Scene, Style, architecture, ground};

fn main() {
    let out = PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out).expect("create architecture output");
    let mut renderer = Renderer::new().expect("graphics adapter");
    for (family, label) in architecture::FAMILIES.into_iter().enumerate() {
        let tower = family >= 5;
        let cell = if tower { 49.0 } else { 26.0 };
        let length = cell * 4.0;
        let width = cell * 3.0;
        let mut plot = ground::Ground::default();
        plot.rect(
            -30.0,
            -30.0,
            length + 30.0,
            width + 30.0,
            ground::layer::BASE,
            us_render::GRASS,
        );
        let mut scene = Scene::dynamic(length, plot.mesh, Vec3::ZERO);
        let mut mesh = Mesh::default();
        let mut id = 1;
        for plan in 0..architecture::PLANS {
            let x = (plan % 4) as f32 * cell + cell * 0.5;
            let y = (plan / 4) as f32 * cell + cell * 0.5;
            architecture::build(family, plan, 5).emit(&mut mesh, &mut id, x, y, 0.0, 1.0);
        }
        scene.chunks.push(Chunk {
            rev: 0,
            mesh,
            far: None,
            lo: Vec3::ZERO,
            hi: Vec3::new(length, width, 300.0),
        });
        let (w, h) = (960, 640);
        let tall = if family == 10 {
            145.0
        } else if tower {
            60.0
        } else {
            0.0
        };
        let camera = Camera::framing(
            Vec3::new(length * 0.5, width * 0.5, tall),
            if family == 10 {
                400.0
            } else if tower {
                266.0
            } else {
                132.0
            },
            w as f32 / h as f32,
            145.0,
            if tower { 30.0 } else { 56.0 },
        );
        let style = Style {
            supersample: 1,
            ink_width: 1,
            high_detail: true,
            ..Style::default()
        };
        let f = renderer.render_to_rgba(&scene, &camera, &style, w, h);
        let filename = label.to_ascii_lowercase().replace(' ', "-") + ".png";
        image::save_buffer(out.join(filename), &f.rgba, w, h, image::ColorType::Rgba8)
            .expect("write model screenshot");
    }
}
