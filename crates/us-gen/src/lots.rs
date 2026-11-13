//! Stage three: lots along the streets.

use crate::geom::{
    Pt, atan2, bbox, cos, dist_pt_ring, interpolate, length, point_in_ring, pt, sin,
};
use crate::plan::Draft;
use crate::rng::Rng;
use crate::terrain::Terrain;
use crate::world::{HWY_Y, S, dist_hall};
use crate::{Addition, CulDeSac, Lot, LotKind, RoadClass, Zone};

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
}

impl Ctx<'_> {
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
                    let k = kind_from(
                        rng.weighted(&[5.0, 3.0, w_office, 1.2, 1.0]),
                        &[Shop, Apartments, Office, Parking, Row],
                    );
                    if k == Office {
                        self.offices_left -= 1;
                    }
                    k
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
) -> Vec<Lot> {
    let mut h = DiskHash::new();
    for d in drafts {
        road_disks(&mut h, &d.line, d.class.half_width());
    }
    for c in culs {
        h.add(c.pos.x, c.pos.y, 9.0);
    }
    h.add(hall.x, hall.y, hall_r + 10.0);
    let mut ctx = Ctx {
        additions,
        boxes: additions.iter().map(|a| bbox(&a.polygon)).collect(),
        hall,
        offices_left: 5,
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
            if site_ok(ctr, r) && !h.hit(ctr.x, ctr.y, r) {
                lots.push(Lot {
                    centre: ctr,
                    radius: r,
                    kind: LotKind::House,
                    facing: atan2(cos(ang), -sin(ang)) + std::f64::consts::PI,
                    zone: Zone::Outer,
                    variant: rng.f64(),
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
        let mut k = 7.0;
        while k < total - 7.0 {
            let (p, u) = interpolate(&d.line, k);
            let mut adv: f64 = 14.0;
            for &side in sides {
                let n = pt(-u.y * side, u.x * side);
                let probe = pt(p.x + n.x * (hw + 8.0), p.y + n.y * (hw + 8.0));
                let (lz, dc) = ctx.zone_at(probe);
                let (zone, kind) = match lz {
                    Lz::Z(Zone::Park) => continue,
                    Lz::Open => {
                        if class != RoadClass::Arterial || !rng.chance(0.02) {
                            continue;
                        }
                        (Zone::Farm, LotKind::Farm)
                    }
                    Lz::Fringe => {
                        if class != RoadClass::Arterial || rng.f64() > 0.22 {
                            continue;
                        }
                        let k = rng.weighted(&[2.0, 1.5, 1.5, 2.0, 1.0]);
                        (
                            Zone::Farm,
                            [
                                LotKind::Strip,
                                LotKind::Gas,
                                LotKind::FastFood,
                                LotKind::House,
                                LotKind::Warehouse,
                            ][k],
                        )
                    }
                    Lz::Z(z) => {
                        let at = pt(p.x + n.x * (hw + 12.0), p.y + n.y * (hw + 12.0));
                        (z, ctx.lot_kind(rng, z, class, dc, at))
                    }
                };
                let r = kind.nominal_radius();
                let dense = zone == Zone::Core
                    && dc < 450.0
                    && !matches!(kind, LotKind::Parking | LotKind::Church);
                let off = hw + if dense { 1.0 } else { 3.0 } + r * 0.85;
                let ctr = pt(p.x + n.x * off, p.y + n.y * off);
                let rc = if dense { r * 0.7 } else { r };
                if !site_ok(ctr, rc) || h.hit(ctr.x, ctr.y, rc) {
                    continue;
                }
                lots.push(Lot {
                    centre: ctr,
                    radius: rc,
                    kind,
                    facing: atan2(-n.x, n.y),
                    zone,
                    variant: rng.f64(),
                });
                h.add(ctr.x, ctr.y, rc);
                let step = if zone == Zone::Farm {
                    300.0
                } else if dense {
                    r * 1.4
                } else {
                    r * 1.6
                };
                adv = adv.max(step);
            }
            k += adv * rng.uniform(0.95, 1.15);
        }
    }
    lots
}
