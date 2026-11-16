//! Frame time with and without ground cover chunks (Grid 3): whole town and close views.
use glam::Vec3;
use us_gen::{Preset, generate};
use us_render::{Calendar, Camera, Renderer, Scene, Style, cover};
fn main() {
    let t = generate(Preset::Grid, 3);
    let land = us_render::Land::from_town(&t);
    let k = cover::build_all(&t, &land).len();
    let mut scene = Scene::from_town(&t);
    scene.set_time(1.0);
    scene.set_calendar(Some(Calendar::from_date(6, 15, 13.0)));
    let mut r = Renderer::new().expect("adapter");
    let style = Style {
        shadow_size: 4096,
        ink_width: 2,
        supersample: 1,
        ..Style::default()
    };
    let hall = Vec3::new(t.hall.x as f32, t.hall.y as f32, 0.0);
    let only = std::env::args().nth(1);
    let views: Vec<(&str, Camera)> = vec![
        (
            "whole",
            Camera::framing(Vec3::new(2048.0, 1900.0, 0.0), 4200.0, 1.6, 30.0, 48.0),
        ),
        ("wide", Camera::framing(hall, 1500.0, 1.6, 30.0, 45.0)),
        (
            "mid",
            Camera::framing(Vec3::new(1543.0, 2705.0, 0.0), 420.0, 1.6, 30.0, 40.0),
        ),
        (
            "close",
            Camera::framing(Vec3::new(3324.0, 968.0, 0.0), 110.0, 1.6, 30.0, 38.0),
        ),
    ];
    let total = scene.chunks.len();
    let mut tris_on = 0usize;
    for c in &scene.chunks[total - k..] {
        tris_on += c.mesh.triangles();
    }
    eprintln!("cover chunks {k}, triangles {tris_on}");
    for pass in ["on", "off"] {
        if pass == "off" {
            scene.chunks.truncate(total - k);
        }
        for (n, cam) in views
            .iter()
            .filter(|v| only.as_deref().is_none_or(|o| o == v.0))
        {
            let mut best = f32::MAX;
            for _ in 0..5 {
                let f = r.render_to_rgba(&scene, cam, &style, 1280, 800);
                best = best.min(f.stats.gpu_wait.as_secs_f32());
            }
            eprintln!("{pass} {n}: {best:.2}s");
        }
    }
}
