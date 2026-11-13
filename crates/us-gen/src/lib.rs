//! Urban Sprawl town generation: road skeleton, additions, street patterns and lots.
//!
//! `generate(preset, seed)` is deterministic on every platform: seeded SplitMix64 RNG, `libm`
//! for transcendental functions, no hash-order dependence in any output.

mod geom;
mod index;
mod lots;
mod patterns;
mod plan;
mod polygonize;
mod rng;
mod skeleton;
mod world;

pub use geom::{Pt, pt};
pub use world::{HALL, HWY_Y, S as MAP_SIZE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    Grid,
    Winding,
}

impl Preset {
    pub fn name(self) -> &'static str {
        match self {
            Preset::Grid => "grid",
            Preset::Winding => "winding",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoadClass {
    Highway,
    Arterial,
    Collector,
    Local,
    Alley,
    Rail,
}

impl RoadClass {
    /// Half the paved width in metres.
    pub fn half_width(self) -> f64 {
        match self {
            RoadClass::Highway => 12.0,
            RoadClass::Arterial => 9.0,
            RoadClass::Collector => 6.0,
            RoadClass::Local => 4.0,
            RoadClass::Alley => 2.5,
            RoadClass::Rail => 3.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            RoadClass::Highway => "highway",
            RoadClass::Arterial => "arterial",
            RoadClass::Collector => "collector",
            RoadClass::Local => "local",
            RoadClass::Alley => "alley",
            RoadClass::Rail => "rail",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Core,
    Inner,
    Outer,
    Industry,
    Mall,
    Park,
    Campus,
    Farm,
}

impl Zone {
    pub fn name(self) -> &'static str {
        match self {
            Zone::Core => "core",
            Zone::Inner => "inner",
            Zone::Outer => "outer",
            Zone::Industry => "industry",
            Zone::Mall => "mall",
            Zone::Park => "park",
            Zone::Campus => "campus",
            Zone::Farm => "farm",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pattern {
    Farmland,
    Mall,
    Industry,
    OldTownGrid,
    CrookedGrid,
    RailGrid,
    CatalogueGrid,
    LongBlockGrid,
    Loops,
    Pods,
    Campus,
    Golf,
    Cemetery,
    Park,
}

impl Pattern {
    pub fn name(self) -> &'static str {
        match self {
            Pattern::Farmland => "farmland",
            Pattern::Mall => "mall",
            Pattern::Industry => "industry",
            Pattern::OldTownGrid => "old-town-grid",
            Pattern::CrookedGrid => "crooked-grid",
            Pattern::RailGrid => "rail-grid",
            Pattern::CatalogueGrid => "catalogue-grid",
            Pattern::LongBlockGrid => "long-block-grid",
            Pattern::Loops => "loops",
            Pattern::Pods => "pods",
            Pattern::Campus => "campus",
            Pattern::Golf => "golf",
            Pattern::Cemetery => "cemetery",
            Pattern::Park => "park",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LotKind {
    House,
    Row,
    Shop,
    Apartments,
    Office,
    Gas,
    Workshop,
    Church,
    School,
    Fire,
    Water,
    Parking,
    Park,
    Sports,
    Strip,
    BigBox,
    Warehouse,
    FastFood,
    Farm,
}

impl LotKind {
    /// Footprint radius in metres before any downtown squeeze.
    pub fn nominal_radius(self) -> f64 {
        match self {
            LotKind::House => 8.0,
            LotKind::Row => 9.5,
            LotKind::Shop => 8.5,
            LotKind::Apartments => 11.5,
            LotKind::Office => 11.5,
            LotKind::Gas => 13.5,
            LotKind::Workshop => 16.0,
            LotKind::Church => 11.5,
            LotKind::School => 16.0,
            LotKind::Fire => 12.0,
            LotKind::Water => 6.0,
            LotKind::Parking => 10.0,
            LotKind::Park => 13.0,
            LotKind::Sports => 14.0,
            LotKind::Strip => 20.0,
            LotKind::BigBox => 42.0,
            LotKind::Warehouse => 30.0,
            LotKind::FastFood => 10.0,
            LotKind::Farm => 14.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            LotKind::House => "house",
            LotKind::Row => "row",
            LotKind::Shop => "shop",
            LotKind::Apartments => "apartments",
            LotKind::Office => "office",
            LotKind::Gas => "gas",
            LotKind::Workshop => "workshop",
            LotKind::Church => "church",
            LotKind::School => "school",
            LotKind::Fire => "fire",
            LotKind::Water => "water",
            LotKind::Parking => "parking",
            LotKind::Park => "park",
            LotKind::Sports => "sports",
            LotKind::Strip => "strip",
            LotKind::BigBox => "bigbox",
            LotKind::Warehouse => "warehouse",
            LotKind::FastFood => "fastfood",
            LotKind::Farm => "farm",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Road {
    pub class: RoadClass,
    pub points: Vec<Pt>,
}

/// One piece of land laid out at once.
#[derive(Clone, Debug)]
pub struct Addition {
    pub polygon: Vec<Pt>,
    pub zone: Zone,
    pub pattern: Pattern,
    /// Distance of its centroid from the town hall, metres.
    pub distance: f64,
    /// Where a pod's entrance streets meet a bigger road (empty for other patterns).
    pub entrances: Vec<Pt>,
}

#[derive(Clone, Copy, Debug)]
pub struct CulDeSac {
    /// Centre of the turning bulb at the dead end.
    pub pos: Pt,
    /// Index into `Town::additions`.
    pub addition: usize,
}

#[derive(Clone, Debug)]
pub struct Lot {
    pub centre: Pt,
    /// Collision radius in metres: lots never overlap each other, roads or water by this radius.
    pub radius: f64,
    pub kind: LotKind,
    /// Direction the front faces, radians.
    pub facing: f64,
    pub zone: Zone,
    /// Stable per-lot random roll in [0, 1) for visual variants.
    pub variant: f64,
}

#[derive(Clone, Debug)]
pub struct Town {
    pub preset: Preset,
    pub seed: u64,
    /// The map is the square [0, size] x [0, size]. Nothing but the highway, arterials and the
    /// railroad reaches its edge.
    pub size: f64,
    /// y of the highway, which runs the full width 70 m inside the south edge. Land south of it is
    /// outside the generated town.
    pub highway_y: f64,
    pub roads: Vec<Road>,
    pub cul_de_sacs: Vec<CulDeSac>,
    pub additions: Vec<Addition>,
    pub lots: Vec<Lot>,
    /// River band polygons (the water itself).
    pub water: Vec<Vec<Pt>>,
    pub hall: Pt,
    pub hall_radius: f64,
}

impl Town {
    /// Canonical byte form of the whole town: the same (preset, seed) gives the same bytes anywhere.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut b = Vec::new();
        let f = |b: &mut Vec<u8>, v: f64| b.extend_from_slice(&v.to_bits().to_le_bytes());
        let u = |b: &mut Vec<u8>, v: u64| b.extend_from_slice(&v.to_le_bytes());
        let pts = |b: &mut Vec<u8>, ps: &[Pt]| {
            b.extend_from_slice(&(ps.len() as u64).to_le_bytes());
            for p in ps {
                b.extend_from_slice(&p.x.to_bits().to_le_bytes());
                b.extend_from_slice(&p.y.to_bits().to_le_bytes());
            }
        };
        u(&mut b, self.preset as u64);
        u(&mut b, self.seed);
        f(&mut b, self.size);
        f(&mut b, self.highway_y);
        u(&mut b, self.roads.len() as u64);
        for r in &self.roads {
            u(&mut b, r.class as u64);
            pts(&mut b, &r.points);
        }
        u(&mut b, self.cul_de_sacs.len() as u64);
        for c in &self.cul_de_sacs {
            pts(&mut b, &[c.pos]);
            u(&mut b, c.addition as u64);
        }
        u(&mut b, self.additions.len() as u64);
        for a in &self.additions {
            pts(&mut b, &a.polygon);
            u(&mut b, a.zone as u64);
            u(&mut b, a.pattern as u64);
            f(&mut b, a.distance);
            pts(&mut b, &a.entrances);
        }
        u(&mut b, self.lots.len() as u64);
        for l in &self.lots {
            pts(&mut b, &[l.centre]);
            f(&mut b, l.radius);
            u(&mut b, l.kind as u64);
            f(&mut b, l.facing);
            u(&mut b, l.zone as u64);
            f(&mut b, l.variant);
        }
        u(&mut b, self.water.len() as u64);
        for w in &self.water {
            pts(&mut b, w);
        }
        pts(&mut b, &[self.hall]);
        f(&mut b, self.hall_radius);
        b
    }

    /// 64-bit FNV-1a of the canonical bytes.
    pub fn fingerprint(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self.canonical_bytes() {
            h ^= byte as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
}

/// Generate a 4096 m town. Same `(preset, seed)` gives the same town on every platform.
pub fn generate(preset: Preset, seed: u64) -> Town {
    plan::generate(preset, seed)
}
