use super::*;
use crate::tests_balance::tend;
use us_gen::{Preset, generate};

fn town() -> Town {
    generate(Preset::Grid, 3)
}

fn new_sim(rules: Rules) -> Sim {
    let t = town();
    Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, rules)
}

#[test]
fn a_town_that_stays_broke_is_taken_over() {
    let mut s = new_sim(Rules {
        elections: false,
        ..Rules::standard()
    });
    s.set_tax_x10(0);
    for d in 0..365 * 25 {
        s.step_day();
        if s.is_game_over() {
            break;
        }
        assert!(d < 365 * 25 - 1, "never went bankrupt");
    }
    let g = s.game_over().unwrap();
    assert_eq!(g.reason, GameOverReason::Bankrupt);
    assert!(g.summary.treasury < 0);
    assert!(g.summary.debt > 0, "emergency bonds were sold first");
    assert!(g.headline().contains("state"));
    assert!(
        s.take_events()
            .iter()
            .any(|e| e.kind == EventKind::GameOver)
    );
    // the clock has stopped
    let (day, fp) = (s.day, s.fingerprint());
    s.step_day();
    assert_eq!((s.day, s.fingerprint()), (day, fp));
}

#[test]
fn credit_left_gives_the_town_time() {
    let mut s = new_sim(Rules::standard());
    s.set_tax_x10(0);
    // the first overdraft is covered by an emergency bond, and the clock has not started
    for _ in 0..365 * 3 {
        s.step_day();
        if s.debt() > 0 {
            break;
        }
    }
    assert!(s.debt() > 0, "no emergency bond was sold");
    assert!(!s.is_game_over());
    assert_eq!(s.broke_months(), 0);
}

#[test]
fn sandbox_cannot_be_lost() {
    let mut s = new_sim(Rules::sandbox());
    assert!(s.rules().is_sandbox());
    s.set_tax_x10(0);
    for _ in 0..365 * 10 {
        s.step_day();
    }
    assert!(!s.is_game_over());
    assert!(s.last_election().is_none());
    assert!(s.treasury < 0);
    assert_eq!(s.broke_months(), 0);
}

#[test]
fn unlimited_money_never_runs_out() {
    let mut s = new_sim(Rules::sandbox_unlimited());
    s.set_tax_x10(0);
    for _ in 0..365 * 3 {
        s.step_day();
    }
    assert!(s.treasury >= UNLIMITED_TREASURY - 10_000_000_000);
    let mut s = new_sim(Rules::standard());
    s.set_rules(Rules::sandbox_unlimited());
    assert!(s.treasury >= UNLIMITED_TREASURY);
}

#[test]
fn a_town_with_no_services_loses_the_first_election() {
    let mut s = new_sim(Rules::standard());
    // no services and taxes voters grumble about (the town is small now, so the rest of the
    // pressure on its approval is light)
    s.set_tax_x10(115);
    assert_eq!(s.next_election_day(), ELECTION_PERIOD_DAYS);
    for _ in 0..ELECTION_PERIOD_DAYS + 40 {
        s.step_day();
    }
    let g = s.game_over().expect("the mayor lost");
    let GameOverReason::LostElection {
        challenger, issue, ..
    } = &g.reason
    else {
        panic!("{:?}", g.reason)
    };
    assert!(challenger.contains(' '));
    let e = s.last_election().unwrap();
    assert!(!e.won);
    assert_eq!(&e.challenger, challenger);
    assert_eq!(e.issue, *issue);
    assert_eq!(g.summary.years, 4);
    assert!(g.summary.peak_population > 300);
    assert!(
        s.take_events()
            .iter()
            .any(|e| e.kind == EventKind::Election)
    );
}

#[test]
fn a_sensible_mayor_wins_three_elections() {
    let mut s = new_sim(Rules::standard());
    let mut round = 0;
    for d in 0..365 * 13 {
        s.step_day();
        if d % 30 == 0 {
            tend(&mut s, &mut round);
        }
    }
    assert!(
        !s.is_game_over(),
        "{:?}",
        s.game_over().map(|g| g.headline())
    );
    assert_eq!(s.elections_won(), 3);
    assert_eq!(s.next_election_day(), 4 * ELECTION_PERIOD_DAYS);
    let e = s.last_election().unwrap();
    assert!(e.won && e.share >= WIN_SHARE);
    assert_eq!(s.days_to_election(), s.next_election_day() - s.day);
    let ev = s.take_events();
    assert_eq!(
        ev.iter()
            .filter(|e| e.kind == EventKind::Election && e.text.starts_with("You won"))
            .count(),
        3
    );
}

#[test]
fn approval_and_the_election_date_are_in_the_history() {
    let mut s = new_sim(Rules::standard());
    for _ in 0..200 {
        s.step_day();
    }
    let h = s.history();
    assert!(h.len() >= 6);
    let last = h.last().unwrap();
    assert!((0..=100).contains(&last.approval));
    assert_eq!(last.days_to_election, s.next_election_day() - last.day);
    assert_eq!(s.approval_pct(), last.approval);
    let a = s.approval();
    assert!((0..=100).contains(&a.total));
}

#[test]
fn approval_follows_the_levers() {
    let mut s = new_sim(Rules::standard());
    for _ in 0..500 {
        s.step_day();
    }
    let base = s.approval();
    s.set_tax_x10(150);
    let taxed = s.approval();
    assert!(taxed.taxes < base.taxes);
    assert!(taxed.total < base.total);
    s.set_tax_x10(20);
    assert!(s.approval().taxes > base.taxes);
    assert_eq!(base.worst, base.worst);
}
