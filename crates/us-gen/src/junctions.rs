//! Generator version 3 road features: roundabouts at suburban collector junctions, diamond
//! ramps where an arterial meets the highway, cul-de-sacs that join the next street, collectors
//! that run on between neighbourhoods, and grid streets that bend with the ground.
//!
//! Everything here runs after the version 2 rules and only for version 3, with its own random
//! stream, so older towns come out exactly as before.

use crate::geom::{
    Pt, atan2, bbox, clip_line, cos, dedup, dist_pt_line, dist_pt_ring, interpolate, length,
    point_in_ring, project, pt, seg_intersect, sin,
};
use crate::index::SegIndex;
use crate::plan::{Draft, Tag};
use crate::rng::Rng;
use crate::terrain::Terrain;
use crate::world::{HWY_Y, S, dist_hall};
use crate::{Addition, Pattern, RoadClass};

/// Centre line radius of a roundabout ring, metres. The ring is a collector, so its paved edge
/// is 6 m further out and the island inside is 10 m across the middle.
pub const RING_R: f64 = 16.0;
const RING_SIDES: usize = 32;

fn is_collector(d: &Draft) -> bool {
    d.class == RoadClass::Collector
        && d.line.len() >= 2
        && d.line[0].dist(*d.line.last().unwrap()) > 1.0
}

/// The ring polygon of a roundabout at `c`, first point repeated.
fn ring_at(c: Pt, r: f64) -> Vec<Pt> {
    (0..=RING_SIDES)
        .map(|i| {
            let a = std::f64::consts::TAU * (i % RING_SIDES) as f64 / RING_SIDES as f64;
            pt(c.x + r * cos(a), c.y + r * sin(a))
        })
        .collect()
}

fn index_of(drafts: &[Draft]) -> SegIndex {
    let mut idx = SegIndex::new(S, 64.0);
    for (i, d) in drafts.iter().enumerate() {
        if d.class != RoadClass::Rail {
            idx.add_line(&d.line, i as u32);
        }
    }
    idx
}

/// Places where two collectors cross or one ends on the other, in a fixed order.
fn collector_nodes(drafts: &[Draft]) -> Vec<Pt> {
    let col: Vec<usize> = (0..drafts.len())
        .filter(|&i| is_collector(&drafts[i]))
        .collect();
    let boxes: Vec<(Pt, Pt)> = col.iter().map(|&i| bbox(&drafts[i].line)).collect();
    let mut out: Vec<Pt> = Vec::new();
    for a in 0..col.len() {
        for b in a + 1..col.len() {
            let (ba, bb) = (&boxes[a], &boxes[b]);
            if ba.1.x + 2.0 < bb.0.x
                || bb.1.x + 2.0 < ba.0.x
                || ba.1.y + 2.0 < bb.0.y
                || bb.1.y + 2.0 < ba.0.y
            {
                continue;
            }
            let (la, lb) = (&drafts[col[a]].line, &drafts[col[b]].line);
            for wa in la.windows(2) {
                for wb in lb.windows(2) {
                    if let Some((t, _)) = seg_intersect(wa[0], wa[1], wb[0], wb[1]) {
                        out.push(wa[0].lerp(wa[1], t));
                    }
                }
            }
            for (e, other) in [
                (la[0], lb),
                (*la.last().unwrap(), lb),
                (lb[0], la),
                (*lb.last().unwrap(), la),
            ] {
                let (_, d, q) = project(other, e);
                if d < 1.5 {
                    out.push(q);
                }
            }
        }
    }
    let mut keep: Vec<Pt> = Vec::new();
    for p in out {
        if keep.iter().all(|k| k.dist(p) > 8.0) {
            keep.push(p);
        }
    }
    keep
}

/// Roundabouts at some collector junctions well out of the old town. The collectors are cut
/// where they meet the ring, which is itself a closed collector street.
pub fn roundabouts(drafts: &mut Vec<Draft>, terrain: &Terrain, wet: &[Vec<Pt>], rng: &mut Rng) {
    let nodes = collector_nodes(drafts);
    let mut taken: Vec<Pt> = Vec::new();
    let reach = RING_R + 22.0;
    for n in nodes {
        if dist_hall(n) < 650.0
            || n.x < 200.0
            || n.x > S - 200.0
            || n.y < HWY_Y + 200.0
            || n.y > S - 200.0
            || terrain.slope(n) > 0.07
            || taken.iter().any(|t| t.dist(n) < 260.0)
            || wet.iter().any(|w| point_in_ring(n, w))
        {
            continue;
        }
        let idx = index_of(drafts);
        let mut owners: Vec<u32> = Vec::new();
        idx.for_each_within(n, reach, |_, o| {
            if !owners.contains(&o) {
                owners.push(o);
            }
        });
        owners.sort_unstable();
        if owners.len() < 2
            || owners.len() > 3
            || owners.iter().any(|&o| !is_collector(&drafts[o as usize]))
        {
            continue;
        }
        let gon = ring_at(n, RING_R);
        let mut pieces: Vec<(usize, Vec<Vec<Pt>>)> = Vec::new();
        let mut arms: Vec<f64> = Vec::new();
        let mut ok = true;
        for &o in &owners {
            let outside: Vec<Vec<Pt>> = clip_line(&drafts[o as usize].line, &gon, false)
                .into_iter()
                .map(|l| dedup(&l))
                .filter(|l| l.len() >= 2)
                .collect();
            for l in &outside {
                if length(l) < 30.0 {
                    ok = false;
                }
                let e = if l[0].dist(n) < l[l.len() - 1].dist(n) {
                    l[0]
                } else {
                    l[l.len() - 1]
                };
                arms.push(atan2(e.y - n.y, e.x - n.x));
            }
            pieces.push((o as usize, outside));
        }
        if !ok || !(3..=4).contains(&arms.len()) {
            continue;
        }
        arms.sort_by(|a, b| a.total_cmp(b));
        let tau = std::f64::consts::TAU;
        let mut tight = false;
        for k in 0..arms.len() {
            let next = if k + 1 < arms.len() {
                arms[k + 1]
            } else {
                arms[0] + tau
            };
            if next - arms[k] < 55f64.to_radians() {
                tight = true;
            }
        }
        if tight || !rng.chance(0.75) {
            continue;
        }
        let addition = drafts[pieces[0].0].addition;
        let tag = drafts[pieces[0].0].tag;
        for (o, outside) in pieces {
            let base = drafts[o].clone();
            let mut it = outside.into_iter();
            match it.next() {
                Some(first) => drafts[o].line = first,
                None => drafts[o].line.clear(),
            }
            for l in it {
                drafts.push(Draft {
                    line: l,
                    ..base.clone()
                });
            }
        }
        drafts.retain(|d| d.line.len() >= 2);
        drafts.push(Draft {
            line: gon,
            class: RoadClass::Collector,
            tag: if tag == Tag::Cul { Tag::Interior } else { tag },
            addition,
        });
        taken.push(n);
    }
}

/// Diamond ramps where an arterial ends on the highway: on each side of the arterial a ramp
/// leaves the highway along it and bends in to meet the arterial square on.
pub fn ramps(drafts: &mut Vec<Draft>, terrain: &Terrain, wet: &[Vec<Pt>]) {
    let hwy_ids: Vec<usize> = (0..drafts.len())
        .filter(|&i| drafts[i].class == RoadClass::Highway)
        .collect();
    let art_ids: Vec<usize> = (0..drafts.len())
        .filter(|&i| drafts[i].class == RoadClass::Arterial)
        .collect();
    let mut idx = index_of(drafts);
    for &h in &hwy_ids {
        let hline = drafts[h].line.clone();
        let hlen = length(&hline);
        for &a in &art_ids {
            let mut al = drafts[a].line.clone();
            let near_front = dist_pt_line(al[0], &hline) < 1.5;
            let near_back = dist_pt_line(*al.last().unwrap(), &hline) < 1.5;
            if near_front == near_back {
                continue;
            }
            if near_back {
                al.reverse();
            }
            if length(&al) < 140.0 {
                continue;
            }
            let (pos, _, _) = project(&hline, al[0]);
            let (_, ad) = interpolate(&al, 0.0);
            for side in [-1.0f64, 1.0] {
                let hp = pos + side * 240.0;
                if hp < 30.0 || hp > hlen - 30.0 {
                    continue;
                }
                let (hpt, ht) = interpolate(&hline, hp);
                let (apt, _) = interpolate(&al, 95.0);
                let toward = ht * (-side);
                // the side of the highway the arterial leaves on
                let mut nrm = pt(-ht.y, ht.x);
                if nrm.dot(ad) < 0.0 {
                    nrm = nrm * -1.0;
                }
                // a slip lane: it joins the highway on its centre line (so the network is one
                // piece), tapers out to the edge, runs alongside, then curves off to the arterial
                let p0 = hpt;
                let q1 = hpt + toward * 60.0 + nrm * 14.0;
                let q2 = hpt + toward * 125.0 + nrm * 16.0;
                let c1 = q2 + toward * 55.0;
                let c2 = apt - toward * 45.0;
                let p3 = apt;
                let mut line: Vec<Pt> = vec![p0, q1];
                for k in 0..=12 {
                    let t = k as f64 / 12.0;
                    let u = 1.0 - t;
                    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    line.push(pt(
                        w[0] * q2.x + w[1] * c1.x + w[2] * c2.x + w[3] * p3.x,
                        w[0] * q2.y + w[1] * c1.y + w[2] * c2.y + w[3] * p3.y,
                    ));
                }
                let last = line.len() - 1;
                line[last] = p3;
                let mut clear = true;
                for (k, p) in line.iter().enumerate() {
                    if wet.iter().any(|w| point_in_ring(*p, w)) || terrain.slope(*p) > 0.1 {
                        clear = false;
                    }
                    if k == 0 || k == last {
                        continue;
                    }
                    idx.for_each_within(*p, 30.0, |d, o| {
                        let o = o as usize;
                        if o == h || o == a {
                            return;
                        }
                        if d < 6.0 + drafts[o].class.half_width() + 8.0 {
                            clear = false;
                        }
                    });
                }
                // never cross the arterial before the joining point
                for w in line[..last].windows(2) {
                    for v in al.windows(2) {
                        if seg_intersect(w[0], w[1], v[0], v[1]).is_some() {
                            clear = false;
                        }
                    }
                }
                if !clear {
                    continue;
                }
                let id = drafts.len() as u32;
                idx.add_line(&line, id);
                drafts.push(Draft {
                    line,
                    class: RoadClass::Collector,
                    tag: Tag::Skeleton,
                    addition: usize::MAX,
                });
            }
        }
    }
}

/// The allowed direction of a generated slip lane, relative to its point order.
/// Returns `None` for a road without the ramp template.
pub fn slip_lane_forward(line: &[Pt], highways: &[&[Pt]]) -> Option<bool> {
    if line.len() != 15 {
        return None;
    }
    for (on, near, other, forward) in [
        (line[0], line[1], line[14], true),
        (line[14], line[13], line[0], false),
    ] {
        for &highway in highways.iter().filter(|h| h.len() >= 2) {
            let (_, distance, _) = project(highway, on);
            if distance > 1.5 || project(highway, other).1 < 28.0 {
                continue;
            }
            let (along, distance, centre) = project(highway, near);
            let (_, tangent) = interpolate(highway, along);
            let heading = (near - on).unit();
            if !(7.0..=27.0).contains(&distance) || heading.dot(tangent).abs() < 0.75 {
                continue;
            }
            let on_right = pt(tangent.y, -tangent.x).dot(near - centre) > 0.0;
            let away_from_highway = (heading.dot(tangent) > 0.0) == on_right;
            return Some(away_from_highway == forward);
        }
    }
    None
}

/// A cul-de-sac whose tip is close to another street, with open ground between, is joined to
/// it. It stops being a dead end.
pub fn link_culs(drafts: &mut [Draft], additions: &[Addition]) {
    let idx = index_of(drafts);
    for i in 0..drafts.len() {
        if drafts[i].tag != Tag::Cul || drafts[i].line.len() < 2 {
            continue;
        }
        // a pod keeps its two cul-de-sacs
        if additions
            .get(drafts[i].addition)
            .is_some_and(|a| a.pattern == Pattern::Pods)
        {
            continue;
        }
        let l = &drafts[i].line;
        let tip = l[l.len() - 1];
        let dir = (tip - l[l.len() - 2]).unit();
        let Some(hit) = idx.nearest(tip, 70.0, Some(i as u32)) else {
            continue;
        };
        let other = &drafts[hit.owner as usize];
        if !matches!(other.class, RoadClass::Local | RoadClass::Collector) {
            continue;
        }
        let to = (hit.pt - tip).unit();
        if to.dot(dir) < 0.94 || hit.dist < 14.0 {
            continue;
        }
        let mut clear = true;
        for k in 1..8 {
            let p = tip.lerp(hit.pt, k as f64 / 8.0);
            idx.for_each_within(p, 14.0, |d, o| {
                if o as usize != i && o != hit.owner && d < 12.0 {
                    clear = false;
                }
            });
            // the new piece must come in at an angle, not run alongside the street it joins
            let want = 0.7 * hit.dist * (1.0 - k as f64 / 8.0);
            if dist_pt_line(p, &drafts[hit.owner as usize].line) < want {
                clear = false;
            }
        }
        if !clear {
            continue;
        }
        drafts[i].line.push(hit.pt);
        drafts[i].tag = Tag::Interior;
    }
}

/// Straight chains of local streets that run through more than one neighbourhood become
/// collectors, so the town has through routes between its parts.
pub fn through_collectors(drafts: &mut [Draft], min_len: f64) {
    let cand: Vec<usize> = (0..drafts.len())
        .filter(|&i| {
            drafts[i].class == RoadClass::Local
                && drafts[i].tag != Tag::Cul
                && drafts[i].line.len() >= 2
                && drafts[i].line[0].dist(*drafts[i].line.last().unwrap()) > 1.0
                // the old town keeps its own streets
                && dist_hall(interpolate(&drafts[i].line, length(&drafts[i].line) / 2.0).0) > 700.0
        })
        .collect();
    let n = cand.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], x: usize) -> usize {
        let mut r = x;
        while p[r] != r {
            r = p[r];
        }
        let mut c = x;
        while p[c] != r {
            let nx = p[c];
            p[c] = r;
            c = nx;
        }
        r
    }
    let mut ends: Vec<(Pt, Pt, usize)> = Vec::new();
    for (k, &i) in cand.iter().enumerate() {
        let l = &drafts[i].line;
        ends.push((l[0], (l[1] - l[0]).unit(), k));
        let m = l.len();
        ends.push((l[m - 1], (l[m - 2] - l[m - 1]).unit(), k));
    }
    for a in 0..ends.len() {
        let mut best: Option<(f64, usize)> = None;
        for b in 0..ends.len() {
            if ends[a].2 == ends[b].2 || ends[a].0.dist(ends[b].0) > 3.0 {
                continue;
            }
            let straight = -ends[a].1.dot(ends[b].1);
            if straight > 0.93 && best.is_none_or(|x| straight > x.0) {
                best = Some((straight, b));
            }
        }
        if let Some((_, b)) = best {
            let (ra, rb) = (find(&mut parent, ends[a].2), find(&mut parent, ends[b].2));
            if ra != rb {
                parent[ra] = rb;
            }
        }
    }
    let mut total = vec![0.0f64; n];
    let mut adds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for k in 0..n {
        let r = find(&mut parent, k);
        total[r] += length(&drafts[cand[k]].line);
        let a = drafts[cand[k]].addition;
        if !adds[r].contains(&a) {
            adds[r].push(a);
        }
    }
    for k in 0..n {
        let r = find(&mut parent, k);
        if total[r] >= min_len && adds[r].len() >= 2 {
            drafts[cand[k]].class = RoadClass::Collector;
        }
    }
}

/// Grid patterns whose streets bend with the ground.
pub fn bends(pattern: Pattern) -> bool {
    matches!(
        pattern,
        Pattern::CatalogueGrid | Pattern::LongBlockGrid | Pattern::RailGrid
    )
}

/// Move a street sideways with the lie of the land: streets shift up the slope a little, more
/// where it is steeper, and not at all near the edge of their neighbourhood. The shift depends
/// only on the position, so streets that meet still meet.
pub fn bend_with_ground(line: &[Pt], poly: &[Pt], terrain: &Terrain) -> Vec<Pt> {
    if line.len() < 2 {
        return line.to_vec();
    }
    let mut dense: Vec<Pt> = vec![line[0]];
    for w in line.windows(2) {
        let k = ((w[0].dist(w[1]) / 20.0).ceil() as usize).max(1);
        for s in 1..=k {
            dense.push(w[0].lerp(w[1], s as f64 / k as f64));
        }
    }
    dense
        .into_iter()
        .map(|p| {
            let edge = ((dist_pt_ring(p, poly) - 40.0) / 80.0).clamp(0.0, 1.0);
            let fade = edge * edge * (3.0 - 2.0 * edge);
            let g = terrain.gradient(p);
            let strength = (g.len() / 0.05).min(1.0) * 16.0 * fade;
            if strength < 1e-9 {
                p
            } else {
                p + g.unit() * strength
            }
        })
        .collect()
}
