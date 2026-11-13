use serde_json::json;
use us_gen::RoadClass;
use us_metrics::corpus::{classify, parse};

#[test]
fn osm_classes_and_projection() {
    assert_eq!(classify("motorway", None), Some(RoadClass::Highway));
    assert_eq!(classify("trunk", None), Some(RoadClass::Highway));
    assert_eq!(classify("primary", None), Some(RoadClass::Arterial));
    assert_eq!(classify("secondary", None), Some(RoadClass::Arterial));
    assert_eq!(classify("tertiary", None), Some(RoadClass::Collector));
    assert_eq!(classify("residential", None), Some(RoadClass::Local));
    assert_eq!(classify("unclassified", None), Some(RoadClass::Local));
    assert_eq!(classify("living_street", None), Some(RoadClass::Local));
    assert_eq!(classify("service", Some("alley")), Some(RoadClass::Alley));
    assert_eq!(classify("service", Some("driveway")), None);
    assert_eq!(classify("service", None), None);
    assert_eq!(classify("footway", None), None);

    // a street through the middle of the bounding box lands in the middle of the window
    let v = json!({
        "place": {"name": "Testville", "pop": 1000, "bbox": "40.0,-90.0,40.036,-89.953"},
        "osm": {"elements": [
            {"type": "way", "tags": {"highway": "residential"},
             "geometry": [{"lat": 40.018, "lon": -90.0}, {"lat": 40.018, "lon": -89.953}]},
            {"type": "way", "tags": {"highway": "footway"},
             "geometry": [{"lat": 40.018, "lon": -90.0}, {"lat": 40.018, "lon": -89.953}]}
        ]}
    });
    let t = parse(&v).expect("parse");
    assert_eq!(t.lines.len(), 1);
    let pts = &t.lines[0].1;
    assert!((pts[0].y - 2048.0).abs() < 1.0, "{:?}", pts[0]);
    assert!(pts[0].x.abs() < 1.0 || pts[0].x > 0.0);
    // 0.047 degrees of longitude at 40 N is about 4 km: the street spans the window
    let xs: Vec<f64> = pts.iter().map(|p| p.x).collect();
    assert!(xs.iter().cloned().fold(f64::MAX, f64::min) >= 0.0);
    assert!(xs.iter().cloned().fold(f64::MIN, f64::max) <= 4096.0);
}
