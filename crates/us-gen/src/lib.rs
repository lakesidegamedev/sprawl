//! Urban Sprawl town generation: terrain, road skeleton, additions, street patterns and lots.
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
mod terrain;
mod world;

pub use geom::{Pt, pt};
pub use terrain::Terrain;
pub use world::{HALL, HWY_Y, S as MAP_SIZE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// Prairie railroad town: straight streets, a grid of old roads.
    Grid,
    /// River and hills town: crooked streets that follow the land.
    Winding,
    /// A one-mile arterial grid, each square filled with walled subdivision pods and commercial corners.
    Sunbelt,
    /// An old town where two highways cross: radial old roads and a compact grid.
    Crossroads,
    /// A shoreline that the streets and a lakeside drive follow.
    LakeTown,
}

impl Preset {
    pub const ALL: [Preset; 5] = [
        Preset::Grid,
        Preset::Winding,
        Preset::Sunbelt,
        Preset::Crossroads,
        Preset::LakeTown,
    ];

    /// Short lower-case name, as used on command lines.
    pub fn name(self) -> &'static str {
        match self {
            Preset::Grid => "grid",
            Preset::Winding => "winding",
            Preset::Sunbelt => "sunbelt",
            Preset::Crossroads => "crossroads",
            Preset::LakeTown => "lake",
        }
    }

    /// Name for menus and captions.
    pub fn label(self) -> &'static str {
        match self {
            Preset::Grid => "Grid",
            Preset::Winding => "Winding",
            Preset::Sunbelt => "Sunbelt",
            Preset::Crossroads => "Crossroads",
            Preset::LakeTown => "Lake town",
        }
    }

    /// One line describing the town style.
    pub fn blurb(self) -> &'static str {
        match self {
            Preset::Grid => "Prairie railroad town: straight streets, a grid of old roads.",
            Preset::Winding => "River and hills town: crooked streets that follow the land.",
            Preset::Sunbelt => {
                "Sunbelt suburb: a one-mile road grid filled with walled subdivisions and shopping corners."
            }
            Preset::Crossroads => {
                "Crossroads town: two highways cross in a compact old grid with roads fanning out."
            }
            Preset::LakeTown => "Lake town: streets and a lakeside drive follow the shore.",
        }
    }

    pub fn from_name(s: &str) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| p.name() == s)
    }

    /// The tuned generator settings of this preset.
    pub fn settings(self) -> Settings {
        Settings::for_preset(self)
    }
}

/// Knobs of the generator. `generate` uses the tuned values of each preset; the realism tuner
/// searches over these.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Distance between the big arterial roads in the grid-like presets, metres.
    pub arterial_spacing: f64,
    /// Multiplier on every grid block size outside the old core.
    pub block_scale: f64,
    /// Block depth and length of the old core grid, metres.
    pub core_depth: f64,
    pub core_len: f64,
    /// Chance that an old core grid has alleys.
    pub core_alley: f64,
    /// Chance that a catalogue grid has alleys, where its catalogue entry allows them.
    pub cat_alley: f64,
    /// Chance a cross street jogs (T-junctions instead of a four-way crossing) and the jog, metres.
    pub jog_prob: f64,
    pub jog: f64,
    /// Share of outer patches laid out as walled pods.
    pub pod_share: f64,
    /// Distance from the hall beyond which pods are built, metres.
    pub pod_from: f64,
    /// Joined edge streets at least this long become collectors, metres.
    pub collector_len: f64,
    /// Multiplier on the spacing of curving streets.
    pub street_spacing: f64,
    /// Share of curving outer patches with cul-de-sacs.
    pub cul_share: f64,
    /// Roughly how far the built-up town reaches from the hall, metres.
    pub reach: f64,
}

impl Settings {
    pub fn for_preset(p: Preset) -> Settings {
        let base = Settings {
            arterial_spacing: 805.0,
            block_scale: 1.0,
            core_depth: 80.0,
            core_len: 100.0,
            core_alley: 1.0,
            cat_alley: 1.0,
            jog_prob: 0.0,
            jog: 20.0,
            pod_share: 0.7,
            pod_from: 1500.0,
            collector_len: 800.0,
            street_spacing: 1.0,
            cul_share: 0.6,
            reach: 1900.0,
        };
        // Tuned offline against road statistics of real towns (`us-metrics tune`).
        match p {
            Preset::Grid => Settings {
                arterial_spacing: 1400.0,
                block_scale: 1.84,
                core_depth: 111.5,
                core_len: 151.4,
                core_alley: 0.15,
                cat_alley: 0.18,
                jog_prob: 0.08,
                jog: 38.9,
                pod_share: 1.0,
                collector_len: 695.5,
                street_spacing: 1.39,
                cul_share: 0.88,
                ..base
            },
            Preset::Winding => Settings {
                block_scale: 1.75,
                pod_share: 0.24,
                collector_len: 554.7,
                street_spacing: 1.6,
                cul_share: 0.47,
                ..base
            },
            Preset::Sunbelt => Settings {
                arterial_spacing: 1609.0,
                block_scale: 2.2,
                pod_share: 0.39,
                pod_from: 420.0,
                collector_len: 916.0,
                street_spacing: 1.05,
                cul_share: 0.83,
                reach: 2700.0,
                ..base
            },
            Preset::Crossroads => Settings {
                block_scale: 2.12,
                core_depth: 123.3,
                core_len: 126.3,
                core_alley: 0.32,
                cat_alley: 0.32,
                jog_prob: 0.004,
                jog: 24.3,
                pod_share: 1.0,
                collector_len: 752.5,
                street_spacing: 1.0,
                cul_share: 0.71,
                reach: 1900.0,
                ..base
            },
            Preset::LakeTown => Settings {
                block_scale: 2.19,
                core_depth: 139.6,
                core_len: 164.4,
                core_alley: 0.01,
                cat_alley: 0.12,
                jog_prob: 0.087,
                jog: 39.5,
                pod_share: 0.69,
                collector_len: 900.7,
                street_spacing: 0.99,
                cul_share: 0.5,
                reach: 1800.0,
                ..base
            },
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
    /// The ground: heights and slopes anywhere on the map.
    pub terrain: Terrain,
}

impl Town {
    /// Ground height in metres at a point.
    pub fn height_at(&self, p: Pt) -> f64 {
        self.terrain.height_at(p)
    }

    /// Steepness (rise over run) at a point.
    pub fn slope_at(&self, p: Pt) -> f64 {
        self.terrain.slope(p)
    }

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
        for v in self.terrain.heightfield(17) {
            b.extend_from_slice(&v.to_bits().to_le_bytes());
        }
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
    plan::generate(preset, seed, &preset.settings())
}

/// Like `generate`, with explicit settings (used by the realism tuner).
pub fn generate_with(preset: Preset, seed: u64, settings: &Settings) -> Town {
    plan::generate(preset, seed, settings)
}
