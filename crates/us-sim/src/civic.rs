//! Placing civic buildings.

use crate::*;

/// How far from a revealed road a civic building may stand, metres.
const ROAD_REACH: f64 = 45.0;

/// Amount of service each built tier provides relative to tier one.
pub fn civic_capacity_pct(tier: u8) -> i64 {
    match tier {
        3.. => 290,
        2 => 180,
        _ => 100,
    }
}

/// Additional road-distance coverage from expanding on the same lot.
pub fn civic_reach_multiplier(tier: u8) -> f64 {
    match tier {
        3.. => 1.75,
        2 => 1.35,
        _ => 1.0,
    }
}

impl Sim {
    /// Cost and next tier of an operating civic building.
    pub fn civic_upgrade(&self, i: usize) -> Option<(u8, i64)> {
        let lot = self.lots.get(i)?;
        let kind = lot.civic?;
        if lot.level >= 3 || !matches!(lot.phase, Phase::Built { .. }) || lot.fire.is_some() {
            return None;
        }
        let cost = kind.cost() * if lot.level == 1 { 65 } else { 95 } / 100;
        Some((lot.level + 1, cost))
    }

    /// Expand an existing service in place while keeping it in operation.
    pub fn upgrade_civic(&mut self, i: usize) -> Result<u8, PlaceError> {
        let (tier, cost) = self.civic_upgrade(i).ok_or(PlaceError::Occupied)?;
        if !self.can_afford(cost) {
            return Err(PlaceError::TooPoor { cost });
        }
        self.charge(cost);
        let lot = &mut self.lots[i];
        lot.level = tier;
        let at = lot.pos;
        self.changed_lots.push(i);
        self.recompute_values();
        self.log(
            EventKind::Civic,
            format!("Civic building expanded to tier {tier}."),
            Some(at),
        );
        Ok(tier)
    }

    /// Place a civic building with its centre at `p`. It takes empty land beside a road the
    /// town already has: unbuilt lots under it are given up. It pays its price now, goes up over
    /// 40 to 70 days, and then covers lots within its reach by road.
    ///
    /// Returns the index of the new lot in `lots()` (lots are only ever appended).
    pub fn place_civic(&mut self, kind: CivicKind, p: Pt) -> Result<usize, PlaceError> {
        let (at, give_up) = self.civic_plan(kind, p)?;
        let r = kind.radius();
        let cost = kind.cost();
        self.charge(cost);
        for i in give_up {
            // never revealed or built: the land is now taken
            self.lots[i].phase = Phase::Hidden;
            self.lots[i].addition = usize::MAX;
            self.vacant.retain(|&v| v != i);
            self.changed_lots.push(i);
        }
        let days = 40 + (roll(self.seed, self.lots.len() as u64, 31) * 30.0) as i32;
        let src = self
            .town
            .lots
            .iter()
            .enumerate()
            .min_by(|a, b| p.dist(a.1.centre).total_cmp(&p.dist(b.1.centre)))
            .map_or(0, |(i, _)| i);
        let idx = self.lots.len();
        self.lots.push(LotState {
            phase: Phase::Building {
                start: self.day,
                end: self.day + days,
            },
            kind: kind.lot_kind(),
            pos: p,
            addition: self.addition_of(p).unwrap_or(0),
            radius: r,
            facing: libm::atan2(at.y - p.y, at.x - p.x),
            src,
            exempt: false,
            civic: Some(kind),
            level: 1,
            next_level: 0,
            fire: None,
        });
        self.changed_lots.push(idx);
        self.log(
            EventKind::Civic,
            format!("Work started on a {}.", kind.label().to_lowercase()),
            Some(p),
        );
        Ok(idx)
    }
    /// Snap a placement to the near side of the closest open street within reach.
    /// The caller previews and validates this exact point before spending anything.
    pub fn snap_civic_site(&self, kind: CivicKind, p: Pt) -> Pt {
        let mut nearest = ROAD_REACH * ROAD_REACH;
        let mut site = p;
        for run in self.runs.iter().filter(|run| self.civic_frontage(run)) {
            for edge in run.points.windows(2) {
                let (a, b) = (edge[0], edge[1]);
                // Adjacent segments that cannot beat the current pick need no projection.
                let reach = ROAD_REACH;
                if p.x < a.x.min(b.x) - reach
                    || p.x > a.x.max(b.x) + reach
                    || p.y < a.y.min(b.y) - reach
                    || p.y > a.y.max(b.y) + reach
                {
                    continue;
                }
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let len2 = dx * dx + dy * dy;
                if len2 < 1e-12 {
                    continue;
                }
                let at = nearest_on_seg(p, a, b);
                let (ox, oy) = (p.x - at.x, p.y - at.y);
                let dist2 = ox * ox + oy * oy;
                if dist2 >= nearest {
                    continue;
                }
                nearest = dist2;
                let side = if dx * oy - dy * ox < 0.0 { -1.0 } else { 1.0 };
                let setback = (run.class.half_width() + kind.radius() + 2.0).min(ROAD_REACH - 0.1);
                let offset = side * setback / libm::sqrt(len2);
                site = pt(at.x - dy * offset, at.y + dx * offset);
            }
        }
        site
    }

    fn civic_frontage(&self, run: &RoadRun) -> bool {
        !matches!(run.class, RoadClass::Rail | RoadClass::Highway) && self.street_open(run.street)
    }

    /// Whether a civic building could be placed at `p` right now, without placing it. The same
    /// rules as `place_civic`, for a ghost preview.
    pub fn check_civic(&self, kind: CivicKind, p: Pt) -> Result<(), PlaceError> {
        self.civic_plan(kind, p).map(|_| ())
    }

    /// Validate a placement. Returns the nearest point on the road and the lots it takes over.
    fn civic_plan(&self, kind: CivicKind, p: Pt) -> Result<(Pt, Vec<usize>), PlaceError> {
        if !self.is_unlocked(kind) {
            return Err(PlaceError::Locked {
                people: kind.unlock_at(),
            });
        }
        let r = kind.radius();
        let size = self.town.size;
        if p.x < 20.0 || p.y < 20.0 || p.x > size - 20.0 || p.y > size - 20.0 {
            return Err(PlaceError::OutOfBounds);
        }
        if p.dist(self.hall) < self.town.hall_radius + r + 8.0 {
            return Err(PlaceError::Occupied);
        }
        if self
            .town
            .stations
            .iter()
            .any(|st| p.dist(st.pos) < st.radius + r + 4.0)
        {
            return Err(PlaceError::Occupied);
        }
        for ring in &self.town.water {
            if point_in_ring(p, ring) || dist_pt_ring_edge(p, ring) < r {
                return Err(PlaceError::InWater);
            }
        }
        let mut near_road: Option<(f64, Pt)> = None;
        for run in &self.runs {
            let frontage = self.civic_frontage(run);
            let hw = run.class.half_width();
            for w in run.points.windows(2) {
                let d = dist_pt_seg(p, w[0], w[1]);
                if d - hw < r * 0.7 + 1.0 {
                    return Err(PlaceError::OnRoad);
                }
                if frontage && near_road.is_none_or(|b| d < b.0) {
                    let t = nearest_on_seg(p, w[0], w[1]);
                    near_road = Some((d, t));
                }
            }
        }
        let Some((d, at)) = near_road else {
            return Err(PlaceError::NoRoad);
        };
        if d > ROAD_REACH {
            return Err(PlaceError::NoRoad);
        }
        let mut give_up: Vec<usize> = Vec::new();
        for (i, l) in self.lots.iter().enumerate() {
            if p.dist(l.pos) < l.radius + r - 2.0 {
                match l.phase {
                    Phase::Hidden | Phase::Vacant => give_up.push(i),
                    _ => return Err(PlaceError::Occupied),
                }
            }
        }
        let cost = kind.cost();
        if !self.can_afford(cost) {
            return Err(PlaceError::TooPoor { cost });
        }
        Ok((at, give_up))
    }
}

fn nearest_on_seg(p: Pt, a: Pt, b: Pt) -> Pt {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let l2 = abx * abx + aby * aby;
    let t = if l2 < 1e-12 {
        0.0
    } else {
        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
    };
    pt(a.x + abx * t, a.y + aby * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::{Preset, generate};

    #[test]
    fn service_upgrades_expand_capacity_and_survive_reload() {
        let town = generate(Preset::Grid, 23);
        let mut sim = Sim::new_with_rules(
            &town,
            hall_sites(&town)[0],
            23,
            2,
            Rules::sandbox_unlimited(),
        );
        let kind = CivicKind::Water;
        let site = sim
            .runs
            .iter()
            .filter(|run| sim.civic_frontage(run))
            .flat_map(|run| run.points.windows(2).map(|seg| seg[0].lerp(seg[1], 0.5)))
            .map(|p| sim.snap_civic_site(kind, p))
            .find(|&p| sim.check_civic(kind, p).is_ok())
            .expect("valid roadside site");
        let i = sim.place_civic(kind, site).unwrap();
        assert!(sim.civic_upgrade(i).is_none());
        sim.lots[i].phase = Phase::Built {
            since: 0,
            tax_x10: TAX_DEFAULT_X10,
        };
        let tier_one = sim.capacity(Capacity::Water).supply;
        let treasury = sim.treasury;
        assert_eq!(sim.upgrade_civic(i), Ok(2));
        assert_eq!(sim.capacity(Capacity::Water).supply - tier_one, 1600);
        assert_eq!(sim.upgrade_civic(i), Ok(3));
        assert_eq!(sim.capacity(Capacity::Water).supply - tier_one, 3800);
        assert_eq!(sim.treasury, treasury);
        assert!(sim.civic_upgrade(i).is_none());
        let loaded = Sim::load(&sim.save()).unwrap();
        assert_eq!(loaded.lots()[i].level, 3);
        assert_eq!(
            loaded.capacity(Capacity::Water),
            sim.capacity(Capacity::Water)
        );
    }

    #[test]
    fn roadside_snap_previews_the_same_point_that_is_built() {
        let town = generate(Preset::Grid, 3);
        let mut sim = Sim::new(&town, hall_sites(&town)[0], 3);
        let kind = CivicKind::Water;
        let (raw, site) = sim
            .road_runs()
            .iter()
            .filter(|run| sim.civic_frontage(run))
            .flat_map(|run| run.points.windows(2))
            .filter(|edge| edge[0].dist(edge[1]) > 10.0)
            .map(|edge| edge[0].lerp(edge[1], 0.5))
            .map(|raw| (raw, sim.snap_civic_site(kind, raw)))
            .find(|&(raw, site)| {
                sim.check_civic(kind, raw) == Err(PlaceError::OnRoad)
                    && sim.check_civic(kind, site).is_ok()
            })
            .expect("a legal shoulder beside an open street");
        assert!(raw.dist(site) > kind.radius());
        let before = sim.treasury;
        assert_eq!(sim.snap_civic_site(kind, raw), site);
        let placed = sim
            .place_civic(kind, site)
            .expect("previewed site permits placement");
        assert_eq!(sim.lots()[placed].pos, site);
        assert_eq!(sim.treasury, before - kind.cost());
    }

    #[test]
    fn roadside_snap_keeps_the_tapped_side_and_ignores_inaccessible_roads() {
        let town = generate(Preset::Grid, 3);
        let mut sim = Sim::new(&town, hall_sites(&town)[0], 3);
        let street = sim
            .road_runs()
            .iter()
            .find(|run| sim.civic_frontage(run))
            .unwrap()
            .street;
        let road = RoadRun {
            class: RoadClass::Local,
            points: vec![pt(100.0, 200.0), pt(300.0, 200.0)],
            addition: None,
            street,
        };
        sim.runs = vec![road];
        let kind = CivicKind::School;
        let site_north = sim.snap_civic_site(kind, pt(200.0, 205.0));
        let site_south = sim.snap_civic_site(kind, pt(200.0, 195.0));
        assert_eq!(site_north.x, 200.0);
        assert_eq!(site_south.x, 200.0);
        assert!(site_north.y > 205.0 && site_south.y < 195.0);
        assert_eq!(
            sim.snap_civic_site(kind, pt(200.0, 300.0)),
            pt(200.0, 300.0)
        );

        for class in [RoadClass::Highway, RoadClass::Rail] {
            sim.runs[0].class = class;
            assert_eq!(
                sim.snap_civic_site(kind, pt(200.0, 205.0)),
                pt(200.0, 205.0)
            );
        }
    }
}
