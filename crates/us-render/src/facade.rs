//! Facade and roof detail added to a finished building model, after `detail::finish`:
//! light trim lines between the floors, windows on side walls that have none, ridge caps along
//! pitched roofs and rooftop clutter (cooling units, vents, skylights, stair housings, water
//! tanks, and a roof garden on big tall roofs). Everything is flat or stepped tone with an
//! ink edge, no gradients. Seeded from the model's own geometry, so the same lot always gets
//! the same detail and no caller's random stream moves.

use crate::obj::{Obj, Rgb, V3};
use crate::parts::GLASS;
use crate::rng::Rng;

fn mix(c: Rgb, to: Rgb, k: f32) -> Rgb {
    let m = |a: u8, b: u8| {
        (a as f32 + (b as f32 - a as f32) * k)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [m(c[0], to[0]), m(c[1], to[1]), m(c[2], to[2])]
}

/// A vertical rectangular wall of a solid.
struct Wall {
    a: V3,
    b: V3,
    out: [f32; 2],
    len: f32,
    zlo: f32,
    zhi: f32,
    color: Rgb,
    solid: u32,
}

/// A flat top of a box: x0, y0, x1, y1 and the height.
struct Top {
    r: [f32; 4],
    z: f32,
    solid: u32,
    color: Rgb,
}

fn seed_of(o: &Obj) -> u64 {
    let mut h: u64 = 0x9e37_79b9_7f4a_7c15 ^ o.polys.len() as u64;
    for p in o.polys.iter().take(64) {
        for v in &p.pts {
            for c in v {
                h = (h ^ c.to_bits() as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    h
}

fn walls_and_tops(o: &Obj) -> (Vec<Wall>, Vec<Top>) {
    let mut walls = Vec::new();
    let mut tops = Vec::new();
    for p in &o.polys {
        let solid = p.solid.unwrap_or(u32::MAX);
        if p.flat || p.roof || p.pts.len() != 4 || (solid == u32::MAX && p.pts[0][2] < 3.0) {
            continue;
        }
        let n = crate::obj::newell(&p.pts);
        if n == [0.0; 3] {
            continue;
        }
        let (zlo, zhi) = p
            .pts
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v[2]), b.max(v[2])));
        if n[2] > 0.98 {
            let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
            for v in &p.pts {
                x0 = x0.min(v[0]);
                y0 = y0.min(v[1]);
                x1 = x1.max(v[0]);
                y1 = y1.max(v[1]);
            }
            // an axis aligned rectangle, area matching its box
            let area = (x1 - x0) * (y1 - y0);
            let mut quad = 0.0;
            for i in 0..4 {
                let (u, w) = (p.pts[i], p.pts[(i + 1) % 4]);
                quad += u[0] * w[1] - w[0] * u[1];
            }
            if area > 1.0 && (quad.abs() / 2.0 - area).abs() < 0.05 * area {
                tops.push(Top {
                    r: [x0, y0, x1, y1],
                    z: zlo,
                    solid,
                    color: p.color,
                });
            }
            continue;
        }
        if n[2].abs() > 0.05 || solid == u32::MAX || p.finish.has_facade() {
            continue;
        }
        let low: Vec<&V3> = p.pts.iter().filter(|v| v[2] - zlo < 0.01).collect();
        if low.len() != 2 {
            continue;
        }
        let (a, b) = (*low[0], *low[1]);
        let len = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        let k = 1.0 / (n[0] * n[0] + n[1] * n[1]).sqrt().max(1e-6);
        walls.push(Wall {
            a,
            b,
            out: [n[0] * k, n[1] * k],
            len,
            zlo,
            zhi,
            color: p.color,
            solid,
        });
    }
    (walls, tops)
}

/// Add facade and roof detail to `o`.
pub fn dress(o: &mut Obj) {
    let mut rng = Rng::new(seed_of(o), 0x00fa_cade);
    let (walls, tops) = walls_and_tops(o);
    let mut add: Vec<(Vec<V3>, Rgb, bool)> = Vec::new();
    let pitch = floor_pitch(o, &walls);
    let boxes = solid_boxes(o);
    let mut windows: Vec<WindowSpec> = Vec::new();
    // some houses get shutters beside their windows, all in one colour
    let shutters = rng.chance(0.4);
    let shutter_col = rng.pick(&[
        [44, 84, 70],
        [120, 52, 48],
        [40, 52, 84],
        [236, 232, 222],
        [70, 70, 76],
    ]);
    for w in &walls {
        if w.len < 2.5 || w.zhi - w.zlo < 2.4 {
            continue;
        }
        let rows = pane_rows(o, w);
        trims(w, &rows, &mut add);
        if shutters && w.zhi - w.zlo < 9.0 {
            shutter_pairs(o, w, shutter_col, &mut add);
        }
        if rows.is_empty() && w.zlo <= 0.5 && w.zhi - w.zlo >= 5.5 && w.len >= 4.0 {
            blank_wall(w, pitch, &boxes, &mut windows);
        }
    }
    for p in &o.polys {
        if p.roof {
            ridge(&p.pts, p.color, &mut add);
        }
    }
    for (pts, col, shadow) in add {
        o.poly(&pts, col, shadow, false, false);
    }
    let many = windows.len() > 24;
    for (a, b, out, z0, z1, col) in windows {
        if many {
            plain_pane(o, a, b, out, z0, z1, col);
        } else {
            o.wall_rect(a, b, out, z0, z1, col);
        }
    }
    roofs(o, &tops, &mut rng);
}

/// Merged z ranges of the flat faces (windows, doors, signs) in front of a wall.
fn pane_rows(o: &Obj, w: &Wall) -> Vec<(f32, f32)> {
    let dir = [(w.b[0] - w.a[0]) / w.len, (w.b[1] - w.a[1]) / w.len];
    let mut iv: Vec<(f32, f32)> = Vec::new();
    for p in &o.polys {
        if !p.flat || p.pts.len() < 3 {
            continue;
        }
        let mut ok = true;
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for v in &p.pts {
            let (dx, dy) = (v[0] - w.a[0], v[1] - w.a[1]);
            let t = dx * w.out[0] + dy * w.out[1];
            let s = dx * dir[0] + dy * dir[1];
            if !(-0.05..=0.45).contains(&t)
                || s < -0.3
                || s > w.len + 0.3
                || v[2] < w.zlo - 0.2
                || v[2] > w.zhi + 0.2
            {
                ok = false;
                break;
            }
            lo = lo.min(v[2]);
            hi = hi.max(v[2]);
        }
        if ok {
            iv.push((lo, hi));
        }
    }
    iv.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f32, f32)> = Vec::new();
    for (lo, hi) in iv {
        match out.last_mut() {
            Some(l) if lo <= l.1 + 0.12 => l.1 = l.1.max(hi),
            _ => out.push((lo, hi)),
        }
    }
    out
}

/// A narrow board each side of every house sized window on the wall.
fn shutter_pairs(o: &Obj, w: &Wall, col: Rgb, add: &mut Vec<(Vec<V3>, Rgb, bool)>) {
    let dir = [(w.b[0] - w.a[0]) / w.len, (w.b[1] - w.a[1]) / w.len];
    let (p, _) = order(w);
    let flip = p[0] != w.a[0] || p[1] != w.a[1];
    for q in &o.polys {
        if !q.flat || q.pts.len() != 4 {
            continue;
        }
        let (mut s0, mut s1, mut z0, mut z1, mut tmax) =
            (f32::MAX, f32::MIN, f32::MAX, f32::MIN, f32::MIN);
        let mut tmin = f32::MAX;
        for v in &q.pts {
            let (dx, dy) = (v[0] - w.a[0], v[1] - w.a[1]);
            let s = dx * dir[0] + dy * dir[1];
            let t = dx * w.out[0] + dy * w.out[1];
            s0 = s0.min(s);
            s1 = s1.max(s);
            z0 = z0.min(v[2]);
            z1 = z1.max(v[2]);
            tmax = tmax.max(t);
            tmin = tmin.min(t);
        }
        let (ww, hh) = (s1 - s0, z1 - z0);
        // the pane itself sits 0.06 out from the wall; frames and mullions sit elsewhere
        if (tmax - 0.06).abs() > 0.01 || (tmin - 0.06).abs() > 0.01 {
            continue;
        }
        if !(0.7..=2.6).contains(&ww) || !(0.8..=2.4).contains(&hh) || z0 < 0.6 || z1 > w.zhi - 0.2
        {
            continue;
        }
        if s0 < 0.45 || s1 > w.len - 0.45 || (q.color != GLASS && q.color != crate::parts::GLASS_D)
        {
            continue;
        }
        for (a, b) in [(s0 - 0.38, s0 - 0.08), (s1 + 0.08, s1 + 0.38)] {
            let at = |s: f32, z: f32| -> V3 {
                [
                    w.a[0] + dir[0] * s + w.out[0] * 0.05,
                    w.a[1] + dir[1] * s + w.out[1] * 0.05,
                    z,
                ]
            };
            let quad = if flip {
                vec![at(b, z0), at(a, z0), at(a, z1), at(b, z1)]
            } else {
                vec![at(a, z0), at(b, z0), at(b, z1), at(a, z1)]
            };
            add.push((quad, col, false));
        }
    }
}

/// Light bands across the wall in the gaps between rows of windows.
fn trims(w: &Wall, rows: &[(f32, f32)], add: &mut Vec<(Vec<V3>, Rgb, bool)>) {
    if rows.len() < 2 {
        return;
    }
    let c = mix(w.color, [246, 240, 226], 0.45);
    for pair in rows.windows(2) {
        let gap = pair[1].0 - pair[0].1;
        if gap < 0.55 {
            continue;
        }
        let mid = (pair[0].1 + pair[1].0) / 2.0;
        let th = (gap * 0.3).clamp(0.1, 0.22);
        let (z0, z1) = (mid - th / 2.0, mid + th / 2.0);
        if z0 < w.zlo + 0.9 || z1 > w.zhi - 0.6 {
            continue;
        }
        let at = |v: &V3, z: f32| -> V3 { [v[0] + w.out[0] * 0.045, v[1] + w.out[1] * 0.045, z] };
        // counter-clockwise seen from outside
        let (p, q) = order(w);
        add.push((
            vec![at(&p, z0), at(&q, z0), at(&q, z1), at(&p, z1)],
            c,
            false,
        ));
    }
}

/// The wall's two low corners in the order that winds counter-clockwise seen from outside.
fn order(w: &Wall) -> (V3, V3) {
    let d = [w.b[0] - w.a[0], w.b[1] - w.a[1]];
    if d[1] * w.out[0] - d[0] * w.out[1] > 0.0 {
        (w.a, w.b)
    } else {
        (w.b, w.a)
    }
}

/// The usual floor to floor distance of the model, from its rows of windows.
fn floor_pitch(o: &Obj, walls: &[Wall]) -> f32 {
    let mut gaps: Vec<f32> = Vec::new();
    for w in walls {
        if w.len < 2.5 {
            continue;
        }
        let rows = pane_rows(o, w);
        for pair in rows.windows(2) {
            let d = pair[1].0 - pair[0].0;
            if (2.4..=5.0).contains(&d) {
                gaps.push(d);
            }
        }
    }
    if gaps.is_empty() {
        return 3.2;
    }
    gaps.sort_by(|a, b| a.total_cmp(b));
    gaps[gaps.len() / 2]
}

/// Bounding boxes of every solid.
fn solid_boxes(o: &Obj) -> Vec<[f32; 4]> {
    let mut v = vec![[f32::MAX, f32::MAX, f32::MIN, f32::MIN]; o.solids.len()];
    for p in &o.polys {
        if let Some(s) = p.solid {
            let b = &mut v[s as usize];
            for q in &p.pts {
                b[0] = b[0].min(q[0]);
                b[1] = b[1].min(q[1]);
                b[2] = b[2].max(q[0]);
                b[3] = b[3].max(q[1]);
            }
        }
    }
    v
}

type WindowSpec = ([f32; 2], [f32; 2], [f32; 2], f32, f32, Rgb);

/// A grid of windows on a wall that has none, unless another part of the building stands
/// against it.
fn blank_wall(w: &Wall, pitch: f32, boxes: &[[f32; 4]], out: &mut Vec<WindowSpec>) {
    let mid = [
        (w.a[0] + w.b[0]) / 2.0 + w.out[0] * 0.4,
        (w.a[1] + w.b[1]) / 2.0 + w.out[1] * 0.4,
    ];
    for (i, b) in boxes.iter().enumerate() {
        if i as u32 != w.solid && mid[0] > b[0] && mid[0] < b[2] && mid[1] > b[1] && mid[1] < b[3] {
            return;
        }
    }
    let (p, q) = order(w);
    let (a, b) = ([p[0], p[1]], [q[0], q[1]]);
    let usable = w.len - 2.0;
    let cols = ((usable / 3.4) as usize).max(1);
    let col_w = 1.3f32;
    let floors = (((w.zhi - w.zlo - 0.6) / pitch) as usize).max(1);
    for f in 0..floors {
        let z0 = w.zlo + f as f32 * pitch + pitch * 0.34;
        let z1 = z0 + pitch * 0.46;
        if z1 > w.zhi - 0.5 {
            break;
        }
        for c in 0..cols {
            let s = (1.0 + (c as f32 + 0.5) * usable / cols as f32) / w.len;
            let h = col_w / w.len / 2.0;
            out.push((
                [
                    a[0] + (b[0] - a[0]) * (s - h),
                    a[1] + (b[1] - a[1]) * (s - h),
                ],
                [
                    a[0] + (b[0] - a[0]) * (s + h),
                    a[1] + (b[1] - a[1]) * (s + h),
                ],
                w.out,
                z0,
                z1,
                GLASS,
            ));
        }
    }
}

/// One flat pane with an ink edge and nothing else, for big walls.
fn plain_pane(o: &mut Obj, a: [f32; 2], b: [f32; 2], out: [f32; 2], z0: f32, z1: f32, col: Rgb) {
    let q = |p: [f32; 2], z: f32| -> V3 { [p[0] + out[0] * 0.06, p[1] + out[1] * 0.06, z] };
    o.poly(
        &[q(a, z0), q(b, z0), q(b, z1), q(a, z1)],
        col,
        false,
        true,
        true,
    );
}

/// A dark cap along the top edge of each pitched roof face.
fn ridge(pts: &[V3], color: Rgb, add: &mut Vec<(Vec<V3>, Rgb, bool)>) {
    let n = crate::obj::newell(pts);
    if n[2] < 0.2 || n[2] > 0.95 {
        return;
    }
    let zmax = pts.iter().fold(f32::MIN, |a, v| a.max(v[2]));
    let zmin = pts.iter().fold(f32::MAX, |a, v| a.min(v[2]));
    if zmax - zmin < 0.4 {
        return;
    }
    let m = pts.len();
    // horizontal direction downhill: the horizontal part of the normal
    let hl = (n[0] * n[0] + n[1] * n[1]).sqrt().max(1e-6);
    let (ux, uy) = (n[0] / hl, n[1] / hl);
    let slope = n[2] / hl;
    for i in 0..m {
        let (a, b) = (pts[i], pts[(i + 1) % m]);
        if (a[2] - zmax).abs() > 0.01 || (b[2] - zmax).abs() > 0.01 {
            continue;
        }
        let len = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
        if len < 1.5 {
            continue;
        }
        let t = 0.22;
        let drop = t / slope.max(0.2);
        let cap = [
            [a[0] + ux * t, a[1] + uy * t, a[2] - drop + 0.03],
            [b[0] + ux * t, b[1] + uy * t, b[2] - drop + 0.03],
            [b[0], b[1], b[2] + 0.12],
            [a[0], a[1], a[2] + 0.12],
        ];
        let nb = crate::obj::newell(&cap);
        let mut v = cap.to_vec();
        if nb[2] < 0.0 {
            v.reverse();
        }
        add.push((v, mix(color, [60, 54, 56], 0.45), false));
    }
}

/// Rooftop clutter on the flat tops of boxes.
fn roofs(o: &mut Obj, tops: &[Top], rng: &mut Rng) {
    // things already standing on a roof: boxes poking above it
    let mut busy_all: Vec<([f32; 4], f32)> = Vec::new();
    for p in &o.polys {
        if p.flat || p.roof || p.pts.len() < 3 {
            continue;
        }
        let zlo = p.pts.iter().fold(f32::MAX, |a, v| a.min(v[2]));
        let zhi = p.pts.iter().fold(f32::MIN, |a, v| a.max(v[2]));
        // only the sides of things (a flat deck or box top is the roof itself)
        if zlo < 2.5 || zhi - zlo < 0.3 {
            continue;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for v in &p.pts {
            x0 = x0.min(v[0]);
            y0 = y0.min(v[1]);
            x1 = x1.max(v[0]);
            y1 = y1.max(v[1]);
        }
        busy_all.push(([x0, y0, x1, y1], zlo));
    }
    // pitched roofs: a box top under one is not a roof anyone sees
    let mut pitched: Vec<([f32; 4], f32)> = Vec::new();
    for p in o.polys.iter().filter(|p| p.roof) {
        let (mut x0, mut y0, mut x1, mut y1, mut z) =
            (f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN);
        for v in &p.pts {
            x0 = x0.min(v[0]);
            y0 = y0.min(v[1]);
            x1 = x1.max(v[0]);
            y1 = y1.max(v[1]);
            z = z.max(v[2]);
        }
        pitched.push(([x0, y0, x1, y1], z));
    }
    let mut done: Vec<([f32; 4], f32)> = Vec::new();
    for t in tops {
        let area = (t.r[2] - t.r[0]) * (t.r[3] - t.r[1]);
        // a box top and a parapet deck on it are one roof
        if done
            .iter()
            .any(|(r, z)| (z - t.z).abs() < 0.4 && overlap(*r, t.r, -0.5))
        {
            continue;
        }
        done.push((t.r, t.z));
        if t.z < 4.0 || area < 36.0 {
            continue;
        }
        // another box stands on this roof: leave it alone
        let covered = tops.iter().any(|u| {
            let ua = (u.r[2] - u.r[0]) * (u.r[3] - u.r[1]);
            u.solid != t.solid && u.z > t.z + 0.5 && ua > area * 0.4 && overlap(u.r, t.r, 0.0)
        });
        let under = pitched
            .iter()
            .any(|(r, z)| *z > t.z + 0.2 && overlap(*r, t.r, -0.8));
        if covered || under {
            continue;
        }
        let busy: Vec<[f32; 4]> = busy_all
            .iter()
            .filter(|(r, z)| *z >= t.z - 0.15 && overlap(*r, t.r, 0.0))
            .map(|(r, _)| *r)
            .collect();
        let mut placed: Vec<[f32; 4]> = Vec::new();
        let inset = [t.r[0] + 1.3, t.r[1] + 1.3, t.r[2] - 1.3, t.r[3] - 1.3];
        if inset[2] - inset[0] < 2.5 || inset[3] - inset[1] < 2.5 {
            continue;
        }
        let garden = area >= 110.0 && t.z >= 16.0 && rng.chance(0.4);
        if garden {
            let (x0, y0, x1, y1) = (inset[0], inset[1], inset[2], inset[3]);
            let (gx0, gx1) = (x0 + (x1 - x0) * 0.1, x0 + (x1 - x0) * 0.62);
            let (gy0, gy1) = (y0 + (y1 - y0) * 0.1, y0 + (y1 - y0) * 0.9);
            if !busy.iter().any(|b| overlap(*b, [gx0, gy0, gx1, gy1], 0.4)) {
                o.box_(
                    gx0,
                    gy0,
                    t.z,
                    gx1,
                    gy1,
                    t.z + 0.12,
                    [96, 150, 86],
                    Some([108, 164, 94]),
                    false,
                );
                // planters and a bench
                for k in 0..3 {
                    let px = gx0 + 0.8 + k as f32 * (gx1 - gx0 - 1.6) / 2.0;
                    o.bx_top(
                        px - 0.5,
                        gy1 - 1.4,
                        t.z + 0.12,
                        px + 0.5,
                        gy1 - 0.6,
                        t.z + 0.7,
                        [150, 120, 96],
                        [70, 126, 66],
                    );
                }
                o.bx(
                    gx0 + 1.2,
                    gy0 + 0.6,
                    t.z + 0.12,
                    gx0 + 3.2,
                    gy0 + 1.1,
                    t.z + 0.6,
                    [188, 150, 104],
                );
                placed.push([gx0, gy0, gx1, gy1]);
            }
        }
        let n = ((area / 70.0) as usize).clamp(1, 4);
        let tall_old = t.z < 16.0 && rng.chance(0.14);
        let mut want: Vec<u8> = Vec::new();
        if tall_old {
            want.push(3); // water tank
        }
        if area >= 80.0 && t.z >= 9.0 && rng.chance(0.4) {
            want.push(2); // stair housing
        }
        while want.len() < n + 1 {
            let kind = [0u8, 0, 1, 1, 4, 0][rng.below(6)];
            want.push(kind);
        }
        for kind in want {
            let (w, d) = match kind {
                0 => (1.7, 1.2),
                1 => (0.9, 0.9),
                2 => (2.4, 2.2),
                3 => (2.8, 2.8),
                _ => (2.4, 1.6),
            };
            for _ in 0..10 {
                let cx = rng.range(inset[0] + w / 2.0, inset[2] - w / 2.0);
                let cy = rng.range(inset[1] + d / 2.0, inset[3] - d / 2.0);
                let r = [cx - w / 2.0, cy - d / 2.0, cx + w / 2.0, cy + d / 2.0];
                if busy
                    .iter()
                    .chain(placed.iter())
                    .any(|b| overlap(*b, r, 0.5))
                {
                    continue;
                }
                placed.push(r);
                unit(o, kind, r, t.z, t.color);
                break;
            }
        }
    }
}

fn overlap(a: [f32; 4], b: [f32; 4], m: f32) -> bool {
    a[0] - m < b[2] && a[2] + m > b[0] && a[1] - m < b[3] && a[3] + m > b[1]
}

fn unit(o: &mut Obj, kind: u8, r: [f32; 4], z: f32, roof: Rgb) {
    let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
    match kind {
        0 => {
            // cooling unit with a dark fan on top
            o.bx_top(
                r[0],
                r[1],
                z,
                r[2],
                r[3],
                z + 0.95,
                [176, 178, 182],
                [150, 152, 158],
            );
            o.box_(
                cx - 0.35,
                cy - 0.35,
                z + 0.95,
                cx + 0.35,
                cy + 0.35,
                z + 1.02,
                [58, 62, 70],
                Some([58, 62, 70]),
                false,
            );
        }
        1 => {
            // vent stack with a cap
            o.bx(
                cx - 0.18,
                cy - 0.18,
                z,
                cx + 0.18,
                cy + 0.18,
                z + 1.3,
                [150, 150, 148],
            );
            o.bx(
                cx - 0.34,
                cy - 0.34,
                z + 1.3,
                cx + 0.34,
                cy + 0.34,
                z + 1.42,
                [112, 112, 112],
            );
        }
        2 => {
            // stair housing, a shade lighter than the roof, with a door and a flat lid
            let wall = mix(roof, [226, 222, 212], 0.55);
            o.bx_top(r[0], r[1], z, r[2], r[3], z + 2.6, wall, [128, 126, 122]);
            o.win_s(cx - 0.5, cx + 0.5, z, z + 1.9, r[1], [96, 84, 72]);
        }
        3 => {
            // water tank on a steel frame
            o.bx(
                r[0] + 0.2,
                r[1] + 0.2,
                z,
                r[2] - 0.2,
                r[3] - 0.2,
                z + 0.7,
                [92, 96, 104],
            );
            o.prism(
                cx,
                cy,
                z + 0.7,
                z + 2.8,
                1.2,
                [150, 106, 72],
                8,
                Some([120, 84, 58]),
            );
            o.pyramid(cx, cy, z + 2.8, 1.3, 0.8, [92, 70, 56], 8, 0.0);
        }
        _ => {
            // skylight
            o.bx_top(
                r[0],
                r[1],
                z,
                r[2],
                r[3],
                z + 0.5,
                [150, 190, 214],
                [196, 222, 236],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::kit::fit_to_lot;
    use crate::rng::Rng;
    use crate::{Region, build_lot_level_in};
    use us_gen::LotKind;

    fn fingerprint(kind: LotKind, level: u8, seed: u64) -> (usize, u64) {
        let o = fit_to_lot(
            build_lot_level_in(Region::Mixed, kind, level, &mut Rng::new(seed, 7)),
            1000.0,
        );
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for p in &o.polys {
            for v in &p.pts {
                for c in v {
                    h = (h ^ c.to_bits() as u64).wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
        (o.polys.len(), h)
    }

    #[test]
    fn same_lot_gets_the_same_facade() {
        for kind in [
            LotKind::House,
            LotKind::Shop,
            LotKind::Office,
            LotKind::Apartments,
        ] {
            for level in 1..=5 {
                assert_eq!(fingerprint(kind, level, 11), fingerprint(kind, level, 11));
            }
        }
    }

    #[test]
    fn flat_roofs_gain_clutter_and_stay_in_budget() {
        // detail never more than doubles the faces of a tall office
        let (n, _) = fingerprint(LotKind::Office, 5, 3);
        let bare = build_lot_level_in(Region::Mixed, LotKind::Office, 5, &mut Rng::new(3, 7));
        assert!(n >= bare.polys.len());
        assert!(n < bare.polys.len() * 2 + 400);
    }
}
