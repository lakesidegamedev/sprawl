//! Stage three: lots along the streets.

use crate::geom::{
    Pt, atan2, bbox, cos, dist_pt_ring, interpolate, length, point_in_ring, project, pt,
    seg_intersect, sin,
};
use crate::plan::Draft;
use crate::rng::Rng;
use crate::terrain::Terrain;
use crate::world::{HWY_Y, S, dist_hall};
use crate::{
    Addition, Centre, CentreKind, CulDeSac, Lot, LotKind, Preset, RoadClass, Station, StationKind,
    Zone, centrality,
};

/// Disks in a flat grid, for fast overlap tests.
struct DiskHash {
    cell: f64,
    n: i64,
    cells: Vec<Vec<(f64, f64, f64)>>,
}

impl DiskHash {
    fn new() -> DiskHash {
        let cell = 40.0;
        let n = (S / cell) as i64 + 8;
        DiskHash {
            cell,
            n,
            cells: vec![Vec::new(); (n * n) as usize],
        }
    }

    fn ci(&self, v: f64) -> i64 {
        (((v / self.cell).floor() as i64) + 2).clamp(0, self.n - 1)
    }

    fn add(&mut self, x: f64, y: f64, r: f64) {
        let (i, j) = (self.ci(x), self.ci(y));
        self.cells[(j * self.n + i) as usize].push((x, y, r));
    }

    fn hit(&self, x: f64, y: f64, r: f64) -> bool {
        let reach = r + 60.0;
        for i in self.ci(x - reach)..=self.ci(x + reach) {
            for j in self.ci(y - reach)..=self.ci(y + reach) {
                for &(a, b, rr) in &self.cells[(j * self.n + i) as usize] {
                    let (dx, dy) = (a - x, b - y);
                    let lim = r + rr;
                    if dx * dx + dy * dy < lim * lim {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Lz {
    Z(Zone),
    Fringe,
    Open,
}

fn kind_from(i: usize, list: &[LotKind]) -> LotKind {
    list[i]
}

struct Ctx<'a> {
    additions: &'a [Addition],
    boxes: Vec<(Pt, Pt)>,
    hall: Pt,
    offices_left: u32,
    /// Generator version 2 rules: centres shape the lot kinds, parks stay clear.
    v2: bool,
    centres: Vec<Centre>,
    /// Indices of park additions.
    parks: Vec<usize>,
}

impl Ctx<'_> {
    /// True when a disk at `c` reaches into a park (version 2 rules only).
    fn touches_park(&self, c: Pt, r: f64) -> bool {
        if !self.v2 {
            return false;
        }
        self.parks.iter().any(|&i| {
            let (lo, hi) = self.boxes[i];
            if c.x < lo.x - r || c.x > hi.x + r || c.y < lo.y - r || c.y > hi.y + r {
                return false;
            }
            let poly = &self.additions[i].polygon;
            point_in_ring(c, poly) || dist_pt_ring(c, poly) < r
        })
    }

    fn central(&self, p: Pt) -> f64 {
        centrality(&self.centres, p)
    }

    fn zone_at(&self, p: Pt) -> (Lz, f64) {
        for (i, a) in self.additions.iter().enumerate() {
            let (lo, hi) = self.boxes[i];
            if p.x < lo.x || p.x > hi.x || p.y < lo.y || p.y > hi.y {
                continue;
            }
            if point_in_ring(p, &a.polygon) {
                if a.zone == Zone::Farm {
                    return (
                        if a.distance < 2600.0 {
                            Lz::Fringe
                        } else {
                            Lz::Open
                        },
                        a.distance,
                    );
                }
                return (Lz::Z(a.zone), a.distance);
            }
        }
        (Lz::Open, dist_hall(p))
    }

    fn lot_kind(
        &mut self,
        rng: &mut Rng,
        zone: Zone,
        class: RoadClass,
        dc: f64,
        at: Pt,
    ) -> LotKind {
        use LotKind::*;
        let big = matches!(
            class,
            RoadClass::Highway | RoadClass::Arterial | RoadClass::Collector
        );
        if self.v2 {
            let c = self.central(at);
            match zone {
                Zone::Core if dc < 380.0 => {
                    let tower_ok = c > 0.6 && self.offices_left > 0;
                    let w_office = if !tower_ok {
                        0.0
                    } else if big {
                        2.4 * c
                    } else {
                        0.9 * c
                    };
                    return kind_from(
                        rng.weighted(&[5.0, 3.0 + 2.0 * c, w_office, 1.0, 1.0 - 0.6 * c]),
                        &[Shop, Apartments, Office, Parking, Row],
                    );
                }
                Zone::Inner if c > 0.4 => {
                    // the blocks round the station or a big crossroads: shops and flats
                    let shop = if big { 4.5 * c } else { 2.5 * c };
                    return kind_from(
                        rng.weighted(&[
                            6.0 * (1.0 - c),
                            2.0,
                            1.0 + 2.5 * c,
                            shop,
                            0.12,
                            0.4 * c,
                            0.1,
                            0.1,
                        ]),
                        &[
                            House, Row, Apartments, Shop, Church, Parking, School, Sports,
                        ],
                    );
                }
                Zone::Outer if c > 0.45 && big => {
                    // a neighbourhood centre at a crossroads out in the suburbs
                    return kind_from(
                        rng.weighted(&[16.0, 6.0 * c, 4.0 * c, 1.0, 1.2, 0.25, 0.25]),
                        &[House, Shop, Apartments, Gas, FastFood, School, Sports],
                    );
                }
                _ => {}
            }
        }
        match zone {
            Zone::Core => {
                if dc < 380.0 {
                    let tower_ok = at.dist(self.hall) < 250.0 && self.offices_left > 0;
                    let w_office = if !tower_ok {
                        0.0
                    } else if big {
                        1.4
                    } else {
                        0.4
                    };
                    kind_from(
                        rng.weighted(&[5.0, 3.0, w_office, 1.2, 1.0]),
                        &[Shop, Apartments, Office, Parking, Row],
                    )
                } else {
                    kind_from(
                        rng.weighted(&[5.0, 2.0, 3.0, 1.0, 0.6, 0.15]),
                        &[Row, Shop, House, Apartments, Parking, Church],
                    )
                }
            }
            Zone::Industry => kind_from(
                rng.weighted(&[4.0, 2.0, 1.0]),
                &[Warehouse, Workshop, Parking],
            ),
            Zone::Mall => kind_from(
                rng.weighted(&[3.0, 2.0, 1.0, 0.6]),
                &[BigBox, Parking, FastFood, Gas],
            ),
            Zone::Campus => kind_from(
                rng.weighted(&[3.0, 3.0, 2.0, 2.0, 0.3]),
                &[School, Apartments, Sports, Parking, Church],
            ),
            _ if class == RoadClass::Arterial => kind_from(
                rng.weighted(&[4.0, 2.0, 1.2, 2.0, 1.0, 1.0]),
                &[Strip, FastFood, Gas, Shop, Parking, Apartments],
            ),
            Zone::Inner => kind_from(
                rng.weighted(&[
                    8.0,
                    2.0,
                    if class == RoadClass::Collector {
                        0.8
                    } else {
                        0.1
                    },
                    0.12,
                    0.08,
                    0.08,
                ]),
                &[House, Row, Apartments, Church, School, Sports],
            ),
            _ => kind_from(rng.weighted(&[30.0, 0.25, 0.25]), &[House, School, Sports]),
        }
    }
}

fn road_disks(h: &mut DiskHash, line: &[Pt], hw: f64) {
    let total = length(line);
    let step = hw.max(4.0);
    let mut k = 0.0;
    while k <= total {
        let p = interpolate(line, k).0;
        h.add(p.x, p.y, hw + 1.0);
        k += step;
    }
    for p in line {
        h.add(p.x, p.y, hw + 1.0);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn make_lots(
    drafts: &[Draft],
    culs: &[CulDeSac],
    additions: &[Addition],
    hall: Pt,
    hall_r: f64,
    water: &[Vec<Pt>],
    terrain: &Terrain,
    rng: &mut Rng,
    preset: Preset,
    v2: bool,
) -> (Vec<Lot>, Vec<Station>, Vec<Centre>) {
    let mut h = DiskHash::new();
    for d in drafts {
        road_disks(&mut h, &d.line, d.class.half_width());
    }
    for c in culs {
        h.add(c.pos.x, c.pos.y, 9.0);
    }
    // the island of a roundabout stays clear
    for d in drafts {
        if d.class == RoadClass::Collector
            && d.line.len() == 33
            && d.line[0].dist(d.line[32]) < 1e-6
        {
            let n = 32.0;
            let c = pt(
                d.line[..32].iter().map(|p| p.x).sum::<f64>() / n,
                d.line[..32].iter().map(|p| p.y).sum::<f64>() / n,
            );
            if d.line.iter().all(|p| (p.dist(c) - crate::junctions::RING_R).abs() < 0.3) {
                h.add(c.x, c.y, crate::junctions::RING_R - 6.0);
            }
        }
    }
    h.add(hall.x, hall.y, hall_r + 10.0);
    let mut ctx = Ctx {
        additions,
        boxes: additions.iter().map(|a| bbox(&a.polygon)).collect(),
        hall,
        offices_left: if v2 { 14 } else { 5 },
        v2,
        centres: Vec::new(),
        parks: (0..additions.len())
            .filter(|&i| additions[i].zone == Zone::Park)
            .collect(),
    };
    let mut lots: Vec<Lot> = Vec::new();
    let wboxes: Vec<(Pt, Pt)> = water.iter().map(|w| bbox(w)).collect();

    let site_ok = |c: Pt, r: f64| -> bool {
        let m = 20.0_f64.max(r + 2.0);
        if c.x < m || c.x > S - m || c.y < HWY_Y + m || c.y > S - m {
            return false;
        }
        for (w, (wlo, whi)) in water.iter().zip(&wboxes) {
            let near_box = c.x > wlo.x - 200.0
                && c.x < whi.x + 200.0
                && c.y > wlo.y - 200.0
                && c.y < whi.y + 200.0;
            if near_box {
                let clear = 25.0_f64.max(r + 6.0);
                if point_in_ring(c, w) || dist_pt_ring(c, w) < clear {
                    return false;
                }
            }
        }
        // steep hillsides stay unbuilt
        terrain.slope(c) < 0.15
    };

    let stations = if v2 {
        let mut srng = Rng::new((rng.f64() * 9.0e15) as u64, 0x57a7);
        place_stations(drafts, hall, preset, &site_ok, &mut h, &ctx, &mut srng)
    } else {
        Vec::new()
    };
    ctx.centres = find_centres(drafts, additions, hall, &stations);

    // cul-de-sac bulbs: a ring of houses round the turning circle
    for c in culs {
        let Some(d) = drafts.iter().find(|d| {
            d.tag == crate::plan::Tag::Cul && d.line.last().is_some_and(|e| e.dist(c.pos) < 1e-9)
        }) else {
            continue;
        };
        let a = atan2(
            d.line[d.line.len() - 1].y - d.line[0].y,
            d.line[d.line.len() - 1].x - d.line[0].x,
        );
        for da in [-1.6, -0.8, 0.0, 0.8, 1.6] {
            let ang = a + da;
            let r = 8.0;
            let ctr = pt(c.pos.x + cos(ang) * 22.0, c.pos.y + sin(ang) * 22.0);
            let zone = if v2 {
                match ctx.zone_at(ctr).0 {
                    Lz::Z(Zone::Park) => continue,
                    Lz::Z(z) => z,
                    _ => Zone::Outer,
                }
            } else {
                Zone::Outer
            };
            if site_ok(ctr, r) && !h.hit(ctr.x, ctr.y, r) && !ctx.touches_park(ctr, r) {
                lots.push(Lot {
                    centre: ctr,
                    radius: r,
                    kind: LotKind::House,
                    facing: atan2(cos(ang), -sin(ang)) + std::f64::consts::PI,
                    zone,
                    variant: rng.f64(),
                    density: 0.0,
                });
                h.add(ctr.x, ctr.y, r);
            }
        }
    }

    let mut order: Vec<usize> = (0..drafts.len()).collect();
    order.sort_by_key(|&i| match drafts[i].class {
        RoadClass::Highway => 0,
        RoadClass::Arterial => 1,
        RoadClass::Collector => 2,
        RoadClass::Local => 3,
        _ => 9,
    });
    for i in order {
        let d = &drafts[i];
        let class = d.class;
        if matches!(class, RoadClass::Alley | RoadClass::Rail) {
            continue;
        }
        let hw = class.half_width();
        let sides: &[f64] = if class == RoadClass::Highway {
            &[1.0]
        } else {
            &[1.0, -1.0]
        };
        let total = length(&d.line);
        for &side in sides {
            // `k` is where the next lot's centre would sit along the street; `edge` is where the
            // last lot ended, so lots are packed shoulder to shoulder with only the zone's gap.
            let mut k = 7.0;
            let mut edge = 0.0_f64;
            while k < total - 7.0 {
                let (p, u) = interpolate(&d.line, k);
                let n = pt(-u.y * side, u.x * side);
                let probe = pt(p.x + n.x * (hw + 8.0), p.y + n.y * (hw + 8.0));
                let (lz, dc) = ctx.zone_at(probe);
                let (zone, kind) = match lz {
                    Lz::Z(Zone::Park) => {
                        k += 14.0;
                        continue;
                    }
                    Lz::Open => {
                        k += 14.0 * rng.uniform(0.95, 1.15);
                        if class != RoadClass::Arterial || !rng.chance(0.02) {
                            continue;
                        }
                        (Zone::Farm, LotKind::Farm)
                    }
                    Lz::Fringe => {
                        k += 14.0 * rng.uniform(0.95, 1.15);
                        if class != RoadClass::Arterial || rng.f64() > 0.22 {
                            continue;
                        }
                        let kk = rng.weighted(&[2.0, 1.5, 1.5, 2.0, 1.0]);
                        (
                            Zone::Farm,
                            [
                                LotKind::Strip,
                                LotKind::Gas,
                                LotKind::FastFood,
                                LotKind::House,
                                LotKind::Warehouse,
                            ][kk],
                        )
                    }
                    Lz::Z(z) => {
                        let at = pt(p.x + n.x * (hw + 12.0), p.y + n.y * (hw + 12.0));
                        (z, ctx.lot_kind(rng, z, class, dc, at))
                    }
                };
                let r = kind.nominal_radius();
                let attached = attached_at(zone, dc, kind);
                let rc = if attached { r * 0.85 } else { r };
                let (gap, setback) = frontage(zone, dc, kind, attached, rng);
                // pack against the last lot, but never behind where the search stands
                let kc = if zone == Zone::Farm {
                    k
                } else {
                    (edge + gap + rc).max(k)
                };
                let (pc, uc) = interpolate(&d.line, kc.min(total - 1.0));
                let nc = pt(-uc.y * side, uc.x * side);
                let off = hw + setback + rc;
                let ctr = pt(pc.x + nc.x * off, pc.y + nc.y * off);
                let office_too_far = kind == LotKind::Office
                    && if v2 {
                        ctx.central(ctr) < 0.55
                    } else {
                        ctr.dist(hall) > 245.0
                    };
                // version 2: the zone is checked again where the building really stands, so
                // packing along the street never carries a lot into a park or open land
                let mut zone = zone;
                let mut misplaced = false;
                if v2 {
                    match ctx.zone_at(ctr).0 {
                        Lz::Z(Zone::Park) => misplaced = true,
                        Lz::Open | Lz::Fringe => misplaced |= zone != Zone::Farm,
                        Lz::Z(z) if zone != Zone::Farm => zone = z,
                        Lz::Z(_) => {}
                    }
                    misplaced |= ctx.touches_park(ctr, rc);
                }
                if misplaced || office_too_far || !site_ok(ctr, rc) || h.hit(ctr.x, ctr.y, rc) {
                    // blocked by a cross street or a neighbour: try again a little further on
                    k = kc + 3.0;
                    continue;
                }
                if kind == LotKind::Office {
                    ctx.offices_left = ctx.offices_left.saturating_sub(1);
                }
                lots.push(Lot {
                    centre: ctr,
                    radius: rc,
                    kind,
                    facing: atan2(-nc.x, nc.y),
                    zone,
                    variant: rng.f64(),
                    density: 0.0,
                });
                h.add(ctr.x, ctr.y, rc);
                if zone == Zone::Farm {
                    k = kc + 300.0;
                } else {
                    edge = kc + rc;
                    k = edge + 1.0;
                }
            }
        }
    }

    // Back lots: a second and third row behind the street lots, so blocks are no longer hollow.
    // Outer suburbs keep their big back yards and only get an occasional back house.
    let front = lots.len();
    for i in 0..front {
        let mut cur = lots[i].clone();
        for depth in 0..3 {
            let Some(next) = back_lot(&mut ctx, rng, &cur, depth, &site_ok, &h) else {
                break;
            };
            h.add(next.centre.x, next.centre.y, next.radius);
            lots.push(next.clone());
            cur = next;
        }
    }
    for l in &mut lots {
        l.density = lot_density(l.zone, ctx.central(l.centre));
    }
    (lots, stations, ctx.centres)
}

/// How dense a lot may become, from its zone and how central it is.
fn lot_density(zone: Zone, central: f64) -> f64 {
    let (base, mult) = match zone {
        Zone::Core => (0.3, 0.9),
        Zone::Inner => (0.1, 0.85),
        Zone::Outer => (0.0, 0.55),
        Zone::Industry => (0.1, 0.5),
        Zone::Mall => (0.15, 0.6),
        Zone::Campus => (0.1, 0.6),
        Zone::Park | Zone::Farm => (0.0, 0.2),
    };
    (base + mult * central).clamp(0.0, 1.0)
}

/// Where the town gathers: downtown round the hall, the station blocks, crossings of big roads
/// and malls. Strongest first.
fn find_centres(
    drafts: &[Draft],
    additions: &[Addition],
    hall: Pt,
    stations: &[Station],
) -> Vec<Centre> {
    let mut out = vec![Centre {
        kind: CentreKind::Downtown,
        pos: hall,
        weight: 1.0,
        radius: 560.0,
    }];
    for s in stations.iter().filter(|s| s.kind == StationKind::Depot) {
        out.push(Centre {
            kind: CentreKind::Station,
            pos: s.pos,
            weight: 0.72,
            radius: 300.0,
        });
    }
    for a in additions.iter().filter(|a| a.zone == Zone::Mall) {
        out.push(Centre {
            kind: CentreKind::Mall,
            pos: crate::geom::centroid(&a.polygon),
            weight: 0.4,
            radius: 260.0,
        });
    }
    // crossings of big roads: arterial with arterial or highway, then arterial with collector
    let big: Vec<&Draft> = drafts
        .iter()
        .filter(|d| matches!(d.class, RoadClass::Highway | RoadClass::Arterial))
        .collect();
    let coll: Vec<&Draft> = drafts
        .iter()
        .filter(|d| d.class == RoadClass::Collector)
        .collect();
    let crossings = |a: &[&Draft], b: &[&Draft], same: bool, max_d: f64| -> Vec<Pt> {
        let mut pts: Vec<Pt> = Vec::new();
        for (i, da) in a.iter().enumerate() {
            let (alo, ahi) = bbox(&da.line);
            for (j, db) in b.iter().enumerate() {
                if same && j <= i {
                    continue;
                }
                if same && da.class == RoadClass::Highway && db.class == RoadClass::Highway {
                    continue;
                }
                let (blo, bhi) = bbox(&db.line);
                if bhi.x < alo.x || blo.x > ahi.x || bhi.y < alo.y || blo.y > ahi.y {
                    continue;
                }
                for wa in da.line.windows(2) {
                    for wb in db.line.windows(2) {
                        if let Some((t, _)) = seg_intersect(wa[0], wa[1], wb[0], wb[1]) {
                            let p = wa[0].lerp(wa[1], t);
                            if p.dist(hall) < max_d {
                                pts.push(p);
                            }
                        }
                    }
                }
            }
        }
        pts.sort_by(|p, q| {
            p.dist(hall)
                .total_cmp(&q.dist(hall))
                .then(p.x.total_cmp(&q.x))
                .then(p.y.total_cmp(&q.y))
        });
        pts
    };
    let add = |out: &mut Vec<Centre>, pts: Vec<Pt>, weight: f64, radius: f64, cap: usize| {
        let mut n = 0;
        for p in pts {
            if n >= cap {
                break;
            }
            if out
                .iter()
                .any(|c| c.pos.dist(p) < 0.6 * c.radius.max(radius))
            {
                continue;
            }
            out.push(Centre {
                kind: CentreKind::Crossroads,
                pos: p,
                weight,
                radius,
            });
            n += 1;
        }
    };
    add(&mut out, crossings(&big, &big, true, 2000.0), 0.5, 240.0, 8);
    add(
        &mut out,
        crossings(&big, &coll, false, 1600.0),
        0.32,
        170.0,
        12,
    );
    out.sort_by(|a, b| {
        b.weight
            .total_cmp(&a.weight)
            .then(a.pos.x.total_cmp(&b.pos.x))
            .then(a.pos.y.total_cmp(&b.pos.y))
    });
    out
}

/// A depot on each railway where it passes closest to downtown, beside the street that crosses
/// the track there, and in prairie towns a grain elevator across the track.
fn place_stations(
    drafts: &[Draft],
    hall: Pt,
    preset: Preset,
    site_ok: &dyn Fn(Pt, f64) -> bool,
    h: &mut DiskHash,
    ctx: &Ctx,
    rng: &mut Rng,
) -> Vec<Station> {
    const BUILDING_R: f64 = 13.0;
    const SET_BACK: f64 = 18.0;
    const PLATFORM: f64 = 48.0;
    let mut out = Vec::new();
    for rail in drafts.iter().filter(|d| d.class == RoadClass::Rail) {
        let line = &rail.line;
        let total = length(line);
        // where streets cross the track, nearest the hall first; the closest point as a fallback
        let mut cands: Vec<(f64, f64)> = Vec::new();
        for d in drafts.iter().filter(|d| {
            matches!(
                d.class,
                RoadClass::Arterial | RoadClass::Collector | RoadClass::Local
            )
        }) {
            for wa in line.windows(2) {
                for wb in d.line.windows(2) {
                    if let Some((t, _)) = seg_intersect(wa[0], wa[1], wb[0], wb[1]) {
                        let p = wa[0].lerp(wa[1], t);
                        let bonus = if d.class == RoadClass::Local {
                            60.0
                        } else {
                            0.0
                        };
                        cands.push((p.dist(hall) + bonus, project(line, p).0));
                    }
                }
            }
        }
        let (along_h, dist_h, _) = project(line, hall);
        cands.push((dist_h + 200.0, along_h));
        cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        'cand: for &(dh, a0) in &cands {
            if dh > 1500.0 {
                break;
            }
            for off in [30.0, -30.0, 46.0, -46.0, 64.0, -64.0] {
                let a = a0 + off;
                if a < PLATFORM || a > total - PLATFORM {
                    continue;
                }
                let (p, u) = interpolate(line, a);
                let mut n = pt(-u.y, u.x);
                if (hall - p).dot(n) < 0.0 {
                    n = n * -1.0;
                }
                let ctr = p + n * SET_BACK;
                let mut disks = vec![(ctr, BUILDING_R)];
                let mut k = -PLATFORM / 2.0;
                while k <= PLATFORM / 2.0 {
                    disks.push((p + n * 7.5 + u * k, 3.0));
                    k += 8.0;
                }
                let clear = disks
                    .iter()
                    .all(|&(c, r)| site_ok(c, r) && !h.hit(c.x, c.y, r) && !ctx.touches_park(c, r));
                if !clear {
                    continue;
                }
                for &(c, r) in &disks {
                    h.add(c.x, c.y, r);
                }
                out.push(Station {
                    kind: StationKind::Depot,
                    pos: ctr,
                    facing: atan2(n.x, -n.y),
                    radius: BUILDING_R,
                    length: PLATFORM,
                });
                if preset == Preset::Grid {
                    // the elevator stands across the track, a little down the line
                    let sign = if rng.chance(0.5) { 1.0 } else { -1.0 };
                    for step in 0..24usize {
                        let mag = 80.0 + 15.0 * (step / 2) as f64;
                        let b = a + sign * if step % 2 == 0 { mag } else { -mag };
                        if b < 30.0 || b > total - 30.0 {
                            continue;
                        }
                        let (q, v) = interpolate(line, b);
                        let mut m = pt(v.y, -v.x);
                        if m.dot(n) > 0.0 {
                            m = m * -1.0;
                        }
                        let c = q + m * 15.0;
                        let r = 10.0;
                        if site_ok(c, r) && !h.hit(c.x, c.y, r) && !ctx.touches_park(c, r) {
                            h.add(c.x, c.y, r);
                            out.push(Station {
                                kind: StationKind::Elevator,
                                pos: c,
                                facing: atan2(m.x, -m.y),
                                radius: r,
                                length: 30.0,
                            });
                            break;
                        }
                    }
                }
                break 'cand;
            }
        }
    }
    out
}

/// Whether a lot at this spot is built shoulder to shoulder with its neighbours.
fn attached_at(zone: Zone, dc: f64, kind: LotKind) -> bool {
    matches!(kind, LotKind::Shop | LotKind::Row | LotKind::Apartments)
        && match zone {
            Zone::Core => dc < 600.0,
            Zone::Inner => kind == LotKind::Row,
            _ => false,
        }
}

/// Gap between neighbours and distance from the kerb to the building line, in metres.
fn frontage(zone: Zone, dc: f64, kind: LotKind, attached: bool, rng: &mut Rng) -> (f64, f64) {
    if attached {
        return (0.0, 1.0);
    }
    match zone {
        Zone::Core => (rng.uniform(0.0, 1.5), 1.5),
        Zone::Inner => {
            if dc < 900.0 {
                (rng.uniform(0.5, 2.5), 2.5)
            } else {
                (rng.uniform(1.0, 4.0), 3.5)
            }
        }
        Zone::Outer => (rng.uniform(2.5, 6.5), 5.0),
        Zone::Industry | Zone::Mall | Zone::Campus => (rng.uniform(1.5, 4.0), 3.0),
        _ => {
            let _ = kind;
            (rng.uniform(2.0, 5.0), 3.0)
        }
    }
}

/// A lot behind `front`, in the block interior, or `None` if the ground is taken or too far from
/// the street for this zone.
fn back_lot(
    ctx: &mut Ctx,
    rng: &mut Rng,
    front: &Lot,
    depth: usize,
    site_ok: &dyn Fn(Pt, f64) -> bool,
    h: &DiskHash,
) -> Option<Lot> {
    use LotKind::*;
    let zone = front.zone;
    let max_depth = match zone {
        Zone::Core => 3,
        Zone::Inner => 2,
        Zone::Outer => 1,
        _ => 0,
    };
    if depth >= max_depth {
        return None;
    }
    if !matches!(front.kind, House | Row | Shop | Apartments) {
        return None;
    }
    if zone == Zone::Outer && (depth > 0 || rng.f64() > 0.3) {
        return None;
    }
    let kind = match (zone, front.kind) {
        (Zone::Core, Apartments) => Apartments,
        (Zone::Core, _) => kind_from(rng.weighted(&[3.0, 2.0, 1.0]), &[Row, House, Apartments]),
        (Zone::Inner, Apartments) => Apartments,
        (Zone::Inner, _) => kind_from(rng.weighted(&[5.0, 2.0]), &[House, Row]),
        _ => House,
    };
    let r = kind.nominal_radius();
    let courtyard = if front.kind == Apartments && kind == Apartments {
        7.0
    } else {
        0.0
    };
    let gap = match zone {
        Zone::Core => 1.0,
        Zone::Inner => 2.0,
        _ => 7.0,
    } + courtyard;
    // behind the front lot: the front faces `facing`, so back is the direction it looks away from
    let (fx, fy) = (-sin(front.facing), cos(front.facing));
    let d = front.radius + r + gap;
    let ctr = pt(front.centre.x + fx * d, front.centre.y + fy * d);
    if !site_ok(ctr, r) || h.hit(ctr.x, ctr.y, r) || ctx.touches_park(ctr, r) {
        return None;
    }
    // stay inside the same sort of ground: never back onto a park or open land
    match ctx.zone_at(ctr).0 {
        Lz::Z(z) if z == zone => {}
        _ => return None,
    }
    Some(Lot {
        centre: ctr,
        radius: r,
        kind,
        facing: front.facing,
        zone,
        variant: rng.f64(),
        density: 0.0,
    })
}
