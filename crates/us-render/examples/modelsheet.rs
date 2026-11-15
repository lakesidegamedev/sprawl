//! `cargo run --release -p us-render --example modelsheet -- <out_dir>`
//!
//! Draws every model added in 1.1 (shops, houses, towers, industry, civic look-alikes, vehicles,
//! props and tree species) into pictures, a handful per sheet, in the order printed.

use glam::Vec3;
use std::path::PathBuf;
use us_render::fleet::{Prop, Vehicle};
use us_render::kit_more as k;
use us_render::{Camera, Chunk, Mesh, Obj, Region, Renderer, Rng, Scene, Style, ground};

type Maker = Box<dyn Fn(&mut Rng) -> Obj>;

fn sheet(
    r: &mut Renderer,
    dir: &std::path::Path,
    name: &str,
    items: &[(&str, Maker)],
    cell: f32,
    cols: usize,
) {
    sheet_span(r, dir, name, items, cell, cols, 1.0)
}

fn sheet_span(
    r: &mut Renderer,
    dir: &std::path::Path,
    name: &str,
    items: &[(&str, Maker)],
    cell: f32,
    cols: usize,
    span: f32,
) {
    let rows = items.len().div_ceil(cols);
    let (w, h) = (
        1600u32,
        (1600.0 * rows as f32 / cols as f32 * 0.62).max(500.0) as u32,
    );
    let mut scene = Scene::dynamic(cell * cols.max(rows) as f32, Mesh::default(), Vec3::ZERO);
    let mut mesh = Mesh::default();
    let mut id = 1u32;
    for (i, (label, mk)) in items.iter().enumerate() {
        let (cx, cy) = ((i % cols) as f32, (rows - 1 - i / cols) as f32);
        let mut rng = Rng::new(0x51ee7 + i as u64 * 31, 9);
        let o = mk(&mut rng);
        o.emit(
            &mut mesh,
            &mut id,
            cell * (cx + 0.5),
            cell * (cy + 0.5),
            0.0,
            1.0,
        );
        eprintln!("{name} #{i}: {label}");
    }
    let mut g = ground::Ground::default();
    g.rect(
        -20.0,
        -20.0,
        cell * cols as f32 + 40.0,
        cell * rows as f32 + 40.0,
        ground::layer::BASE,
        us_render::GRASS,
    );
    scene.ground = g.mesh;
    scene.chunks.push(Chunk {
        rev: 0,
        mesh,
        far: None,
        lo: Vec3::new(-40.0, -40.0, 0.0),
        hi: Vec3::new(cell * cols as f32 + 40.0, cell * rows as f32 + 40.0, 160.0),
    });
    let centre = Vec3::new(cell * cols as f32 / 2.0, cell * rows as f32 / 2.0, 0.0);
    let cam = Camera::framing(
        centre,
        cell * cols as f32 * span,
        w as f32 / h as f32,
        0.0,
        50.0,
    );
    let style = Style {
        supersample: 2,
        shadow_size: 4096,
        ink_width: 1,
        ..Style::default()
    };
    let f = r.render_to_rgba(&scene, &cam, &style, w, h);
    let path = dir.join(format!("{name}.png"));
    image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8).expect("png");
    eprintln!("wrote {}", path.display());
}

macro_rules! m {
    ($name:expr, $f:expr) => {
        ($name, Box::new($f) as Maker)
    };
}

fn main() {
    let dir: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/modelsheets".into())
        .into();
    std::fs::create_dir_all(&dir).expect("dir");
    let mut r = Renderer::new().expect("adapter");
    let rg = Region::Mixed;
    sheet(
        &mut r,
        &dir,
        "commercial-a",
        &[
            m!("supermarket", move |g: &mut Rng| k::supermarket(rg, g)),
            m!("bank", move |g: &mut Rng| k::bank(rg, g)),
            m!("hotel mid", move |g: &mut Rng| k::hotel_mid(rg, g)),
            m!("cinema", move |g: &mut Rng| k::cinema(rg, g)),
            m!("office park", move |g: &mut Rng| k::office_park(rg, g)),
        ],
        40.0,
        3,
    );
    sheet(
        &mut r,
        &dir,
        "commercial-b",
        &[
            m!("car dealership", move |g: &mut Rng| k::car_dealership(
                rg, g
            )),
            m!("diner", move |g: &mut Rng| k::diner(rg, g)),
            m!("petrol A", move |g: &mut Rng| k::petrol_a(rg, g)),
            m!("petrol B", move |g: &mut Rng| k::petrol_b(rg, g)),
            m!("corner store", move |g: &mut Rng| k::corner_store(rg, g)),
            m!("pharmacy", move |g: &mut Rng| k::pharmacy(rg, g)),
        ],
        36.0,
        3,
    );
    sheet(
        &mut r,
        &dir,
        "residential",
        &[
            m!("townhouse bay", move |g: &mut Rng| k::townhouse_row(
                rg, 0, g
            )),
            m!("townhouse stoop", move |g: &mut Rng| k::townhouse_row(
                rg, 1, g
            )),
            m!("townhouse mansard", move |g: &mut Rng| k::townhouse_row(
                rg, 2, g
            )),
            m!("townhouse balcony", move |g: &mut Rng| k::townhouse_row(
                rg, 3, g
            )),
            m!("midrise", move |g: &mut Rng| k::midrise_balconies(rg, 7, g)),
            m!("duplex", move |g: &mut Rng| k::duplex(rg, g)),
            m!("mobile homes", move |g: &mut Rng| k::mobile_homes(rg, g)),
        ],
        40.0,
        3,
    );
    sheet_span(
        &mut r,
        &dir,
        "towers",
        &[
            m!("hotel tall", move |g: &mut Rng| k::hotel_tall(rg, g)),
            m!("tower slab", move |g: &mut Rng| k::glass_tower(
                rg, 18, 0, g
            )),
            m!("tower stepped", move |g: &mut Rng| k::glass_tower(
                rg, 22, 1, g
            )),
            m!("tower twin", move |g: &mut Rng| k::glass_tower(
                rg, 20, 2, g
            )),
        ],
        50.0,
        4,
        1.9,
    );
    sheet(
        &mut r,
        &dir,
        "industrial",
        &[
            m!("sawtooth", move |g: &mut Rng| k::factory_sawtooth(rg, g)),
            m!("stacks", move |g: &mut Rng| k::factory_stacks(rg, g)),
            m!("tank farm", move |g: &mut Rng| k::tank_farm(rg, g)),
            m!("freight yard", move |g: &mut Rng| k::freight_yard(rg, g)),
            m!("port cranes", move |g: &mut Rng| k::port_cranes(rg, g)),
            m!("recycling", move |g: &mut Rng| k::recycling_centre(rg, g)),
        ],
        36.0,
        3,
    );
    sheet(
        &mut r,
        &dir,
        "civic",
        &[
            m!("school B", move |g: &mut Rng| k::school_b(rg, g)),
            m!("church B", move |g: &mut Rng| k::church_b(rg, g)),
            m!("fire station B", move |g: &mut Rng| k::fire_b(rg, g)),
            m!("water tank B", move |g: &mut Rng| k::water_b(rg, g)),
        ],
        36.0,
        3,
    );
    let veh: Vec<(&str, Maker)> = Vehicle::ALL
        .iter()
        .map(|v| {
            let v = *v;
            (
                v.name(),
                Box::new(move |_: &mut Rng| {
                    let mut o = v.obj();
                    for p in &mut o.polys {
                        if p.color == us_render::instances::MARKER {
                            p.color = [226, 170, 50];
                        }
                    }
                    o
                }) as Maker,
            )
        })
        .collect();
    sheet(&mut r, &dir, "vehicles", &veh, 16.0, 5);
    let props: Vec<(&str, Maker)> = Prop::ALL
        .iter()
        .map(|p| {
            let p = *p;
            (
                p.name(),
                Box::new(move |_: &mut Rng| {
                    let mut o = p.obj();
                    for q in &mut o.polys {
                        if q.color == us_render::instances::MARKER {
                            q.color = [196, 58, 52];
                        }
                    }
                    o
                }) as Maker,
            )
        })
        .collect();
    sheet(&mut r, &dir, "props", &props, 12.0, 5);
    let trees: Vec<(&str, Maker)> = [0.1f32, 0.55, 0.75, 0.9]
        .iter()
        .map(|&roll| {
            (
                "tree",
                Box::new(move |_: &mut Rng| k::tree_species(roll, 2.4, 7.0, us_render::kit::TREE))
                    as Maker,
            )
        })
        .collect();
    sheet(&mut r, &dir, "trees", &trees, 10.0, 4);
}
