//! Plan-view PNG of a town, in the colours of the approved mockups.

use crate::font;
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform,
};
use us_gen::{LotKind, Pt, RoadClass, Town, Zone};

const PX: f32 = 1400.0;
const TOP: f32 = 86.0;
const FOOT: f32 = 46.0;

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgba8(r, g, b, 255)
}

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

fn road_col(c: RoadClass) -> Color {
    match c {
        RoadClass::Highway => rgb(236, 150, 60),
        RoadClass::Arterial => rgb(246, 214, 110),
        RoadClass::Collector => rgb(250, 250, 250),
        RoadClass::Local => rgb(214, 216, 222),
        RoadClass::Alley => rgb(176, 178, 186),
        RoadClass::Rail => rgb(128, 92, 66),
    }
}

fn zone_col(z: Zone) -> Option<Color> {
    Some(match z {
        Zone::Core => rgb(212, 196, 184),
        Zone::Inner => rgb(200, 214, 184),
        Zone::Outer => rgb(190, 214, 170),
        Zone::Industry => rgb(196, 190, 206),
        Zone::Mall => rgb(222, 196, 196),
        Zone::Park => rgb(120, 182, 100),
        Zone::Campus => rgb(214, 206, 176),
        Zone::Farm => return None,
    })
}

fn lot_col(k: LotKind) -> Color {
    match k {
        LotKind::House => rgb(120, 96, 86),
        LotKind::Row => rgb(140, 84, 70),
        LotKind::Shop => rgb(160, 70, 60),
        LotKind::Apartments => rgb(110, 100, 120),
        LotKind::Office => rgb(80, 100, 140),
        LotKind::Parking => rgb(130, 132, 140),
        LotKind::Strip => rgb(170, 90, 80),
        LotKind::BigBox => rgb(150, 80, 80),
        LotKind::Warehouse => rgb(120, 120, 140),
        LotKind::Workshop => rgb(110, 110, 130),
        LotKind::Gas => rgb(60, 120, 180),
        LotKind::FastFood => rgb(200, 80, 60),
        LotKind::Church => rgb(150, 140, 120),
        LotKind::School => rgb(200, 130, 80),
        LotKind::Sports | LotKind::Park => rgb(90, 160, 80),
        LotKind::Farm => rgb(170, 60, 50),
        LotKind::Fire => rgb(180, 60, 50),
        LotKind::Water => rgb(140, 150, 160),
    }
}

fn text(pm: &mut Pixmap, s: &str, x: f32, y: f32, scale: f32, c: Color) {
    let p = paint(c);
    for (px, py) in font::pixels(s) {
        if let Some(r) = Rect::from_xywh(x + px as f32 * scale, y + py as f32 * scale, scale, scale)
        {
            pm.fill_rect(r, &p, Transform::identity(), None);
        }
    }
}

pub fn plan_png(town: &Town, caption: &str, sub: &str) -> Pixmap {
    let size = town.size as f32;
    let sc = PX / size;
    let h_px = (size - 0.0) * sc;
    let mut pm = Pixmap::new(PX as u32, (h_px + TOP + FOOT) as u32).expect("pixmap");
    pm.fill(rgb(34, 37, 46));
    let map = |p: Pt| -> (f32, f32) { (p.x as f32 * sc, TOP + (size - p.y as f32) * sc) };
    let poly_path = |ring: &[Pt]| {
        let mut pb = PathBuilder::new();
        for (i, p) in ring.iter().enumerate() {
            let (x, y) = map(*p);
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        pb.close();
        pb.finish()
    };
    let line_path = |line: &[Pt]| {
        let mut pb = PathBuilder::new();
        for (i, p) in line.iter().enumerate() {
            let (x, y) = map(*p);
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        pb.finish()
    };
    // ground
    if let Some(r) = Rect::from_xywh(0.0, TOP, PX, h_px) {
        pm.fill_rect(r, &paint(rgb(150, 186, 110)), Transform::identity(), None);
    }
    let fields = [
        rgb(176, 196, 104),
        rgb(196, 184, 110),
        rgb(150, 186, 100),
        rgb(204, 196, 140),
        rgb(132, 170, 90),
    ];
    for (i, a) in town.additions.iter().enumerate() {
        let col = if a.zone == Zone::Farm {
            Some(fields[(i * 7 + (a.distance as usize)) % fields.len()])
        } else {
            zone_col(a.zone)
        };
        if let (Some(c), Some(path)) = (col, poly_path(&a.polygon)) {
            pm.fill_path(
                &path,
                &paint(c),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
            if a.zone == Zone::Farm {
                let st = Stroke {
                    width: 2.0,
                    ..Stroke::default()
                };
                pm.stroke_path(
                    &path,
                    &paint(rgb(150, 186, 110)),
                    &st,
                    Transform::identity(),
                    None,
                );
            }
        }
    }
    for w in &town.water {
        if let Some(path) = poly_path(w) {
            pm.fill_path(
                &path,
                &paint(rgb(66, 136, 200)),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    let order = [
        RoadClass::Rail,
        RoadClass::Alley,
        RoadClass::Local,
        RoadClass::Collector,
        RoadClass::Arterial,
        RoadClass::Highway,
    ];
    for pass in 0..2 {
        for cls in order {
            for r in town.roads.iter().filter(|r| r.class == cls) {
                let Some(path) = line_path(&r.points) else {
                    continue;
                };
                let w = ((cls.half_width() * 2.0) as f32 * sc).max(1.0)
                    + if pass == 0 { 2.0 } else { 0.0 };
                let st = Stroke {
                    width: w,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Stroke::default()
                };
                let c = if pass == 0 {
                    rgb(40, 42, 50)
                } else {
                    road_col(cls)
                };
                pm.stroke_path(&path, &paint(c), &st, Transform::identity(), None);
            }
        }
        if pass == 0 {
            // bulbs sit under the road fill so their outline merges with the street
            for c in &town.cul_de_sacs {
                let (x, y) = map(c.pos);
                if let Some(p) = PathBuilder::from_circle(x, y, 9.0 * sc + 1.0) {
                    pm.fill_path(
                        &p,
                        &paint(rgb(40, 42, 50)),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
    }
    for c in &town.cul_de_sacs {
        let (x, y) = map(c.pos);
        if let Some(p) = PathBuilder::from_circle(x, y, 9.0 * sc) {
            pm.fill_path(
                &p,
                &paint(road_col(RoadClass::Local)),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    for l in &town.lots {
        let rr = ((l.radius * 0.7) as f32 * sc).max(1.0);
        let (x, y) = map(l.centre);
        if let Some(r) = Rect::from_xywh(x - rr, y - rr, rr * 2.0, rr * 2.0) {
            pm.fill_rect(r, &paint(lot_col(l.kind)), Transform::identity(), None);
        }
    }
    let (hx, hy) = map(town.hall);
    if let Some(r) = Rect::from_xywh(hx - 5.0, hy - 5.0, 10.0, 10.0) {
        pm.fill_rect(r, &paint(rgb(30, 30, 30)), Transform::identity(), None);
    }
    if let Some(r) = Rect::from_xywh(hx - 4.0, hy - 4.0, 8.0, 8.0) {
        pm.fill_rect(r, &paint(rgb(250, 250, 250)), Transform::identity(), None);
    }
    if let Some(r) = Rect::from_xywh(0.0, 0.0, PX, TOP - 6.0) {
        pm.fill_rect(r, &paint(rgb(20, 22, 28)), Transform::identity(), None);
    }
    text(&mut pm, caption, 18.0, 12.0, 4.0, rgb(244, 244, 244));
    text(&mut pm, sub, 18.0, 50.0, 2.0, rgb(200, 204, 212));
    let mut lx = 18.0;
    let ly = TOP + h_px + 12.0;
    for (label, c) in [
        ("HIGHWAY", RoadClass::Highway),
        ("ARTERIAL", RoadClass::Arterial),
        ("COLLECTOR", RoadClass::Collector),
        ("LOCAL STREET", RoadClass::Local),
        ("ALLEY", RoadClass::Alley),
        ("RAILROAD", RoadClass::Rail),
    ] {
        let thick = if matches!(c, RoadClass::Highway | RoadClass::Arterial) {
            10.0
        } else {
            6.0
        };
        if let Some(r) = Rect::from_xywh(lx, ly + 4.0, 34.0, thick) {
            pm.fill_rect(r, &paint(road_col(c)), Transform::identity(), None);
        }
        text(&mut pm, label, lx + 42.0, ly, 2.0, rgb(220, 222, 228));
        lx += 42.0 + font::width(label) as f32 * 2.0 + 24.0;
    }
    pm
}
