//! Plan view with traffic: street width by volume, colour by volume over capacity.

use crate::font;
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform,
};
use us_gen::{Pt, RoadClass, Town};
use us_grow::Evaluation;

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

fn ramp(vc: f64) -> Color {
    if vc < 0.4 {
        rgb(150, 170, 200)
    } else if vc < 0.7 {
        rgb(80, 180, 110)
    } else if vc < 1.0 {
        rgb(240, 214, 70)
    } else if vc < 1.5 {
        rgb(240, 140, 40)
    } else {
        rgb(220, 40, 40)
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

pub fn traffic_png(
    town: &Town,
    ev: &Evaluation,
    caption: &str,
    sub: &str,
    links: &[(Pt, Pt)],
) -> Pixmap {
    let size = town.size as f32;
    let sc = PX / size;
    let h_px = size * sc;
    let mut pm = Pixmap::new(PX as u32, (h_px + TOP + FOOT) as u32).expect("pixmap");
    pm.fill(rgb(34, 37, 46));
    let map = |p: Pt| -> (f32, f32) { (p.x as f32 * sc, TOP + (size - p.y as f32) * sc) };
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
    let stroke = |pm: &mut Pixmap, line: &[Pt], w: f32, c: Color| {
        if let Some(path) = line_path(line) {
            let st = Stroke {
                width: w,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            pm.stroke_path(&path, &paint(c), &st, Transform::identity(), None);
        }
    };
    if let Some(r) = Rect::from_xywh(0.0, TOP, PX, h_px) {
        pm.fill_rect(r, &paint(rgb(222, 224, 216)), Transform::identity(), None);
    }
    for w in &town.water {
        let mut pb = PathBuilder::new();
        for (i, p) in w.iter().enumerate() {
            let (x, y) = map(*p);
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(
                &path,
                &paint(rgb(150, 190, 225)),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    for l in &town.lots {
        let rr = ((l.radius * 0.7) as f32 * sc).max(0.8);
        let (x, y) = map(l.centre);
        if let Some(r) = Rect::from_xywh(x - rr, y - rr, rr * 2.0, rr * 2.0) {
            pm.fill_rect(r, &paint(rgb(196, 190, 184)), Transform::identity(), None);
        }
    }
    // alleys and rail, faint
    for r in &town.roads {
        match r.class {
            RoadClass::Alley => stroke(&mut pm, &r.points, 0.8, rgb(190, 192, 198)),
            RoadClass::Rail => stroke(&mut pm, &r.points, 1.6, rgb(128, 92, 66)),
            _ => {}
        }
    }
    // streets: dark casing, then volume width in the congestion colour
    let order: Vec<usize> = {
        let mut o: Vec<usize> = (0..ev.net.streets.edges.len()).collect();
        o.sort_by(|&a, &b| {
            ev.assignment.edge_vc[a]
                .total_cmp(&ev.assignment.edge_vc[b])
                .then(a.cmp(&b))
        });
        o
    };
    for (a, b) in links {
        stroke(&mut pm, &[*a, *b], 9.0, rgb(40, 200, 230));
    }
    for pass in 0..2 {
        for &i in &order {
            let e = &ev.net.streets.edges[i];
            let vol = ev.assignment.edge_volume[i] as f32;
            let cw = (e.class.half_width() * 2.0) as f32 * sc * 0.8;
            let w = (1.2 + vol / 450.0).min(9.0).max(cw);
            if pass == 0 {
                stroke(&mut pm, &e.pts, w + 1.6, rgb(50, 52, 60));
            } else {
                stroke(&mut pm, &e.pts, w, ramp(ev.assignment.edge_vc[i]));
            }
        }
    }
    let (hx, hy) = map(town.hall);
    if let Some(r) = Rect::from_xywh(hx - 5.0, hy - 5.0, 10.0, 10.0) {
        pm.fill_rect(r, &paint(rgb(30, 30, 30)), Transform::identity(), None);
    }
    if let Some(r) = Rect::from_xywh(0.0, 0.0, PX, TOP - 6.0) {
        pm.fill_rect(r, &paint(rgb(20, 22, 28)), Transform::identity(), None);
    }
    text(&mut pm, caption, 18.0, 12.0, 4.0, rgb(244, 244, 244));
    text(&mut pm, sub, 18.0, 50.0, 2.0, rgb(200, 204, 212));
    let mut lx = 18.0;
    let ly = TOP + h_px + 12.0;
    for (label, c) in [
        ("UNDER 40 PERCENT", ramp(0.2)),
        ("40 TO 70", ramp(0.5)),
        ("70 TO 100", ramp(0.8)),
        ("OVER CAPACITY", ramp(1.2)),
        ("OVER 150", ramp(2.0)),
    ] {
        if let Some(r) = Rect::from_xywh(lx, ly + 4.0, 34.0, 8.0) {
            pm.fill_rect(r, &paint(c), Transform::identity(), None);
        }
        text(&mut pm, label, lx + 42.0, ly, 2.0, rgb(220, 222, 228));
        lx += 42.0 + font::width(label) as f32 * 2.0 + 24.0;
    }
    if !links.is_empty() {
        if let Some(r) = Rect::from_xywh(lx, ly + 4.0, 34.0, 8.0) {
            pm.fill_rect(r, &paint(rgb(40, 200, 230)), Transform::identity(), None);
        }
        text(
            &mut pm,
            "NEW CONNECTOR",
            lx + 42.0,
            ly,
            2.0,
            rgb(220, 222, 228),
        );
    }
    pm
}
