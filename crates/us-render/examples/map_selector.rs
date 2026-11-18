//! Example maps for the new-town selector. Pass the preview output directory.
use glam::Vec3;
use std::path::PathBuf;
use us_gen::{Preset, generate};
use us_render::{Camera, Renderer, Scene, Style};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let out = PathBuf::from(args.next().expect("preview output directory"));
    let only = args.next();
    std::fs::create_dir_all(&out).expect("create preview directory");
    let mut render = Renderer::new().expect("graphics adapter");
    let style = Style {
        supersample: 1,
        shadow_size: 2048,
        ink_width: 1,
        high_detail: true,
        ..Style::default()
    };
    for preset in Preset::ALL {
        if only.as_ref().is_some_and(|name| name != preset.name()) {
            continue;
        }
        let town = generate(preset, 41);
        let scene = Scene::from_town(&town);
        let (x, y, extent, yaw) = match preset {
            Preset::Coast => (2500.0, 2050.0, 2900.0, 72.0),
            Preset::Mesa => (2400.0, 1900.0, 3000.0, 182.0),
            Preset::Gorge | Preset::Valley => (2000.0, 2050.0, 3100.0, 160.0),
            Preset::LakeTown => (2100.0, 1950.0, 2600.0, 60.0),
            _ => (town.hall.x as f32, town.hall.y as f32, 2200.0, 205.0),
        };
        let z = scene.land.height(x, y);
        let (w, h) = (640, 360);
        let camera = Camera::framing(Vec3::new(x, y, z), extent, w as f32 / h as f32, yaw, 41.0);
        let frame = render.render_to_rgba(&scene, &camera, &style, w, h);
        image::save_buffer(
            out.join(format!("{}.png", preset.name())),
            &frame.rgba,
            w,
            h,
            image::ColorType::Rgba8,
        )
        .expect("write map screenshot");
    }
}
