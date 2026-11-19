//! Every tree and bush model the game can place, side by side, to spot odd shapes.
use glam::Vec3;
use std::path::PathBuf;
use us_render::{Camera, Chunk, Mesh, Renderer, Scene, Style, ground, kit, kit_more};

fn main() {
    let out = PathBuf::from(std::env::args_os().nth(1).expect("output png"));
    let mut renderer = Renderer::new().expect("graphics adapter");
    let mut plot = ground::Ground::default();
    plot.rect(
        -20.0,
        -30.0,
        150.0,
        30.0,
        ground::layer::BASE,
        us_render::GRASS,
    );
    let mut scene = Scene::dynamic(130.0, plot.mesh, Vec3::ZERO);
    let mut mesh = Mesh::default();
    let mut id = 1;
    let col = kit::TREE;
    let mut x = 0.0;
    // rolls pick the species: kit::tree, poplar, oak, pine
    for roll in [0.2, 0.55, 0.62, 0.75, 0.8, 0.95] {
        kit_more::tree_species(roll, 3.0, 10.0, col).emit(&mut mesh, &mut id, x, 0.0, 0.0, 1.0);
        x += 12.0;
    }
    for (i, o) in us_render::cover::bush_sheet().iter().enumerate() {
        o.emit(&mut mesh, &mut id, 5.0 + i as f32 * 6.0, 20.0, 0.0, 1.0);
    }
    scene.chunks.push(Chunk {
        rev: 0,
        mesh,
        far: None,
        lo: Vec3::ZERO,
        hi: Vec3::new(130.0, 30.0, 30.0),
    });
    let (w, h) = (1200, 500);
    let camera = Camera::framing(
        Vec3::new(35.0, 10.0, 4.0),
        75.0,
        w as f32 / h as f32,
        145.0,
        30.0,
    );
    let style = Style {
        supersample: 1,
        ink_width: 1,
        high_detail: true,
        ..Style::default()
    };
    let f = renderer.render_to_rgba(&scene, &camera, &style, w, h);
    image::save_buffer(out, &f.rgba, w, h, image::ColorType::Rgba8).expect("write");
}
