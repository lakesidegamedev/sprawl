//! Real towns from OpenStreetMap Overpass JSON (gzip): classes, projection, clipping.
//!
//! Data (c) OpenStreetMap contributors, ODbL. Used only for statistics, never shipped.

use crate::graph::{self, Graph};
use crate::metrics::{self, Metrics, WINDOW};
use serde_json::Value;
use std::io::Read;
use std::path::Path;
use us_gen::{Pt, RoadClass, pt};

const M_PER_DEG: f64 = 111_194.9;

pub struct Town {
    pub name: String,
    pub pop: u64,
    pub lines: Vec<(RoadClass, Vec<Pt>)>,
}

/// OSM `highway` (and `service`) tags to a road class. `None` means ignored.
pub fn classify(highway: &str, service: Option<&str>) -> Option<RoadClass> {
    match highway {
        "motorway" | "trunk" | "motorway_link" | "trunk_link" => Some(RoadClass::Highway),
        "primary" | "secondary" | "primary_link" | "secondary_link" => Some(RoadClass::Arterial),
        "tertiary" | "tertiary_link" => Some(RoadClass::Collector),
        "residential" | "unclassified" | "living_street" => Some(RoadClass::Local),
        "service" if service == Some("alley") => Some(RoadClass::Alley),
        _ => None,
    }
}

/// Liang-Barsky clip of a segment to the window; returns the clipped segment.
fn clip_seg(a: Pt, b: Pt) -> Option<(Pt, Pt)> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    let d = b - a;
    for (p, q) in [
        (-d.x, a.x),
        (d.x, WINDOW - a.x),
        (-d.y, a.y),
        (d.y, WINDOW - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 {
                    return None;
                }
                t0 = t0.max(r);
            } else {
                if r < t0 {
                    return None;
                }
                t1 = t1.min(r);
            }
        }
    }
    Some((a.lerp(b, t0), a.lerp(b, t1)))
}

/// Clip a polyline to the window, returning the pieces inside.
pub fn clip_line(line: &[Pt]) -> Vec<Vec<Pt>> {
    let mut out: Vec<Vec<Pt>> = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    for w in line.windows(2) {
        match clip_seg(w[0], w[1]) {
            Some((p, q)) => {
                if cur.last().is_none_or(|l| l.dist(p) > 1e-6) {
                    if cur.len() >= 2 {
                        out.push(std::mem::take(&mut cur));
                    }
                    cur.clear();
                    cur.push(p);
                }
                cur.push(q);
            }
            None => {
                if cur.len() >= 2 {
                    out.push(std::mem::take(&mut cur));
                }
                cur.clear();
            }
        }
    }
    if cur.len() >= 2 {
        out.push(cur);
    }
    out
}

pub fn parse(v: &Value) -> Option<Town> {
    let place = v.get("place")?;
    let bbox = place
        .get("bbox")
        .and_then(|b| b.as_str())
        .or_else(|| v.get("bbox")?.as_str())?;
    let nums: Vec<f64> = bbox
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    if nums.len() != 4 {
        return None;
    }
    let (s, w, n, e) = (nums[0], nums[1], nums[2], nums[3]);
    let (lat0, lon0) = ((s + n) / 2.0, (w + e) / 2.0);
    let kx = libm::cos(lat0.to_radians()) * M_PER_DEG;
    let proj = |lat: f64, lon: f64| {
        pt(
            (lon - lon0) * kx + WINDOW / 2.0,
            (lat - lat0) * M_PER_DEG + WINDOW / 2.0,
        )
    };
    let mut lines = Vec::new();
    for el in v.get("osm")?.get("elements")?.as_array()? {
        let Some(tags) = el.get("tags") else { continue };
        let Some(hw) = tags.get("highway").and_then(|x| x.as_str()) else {
            continue;
        };
        let Some(class) = classify(hw, tags.get("service").and_then(|x| x.as_str())) else {
            continue;
        };
        let Some(geom) = el.get("geometry").and_then(|g| g.as_array()) else {
            continue;
        };
        let pts: Vec<Pt> = geom
            .iter()
            .filter_map(|p| Some(proj(p.get("lat")?.as_f64()?, p.get("lon")?.as_f64()?)))
            .collect();
        if pts.len() < 2 {
            continue;
        }
        for piece in clip_line(&pts) {
            lines.push((class, piece));
        }
    }
    let name = place
        .get("name")
        .and_then(|x| x.as_str())
        .unwrap_or("?")
        .to_string();
    let pop = place.get("pop").and_then(|x| x.as_u64()).unwrap_or(0);
    Some(Town { name, pop, lines })
}

pub fn load(path: &Path) -> Result<Town, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut s = String::new();
    flate2::read::GzDecoder::new(f)
        .read_to_string(&mut s)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let v: Value = serde_json::from_str(&s).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&v).ok_or_else(|| format!("{}: unexpected shape", path.display()))
}

pub fn graph_of(t: &Town) -> Graph {
    graph::build(&t.lines, 0.5, false)
}

pub fn metrics_of(t: &Town) -> Metrics {
    metrics::compute(&graph_of(t))
}
