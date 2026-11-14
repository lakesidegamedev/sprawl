//! Finishing detail added to a finished building model: a darker base course at the foot of the
//! walls, a cornice under the roofline of tall walls, and a fascia board along the eaves of
//! pitched roofs. Everything is a flat stepped-tone face with no outline, so it adds edges and
//! shape without changing the cel look. Windows and doors get theirs where they are built.

use crate::obj::{Obj, Rgb, V3};

fn lum(c: Rgb) -> f32 {
    c[0] as f32 * 0.3 + c[1] as f32 * 0.59 + c[2] as f32 * 0.11
}

fn mix(c: Rgb, to: Rgb, k: f32) -> Rgb {
    let m = |a: u8, b: u8| {
        (a as f32 + (b as f32 - a as f32) * k)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [m(c[0], to[0]), m(c[1], to[1]), m(c[2], to[2])]
}

/// Add the finishing detail to `o`.
pub fn finish(o: &mut Obj) {
    let mut add: Vec<(Vec<V3>, Rgb, bool)> = Vec::new();
    for p in &o.polys {
        if p.flat || p.pts.len() < 3 {
            continue;
        }
        let n = crate::obj::newell(&p.pts);
        if n == [0.0; 3] {
            continue;
        }
        if p.roof {
            eave(&p.pts, n, &mut add);
            continue;
        }
        if p.pts.len() != 4 || n[2].abs() > 0.05 || p.solid.is_none() {
            continue;
        }
        let (zlo, zhi) = p
            .pts
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v[2]), b.max(v[2])));
        let h = zhi - zlo;
        // a vertical rectangle: two corners low, two high
        let low: Vec<&V3> = p.pts.iter().filter(|v| v[2] - zlo < 0.01).collect();
        if low.len() != 2 {
            continue;
        }
        let (a, b) = (low[0], low[1]);
        let len = {
            let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
            (dx * dx + dy * dy).sqrt()
        };
        if len < 2.5 || h < 2.4 {
            continue;
        }
        let k = 1.0 / (n[0] * n[0] + n[1] * n[1]).sqrt().max(1e-6);
        let (ox, oy) = (n[0] * k, n[1] * k);
        let at = |v: &V3, t: f32, z: f32| -> V3 { [v[0] + ox * t, v[1] + oy * t, z] };
        // keep the same corner order as the wall so the face looks outward
        let wall = &p.pts;
        let band = |z0: f32, z1: f32, t: f32| -> Vec<V3> {
            // wall corners in order; map each to the band's heights by whether it is low or high
            wall.iter()
                .map(|v| {
                    let z = if v[2] - zlo < 0.01 { z0 } else { z1 };
                    at(v, t, z)
                })
                .collect()
        };
        if zlo <= 0.35 {
            // base course: a dark foot a little proud of the wall
            let c = mix(p.color, [92, 86, 84], 0.5);
            add.push((band(zlo, zlo + 0.5, 0.05), c, true));
        }
        if zhi >= 5.5 && zlo <= 0.35 {
            // cornice: a lighter band under the roofline with a ledge on top
            let c = mix(p.color, [244, 238, 224], 0.55).max_lum(lum(p.color));
            add.push((band(zhi - 0.4, zhi - 0.05, 0.14), c, true));
            let ledge: Vec<V3> = vec![
                at(wall_pt(wall, zhi, false), 0.0, zhi - 0.05),
                at(wall_pt(wall, zhi, true), 0.0, zhi - 0.05),
                at(wall_pt(wall, zhi, true), 0.14, zhi - 0.05),
                at(wall_pt(wall, zhi, false), 0.14, zhi - 0.05),
            ];
            add.push((ledge, c, true));
        }
    }
    for (pts, col, shadow) in add {
        o.poly(&pts, col, shadow, false, false);
    }
}

trait MaxLum {
    fn max_lum(self, floor: f32) -> Rgb;
}

impl MaxLum for Rgb {
    /// Never darker than the wall it sits on.
    fn max_lum(self, floor: f32) -> Rgb {
        if lum(self) >= floor {
            self
        } else {
            mix(self, [255, 255, 255], 0.3)
        }
    }
}

/// One of the two top corners of a wall rectangle (`second` picks the later one in order).
fn wall_pt(wall: &[V3], zhi: f32, second: bool) -> &V3 {
    let mut tops = wall.iter().filter(|v| (v[2] - zhi).abs() < 0.01);
    let first = tops.next().unwrap();
    if second {
        tops.next().unwrap_or(first)
    } else {
        first
    }
}

/// A fascia board hung under the lowest edge of a pitched roof face.
fn eave(pts: &[V3], n: V3, add: &mut Vec<(Vec<V3>, Rgb, bool)>) {
    let len2 = n[0] * n[0] + n[1] * n[1];
    if len2 < 1e-6 || n[2] <= 0.0 {
        return;
    }
    let zmin = pts.iter().fold(f32::MAX, |a, v| a.min(v[2]));
    let zmax = pts.iter().fold(f32::MIN, |a, v| a.max(v[2]));
    if zmax - zmin < 0.3 {
        return;
    }
    let m = pts.len();
    for i in 0..m {
        let (a, b) = (pts[i], pts[(i + 1) % m]);
        if (a[2] - zmin).abs() > 0.01 || (b[2] - zmin).abs() > 0.01 {
            continue;
        }
        let len = {
            let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
            (dx * dx + dy * dy).sqrt()
        };
        if len < 1.5 {
            continue;
        }
        // travel a to b with the roof on the left: the outward side is the right
        let k = 1.0 / len2.sqrt();
        let (ox, oy) = (n[0] * k, n[1] * k);
        let board = [
            [a[0], a[1], zmin - 0.28],
            [b[0], b[1], zmin - 0.28],
            [b[0], b[1], zmin],
            [a[0], a[1], zmin],
        ];
        // wind so the board faces outward
        let nb = crate::obj::newell(&board);
        let mut v: Vec<V3> = board.to_vec();
        if nb[0] * ox + nb[1] * oy < 0.0 {
            v.reverse();
        }
        for p in &mut v {
            p[0] += ox * 0.04;
            p[1] += oy * 0.04;
        }
        add.push((v, [238, 234, 224], true));
    }
}
