//! The rule based proposer: upgrade streets over capacity, add connectors where the drive is long.

use crate::geom::{closest, dist_pt_ring, dist_pt_seg, point_in_ring, polyline_len, seg_intersect};
use crate::{Change, Context, End, Proposer};
use std::collections::BinaryHeap;
use us_gen::{Pt, RoadClass, pt};

pub struct RuleProposer;

impl Proposer for RuleProposer {
    fn propose(&self, ctx: &Context) -> Vec<Change> {
        let mut v = upgrades(ctx);
        v.extend(connectors(ctx));
        v
    }
}

fn next_class(c: RoadClass) -> Option<RoadClass> {
    match c {
        RoadClass::Local => Some(RoadClass::Collector),
        RoadClass::Collector => Some(RoadClass::Arterial),
        _ => None,
    }
}

/// Lots whose footprint would sit on a road of `hw` half width along `pts`.
fn lots_on(ctx: &Context, pts: &[Pt], hw: f64) -> Vec<u32> {
    let mut out = Vec::new();
    for w in pts.windows(2) {
        let mid = w[0].lerp(w[1], 0.5);
        for li in ctx.index.lots.near(mid, w[0].dist(w[1]) * 0.5 + 30.0) {
            let l = &ctx.town.lots[li as usize];
            if dist_pt_seg(l.centre, w[0], w[1]) < hw + 1.0 + 0.55 * l.radius {
                out.push(li);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn upgrades(ctx: &Context) -> Vec<Change> {
    let mut out = Vec::new();
    for (i, e) in ctx.net.streets.edges.iter().enumerate() {
        if ctx.assignment.edge_vc[i] < ctx.params.upgrade_vc || e.len < 20.0 {
            continue;
        }
        let Some(to) = next_class(e.class) else {
            continue;
        };
        out.push(Change::Upgrade {
            street: i as u32,
            to,
            lots_removed: lots_on(ctx, &e.pts, to.half_width()),
        });
    }
    out
}

/// Distances by street length from `src`, stopping beyond `cutoff`. Fills `dist` and returns touched nodes.
fn bounded(ctx: &Context, src: u32, cutoff: f64, dist: &mut [f64], touched: &mut Vec<u32>) {
    #[derive(PartialEq)]
    struct It(f64, u32);
    impl Eq for It {}
    impl PartialOrd for It {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for It {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
        }
    }
    for &t in touched.iter() {
        dist[t as usize] = f64::INFINITY;
    }
    touched.clear();
    let net = ctx.net;
    let mut h = BinaryHeap::new();
    dist[src as usize] = 0.0;
    touched.push(src);
    h.push(It(0.0, src));
    while let Some(It(d, u)) = h.pop() {
        if d > dist[u as usize] || d > cutoff {
            continue;
        }
        for k in net.off[u as usize]..net.off[u as usize + 1] {
            let a = &net.arcs[net.out[k as usize] as usize];
            let nd = d + net.streets.edges[a.street as usize].len;
            if nd < dist[a.to as usize] {
                if dist[a.to as usize].is_infinite() {
                    touched.push(a.to);
                }
                dist[a.to as usize] = nd;
                h.push(It(nd, a.to));
            }
        }
    }
}

fn connectors(ctx: &Context) -> Vec<Change> {
    let p = ctx.params;
    let net = ctx.net;
    let sg = &net.streets;
    let nn = sg.nodes.len();
    let mut deg = vec![0u32; nn];
    // outgoing direction of every incident street at each node
    let mut dirs: Vec<Vec<Pt>> = vec![Vec::new(); nn];
    for e in &sg.edges {
        deg[e.a as usize] += 1;
        deg[e.b as usize] += 1;
        let first = e.pts[1] - e.pts[0];
        let n = e.pts.len();
        let last = e.pts[n - 2] - e.pts[n - 1];
        dirs[e.a as usize].push(first.unit());
        dirs[e.b as usize].push(last.unit());
    }
    let size = ctx.town.size;
    let inside = |q: Pt| {
        q.x > 40.0 && q.y > ctx.town.highway_y + 40.0 && q.x < size - 40.0 && q.y < size - 40.0
    };
    let cutoff = p.link_max * p.min_circuity;
    let mut dist = vec![f64::INFINITY; nn];
    let mut touched: Vec<u32> = Vec::new();
    let mut out = Vec::new();
    for a in 0..nn as u32 {
        let pa = sg.nodes[a as usize];
        if !inside(pa) {
            continue;
        }
        bounded(ctx, a, cutoff, &mut dist, &mut touched);
        // node to node
        for b in ctx.index.nodes.near(pa, p.link_max) {
            if b <= a {
                continue;
            }
            let pb = sg.nodes[b as usize];
            let l = pa.dist(pb);
            if l < p.link_min || l > p.link_max || !inside(pb) {
                continue;
            }
            if dist[b as usize] < p.min_circuity * l {
                continue;
            }
            if let Some(c) = try_link(ctx, End::Node(a), End::Node(b), pa, pb, &dirs, None) {
                out.push(c);
            }
        }
        // dead end to the side of a street
        if deg[a as usize] == 1 {
            let mut found: Vec<(f64, u32, u32, Pt, f64)> = Vec::new();
            for (si, seg) in ctx.index.street_segs.near(pa, pa, p.link_max) {
                let e = &sg.edges[si as usize];
                if e.a == a || e.b == a {
                    continue;
                }
                let (q, t) = closest(pa, e.pts[seg as usize], e.pts[seg as usize + 1]);
                let l = pa.dist(q);
                if l < p.link_min || l > p.link_max || !inside(q) {
                    continue;
                }
                let before: f64 = polyline_len(&e.pts[..=seg as usize])
                    + e.pts[seg as usize]
                        .dist(e.pts[seg as usize].lerp(e.pts[seg as usize + 1], t));
                let (da, db) = (dist[e.a as usize], dist[e.b as usize]);
                let route = (da + before).min(db + (e.len - before));
                if before < 40.0 || e.len - before < 40.0 || route < p.min_circuity * l {
                    continue;
                }
                found.push((l, si, seg, q, before / e.len));
            }
            found.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2)));
            let mut seen_streets: Vec<u32> = Vec::new();
            for (_, si, seg, q, frac) in found {
                if seen_streets.contains(&si) {
                    continue;
                }
                seen_streets.push(si);
                if seen_streets.len() > 3 {
                    break;
                }
                let e = &sg.edges[si as usize];
                let tangent = (e.pts[seg as usize + 1] - e.pts[seg as usize]).unit();
                if let Some(c) = try_link(
                    ctx,
                    End::Node(a),
                    End::Street {
                        street: si,
                        seg,
                        frac,
                    },
                    pa,
                    q,
                    &dirs,
                    Some(tangent),
                ) {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// Geometric rules for one straight connector. `tangent` is the street direction at the far end
/// when it lands on the side of a street.
fn try_link(
    ctx: &Context,
    a: End,
    b: End,
    pa: Pt,
    pb: Pt,
    dirs: &[Vec<Pt>],
    tangent: Option<Pt>,
) -> Option<Change> {
    let p = ctx.params;
    let town = ctx.town;
    let l = pa.dist(pb);
    let u = (pb - pa).unit();
    // direction rules: a dead end is continued roughly straight; between two junctions the link
    // runs along or across the streets at both ends; the side of a street is met squarely
    let ends = [(a, u), (b, pt(-u.x, -u.y))];
    let dead_end = ends
        .iter()
        .any(|(e, _)| matches!(e, End::Node(n) if dirs[*n as usize].len() == 1));
    for (end, dir) in ends {
        let End::Node(n) = end else { continue };
        let ds = &dirs[n as usize];
        // no sharp angles to any street at the node
        for d in ds {
            if dir.cross(*d).abs() < 0.57 && dir.dot(*d) > 0.0 {
                return None;
            }
        }
        if ds.len() == 1 {
            if -dir.dot(ds[0]) < 0.866 {
                return None;
            }
        } else if !dead_end
            && !ds
                .iter()
                .any(|d| dir.cross(*d).abs() < 0.34 || dir.dot(*d).abs() < 0.34)
        {
            return None;
        }
    }
    if let Some(t) = tangent
        && u.cross(t).abs() < 0.85
    {
        return None;
    }
    // crossings and clearance against every road
    let near = ctx.index.segs.near(pa, pb, 16.0);
    for &(ri, si) in &near {
        let pts = &town.roads[ri as usize].points;
        let (c, d) = (pts[si as usize], pts[si as usize + 1]);
        if let Some((t, _)) = seg_intersect(pa, pb, c, d) {
            let x = pa.lerp(pb, t);
            if x.dist(pa) > 1.0 && x.dist(pb) > 1.0 {
                return None;
            }
        }
    }
    let mut d = 24.0;
    while d <= l - 24.0 {
        let q = pa + u * d;
        for &(ri, si) in &near {
            let pts = &town.roads[ri as usize].points;
            if dist_pt_seg(q, pts[si as usize], pts[si as usize + 1]) < 13.0 {
                return None;
            }
        }
        d += 8.0;
    }
    // hall, water, cul-de-sac bulbs
    if dist_pt_seg(town.hall, pa, pb) < town.hall_radius + 15.0 {
        return None;
    }
    for ring in &town.water {
        let n = ring.len();
        for i in 0..n {
            if seg_intersect(pa, pb, ring[i], ring[(i + 1) % n]).is_some() {
                return None;
            }
        }
        let mut d = 0.0;
        while d <= l {
            let q = pa + u * d;
            if point_in_ring(q, ring) || dist_pt_ring(q, ring) < 25.0 {
                return None;
            }
            d += 20.0;
        }
    }
    for c in &town.cul_de_sacs {
        let d = dist_pt_seg(c.pos, pa, pb);
        if d < 14.0 && c.pos.dist(pa) > 12.0 && c.pos.dist(pb) > 12.0 {
            return None;
        }
    }
    // lots in the way
    let hw = RoadClass::Local.half_width();
    let mut lots = Vec::new();
    for li in ctx.index.lots.near(pa.lerp(pb, 0.5), l * 0.5 + 30.0) {
        let lot = &town.lots[li as usize];
        if dist_pt_seg(lot.centre, pa, pb) < hw + 1.0 + 0.55 * lot.radius {
            lots.push(li);
            if lots.len() > p.max_lots_removed {
                return None;
            }
        }
    }
    Some(Change::Connector {
        a,
        b,
        pa,
        pb,
        class: RoadClass::Local,
        lots_removed: lots,
    })
}
