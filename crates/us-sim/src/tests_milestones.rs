//! Milestone unlocks: buildings that open as the town grows, and what they do around them.

use super::*;
use crate::milestones::Near;
use us_gen::{Preset, generate};

fn new_sim() -> Sim {
    new_sim_days(200)
}

/// A younger town has more empty ground left for big buildings: the settlers fill the lots
/// near the hall within a few months.
fn new_sim_days(days: i32) -> Sim {
    let t = generate(Preset::Grid, 3);
    let mut s = Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::sandbox_unlimited());
    for _ in 0..days {
        s.step_day();
    }
    s
}

fn spot(s: &Sim, k: CivicKind, skip: usize) -> Pt {
    s.lots()
        .iter()
        .filter(|l| l.phase == Phase::Vacant)
        .map(|l| l.pos)
        .filter(|&p| s.check_civic(k, p).is_ok())
        .nth(skip)
        .unwrap_or_else(|| {
            panic!(
                "a spot for {k:?} skip {skip}; vacant {}",
                s.lots().iter().filter(|l| l.phase == Phase::Vacant).count()
            )
        })
}

fn nearest_hall_spot(s: &Sim, k: CivicKind) -> Pt {
    let mut c: Vec<Pt> = s
        .lots()
        .iter()
        .filter(|l| l.phase == Phase::Vacant)
        .map(|l| l.pos)
        .filter(|&p| s.check_civic(k, p).is_ok())
        .collect();
    c.sort_by(|a, b| a.dist(s.hall).total_cmp(&b.dist(s.hall)));
    c[0]
}

fn unlock_all(s: &mut Sim) {
    s.peak_population = 20_000;
    s.step_unlocks();
}

/// Place and finish a building.
fn build(s: &mut Sim, k: CivicKind, skip: usize) -> (usize, Pt) {
    let p = spot(s, k, skip);
    let i = s.place_civic(k, p).expect("placed");
    for _ in 0..80 {
        s.step_day();
    }
    assert!(matches!(s.lots()[i].phase, Phase::Built { .. }));
    s.recompute_values();
    (i, p)
}

#[test]
fn new_kinds_are_appended_after_the_old_ones() {
    use CivicKind::*;
    assert_eq!(
        &CivicKind::ALL[..7],
        &[School, Police, Fire, Clinic, Park, Water, Power]
    );
    assert_eq!(
        &CivicKind::ALL[7..],
        &[Library, HighSchool, Hospital, Transit, Stadium, University]
    );
}

#[test]
fn locked_buildings_are_refused_until_the_town_is_big_enough() {
    let mut s = new_sim();
    let k = CivicKind::Library;
    assert!(!s.is_unlocked(k));
    assert_eq!(s.next_unlock(), Some((k, 1500)));
    let p = spot(&s, CivicKind::Park, 0);
    assert_eq!(
        s.check_civic(k, p),
        Err(PlaceError::Locked { people: 1500 })
    );
    assert_eq!(
        PlaceError::Locked { people: 14_000 }.text(),
        "Unlocks at 14,000 people."
    );
    s.take_events();
    s.peak_population = 1600;
    s.step_unlocks();
    assert!(s.is_unlocked(k));
    assert!(!s.is_unlocked(CivicKind::HighSchool));
    assert_eq!(s.next_unlock(), Some((CivicKind::HighSchool, 3000)));
    let news = s.take_events();
    assert!(
        news.iter()
            .any(|e| e.kind == EventKind::Milestone && e.text.contains("library")),
        "{news:?}"
    );
    // said once
    s.step_unlocks();
    assert!(s.take_events().is_empty());
    assert!(s.check_civic(k, spot(&s, CivicKind::Park, 0)).is_ok());
    // the originals are never locked
    assert!(s.is_unlocked(CivicKind::School) && s.is_unlocked(CivicKind::Power));
}

#[test]
fn hospital_and_high_school_add_capacity() {
    let mut s = new_sim();
    unlock_all(&mut s);
    let seats = s.capacity(Capacity::Schools).supply;
    let health = s.capacity(Capacity::Health).supply;
    build(&mut s, CivicKind::HighSchool, 0);
    build(&mut s, CivicKind::Hospital, 3);
    let f = |d: Dept| s.budget[d.index()].min(130) as i64;
    let want_seats = SEATS_PER_SCHOOL_TEST * 3 * f(Dept::Schools) / 100;
    let want_health = CLINIC_SERVES_TEST * 4 * f(Dept::Health) / 100;
    assert_eq!(s.capacity(Capacity::Schools).supply - seats, want_seats);
    assert_eq!(s.capacity(Capacity::Health).supply - health, want_health);
}
const SEATS_PER_SCHOOL_TEST: i64 = 450;
const CLINIC_SERVES_TEST: i64 = 2500;

#[test]
fn hospital_reaches_further_than_a_clinic() {
    assert!(CivicKind::Hospital.reach_at(1.0) > 1.5 * CivicKind::Clinic.reach_at(1.0));
    assert!(CivicKind::Library.reach_at(1.0) < CivicKind::School.reach_at(1.0));
    assert_eq!(CivicKind::Transit.reach_at(1.0), 0.0);
    assert_eq!(CivicKind::University.reach_at(1.0), 0.0);
}

#[test]
fn library_transit_stadium_and_university_change_their_surroundings() {
    let mut s = new_sim_days(20);
    unlock_all(&mut s);
    let probe = s.hall;
    let before = s.civic_near(probe, true);
    assert_eq!(before, Near::default());
    // an uncovered far corner feels nothing
    let far = pt(s.town().size - 30.0, s.town().size - 30.0);
    assert!(s.civic_near(far, false).value < 0.01);
    // all four are placed together: the settlers fill the open ground quickly, so the big sites
    // have to be taken while it is there
    let stadium_at = spot(&s, CivicKind::Stadium, 0);
    s.place_civic(CivicKind::Stadium, stadium_at).unwrap();
    let uni_at = spot(&s, CivicKind::University, 0);
    s.place_civic(CivicKind::University, uni_at).unwrap();
    // a library and a transit stop close to the hall
    for k in [CivicKind::Library, CivicKind::Transit] {
        let p = nearest_hall_spot(&s, k);
        s.place_civic(k, p).unwrap();
    }
    for _ in 0..100 {
        s.step_day();
    }
    s.recompute_values();
    let lib = s
        .fx_sites
        .iter()
        .find(|x| x.0 == CivicKind::Library)
        .unwrap()
        .1;
    let near = s.civic_near(lib, false);
    assert!(near.value > 0.05, "{near:?}");
    assert!(near.traffic_relief >= 0.0);
    assert!(near.notes.iter().any(|n| n.contains("library")));

    // a stadium crowds the streets and helps shops around it
    let near = s.civic_near(stadium_at, true);
    assert!(near.traffic_add > 0.0 && near.value > 0.0, "{near:?}");
    assert!(near.notes.iter().any(|n| n.contains("stadium")));
    // a university lifts every lot a little
    assert!(s.civic_near(far, false).value >= 0.04);
    assert!(s.arrival_bonus() > 1.05);
    assert!(s.shop_demand_bonus() > 0.05);
    assert!(s.stadium_approval() > 0.0);
    assert!(s.civic_adjusted_congestion(0.5) > 0.0);
}

#[test]
fn a_built_library_shows_a_why_line_and_lifts_land_value() {
    let mut s = new_sim();
    unlock_all(&mut s);
    s.recompute_values();
    let p = nearest_hall_spot(&s, CivicKind::Library);
    let around: Vec<usize> = s
        .lots()
        .iter()
        .enumerate()
        .filter(|(_, l)| matches!(l.phase, Phase::Built { .. }) && l.civic.is_none())
        .filter(|(_, l)| l.pos.dist(p) < 300.0)
        .map(|(i, _)| i)
        .collect();
    assert!(!around.is_empty());
    let before: Vec<f32> = around.iter().map(|&i| s.land_value(i)).collect();
    assert!(around.iter().all(|&i| s.civic_notes(i).is_empty()));
    let i = s.place_civic(CivicKind::Library, p).unwrap();
    for _ in 0..80 {
        s.step_day();
    }
    assert!(matches!(s.lots()[i].phase, Phase::Built { .. }));
    s.recompute_values();
    assert!(around.iter().all(|&i| !s.civic_notes(i).is_empty()));
    let gain: f32 = around
        .iter()
        .zip(&before)
        .map(|(&i, b)| s.land_value(i) - b)
        .sum();
    assert!(gain > 0.0, "land value did not rise: {gain}");
}

#[test]
fn saves_keep_unlocks_and_buildings_and_stay_identical() {
    let mut s = new_sim();
    unlock_all(&mut s);
    build(&mut s, CivicKind::Hospital, 0);
    build(&mut s, CivicKind::Transit, 2);
    let bytes = s.save();
    let mut l = Sim::load(&bytes).expect("loads");
    assert_eq!(l.unlocked, s.unlocked);
    assert_eq!(l.fingerprint(), s.fingerprint());
    assert_eq!(l.fx_sites, s.fx_sites);
    for _ in 0..200 {
        s.step_day();
        l.step_day();
    }
    assert_eq!(l.fingerprint(), s.fingerprint());
    assert_eq!(l.save(), s.save());
}

#[test]
fn a_save_without_the_section_opens_with_earned_unlocks_and_no_news() {
    let mut s = new_sim();
    s.peak_population = 4000;
    s.step_unlocks();
    let mut n = s.save();
    // cut the sections off by rewriting a body with the MILE section removed
    let mut a = Sim::load(&n).unwrap();
    a.unlocked = 0;
    n = a.save();
    // a fresh load of a save whose MILE section says nothing still honours the town's size
    let l = Sim::load(&n).unwrap();
    assert_eq!(l.unlocked, 0);
    let mut old = Sim::load(&strip_mile(&n)).unwrap();
    assert!(old.is_unlocked(CivicKind::HighSchool));
    assert!(!old.is_unlocked(CivicKind::Hospital));
    old.take_events();
    old.step_day();
    assert!(
        old.take_events()
            .iter()
            .all(|e| !e.text.contains("You can now build"))
    );
}

/// The same save with its MILE section dropped (and the checksum fixed).
fn strip_mile(save: &[u8]) -> Vec<u8> {
    let body = &save[22..];
    // sections are the last thing: count u64, then tag, len u64, bytes each
    let (n, mut at) = {
        // find the section table by scanning for the tag: MILE is the only one today
        let pos = body
            .windows(4)
            .rposition(|w| w == b"MILE")
            .expect("section");
        (pos, pos)
    };
    let _ = n;
    // layout before the tag: count (8 bytes); after: len (8) + 4 payload bytes
    at -= 8;
    let mut b = body[..at].to_vec();
    b.extend_from_slice(&0u64.to_le_bytes());
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &x in &b {
        h ^= x as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let mut out = b"USSV".to_vec();
    out.extend_from_slice(&u16::from_le_bytes([save[4], save[5]]).to_le_bytes());
    out.extend_from_slice(&(b.len() as u64).to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&b);
    out
}
