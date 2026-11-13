//! Stage two: split the land into additions, lay each out, tie everything together.

use crate::geom::{
    Pt, area, area_signed, axis_angle, bbox, centroid, closed, cos, dedup, dist_ring_line, length,
    min_rect, open_ring, point_in_ring, pt, rotate_all, sin,
};
use crate::index::{SegIndex, uncovered_pieces};
use crate::lots;
use crate::patterns::{Layout, Line, curvy_local, grid_lines, pods};
use crate::polygonize::{polygonize, split_polygon};
use crate::rng::Rng;
use crate::skeleton::{self, highway};
use crate::world::{HALL, HWY_Y, River, S, dist_hall, map_ring, noise};
use crate::{Addition, CulDeSac, Pattern, Preset, Road, RoadClass, Town, Zone};

const REACH: f64 = 1900.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tag {
    Skeleton,
    Border,
    Interior,
    Cul,
}

#[derive(Clone, Debug)]
pub struct Draft {
    pub line: Line,
    pub class: RoadClass,
    pub tag: Tag,
    pub addition: usize,
}

pub struct Built {
    pub drafts: Vec<Draft>,
    pub additions: Vec<Addition>,
}

fn to_geo(ring: &[Pt]) -> geo::Polygon<f64> {
    geo::Polygon::new(
        geo::LineString::from(ring.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>()),
        vec![],
    )
}

fn ring_from_geo(p: &geo::Polygon<f64>) -> Vec<Pt> {
    let mut v: Vec<Pt> = p.exterior().coords().map(|c| pt(c.x, c.y)).collect();
    v = open_ring(&v);
    if area_signed(&v) < 0.0 {
        v.reverse();
    }
    v
}

fn poly_difference(a: &[Pt], b: &[Pt]) -> Vec<Vec<Pt>> {
    use geo::BooleanOps;
    let res = to_geo(a).difference(&to_geo(b));
    res.0.iter().map(ring_from_geo).collect()
}

fn touches_edge(p: &[Pt]) -> bool {
    p.iter()
        .any(|q| q.x <= 2.0 || q.x >= S - 2.0 || q.y >= S - 2.0)
}

fn make_patches(
    roads: &[(Line, RoadClass)],
    rails: &[Line],
    river: &River,
    rng: &mut Rng,
) -> Vec<Vec<Pt>> {
    let mut lines: Vec<Line> = roads.iter().map(|(l, _)| l.clone()).collect();
    lines.extend(rails.iter().cloned());
    lines.push(closed(&map_ring()));
    let band = river.band(-60.0, 60.0);
    let mut out = Vec::new();
    for sb in polygonize(&lines) {
        for p in poly_difference(&sb, &band) {
            let a = area(&p);
            // slivers (a thin strip between two roads that nearly meet) make no use of land
            if a < 3000.0 || min_rect(&p).2 < 30.0 {
                continue;
            }
            if a < 220000.0 {
                out.push(p);
                continue;
            }
            let (ang, l, w) = min_rect(&p);
            let c = centroid(&p);
            let n = ((l / 420.0) as usize).max(2);
            let mut cuts: Vec<Line> = Vec::new();
            for i in 1..n {
                let f = (i as f64 / n as f64 - 0.5) * l + rng.uniform(-40.0, 40.0);
                cuts.push(rotate_all(
                    &[pt(c.x + f, c.y - 4000.0), pt(c.x + f, c.y + 4000.0)],
                    c,
                    ang,
                ));
            }
            if w > 600.0 {
                cuts.push(rotate_all(
                    &[pt(c.x - 4000.0, c.y), pt(c.x + 4000.0, c.y)],
                    c,
                    ang,
                ));
            }
            let pieces = if cuts.is_empty() {
                vec![p.clone()]
            } else {
                split_polygon(&p, &cuts)
            };
            out.extend(
                pieces
                    .into_iter()
                    .filter(|q| area(q) > 3000.0 && min_rect(q).2 >= 30.0),
            );
        }
    }
    out
}

const CATALOGUE: [(f64, f64, bool); 7] = [
    (100.0, 80.0, true),
    (120.0, 75.0, true),
    (160.0, 70.0, false),
    (200.0, 80.0, false),
    (95.0, 95.0, true),
    (180.0, 65.0, true),
    (240.0, 90.0, false),
];

struct Specials {
    campus: bool,
    cemetery: bool,
    golf: bool,
    mall: bool,
}

pub fn generate(preset: Preset, seed: u64) -> Town {
    let mut river_rng = Rng::new(seed, 0x71e7);
    let river = River::new(&mut river_rng);
    let built = build(preset, seed, &river);
    let mut drafts = built.drafts;
    let additions = built.additions;

    for d in &mut drafts {
        snap_to_bounds(&mut d.line);
    }
    // cul-de-sac ends
    let mut culs: Vec<CulDeSac> = Vec::new();
    for d in &drafts {
        if d.tag == Tag::Cul {
            culs.push(CulDeSac {
                pos: *d.line.last().unwrap(),
                addition: d.addition,
            });
        }
    }

    // hall site and lots
    let mut idx = SegIndex::new(S, 64.0);
    for (i, d) in drafts.iter().enumerate() {
        idx.add_line(&d.line, i as u32);
    }
    let hall_radius = 14.0;
    let hall = pick_hall_site(&idx, &drafts, hall_radius, &river);
    let mut lot_rng = Rng::new(seed, 0x107);
    let water = river.band(-40.0, 40.0);
    let lots = lots::make_lots(
        &drafts,
        &culs,
        &additions,
        hall,
        hall_radius,
        &water,
        &mut lot_rng,
    );

    // output roads: big roads first, then streets by class
    drafts.sort_by_key(|d| match d.class {
        RoadClass::Highway => 0,
        RoadClass::Arterial => 1,
        RoadClass::Rail => 2,
        RoadClass::Collector => 3,
        RoadClass::Local => 4,
        RoadClass::Alley => 5,
    });
    let roads = drafts
        .into_iter()
        .map(|d| Road {
            class: d.class,
            points: d.line,
        })
        .collect();
    Town {
        preset,
        seed,
        size: S,
        highway_y: HWY_Y,
        roads,
        cul_de_sacs: culs,
        additions,
        lots,
        water: vec![water],
        hall,
        hall_radius,
    }
}

/// The overlay step leaves coordinates a millionth of a metre off the map bounds: put them back.
fn snap_to_bounds(line: &mut [Pt]) {
    let snap = |v: &mut f64, target: f64| {
        if (*v - target).abs() < 1e-4 {
            *v = target;
        }
    };
    for p in line {
        snap(&mut p.x, 0.0);
        snap(&mut p.x, S);
        snap(&mut p.y, S);
        snap(&mut p.y, HWY_Y);
    }
}

fn pick_hall_site(idx: &SegIndex, drafts: &[Draft], radius: f64, river: &River) -> Pt {
    let nominal = pt(HALL.x + 50.0, HALL.y + 40.0);
    let band = river.band(-40.0 - radius - 8.0, 40.0 + radius + 8.0);
    let ok = |c: Pt| {
        let mut clear = true;
        idx.for_each_within(c, radius + 14.0, |d, o| {
            if d < drafts[o as usize].class.half_width() + radius + 1.0 {
                clear = false;
            }
        });
        clear && !point_in_ring(c, &band)
    };
    if ok(nominal) {
        return nominal;
    }
    let mut r = 4.0;
    while r < 260.0 {
        let n = ((r * 0.5) as usize).max(8);
        for k in 0..n {
            let a = std::f64::consts::TAU * k as f64 / n as f64;
            let c = pt(nominal.x + r * cos(a), nominal.y + r * sin(a));
            if ok(c) {
                return c;
            }
        }
        r += 4.0;
    }
    nominal
}

pub fn build(preset: Preset, seed: u64, river: &River) -> Built {
    let mut rng = Rng::new(seed, 0xa11);
    let sk = skeleton::build(preset, seed, river);
    let main_ang = std::f64::consts::FRAC_PI_2;
    let rail_ang = sk.rail_angle;
    let patches = make_patches(&sk.roads, &sk.rails, river, &mut rng);

    let mut drafts: Vec<Draft> = Vec::new();
    for (l, c) in &sk.roads {
        drafts.push(Draft {
            line: l.clone(),
            class: *c,
            tag: Tag::Skeleton,
            addition: usize::MAX,
        });
    }
    for r in &sk.rails {
        drafts.push(Draft {
            line: r.clone(),
            class: RoadClass::Rail,
            tag: Tag::Skeleton,
            addition: usize::MAX,
        });
    }
    let big: Vec<Line> = sk
        .roads
        .iter()
        .filter(|(_, c)| matches!(c, RoadClass::Highway | RoadClass::Arterial))
        .map(|(l, _)| l.clone())
        .collect();
    let hwy = highway();

    let mut order: Vec<usize> = (0..patches.len()).collect();
    let keyed: Vec<(f64, f64, f64)> = patches
        .iter()
        .map(|p| {
            let c = centroid(p);
            (dist_hall(c), c.x, c.y)
        })
        .collect();
    order.sort_by(|&a, &b| {
        keyed[a]
            .0
            .total_cmp(&keyed[b].0)
            .then(keyed[a].1.total_cmp(&keyed[b].1))
            .then(keyed[a].2.total_cmp(&keyed[b].2))
    });

    let mut sp = Specials {
        campus: false,
        cemetery: false,
        golf: false,
        mall: false,
    };
    let mut additions: Vec<Addition> = Vec::new();
    let mut borders_wanted: Vec<usize> = Vec::new();

    for &pi in &order {
        let p = &patches[pi];
        let c = centroid(p);
        let d = dist_hall(c);
        let reff = REACH * (0.85 + 0.3 * noise(c.x, c.y));
        let near_rail = sk.rails.iter().any(|r| dist_ring_line(p, r) < 5.0);
        let near_hwy = dist_ring_line(p, &hwy) < 5.0;
        let ai = additions.len();
        if (touches_edge(p) && d > 1200.0) || d > reff {
            additions.push(Addition {
                polygon: p.clone(),
                zone: Zone::Farm,
                pattern: Pattern::Farmland,
                distance: d,
                entrances: vec![],
            });
            continue;
        }
        let r = rng.f64();
        let mut lay = Layout::default();
        let mut zone: Zone;
        let mut pattern: Pattern;
        let mut is_pod = false;
        let mut street_class = RoadClass::Local;
        let origin_c = c;
        if near_hwy && !sp.mall && (c.x - HALL.x).abs() < 900.0 && d > 600.0 {
            zone = Zone::Mall;
            pattern = Pattern::Mall;
            sp.mall = true;
            lay.streets = grid_lines(p, 0.0, 1e9, 160.0, (0.0, 0.0), false, HALL, 0.0, &mut rng).0;
        } else if near_hwy && d > 500.0 {
            zone = Zone::Industry;
            pattern = Pattern::Industry;
            lay.streets = grid_lines(p, 0.0, 220.0, 1e9, (0.0, 0.0), false, HALL, 0.0, &mut rng).0;
        } else if d < 520.0 {
            zone = Zone::Core;
            if preset == Preset::Grid {
                pattern = Pattern::OldTownGrid;
                let (s, l) = grid_lines(
                    p,
                    main_ang,
                    80.0,
                    100.0,
                    (0.0, 0.0),
                    true,
                    HALL,
                    0.0,
                    &mut rng,
                );
                lay.streets = s;
                lay.lanes = l;
            } else {
                pattern = Pattern::CrookedGrid;
                let a = main_ang + rng.uniform(-14.0, 14.0).to_radians();
                let ph = (rng.uniform(0.0, 85.0), rng.uniform(0.0, 70.0));
                let (s, l) = grid_lines(p, a, 70.0, 85.0, ph, false, HALL, 8.0, &mut rng);
                lay.streets = s;
                lay.lanes = l;
            }
        } else if near_rail && d < 1400.0 && r < 0.55 {
            zone = if r < 0.3 { Zone::Industry } else { Zone::Inner };
            pattern = if zone == Zone::Industry {
                Pattern::Industry
            } else {
                Pattern::RailGrid
            };
            let ph = (rng.uniform(0.0, 140.0), 0.0);
            let (s, l) = grid_lines(
                p,
                rail_ang,
                90.0,
                140.0,
                ph,
                zone == Zone::Inner,
                HALL,
                0.0,
                &mut rng,
            );
            lay.streets = s;
            lay.lanes = l;
        } else if d < 1100.0 && !sp.cemetery && r < 0.1 {
            zone = Zone::Park;
            pattern = Pattern::Cemetery;
            sp.cemetery = true;
        } else if d > 650.0 && d < 1200.0 && !sp.campus && area(p) > 120000.0 {
            zone = Zone::Campus;
            pattern = Pattern::Campus;
            sp.campus = true;
            street_class = RoadClass::Collector;
            let (s, cs) = curvy_local(p, &mut rng, 150.0, 40.0, Some(260.0), false, 170.0);
            lay.streets = s;
            lay.culs = cs;
        } else if d < 1150.0 {
            zone = Zone::Inner;
            if preset == Preset::Grid {
                pattern = Pattern::CatalogueGrid;
                let (depth, len, al) = CATALOGUE[rng.below(CATALOGUE.len())];
                let ang = if rng.f64() < 0.65 {
                    main_ang
                } else if rng.f64() < 0.5 {
                    rail_ang
                } else {
                    axis_angle(p)
                };
                let ph = (rng.uniform(0.0, len), rng.uniform(0.0, depth));
                let (s, l) = grid_lines(p, ang, depth, len, ph, al, origin_c, 0.0, &mut rng);
                lay.streets = s;
                lay.lanes = l;
            } else {
                pattern = Pattern::CrookedGrid;
                let (s, cs) = curvy_local(p, &mut rng, 85.0, 22.0, Some(180.0), false, 140.0);
                lay.streets = s;
                lay.culs = cs;
            }
        } else if d < 1500.0 {
            zone = Zone::Outer;
            if preset == Preset::Grid && r < 0.4 {
                pattern = Pattern::LongBlockGrid;
                let ph = (rng.uniform(0.0, 260.0), rng.uniform(0.0, 75.0));
                let (s, l) = grid_lines(
                    p, main_ang, 75.0, 260.0, ph, false, origin_c, 14.0, &mut rng,
                );
                lay.streets = s;
                lay.lanes = l;
            } else if !sp.golf && r > 0.85 && area(p) > 150000.0 {
                zone = Zone::Park;
                pattern = Pattern::Golf;
                sp.golf = true;
                let (s, cs) = curvy_local(p, &mut rng, 1e9, 60.0, None, false, 200.0);
                lay.streets = s;
                lay.culs = cs;
            } else {
                pattern = Pattern::Loops;
                let (s, cs) = curvy_local(p, &mut rng, 110.0, 38.0, Some(320.0), r < 0.6, 150.0);
                lay.streets = s;
                lay.culs = cs;
            }
        } else {
            zone = Zone::Outer;
            let entries: Vec<&[Pt]> = big
                .iter()
                .filter(|e| dist_ring_line(p, e) < 3.0)
                .map(|e| e.as_slice())
                .collect();
            let pod = if r < 0.7 && !entries.is_empty() {
                pods(p, &entries, &mut rng)
            } else {
                None
            };
            if let Some(l) = pod {
                pattern = Pattern::Pods;
                is_pod = true;
                lay = l;
            } else {
                pattern = Pattern::Loops;
                let (s, cs) = curvy_local(p, &mut rng, 120.0, 42.0, None, true, 150.0);
                lay.streets = s;
                lay.culs = cs;
            }
        }
        if zone != Zone::Park && rng.f64() < 0.05 && d > 600.0 {
            zone = Zone::Park;
            pattern = Pattern::Park;
            is_pod = false;
            lay = Layout::default();
        }
        let has_streets = !lay.streets.is_empty();
        additions.push(Addition {
            polygon: p.clone(),
            zone,
            pattern,
            distance: d,
            entrances: lay.entrances.clone(),
        });
        for s in lay.streets {
            drafts.push(Draft {
                line: s,
                class: street_class,
                tag: Tag::Interior,
                addition: ai,
            });
        }
        for l in lay.lanes {
            drafts.push(Draft {
                line: l,
                class: RoadClass::Alley,
                tag: Tag::Interior,
                addition: ai,
            });
        }
        for q in lay.culs {
            drafts.push(Draft {
                line: q,
                class: RoadClass::Local,
                tag: Tag::Cul,
                addition: ai,
            });
        }
        if !is_pod && (zone != Zone::Park || has_streets) {
            borders_wanted.push(ai);
        }
    }

    trim_covered_interiors(&mut drafts);
    add_border_streets(&mut drafts, &additions, &borders_wanted);
    connect_ends(&mut drafts);
    promote_collectors(&mut drafts);
    Built { drafts, additions }
}

/// A grid street that happens to run along a skeleton road, or along a street of the next
/// addition over, is not a second road: cut it away.
fn trim_covered_interiors(drafts: &mut Vec<Draft>) {
    let mut cover = SegIndex::new(S, 64.0);
    for (i, d) in drafts.iter().enumerate() {
        if d.tag == Tag::Skeleton {
            cover.add_line(&d.line, i as u32);
        }
    }
    let mut out: Vec<Draft> = Vec::with_capacity(drafts.len());
    for d in drafts.drain(..) {
        if d.tag == Tag::Skeleton {
            out.push(d);
            continue;
        }
        for piece in uncovered_pieces(&d.line, &cover, 2.5, 1.5) {
            let piece = dedup(&piece);
            if length(&piece) >= 12.0 {
                cover.add_line(&piece, u32::MAX);
                out.push(Draft {
                    line: piece,
                    ..d.clone()
                });
            }
        }
    }
    *drafts = out;
}

/// Addition edges become streets, except where a bigger road, rail, the map edge or an earlier
/// street already runs there.
fn add_border_streets(drafts: &mut Vec<Draft>, additions: &[Addition], wanted: &[usize]) {
    let mut cover = SegIndex::new(S, 64.0);
    for (i, d) in drafts.iter().enumerate() {
        cover.add_line(&d.line, i as u32);
    }
    for e in [
        vec![pt(0.0, HWY_Y), pt(0.0, S)],
        vec![pt(S, HWY_Y), pt(S, S)],
        vec![pt(0.0, S), pt(S, S)],
        vec![pt(0.0, HWY_Y), pt(S, HWY_Y)],
    ] {
        cover.add_line(&e, u32::MAX - 1);
    }
    for &ai in wanted {
        let ring = closed(&additions[ai].polygon);
        for piece in uncovered_pieces(&ring, &cover, 2.5, 1.5) {
            let piece = dedup(&piece);
            if length(&piece) < 12.0 {
                continue;
            }
            cover.add_line(&piece, u32::MAX);
            drafts.push(Draft {
                line: piece,
                class: RoadClass::Local,
                tag: Tag::Border,
                addition: ai,
            });
        }
    }
}

fn is_street(d: &Draft) -> bool {
    matches!(
        d.class,
        RoadClass::Local | RoadClass::Alley | RoadClass::Collector
    )
}

/// Every street end must sit on another road or be a cul-de-sac tip. Snap near misses, extend
/// short gaps along the street, drop what cannot be tied in.
fn connect_ends(drafts: &mut Vec<Draft>) {
    for _pass in 0..6 {
        let mut idx = SegIndex::new(S, 64.0);
        for (i, d) in drafts.iter().enumerate() {
            idx.add_line(&d.line, i as u32);
        }
        let mut changed = false;
        let mut dead: Vec<usize> = Vec::new();
        for i in 0..drafts.len() {
            if !is_street(&drafts[i]) {
                continue;
            }
            let l = &drafts[i].line;
            if l.len() < 2 || l[0].dist(*l.last().unwrap()) < 1e-6 {
                continue;
            }
            let ends: [bool; 2] = [true, false];
            let mut line = l.clone();
            let mut dropped = false;
            for &front in &ends {
                if drafts[i].tag == Tag::Cul && !front {
                    continue;
                }
                let e = if front {
                    line[0]
                } else {
                    *line.last().unwrap()
                };
                if idx.nearest(e, 1.2, Some(i as u32)).is_some() {
                    continue;
                }
                let dir = if front {
                    (line[0] - line[1]).unit()
                } else {
                    (line[line.len() - 1] - line[line.len() - 2]).unit()
                };
                // a ray along the street first, then the closest road
                let target = idx
                    .nearest(e, 12.0, Some(i as u32))
                    .map(|h| h.pt)
                    .or_else(|| {
                        ray_hit(drafts, i, e, dir, 90.0).filter(|t| {
                            // never run the extension along another road
                            [0.25, 0.5, 0.75]
                                .iter()
                                .all(|f| idx.nearest(e.lerp(*t, *f), 1.5, Some(i as u32)).is_none())
                        })
                    })
                    .or_else(|| idx.nearest(e, 22.0, Some(i as u32)).map(|h| h.pt));
                match target {
                    Some(t) if own_overshoot(&line, t, front) => {
                        // the street ran a little past the road: cut the overshoot off
                        line = trim_end_to(&line, t, front);
                        changed = true;
                    }
                    Some(t) => {
                        if front {
                            line.insert(0, t);
                        } else {
                            line.push(t);
                        }
                        changed = true;
                    }
                    None => {
                        dropped = true;
                    }
                }
            }
            if dropped {
                dead.push(i);
                changed = true;
            } else {
                drafts[i].line = line;
            }
        }
        for &i in dead.iter().rev() {
            drafts.remove(i);
        }
        if !changed {
            break;
        }
    }
}

/// True when `t` lies on the street itself, near the given end (an overshoot past a crossing road).
fn own_overshoot(line: &[Pt], t: Pt, front: bool) -> bool {
    let (along, dist, _) = crate::geom::project(line, t);
    let total = length(line);
    let from_end = if front { along } else { total - along };
    dist < 0.3 && from_end > 0.2 && from_end < 20.0
}

fn trim_end_to(line: &[Pt], t: Pt, front: bool) -> Vec<Pt> {
    let (along, _, _) = crate::geom::project(line, t);
    let total = length(line);
    let mut out: Vec<Pt> = Vec::new();
    let mut acc = 0.0;
    if front {
        out.push(t);
        for w in line.windows(2) {
            acc += w[0].dist(w[1]);
            if acc > along + 1e-9 {
                out.push(w[1]);
            }
        }
    } else {
        for w in line.windows(2) {
            if acc < along - 1e-9 {
                out.push(w[0]);
            }
            acc += w[0].dist(w[1]);
        }
        out.push(t);
        let _ = total;
    }
    dedup(&out)
}

/// First other road met walking from `from` along `dir` for up to `max` metres.
fn ray_hit(drafts: &[Draft], own: usize, from: Pt, dir: Pt, max: f64) -> Option<Pt> {
    let end = from + dir * max;
    let mut best: Option<(f64, Pt)> = None;
    let (lo, hi) = (
        pt(from.x.min(end.x) - 1.0, from.y.min(end.y) - 1.0),
        pt(from.x.max(end.x) + 1.0, from.y.max(end.y) + 1.0),
    );
    for (j, d) in drafts.iter().enumerate() {
        if j == own {
            continue;
        }
        let (blo, bhi) = bbox(&d.line);
        if bhi.x < lo.x || blo.x > hi.x || bhi.y < lo.y || blo.y > hi.y {
            continue;
        }
        for w in d.line.windows(2) {
            if let Some((t, _)) = crate::geom::seg_intersect(from, end, w[0], w[1])
                && best.is_none_or(|b| t * max < b.0)
            {
                best = Some((t * max, from + dir * (t * max)));
            }
        }
    }
    best.map(|b| b.1)
}

/// Long continuous addition-edge streets become collectors.
fn promote_collectors(drafts: &mut [Draft]) {
    let border: Vec<usize> = (0..drafts.len())
        .filter(|&i| drafts[i].tag == Tag::Border && drafts[i].class == RoadClass::Local)
        .collect();
    let n = border.len();
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
    // ends: (point, outgoing direction, owner)
    let mut ends: Vec<(Pt, Pt, usize)> = Vec::new();
    for (k, &i) in border.iter().enumerate() {
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
            if straight > 0.82 && best.is_none_or(|x| straight > x.0) {
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
    let mut total: Vec<f64> = vec![0.0; n];
    for k in 0..n {
        let r = find(&mut parent, k);
        total[r] += length(&drafts[border[k]].line);
    }
    for k in 0..n {
        let r = find(&mut parent, k);
        if total[r] >= 800.0 {
            drafts[border[k]].class = RoadClass::Collector;
        }
    }
}
