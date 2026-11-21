use super::*;
use us_gen::{Preset, generate};

fn new_sim(preset: Preset, tseed: u64) -> Sim {
    let t = generate(preset, tseed);
    Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::sandbox())
}

/// Find a road that can be built from an open road: start and end points.
fn find_road(s: &Sim, class: RoadClass) -> Option<Vec<Pt>> {
    for (r, run) in s.road_runs().iter().enumerate() {
        if !s.road_open(r) || !matches!(run.class, RoadClass::Local | RoadClass::Collector) {
            continue;
        }
        for w in run.points.windows(2) {
            let a = w[0].lerp(w[1], 0.5);
            for k in 0..12 {
                let ang = k as f64 * 0.52;
                for len in [160.0, 260.0, 380.0] {
                    let b = pt(a.x + len * libm::cos(ang), a.y + len * libm::sin(ang));
                    if s.check_road(class, &[a, b]).is_ok() {
                        return Some(vec![a, b]);
                    }
                }
            }
        }
    }
    None
}

fn grown(preset: Preset, seed: u64, days: i32) -> Sim {
    let mut s = new_sim(preset, seed);
    for _ in 0..days {
        s.step_day();
    }
    s
}

#[test]
fn avenue_is_checked_paid_built_and_opened() {
    let mut s = grown(Preset::Grid, 3, 400);
    let pts = find_road(&s, RoadClass::Arterial).expect("some avenue fits");
    let plan = s.check_road(RoadClass::Arterial, &pts).unwrap();
    assert!(plan.cost > 0 && plan.length > 100.0);
    let (t0, runs0) = (s.treasury, s.road_runs().len());
    let r = s.build_road(RoadClass::Arterial, &pts).unwrap();
    assert_eq!(s.road_runs().len(), runs0 + 1);
    assert_eq!(t0 - s.treasury, plan.cost);
    assert!(s.road_visible(r) && !s.road_open(r));
    assert!(s.road_build_state(r).is_some());
    let upkeep0: i64 = s.road_cost_per_month.iter().sum();
    for _ in 0..100 {
        s.step_day();
    }
    assert!(s.road_open(r), "the road opens");
    assert!(s.road_build_state(r).is_none());
    let upkeep1: i64 = s.road_cost_per_month.iter().sum();
    assert!(upkeep1 > upkeep0, "upkeep grows when it opens");
}

#[test]
fn refusals_say_why() {
    let s = grown(Preset::Grid, 3, 200);
    let far = pt(s.town().size / 2.0 + 77.0, s.town().size / 2.0 + 913.0);
    assert!(matches!(
        s.check_road(RoadClass::Arterial, &[far, pt(far.x + 100.0, far.y)]),
        Err(RoadError::StartNotOnRoad { .. }) | Err(RoadError::OutOfBounds { .. })
    ));
    assert_eq!(
        s.check_road(RoadClass::Arterial, &[far]).err(),
        Some(RoadError::NeedsPoints)
    );
    // through a standing building
    let b = s
        .lots()
        .iter()
        .find(|l| matches!(l.phase, Phase::Built { .. }) && !l.exempt)
        .unwrap();
    let road = s
        .road_runs()
        .iter()
        .enumerate()
        .find(|(i, r)| s.road_open(*i) && r.class == RoadClass::Local)
        .unwrap()
        .1;
    let start = road.points[0].lerp(road.points[1], 0.5);
    let err = s.check_road(RoadClass::Collector, &[start, b.pos]).err();
    assert!(err.is_some(), "a drawn road onto a building is refused");
    assert!(!err.unwrap().text().is_empty());
}

#[test]
fn bridges_and_water() {
    for preset in [Preset::Grid, Preset::Winding] {
        let s = grown(preset, 5, 100);
        // a road that crosses water must report a bridge or refuse, never ignore the water
        for ring in &s.town().water {
            let c = ring[0];
            let d = ring[ring.len() / 2];
            for (r, run) in s.road_runs().iter().enumerate() {
                if !s.road_open(r) || run.class != RoadClass::Local {
                    continue;
                }
                let a = run.points[0];
                if let Ok(p) = s.check_road(RoadClass::Collector, &[a, c, d]) {
                    assert!(p.bridge_m > 0.0 || p.bridges.is_empty());
                }
            }
        }
    }
}

#[test]
fn upgrade_changes_class_and_costs() {
    let mut s = grown(Preset::Grid, 3, 300);
    let mut done = false;
    for (r, run) in s.road_runs().to_vec().iter().enumerate() {
        if run.class != RoadClass::Local || !s.road_open(r) {
            continue;
        }
        let p = run.points[0].lerp(run.points[1], 0.5);
        if let Ok(plan) = s.check_upgrade(p) {
            let t0 = s.treasury;
            let v0 = s.roads_version();
            s.upgrade_street(p).unwrap();
            assert_eq!(s.road_runs()[plan.run].class, RoadClass::Collector);
            assert_eq!(t0 - s.treasury, plan.cost);
            assert!(s.roads_version() > v0);
            done = true;
            break;
        }
    }
    assert!(done, "some local street can be widened");
    assert_eq!(
        s.check_upgrade(pt(-50.0, -50.0)).err(),
        Some(RoadError::NoStreet)
    );
}

#[test]
fn avenue_save_mid_build_matches() {
    let mut s = grown(Preset::Grid, 3, 400);
    let pts = find_road(&s, RoadClass::Arterial).unwrap();
    s.build_road(RoadClass::Arterial, &pts).unwrap();
    for _ in 0..10 {
        s.step_day();
    }
    let bytes = s.save();
    let mut b = Sim::load(&bytes).unwrap();
    assert_eq!(b.fingerprint(), s.fingerprint());
    for _ in 0..400 {
        s.step_day();
        b.step_day();
    }
    assert_eq!(b.fingerprint(), s.fingerprint());
    assert_eq!(b.save(), s.save());
    assert_eq!(b.avenues_built(), 1);
}

#[test]
fn unused_feature_changes_nothing() {
    let s = grown(Preset::Grid, 3, 50);
    assert_eq!(s.avenue_effect(s.hall), (0.0, 0.0));
    assert_eq!(s.avenues_built(), 0);
}

#[test]
fn road_dragged_onto_wide_avenue_joins_the_routing_graph() {
    let mut s = new_sim(Preset::Grid, 3);
    s.hall = pt(3000.0, 3000.0);
    s.lots.clear();
    s.town.stations.clear();
    s.town.water.clear();
    let a = pt(1300.0, 1100.0);
    let b = pt(1500.0, 1100.0);
    let end = pt(1400.0, 1100.0);
    let c = pt(1300.0, 1200.0);
    let d = pt(1500.0, 1200.0);
    let junction = pt(1400.0, 1200.0);
    s.streets = [(a, b), (c, d)]
        .into_iter()
        .map(|(a, b)| streets::Street::open_connector(RoadClass::Arterial, a, b))
        .collect();
    s.runs = [(a, b), (c, d)]
        .into_iter()
        .enumerate()
        .map(|(i, (a, b))| RoadRun {
            class: RoadClass::Arterial,
            points: vec![a, b],
            addition: None,
            street: i as u32,
        })
        .collect();
    let free = pt(1407.0, 1150.0);
    assert_eq!(s.road_draft_point(&[end], free, false), free);
    assert_eq!(s.road_draft_point(&[end], free, true), pt(1400.0, 1150.0));

    let path = [pt(1400.0, 1114.0), pt(1400.0, 1186.0)];
    assert_eq!(s.road_draft_point(&[end], path[1], true), junction);
    let index = s.build_road(RoadClass::Collector, &path).unwrap();
    let run = &s.road_runs()[index];
    assert_eq!(run.points, [end, junction]);
    assert!(s.road_runs()[0].points.contains(&end));
    assert!(s.road_runs()[1].points.contains(&junction));
    assert!(!s.road_open(index), "construction blocks traffic");
    s.open_street(run.street as usize);
    let graph = roadnet::RoadNet::build(
        s.town(),
        &s.road_runs()
            .iter()
            .enumerate()
            .filter(|(i, _)| s.road_open(*i))
            .map(|(_, r)| (r.class, r.points.clone()))
            .collect::<Vec<_>>(),
    );
    let (start, _) = graph.attach(a).unwrap();
    let (finish, _) = graph.attach(d).unwrap();
    assert!((graph.distances(start, 500.0)[finish as usize] - 300.0).abs() < 1.0);
}
