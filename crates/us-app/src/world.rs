//! The town on screen: a render scene that follows the simulation. Ground meshes appear with
//! their addition, building meshes are rebuilt per 256 m cell only when a lot in it changes.

use glam::Vec3;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use us_gen::{LotKind, Pt, RoadClass, Town, Zone};
use us_render::levels::{building_site, burned};
use us_render::live::{RoadDraw, abandoned, base_ground, roads_ground_state, zone_ground};
use us_render::obj::Obj;
use us_render::{
    Chunk, CivicModel, LotInfo, Mesh, Rng, RoadInfo, Scene, build_civic, build_lot_level, kit,
};
use us_sim::{CivicKind, LotState, Phase, Sim};

const CELL: f32 = 256.0;
const IDS_PER_CELL: u32 = 60_000;

/// Game days a road stays under construction after it is widened or built.
const WORKS_DAYS: i32 = 40;
/// Roadworks are redrawn this many times over their length.
const WORKS_STEPS: i32 = 5;

type Cell = (i32, i32);
/// Which ground chunk a road belongs to: an addition, or `None` for the skeleton and connectors.
type Group = Option<usize>;

/// What one lot looks like right now. A lot's mesh is rebuilt only when this changes.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct Look {
    /// 0 not drawn, 1 building site, 2 standing, 3 burned, 4 abandoned.
    pub state: u8,
    /// Building level 1 to 5 the model is drawn at (the new level while a site).
    pub level: u8,
    /// Progress step of a building site, 0 to 4.
    pub step: u8,
}

/// Roadworks on one road run.
#[derive(Clone, Copy, Default)]
struct RunWork {
    /// Day the work started.
    start: Option<i32>,
    /// The step last drawn.
    step: u8,
}

struct PrevRun {
    class: RoadClass,
    points: Vec<Pt>,
    work: RunWork,
}

pub struct LiveScene {
    pub scene: Scene,
    town: Arc<Town>,
    show_hall: bool,
    hall_theta: f32,
    hall_obj: Obj,
    cell_chunk: HashMap<Cell, usize>,
    cell_lots: HashMap<Cell, Vec<usize>>,
    hall_cell: Cell,
    dirty: HashSet<Cell>,
    queue: VecDeque<Cell>,
    pub look: Vec<Look>,
    /// Finished model per lot, with the look and radius it was made for.
    objs: Vec<Option<(Look, u16, Obj)>>,
    /// Lots parked off the map in the scene's geometry (not drawn yet).
    parked: Vec<bool>,
    road_chunk: HashMap<Group, usize>,
    road_dirty: Vec<Group>,
    roads_ver: u32,
    run_work: Vec<RunWork>,
    prev_runs: Vec<PrevRun>,
    rev: u64,
    ground_chunks: usize,
    pub triangles: usize,
    /// Lots or roads changed since the scene was last told the geometry.
    geo_dirty: bool,
    geo_at: Instant,
}

fn cell_of(x: f64, y: f64) -> Cell {
    (
        (x as f32 / CELL).floor() as i32,
        (y as f32 / CELL).floor() as i32,
    )
}

fn look_of(l: &LotState, day: i32) -> Look {
    let level = l.level.clamp(1, 5);
    match l.phase {
        Phase::Hidden | Phase::Vacant | Phase::Cleared { .. } => Look::default(),
        _ if l.fire.is_some() => Look {
            state: 3,
            level,
            step: 0,
        },
        Phase::Building { start, end } => {
            let p = (day - start) as f32 / (end - start).max(1) as f32;
            Look {
                state: 1,
                level: if l.next_level > 0 {
                    l.next_level.clamp(1, 5)
                } else {
                    level
                },
                step: (p * 5.0).clamp(0.0, 4.0) as u8,
            }
        }
        Phase::Built { .. } => Look {
            state: 2,
            level,
            step: 0,
        },
        Phase::Empty { .. } => Look {
            state: 4,
            level,
            step: 0,
        },
    }
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

fn civic_model(c: Option<CivicKind>) -> Option<CivicModel> {
    match c {
        Some(CivicKind::Police) => Some(CivicModel::Police),
        Some(CivicKind::Clinic) => Some(CivicModel::Clinic),
        Some(CivicKind::Water) => Some(CivicModel::WaterTower),
        _ => None,
    }
}

/// The model of one lot (building and its trees) at a level, local to the lot, facing south.
fn lot_model(seed: u64, l: &LotState, level: u8) -> Obj {
    let mut rng = Rng::new(seed ^ 0x5eed_0000, l.src as u64 * 7919 + 13);
    let mut obj = match civic_model(l.civic) {
        Some(m) => build_civic(m, &mut rng),
        None if l.civic.is_some() => kit::build_lot(l.kind, &mut rng),
        None => build_lot_level(l.kind, level, &mut rng),
    };
    let (kind, radius) = (l.kind, l.radius);
    let n = trees_for(kind, &mut rng);
    if n > 0 {
        let fp = obj.footprint();
        let green = [kit::TREE, kit::TREE_D];
        let mut placed: Vec<[f32; 2]> = Vec::new();
        for _ in 0..n {
            for _try in 0..14 {
                let ang = rng.range(0.0, std::f32::consts::TAU);
                let r = rng.range(0.35, 1.0) * radius as f32 + 0.3;
                let (lx, ly) = (r * ang.cos(), r * ang.sin());
                let m = 0.4;
                let inside = lx > fp[0] - m && lx < fp[2] + m && ly > fp[1] - m && ly < fp[3] + m;
                if inside || placed.iter().any(|p| (p[0] - lx).hypot(p[1] - ly) < 3.2) {
                    continue;
                }
                placed.push([lx, ly]);
                let (rad, h) = (rng.range(1.8, 2.8), rng.range(5.0, 8.0));
                let tree = kit::tree(rad, h, rng.pick(&green));
                obj.add_placed(&tree, lx, ly, 0.0);
                break;
            }
        }
    }
    obj
}

/// The model to draw for a lot in its current look.
fn look_obj(seed: u64, i: usize, l: &LotState, look: Look) -> Obj {
    let level = if l.civic.is_some() { 1 } else { look.level };
    let full = lot_model(seed, l, level);
    match look.state {
        1 => building_site(&full, (look.step as f32 + 0.5) / 5.0),
        3 => burned(&full, &mut Rng::new(seed ^ 0xb0b0, i as u64)),
        4 => abandoned(&full),
        _ => full,
    }
}

fn dist_seg(p: Pt, a: Pt, b: Pt) -> f64 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let l2 = abx * abx + aby * aby;
    let t = if l2 < 1e-12 {
        0.0
    } else {
        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
    };
    p.dist(us_gen::pt(a.x + abx * t, a.y + aby * t))
}

/// Segments of the roads as they were, to tell which stretch of a changed road is new.
struct OldRoads {
    segs: Vec<(Pt, Pt, RoadClass, RunWork)>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl OldRoads {
    fn new(prev: &[PrevRun]) -> OldRoads {
        let mut o = OldRoads {
            segs: Vec::new(),
            cells: HashMap::new(),
        };
        for r in prev {
            for w in r.points.windows(2) {
                let id = o.segs.len() as u32;
                o.segs.push((w[0], w[1], r.class, r.work));
                let n = (w[0].dist(w[1]) / 16.0).ceil().max(1.0) as usize;
                let mut last = None;
                for k in 0..=n {
                    let t = k as f64 / n as f64;
                    let c = (
                        ((w[0].x + (w[1].x - w[0].x) * t) / 32.0).floor() as i32,
                        ((w[0].y + (w[1].y - w[0].y) * t) / 32.0).floor() as i32,
                    );
                    if last != Some(c) {
                        o.cells.entry(c).or_default().push(id);
                        last = Some(c);
                    }
                }
            }
        }
        o
    }

    /// The old class and work at a point on a road, if a road was there.
    fn at(&self, p: Pt) -> Option<(RoadClass, RunWork)> {
        let (cx, cy) = ((p.x / 32.0).floor() as i32, (p.y / 32.0).floor() as i32);
        let mut best: Option<(f64, RoadClass, RunWork)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &id in self.cells.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    let (a, b, c, w) = self.segs[id as usize];
                    let d = dist_seg(p, a, b);
                    if d < 1.0 && best.is_none_or(|x| d < x.0) {
                        best = Some((d, c, w));
                    }
                }
            }
        }
        best.map(|b| (b.1, b.2))
    }
}

impl LiveScene {
    pub fn new(town: Arc<Town>, sim: &Sim, show_hall: bool) -> LiveScene {
        let farm: Vec<usize> = (0..town.additions.len())
            .filter(|&i| town.additions[i].zone == Zone::Farm)
            .collect();
        let ground = base_ground(&town, &farm);
        let hall = sim.hall;
        let mut scene = Scene::dynamic(
            town.size as f32,
            ground,
            Vec3::new(hall.x as f32, hall.y as f32, 0.0),
        );
        // road ground per group; the skeleton and connectors are chunk 0
        scene.chunks.push(Chunk {
            rev: 1,
            mesh: Mesh::default(),
            far: None,
            lo: Vec3::new(-100.0, -100.0, 0.0),
            hi: Vec3::new(town.size as f32 + 100.0, town.size as f32 + 100.0, 2.0),
        });
        let mut cell_lots: HashMap<Cell, Vec<usize>> = HashMap::new();
        for (i, l) in sim.lots().iter().enumerate() {
            cell_lots
                .entry(cell_of(l.pos.x, l.pos.y))
                .or_default()
                .push(i);
        }
        // the town hall faces the nearest road
        let mut best: Option<(f64, Pt)> = None;
        for r in &town.roads {
            for w in r.points.windows(2) {
                let (abx, aby) = (w[1].x - w[0].x, w[1].y - w[0].y);
                let l2 = abx * abx + aby * aby;
                if l2 < 1e-12 {
                    continue;
                }
                let t = (((hall.x - w[0].x) * abx + (hall.y - w[0].y) * aby) / l2).clamp(0.0, 1.0);
                let q = us_gen::pt(w[0].x + abx * t, w[0].y + aby * t);
                let d = hall.dist(q);
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, q));
                }
            }
        }
        let hall_theta = best.map_or(0.0, |(_, q)| {
            ((q.x - hall.x) as f32).atan2(-(q.y - hall.y) as f32)
        });
        let mut hrng = Rng::new(town.seed, 0x4a11);
        let hall_obj = kit::town_hall(&mut hrng);
        let hall_cell = cell_of(hall.x, hall.y);
        let n = sim.lots().len();
        let mut ls = LiveScene {
            scene,
            town,
            show_hall,
            hall_theta,
            hall_obj,
            cell_chunk: HashMap::new(),
            cell_lots,
            hall_cell,
            dirty: HashSet::new(),
            queue: VecDeque::new(),
            look: vec![Look::default(); n],
            objs: (0..n).map(|_| None).collect(),
            parked: vec![false; n],
            road_chunk: HashMap::from([(None, 0)]),
            road_dirty: Vec::new(),
            roads_ver: sim.roads_version(),
            run_work: vec![RunWork::default(); sim.road_runs().len()],
            prev_runs: sim
                .road_runs()
                .iter()
                .map(|r| PrevRun {
                    class: r.class,
                    points: r.points.clone(),
                    work: RunWork::default(),
                })
                .collect(),
            rev: 1,
            ground_chunks: 0,
            triangles: 0,
            geo_dirty: true,
            geo_at: Instant::now(),
        };
        if show_hall {
            ls.mark(hall_cell);
        }
        ls.rebuild_roads(sim, None);
        ls.ensure_geometry(sim);
        ls
    }

    /// Tell the scene where lots and roads are, so map views, traffic and smoke work. Lots and
    /// roads that do not exist yet are parked far off the map so a view never shows them.
    pub fn ensure_geometry(&mut self, sim: &Sim) {
        if !self.geo_dirty {
            return;
        }
        self.geo_dirty = false;
        self.geo_at = Instant::now();
        const OFF: f64 = -50_000.0;
        let day = sim.day;
        self.parked = sim
            .lots()
            .iter()
            .map(|l| matches!(l.phase, Phase::Hidden | Phase::Cleared { .. }))
            .collect();
        let lots: Vec<LotInfo> = sim
            .lots()
            .iter()
            .zip(&self.parked)
            .map(|(l, &gone)| LotInfo {
                x: if gone { OFF as f32 } else { l.pos.x as f32 },
                y: if gone { OFF as f32 } else { l.pos.y as f32 },
                theta: l.facing as f32,
                kind: l.kind,
                level: l.level.clamp(1, 5),
            })
            .collect();
        let works: Vec<Option<f32>> = (0..sim.road_runs().len())
            .map(|i| self.works_progress(i, day))
            .collect();
        let roads: Vec<RoadInfo> = sim
            .road_runs()
            .iter()
            .zip(&works)
            .map(|(r, &w)| {
                let built = r.addition.is_none_or(|a| sim.is_revealed(a));
                RoadInfo {
                    class: r.class,
                    points: if built {
                        r.points.clone()
                    } else {
                        vec![us_gen::pt(OFF, OFF), us_gen::pt(OFF, OFF + 10.0)]
                    },
                    works: w,
                }
            })
            .collect();
        self.scene.set_geometry(lots, roads);
        // cars keep off roads under construction
        let mut vol = sim.road_values(us_sim::MapKind::Traffic);
        for (v, w) in vol.iter_mut().zip(&works) {
            if w.is_some() {
                *v = 0.0;
            }
        }
        self.scene.set_road_volumes(&vol);
        for (i, k) in self.look.iter().enumerate() {
            if matches!(k.state, 1 | 3 | 4) {
                self.scene.set_lot_info(i, k.level, false);
            }
        }
    }

    /// Roadworks progress 0 to 1 of a road run, `None` when no work is going on.
    fn works_progress(&self, run: usize, day: i32) -> Option<f32> {
        let start = self.run_work.get(run)?.start?;
        let p = (day - start) as f32 / WORKS_DAYS as f32;
        (0.0..1.0).contains(&p).then_some(p)
    }

    /// Where the first stretch of road works is, for a camera or a test.
    pub fn works_place(&self, sim: &Sim) -> Option<Pt> {
        sim.road_runs()
            .iter()
            .enumerate()
            .find(|(i, _)| self.works_progress(*i, sim.day).is_some())
            .map(|(_, r)| r.points[r.points.len() / 2])
    }

    /// Redraw the roads of one group (an addition, or the skeleton) from the current runs.
    fn rebuild_roads(&mut self, sim: &Sim, group: Group) {
        let day = sim.day;
        let runs = sim.road_runs();
        let draws: Vec<RoadDraw> = runs
            .iter()
            .enumerate()
            .filter(|(_, r)| r.addition == group)
            .map(|(i, r)| RoadDraw {
                class: r.class,
                points: r.points.as_slice(),
                works: self.works_progress(i, day),
            })
            .collect();
        let culs: Vec<Pt> = match group {
            Some(a) => self
                .town
                .cul_de_sacs
                .iter()
                .filter(|c| c.addition == a)
                .map(|c| c.pos)
                .collect(),
            None => Vec::new(),
        };
        let mesh = roads_ground_state(&draws, &culs);
        let (lo, hi) = match group {
            None => (
                Vec3::new(-100.0, -100.0, 0.0),
                Vec3::new(
                    self.town.size as f32 + 100.0,
                    self.town.size as f32 + 100.0,
                    2.0,
                ),
            ),
            Some(_) => {
                let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                for d in &draws {
                    for p in d.points {
                        lo = lo.min(Vec3::new(p.x as f32 - 30.0, p.y as f32 - 30.0, 0.0));
                        hi = hi.max(Vec3::new(p.x as f32 + 30.0, p.y as f32 + 30.0, 2.0));
                    }
                }
                for c in &culs {
                    lo = lo.min(Vec3::new(c.x as f32 - 30.0, c.y as f32 - 30.0, 0.0));
                    hi = hi.max(Vec3::new(c.x as f32 + 30.0, c.y as f32 + 30.0, 2.0));
                }
                (lo, hi)
            }
        };
        self.rev += 1;
        let chunk = Chunk {
            rev: self.rev,
            mesh,
            far: None,
            lo,
            hi,
        };
        match self.road_chunk.get(&group) {
            Some(&ci) => self.scene.chunks[ci] = chunk,
            None => {
                self.scene.chunks.push(chunk);
                self.road_chunk.insert(group, self.scene.chunks.len() - 1);
            }
        }
    }

    fn mark_roads(&mut self, g: Group) {
        if !self.road_dirty.contains(&g) {
            self.road_dirty.push(g);
        }
    }

    /// Follow road changes: new class or new streets show as roadworks that finish over time.
    fn sync_roads(&mut self, sim: &Sim) {
        let day = sim.day;
        let runs = sim.road_runs();
        if sim.roads_version() != self.roads_ver || self.run_work.len() != runs.len() {
            self.roads_ver = sim.roads_version();
            // the roads as they were drawn last time: same order as the old `run_work`
            let old_work = std::mem::take(&mut self.run_work);
            let prev: Vec<PrevRun> = self
                .prev_runs
                .drain(..)
                .zip(old_work.iter())
                .map(|(mut p, w)| {
                    p.work = *w;
                    p
                })
                .collect();
            let old = OldRoads::new(&prev);
            let mut work = Vec::with_capacity(runs.len());
            for r in runs {
                let probe = r.points.windows(2).next().map(|w| w[0].lerp(w[1], 0.5));
                let was = probe.and_then(|p| old.at(p));
                let w = match was {
                    Some((c, w)) if c == r.class => w,
                    Some(_) | None => {
                        if r.addition.is_none_or(|a| sim.is_revealed(a)) && !prev.is_empty() {
                            RunWork {
                                start: Some(day),
                                step: 255,
                            }
                        } else {
                            RunWork::default()
                        }
                    }
                };
                work.push(w);
            }
            self.run_work = work;
            for (i, r) in runs.iter().enumerate() {
                if self.run_work[i].step == 255 {
                    self.mark_roads(r.addition);
                }
            }
            self.prev_runs = runs
                .iter()
                .map(|r| PrevRun {
                    class: r.class,
                    points: r.points.clone(),
                    work: RunWork::default(),
                })
                .collect();
            self.geo_dirty = true;
            self.geo_at = Instant::now() - std::time::Duration::from_secs(5);
        }
        // advance work in progress, and finish it
        let mut touched: Vec<Group> = Vec::new();
        for (i, r) in runs.iter().enumerate().take(self.run_work.len()) {
            let w = self.run_work[i];
            let Some(start) = w.start else { continue };
            let p = (day - start) as f32 / WORKS_DAYS as f32;
            let step = if p >= 1.0 {
                254
            } else {
                (p.max(0.0) * WORKS_STEPS as f32) as u8
            };
            if step != w.step {
                self.run_work[i].step = step;
                if step == 254 {
                    self.run_work[i].start = None;
                    self.geo_dirty = true;
                }
                touched.push(r.addition);
            }
        }
        for g in touched {
            self.mark_roads(g);
        }
        let dirty = std::mem::take(&mut self.road_dirty);
        for g in dirty {
            if g.is_none_or(|a| sim.is_revealed(a)) {
                self.rebuild_roads(sim, g);
            }
        }
    }

    fn add_ground_chunk(&mut self, sim: &Sim, addition: usize) {
        let town = self.town.clone();
        let a = &town.additions[addition];
        let mesh = zone_ground(&town, addition);
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in &a.polygon {
            lo = lo.min(Vec3::new(p.x as f32 - 30.0, p.y as f32 - 30.0, 0.0));
            hi = hi.max(Vec3::new(p.x as f32 + 30.0, p.y as f32 + 30.0, 2.0));
        }
        self.rev += 1;
        self.scene.chunks.push(Chunk {
            rev: self.rev,
            mesh,
            far: None,
            lo,
            hi,
        });
        self.ground_chunks += 1;
        self.rebuild_roads(sim, Some(addition));
    }

    fn mark(&mut self, c: Cell) {
        if self.dirty.insert(c) {
            self.queue.push_back(c);
        }
    }

    /// Follow the simulation: new additions, lot stage changes, and cell rebuilds until about
    /// `max_ms` milliseconds are spent (always at least one when something is waiting).
    pub fn sync(&mut self, sim: &mut Sim, max_ms: f32) {
        let t0 = Instant::now();
        let (_, revealed) = sim.take_changes();
        if !revealed.is_empty() {
            self.geo_dirty = true;
        }
        for a in revealed {
            self.add_ground_chunk(sim, a);
        }
        // lots the mayor placed are appended to the simulation's list
        if self.look.len() < sim.lots().len() {
            self.geo_dirty = true;
        }
        while self.look.len() < sim.lots().len() {
            let i = self.look.len();
            let l = &sim.lots()[i];
            self.cell_lots
                .entry(cell_of(l.pos.x, l.pos.y))
                .or_default()
                .push(i);
            self.look.push(Look::default());
            self.objs.push(None);
            self.parked.push(true);
        }
        self.sync_roads(sim);
        let day = sim.day;
        for (i, l) in sim.lots().iter().enumerate() {
            let k = look_of(l, day);
            if k != self.look[i] {
                let was = self.look[i];
                self.look[i] = k;
                let c = cell_of(l.pos.x, l.pos.y);
                if self.dirty.insert(c) {
                    self.queue.push_back(c);
                }
                let gone = matches!(l.phase, Phase::Hidden | Phase::Cleared { .. });
                if gone != self.parked[i] {
                    // appeared or vanished: the scene's lot positions need refreshing
                    self.geo_dirty = true;
                } else if i < self.scene.lot_count()
                    && (was.level != k.level || was.state != k.state)
                {
                    self.scene
                        .set_lot_info(i, k.level, matches!(k.state, 0 | 2));
                }
            }
        }
        while let Some(c) = self.queue.pop_front() {
            self.dirty.remove(&c);
            self.rebuild_cell(sim, c);
            if t0.elapsed().as_secs_f32() * 1000.0 > max_ms {
                break;
            }
        }
        if self.geo_dirty && self.geo_at.elapsed().as_secs_f32() > 1.0 {
            self.ensure_geometry(sim);
        }
        self.triangles = self.scene.triangles();
    }

    fn rebuild_cell(&mut self, sim: &Sim, cell: Cell) {
        let base =
            (((cell.1.rem_euclid(16)) * 16 + cell.0.rem_euclid(16)) as u32) * IDS_PER_CELL + 1;
        let mut next_id = base;
        let mut mesh = Mesh::default();
        let seed = self.town.seed;
        if self.show_hall && cell == self.hall_cell {
            let h = sim.hall;
            self.hall_obj.emit(
                &mut mesh,
                &mut next_id,
                h.x as f32,
                h.y as f32,
                self.hall_theta,
                1.0,
            );
        }
        if let Some(ids) = self.cell_lots.get(&cell) {
            for &i in ids {
                let k = self.look[i];
                if k.state == 0 {
                    continue;
                }
                let l = &sim.lots()[i];
                let r = l.radius as u16;
                if !matches!(&self.objs[i], Some((lk, lr, _)) if *lk == k && *lr == r) {
                    self.objs[i] = Some((k, r, look_obj(seed, i, l, k)));
                }
                let obj = &self.objs[i].as_ref().unwrap().2;
                obj.emit(
                    &mut mesh,
                    &mut next_id,
                    l.pos.x as f32,
                    l.pos.y as f32,
                    l.facing as f32,
                    1.0,
                );
                if next_id > base + IDS_PER_CELL - 200 {
                    next_id = base;
                }
            }
        }
        let lo = Vec3::new(
            cell.0 as f32 * CELL - 60.0,
            cell.1 as f32 * CELL - 60.0,
            0.0,
        );
        let hi = Vec3::new(
            (cell.0 + 1) as f32 * CELL + 60.0,
            (cell.1 + 1) as f32 * CELL + 60.0,
            70.0,
        );
        self.rev += 1;
        let chunk = Chunk {
            rev: self.rev,
            mesh,
            far: None,
            lo,
            hi,
        };
        match self.cell_chunk.get(&cell) {
            Some(&ci) => self.scene.chunks[ci] = chunk,
            None => {
                self.scene.chunks.push(chunk);
                self.cell_chunk.insert(cell, self.scene.chunks.len() - 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::{Preset, generate};
    use us_sim::{CivicKind, Phase, hall_sites};

    /// Placing a civic building appends a lot; the scene must follow without panicking.
    #[test]
    fn scene_follows_placed_civic_building() {
        let town = Arc::new(generate(Preset::Grid, 3));
        let hall = hall_sites(&town)[0];
        let mut sim = Sim::new(&town, hall, 3);
        for _ in 0..300 {
            sim.step_day();
        }
        let mut live = LiveScene::new(town, &sim, true);
        live.sync(&mut sim, 1.0e9);
        let spot = sim
            .lots()
            .iter()
            .filter(|l| l.phase == Phase::Vacant)
            .map(|l| l.pos)
            .find(|&p| sim.check_civic(CivicKind::Park, p).is_ok())
            .expect("an empty lot beside a road");
        let before = sim.lots().len();
        sim.place_civic(CivicKind::Park, spot).expect("placed");
        assert_eq!(sim.lots().len(), before + 1);
        for _ in 0..90 {
            sim.step_day();
        }
        live.sync(&mut sim, 1.0e9);
        assert_eq!(live.look.len(), sim.lots().len());
        assert!(live.look[before].state != 0, "the new building is drawn");
    }

    /// A widened street shows as roadworks that finish; upgrades and fires change a lot's look.
    #[test]
    fn scene_follows_roads_and_lot_looks() {
        let town = Arc::new(generate(Preset::Grid, 1));
        let hall = hall_sites(&town)[0];
        let mut sim = Sim::new(&town, hall, 1);
        let mut live = LiveScene::new(town, &sim, true);
        let (mut works, mut site, mut done) = (false, false, false);
        for _ in 0..2500 {
            sim.step_day();
            live.sync(&mut sim, 1.0e9);
            works |= live.works_place(&sim).is_some();
            if works && live.works_place(&sim).is_none() {
                done = true;
            }
            site |= sim
                .lots()
                .iter()
                .zip(&live.look)
                .any(|(l, k)| l.next_level > 0 && k.state == 1 && k.level == l.next_level);
        }
        assert!(
            site,
            "an upgrade is drawn as a building site at the new level"
        );
        assert!(works, "a road change shows as roadworks");
        assert!(done, "roadworks finish");
        assert_eq!(live.run_work.len(), sim.road_runs().len());
    }
}
