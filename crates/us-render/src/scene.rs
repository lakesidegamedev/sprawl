//! Town to meshes: ground layers, roads, and the buildings merged into chunk meshes.

use crate::calendar::Calendar;
use crate::ground::{Ground, layer};
use crate::instances::{self, Car, Instance, Path, Walker};
use crate::kit;
use crate::levels;
use crate::live::{self, RoadDraw};
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb};
use crate::overlay::{self, LotInfo, Overlay};
use crate::rng::Rng;
use glam::Vec3;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use us_gen::{LotKind, Pattern, Pt, RoadClass, Town, Zone, pt};

pub const GRASS: Rgb = [116, 172, 92];
pub const PAVING_CORE: Rgb = [206, 202, 194];
pub const PAVING_INDUSTRY: Rgb = [194, 192, 202];
pub const PAVING_MALL: Rgb = [208, 200, 198];
pub const PARK_GREEN: Rgb = [96, 176, 84];
pub const CAMPUS: Rgb = [196, 210, 160];
pub const CEMETERY: Rgb = [146, 178, 118];
pub const FIELDS: [Rgb; 6] = [
    [176, 198, 106],
    [198, 186, 124],
    [140, 172, 96],
    [168, 190, 110],
    [206, 194, 140],
    [128, 164, 88],
];
pub const WATER: Rgb = [66, 132, 200];
pub const SAND: Rgb = [214, 200, 150];
pub const ASPHALT: Rgb = [108, 108, 120];
pub const ASPHALT_HWY: Rgb = [96, 96, 108];
pub const EDGE: Rgb = [44, 46, 56];
pub const YELLOW: Rgb = [226, 196, 70];
pub const BALLAST: Rgb = [128, 104, 84];
pub const STEEL: Rgb = [66, 68, 78];

const CHUNK: f32 = 256.0;

static NEXT_UID: AtomicU64 = AtomicU64::new(1);

/// Outline ids reserved for each chunk of a town scene.
const IDS_PER_CHUNK: u32 = 24_000;

pub struct Chunk {
    /// Bump when `mesh` changes so the renderer re-uploads just this chunk.
    pub rev: u64,
    pub mesh: Mesh,
    /// A simpler form of `mesh` for when the chunk is small on screen (big faces only, no ink).
    /// `None` always draws `mesh`.
    pub far: Option<Mesh>,
    pub lo: Vec3,
    pub hi: Vec3,
}

/// A road as the scene keeps it, so it can change class and show roadworks.
#[derive(Clone, Debug)]
pub struct RoadInfo {
    pub class: RoadClass,
    pub points: Vec<Pt>,
    /// `Some(progress)` while the road is being rebuilt.
    pub works: Option<f32>,
}

/// What a lot's building is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LotState {
    /// Standing and in use.
    Normal,
    /// Going up (a new building or an upgrade): progress 0 to 1, with fence and crane.
    Building(f32),
    /// Burned out; follow with `Building` to show it rebuilt.
    Burned,
}

struct Placement {
    obj: Obj,
    x: f32,
    y: f32,
    theta: f32,
    chunk: usize,
}

struct LotLook {
    original: Obj,
    level: Option<u8>,
    state: LotState,
}

struct TownData {
    town: Town,
    placements: Vec<Placement>,
    /// Placement index of each lot's building.
    lot_place: Vec<usize>,
    looks: Vec<LotLook>,
    chunk_members: Vec<Vec<usize>>,
}

struct OverlayState {
    mesh: Mesh,
    rev: u64,
}

#[derive(Default)]
struct Extras {
    lots: Vec<LotInfo>,
    lot_active: Vec<bool>,
    roads: Vec<RoadInfo>,
    paths: Vec<Path>,
    volumes: Vec<f32>,
    cars: Vec<Car>,
    chimneys: Vec<[f32; 3]>,
    walkers: Vec<Walker>,
    centre: [f32; 2],
    overlay: Option<OverlayState>,
    overlay_rev: u64,
    town: Option<TownData>,
}

/// Everything that moves, worked out for one frame.
#[derive(Default)]
pub struct FrameInstances {
    pub cars: Vec<Instance>,
    pub puffs: Vec<Instance>,
    pub people: Vec<Instance>,
}

pub struct Scene {
    /// Unique per built scene: the renderer caches GPU buffers by it.
    pub uid: u64,
    pub size: f32,
    pub ground: Mesh,
    /// Bump when `ground` changes.
    pub ground_rev: u64,
    pub chunks: Vec<Chunk>,
    pub hall: Vec3,
    /// The game date and hour. `None` draws the plain noon look.
    pub calendar: Option<Calendar>,
    /// Animation clock in seconds: cars, smoke and people move with it.
    pub time: f32,
    x: Extras,
}

impl Scene {
    /// An empty scene a host fills and edits over time (chunks added, chunk meshes replaced with
    /// a new `rev`). The GPU copy follows by `uid`, `ground_rev` and each chunk's `rev`.
    pub fn dynamic(size: f32, ground: Mesh, hall: Vec3) -> Scene {
        Scene {
            uid: NEXT_UID.fetch_add(1, Ordering::Relaxed),
            size,
            ground,
            ground_rev: 0,
            chunks: Vec::new(),
            hall,
            calendar: None,
            time: 0.0,
            x: Extras::default(),
        }
    }

    pub fn triangles(&self) -> usize {
        self.ground.triangles()
            + self
                .chunks
                .iter()
                .map(|c| c.mesh.triangles())
                .sum::<usize>()
    }

    pub fn from_town(t: &Town) -> Scene {
        let roads: Vec<RoadInfo> = t
            .roads
            .iter()
            .map(|r| RoadInfo {
                class: r.class,
                points: r.points.clone(),
                works: None,
            })
            .collect();
        let ground = build_ground(t, &roads);
        let (td, chunks) = build_chunks(t);
        let mut s = Scene {
            uid: NEXT_UID.fetch_add(1, Ordering::Relaxed),
            size: t.size as f32,
            ground,
            ground_rev: 0,
            chunks,
            hall: Vec3::new(t.hall.x as f32, t.hall.y as f32, 0.0),
            calendar: None,
            time: 0.0,
            x: Extras::default(),
        };
        let lots: Vec<LotInfo> = t
            .lots
            .iter()
            .map(|l| LotInfo {
                x: l.centre.x as f32,
                y: l.centre.y as f32,
                theta: l.facing as f32,
                kind: l.kind,
                level: 2,
            })
            .collect();
        s.x.town = Some(td);
        s.set_geometry(lots, roads);
        s
    }

    // ---- what the scene knows about the town ----

    /// Tell the scene about the lots and roads of a hand-built scene, so map views, traffic and
    /// smoke work on it. (`from_town` does this itself.)
    pub fn set_geometry(&mut self, lots: Vec<LotInfo>, roads: Vec<RoadInfo>) {
        self.x.paths = roads
            .iter()
            .map(|r| Path::new(r.class, &r.points))
            .collect();
        self.x.lot_active = vec![true; lots.len()];
        self.x.lots = lots;
        self.x.roads = roads;
        self.x.centre = [self.hall.x, self.hall.y];
        self.x.walkers = instances::make_walkers(&self.x.paths, self.x.centre, 260.0);
        self.x.cars = instances::make_cars(&self.x.paths, &self.x.volumes);
        self.refresh_chimneys();
    }

    /// Change what the overlay and smoke know about one lot of a hand-built scene.
    pub fn set_lot_info(&mut self, i: usize, level: u8, active: bool) {
        if let Some(l) = self.x.lots.get_mut(i) {
            l.level = level.clamp(1, levels::MAX_LEVEL);
            self.x.lot_active[i] = active;
            self.refresh_chimneys();
        }
    }

    pub fn lot_count(&self) -> usize {
        self.x.lots.len()
    }

    pub fn road_count(&self) -> usize {
        self.x.roads.len()
    }

    fn refresh_chimneys(&mut self) {
        let mut v = Vec::new();
        for (i, l) in self.x.lots.iter().enumerate() {
            if !self.x.lot_active.get(i).copied().unwrap_or(true) {
                continue;
            }
            let (s, c) = (libm::sinf(l.theta), libm::cosf(l.theta));
            for [cx, cy, top] in levels::chimneys(l.kind, l.level) {
                v.push([l.x + c * cx - s * cy, l.y + s * cx + c * cy, top]);
            }
        }
        self.x.chimneys = v;
    }

    // ---- map views ----

    /// Show a map view, or `None` to go back to the town. `lots` and `roads` of the overlay are
    /// indexed like the town's lot and road lists.
    pub fn set_overlay(&mut self, ov: Option<Overlay>) {
        match ov {
            None => {
                self.x.overlay = None;
            }
            Some(ov) => {
                let roads: Vec<(RoadClass, &[Pt])> = self
                    .x
                    .roads
                    .iter()
                    .map(|r| (r.class, r.points.as_slice()))
                    .collect();
                let (mass, hall) = match &self.x.town {
                    Some(td) => (
                        (0..self.x.lots.len())
                            .map(|i| levels::massing(&td.placements[td.lot_place[i]].obj))
                            .collect::<Vec<_>>(),
                        td.placements
                            .first()
                            .map(|h| (h.x, h.y, h.theta, levels::massing(&h.obj))),
                    ),
                    None => (Vec::new(), None),
                };
                let mesh = overlay::build_mesh(&self.x.lots, &mass, hall, &roads, &ov);
                self.x.overlay_rev += 1;
                self.x.overlay = Some(OverlayState {
                    mesh,
                    rev: self.x.overlay_rev,
                });
            }
        }
    }

    pub fn overlay_active(&self) -> bool {
        self.x.overlay.is_some()
    }

    pub(crate) fn overlay_mesh(&self) -> Option<(&Mesh, u64)> {
        self.x.overlay.as_ref().map(|o| (&o.mesh, o.rev))
    }

    // ---- traffic, smoke, people ----

    /// Traffic volume of every road, 0 (empty) to 1 (jammed), indexed like the road list. The
    /// number of cars on each road follows it.
    pub fn set_road_volumes(&mut self, volumes: &[f32]) {
        self.x.volumes = volumes.to_vec();
        self.x.cars = instances::make_cars(&self.x.paths, &self.x.volumes);
    }

    pub fn car_count(&self) -> usize {
        self.x.cars.len()
    }

    pub fn set_time(&mut self, seconds: f32) {
        self.time = seconds;
    }

    pub fn set_calendar(&mut self, c: Option<Calendar>) {
        self.calendar = c;
    }

    /// Cars, smoke puffs and people at the scene's current clock. Empty in a map view.
    pub fn instances(&self) -> FrameInstances {
        if self.x.overlay.is_some() {
            return FrameInstances::default();
        }
        FrameInstances {
            cars: instances::car_instances(&self.x.paths, &self.x.cars, self.time),
            puffs: instances::puff_instances(&self.x.chimneys, self.time),
            people: instances::walker_instances(&self.x.paths, &self.x.walkers, self.time),
        }
    }

    // ---- buildings over time ----

    /// Change what lots look like: level (1 to 5, `None` for the original look) and state. Only
    /// the chunks that hold changed lots are rebuilt. Works on scenes made by `from_town`.
    pub fn set_lots(&mut self, changes: &[(usize, Option<u8>, LotState)]) {
        let Some(td) = self.x.town.as_mut() else {
            return;
        };
        let mut dirty: Vec<usize> = Vec::new();
        for &(i, level, state) in changes {
            if i >= td.looks.len() {
                continue;
            }
            let level = level.map(|l| l.clamp(1, levels::MAX_LEVEL));
            let look = &mut td.looks[i];
            look.level = level;
            look.state = state;
            let lot = &td.town.lots[i];
            let base = match level {
                None => look.original.clone(),
                Some(l) => levels::build_lot_level(
                    lot.kind,
                    l,
                    &mut Rng::new(td.town.seed ^ 0x1e7e_1000, i as u64 * 131 + l as u64),
                ),
            };
            let obj = match state {
                LotState::Normal => base,
                LotState::Building(p) => levels::building_site(&base, p),
                LotState::Burned => {
                    levels::burned(&base, &mut Rng::new(td.town.seed ^ 0xb0b0, i as u64))
                }
            };
            let pi = td.lot_place[i];
            td.placements[pi].obj = obj;
            let ch = td.placements[pi].chunk;
            if !dirty.contains(&ch) {
                dirty.push(ch);
            }
            self.x.lots[i].level = level.unwrap_or(2);
            self.x.lot_active[i] = state == LotState::Normal;
        }
        for ch in dirty {
            let (mesh, far, lo, hi) = emit_chunk(&td.placements, &td.chunk_members[ch], ch);
            let c = &mut self.chunks[ch];
            c.mesh = mesh;
            c.far = Some(far);
            c.lo = lo;
            c.hi = hi;
            c.rev += 1;
        }
        self.refresh_chimneys();
    }

    pub fn set_lot(&mut self, i: usize, level: Option<u8>, state: LotState) {
        self.set_lots(&[(i, level, state)]);
    }

    // ---- roads over time ----

    /// Change roads in place: new class and roadworks state (`Some(progress)` while it is being
    /// rebuilt). The ground is re-meshed once for the whole batch.
    pub fn set_roads(&mut self, changes: &[(usize, RoadClass, Option<f32>)]) {
        let mut any = false;
        for &(i, class, works) in changes {
            let Some(r) = self.x.roads.get_mut(i) else {
                continue;
            };
            if r.class != class || r.works != works {
                r.class = class;
                r.works = works;
                self.x.paths[i] = Path::new(class, &r.points);
                any = true;
            }
        }
        if !any {
            return;
        }
        if let Some(td) = &self.x.town {
            self.ground = build_ground(&td.town, &self.x.roads);
            self.ground_rev += 1;
        }
        self.x.cars = instances::make_cars(&self.x.paths, &self.x.volumes);
    }

    pub fn road_class(&self, i: usize) -> Option<RoadClass> {
        self.x.roads.get(i).map(|r| r.class)
    }

    /// The ground of a hand-built scene is the host's; this rebuilds it for `from_town` scenes.
    pub fn road_draws(&self) -> Vec<RoadDraw<'_>> {
        self.x
            .roads
            .iter()
            .map(|r| RoadDraw {
                class: r.class,
                points: &r.points,
                works: r.works,
            })
            .collect()
    }
}

fn hash2(a: u64, b: u64) -> u64 {
    let mut r = Rng::new(a, b);
    r.next_u64()
}

fn zone_colour(zone: Zone, pattern: Pattern, seed: u64, ai: usize) -> Option<Rgb> {
    match zone {
        Zone::Core => Some(PAVING_CORE),
        Zone::Industry => Some(PAVING_INDUSTRY),
        Zone::Mall => Some(PAVING_MALL),
        Zone::Campus => Some(CAMPUS),
        Zone::Park => Some(if pattern == Pattern::Cemetery {
            CEMETERY
        } else {
            PARK_GREEN
        }),
        Zone::Farm => Some(FIELDS[(hash2(seed, ai as u64) % FIELDS.len() as u64) as usize]),
        Zone::Inner | Zone::Outer => None,
    }
}

/// Uniform grid over road segments, for "how far is this spot from any road".
type SegCells = BTreeMap<(i32, i32), Vec<(Pt, Pt, f64)>>;

struct RoadGrid {
    cells: SegCells,
}

impl RoadGrid {
    fn new(t: &Town) -> RoadGrid {
        let mut cells: SegCells = BTreeMap::new();
        for r in &t.roads {
            let hw = r.class.half_width();
            for w in r.points.windows(2) {
                let n = ((w[0].dist(w[1]) / 16.0).ceil() as usize).max(1);
                let mut last = (i32::MIN, i32::MIN);
                for k in 0..=n {
                    let p = w[0].lerp(w[1], k as f64 / n as f64);
                    let c = ((p.x / 48.0).floor() as i32, (p.y / 48.0).floor() as i32);
                    if c != last {
                        cells.entry(c).or_default().push((w[0], w[1], hw));
                        last = c;
                    }
                }
            }
        }
        RoadGrid { cells }
    }

    /// Smallest gap between `p` and the edge of any road, within 2 cells.
    fn clearance(&self, p: Pt) -> f64 {
        let (cx, cy) = ((p.x / 48.0).floor() as i32, (p.y / 48.0).floor() as i32);
        let mut best = f64::INFINITY;
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                for (a, b, hw) in self.cells.get(&(x, y)).map_or(&[][..], |v| v.as_slice()) {
                    let (abx, aby) = (b.x - a.x, b.y - a.y);
                    let l2 = abx * abx + aby * aby;
                    let tt = if l2 < 1e-12 {
                        0.0
                    } else {
                        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
                    };
                    let d = p.dist(pt(a.x + abx * tt, a.y + aby * tt)) - hw;
                    best = best.min(d);
                }
            }
        }
        best
    }
}

fn in_ring(p: Pt, ring: &[Pt]) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn trees_for(kind: LotKind, rng: &mut Rng) -> usize {
    match kind {
        LotKind::House => rng.below(4),
        LotKind::Row | LotKind::Apartments | LotKind::Shop | LotKind::FastFood => rng.below(2),
        LotKind::Church => 1 + rng.below(3),
        LotKind::School => 2 + rng.below(3),
        LotKind::Sports => 3,
        LotKind::Farm => 5 + rng.below(5),
        LotKind::Workshop | LotKind::Warehouse => rng.below(2),
        _ => 0,
    }
}

fn build_ground(t: &Town, roads: &[RoadInfo]) -> Mesh {
    let mut g = Ground::default();
    let s = t.size as f32;
    g.rect(-1500.0, -1500.0, s + 1500.0, s + 1500.0, layer::BASE, GRASS);
    for (ai, a) in t.additions.iter().enumerate() {
        if let Some(c) = zone_colour(a.zone, a.pattern, t.seed, ai) {
            g.polygon(&a.polygon, layer::ZONE, c);
        }
    }
    for w in &t.water {
        g.polygon(&Ground::grow(w, 14.0), layer::SAND, SAND);
        g.polygon(w, layer::WATER, WATER);
    }
    let draws: Vec<RoadDraw> = roads
        .iter()
        .map(|r| RoadDraw {
            class: r.class,
            points: &r.points,
            works: r.works,
        })
        .collect();
    let culs: Vec<Pt> = t.cul_de_sacs.iter().map(|c| c.pos).collect();
    live::add_roads(&mut g, &draws, &culs);
    g.mesh
}

fn chunk_key(x: f32, y: f32) -> (i32, i32) {
    ((x / CHUNK).floor() as i32, (y / CHUNK).floor() as i32)
}

/// One chunk's mesh and bounds from the placements it holds.
fn emit_chunk(
    placements: &[Placement],
    members: &[usize],
    ordinal: usize,
) -> (Mesh, Mesh, Vec3, Vec3) {
    let mut mesh = Mesh::default();
    let mut far = Mesh::default();
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    let mut next_id = ordinal as u32 * IDS_PER_CHUNK + 1;
    for &pi in members {
        let p = &placements[pi];
        p.obj.emit(&mut mesh, &mut next_id, p.x, p.y, p.theta, 1.0);
        p.obj.emit_far(&mut far, p.x, p.y, p.theta);
        let b = p.obj.footprint();
        let reach = b[0].abs().max(b[1].abs()).max(b[2].abs()).max(b[3].abs()) * 1.5 + 1.0;
        lo = lo.min(Vec3::new(p.x - reach, p.y - reach, 0.0));
        hi = hi.max(Vec3::new(p.x + reach, p.y + reach, 90.0));
    }
    (mesh, far, lo, hi)
}

fn build_chunks(t: &Town) -> (TownData, Vec<Chunk>) {
    let mut raw: Vec<(Obj, f32, f32, f32)> = Vec::new();
    // town hall faces the nearest road
    let hall = pt(t.hall.x, t.hall.y);
    let mut best: Option<(f64, Pt)> = None;
    for r in &t.roads {
        for w in r.points.windows(2) {
            let (abx, aby) = (w[1].x - w[0].x, w[1].y - w[0].y);
            let l2 = abx * abx + aby * aby;
            if l2 < 1e-12 {
                continue;
            }
            let tt = (((hall.x - w[0].x) * abx + (hall.y - w[0].y) * aby) / l2).clamp(0.0, 1.0);
            let q = pt(w[0].x + abx * tt, w[0].y + aby * tt);
            let d = hall.dist(q);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, q));
            }
        }
    }
    let theta = best.map_or(0.0, |(_, q)| {
        libm::atan2f((q.x - hall.x) as f32, -(q.y - hall.y) as f32)
    });
    let mut hrng = Rng::new(t.seed, 0x4a11);
    raw.push((
        kit::town_hall(&mut hrng),
        hall.x as f32,
        hall.y as f32,
        theta,
    ));

    let green = [kit::TREE, kit::TREE_D];
    let mut lot_place = Vec::with_capacity(t.lots.len());
    let mut looks = Vec::with_capacity(t.lots.len());
    for (i, lot) in t.lots.iter().enumerate() {
        let mut rng = Rng::new(t.seed ^ 0x5eed_0000, i as u64 * 7919 + 13);
        let obj = kit::build_lot(lot.kind, &mut rng);
        let (x, y) = (lot.centre.x as f32, lot.centre.y as f32);
        let theta = lot.facing as f32;
        lot_place.push(raw.len());
        looks.push(LotLook {
            original: obj.clone(),
            level: None,
            state: LotState::Normal,
        });
        raw.push((obj.clone(), x, y, theta));
        let n = trees_for(lot.kind, &mut rng);
        if n == 0 {
            continue;
        }
        let fp = obj.footprint();
        let (st, ct) = (libm::sinf(theta), libm::cosf(theta));
        let mut placed: Vec<[f32; 2]> = Vec::new();
        for _ in 0..n {
            for _try in 0..14 {
                let ang = rng.range(0.0, std::f32::consts::TAU);
                let r = rng.range(0.35, 1.0) * lot.radius as f32 + 0.3;
                let (lx, ly) = (r * libm::cosf(ang), r * libm::sinf(ang));
                let m = 0.4;
                let inside = lx > fp[0] - m && lx < fp[2] + m && ly > fp[1] - m && ly < fp[3] + m;
                if inside || placed.iter().any(|p| (p[0] - lx).hypot(p[1] - ly) < 3.2) {
                    continue;
                }
                placed.push([lx, ly]);
                let (rad, h) = (rng.range(1.8, 2.8), rng.range(5.0, 8.0));
                let (tx, ty) = (x + ct * lx - st * ly, y + st * lx + ct * ly);
                let col = kit::tree_variant(rng.pick(&green), tx, ty);
                raw.push((kit::tree(rad, h, col), tx, ty, 0.0));
                break;
            }
        }
    }

    // trees in open park land
    let grid = RoadGrid::new(t);
    for (ai, a) in t.additions.iter().enumerate() {
        let per = match (a.zone, a.pattern) {
            (Zone::Park, Pattern::Golf) => 1800.0,
            (Zone::Park, Pattern::Cemetery) => 900.0,
            (Zone::Park, _) => 520.0,
            (Zone::Campus, _) => 1600.0,
            _ => continue,
        };
        let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
        for p in &a.polygon {
            lo = pt(lo.x.min(p.x), lo.y.min(p.y));
            hi = pt(hi.x.max(p.x), hi.y.max(p.y));
        }
        let area = (hi.x - lo.x) * (hi.y - lo.y);
        let mut rng = Rng::new(t.seed ^ 0x7ee5, ai as u64);
        let tries = (area / per) as usize;
        for _ in 0..tries {
            let p = pt(
                lo.x + rng.f32() as f64 * (hi.x - lo.x),
                lo.y + rng.f32() as f64 * (hi.y - lo.y),
            );
            if !in_ring(p, &a.polygon) || grid.clearance(p) < 5.0 {
                continue;
            }
            if t.lots.iter().any(|l| l.centre.dist(p) < l.radius + 3.0) {
                continue;
            }
            let (rad, h) = (rng.range(1.8, 2.8), rng.range(5.0, 8.0));
            let col = kit::tree_variant(rng.pick(&green), p.x as f32, p.y as f32);
            raw.push((kit::tree(rad, h, col), p.x as f32, p.y as f32, 0.0));
        }
    }

    // group into chunks, in chunk order
    let mut cells: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    for (pi, r) in raw.iter().enumerate() {
        cells.entry(chunk_key(r.1, r.2)).or_default().push(pi);
    }
    let chunk_members: Vec<Vec<usize>> = cells.into_values().collect();
    let mut placements: Vec<Placement> = raw
        .into_iter()
        .map(|(obj, x, y, theta)| Placement {
            obj,
            x,
            y,
            theta,
            chunk: 0,
        })
        .collect();
    for (ci, m) in chunk_members.iter().enumerate() {
        for &pi in m {
            placements[pi].chunk = ci;
        }
    }
    let chunks = chunk_members
        .iter()
        .enumerate()
        .map(|(ci, m)| {
            let (mesh, far, lo, hi) = emit_chunk(&placements, m, ci);
            Chunk {
                rev: 0,
                mesh,
                far: Some(far),
                lo,
                hi,
            }
        })
        .collect();
    (
        TownData {
            town: t.clone(),
            placements,
            lot_place,
            looks,
            chunk_members,
        },
        chunks,
    )
}
