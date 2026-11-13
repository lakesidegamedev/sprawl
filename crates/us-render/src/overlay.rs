//! Map views: each lot and each road gets one flat, stepped colour from a fixed palette, and
//! everything else is shown desaturated. Palettes are lists of solid colours, never gradients.

use crate::ground::Ground;
use crate::levels::{self, massing};
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb};
use crate::rng::Rng;
use std::collections::BTreeMap;
use us_gen::{LotKind, RoadClass};

/// First outline id used by overlay blocks.
pub const OVERLAY_ID_BASE: u32 = 11_000_000;

/// Which map view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayKind {
    /// Lot value 0 (cheap) to 1 (dear).
    LandValue,
    /// Lot value is the building level over 5: 0.2 is level 1, 1.0 is level 5.
    BuildingLevel,
    /// Lot value is a category id, see [`use_category`].
    Use,
    /// Lot value 0 (no service) to 1 (fully served).
    ServiceCoverage,
    /// Lot value 0 (empty, no demand) through 0.5 (balanced) to 1 (crowded, high demand).
    Demand,
    /// Road value 0 (empty) to 1 (jammed); lots stay neutral.
    Traffic,
}

const LAND_VALUE: [Rgb; 6] = [
    [64, 92, 168],
    [72, 150, 196],
    [112, 190, 150],
    [226, 214, 96],
    [238, 150, 64],
    [206, 56, 56],
];
const LEVEL: [Rgb; 5] = [
    [214, 232, 150],
    [120, 200, 150],
    [64, 170, 182],
    [64, 110, 196],
    [92, 56, 160],
];
/// Residential, commercial, industrial, civic, open land, farm, other.
const USE: [Rgb; 7] = [
    [92, 190, 110],
    [64, 128, 214],
    [236, 176, 60],
    [150, 96, 200],
    [60, 160, 150],
    [190, 170, 100],
    [160, 160, 168],
];
const COVERAGE: [Rgb; 5] = [
    [190, 60, 60],
    [226, 140, 60],
    [232, 214, 90],
    [140, 200, 100],
    [56, 164, 96],
];
const DEMAND: [Rgb; 7] = [
    [52, 84, 176],
    [92, 140, 208],
    [156, 190, 226],
    [204, 204, 208],
    [240, 196, 150],
    [232, 140, 70],
    [200, 60, 44],
];
const TRAFFIC: [Rgb; 5] = [
    [64, 184, 92],
    [150, 204, 72],
    [238, 214, 60],
    [238, 140, 52],
    [206, 44, 44],
];

impl OverlayKind {
    /// The fixed palette of this view, lowest value first.
    pub fn palette(self) -> &'static [Rgb] {
        match self {
            OverlayKind::LandValue => &LAND_VALUE,
            OverlayKind::BuildingLevel => &LEVEL,
            OverlayKind::Use => &USE,
            OverlayKind::ServiceCoverage => &COVERAGE,
            OverlayKind::Demand => &DEMAND,
            OverlayKind::Traffic => &TRAFFIC,
        }
    }

    /// The step colour for a value (a 0 to 1 value, or a category id for `Use`).
    pub fn colour(self, v: f32) -> Rgb {
        let p = self.palette();
        let i = if self == OverlayKind::Use {
            (v.max(0.0) as usize).min(p.len() - 1)
        } else {
            ((v.clamp(0.0, 1.0) * p.len() as f32) as usize).min(p.len() - 1)
        };
        p[i]
    }
}

/// Category id of a lot kind for the `Use` view.
pub fn use_category(kind: LotKind) -> u8 {
    match kind {
        LotKind::House | LotKind::Row | LotKind::Apartments => 0,
        LotKind::Shop
        | LotKind::Office
        | LotKind::Gas
        | LotKind::Strip
        | LotKind::BigBox
        | LotKind::FastFood => 1,
        LotKind::Workshop | LotKind::Warehouse => 2,
        LotKind::Church | LotKind::School | LotKind::Fire | LotKind::Water => 3,
        LotKind::Park | LotKind::Sports => 4,
        LotKind::Farm => 5,
        LotKind::Parking => 6,
    }
}

/// One map view's values. `lots` and `roads` are indexed like the scene's lot and road lists;
/// a missing entry (list too short) is drawn neutral.
#[derive(Clone, Debug)]
pub struct Overlay {
    pub kind: OverlayKind,
    pub lots: Vec<f32>,
    pub roads: Vec<f32>,
}

/// A lot as the overlay and the smoke need to know it.
#[derive(Clone, Copy, Debug)]
pub struct LotInfo {
    pub x: f32,
    pub y: f32,
    pub theta: f32,
    pub kind: LotKind,
    /// Building level 1 to 5.
    pub level: u8,
}

pub const NEUTRAL: Rgb = [176, 176, 182];

/// Mark every vertex of the mesh from `from` on as "keep my colour in a map view".
pub fn keep_colour(m: &mut Mesh, from: usize) {
    for v in &mut m.vertices[from..] {
        v.color[3] = if v.color[3] == 0 { 1 } else { 254 };
    }
}

fn shade(c: Rgb, k: f32) -> Rgb {
    [
        (c[0] as f32 * k) as u8,
        (c[1] as f32 * k) as u8,
        (c[2] as f32 * k) as u8,
    ]
}

/// The mesh drawn over the town in a map view: a flat tile under each lot, a block in the lot's
/// colour standing where its building is, and coloured roads for the traffic view.
///
/// `mass` is the real body of each lot's building (see [`levels::massing`]); a lot with no entry
/// uses the standard model of its kind and level. `hall` is the town hall's place and body, drawn
/// as a neutral block.
pub fn build_mesh(
    lots: &[LotInfo],
    mass: &[[f32; 5]],
    hall: Option<(f32, f32, f32, [f32; 5])>,
    roads: &[(RoadClass, &[us_gen::Pt])],
    ov: &Overlay,
) -> Mesh {
    let mut mesh = Mesh::default();
    let mut cache: BTreeMap<(u8, u8), [f32; 5]> = BTreeMap::new();
    let mut g = Ground::default();
    // roads first (underneath the lot tiles are not needed: lots never overlap roads)
    if ov.kind == OverlayKind::Traffic || !ov.roads.is_empty() {
        let mut order: Vec<usize> = (0..roads.len())
            .filter(|&i| roads[i].0 != RoadClass::Rail)
            .collect();
        order.sort_by_key(|&i| (roads[i].0.half_width() as i32, i));
        // all dark edges first at one height, then each class of surface a step higher
        for &i in &order {
            let (c, pts) = roads[i];
            g.ribbon(pts, c.half_width() as f32 + 0.45, 0.0, 0.44, [34, 36, 44]);
        }
        for &i in &order {
            let (c, pts) = roads[i];
            let col = ov.roads.get(i).map_or(NEUTRAL, |v| ov.kind.colour(*v));
            g.ribbon(
                pts,
                c.half_width() as f32,
                0.0,
                0.46 + 0.004 * c.half_width() as f32,
                col,
            );
        }
    }
    let ground_end = g.mesh.vertices.len();
    mesh.vertices.append(&mut g.mesh.vertices);
    mesh.plain.append(&mut g.mesh.plain);
    keep_colour(&mut mesh, 0);
    let _ = ground_end;
    for (i, l) in lots.iter().enumerate() {
        let Some(v) = ov.lots.get(i) else { continue };
        let col = ov.kind.colour(*v);
        let m = match mass.get(i) {
            Some(m) => *m,
            None => *cache.entry((l.kind as u8, l.level)).or_insert_with(|| {
                let o = levels::build_lot_level(l.kind, l.level, &mut Rng::new(7, 7));
                massing(&o)
            }),
        };
        let (x0, y0, x1, y1, h) = (m[0] - 0.5, m[1] - 0.5, m[2] + 0.5, m[3] + 0.5, m[4]);
        let from = mesh.vertices.len();
        let mut o = Obj::new();
        // the ground tile under the lot, then the block on top of it
        o.slab(
            x0 - 2.0,
            y0 - 2.0,
            0.34,
            x1 + 2.0,
            y1 + 2.0,
            0.40,
            shade(col, 0.82),
        );
        if h >= 1.0 {
            o.bx_top(x0, y0, 0.40, x1, y1, h + 0.5, shade(col, 0.9), col);
        } else {
            o.slab(x0, y0, 0.40, x1, y1, 0.55, col);
        }
        let mut id = OVERLAY_ID_BASE + i as u32 * 16;
        o.emit(&mut mesh, &mut id, l.x, l.y, l.theta, 1.0);
        keep_colour(&mut mesh, from);
    }
    if let Some((x, y, theta, m)) = hall {
        let from = mesh.vertices.len();
        let mut o = Obj::new();
        o.bx_top(
            m[0] - 0.5,
            m[1] - 0.5,
            0.40,
            m[2] + 0.5,
            m[3] + 0.5,
            m[4] + 0.6,
            [196, 194, 190],
            [232, 230, 226],
        );
        let mut id = OVERLAY_ID_BASE + lots.len() as u32 * 16;
        o.emit(&mut mesh, &mut id, x, y, theta, 1.0);
        keep_colour(&mut mesh, from);
    }
    mesh
}
