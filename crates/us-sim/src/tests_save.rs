use super::*;
use us_gen::{Preset, generate};

fn new_sim(preset: Preset, rules: Rules) -> Sim {
    let t = generate(preset, 3);
    Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, rules)
}

/// Save at several points of a long game, load, and check both copies stay identical.
fn round_trips(preset: Preset, points: &[i32], more: i32) {
    let mut s = new_sim(preset, Rules::sandbox());
    s.set_tax_x10(80);
    s.set_budget(Dept::Parks, 70);
    let _ = s.take_loan();
    for &p in points {
        while s.day < p {
            s.step_day();
        }
        let bytes = s.save();
        let mut loaded = Sim::load(&bytes).expect("loads");
        assert_eq!(
            loaded.save(),
            bytes,
            "day {p}: save, load, save changed bytes"
        );
        // never saved: a fresh copy of the same game
        let mut twin = Sim::load(&bytes).unwrap();
        for _ in 0..more {
            s.step_day();
            loaded.step_day();
            twin.step_day();
        }
        assert_eq!(loaded.fingerprint(), s.fingerprint(), "day {p}");
        assert_eq!(
            loaded.save(),
            s.save(),
            "day {p}: states differ after stepping"
        );
        assert_eq!(twin.save(), s.save());
        // go on from where the original is; the next save point continues the same game
    }
}

#[test]
fn loading_then_stepping_equals_never_saving() {
    // 133 is while a traffic run is in flight; 1000 and 2500 are mid game
    round_trips(Preset::Grid, &[133, 400, 1000, 2500], 500);
}

#[test]
fn winding_town_round_trips_too() {
    round_trips(Preset::Winding, &[45, 700], 400);
}

#[test]
fn every_preset_saves_and_loads() {
    for p in Preset::ALL {
        let mut s = new_sim(p, Rules::sandbox());
        for _ in 0..60 {
            s.step_day();
        }
        let bytes = s.save();
        assert_eq!(Sim::load(&bytes).expect("loads").save(), bytes, "{p:?}");
    }
}

#[test]
fn a_game_in_progress_with_civics_and_events_round_trips() {
    let t = generate(Preset::Grid, 3);
    let mut s = Sim::new(&t, hall_sites(&t)[0], 7);
    for _ in 0..300 {
        s.step_day();
    }
    let mut round = 0;
    for _ in 0..400 {
        s.step_day();
        if s.day % 30 == 0 {
            crate::tests_balance::tend(&mut s, &mut round);
        }
    }
    assert!(s.lots().iter().any(|l| l.civic.is_some()));
    let bytes = s.save();
    let l = Sim::load(&bytes).unwrap();
    assert_eq!(l.history().len(), s.history().len());
    assert_eq!(l.loans().len(), s.loans().len());
    assert_eq!(l.rival().name, s.rival().name);
    assert_eq!(l.approval(), s.approval());
    assert_eq!(l.next_election_day(), s.next_election_day());
    assert_eq!(l.rules(), s.rules());
    assert_eq!(l.lots().len(), s.lots().len());
    assert_eq!(l.road_runs().len(), s.road_runs().len());
}

#[test]
fn a_lost_game_stays_lost_after_loading() {
    let mut s = new_sim(Preset::Grid, Rules::standard());
    s.set_tax_x10(115);
    for _ in 0..ELECTION_PERIOD_DAYS + 40 {
        s.step_day();
    }
    let g = s.game_over().cloned().expect("lost");
    let mut l = Sim::load(&s.save()).unwrap();
    assert_eq!(l.game_over(), Some(&g));
    let day = l.day;
    l.step_day();
    assert_eq!(l.day, day);
    assert_eq!(
        l.last_election().unwrap().challenger,
        s.last_election().unwrap().challenger
    );
}

#[test]
fn bad_files_are_refused_with_a_clear_error() {
    let mut s = new_sim(Preset::Grid, Rules::sandbox());
    for _ in 0..100 {
        s.step_day();
    }
    let good = s.save();
    assert_eq!(Sim::load(b"hello").err(), Some(LoadError::NotASave));
    assert_eq!(Sim::load(&[]).err(), Some(LoadError::NotASave));
    let mut old = good.clone();
    old[4..6].copy_from_slice(&0u16.to_le_bytes());
    let e = Sim::load(&old).err().unwrap();
    assert_eq!(
        e,
        LoadError::TooOld {
            found: 0,
            current: SAVE_VERSION
        }
    );
    assert!(e.to_string().contains("before 1.0.0 preview 6"));
    assert!(e.to_string().contains("cannot be opened"));
    let mut new = good.clone();
    new[4..6].copy_from_slice(&(SAVE_VERSION + 1).to_le_bytes());
    let e = Sim::load(&new).err().unwrap();
    assert!(matches!(e, LoadError::TooNew { .. }));
    assert!(e.to_string().contains("newer version"));
    assert_eq!(
        Sim::load(&good[..good.len() / 2]).err(),
        Some(LoadError::Truncated)
    );
    assert_eq!(Sim::load(&good[..10]).err(), Some(LoadError::Truncated));
    let mut bad = good.clone();
    let mid = bad.len() / 2;
    bad[mid] ^= 0x55;
    assert_eq!(Sim::load(&bad).err(), Some(LoadError::Corrupt));
    assert!(Sim::load(&good).is_ok());
}

#[test]
fn autosave_keeps_the_three_newest_files() {
    let dir = std::env::temp_dir().join(format!("us-sim-autosave-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut s = new_sim(Preset::Grid, Rules::sandbox());
    let mut days = Vec::new();
    for _ in 0..5 {
        for _ in 0..30 {
            s.step_day();
        }
        days.push(s.day);
        let p = s.autosave(&dir).unwrap();
        assert_eq!(p, autosave_path(&dir, 1));
    }
    let day_of = |n: usize| {
        Sim::load(&std::fs::read(autosave_path(&dir, n)).unwrap())
            .unwrap()
            .day
    };
    assert_eq!(day_of(1), days[4]);
    assert_eq!(day_of(2), days[3]);
    assert_eq!(day_of(3), days[2]);
    assert!(!autosave_path(&dir, 4).exists());
    assert_eq!(Sim::load_latest_autosave(&dir).unwrap().day, days[4]);
    // a damaged newest file falls back to the one before
    std::fs::write(autosave_path(&dir, 1), b"junk").unwrap();
    assert_eq!(Sim::load_latest_autosave(&dir).unwrap().day, days[3]);
    let _ = std::fs::remove_dir_all(&dir);
}
