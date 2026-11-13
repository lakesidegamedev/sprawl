//! The town on screen: a render scene that follows the simulation. Ground meshes appear with
//! their addition, building meshes are rebuilt per 256 m cell only when a lot in it changes.

use glam::Vec3;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use us_gen::{LotKind, Pt, RoadClass, Town, Zone};
use us_render::live::{abandoned, base_ground, construction, roads_ground, zone_ground};
use us_render::obj::Obj;
use us_render::{Chunk, LotInfo, Mesh, Rng, RoadInfo, Scene, kit};
use us_sim::{LotState, Phase, Sim};

const CELL: f32 = 256.0;
const IDS_PER_CELL: u32 = 60_000;

type Cell = (i32, i32);

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
    stage: Vec<u8>,
    objs: Vec<Option<Obj>>,
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

fn stage_of(l: &LotState, day: i32) -> u8 {
    match l.phase {
        Phase::Hidden | Phase::Vacant | Phase::Cleared { .. } => 0,
        Phase::Building { start, end } => {
            let p = (day - start) as f32 / (end - start).max(1) as f32;
            1 + (p * 4.0).clamp(0.0, 3.0) as u8
        }
        Phase::Built { .. } => 10,
        Phase::Empty { .. } => 20,
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

/// The full model of one lot (building and its trees), local to the lot, facing south.
fn lot_model(seed: u64, src: usize, kind: LotKind, radius: f64) -> Obj {
    let mut rng = Rng::new(seed ^ 0x5eed_0000, src as u64 * 7919 + 13);
    let mut obj = kit::build_lot(kind, &mut rng);
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
        // skeleton roads: always there
        let skel: Vec<(RoadClass, &[Pt])> = sim
            .road_runs()
            .iter()
            .filter(|r| r.addition.is_none())
            .map(|r| (r.class, r.points.as_slice()))
            .collect();
        let mesh = roads_ground(&skel, &[]);
        scene.chunks.push(Chunk {
            rev: 1,
            mesh,
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
            stage: vec![0; n],
            objs: vec![None; n],
            rev: 1,
            ground_chunks: 0,
            triangles: 0,
            geo_dirty: true,
            geo_at: Instant::now(),
        };
        if show_hall {
            ls.mark(hall_cell);
        }
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
        let lots: Vec<LotInfo> = sim
            .lots()
            .iter()
            .map(|l| {
                let gone = matches!(l.phase, Phase::Hidden | Phase::Cleared { .. });
                LotInfo {
                    x: if gone { OFF as f32 } else { l.pos.x as f32 },
                    y: if gone { OFF as f32 } else { l.pos.y as f32 },
                    theta: l.facing as f32,
                    kind: l.kind,
                    level: l.level.clamp(1, 5),
                }
            })
            .collect();
        let roads: Vec<RoadInfo> = sim
            .road_runs()
            .iter()
            .map(|r| {
                let built = r.addition.is_none_or(|a| sim.is_revealed(a));
                RoadInfo {
                    class: r.class,
                    points: if built {
                        r.points.clone()
                    } else {
                        vec![us_gen::pt(OFF, OFF), us_gen::pt(OFF, OFF + 10.0)]
                    },
                    works: None,
                }
            })
            .collect();
        self.scene.set_geometry(lots, roads);
        self.scene
            .set_road_volumes(&sim.road_values(us_sim::MapKind::Traffic));
    }

    fn add_ground_chunk(&mut self, sim: &Sim, addition: usize) {
        let town = self.town.clone();
        let a = &town.additions[addition];
        let mut mesh = zone_ground(&town, addition);
        let runs: Vec<(RoadClass, &[Pt])> = sim
            .road_runs()
            .iter()
            .filter(|r| r.addition == Some(addition))
            .map(|r| (r.class, r.points.as_slice()))
            .collect();
        let culs: Vec<Pt> = town
            .cul_de_sacs
            .iter()
            .filter(|c| c.addition == addition)
            .map(|c| c.pos)
            .collect();
        let roads = roads_ground(&runs, &culs);
        append(&mut mesh, roads);
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let mut grow = |p: Pt| {
            lo = lo.min(Vec3::new(p.x as f32 - 30.0, p.y as f32 - 30.0, 0.0));
            hi = hi.max(Vec3::new(p.x as f32 + 30.0, p.y as f32 + 30.0, 2.0));
        };
        for p in &a.polygon {
            grow(*p);
        }
        for (_, pts) in &runs {
            for p in *pts {
                grow(*p);
            }
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
        if self.stage.len() < sim.lots().len() {
            self.geo_dirty = true;
        }
        while self.stage.len() < sim.lots().len() {
            let i = self.stage.len();
            let l = &sim.lots()[i];
            self.cell_lots
                .entry(cell_of(l.pos.x, l.pos.y))
                .or_default()
                .push(i);
            self.stage.push(0);
            self.objs.push(None);
        }
        let day = sim.day;
        for (i, l) in sim.lots().iter().enumerate() {
            let s = stage_of(l, day);
            if s != self.stage[i] {
                self.stage[i] = s;
                let c = cell_of(l.pos.x, l.pos.y);
                if self.dirty.insert(c) {
                    self.queue.push_back(c);
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
                let st = self.stage[i];
                if st == 0 {
                    continue;
                }
                let l = &sim.lots()[i];
                if self.objs[i].is_none() {
                    self.objs[i] = Some(lot_model(seed, l.src, l.kind, l.radius));
                }
                let full = self.objs[i].as_ref().unwrap();
                let (x, y, th) = (l.pos.x as f32, l.pos.y as f32, l.facing as f32);
                match st {
                    1..=4 => {
                        let o = construction(full, st as f32 / 5.0);
                        o.emit(&mut mesh, &mut next_id, x, y, th, 1.0);
                    }
                    20 => abandoned(full).emit(&mut mesh, &mut next_id, x, y, th, 1.0),
                    _ => full.emit(&mut mesh, &mut next_id, x, y, th, 1.0),
                }
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

fn append(dst: &mut Mesh, src: Mesh) {
    let off = dst.vertices.len() as u32;
    dst.vertices.extend(src.vertices);
    dst.cast.extend(src.cast.into_iter().map(|i| i + off));
    dst.plain.extend(src.plain.into_iter().map(|i| i + off));
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
        assert_eq!(live.stage.len(), sim.lots().len());
        assert!(live.stage[before] != 0, "the new building is drawn");
    }
}
