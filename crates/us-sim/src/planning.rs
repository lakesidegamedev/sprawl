//! City Planning creates parcels with street frontage inside a player-painted area.
//! Existing Town Growth saves keep their original generated parcels and growth rules.

use crate::save::{LoadError, R, Res, W};
use crate::streets::NO_STREET;
use crate::*;

const MAX_AREA_SIDE: f64 = 850.0;
const MAX_PARCELS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanningUse {
    Residential,
    Apartments,
    Commercial,
    Office,
    Industrial,
}

impl PlanningUse {
    pub const ALL: [Self; 5] = [
        Self::Residential,
        Self::Apartments,
        Self::Commercial,
        Self::Office,
        Self::Industrial,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Residential => "Residential",
            Self::Apartments => "Apartments",
            Self::Commercial => "Commercial",
            Self::Office => "Office",
            Self::Industrial => "Industrial",
        }
    }

    fn parcel(self) -> (LotKind, f64) {
        match self {
            Self::Residential => (LotKind::House, 7.4),
            Self::Apartments => (LotKind::Apartments, 13.0),
            Self::Commercial => (LotKind::Shop, 8.5),
            Self::Office => (LotKind::Office, 14.0),
            Self::Industrial => (LotKind::Workshop, 13.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneError {
    NeedsCityPlanning,
    BadBounds,
    NoBuildableFrontage,
}

impl ZoneError {
    pub fn message(self) -> &'static str {
        match self {
            Self::NeedsCityPlanning => "Zoning is available in City Planning.",
            Self::BadBounds => "Choose a smaller zoning area within the map.",
            Self::NoBuildableFrontage => "Choose open land beside a finished street.",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parcel {
    pub centre: Pt,
    pub radius: f64,
    pub facing: f64,
    street: usize,
}

impl Sim {
    /// A preview and commit use the same candidate order and constraints, including the
    /// footprints of all other parcels in the paint operation.
    pub fn zone_preview(&self, kind: PlanningUse, a: Pt, b: Pt) -> Result<Vec<Parcel>, ZoneError> {
        if !self.rules.city_planning {
            return Err(ZoneError::NeedsCityPlanning);
        }
        let (min, max) = (
            pt(a.x.min(b.x), a.y.min(b.y)),
            pt(a.x.max(b.x), a.y.max(b.y)),
        );
        let size = self.town.size;
        if ![a.x, a.y, b.x, b.y].into_iter().all(f64::is_finite)
            || min.x < 0.0
            || min.y < 0.0
            || max.x > size
            || max.y > size
            || max.x - min.x > MAX_AREA_SIDE
            || max.y - min.y > MAX_AREA_SIDE
        {
            return Err(ZoneError::BadBounds);
        }
        let (_, radius) = kind.parcel();
        let fits = |p: Pt| p.x >= min.x && p.x <= max.x && p.y >= min.y && p.y <= max.y;
        let visible: Vec<_> = self
            .runs
            .iter()
            .enumerate()
            .filter(|(i, _)| self.road_visible(*i))
            .collect();
        let mut plan: Vec<Parcel> = Vec::new();
        'roads: for &(i, road) in &visible {
            if !self.road_open(i)
                || matches!(
                    road.class,
                    RoadClass::Rail | RoadClass::Highway | RoadClass::Alley
                )
            {
                continue;
            }
            let setback = road.class.half_width() + radius + 2.4;
            let mut to_next = radius + 3.0;
            for line in road.points.windows(2) {
                let (p, q) = (line[0], line[1]);
                let len = p.dist(q);
                if len < 1e-6 {
                    continue;
                }
                let direction = (q - p) * len.recip();
                let lateral = pt(-direction.y, direction.x);
                while to_next <= len {
                    let at = p + direction * to_next;
                    if at.x >= min.x - setback
                        && at.x <= max.x + setback
                        && at.y >= min.y - setback
                        && at.y <= max.y + setback
                    {
                        for side in [-1.0, 1.0] {
                            let centre = at + lateral * (side * setback);
                            if fits(centre)
                                && self.zone_site_clear(centre, radius, &visible)
                                && plan
                                    .iter()
                                    .all(|l| centre.dist(l.centre) >= radius + l.radius + 0.5)
                            {
                                plan.push(Parcel {
                                    centre,
                                    radius,
                                    facing: libm::atan2(at.y - centre.y, at.x - centre.x),
                                    street: road.street as usize,
                                });
                                if plan.len() == MAX_PARCELS {
                                    break 'roads;
                                }
                            }
                        }
                    }
                    to_next += 2.0 * radius + 1.4;
                }
                to_next -= len;
            }
        }
        if plan.is_empty() {
            Err(ZoneError::NoBuildableFrontage)
        } else {
            Ok(plan)
        }
    }

    fn zone_site_clear(&self, p: Pt, radius: f64, visible: &[(usize, &RoadRun)]) -> bool {
        let margin = radius + 3.0;
        let size = self.town.size - margin;
        let blocked = |a: Pt, r: f64| p.dist(a) < r + margin;
        if p.x < margin
            || p.y < margin
            || p.x > size
            || p.y > size
            || blocked(self.hall, self.town.hall_radius)
            || self.town.stations.iter().any(|s| blocked(s.pos, s.radius))
            || self
                .lots
                .iter()
                .any(|l| l.addition != usize::MAX && blocked(l.pos, l.radius - 2.4))
            || self.town.terrain.slope(p) > 0.12
        {
            return false;
        }
        if self
            .town
            .water
            .iter()
            .any(|ring| point_in_ring(p, ring) || dist_pt_ring_edge(p, ring) < margin)
        {
            return false;
        }
        !visible.iter().any(|(_, r)| {
            let gap = r.class.half_width() + radius + 0.8;
            r.points
                .windows(2)
                .any(|edge| dist_pt_seg(p, edge[0], edge[1]) < gap)
        })
    }

    pub fn paint_zone(&mut self, kind: PlanningUse, a: Pt, b: Pt) -> Result<usize, ZoneError> {
        let plan = self.zone_preview(kind, a, b)?;
        let count = plan.len();
        let (kind, _) = kind.parcel();
        for site in plan {
            let i = self.lots.len();
            self.lots.push(LotState {
                phase: Phase::Vacant,
                kind,
                pos: site.centre,
                addition: self.addition_of(site.centre).unwrap_or(0),
                radius: site.radius,
                facing: site.facing,
                src: self.town.lots.len() + i,
                exempt: false,
                civic: None,
                level: 1,
                next_level: 0,
                fire: None,
            });
            self.vacant.push(i);
            self.changed_lots.push(i);
            self.lot_street.resize(i, NO_STREET);
            self.lot_street.push(site.street as u32);
            if let Some(street) = self.streets.get_mut(site.street) {
                street.lots.push(i as u32);
            }
        }
        self.near = crate::density::neighbours(&self.lots);
        self.values_stale = true;
        self.net_dirty = true;
        Ok(count)
    }

    pub(crate) fn save_planning(&self, writer: &mut W) {
        writer.usize(self.lots.len());
        for i in 0..self.lots.len() {
            writer.u32(self.lot_street.get(i).copied().unwrap_or(NO_STREET));
        }
    }

    pub(crate) fn load_planning(&mut self, reader: &mut R) -> Res<()> {
        let count = reader.len(4)?;
        if count != self.lots.len() {
            return Err(LoadError::Invalid("planning frontage count"));
        }
        self.lot_street = (0..count).map(|_| reader.u32()).collect::<Res<_>>()?;
        if self
            .lot_street
            .iter()
            .any(|&i| i != NO_STREET && i as usize >= self.streets.len())
        {
            return Err(LoadError::Invalid("planning frontage"));
        }
        for street in &mut self.streets {
            street.lots.clear();
        }
        for (i, &street) in self.lot_street.iter().enumerate() {
            if let Some(s) = self.streets.get_mut(street as usize) {
                s.lots.push(i as u32);
            }
        }
        self.rules.city_planning = true;
        self.near = crate::density::neighbours(&self.lots);
        Ok(())
    }

    pub fn is_planned_lot(&self, i: usize) -> bool {
        self.rules.city_planning
            && self
                .lots
                .get(i)
                .is_some_and(|l| l.civic.is_none() && l.src >= self.town.lots.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::{Preset, generate};

    #[test]
    fn city_planning_new_local_street_supplies_frontage_after_opening() {
        let town = generate(Preset::Grid, 2);
        let hall = hall_sites(&town)[0];
        let rules = Rules::sandbox_unlimited().with_city_planning(true);
        let mut city = Sim::new_with_rules(&town, hall, town.seed, 0, rules);
        let path = city
            .road_runs()
            .iter()
            .enumerate()
            .filter(|(i, r)| city.road_open(*i) && r.class == RoadClass::Arterial)
            .flat_map(|(_, r)| r.points.windows(2))
            .flat_map(|s| {
                [
                    s[0].lerp(s[1], 0.28),
                    s[0].lerp(s[1], 0.50),
                    s[0].lerp(s[1], 0.72),
                ]
            })
            .flat_map(|a| {
                (0..12).map(move |k| {
                    let az = k as f64 * std::f64::consts::TAU / 12.0;
                    [
                        a,
                        pt(a.x + 140.0 * libm::cos(az), a.y + 140.0 * libm::sin(az)),
                    ]
                })
            })
            .find(|p| city.check_road(RoadClass::Local, p).is_ok())
            .expect("an admissible local street");
        let index = city.build_road(RoadClass::Local, &path).unwrap();
        assert_eq!(city.road_runs()[index].class, RoadClass::Local);
        let street = city.road_runs()[index].street as usize;
        assert!(!city.road_open(index));
        for _ in 0..90 {
            city.step_day();
        }
        assert!(city.road_open(index));
        let anchor = path[0].lerp(path[1], 0.65);
        let a = pt(anchor.x - 75.0, anchor.y - 75.0);
        let b = pt(anchor.x + 75.0, anchor.y + 75.0);
        let planned = city.zone_preview(PlanningUse::Commercial, a, b).unwrap();
        assert!(planned.iter().any(|p| p.street == street));
        city.paint_zone(PlanningUse::Commercial, a, b).unwrap();
        let saved = Sim::load(&city.save()).unwrap();
        assert!(saved.road_open(index));
        assert!(saved.lot_street.contains(&(street as u32)));
    }

    #[test]
    fn city_planning_grows_only_user_zoned_parcels_and_reopens_them() {
        let town = generate(Preset::Grid, 2);
        let hall = hall_sites(&town)[0];
        let corner = pt(hall.x - 330.0, hall.y - 330.0);
        let end = pt(hall.x + 330.0, hall.y + 330.0);
        let mut standard = Sim::new_with_rules(&town, hall, town.seed, 2, Rules::standard());
        assert!(matches!(
            standard.paint_zone(PlanningUse::Residential, corner, end),
            Err(ZoneError::NeedsCityPlanning)
        ));

        let mut city = Sim::new_with_rules(
            &town,
            hall,
            town.seed,
            2,
            Rules::sandbox_unlimited().with_city_planning(true),
        );
        let roads_before = city.roads_version();
        for _ in 0..40 {
            city.step_day();
        }
        assert_eq!(roads_before, city.roads_version());
        assert!(city.lots.iter().all(|l| l.exempt));
        let preview = city
            .zone_preview(PlanningUse::Residential, corner, end)
            .unwrap();
        let count = city
            .paint_zone(PlanningUse::Residential, corner, end)
            .unwrap();
        assert_eq!(count, preview.len());
        let positions = |city: &Sim| {
            city.lots
                .iter()
                .enumerate()
                .filter(|(i, _)| city.is_planned_lot(*i))
                .map(|(_, l)| l.pos)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            positions(&city),
            preview.iter().map(|p| p.centre).collect::<Vec<_>>()
        );
        let mut loaded = Sim::load(&city.save()).unwrap();
        assert!(loaded.rules().city_planning);
        assert_eq!(positions(&loaded), positions(&city));
        for _ in 0..120 {
            loaded.step_day();
        }
        assert!(loaded.population() > 0);
        assert_eq!(loaded.roads_version(), roads_before);
    }
}
