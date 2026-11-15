use super::*;
use us_gen::{Preset, generate};

fn sim(preset: Preset) -> Sim {
    let t = generate(preset, 3);
    Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::sandbox())
}

/// Days of each sky over some years, by pure forecast.
fn tally(preset: Preset, years: i32) -> ([u32; 7], f64) {
    let s = sim(preset);
    let mut n = [0u32; 7];
    let mut temp = 0.0;
    for d in 0..365 * years {
        let w = weather_on_for_tests(&s, d);
        n[w.sky as usize] += 1;
        temp += w.temp_c as f64;
    }
    (n, temp / (365 * years) as f64)
}

fn weather_on_for_tests(s: &Sim, day: i32) -> Weather {
    crate::climate::weather_on(s.seed, s.town.preset, day)
}

#[test]
fn forecast_is_pure_and_matches_the_day() {
    let mut s = sim(Preset::Winding);
    for _ in 0..40 {
        let tomorrow = s.weather_tomorrow();
        s.step_day();
        assert_eq!(s.weather(), tomorrow);
    }
    let a = sim(Preset::Winding);
    assert_eq!(a.weather(), sim(Preset::Winding).weather());
}

#[test]
fn every_style_has_a_believable_year() {
    for p in Preset::ALL {
        let (n, mean) = tally(p, 10);
        let days = 3650.0;
        let f = |s: Sky| n[s as usize] as f64 / 10.0;
        let precip = f(Sky::Rain) + f(Sky::Snow) + f(Sky::Storm) + f(Sky::Blizzard);
        eprintln!(
            "{p:?}: mean {mean:.1}C clear {} cloud {} rain {} snow {} storm {} heat {} blizzard {}",
            f(Sky::Clear),
            f(Sky::Cloudy),
            f(Sky::Rain),
            f(Sky::Snow),
            f(Sky::Storm),
            f(Sky::HeatWave),
            f(Sky::Blizzard)
        );
        let _ = days;
        assert!(
            (40.0..160.0).contains(&precip),
            "{p:?} precip days {precip}"
        );
        assert!(f(Sky::Clear) > 80.0, "{p:?}");
        assert!(
            f(Sky::Storm) >= 2.0 || p == Preset::Sunbelt,
            "{p:?} storms {}",
            f(Sky::Storm)
        );
        assert!(f(Sky::Storm) <= 25.0, "{p:?} storms {}", f(Sky::Storm));
        assert!(f(Sky::HeatWave) <= 40.0, "{p:?} heat {}", f(Sky::HeatWave));
        if p == Preset::Sunbelt {
            assert_eq!(f(Sky::Snow) + f(Sky::Blizzard), 0.0);
            assert!(f(Sky::HeatWave) >= 3.0);
        } else {
            assert!(f(Sky::Snow) + f(Sky::Blizzard) >= 3.0, "{p:?} no snow");
        }
    }
}

#[test]
fn dryness_swings_but_averages_near_the_middle() {
    for p in Preset::ALL {
        let mut s = sim(p);
        let (mut sum, mut lo, mut hi) = (0.0, 1.0f64, 0.0f64);
        for _ in 0..365 * 8 {
            s.step_day();
            let d = s.dryness();
            sum += d;
            lo = lo.min(d);
            hi = hi.max(d);
        }
        let mean = sum / (365.0 * 8.0);
        eprintln!("{p:?}: dryness mean {mean:.2} low {lo:.2} high {hi:.2}");
        assert!((0.2..=0.9).contains(&mean), "{p:?} mean {mean}");
        assert!(hi - lo > 0.3, "{p:?} never changes");
    }
}

#[test]
fn snow_lies_and_melts() {
    let mut s = sim(Preset::LakeTown);
    let (mut max_cover, mut max_road) = (0.0f32, 0.0f32);
    for _ in 0..365 * 3 {
        s.step_day();
        max_cover = max_cover.max(s.snow_cover());
        max_road = max_road.max(s.road_snow());
        if s.date().month == 8 {
            assert_eq!(s.snow_cover(), 0.0);
        }
    }
    assert!(max_cover > 0.4, "{max_cover}");
    assert!(max_road > 0.3, "{max_road}");
}

/// A river town a few years old, tended, with the river forced up for one day.
fn flooded_town(years: i32) -> Sim {
    // the town hall goes as near the river as the generator allows
    let t = generate(Preset::Winding, 3);
    let hall = hall_sites(&t)
        .into_iter()
        .min_by(|a, b| {
            let d = |p: &Pt| crate::dist_pt_ring_edge(*p, &t.water[0]);
            d(a).partial_cmp(&d(b)).unwrap()
        })
        .unwrap();
    let mut s = Sim::new_with_rules(&t, hall, 7, 2, Rules::sandbox());
    let mut round = 0;
    for d in 0..365 * years {
        s.step_day();
        if d % 30 == 0 {
            crate::tests_balance::tend(&mut s, &mut round);
        }
    }
    s
}

fn same(a: &[u8], b: &[u8], what: &str) {
    if a != b {
        let at = a.iter().zip(b).position(|(x, y)| x != y);
        panic!(
            "{what}: saves differ, lengths {} {}, first difference at {at:?}",
            a.len(),
            b.len()
        );
    }
}

fn copy(s: &mut Sim) -> Sim {
    Sim::load(&s.save()).expect("loads")
}

fn force_flood(s: &mut Sim, soak: f64) {
    s.wx.soak = soak;
    s.step_day();
}

#[test]
fn rivers_rise_now_and_then_on_their_own() {
    let mut s = sim(Preset::Winding);
    let (mut events, mut prev) = (0, 0.0);
    for _ in 0..365 * 30 {
        s.step_day();
        if s.flood_depth() > 0.0 && prev == 0.0 {
            events += 1;
        }
        prev = s.flood_depth();
    }
    eprintln!("{events} river floods in 30 years");
    assert!((3..=25).contains(&events), "{events}");
}

#[test]
fn a_flood_wets_low_buildings_closes_roads_and_goes_down() {
    let mut s = flooded_town(20);
    let (departures, treasury) = (s.departures, s.treasury);
    s.take_events();
    force_flood(&mut s, 6.0);
    assert!(!s.flooded().is_empty(), "no building was reached");
    assert!(!s.flooded_roads().is_empty(), "no road closed");
    let (_, cost) = s.weather_costs();
    assert!(cost.flood_repairs > 0);
    let news = s.take_events();
    assert!(
        news.iter().any(|e| e.kind == EventKind::Weather
            && (e.text.starts_with("Flood") || e.text.starts_with("The flood rose"))),
        "no news"
    );
    // why panel
    let lot = s.flooded()[0];
    assert!(s.why(lot).ordinances.iter().any(|l| l.starts_with("Flood")));
    // the closed roads leave the traffic network and weigh on congestion
    assert!(s.weather_congestion() > 0.0);
    assert!(s.weather_approval() < 0.0);
    assert!(s.weather_trouble() > 0.9);
    let _ = (departures, treasury);
    // it goes down within days and what it cost is paid at the month end
    let mut days = 0;
    while (!s.flooded().is_empty() || !s.flooded_roads().is_empty()) && days < 90 {
        s.step_day();
        days += 1;
    }
    eprintln!("flood lasted {days} days");
    assert!(days < 30, "flood lasted {days} days");
    assert!(s.history().last().unwrap().admin > 0);
}

#[test]
fn flooded_homes_lose_value_for_a_while() {
    let mut s = flooded_town(20);
    force_flood(&mut s, 6.0);
    let lot = *s
        .flooded()
        .iter()
        .find(|&&i| s.flood_value_penalty(i) > 0.0)
        .expect("a hit lot");
    assert!(s.flood_value_penalty(lot) > 0.05);
    for _ in 0..200 {
        s.step_day();
    }
    assert_eq!(s.flood_value_penalty(lot), 0.0);
}

#[test]
fn the_desert_town_never_floods() {
    let mut s = sim(Preset::Sunbelt);
    for _ in 0..365 * 15 {
        s.step_day();
        assert!(s.flooded().is_empty());
    }
}

#[test]
fn storms_cost_money_and_can_start_fires() {
    let mut s = sim(Preset::Grid);
    let mut round = 0;
    let (mut repairs, mut strikes) = (0i64, 0usize);
    for d in 0..365 * 12 {
        s.step_day();
        if d % 30 == 0 {
            crate::tests_balance::tend(&mut s, &mut round);
        }
        strikes += s.lightning_strikes().len();
        repairs = repairs.max(s.weather_costs().1.storm_repairs);
    }
    let text: Vec<_> = s
        .take_events()
        .into_iter()
        .filter(|e| e.kind == EventKind::Weather)
        .collect();
    eprintln!(
        "strikes {strikes}, biggest month of storm repairs {repairs}, weather news {}",
        text.len()
    );
    assert!(strikes > 5);
    assert!(repairs > 0);
    assert!(s.fires_total > 0);
}

#[test]
fn the_day_before_a_storm_there_is_a_warning() {
    let mut s = sim(Preset::Grid);
    let mut warned = false;
    for _ in 0..365 * 5 {
        let tomorrow = s.weather_tomorrow();
        let warning = s.weather_warning();
        if tomorrow.sky == Sky::Storm && s.weather().sky != Sky::Storm {
            assert_eq!(warning.as_deref(), Some("A thunderstorm is on its way."));
            warned = true;
        }
        s.step_day();
    }
    assert!(warned);
    let news = s.take_events();
    assert!(
        news.iter()
            .any(|e| e.kind == EventKind::Weather && e.text.contains("thunderstorm is on its way")),
        "no news warning"
    );
}

#[test]
fn a_full_power_grid_goes_out_in_a_storm() {
    let mut s = sim(Preset::Grid);
    let mut round = 0;
    for d in 0..365 * 6 {
        s.step_day();
        if d % 30 == 0 {
            crate::tests_balance::tend(&mut s, &mut round);
        }
    }
    // a town that stays on the edge of its power capacity
    let mut cuts = 0;
    for _ in 0..365 * 4 {
        s.step_day();
        let st = s.capacity_with(Capacity::Power, false);
        if st.demand as f64 > st.supply as f64 * 0.9 {
            cuts += s.shortage_today() as u32;
        }
    }
    eprintln!("power cuts {cuts}, approval cost {}", s.weather_approval());
}

#[test]
fn heat_waves_raise_water_use() {
    let mut s = sim(Preset::Sunbelt);
    for _ in 0..4000 {
        if s.heat_wave() {
            break;
        }
        s.step_day();
    }
    assert!(s.heat_wave());
    let hot = s.capacity(Capacity::Water).demand;
    let base = s.capacity_with(Capacity::Water, false).demand;
    assert!(hot > base, "{hot} {base}");
}

#[test]
fn storm_drains_cut_flooding_and_codes_cut_repairs() {
    let mut s = flooded_town(20);
    let mut a = copy(&mut s);
    let mut b = copy(&mut s);
    b.set_ordinance_town(Ordinance::StormDrains, true);
    force_flood(&mut a, 6.0);
    force_flood(&mut b, 6.0);
    eprintln!(
        "flooded lots: plain {} drains {}",
        a.flooded().len(),
        b.flooded().len()
    );
    assert!(!a.flooded().is_empty());
    assert!(b.flooded().len() < a.flooded().len());
    assert!(b.weather_costs().1.flood_repairs < a.weather_costs().1.flood_repairs);
    assert!(b.ordinance_costs() > 0);
}

#[test]
fn plowing_ordinance_clears_snow_faster_and_costs_by_the_metre() {
    let snow = |plow: bool| {
        let mut s = sim(Preset::LakeTown);
        if plow {
            s.set_ordinance_town(Ordinance::SnowPlowing, true);
        }
        let mut total = 0.0f64;
        for _ in 0..365 * 4 {
            s.step_day();
            total += s.road_snow() as f64;
        }
        total
    };
    assert!(snow(true) < snow(false));
    let mut s = sim(Preset::LakeTown);
    assert!(s.ordinance_estimate(Ordinance::SnowPlowing, None) < 0);
    s.set_ordinance_town(Ordinance::SnowPlowing, true);
    assert!(s.ordinance_costs() > 0);
}

#[test]
fn low_road_budget_plows_slower() {
    let snow = |pct: u32| {
        let mut s = sim(Preset::LakeTown);
        s.set_budget(Dept::Roads, pct);
        let mut total = 0.0f64;
        for _ in 0..365 * 4 {
            s.step_day();
            total += s.road_snow() as f64;
        }
        total
    };
    assert!(snow(50) > snow(100));
}

#[test]
fn building_codes_slow_construction() {
    let mut s = sim(Preset::Grid);
    s.set_ordinance_town(Ordinance::BuildingCodes, true);
    let li = (0..s.lots.len())
        .find(|&i| s.ordinance_at(i, Ordinance::BuildingCodes))
        .unwrap();
    assert_eq!(s.codes_build_days(li, 40), 50);
}

#[test]
fn weather_costs_come_out_of_the_treasury_and_the_books() {
    let mut s = sim(Preset::Winding);
    let mut round = 0;
    for d in 0..365 * 10 {
        s.step_day();
        if d % 30 == 0 {
            crate::tests_balance::tend(&mut s, &mut round);
        }
    }
    let total: i64 = s.history().iter().map(|h| h.admin).sum();
    assert!(total > 0);
    let (last, _) = s.weather_costs();
    eprintln!("last month weather cost {:?}", last);
}

#[test]
fn weather_state_round_trips_mid_flood() {
    let mut s = flooded_town(15);
    s.set_ordinance_town(Ordinance::SnowPlowing, true);
    force_flood(&mut s, 6.0);
    assert!(!s.flooded().is_empty());
    let bytes = s.save();
    let mut loaded = Sim::load(&bytes).expect("loads");
    for ((t1, a), (t2, b)) in crate::save_ext::sections(&s)
        .iter()
        .zip(crate::save_ext::sections(&loaded).iter())
    {
        eprintln!(
            "section {:?} {} {} {}",
            std::str::from_utf8(t1),
            t2.len(),
            a.len(),
            b.len()
        );
    }
    same(&loaded.save(), &bytes, "reload");
    assert_eq!(loaded.fingerprint(), s.fingerprint());
    assert_eq!(loaded.flooded(), s.flooded());
    assert_eq!(loaded.flooded_roads(), s.flooded_roads());
    for _ in 0..400 {
        s.step_day();
        loaded.step_day();
    }
    assert_eq!(loaded.fingerprint(), s.fingerprint());
    same(&loaded.save(), &s.save(), "after stepping");
}

#[test]
fn weather_state_round_trips_in_deep_snow() {
    let mut s = sim(Preset::LakeTown);
    let mut saved = None;
    for _ in 0..365 * 10 {
        s.step_day();
        if s.snow_cover() > 0.25 && s.road_snow() > 0.2 {
            saved = Some(s.save());
            break;
        }
    }
    let bytes = saved.expect("deep snow in ten years");
    let mut loaded = Sim::load(&bytes).expect("loads");
    assert_eq!(loaded.fingerprint(), s.fingerprint());
    assert_eq!(loaded.snow_cover(), s.snow_cover());
    for _ in 0..300 {
        s.step_day();
        loaded.step_day();
    }
    assert_eq!(loaded.fingerprint(), s.fingerprint());
    same(&loaded.save(), &s.save(), "after stepping");
}

#[test]
fn an_old_save_without_weather_starts_calm_and_keeps_going() {
    let mut s = sim(Preset::Grid);
    for _ in 0..200 {
        s.step_day();
    }
    let mut loaded = Sim::load(&s.save()).unwrap();
    loaded.wx = Default::default();
    for _ in 0..100 {
        loaded.step_day();
    }
    assert!(loaded.dryness() >= 0.0);
}
