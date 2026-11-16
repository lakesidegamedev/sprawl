//! `coverview <x> <y> <width_m> <month> <out.png> [preset seed]`: one view for ground cover checks.
use glam::Vec3;
use us_gen::{Preset, generate};
use us_render::{Calendar, Camera, Renderer, Scene, Style};
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (x, y, wm): (f32, f32, f32) = (
        a[0].parse().unwrap(),
        a[1].parse().unwrap(),
        a[2].parse().unwrap(),
    );
    let month: u32 = a[3].parse().unwrap();
    let t = generate(Preset::Grid, 3);
    let mut scene = Scene::from_town(&t);
    if a.get(5).is_some_and(|v| v == "off") {
        let k = us_render::cover::build_all(&t, &us_render::Land::from_town(&t)).len();
        let n = scene.chunks.len();
        scene.chunks.truncate(n - k);
    }
    scene.set_time(1.0);
    scene.set_calendar(Some(Calendar::from_date(month, 15, 13.0)));
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 1000u32);
    let cam = Camera::framing(Vec3::new(x, y, scene.height_at(x, y)), wm, 1.6, 30.0, 38.0);
    let style = Style {
        shadow_size: 4096,
        ink_width: 2,
        supersample: 2,
        ..Style::default()
    };
    let f = r.render_to_rgba(&scene, &cam, &style, w, h);
    eprintln!(
        "chunks drawn {} far {} total {} gpu {:.2}",
        f.stats.chunks_drawn,
        f.stats.chunks_far,
        f.stats.chunks_total,
        f.stats.gpu_wait.as_secs_f32()
    );
    image::save_buffer(&a[4], &f.rgba, f.width, f.height, image::ColorType::Rgba8).unwrap();
}
