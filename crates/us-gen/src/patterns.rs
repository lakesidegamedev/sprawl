//! Street patterns for one addition: oriented grids, curving streets, and pods.

use crate::geom::{
    Pt, area, atan2, axis_angle, centroid, clip_line, closed, cos, dedup, dist_line_line,
    dist_pt_line, dist_pt_ring, dist_ring_line, interpolate, length, open_ring, point_in_ring,
    project, pt, rotate, rotate_all, sin,
};
use crate::polygonize::split_polygon;
use crate::rng::Rng;

pub type Line = Vec<Pt>;

#[derive(Default)]
pub struct Layout {
    pub streets: Vec<Line>,
    pub lanes: Vec<Line>,
    /// Cul-de-sac stubs, each starting on a street and ending at the bulb.
    pub culs: Vec<Line>,
    pub entrances: Vec<Pt>,
}

fn frange(a: f64, b: f64, st: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let mut x = a;
    while x <= b {
        out.push(x);
        x += st;
    }
    out
}

fn min_dist_lines(p: Pt, lines: &[&[Pt]]) -> f64 {
    lines
        .iter()
        .map(|l| dist_pt_line(p, l))
        .fold(f64::INFINITY, f64::min)
}

/// Streets of an oriented grid clipped to `poly`: lines along `ang` every `depth` metres and
/// cross lines every `length_` metres, phased from `origin`. Optional alleys mid-block.
#[allow(clippy::too_many_arguments)]
pub fn grid_lines(
    poly: &[Pt],
    ang: f64,
    depth: f64,
    length_: f64,
    phase: (f64, f64),
    alleys: bool,
    origin: Pt,
    bend: f64,
    jog: (f64, f64),
    rng: &mut Rng,
) -> (Vec<Line>, Vec<Line>) {
    let loc = rotate_all(poly, origin, -ang);
    let (lo, hi) = crate::geom::bbox(&loc);
    let (minx, miny, maxx, maxy) = (lo.x, lo.y, hi.x, hi.y);
    let mut streets = Vec::new();
    let mut lanes = Vec::new();
    let add = |pts: Vec<Pt>, bucket: &mut Vec<Line>| {
        let w = rotate_all(&pts, origin, ang);
        for p in clip_line(&w, poly, true) {
            let p = dedup(&p);
            if length(&p) > 30.0 {
                bucket.push(p);
            }
        }
    };
    let mut y = ((miny - phase.1 - origin.y) / depth).floor() * depth + phase.1 + origin.y;
    let mut rows: Vec<f64> = Vec::new();
    let mut guard = 0;
    while y < maxy + depth && guard < 400 {
        rows.push(y);
        let ph = rng.uniform(0.0, 6.0);
        if bend != 0.0 {
            add(
                frange(minx - 20.0, maxx + 20.0, 10.0)
                    .into_iter()
                    .map(|x| pt(x, y + bend * sin(x / 300.0 + ph)))
                    .collect(),
                &mut streets,
            );
        } else {
            add(vec![pt(minx - 20.0, y), pt(maxx + 20.0, y)], &mut streets);
        }
        if alleys {
            add(
                vec![
                    pt(minx - 20.0, y + depth / 2.0),
                    pt(maxx + 20.0, y + depth / 2.0),
                ],
                &mut lanes,
            );
        }
        y += depth;
        guard += 1;
    }
    let mut x = ((minx - phase.0 - origin.x) / length_).floor() * length_ + phase.0 + origin.x;
    guard = 0;
    while x < maxx + length_ && guard < 400 {
        if jog.0 > 0.0 && rows.len() > 2 {
            // between each pair of cross streets the street shifts sideways, so it meets the
            // along-street in two T-junctions instead of one four-way crossing
            let mut edges: Vec<f64> = vec![miny - 20.0];
            edges.extend(rows.iter().copied().filter(|r| *r > miny && *r < maxy));
            edges.push(maxy + 20.0);
            for (i, w) in edges.windows(2).enumerate() {
                let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
                let off = if rng.chance(jog.0) { sign * jog.1 } else { 0.0 };
                add(vec![pt(x + off, w[0]), pt(x + off, w[1])], &mut streets);
            }
        } else {
            add(vec![pt(x, miny - 20.0), pt(x, maxy + 20.0)], &mut streets);
        }
        x += length_;
        guard += 1;
    }
    (streets, lanes)
}

/// Curving streets along the cell's long axis, optional wavy cross streets, optional cul-de-sacs.
/// Built in the cell's own frame and rotated back.
#[allow(clippy::too_many_arguments)]
pub fn curvy_local(
    cell: &[Pt],
    rng: &mut Rng,
    spacing: f64,
    amp: f64,
    cross: Option<f64>,
    culs: bool,
    wl: f64,
    flow: Option<f64>,
) -> (Vec<Line>, Vec<Line>) {
    let ang = flow.unwrap_or_else(|| axis_angle(cell));
    let c = centroid(cell);
    let loc = rotate_all(cell, c, -ang);
    let (lo, hi) = crate::geom::bbox(&loc);
    let (minx, miny, maxx, maxy) = (lo.x, lo.y, hi.x, hi.y);
    let ph = rng.uniform(0.0, 6.3);
    let n = (((maxy - miny) / spacing) as usize).max(1);
    let xs: Vec<f64> = (0..=((maxx - minx + 60.0) / 8.0) as usize)
        .map(|k| minx - 30.0 + k as f64 * 8.0)
        .collect();
    let mut raw: Vec<Line> = Vec::new();
    for i in 0..n {
        let y = miny + (i as f64 + 0.5) * (maxy - miny) / n as f64;
        let a = amp * rng.uniform(0.7, 1.2);
        raw.push(
            xs.iter()
                .map(|&x| pt(x, y + a * sin(x / wl + ph + i as f64 * 0.9)))
                .collect(),
        );
    }
    if let Some(cr) = cross {
        let m = (((maxx - minx) / cr) as usize).max(1);
        let ys: Vec<f64> = (0..=((maxy - miny + 60.0) / 8.0) as usize)
            .map(|k| miny - 30.0 + k as f64 * 8.0)
            .collect();
        for i in 1..m {
            let x = minx + i as f64 * (maxx - minx) / m as f64 + rng.uniform(-cr * 0.15, cr * 0.15);
            let a = amp * 0.5;
            raw.push(
                ys.iter()
                    .map(|&y| pt(x + a * sin(y / (wl * 0.8) + ph + i as f64), y))
                    .collect(),
            );
        }
    }
    let mut out: Vec<Line> = Vec::new();
    for ln in raw {
        let w = rotate_all(&ln, c, ang);
        for p in clip_line(&w, cell, true) {
            let p = dedup(&p);
            if length(&p) > 50.0 {
                out.push(p);
            }
        }
    }
    let mut stubs: Vec<Line> = Vec::new();
    if culs {
        let ring_line = closed(cell);
        for ln in &out {
            let total = length(ln);
            let mut k = 90.0;
            let mut side = 1.0;
            while k < total - 90.0 {
                let (p, _) = interpolate(ln, k);
                let (q, _) = interpolate(ln, k + 2.0);
                let (dx, dy) = (q.x - p.x, q.y - p.y);
                let mm = (dx * dx + dy * dy).sqrt().max(1e-9);
                let (nx, ny) = (-dy / mm * side, dx / mm * side);
                let l2 = 70.0 + rng.uniform(-10.0, 15.0);
                let bend = rng.uniform(-0.35, 0.35);
                let mid = pt(
                    p.x + nx * l2 * 0.5 + ny * bend * 12.0,
                    p.y + ny * l2 * 0.5 - nx * bend * 12.0,
                );
                let end = pt(
                    p.x + nx * l2 + ny * bend * 25.0,
                    p.y + ny * l2 - nx * bend * 25.0,
                );
                let inside = point_in_ring(end, cell) && dist_pt_ring(end, cell) >= 28.0;
                if inside {
                    let mut others: Vec<&[Pt]> = vec![&ring_line];
                    others.extend(out.iter().map(|l| l.as_slice()));
                    others.extend(stubs.iter().map(|l| l.as_slice()));
                    if min_dist_lines(end, &others) > 34.0 {
                        stubs.push(vec![p, mid, end]);
                    }
                }
                k += rng.uniform(95.0, 130.0);
                side = -side;
            }
        }
    }
    (out, stubs)
}

/// Inward offset of a ring by `d` using mitred corners. None when the result is not clean.
pub fn inset_ring(ring: &[Pt], d: f64) -> Option<Vec<Pt>> {
    let mut r = open_ring(ring);
    if crate::geom::area_signed(&r) < 0.0 {
        r.reverse();
    }
    let n = r.len();
    if n < 3 {
        return None;
    }
    // offset lines, one per edge: point and direction
    let mut lines: Vec<(Pt, Pt)> = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b) = (r[i], r[(i + 1) % n]);
        let dir = (b - a).unit();
        let nrm = pt(-dir.y, dir.x);
        lines.push((a + nrm * d, dir));
    }
    let mut out: Vec<Pt> = Vec::with_capacity(n);
    for i in 0..n {
        let (p0, d0) = lines[(i + n - 1) % n];
        let (p1, d1) = lines[i];
        let den = d0.cross(d1);
        let v = if den.abs() < 1e-9 {
            p1
        } else {
            let t = (p1 - p0).cross(d1) / den;
            let q = p0 + d0 * t;
            if q.dist(r[i]) > 4.0 * d {
                r[i] + (p1 - r[i])
            } else {
                q
            }
        };
        out.push(v);
    }
    let out = open_ring(&dedup(&out));
    if out.len() < 3 || crate::geom::area_signed(&out) <= 0.0 {
        return None;
    }
    for p in &out {
        if !point_in_ring(*p, &r) || dist_pt_ring(*p, &r) < d * 0.9 {
            return None;
        }
    }
    // reject self-intersection
    let m = out.len();
    for i in 0..m {
        for j in i + 2..m {
            if i == 0 && j == m - 1 {
                continue;
            }
            if crate::geom::seg_intersect(out[i], out[(i + 1) % m], out[j], out[(j + 1) % m])
                .is_some()
            {
                return None;
            }
        }
    }
    if area(&out) < 0.2 * area(&r) {
        return None;
    }
    Some(out)
}

struct CulRules {
    k0: (f64, f64),
    step: (f64, f64),
    len: (f64, f64),
    clear: f64,
    gap: f64,
}

#[allow(clippy::too_many_arguments)]
fn place_culs(
    loop_line: &[Pt],
    loop_ring: &[Pt],
    rules: &CulRules,
    entrance_ends: &[Pt],
    culs: &mut Vec<Line>,
    rng: &mut Rng,
    target: usize,
) {
    let total = length(loop_line);
    let mut k = rng.uniform(rules.k0.0, rules.k0.1);
    while k < total && culs.len() < target {
        let (p, _) = interpolate(loop_line, k);
        let (q, _) = interpolate(loop_line, (k + 2.0) % total);
        let (dx, dy) = (q.x - p.x, q.y - p.y);
        let m = (dx * dx + dy * dy).sqrt().max(1e-9);
        let (nx, ny) = (-dy / m, dx / m);
        let l2 = rng.uniform(rules.len.0, rules.len.1);
        let end = pt(p.x + nx * l2, p.y + ny * l2);
        let mid = p.lerp(end, 0.5);
        let ok = point_in_ring(end, loop_ring)
            && point_in_ring(mid, loop_ring)
            && dist_pt_ring(end, loop_ring) >= rules.clear
            && entrance_ends.iter().all(|e| e.dist(p) > 28.0)
            && culs
                .iter()
                .all(|o| dist_line_line(&[p, end], o) > rules.gap || o[0].dist(p) < 1.0);
        if ok {
            culs.push(vec![p, end]);
        }
        k += rng.uniform(rules.step.0, rules.step.1);
    }
}

/// Late subdivisions: split the patch into pods; each has a loop street, entrances from a bigger
/// road, and at least two cul-de-sacs. No through traffic. None when no clean pod fits.
pub fn pods(patch: &[Pt], entries: &[&[Pt]], rng: &mut Rng) -> Option<Layout> {
    let ang = axis_angle(patch);
    let c = centroid(patch);
    let mut pieces = vec![patch.to_vec()];
    if area(patch) > 140000.0 {
        let cut = rotate_all(&[pt(c.x, c.y - 3000.0), pt(c.x, c.y + 3000.0)], c, ang);
        let split: Vec<Vec<Pt>> = split_polygon(patch, &[cut])
            .into_iter()
            .filter(|p| area(p) > 20000.0)
            .collect();
        if !split.is_empty() {
            pieces = split;
        }
    }
    let mut out = Layout::default();
    for pod in &pieces {
        let Some(lp) = inset_ring(pod, 35.0).or_else(|| inset_ring(pod, 25.0)) else {
            continue;
        };
        if area(&lp) < 6000.0 {
            continue;
        }
        let loop_line = closed(&lp);
        let total = length(&loop_line);
        if total < 280.0 {
            continue;
        }
        let near: Vec<&[Pt]> = entries
            .iter()
            .copied()
            .filter(|e| dist_ring_line(pod, e) <= 3.0)
            .collect();
        if near.is_empty() {
            continue;
        }
        // entrance: loop sample closest to a bigger road
        let mut samples: Vec<(f64, f64, usize, Pt)> = Vec::new(); // (dist, along, road, loop point)
        let ns = (total / 10.0) as usize;
        for s in 0..ns {
            let along = s as f64 * 10.0;
            let p = interpolate(&loop_line, along).0;
            let (bd, bi) = near
                .iter()
                .enumerate()
                .map(|(i, e)| (dist_pt_line(p, e), i))
                .fold((f64::INFINITY, 0), |a, b| if b.0 < a.0 { b } else { a });
            samples.push((bd, along, bi, p));
        }
        samples.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let Some(first) = samples.first().copied() else {
            continue;
        };
        if first.0 > 150.0 {
            continue;
        }
        let mut picks = vec![first];
        if area(pod) > 90000.0
            && let Some(s2) = samples.iter().find(|s| {
                let d = (s.1 - first.1).abs();
                d.min(total - d) >= total * 0.3 && s.0 < 150.0
            })
        {
            picks.push(*s2);
        }
        let mut layout = Layout::default();
        let mut ends = Vec::new();
        for (_, _, ri, p) in &picks {
            let q = project(near[*ri], *p).2;
            layout.streets.push(vec![q, *p]);
            layout.entrances.push(q);
            ends.push(*p);
        }
        layout.streets.push(loop_line.clone());
        let target = ((total / 230.0) as usize).max(2);
        let passes = [
            CulRules {
                k0: (20.0, 60.0),
                step: (110.0, 150.0),
                len: (50.0, 80.0),
                clear: 26.0,
                gap: 30.0,
            },
            CulRules {
                k0: (10.0, 40.0),
                step: (55.0, 80.0),
                len: (35.0, 60.0),
                clear: 16.0,
                gap: 22.0,
            },
            CulRules {
                k0: (5.0, 15.0),
                step: (14.0, 18.0),
                len: (30.0, 45.0),
                clear: 12.0,
                gap: 18.0,
            },
        ];
        let mut culs: Vec<Line> = Vec::new();
        for (pi, rules) in passes.iter().enumerate() {
            let want = if pi == 2 { 2 } else { target };
            if culs.len() < want {
                place_culs(&loop_line, &lp, rules, &ends, &mut culs, rng, target);
            }
        }
        if culs.len() < 2 {
            continue;
        }
        layout.culs = culs;
        out.streets.extend(layout.streets);
        out.culs.extend(layout.culs);
        out.entrances.extend(layout.entrances);
    }
    let _ = (atan2, cos, rotate);
    if out.entrances.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inset_square() {
        let sq = vec![
            pt(0.0, 0.0),
            pt(200.0, 0.0),
            pt(200.0, 200.0),
            pt(0.0, 200.0),
        ];
        let r = inset_ring(&sq, 35.0).unwrap();
        assert!((area(&r) - 130.0 * 130.0).abs() < 1.0);
    }

    #[test]
    fn grid_lines_stay_inside() {
        let sq = vec![
            pt(1000.0, 1000.0),
            pt(1400.0, 1000.0),
            pt(1400.0, 1300.0),
            pt(1000.0, 1300.0),
        ];
        let mut rng = Rng::new(1, 1);
        let (s, l) = grid_lines(
            &sq,
            0.3,
            80.0,
            100.0,
            (0.0, 0.0),
            true,
            pt(1200.0, 1150.0),
            0.0,
            (0.0, 0.0),
            &mut rng,
        );
        assert!(!s.is_empty() && !l.is_empty());
        for line in s.iter().chain(l.iter()) {
            for p in line {
                assert!(dist_pt_ring(*p, &sq) < 1e-6 || point_in_ring(*p, &sq));
            }
        }
    }

    #[test]
    fn pod_has_entrance_and_culs() {
        let sq = vec![
            pt(0.0, 0.0),
            pt(400.0, 0.0),
            pt(400.0, 300.0),
            pt(0.0, 300.0),
        ];
        let road = vec![pt(-100.0, 0.0), pt(500.0, 0.0)];
        let mut rng = Rng::new(3, 3);
        let l = pods(&sq, &[road.as_slice()], &mut rng).unwrap();
        assert!(!l.entrances.is_empty());
        assert!(l.culs.len() >= 2);
    }
}
