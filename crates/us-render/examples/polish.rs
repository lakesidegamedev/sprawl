//! `cargo run --release -p us-render --example polish -- [out_dir] [part]`
//!
//! Preview 3 polish pictures: the civic models, night lamps, seasons, winter roads and a big
//! town on the far form. Parts: `civic`, `night`, `seasons`.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{LotKind, Preset, Town, generate};
use us_render::{
    ALL_CIVIC, Calendar, Camera, Chunk, Frame, Mesh, Renderer, Rng, Scene, Style, build_civic,
};

const W: u32 = 1600;
const H: u32 = 1000;

struct Out {
    dir: PathBuf,
}

impl Out {
    fn save(&self, name: &str, f: &Frame) {
        let path = self.dir.join(format!("{name}.png"));
        image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
            .expect("write png");
        eprintln!("  {name}: {:.2}s", f.stats.total.as_secs_f32());
    }
}

fn style(ink: u32, ss: u32) -> Style {
    Style {
        supersample: ss,
        shadow_size: 4096,
        ink_width: ink,
        ..Style::default()
    }
}

fn civic(r: &mut Renderer, out: &Out) {
    let cell = 84.0f32;
    let n = ALL_CIVIC.len() as f32;
    let mut scene = Scene::dynamic(cell * (n + 1.0), Mesh::default(), Vec3::ZERO);
    let mut mesh = Mesh::default();
    let mut id = 1u32;
    for (i, &m) in ALL_CIVIC.iter().enumerate() {
        let mut rng = Rng::new(7, i as u64);
        build_civic(m, &mut rng).emit(
            &mut mesh,
            &mut id,
            cell * (i as f32 + 0.5),
            cell * 0.5,
            0.0,
            1.0,
        );
    }
    let mut g = us_render::ground::Ground::default();
    g.rect(
        -50.0,
        -50.0,
        cell * (n + 1.0),
        cell * 2.0,
        us_render::ground::layer::BASE,
        us_render::GRASS,
    );
    scene.ground = g.mesh;
    scene.chunks.push(Chunk {
        rev: 0,
        mesh,
        far: None,
        lo: Vec3::new(-60.0, -60.0, 0.0),
        hi: Vec3::new(cell * (n + 1.0), cell * 2.0, 90.0),
    });
    let aspect = W as f32 / H as f32;
    let cam = Camera::framing(
        Vec3::new(cell * n / 2.0, cell * 0.5, 0.0),
        cell * n + 10.0,
        aspect,
        0.0,
        50.0,
    );
    out.save(
        "civic-sheet",
        &r.render_to_rgba(&scene, &cam, &style(1, 2), W, H),
    );
    for (i, &m) in ALL_CIVIC.iter().enumerate() {
        let c = Camera::framing(
            Vec3::new(cell * (i as f32 + 0.5), cell * 0.5, 4.0),
            90.0,
            aspect,
            35.0,
            38.0,
        );
        out.save(
            &format!("civic-{}", m.name().replace(' ', "-")),
            &r.render_to_rgba(&scene, &c, &style(2, 2), W, H),
        );
    }
}

fn town_scene() -> (Town, Scene, Vec3) {
    let town = generate(Preset::Grid, 1);
    let scene = Scene::from_town(&town);
    let hall = Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0);
    (town, scene, hall)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir: PathBuf = args
        .first()
        .map_or("target/polish".into(), |s| s.clone())
        .into();
    let only = args.get(1).cloned();
    std::fs::create_dir_all(&dir).expect("create output dir");
    let mut r = Renderer::new().expect("adapter");
    eprintln!("adapter: {:?}", r.adapter());
    let out = Out { dir };
    let want = |n: &str| only.as_deref().is_none_or(|o| o == n);
    if want("civic") {
        civic(&mut r, &out);
    }
    if want("night") || want("seasons") {
        let (town, mut scene, hall) = town_scene();
        let aspect = W as f32 / H as f32;
        let _ = (&town, LotKind::House, Calendar::from_date(6, 15, 12.0));
        let down = Camera::framing(hall, 260.0, aspect, 45.0, 40.0);
        let _ = (&mut scene, down);
    }
}
