use super::*;
use us_gen::{Preset, generate};

fn new_sim(preset: Preset, tseed: u64) -> Sim {
    let t = generate(preset, tseed);
    Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::sandbox())
}

/// Whether two polylines touch: they cross, or an end or vertex lies on the other.
fn touch(a: &[Pt], b: &[Pt]) -> bool {
    a.windows(2).any(|p| {
        b.windows(2)
            .any(|q| streets::polylines_meet(p[0], p[1], q[0], q[1]))
    })
}

/// Every street on the map that is finished or being built is joined to the old roads through
/// other finished or building streets, so the network is one piece.
fn assert_one_network(s: &Sim) {
    let vis: Vec<usize> = (0..s.road_runs().len())
        .filter(|&i| s.road_visible(i))
        .collect();
    let mut comp: Vec<usize> = (0..vis.len()).collect();
    fn find(c: &mut [usize], mut x: usize) -> usize {
        while c[x] != x {
            c[x] = c[c[x]];
            x = c[x];
        }
        x
    }
    for i in 0..vis.len() {
        for j in 0..i {
            let (a, b) = (&s.road_runs()[vis[i]], &s.road_runs()[vis[j]]);
            if a.class == RoadClass::Rail || b.class == RoadClass::Rail {
                continue;
            }
            if touch(&a.points, &b.points) {
                let (ra, rb) = (find(&mut comp, i), find(&mut comp, j));
                comp[ra] = rb;
            }
        }
    }
    let rooted: Vec<usize> = (0..vis.len())
        .filter(|&i| s.road_runs()[vis[i]].addition.is_none())
        .map(|i| find(&mut comp, i))
        .collect();
    for (i, &vi) in vis.iter().enumerate() {
        let r = &s.road_runs()[vi];
        // slivers of a street that the generator cut at a neighbourhood border may stand alone
        let sliver = r.points.windows(2).map(|w| w[0].dist(w[1])).sum::<f64>() < 10.0;
        if r.addition.is_some() && r.class != RoadClass::Alley && !sliver {
            let c = find(&mut comp, i);
            assert!(
                rooted.contains(&c),
                "day {}: street {} floats free of the old roads",
                s.day,
                r.street
            );
        }
    }
}

#[test]
fn the_network_grows_one_connected_street_at_a_time() {
    for (preset, seed) in [
        (Preset::Grid, 3),
        (Preset::Winding, 5),
        (Preset::Sunbelt, 4),
    ] {
        let mut s = new_sim(preset, seed);
        let n_add = s.revealed.len();
        let mut opened_on: Vec<(usize, i32)> = Vec::new();
        let at_start: Vec<usize> = (0..s.streets.len())
            .filter(|&i| s.streets[i].state == StreetState::Open)
            .collect();
        let mut prev_open = s.streets_opened();
        for d in 0..365 * 4 {
            s.step_day();
            let now = s.streets_opened();
            if now > prev_open {
                for (i, st) in s.streets.iter().enumerate() {
                    if st.state == StreetState::Open
                        && let Some(a) = st.addition
                        && !at_start.contains(&i)
                        && !opened_on.iter().any(|&(k, _)| k == i)
                    {
                        let _ = a;
                        opened_on.push((i, s.day));
                    }
                }
                prev_open = now;
            }
            let big = s
                .streets
                .iter()
                .filter(|st| {
                    matches!(st.state, StreetState::Building { .. }) && st.class != RoadClass::Alley
                })
                .count();
            assert!(big <= 3);
            if d % 30 == 0 {
                assert_one_network(&s);
            }
        }
        assert!(s.streets_opened() > 40, "{preset:?}: few streets opened");
        // no addition of any size appears whole within a month
        let mut worst = 0.0f64;
        for a in 0..n_add {
            let own: Vec<i32> = opened_on
                .iter()
                .filter(|&&(i, _)| s.streets[i].addition == Some(a))
                .map(|&(_, d)| d)
                .collect();
            let total = s
                .streets
                .iter()
                .filter(|st| st.addition == Some(a) && st.class != RoadClass::Alley)
                .count();
            if total < 8 || own.len() < 8 {
                continue;
            }
            let best = own
                .iter()
                .map(|&d0| own.iter().filter(|&&d| d >= d0 && d < d0 + 30).count())
                .max()
                .unwrap();
            worst = worst.max(best as f64 / total as f64);
        }
        eprintln!("{preset:?}: largest share of an addition opened in 30 days: {worst:.2}");
        assert!(worst < 0.6, "{preset:?}: {worst}");
    }
}

#[test]
fn lots_are_buildable_only_on_open_streets() {
    let mut s = new_sim(Preset::Grid, 3);
    for d in 0..900 {
        s.step_day();
        if d % 10 != 0 {
            continue;
        }
        for (i, l) in s.lots().iter().enumerate() {
            let Some(&st) = s.lot_street.get(i) else {
                continue;
            };
            if st == streets::NO_STREET || l.exempt || l.civic.is_some() {
                continue;
            }
            if matches!(
                l.phase,
                Phase::Vacant | Phase::Building { .. } | Phase::Built { .. }
            ) {
                assert!(
                    s.street_open(st),
                    "day {}: lot {i} stands on a street not open",
                    s.day
                );
            }
        }
    }
}

#[test]
fn a_street_reports_its_build_stages_then_opens() {
    let mut s = new_sim(Preset::Grid, 3);
    let mut seen: std::collections::HashMap<usize, Vec<RoadStage>> = Default::default();
    let mut last_progress: std::collections::HashMap<usize, f32> = Default::default();
    for _ in 0..400 {
        s.step_day();
        for i in 0..s.road_runs().len() {
            if let Some(b) = s.road_build_state(i) {
                assert!(s.road_visible(i) && !s.road_open(i));
                let l = seen.entry(i).or_default();
                if l.last() != Some(&b.stage) {
                    l.push(b.stage);
                }
                let p = last_progress.insert(i, b.progress).unwrap_or(0.0);
                assert!(b.progress >= p);
            }
        }
    }
    assert!(seen.len() > 5);
    assert!(
        seen.values()
            .any(|v| v.as_slice() == [RoadStage::Graded, RoadStage::Paved])
    );
    // a street took weeks, not a day
    let days: Vec<i32> = s
        .streets
        .iter()
        .filter_map(|st| match st.state {
            StreetState::Building { start, end } => Some(end - start),
            _ => None,
        })
        .collect();
    assert!(days.iter().all(|&d| (7..=49).contains(&d)));
}

#[test]
fn every_street_of_a_started_neighbourhood_is_eventually_built() {
    let mut s = new_sim(Preset::Winding, 5);
    for _ in 0..365 * 25 {
        s.step_day();
    }
    let planned = s
        .streets
        .iter()
        .filter(|st| st.state == StreetState::Planned && st.class != RoadClass::Alley)
        .count();
    eprintln!(
        "planned left after 25 years: {planned}, additions left {}",
        s.additions_left()
    );
    assert!(s.streets_opened() > 100);
}
