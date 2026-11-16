//! `cargo run --release -p us-render --example surfaces -- <out_dir> <prefix>`
//!
//! The same fixed views every time, for before and after pictures of ground, water, light and
//! life: a close street, a mid view with a park and fields, the whole town, the lake shore, and
//! the close street in winter and at night. Prints the frame times too.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{LotKind, Preset, Town, Zone, generate};
use us_render::{Calendar, Camera, Frame, Renderer, Scene, Style};

fn pt(x: f64, y: f64) -> Vec3 {
    Vec3::new(x as f32, y as f32, 0.0)
}

fn centroid(p: &[us_gen::Pt]) -> Vec3 {
    let n = p.len().max(1) as f64;
    pt(
        p.iter().map(|q| q.x).sum::<f64>() / n,
        p.iter().map(|q| q.y).sum::<f64>() / n,
    )
}

fn suburb(t: &Town) -> Vec3 {
    let houses: Vec<_> = t.lots.iter().filter(|l| l.kind == LotKind::House).collect();
    let mut best = (0usize, 0usize);
    for (i, h) in houses.iter().enumerate() {
        if h.centre.dist(t.hall) < 500.0 {
            continue;
        }
        let n = houses
            .iter()
            .filter(|o| o.centre.dist(h.centre) < 60.0)
            .count();
        if n > best.0 {
            best = (n, i);
        }
    }
    pt(houses[best.1].centre.x, houses[best.1].centre.y)
}

fn main() {
    let mut a = std::env::args().skip(1);
    let out: PathBuf = a.next().unwrap_or_else(|| "target/shots".into()).into();
    let prefix = a.next().unwrap_or_else(|| "after".into());
    std::fs::create_dir_all(&out).expect("dir");
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 1000u32);
    let aspect = w as f32 / h as f32;
    let style = Style {
        shadow_size: 4096,
        ink_width: 2,
        supersample: 2,
        ..Style::default()
    };
    let save = |name: &str, f: &Frame| {
        let p = out.join(format!("{prefix}-{name}.png"));
        image::save_buffer(&p, &f.rgba, f.width, f.height, image::ColorType::Rgba8).expect("png");
        eprintln!(
            "{name}: total {:.2}s gpu {:.2}s",
            f.stats.total.as_secs_f32(),
            f.stats.gpu_wait.as_secs_f32()
        );
    };
    let summer = Calendar::from_date(6, 21, 13.0);
    let winter = Calendar::from_date(1, 15, 13.0);
    let night = Calendar::from_date(6, 21, 22.0);

    let grid = generate(Preset::Grid, 3);
    let mut scene = Scene::from_town(&grid);
    scene.set_time(1.0);
    let sub = suburb(&grid);
    let park = grid
        .additions
        .iter()
        .find(|a| a.zone == Zone::Park)
        .map(|a| centroid(&a.polygon))
        .unwrap_or(sub);
    let farm = grid
        .additions
        .iter()
        .find(|a| a.zone == Zone::Farm)
        .map(|a| centroid(&a.polygon))
        .unwrap_or(sub);
    let mid = (park + farm) * 0.5;
    let hall = pt(grid.hall.x, grid.hall.y);
    let street = Camera::framing(sub, 90.0, aspect, 35.0, 28.0);
    let midc = Camera::framing(mid, 420.0, aspect, 30.0, 40.0);
    let parkc = Camera::framing(park, 200.0, aspect, 30.0, 38.0);
    let farmc = Camera::framing(farm, 260.0, aspect, 30.0, 38.0);
    let whole = Camera::framing(
        hall + Vec3::new(0.0, -150.0, 0.0),
        2600.0,
        aspect,
        30.0,
        48.0,
    );

    scene.set_calendar(Some(summer));
    // a field edge: the first corner of the farm, seen from just outside; and open meadow beside it
    let corner = grid
        .additions
        .iter()
        .find(|a| a.zone == Zone::Farm)
        .map(|a| pt(a.polygon[0].x, a.polygon[0].y))
        .unwrap_or(sub);
    let edgec = Camera::framing(pt(3324.0, 968.0), 110.0, aspect, 30.0, 38.0);
    let meadow = corner + (corner - farm).normalize_or_zero() * 45.0;
    let _ = meadow;
    let meadowc = Camera::framing(pt(1158.0, 1485.0), 60.0, aspect, 25.0, 34.0);
    save(
        "fieldedge-summer",
        &r.render_to_rgba(&scene, &edgec, &style, w, h),
    );
    save(
        "rows-summer",
        &r.render_to_rgba(
            &scene,
            &Camera::framing(pt(1543.0, 2705.0), 110.0, aspect, 30.0, 38.0),
            &style,
            w,
            h,
        ),
    );
    save(
        "meadow-summer",
        &r.render_to_rgba(&scene, &meadowc, &style, w, h),
    );
    save(
        "street-summer",
        &r.render_to_rgba(&scene, &street, &style, w, h),
    );
    save(
        "park-summer",
        &r.render_to_rgba(&scene, &parkc, &style, w, h),
    );
    save(
        "farm-summer",
        &r.render_to_rgba(&scene, &farmc, &style, w, h),
    );
    save("mid-summer", &r.render_to_rgba(&scene, &midc, &style, w, h));
    save(
        "whole-summer",
        &r.render_to_rgba(&scene, &whole, &style, w, h),
    );
    scene.set_calendar(Some(winter));
    save(
        "street-winter",
        &r.render_to_rgba(&scene, &street, &style, w, h),
    );
    save(
        "whole-winter",
        &r.render_to_rgba(&scene, &whole, &style, w, h),
    );
    scene.set_calendar(Some(night));
    save(
        "street-night",
        &r.render_to_rgba(&scene, &street, &style, w, h),
    );
    scene.set_calendar(Some(Calendar::from_date(10, 20, 13.0)));
    save(
        "street-autumn",
        &r.render_to_rgba(&scene, &street, &style, w, h),
    );

    let lake = generate(Preset::LakeTown, 2);
    let mut ls = Scene::from_town(&lake);
    ls.set_time(1.0);
    if let Some(wr) = lake.water.first() {
        let c = centroid(wr);
        // the shore point nearest the hall
        let hallp = pt(lake.hall.x, lake.hall.y);
        let shore = wr
            .iter()
            .map(|p| pt(p.x, p.y))
            .min_by(|a, b| a.distance(hallp).total_cmp(&b.distance(hallp)))
            .unwrap_or(c);
        for (n, cal) in [("summer", summer), ("winter", winter)] {
            ls.set_calendar(Some(cal));
            save(
                &format!("lake-{n}"),
                &r.render_to_rgba(
                    &ls,
                    &Camera::framing(shore, 220.0, aspect, 20.0, 35.0),
                    &style,
                    w,
                    h,
                ),
            );
        }
        ls.set_calendar(Some(summer));
        save(
            "lake-wide",
            &r.render_to_rgba(
                &ls,
                &Camera::framing(shore, 900.0, aspect, 20.0, 40.0),
                &style,
                w,
                h,
            ),
        );
    }
}
