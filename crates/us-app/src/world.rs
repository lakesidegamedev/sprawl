//! The town on screen: a render scene that follows the simulation. Ground meshes appear with
//! their addition, building meshes are rebuilt per 256 m cell only when a lot in it changes.

use glam::Vec3;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use us_gen::{LotKind, Preset, Pt, RoadClass, Town, Zone};
use us_render::decor::{
    DRIVE_CLEAR, Streetscape, footpath_pieces, lot_pad, plan_footpath, town_square,
};
use us_render::levels::{building_site_for, burned};
use us_render::live::{
    RoadDraw, abandoned, base_ground_far_on, base_ground_on, grown, roads_ground_state_ctx_on,
    zone_ground_on,
};
use us_render::obj::{Obj, Sit};
use us_render::site::{step_of, step_progress};
use us_render::{
    Chunk, CivicModel, Land, LotInfo, Mesh, Rng, RoadInfo, Scene, build_civic,
    build_lot_level_dense, kit,
};
use us_sim::{CivicKind, LotState, Phase, Sim};

const CELL: f32 = 256.0;
const IDS_PER_CELL: u32 = 60_000;

/// Game days a road stays under construction after it is widened or built.
const WORKS_DAYS: i32 = 40;
/// Roadworks are redrawn this many times over their length.
const WORKS_STEPS: i32 = 8;

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
    /// Progress step of a building site, 0 to `STEPS - 1`.
    pub step: u8,
    /// Level of the building being replaced while a site, 0 for a bare lot.
    pub from: u8,
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

/// A street tree, planter or bench: placed once from the town, drawn while its street is open.
struct Deco {
    obj: Obj,
    x: f32,
    y: f32,
    turn: f32,
    floor: f32,
    /// Planters and benches also need a finished building near.
    plaza: bool,
    /// The town square around the hall: always there.
    always: bool,
}

/// One piece of a footpath from a finished lot to the street or the square.
struct PathPiece {
    obj: Obj,
    x: f32,
    y: f32,
    floor: f32,
    cell: Cell,
}

/// A road segment with its half width.
type RoadSeg = (Pt, Pt, f64);

/// Open road segments in a grid: "is a finished street within this far of a point".
#[derive(Default)]
struct OpenRoads {
    cells: HashMap<(i32, i32), Vec<RoadSeg>>,
}

impl OpenRoads {
    fn new(sim: &Sim) -> OpenRoads {
        let mut o = OpenRoads::default();
        for (i, r) in sim.road_runs().iter().enumerate() {
            if !sim.road_open(i) || matches!(r.class, RoadClass::Rail | RoadClass::Highway) {
                continue;
            }
            for w in r.points.windows(2) {
                let n = (w[0].dist(w[1]) / 16.0).ceil().max(1.0) as usize;
                let mut last = None;
                for k in 0..=n {
                    let p = w[0].lerp(w[1], k as f64 / n as f64);
                    let c = ((p.x / 32.0).floor() as i32, (p.y / 32.0).floor() as i32);
                    if last != Some(c) {
                        o.cells
                            .entry(c)
                            .or_default()
                            .push((w[0], w[1], r.class.half_width()));
                        last = Some(c);
                    }
                }
            }
        }
        o
    }

    /// The nearest open street edge to `p` within `reach` metres: the point on the street's
    /// side, the gap to it and the street's half width.
    fn nearest(&self, p: Pt, reach: f64) -> Option<(Pt, f64)> {
        let (cx, cy) = ((p.x / 32.0).floor() as i32, (p.y / 32.0).floor() as i32);
        let rings = (reach / 32.0).ceil() as i32 + 1;
        let mut best: Option<(f64, Pt, f64)> = None;
        for ring in 0..=rings {
            if let Some((d, _, _)) = best
                && d < (ring - 1).max(0) as f64 * 32.0
            {
                break;
            }
            for dx in -ring..=ring {
                for dy in -ring..=ring {
                    if dx.abs().max(dy.abs()) != ring {
                        continue;
                    }
                    for (a, b, hw) in self.cells.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                        let (abx, aby) = (b.x - a.x, b.y - a.y);
                        let l2 = abx * abx + aby * aby;
                        let t = if l2 < 1e-9 {
                            0.0
                        } else {
                            (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
                        };
                        let q = us_gen::pt(a.x + abx * t, a.y + aby * t);
                        let d = p.dist(q) - hw;
                        if d < best.map_or(f64::MAX, |b| b.0) {
                            best = Some((d, q, *hw));
                        }
                    }
                }
            }
        }
        let (d, q, hw) = best?;
        if d > reach {
            return None;
        }
        // the sidewalk: a metre off the kerb, toward the lot
        let l = q.dist(p).max(1e-9);
        let side = us_gen::pt(
            q.x + (p.x - q.x) / l * (hw + 1.0),
            q.y + (p.y - q.y) / l * (hw + 1.0),
        );
        Some((side, d))
    }

    fn near(&self, x: f32, y: f32, r: f64) -> bool {
        let p = us_gen::pt(x as f64, y as f64);
        let (cx, cy) = ((p.x / 32.0).floor() as i32, (p.y / 32.0).floor() as i32);
        (-1..=1).any(|dx| {
            (-1..=1).any(|dy| {
                self.cells
                    .get(&(cx + dx, cy + dy))
                    .is_some_and(|v| v.iter().any(|(a, b, _)| dist_seg(p, *a, *b) < r))
            })
        })
    }
}

pub struct LiveScene {
    pub scene: Scene,
    scape: Streetscape,
    /// Street trees and downtown furniture by the cell they stand in.
    decos: HashMap<Cell, Vec<Deco>>,
    /// How many of a cell's decorations were drawn last time.
    deco_drawn: HashMap<Cell, usize>,
    open: OpenRoads,
    /// Whether each road run was open when `open` was built.
    open_flags: Vec<bool>,
    /// Footpaths of finished lots that stand away from a street, by lot.
    paths: HashMap<usize, Vec<PathPiece>>,
    /// Centres of the town square's paving: where a footpath can end.
    square_tiles: Vec<Pt>,
    /// Lots with path pieces, by the cell the pieces are in.
    path_cells: HashMap<Cell, Vec<usize>>,
    town: Arc<Town>,
    show_hall: bool,
    hall_theta: f32,
    hall_obj: Obj,
    /// Railway depots and elevators: model, place, turn and the cell they are drawn in.
    stations: Vec<(Obj, f32, f32, f32, Cell)>,
    /// The hills the town stands on.
    land: Land,
    cell_chunk: HashMap<Cell, usize>,
    cell_lots: HashMap<Cell, Vec<usize>>,
    hall_cell: Cell,
    dirty: HashSet<Cell>,
    queue: VecDeque<Cell>,
    pub look: Vec<Look>,
    /// Finished model per lot, with the look and radius it was made for.
    objs: Vec<Option<(Look, u16, LotDraw)>>,
    /// Lots parked off the map in the scene's geometry (not drawn yet).
    parked: Vec<bool>,
    road_chunk: HashMap<Group, usize>,
    road_dirty: Vec<Group>,
    roads_ver: u32,
    run_work: Vec<RunWork>,
    /// Progress of each street the town is building now (from the simulation), by run.
    build_prog: Vec<Option<f32>>,
    /// The step of `build_prog` last drawn, 0 when not under construction.
    build_step: Vec<u8>,
    prev_runs: Vec<PrevRun>,
    rev: u64,
    ground_chunks: usize,
    /// Chunks 1 to this hold the base ground tiles.
    ground_tile_count: usize,
    pub triangles: usize,
    /// Lots or roads changed since the scene was last told the geometry.
    geo_dirty: bool,
    geo_at: Instant,
    traffic_day: Option<i32>,
    /// The chunk that holds every tram track, and what it was last drawn from.
    tram_chunk: Option<usize>,
    tram_key: Vec<u32>,
}

fn cell_of(x: f64, y: f64) -> Cell {
    (
        (x as f32 / CELL).floor() as i32,
        (y as f32 / CELL).floor() as i32,
    )
}

fn look_of(l: &LotState, day: i32, burning: bool) -> Look {
    let level = l.level.clamp(1, 5);
    match l.phase {
        Phase::Hidden | Phase::Vacant | Phase::Cleared { .. } => Look::default(),
        // a fire still burning shows the building standing, with flames and smoke over it
        _ if l.fire.is_some() && !burning => Look {
            state: 3,
            level,
            step: 0,
            from: 0,
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
                step: step_of(p),
                from: if l.next_level > 0 { level } else { 0 },
            }
        }
        Phase::Built { .. } => Look {
            state: 2,
            level,
            step: 0,
            from: 0,
        },
        Phase::Empty { .. } => Look {
            state: 4,
            level,
            step: 0,
            from: 0,
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
        Some(CivicKind::Power) => Some(CivicModel::PowerPlant),
        Some(CivicKind::Library) => Some(CivicModel::Library),
        Some(CivicKind::HighSchool) => Some(CivicModel::HighSchool),
        Some(CivicKind::Hospital) => Some(CivicModel::Hospital),
        Some(CivicKind::Transit) => Some(CivicModel::Transit),
        Some(CivicKind::Stadium) => Some(CivicModel::Stadium),
        Some(CivicKind::University) => Some(CivicModel::University),
        _ => None,
    }
}

/// What one lot is drawn with: the building, and its trees, each standing on the ground under it.
struct LotDraw {
    building: Obj,
    /// Tree models with their place in the lot (local, facing south).
    trees: Vec<(f32, f32, Obj)>,
    /// Driveway, walk, lawn, shrubs or forecourt furniture, local to the lot.
    yard: Option<Obj>,
    /// Paving under the lot in downtown, industry and mall districts.
    pad: Option<Obj>,
    /// Where the driveway meets the street (world), which street trees keep clear of.
    drive: Option<Pt>,
}

/// The model of one lot at a level, local to the lot, facing south.
fn lot_model(preset: Preset, seed: u64, l: &LotState, level: u8, density: f32) -> LotDraw {
    let mut rng = Rng::new(seed ^ 0x5eed_0000, l.src as u64 * 7919 + 13);
    let building = match civic_model(l.civic) {
        Some(m) => build_civic(m, &mut rng),
        None if l.civic.is_some() => kit::build_lot_for(preset, l.kind, l.radius as f32, &mut rng),
        None => build_lot_level_dense(preset, l.kind, level, l.radius as f32, density, &mut rng),
    };
    let (kind, radius) = (l.kind, l.radius);
    let n = trees_for(kind, &mut rng);
    let mut trees = Vec::new();
    if n > 0 {
        let fp = building.footprint();
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
                let col = rng.pick(&green);
                let roll = rng.f32();
                trees.push((lx, ly, us_render::kit_more::tree_species(roll, rad, h, col)));
                break;
            }
        }
    }
    LotDraw {
        building,
        trees,
        yard: None,
        pad: None,
        drive: None,
    }
}

/// The model to draw for a lot in its current look.
fn look_obj(
    preset: Preset,
    seed: u64,
    i: usize,
    l: &LotState,
    look: Look,
    density: f32,
    ground: (&Streetscape, &Town),
) -> LotDraw {
    let level = if l.civic.is_some() { 1 } else { look.level };
    let mut d = lot_model(preset, seed, l, level, density);
    // the grounds around a finished building, from the model as built
    if l.civic.is_none() && matches!(look.state, 2 | 4) {
        let (yard, drive) = ground.0.lot_yard(ground.1, l.src, &d.building);
        d.yard = yard.map(|y| if look.state == 4 { faded(&y) } else { y });
        d.drive = drive;
    }
    if matches!(look.state, 2..=4)
        && let Some(a) = ground.1.additions.get(l.addition)
    {
        d.pad = lot_pad(a.zone, l.radius as f32, d.building.footprint());
    }
    let mut rng = Rng::new(seed ^ 0xb0b0, i as u64);
    let mut dress = |o: &Obj| match look.state {
        1 => {
            let old =
                (look.from > 0).then(|| lot_model(preset, seed, l, look.from, density).building);
            let tall = look.level >= 4 || l.kind == LotKind::Office;
            building_site_for(o, old.as_ref(), tall, step_progress(look.step))
        }
        3 => burned(o, &mut rng),
        4 => abandoned(o),
        _ => o.clone(),
    };
    if look.state == 1 {
        d.building = dress(&d.building);
        // trees come back at the end of the job, small first, and grow to size
        let k = match look.step {
            0..=7 => 0.0,
            8 => 0.3,
            9 => 0.5,
            10 => 0.75,
            _ => 0.9,
        };
        if k == 0.0 {
            d.trees.clear();
        } else {
            for t in &mut d.trees {
                t.2 = grown(&t.2, k);
            }
        }
    } else if look.state != 0 && look.state != 2 {
        d.building = dress(&d.building);
        for t in &mut d.trees {
            t.2 = dress(&t.2);
        }
    }
    d
}

/// A copy of a yard with its colours pulled toward grey: nobody tends it any more.
fn faded(o: &Obj) -> Obj {
    let mut o = o.clone();
    for p in &mut o.polys {
        let g = (p.color[0] as u32 + p.color[1] as u32 + p.color[2] as u32) / 3;
        let m = |c: u8| ((c as u32 * 4 + g * 6) / 10) as u8;
        p.color = [m(p.color[0]), m(p.color[1]), m(p.color[2])];
    }
    o
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
        // `URBAN_SPRAWL_FLAT=1` draws the town on level ground, to compare frame times
        let land = if std::env::var("URBAN_SPRAWL_FLAT").is_ok_and(|v| v == "1") {
            Land::flat(town.size as f32)
        } else {
            Land::from_town(&town)
        };
        let ground = base_ground_on(&land, &town, &farm);
        let hall = sim.hall;
        let mut scene = Scene::dynamic(
            town.size as f32,
            ground,
            Vec3::new(
                hall.x as f32,
                hall.y as f32,
                land.height(hall.x as f32, hall.y as f32),
            ),
        );
        scene.set_land(land.clone());
        scene.base = us_render::RockLook::of(town.preset)
            .and_then(|l| l.ground)
            .map_or(us_render::GRASS, |g| g.flat);
        // road ground per group; the skeleton and connectors are chunk 0
        scene.chunks.push(Chunk {
            rev: 1,
            mesh: Mesh::default(),
            far: None,
            lo: Vec3::new(-100.0, -100.0, 0.0),
            hi: Vec3::new(town.size as f32 + 100.0, town.size as f32 + 100.0, 2.0),
        });
        // the hills in tiles, each with a coarse form for when it is small on screen
        if !land.is_flat() {
            let coarse = coarse_land(&land, &town);
            let far = base_ground_far_on(&land, &coarse, &town, &farm);
            let near = std::mem::take(&mut scene.ground);
            let exact = ground_tiles_needing_exact_height(&land, &coarse);
            scene
                .chunks
                .extend(ground_tiles(&near, &far, town.size as f32, &exact));
        }
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
        let stations: Vec<(Obj, f32, f32, f32, Cell)> = town
            .stations
            .iter()
            .map(|st| {
                (
                    us_render::rail::station_model(st, town.seed),
                    st.pos.x as f32,
                    st.pos.y as f32,
                    st.facing as f32,
                    cell_of(st.pos.x, st.pos.y),
                )
            })
            .collect();
        let n = sim.lots().len();
        let ground_tile_count = scene.chunks.len() - 1;
        scene.chunks.extend(us_render::cover::build(
            &town,
            &land,
            us_render::cover::Group::Base,
        ));
        let scape = Streetscape::new(&town);
        let mut decos: HashMap<Cell, Vec<Deco>> = HashMap::new();
        for (plaza, list) in [
            (false, scape.street_trees(&town, &land)),
            (true, scape.plaza(&town, &land)),
        ] {
            for (obj, x, y, turn, floor) in list {
                if land.wet_shelf(x, y) {
                    continue;
                }
                decos
                    .entry(cell_of(x as f64, y as f64))
                    .or_default()
                    .push(Deco {
                        obj,
                        x,
                        y,
                        turn,
                        floor,
                        plaza,
                        always: false,
                    });
            }
        }
        let mut square_cells: Vec<Cell> = Vec::new();
        let mut square_tiles: Vec<Pt> = Vec::new();
        if show_hall {
            let sq = town_square(&town, &land, hall, hall_theta);
            square_tiles = sq.tiles;
            for (obj, x, y, turn, floor) in sq.pieces {
                let c = cell_of(x as f64, y as f64);
                square_cells.push(c);
                decos.entry(c).or_default().push(Deco {
                    obj,
                    x,
                    y,
                    turn,
                    floor,
                    plaza: false,
                    always: true,
                });
            }
        }
        let (build_prog, build_step) = (0..sim.road_runs().len())
            .map(|i| Self::road_draw_state(sim, i))
            .unzip();
        let mut ls = LiveScene {
            scene,
            scape,
            decos,
            deco_drawn: HashMap::new(),
            open: OpenRoads::default(),
            open_flags: Vec::new(),
            paths: HashMap::new(),
            square_tiles,
            path_cells: HashMap::new(),
            town,
            show_hall,
            hall_theta,
            hall_obj,
            stations,
            land,
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
            build_prog,
            build_step,
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
            ground_tile_count,
            triangles: 0,
            geo_dirty: true,
            geo_at: Instant::now(),
            traffic_day: None,
            tram_chunk: None,
            tram_key: vec![u32::MAX],
        };
        if show_hall {
            ls.mark(hall_cell);
        }
        for c in square_cells {
            ls.mark(c);
        }
        let station_cells: Vec<Cell> = ls.stations.iter().map(|s| s.4).collect();
        for c in station_cells {
            ls.mark(c);
        }
        ls.rebuild_roads(sim, None);
        for addition in 0..ls.town.additions.len() {
            // A save usually has already consumed the notification that this area opened.
            if sim.is_revealed(addition) {
                ls.add_ground_chunk(sim, addition);
            }
        }
        ls.refresh_open(sim);
        ls.ensure_geometry(sim);
        ls
    }

    /// Follow which streets are open: street trees and downtown furniture appear along a street
    /// when it opens, so the cells that hold them are redrawn.
    fn refresh_open(&mut self, sim: &Sim) {
        let flags: Vec<bool> = (0..sim.road_runs().len())
            .map(|i| sim.road_open(i))
            .collect();
        if flags == self.open_flags {
            return;
        }
        self.open_flags = flags;
        self.open = OpenRoads::new(sim);
        // a street opened or closed: footpaths may have a nearer street to go to
        let standing: Vec<usize> = (0..self.look.len())
            .filter(|&i| self.look[i].state == 2)
            .collect();
        for i in standing {
            self.refresh_path(sim, i);
        }
        let cells: Vec<Cell> = self.decos.keys().copied().collect();
        for c in cells {
            let n = self.decos[&c]
                .iter()
                .filter(|d| self.deco_shown(sim, d))
                .count();
            if self.deco_drawn.get(&c).copied().unwrap_or(0) != n {
                self.mark(c);
            }
        }
    }

    /// Whether a lot gets a footpath: a downtown, shop, office or apartment lot.
    fn wants_path(&self, sim: &Sim, i: usize) -> bool {
        let l = &sim.lots()[i];
        if l.civic.is_some() || l.exempt {
            return false;
        }
        let core = self
            .town
            .additions
            .get(l.addition)
            .is_some_and(|a| a.zone == Zone::Core);
        core && !matches!(l.kind, LotKind::Warehouse | LotKind::Workshop)
            || matches!(
                l.kind,
                LotKind::Shop | LotKind::FastFood | LotKind::Office | LotKind::Apartments
            )
    }

    /// The footpath of lot `i`: made when the building stands more than 12 m from an open
    /// street, dropped when it stops standing. Redraws the chunks it touches.
    fn refresh_path(&mut self, sim: &Sim, i: usize) {
        let mut pieces: Vec<PathPiece> = Vec::new();
        if self.look[i].state == 2 && self.wants_path(sim, i) {
            pieces = self.path_pieces(sim, i);
        }
        let mut touched: Vec<Cell> = Vec::new();
        if let Some(old) = self.paths.remove(&i) {
            for p in &old {
                touched.push(p.cell);
            }
            for c in old.iter().map(|p| p.cell) {
                if let Some(v) = self.path_cells.get_mut(&c) {
                    v.retain(|&k| k != i);
                }
            }
        }
        for p in &pieces {
            touched.push(p.cell);
            let v = self.path_cells.entry(p.cell).or_default();
            if !v.contains(&i) {
                v.push(i);
            }
        }
        if !pieces.is_empty() {
            self.paths.insert(i, pieces);
        }
        touched.sort_unstable();
        touched.dedup();
        for c in touched {
            self.mark(c);
        }
    }

    fn path_pieces(&self, sim: &Sim, i: usize) -> Vec<PathPiece> {
        let l = &sim.lots()[i];
        let c = l.pos;
        let pad_r = l.radius * 0.7;
        // where to go: the nearest open street, or the edge of the town square
        let mut goal: Option<(f64, Pt)> =
            self.open.nearest(c, 400.0).map(|(side, gap)| (gap, side));
        for t in &self.square_tiles {
            let gap = c.dist(*t);
            if goal.is_none_or(|g| gap < g.0) {
                goal = Some((gap, *t));
            }
        }
        let Some((gap, end)) = goal else {
            return Vec::new();
        };
        if gap - pad_r <= 12.0 {
            return Vec::new();
        }
        let dir = us_gen::pt((end.x - c.x) / c.dist(end), (end.y - c.y) / c.dist(end));
        let start = us_gen::pt(c.x + dir.x * pad_r, c.y + dir.y * pad_r);
        // go round the other lots near the line
        let (lo, hi) = (
            us_gen::pt(start.x.min(end.x) - 60.0, start.y.min(end.y) - 60.0),
            us_gen::pt(start.x.max(end.x) + 60.0, start.y.max(end.y) + 60.0),
        );
        let blocks: Vec<(Pt, f64)> = sim
            .lots()
            .iter()
            .enumerate()
            .filter(|(k, o)| {
                *k != i
                    && o.pos.x > lo.x
                    && o.pos.x < hi.x
                    && o.pos.y > lo.y
                    && o.pos.y < hi.y
                    && o.pos.dist(start) > 3.0
                    && o.pos.dist(end) > 3.0
            })
            .map(|(_, o)| (o.pos, o.radius * 0.5 + 1.0))
            .collect();
        let line = plan_footpath(start, end, &blocks);
        footpath_pieces(&line, 40.0)
            .into_iter()
            .map(|(obj, mid)| PathPiece {
                obj,
                x: mid.x as f32,
                y: mid.y as f32,
                floor: self.land.height(mid.x as f32, mid.y as f32),
                cell: cell_of(mid.x, mid.y),
            })
            .collect()
    }

    /// Whether a street tree or plaza piece is drawn now.
    fn deco_shown(&self, sim: &Sim, d: &Deco) -> bool {
        if d.always {
            return true;
        }
        if !self.open.near(d.x, d.y, if d.plaza { 30.0 } else { 14.0 }) {
            return false;
        }
        let here = cell_of(d.x as f64, d.y as f64);
        if d.plaza {
            // only beside a finished building
            return self.nearby_lots(here).any(|i| {
                self.look[i].state == 2
                    && sim.lots()[i].pos.dist(us_gen::pt(d.x as f64, d.y as f64)) < 35.0
            });
        }
        // a driveway of a built lot keeps trees off it
        let p = us_gen::pt(d.x as f64, d.y as f64);
        !self.nearby_lots(here).any(|i| {
            matches!(&self.objs[i], Some((_, _, lw)) if lw.drive.is_some_and(|m| m.dist(p) < DRIVE_CLEAR))
        })
    }

    /// A lot near these cells was finished or lost: redraw the cells whose downtown furniture
    /// shows or hides because of it.
    fn refresh_plazas(&mut self, sim: &Sim, c: Cell) {
        for dx in -1..=1 {
            for dy in -1..=1 {
                let cc = (c.0 + dx, c.1 + dy);
                let Some(list) = self.decos.get(&cc) else {
                    continue;
                };
                if !list.iter().any(|d| d.plaza) {
                    continue;
                }
                let n = list.iter().filter(|d| self.deco_shown(sim, d)).count();
                if self.deco_drawn.get(&cc).copied().unwrap_or(0) != n {
                    self.mark(cc);
                }
            }
        }
    }

    /// Lots in a cell and its eight neighbours.
    fn nearby_lots(&self, c: Cell) -> impl Iterator<Item = usize> + '_ {
        (-1..=1).flat_map(move |dx| {
            (-1..=1).flat_map(move |dy| {
                self.cell_lots
                    .get(&(c.0 + dx, c.1 + dy))
                    .into_iter()
                    .flatten()
                    .copied()
            })
        })
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
            .enumerate()
            .zip(&works)
            .map(|((i, r), &w)| {
                let built = sim.road_visible(i);
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
        self.refresh_traffic(sim);
        for (i, k) in self.look.iter().enumerate() {
            if matches!(k.state, 1 | 3 | 4) {
                self.scene.set_lot_info(i, k.level, false);
            }
        }
    }

    fn refresh_traffic(&mut self, sim: &Sim) {
        self.traffic_day = Some(sim.day);
        let roads: Vec<_> = sim
            .road_traffic()
            .iter()
            .zip(sim.road_runs())
            .enumerate()
            .map(|(i, (traffic, run))| {
                if !sim.road_open(i)
                    || self.works_progress(i, sim.day).is_some()
                    || sim.flooded_roads().binary_search(&i).is_ok()
                {
                    return Default::default();
                }
                traffic
                    .directions_for(run.class)
                    .map(|a| us_render::RoadApproach {
                        rate: a.rate,
                        capacity: a.capacity,
                        stop: a.stop,
                    })
            })
            .collect();
        self.scene.set_road_traffic(&roads);
    }

    fn road_draw_state(sim: &Sim, i: usize) -> (Option<f32>, u8) {
        if !sim.road_visible(i) {
            return (None, 0);
        }
        let progress = sim
            .road_build_state(i)
            .map(|state| state.progress.min(0.999));
        let step = progress.map_or(1, |p| 2 + (p * WORKS_STEPS as f32) as u8);
        (progress, step)
    }

    /// Roadworks progress 0 to 1 of a road run, `None` when no work is going on.
    fn works_progress(&self, run: usize, day: i32) -> Option<f32> {
        if let Some(Some(p)) = self.build_prog.get(run) {
            return Some(*p);
        }
        let start = self.run_work.get(run)?.start?;
        let p = (day - start) as f32 / WORKS_DAYS as f32;
        (0.0..1.0).contains(&p).then_some(p)
    }

    /// The hills the town stands on.
    /// Triangles in the base ground mesh.
    pub fn ground_triangles(&self) -> usize {
        self.scene.ground.triangles()
            + (1..self.ground_tile_count + 1)
                .map(|i| self.scene.chunks[i].mesh.triangles())
                .sum::<usize>()
    }

    pub fn land(&self) -> &Land {
        &self.land
    }

    /// The ground height at a point of the town.
    pub fn ground_at(&self, x: f32, y: f32) -> f32 {
        self.land.height(x, y)
    }

    /// Lowest and highest z a chunk of things `above` metres tall can reach on this land.
    fn z_range(&self, above: f32) -> (f32, f32) {
        (
            self.land.min_height().min(0.0),
            self.land.max_height().max(0.0) + above + 12.0,
        )
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
            .filter(|(i, r)| r.addition == group && sim.road_visible(*i))
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
                // the bulb is drawn once the street that ends in it is there
                .filter(|c| {
                    runs.iter().enumerate().any(|(i, r)| {
                        r.addition == group
                            && sim.road_visible(i)
                            && r.points.iter().any(|p| p.dist(c.pos) < 12.0)
                    })
                })
                .map(|c| c.pos)
                .collect(),
            None => Vec::new(),
        };
        // roads of the skeleton and of the neighbouring additions that meet these ones: they only
        // decide where junctions are, each is drawn by its own group
        let context: Vec<RoadDraw> = match group {
            None => Vec::new(),
            Some(_) => {
                let (mut lo, mut hi) = (
                    Pt {
                        x: f64::MAX,
                        y: f64::MAX,
                    },
                    Pt {
                        x: f64::MIN,
                        y: f64::MIN,
                    },
                );
                for d in &draws {
                    for p in d.points {
                        lo = Pt {
                            x: lo.x.min(p.x),
                            y: lo.y.min(p.y),
                        };
                        hi = Pt {
                            x: hi.x.max(p.x),
                            y: hi.y.max(p.y),
                        };
                    }
                }
                runs.iter()
                    .enumerate()
                    .filter(|(i, r)| r.addition != group && sim.road_visible(*i))
                    .filter(|(_, r)| {
                        r.points.iter().any(|p| {
                            p.x > lo.x - 30.0
                                && p.x < hi.x + 30.0
                                && p.y > lo.y - 30.0
                                && p.y < hi.y + 30.0
                        })
                    })
                    .map(|(i, r)| RoadDraw {
                        class: r.class,
                        points: r.points.as_slice(),
                        works: self.works_progress(i, day),
                    })
                    .collect()
            }
        };
        let mesh = roads_ground_state_ctx_on(&self.land, &draws, &culs, &context);
        let (zlo, zhi) = self.z_range(2.0);
        let (lo, hi) = match group {
            None => (
                Vec3::new(-100.0, -100.0, zlo),
                Vec3::new(
                    self.town.size as f32 + 100.0,
                    self.town.size as f32 + 100.0,
                    zhi,
                ),
            ),
            Some(_) => {
                let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                for d in &draws {
                    for p in d.points {
                        lo = lo.min(Vec3::new(p.x as f32 - 30.0, p.y as f32 - 30.0, zlo));
                        hi = hi.max(Vec3::new(p.x as f32 + 30.0, p.y as f32 + 30.0, zhi));
                    }
                }
                for c in &culs {
                    lo = lo.min(Vec3::new(c.x as f32 - 30.0, c.y as f32 - 30.0, zlo));
                    hi = hi.max(Vec3::new(c.x as f32 + 30.0, c.y as f32 + 30.0, zhi));
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

    /// Tell the trains how often to run and where they stop: open depots, grain elevators and
    /// the town's size decide it.
    pub fn set_rail_service(&mut self, sim: &Sim) {
        self.scene.set_population(sim.population().max(0) as u32);
        let rates = sim.train_rates();
        let depots = sim
            .open_depots()
            .iter()
            .map(|p| [p.x as f32, p.y as f32])
            .collect();
        let elevators = self
            .town
            .stations
            .iter()
            .filter(|s| s.kind == us_gen::StationKind::Elevator)
            .map(|s| [s.pos.x as f32, s.pos.y as f32])
            .collect();
        self.scene.set_trains(us_render::trains::TrainService {
            passenger_per_hour: rates.passenger,
            freight_per_hour: rates.freight,
            depots,
            elevators,
        });
        self.sync_trams(sim);
    }

    /// Draw the tram tracks as the lines are built and tell the scene where trams run.
    fn sync_trams(&mut self, sim: &Sim) {
        use us_render::trams::{BuildStage, TrackDraw, TramLineService, TramService};
        let lines = sim.tram_lines();
        let mut key = vec![sim.trams_version()];
        for l in &lines {
            key.push(
                match l.state {
                    us_sim::TramState::Track => 0,
                    us_sim::TramState::Wires => 100,
                    us_sim::TramState::Open => 200,
                } + (l.progress * 12.0) as u32,
            );
        }
        let per_hour = sim.tram_rate();
        self.scene.set_trams(TramService {
            lines: lines
                .iter()
                .map(|l| TramLineService {
                    points: l.points.iter().map(|p| [p.x as f32, p.y as f32]).collect(),
                    stop_at: l.stop_at.iter().map(|&s| s as f32).collect(),
                    per_hour,
                    open: l.state == us_sim::TramState::Open,
                })
                .collect(),
        });
        if key == self.tram_key {
            return;
        }
        self.tram_key = key;
        let draws: Vec<TrackDraw> = lines
            .iter()
            .map(|l| TrackDraw {
                points: &l.points,
                stage: match l.state {
                    us_sim::TramState::Track => BuildStage::Track,
                    us_sim::TramState::Wires => BuildStage::Wires,
                    us_sim::TramState::Open => BuildStage::Open,
                },
                progress: l.progress,
            })
            .collect();
        let mesh = us_render::trams::track_mesh(&self.land, &draws);
        let (zlo, zhi) = self.z_range(8.0);
        let size = self.town.size as f32;
        self.rev += 1;
        let chunk = Chunk {
            rev: self.rev,
            mesh,
            far: None,
            lo: Vec3::new(-100.0, -100.0, zlo),
            hi: Vec3::new(size + 100.0, size + 100.0, zhi),
        };
        match self.tram_chunk {
            Some(ci) => self.scene.chunks[ci] = chunk,
            None => {
                self.scene.chunks.push(chunk);
                self.tram_chunk = Some(self.scene.chunks.len() - 1);
            }
        }
    }

    /// Fire engines, police cars, ambulances, flames and smoke at the visual hour of today.
    pub fn set_emergency(&mut self, sim: &Sim, hour: f32) {
        use us_render::emergency::{EmBlaze, EmVehicle, RigKind};
        let now = sim.day as f64 * 86_400.0 + hour as f64 * 3600.0;
        let vehicles = sim
            .vehicles_at(now)
            .into_iter()
            .map(|v| EmVehicle {
                kind: match v.kind {
                    us_sim::VehicleKind::FireEngine => RigKind::FireEngine,
                    us_sim::VehicleKind::PoliceCar => RigKind::PoliceCar,
                    us_sim::VehicleKind::Ambulance => RigKind::Ambulance,
                },
                pos: [v.pos.x as f32, v.pos.y as f32],
                heading: v.heading as f32,
                moving: v.moving,
            })
            .collect();
        let blazes = sim
            .blazes_at(now)
            .into_iter()
            .map(|b| {
                let l = &sim.lots()[b.lot];
                EmBlaze {
                    pos: [b.pos.x as f32, b.pos.y as f32],
                    stage: b.stage,
                    top: 4.0 + 3.0 * (l.level.clamp(1, 5) as f32),
                }
            })
            .collect();
        self.scene.set_emergency(vehicles, blazes);
    }

    /// Follow road changes: new class or new streets show as roadworks that finish over time.
    fn sync_roads(&mut self, sim: &Sim) {
        let day = sim.day;
        let runs = sim.road_runs();
        // streets the town is building: drawn as roadworks at the simulation's own progress, and
        // redrawn when the progress moves a step or the street opens
        let same_len = self.build_step.len() == runs.len();
        let mut changed: Vec<Group> = Vec::new();
        let mut steps = Vec::with_capacity(runs.len());
        self.build_prog.resize(runs.len(), None);
        for (i, run) in runs.iter().enumerate() {
            let (progress, step) = Self::road_draw_state(sim, i);
            self.build_prog[i] = progress;
            if !same_len || self.build_step[i] != step {
                changed.push(run.addition);
            }
            steps.push(step);
        }
        self.build_step = steps;
        if !changed.is_empty() {
            self.geo_dirty = true;
            for g in changed {
                self.mark_roads(g);
            }
        }
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
            for (ri, r) in runs.iter().enumerate() {
                let probe = r.points.windows(2).next().map(|w| w[0].lerp(w[1], 0.5));
                let was = probe.and_then(|p| old.at(p));
                let w = match was {
                    Some((c, w)) if c == r.class => w,
                    Some(_) | None => {
                        if r.addition.is_none_or(|a| sim.is_revealed(a))
                            && !prev.is_empty()
                            && sim.road_visible(ri)
                        {
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
        // Opening notifications may still be waiting when a scene is built from a save.
        if self.road_chunk.contains_key(&Some(addition)) {
            return;
        }
        let town = self.town.clone();
        let a = &town.additions[addition];
        let mesh = zone_ground_on(&self.land, &town, addition);
        let (zlo, zhi) = self.z_range(2.0);
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in &a.polygon {
            lo = lo.min(Vec3::new(p.x as f32 - 30.0, p.y as f32 - 30.0, zlo));
            hi = hi.max(Vec3::new(p.x as f32 + 30.0, p.y as f32 + 30.0, zhi));
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
        if a.zone == Zone::Park {
            let cover =
                us_render::cover::build(&town, &self.land, us_render::cover::Group::Park(addition));
            self.scene.chunks.extend(cover);
        }
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
            // New streets show their construction stages from the simulation's own street
            // states (`road_build_state`), street by street; no separate timer here.
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
            if sim.is_planned_lot(i) {
                self.mark(cell_of(l.pos.x, l.pos.y));
            }
            self.look.push(Look::default());
            self.objs.push(None);
            self.parked.push(true);
        }
        self.sync_roads(sim);
        self.refresh_open(sim);
        let day = sim.day;
        for (i, l) in sim.lots().iter().enumerate() {
            let k = look_of(l, day, l.fire.is_some() && sim.is_burning(i));
            if k != self.look[i] {
                let was = self.look[i];
                self.look[i] = k;
                let c = cell_of(l.pos.x, l.pos.y);
                if self.dirty.insert(c) {
                    self.queue.push_back(c);
                }
                if (was.state == 2) != (k.state == 2) {
                    self.refresh_plazas(sim, c);
                    self.refresh_path(sim, i);
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
        if !self.geo_dirty && self.traffic_day != Some(sim.day) {
            self.refresh_traffic(sim);
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
            let (hx, hy) = (h.x as f32, h.y as f32);
            let sit = (!self.land.is_flat()).then(|| Sit {
                land: &self.land,
                z: self.land.sit_height(hx, hy),
            });
            self.hall_obj
                .emit_on(&mut mesh, &mut next_id, hx, hy, self.hall_theta, 1.0, sit);
        }
        for (obj, x, y, th, c) in &self.stations {
            if *c != cell {
                continue;
            }
            let sit = (!self.land.is_flat()).then(|| Sit {
                land: &self.land,
                z: self.land.sit_height(*x, *y),
            });
            obj.emit_on(&mut mesh, &mut next_id, *x, *y, *th, 1.0, sit);
        }
        if let Some(ids) = self.cell_lots.get(&cell) {
            for &i in ids {
                let k = self.look[i];
                let l = &sim.lots()[i];
                if k.state == 0 {
                    if l.phase == Phase::Vacant && sim.is_planned_lot(i) {
                        let colour = match l.kind {
                            LotKind::House | LotKind::Apartments => [92, 169, 111],
                            LotKind::Shop | LotKind::Office => [76, 143, 185],
                            _ => [206, 160, 76],
                        };
                        let (s, c) = (l.facing as f32).sin_cos();
                        let half = l.radius as f32 * 0.67;
                        let pad = [[-half, -half], [half, -half], [half, half], [-half, half]].map(
                            |[a, b]| {
                                let x = l.pos.x as f32 + c * a - s * b;
                                let y = l.pos.y as f32 + s * a + c * b;
                                [x, y, self.ground_at(x, y) + 0.42]
                            },
                        );
                        mesh.push_poly(&pad, colour, true, false, us_render::ID_NONE);
                    }
                    continue;
                }
                let r = l.radius as u16;
                if !matches!(&self.objs[i], Some((lk, lr, _)) if *lk == k && *lr == r) {
                    let density = sim.lot_density(i) as f32;
                    self.objs[i] = Some((
                        k,
                        r,
                        look_obj(
                            self.town.preset,
                            seed,
                            i,
                            l,
                            k,
                            density,
                            (&self.scape, &self.town),
                        ),
                    ));
                }
                let draw = &self.objs[i].as_ref().unwrap().2;
                let (x, y, th) = (l.pos.x as f32, l.pos.y as f32, l.facing as f32);
                let hills = !self.land.is_flat();
                let sit = hills.then(|| Sit {
                    land: &self.land,
                    z: self.land.sit_height(x, y),
                });
                for ground in [&draw.pad, &draw.yard].into_iter().flatten() {
                    let sit = hills.then(|| Sit {
                        land: &self.land,
                        z: self.land.sit_height(x, y),
                    });
                    ground.emit_on(&mut mesh, &mut next_id, x, y, th, 1.0, sit);
                }
                draw.building
                    .emit_on(&mut mesh, &mut next_id, x, y, th, 1.0, sit);
                let (st, ct) = (th.sin(), th.cos());
                for (lx, ly, tree) in &draw.trees {
                    let (tx, ty) = (x + ct * lx - st * ly, y + st * lx + ct * ly);
                    let sit = hills.then(|| Sit {
                        land: &self.land,
                        z: self.land.height(tx, ty),
                    });
                    tree.emit_on(&mut mesh, &mut next_id, tx, ty, 0.0, 1.0, sit);
                }
                if next_id > base + IDS_PER_CELL - 200 {
                    next_id = base;
                }
            }
        }
        for &i in self.path_cells.get(&cell).into_iter().flatten() {
            for p in self
                .paths
                .get(&i)
                .into_iter()
                .flatten()
                .filter(|p| p.cell == cell)
            {
                let sit = (!self.land.is_flat()).then_some(Sit {
                    land: &self.land,
                    z: p.floor,
                });
                p.obj
                    .emit_on(&mut mesh, &mut next_id, p.x, p.y, 0.0, 1.0, sit);
                if next_id > base + IDS_PER_CELL - 200 {
                    next_id = base;
                }
            }
        }
        let mut shown = 0;
        if let Some(list) = self.decos.get(&cell) {
            for d in list.iter().filter(|d| self.deco_shown(sim, d)) {
                let sit = (!self.land.is_flat()).then_some(Sit {
                    land: &self.land,
                    z: d.floor,
                });
                d.obj
                    .emit_on(&mut mesh, &mut next_id, d.x, d.y, d.turn, 1.0, sit);
                shown += 1;
                if next_id > base + IDS_PER_CELL - 200 {
                    next_id = base;
                }
            }
        }
        self.deco_drawn.insert(cell, shown);
        let lo = Vec3::new(
            cell.0 as f32 * CELL - 60.0,
            cell.1 as f32 * CELL - 60.0,
            self.z_range(70.0).0,
        );
        let hi = Vec3::new(
            (cell.0 + 1) as f32 * CELL + 60.0,
            (cell.1 + 1) as f32 * CELL + 60.0,
            self.z_range(70.0).1,
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

/// How many metres of the land the coarse ground skips per vertex (every fourth grid line).
const COARSE_STRIDE: usize = 4;
/// Lower the coarse vertices to leave clearance under roads and lots on gentle terrain.
const COARSE_SINK: f32 = 0.5;

/// The land on every fourth grid line, lowered a little, for the far form of the ground.
fn coarse_land(land: &Land, town: &Town) -> Land {
    let n = (land.n() - 1) / COARSE_STRIDE + 1;
    let size = land.size();
    let step = size / (n - 1) as f32;
    let mut data = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let (x, y) = (i as f32 * step, j as f32 * step);
            data.push(land.height(x, y) - COARSE_SINK);
        }
    }
    Land::from_grid(n, size, data, town.water.clone()).with_rock(land.rock())
}

/// The coarse mesh can bridge a dip and cover its road. Keep the original surface where the
/// height error approaches the pavement offset, including across a coarse triangle's tile edge.
fn ground_tiles_needing_exact_height(fine: &Land, coarse: &Land) -> HashSet<Cell> {
    let mut exact = HashSet::new();
    let pad = coarse.step() as f64;
    let clearance = us_render::ground::layer::ROAD * 0.5;
    for j in 0..fine.n() {
        for i in 0..fine.n() {
            let (x, y) = (i as f32 * fine.step(), j as f32 * fine.step());
            if coarse.height(x, y) - fine.height(x, y) <= clearance {
                continue;
            }
            let (lo_x, lo_y) = cell_of(f64::from(x) - pad, f64::from(y) - pad);
            let (hi_x, hi_y) = cell_of(f64::from(x) + pad, f64::from(y) + pad);
            for cx in lo_x..=hi_x {
                for cy in lo_y..=hi_y {
                    exact.insert((cx, cy));
                }
            }
        }
    }
    exact
}

/// Cut the ground into 256 m tiles by where each face lies; the faces that reach past the map
/// edge go into one more chunk. Each tile carries the matching faces of the coarse ground.
fn ground_tiles(near: &Mesh, far: &Mesh, size: f32, exact: &HashSet<Cell>) -> Vec<Chunk> {
    let tiles = (size / CELL).ceil().max(1.0) as i32;
    let split = |m: &Mesh| -> HashMap<Cell, Mesh> {
        let mut out: HashMap<Cell, (Mesh, HashMap<u32, u32>)> = HashMap::new();
        let inside = |v: &us_render::Vertex| {
            v.pos[0] >= -0.01
                && v.pos[1] >= -0.01
                && v.pos[0] <= size + 0.01
                && v.pos[1] <= size + 0.01
        };
        for tri in m.plain.chunks_exact(3).chain(m.cast.chunks_exact(3)) {
            let vs = [
                &m.vertices[tri[0] as usize],
                &m.vertices[tri[1] as usize],
                &m.vertices[tri[2] as usize],
            ];
            let key = if vs.iter().all(|v| inside(v)) {
                let cx = (vs[0].pos[0] + vs[1].pos[0] + vs[2].pos[0]) / 3.0;
                let cy = (vs[0].pos[1] + vs[1].pos[1] + vs[2].pos[1]) / 3.0;
                (
                    ((cx / CELL).floor() as i32).clamp(0, tiles - 1),
                    ((cy / CELL).floor() as i32).clamp(0, tiles - 1),
                )
            } else {
                (-1, -1)
            };
            let (dst, remap) = out.entry(key).or_default();
            for &old in tri {
                let new = *remap.entry(old).or_insert_with(|| {
                    dst.vertices.push(m.vertices[old as usize]);
                    dst.vertices.len() as u32 - 1
                });
                dst.plain.push(new);
            }
        }
        out.into_iter().map(|(k, (m, _))| (k, m)).collect()
    };
    let mut near = split(near);
    let mut far = split(far);
    let mut keys: Vec<Cell> = near.keys().copied().collect();
    keys.sort();
    keys.into_iter()
        .map(|k| {
            let mesh = near.remove(&k).unwrap_or_default();
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for v in &mesh.vertices {
                let p = Vec3::from(v.pos);
                lo = lo.min(p);
                hi = hi.max(p);
            }
            // the apron chunk has no coarse form and is never culled by size
            let far = (k.0 >= 0 && !exact.contains(&k))
                .then(|| far.remove(&k))
                .flatten();
            Chunk {
                rev: 1,
                mesh,
                far,
                lo,
                hi,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn ground_tiles_keep_every_face() {
        use us_render::Vertex;
        let v = |x: f32, y: f32| Vertex {
            pos: [x, y, 0.0],
            normal: [0.0, 0.0, 1.0],
            color: [0, 255, 0, 255],
            id: 0,
        };
        let mut m = Mesh::default();
        // one face in tile (0, 0), one in tile (1, 1), one past the map edge
        for (x, y) in [(10.0, 10.0), (300.0, 300.0), (-50.0, 10.0)] {
            let b = m.vertices.len() as u32;
            m.vertices.extend([v(x, y), v(x + 5.0, y), v(x, y + 5.0)]);
            m.plain.extend([b, b + 1, b + 2]);
        }
        let chunks = ground_tiles(&m, &m, 512.0, &HashSet::new());
        assert_eq!(chunks.len(), 3);
        let total: usize = chunks.iter().map(|c| c.mesh.triangles()).sum();
        assert_eq!(total, 3);
        assert_eq!(chunks.iter().filter(|c| c.far.is_none()).count(), 1);
    }

    use super::*;
    use us_gen::{Preset, generate};
    use us_sim::{CivicKind, Phase, hall_sites};

    #[test]
    fn coarse_terrain_cannot_cover_roads_in_a_dip() {
        let town = generate(Preset::Mesa, 1);
        let land = Land::from_town(&town);
        let coarse = coarse_land(&land, &town);
        let exact = ground_tiles_needing_exact_height(&land, &coarse);
        let mut affected = 0;
        for road in &town.roads {
            let bed = land.bed(&road.points);
            for (i, p) in bed.pts.iter().enumerate() {
                let (x, y) = (p.x as f32, p.y as f32);
                if coarse.height(x, y) > bed.z(&land, i, x, y) + us_render::ground::layer::ROAD {
                    affected += 1;
                    assert!(
                        exact.contains(&cell_of(p.x, p.y)),
                        "road can sink below terrain at {p:?}"
                    );
                }
            }
        }
        assert!(
            affected > 0,
            "this seed contains the original height conflict"
        );
        assert!(
            exact.len() < 256,
            "there are still safe tiles for distant terrain"
        );
    }

    #[test]
    fn reopening_city_planning_preserves_vacant_zone_markers() {
        let town = Arc::new(generate(Preset::Grid, 2));
        let hall = hall_sites(&town)[0];
        let rules = us_sim::Rules::sandbox_unlimited().with_city_planning(true);
        let mut sim = Sim::new_with_rules(&town, hall, 2, 0, rules);
        let mut live = LiveScene::new(town.clone(), &sim, true);
        let a = Pt {
            x: hall.x - 330.0,
            y: hall.y - 330.0,
        };
        let b = Pt {
            x: hall.x + 330.0,
            y: hall.y + 330.0,
        };
        let count = sim
            .paint_zone(us_sim::PlanningUse::Residential, a, b)
            .unwrap();
        let marker_vertices = |scene: &LiveScene| {
            scene
                .scene
                .chunks
                .iter()
                .flat_map(|c| &c.mesh.vertices)
                .filter(|v| v.id == us_render::ID_NONE && v.color[..3] == [92, 169, 111])
                .count()
        };
        live.sync(&mut sim, 1.0e9);
        assert_eq!(marker_vertices(&live), count * 4);
        let mut loaded = Sim::load(&sim.save()).unwrap();
        let mut live = LiveScene::new(town, &loaded, true);
        live.sync(&mut loaded, 1.0e9);
        assert_eq!(marker_vertices(&live), count * 4);
    }

    #[test]
    fn reopening_a_save_restores_neighbourhood_road_meshes() {
        let town = Arc::new(generate(Preset::Crossroads, 1));
        let hall = hall_sites(&town)[0];
        let mut sim = Sim::new(&town, hall, 1);
        for _ in 0..430 {
            sim.step_day();
        }
        let mut live = LiveScene::new(town.clone(), &sim, true);
        live.sync(&mut sim, 1.0e9);
        let expected: Vec<_> = live
            .road_chunk
            .iter()
            .filter_map(|(group, &chunk)| {
                let addition = (*group)?;
                let mesh = &live.scene.chunks[chunk].mesh;
                (mesh.triangles() > 0).then_some((addition, mesh.triangles()))
            })
            .collect();
        assert!(
            !expected.is_empty(),
            "the original town has neighbourhood roads"
        );

        let mut loaded = Sim::load(&sim.save()).expect("reopen the town");
        let mut restored = LiveScene::new(town, &loaded, true);
        restored.sync(&mut loaded, 1.0e9);
        for (addition, triangles) in expected {
            let chunk = restored
                .road_chunk
                .get(&Some(addition))
                .unwrap_or_else(|| panic!("pavement is missing for neighbourhood {addition}"));
            assert_eq!(restored.scene.chunks[*chunk].mesh.triangles(), triangles);
        }
    }

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

    /// Every new civic building places, finishes and is drawn.
    #[test]
    fn scene_draws_every_new_civic_building() {
        let town = Arc::new(generate(Preset::Grid, 3));
        let hall = hall_sites(&town)[0];
        let mut sim = Sim::new_with_rules(&town, hall, 3, 2, us_sim::Rules::sandbox_unlimited());
        sim.unlock_all();
        // a young town: the settlers fill the open ground within a few months
        for _ in 0..40 {
            sim.step_day();
        }
        let mut live = LiveScene::new(town, &sim, true);
        live.sync(&mut sim, 1.0e9);
        let mut placed = Vec::new();
        for k in CivicKind::ALL[7..].iter() {
            let spot = sim
                .lots()
                .iter()
                .filter(|l| l.phase == Phase::Vacant)
                .map(|l| l.pos)
                .find(|&p| sim.check_civic(*k, p).is_ok())
                .expect("a spot");
            placed.push(sim.place_civic(*k, spot).expect("placed"));
        }
        for _ in 0..90 {
            sim.step_day();
        }
        live.sync(&mut sim, 1.0e9);
        for i in placed {
            assert!(live.look[i].state != 0, "lot {i} is drawn");
        }
    }

    /// A finished downtown lot far from any open street gets a footpath, and lots lose it when
    /// they stop standing; every piece is short enough to belong to one chunk.
    #[test]
    fn far_lots_get_footpaths() {
        let town = Arc::new(generate(Preset::LakeTown, 3));
        let hall = hall_sites(&town)[0];
        let mut sim = Sim::new(&town, hall, 1);
        let mut live = LiveScene::new(town, &sim, true);
        for _ in 0..900 {
            sim.step_day();
            live.sync(&mut sim, 1.0e9);
        }
        assert!(!live.square_tiles.is_empty(), "the square has paving");
        assert!(!live.paths.is_empty(), "some lot is far from a street");
        for (i, pieces) in &live.paths {
            assert_eq!(live.look[*i].state, 2, "only finished lots have a path");
            for p in pieces {
                assert_eq!(p.cell, cell_of(p.x as f64, p.y as f64));
                let f = p.obj.footprint();
                assert!(f[2] - f[0] < 60.0 && f[3] - f[1] < 60.0);
            }
        }
    }

    /// A widened street shows as roadworks that finish; upgrades and fires change a lot's look.
    #[test]
    fn scene_follows_roads_and_lot_looks() {
        let town = Arc::new(generate(Preset::Grid, 1));
        let hall = hall_sites(&town)[0];
        let mut sim = Sim::new(&town, hall, 1);
        let mut live = LiveScene::new(town, &sim, true);
        let (mut works, mut site, mut done) = (false, false, false);
        // streets are always being built, so roadworks finish when a run that was under
        // construction opens
        let mut was_building: HashSet<usize> = HashSet::new();
        for _ in 0..2500 {
            sim.step_day();
            live.sync(&mut sim, 1.0e9);
            works |= live.works_place(&sim).is_some();
            for i in 0..sim.road_runs().len() {
                if sim.road_build_state(i).is_some() {
                    was_building.insert(i);
                } else if was_building.remove(&i) && sim.road_open(i) {
                    done = true;
                }
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

    /// The streets of a neighbourhood that opens are built in front of the player, not shown
    /// finished the moment it opens.
    #[test]
    fn new_neighbourhood_streets_start_as_roadworks() {
        let town = Arc::new(generate(Preset::Grid, 1));
        let hall = hall_sites(&town)[0];
        let mut sim = Sim::new(&town, hall, 1);
        let mut live = LiveScene::new(town.clone(), &sim, true);
        let count = |sim: &Sim| {
            (0..town.additions.len())
                .filter(|&a| sim.is_revealed(a))
                .count()
        };
        let mut seen = count(&sim);
        let mut checked = 0;
        for _ in 0..2500 {
            sim.step_day();
            let before = seen;
            seen = count(&sim);
            live.sync(&mut sim, 1.0e9);
            if seen > before {
                let day = sim.day;
                let fresh: Vec<usize> = sim
                    .road_runs()
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| {
                        r.addition
                            .is_some_and(|a| sim.is_revealed(a) && !live.scene.chunks.is_empty())
                    })
                    .map(|(i, _)| i)
                    .filter(|&i| live.works_progress(i, day).is_some())
                    .collect();
                assert!(
                    !fresh.is_empty(),
                    "a new neighbourhood's streets are under construction"
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "some neighbourhood opened during the run");
    }
}
