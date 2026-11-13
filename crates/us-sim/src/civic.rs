//! Placing civic buildings.

use crate::*;

/// How far from a revealed road a civic building may stand, metres.
const ROAD_REACH: f64 = 45.0;

impl Sim {
    /// Place a civic building with its centre at `p`. It takes empty land beside a road the
    /// town already has: unbuilt lots under it are given up. It pays its price now, goes up over
    /// 40 to 70 days, and then covers lots within its reach by road.
    ///
    /// Returns the index of the new lot in `lots()` (lots are only ever appended).
    pub fn place_civic(&mut self, kind: CivicKind, p: Pt) -> Result<usize, PlaceError> {
        let (at, give_up) = self.civic_plan(kind, p)?;
        let r = kind.radius();
        let cost = kind.cost();
        self.treasury -= cost;
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
        });
        self.changed_lots.push(idx);
        self.log(
            EventKind::Civic,
            format!("Work started on a {}.", kind.label().to_lowercase()),
            Some(p),
        );
        Ok(idx)
    }
    /// Whether a civic building could be placed at `p` right now, without placing it. The same
    /// rules as `place_civic`, for a ghost preview.
    pub fn check_civic(&self, kind: CivicKind, p: Pt) -> Result<(), PlaceError> {
        self.civic_plan(kind, p).map(|_| ())
    }

    /// Validate a placement. Returns the nearest point on the road and the lots it takes over.
    fn civic_plan(&self, kind: CivicKind, p: Pt) -> Result<(Pt, Vec<usize>), PlaceError> {
        let r = kind.radius();
        let size = self.town.size;
        if p.x < 20.0 || p.y < 20.0 || p.x > size - 20.0 || p.y > size - 20.0 {
            return Err(PlaceError::OutOfBounds);
        }
        if p.dist(self.hall) < self.town.hall_radius + r + 8.0 {
            return Err(PlaceError::Occupied);
        }
        for ring in &self.town.water {
            if point_in_ring(p, ring) || dist_pt_ring_edge(p, ring) < r {
                return Err(PlaceError::InWater);
            }
        }
        let mut near_road: Option<(f64, Pt)> = None;
        for run in &self.runs {
            let visible = run.addition.is_none_or(|a| self.revealed[a]);
            let hw = run.class.half_width();
            for w in run.points.windows(2) {
                let d = dist_pt_seg(p, w[0], w[1]);
                if d - hw < r * 0.7 + 1.0 {
                    return Err(PlaceError::OnRoad);
                }
                if visible
                    && !matches!(run.class, RoadClass::Rail | RoadClass::Highway)
                    && near_road.is_none_or(|b| d < b.0)
                {
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
        if self.treasury < cost {
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
