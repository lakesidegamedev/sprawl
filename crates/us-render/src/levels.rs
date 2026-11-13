//! Building levels 1 to 5 for every lot kind, in the style of the base kit: a higher level is
//! taller, denser and more detailed. Homes become row houses then small apartment blocks, shops
//! become bigger shops then offices. Also the look of a building site and of a burned-out lot.

use crate::kit::{self, CONC, GLASS, Region, WHITE};
use crate::obj::{Obj, Rgb};
use crate::rng::Rng;
use us_gen::LotKind;

/// Highest building level.
pub const MAX_LEVEL: u8 = 5;

/// A copy of the model stretched along each axis.
pub fn scaled(obj: &Obj, sx: f32, sy: f32, sz: f32) -> Obj {
    let mut o = obj.clone();
    for p in &mut o.polys {
        for v in &mut p.pts {
            v[0] *= sx;
            v[1] *= sy;
            v[2] *= sz;
        }
    }
    for s in &mut o.solids {
        s.centre[0] *= sx;
        s.centre[1] *= sy;
        s.centre[2] *= sz;
    }
    o
}

/// Height of the highest point of the model.
pub fn top_of(obj: &Obj) -> f32 {
    obj.polys
        .iter()
        .flat_map(|p| p.pts.iter().map(|v| v[2]))
        .fold(0.0, f32::max)
}

fn rooftop_units(o: &mut Obj, n: usize, rng: &mut Rng) {
    let b = o.footprint();
    let top = top_of(o);
    let (w, d) = (b[2] - b[0], b[3] - b[1]);
    for i in 0..n {
        let fx = if i % 2 == 0 { 0.28 } else { 0.68 };
        let fy = 0.3 + 0.4 * rng.f32();
        let (cx, cy) = (b[0] + w * fx, b[1] + d * fy);
        let s = (w.min(d) * 0.07).clamp(0.5, 1.4);
        o.bx_top(
            cx - s,
            cy - s,
            top,
            cx + s,
            cy + s,
            top + s * 1.5,
            [150, 148, 144],
            [118, 116, 112],
        );
    }
}

/// Sheds and lamp posts in the front yard, for models with no flat roof to put things on.
fn yard_units(o: &mut Obj, n: usize, rng: &mut Rng) {
    let b = o.footprint();
    let w = b[2] - b[0];
    for i in 0..n {
        let cx = b[0] + w * (0.15 + 0.7 * (i as f32 + rng.f32() * 0.4) / n as f32);
        let cy = b[1] - 2.2;
        if i % 2 == 0 {
            o.bx_top(
                cx - 0.9,
                cy - 0.9,
                0.0,
                cx + 0.9,
                cy + 0.9,
                2.1,
                [186, 170, 150],
                [150, 84, 52],
            );
        } else {
            o.bx(
                cx - 0.1,
                cy - 0.1,
                0.0,
                cx + 0.1,
                cy + 0.1,
                3.6,
                [90, 94, 100],
            );
            o.bx(
                cx - 0.35,
                cy - 0.35,
                3.6,
                cx + 0.35,
                cy + 0.35,
                3.9,
                [236, 226, 170],
            );
        }
    }
}

/// A low wing against the east side.
fn annex(o: &mut Obj, rng: &mut Rng, tall: f32) {
    let b = o.footprint();
    let top = top_of(o);
    let col = rng.pick(&[CONC, [210, 200, 184], [186, 170, 150]]);
    let (w, d) = (b[2] - b[0], b[3] - b[1]);
    let (x0, x1) = (b[2], b[2] + (w * 0.4).max(3.0));
    let (y0, y1) = (b[1] + d * 0.1, b[1] + d * 0.8);
    let h = (top * tall).clamp(3.0, 14.0);
    o.bx_top(x0, y0, 0.0, x1, y1, h, col, [120, 118, 114]);
    let n = (((y1 - y0) / 3.5) as usize).max(1);
    for i in 0..n {
        let cy = y0 + (i as f32 + 0.5) * (y1 - y0) / n as f32;
        o.win_e(
            cy - 0.8,
            cy + 0.8,
            h * 0.4,
            (h * 0.4 + 1.5).min(h - 0.4),
            x1,
            GLASS,
        );
    }
}

fn generic(region: Region, kind: LotKind, level: u8, rng: &mut Rng) -> Obj {
    let base = kit::build_lot_in(region, kind, rng);
    let ground_kind = matches!(
        kind,
        LotKind::Parking | LotKind::Park | LotKind::Sports | LotKind::Farm | LotKind::Water
    );
    let (sxy, sz) = match level {
        1 => (0.82, 0.85),
        2 => (1.0, 1.0),
        3 => (1.0, 1.0),
        4 => (1.0, 1.08),
        _ => (1.08, 1.18),
    };
    let sz = if ground_kind { 1.0 } else { sz };
    let mut o = scaled(&base, sxy, sxy, sz);
    let sloped = matches!(
        kind,
        LotKind::Church | LotKind::Farm | LotKind::Water | LotKind::Workshop
    );
    if level >= 3 {
        let n = if level == 3 { 2 } else { 4 };
        if sloped || ground_kind {
            yard_units(&mut o, n, rng);
        } else {
            rooftop_units(&mut o, n, rng);
        }
    }
    let wide = {
        let b = o.footprint();
        b[2] - b[0] > 34.0
    };
    if level >= 4 && !ground_kind && !wide {
        annex(&mut o, rng, 0.55);
    }
    if level >= 5 {
        // a roof sign mast and a flag
        let b = o.footprint();
        let top = top_of(&o);
        let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
        o.bx(
            cx - 0.12,
            cy - 0.12,
            top,
            cx + 0.12,
            cy + 0.12,
            top + 4.5,
            [90, 94, 100],
        );
        o.bx(
            cx + 0.12,
            cy - 0.05,
            top + 3.2,
            cx + 2.0,
            cy + 0.05,
            top + 4.4,
            [196, 58, 52],
        );
    }
    if kind == LotKind::Workshop && level >= 3 {
        for (x, h) in [(-10.0f32, 17.0f32), (-6.0, 14.0)] {
            o.prism(x * sxy, 4.0 * sxy, 0.0, h, 1.1, [160, 90, 70], 8, None);
        }
    }
    o
}

/// Where smoke rises from a model of this kind and level: local x, y and the chimney top z.
pub fn chimneys(kind: LotKind, level: u8) -> Vec<[f32; 3]> {
    match kind {
        LotKind::Workshop => {
            let s = match level {
                1 => 0.82,
                5 => 1.08,
                _ => 1.0,
            };
            let sz = match level {
                1 => 0.85,
                4 => 1.08,
                5 => 1.18,
                _ => 1.0,
            };
            let mut v = vec![[11.0 * s, 4.0 * s, 20.0 * sz]];
            if level >= 3 {
                v.push([-10.0 * s, 4.0 * s, 17.0]);
                v.push([-6.0 * s, 4.0 * s, 14.0]);
            }
            v
        }
        _ => Vec::new(),
    }
}

/// The model for one lot at a building level from 1 to 5, from its kind and a seeded RNG, in no
/// particular region.
pub fn build_lot_level(kind: LotKind, level: u8, rng: &mut Rng) -> Obj {
    build_lot_level_in(Region::Mixed, kind, level, rng)
}

/// The model for one lot at a building level from 1 to 5 in a region.
pub fn build_lot_level_in(region: Region, kind: LotKind, level: u8, rng: &mut Rng) -> Obj {
    let level = level.clamp(1, MAX_LEVEL);
    match kind {
        LotKind::House => match level {
            1 => scaled(&kit::house(region, rng), 0.8, 0.8, 0.88),
            2 => kit::house(region, rng),
            3 => kit::two_storey_house(region, rng),
            4 => {
                let n = 2 + rng.below(2);
                kit::row_houses(region, n, 0, rng)
            }
            _ => {
                let o = kit::apartments(region, 4, rng);
                scaled(&o, 0.7, 0.75, 1.0)
            }
        },
        LotKind::Row => match level {
            1 => kit::row_houses(region, 2, 2, rng),
            2 => {
                let n = 2 + rng.below(3);
                kit::row_houses(region, n, 0, rng)
            }
            3 => kit::row_houses(region, 3, 3, rng),
            4 => scaled(&kit::apartments(region, 4, rng), 0.75, 0.8, 1.0),
            _ => scaled(&kit::apartments(region, 6, rng), 0.8, 0.85, 1.0),
        },
        LotKind::Shop => match level {
            1 => scaled(&kit::shop_floors(region, 1, rng), 0.8, 0.8, 0.9),
            2 => kit::shop_floors(region, 2, rng),
            3 => kit::shop_floors(region, 3, rng),
            4 => scaled(&kit::office(region, 4, rng), 0.8, 0.8, 1.0),
            _ => scaled(&kit::office(region, 8, rng), 0.82, 0.82, 1.0),
        },
        LotKind::Apartments => {
            let floors = [3usize, 4, 5, 7, 9][level as usize - 1];
            let mut o = kit::apartments(region, floors, rng);
            if level >= 4 {
                let h = top_of(&o);
                o.bx(-5.0, -3.0, h, 5.0, 3.0, h + 1.2, [120, 150, 110]);
            }
            o
        }
        LotKind::Office => {
            let floors = [3usize, 5, 7, 10, 14][level as usize - 1];
            let mut o = kit::office(region, floors, rng);
            if level >= 5 {
                let h = top_of(&o);
                o.pyramid(0.0, 0.0, h, 4.2, 5.0, [110, 120, 140], 4, 45.0);
            }
            o
        }
        _ => generic(region, kind, level, rng),
    }
}

fn mix(c: Rgb, to: Rgb, k: f32) -> Rgb {
    let m = |a: u8, b: u8| {
        (a as f32 + (b as f32 - a as f32) * k)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [m(c[0], to[0]), m(c[1], to[1]), m(c[2], to[2])]
}

/// A building site: the new model rising to `progress` (0 to 1) inside a hoarding fence, with a
/// tower crane while it is still going up.
pub fn building_site(new: &Obj, progress: f32) -> Obj {
    let p = progress.clamp(0.0, 1.0);
    let mut o = crate::live::construction(new, p);
    let b = new.footprint();
    let (x0, y0, x1, y1) = (b[0] - 1.5, b[1] - 1.5, b[2] + 1.5, b[3] + 1.5);
    let fence: Rgb = [232, 150, 52];
    let post = 0.15;
    o.slab(x0, y0, 0.0, x1, y0 + post, 1.1, fence);
    o.slab(x0, y1 - post, 0.0, x1, y1, 1.1, fence);
    o.slab(x0, y0, 0.0, x0 + post, y1, 1.1, fence);
    o.slab(x1 - post, y0, 0.0, x1, y1, 1.1, fence);
    if p < 0.95 {
        let top = top_of(new).max(6.0) + 5.0;
        let (cx, cy) = (x1 - 0.5, y0 + 0.5);
        o.bx(
            cx - 0.3,
            cy - 0.3,
            0.0,
            cx + 0.3,
            cy + 0.3,
            top,
            [232, 190, 60],
        );
        o.bx(
            cx - (x1 - x0) * 0.9,
            cy - 0.25,
            top,
            cx + 2.0,
            cy + 0.25,
            top + 0.5,
            [232, 190, 60],
        );
        o.bx(
            cx - 3.0,
            cy - 0.5,
            top - 1.4,
            cx - 2.0,
            cy + 0.5,
            top,
            [90, 94, 100],
        );
    }
    o
}

/// A burned-out lot: the old model charred and gutted to a stump, with a few glowing-orange
/// hot spots drawn as flat tiles (no glow).
pub fn burned(old: &Obj, rng: &mut Rng) -> Obj {
    let mut o = crate::live::construction(old, 0.0);
    o.polys.retain(|_| true);
    for p in &mut o.polys {
        if p.flat {
            p.color = [20, 18, 22];
        } else {
            let g = ((p.color[0] as u32 + p.color[1] as u32 + p.color[2] as u32) / 3) as f32;
            let d = (g * 0.22 + 22.0).round().clamp(24.0, 70.0) as u8;
            p.color = mix(
                [d, d.saturating_sub(4), d.saturating_sub(6)],
                [74, 56, 48],
                0.25,
            );
        }
    }
    let b = old.footprint();
    let top = top_of(&o);
    for _ in 0..3 {
        let cx = b[0] + (b[2] - b[0]) * rng.range(0.2, 0.8);
        let cy = b[1] + (b[3] - b[1]) * rng.range(0.2, 0.8);
        let s = rng.range(0.5, 1.0);
        o.poly(
            &[
                [cx - s, cy - s, top + 0.05],
                [cx + s, cy - s, top + 0.05],
                [cx + s, cy + s, top + 0.05],
                [cx - s, cy + s, top + 0.05],
            ],
            [232, 110, 40],
            false,
            true,
            true,
        );
    }
    // a fallen beam
    let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    o.slab(
        cx - 2.5,
        cy - 0.3,
        0.0,
        cx + 2.5,
        cy + 0.3,
        0.5,
        [52, 40, 36],
    );
    let _ = WHITE;
    o
}

fn poly_area(pts: &[[f32; 3]]) -> f32 {
    let mut a = 0.0;
    for i in 0..pts.len() {
        let (p, q) = (pts[i], pts[(i + 1) % pts.len()]);
        a += p[0] * q[1] - q[0] * p[1];
    }
    a.abs() / 2.0
}

/// The body of a model as one block: (min x, min y, max x, max y, height). Small things poking
/// above the roof (chimneys, masts) do not count toward the height.
pub fn massing(obj: &Obj) -> [f32; 5] {
    let b = obj.footprint();
    let mut h = 0.0f32;
    for p in &obj.polys {
        if p.pts.len() >= 3 && !p.flat {
            let n = crate::obj::newell(&p.pts);
            if n[2] > 0.9 && poly_area(&p.pts) >= 6.0 {
                h = h.max(p.pts[0][2]);
            } else if p.roof && n[2] > 0.2 && poly_area(&p.pts) >= 6.0 {
                h = h.max(p.pts.iter().map(|v| v[2]).fold(0.0, f32::max));
            }
        }
    }
    [b[0], b[1], b[2], b[3], h]
}
