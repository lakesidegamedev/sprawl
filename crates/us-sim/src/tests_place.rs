use super::*;
use us_gen::{Preset, generate};

#[test]
fn every_lot_has_a_place_in_words() {
    let t = generate(Preset::Grid, 3);
    let mut s = Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::sandbox());
    for _ in 0..200 {
        s.step_day();
    }
    for i in 0..s.lots().len().min(60) {
        let p = s.place_phrase(i);
        assert!(!p.is_empty());
        assert!(!p.contains('\u{2014}') && !p.contains('\u{2013}'));
    }
}

#[test]
fn big_jobs_are_announced_with_their_place() {
    let t = generate(Preset::Grid, 3);
    let mut s = Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::sandbox());
    for _ in 0..200 {
        s.step_day();
    }
    s.take_events();
    let li = (0..s.lots().len())
        .find(|&i| !s.lots()[i].exempt && s.lots()[i].civic.is_none())
        .unwrap();
    s.lots[li].kind = LotKind::Office;
    s.log_big_job(li, 5);
    s.log_big_job(li, 3);
    let lines: Vec<String> = s.take_events().into_iter().map(|e| e.text).collect();
    assert_eq!(lines.len(), 1, "only tall jobs make news: {lines:?}");
    let line = &lines[0];
    assert!(
        line.starts_with("A 14 storey office is going up "),
        "{line}"
    );
    assert!(
        line.contains(" in ")
            || line.contains("corner")
            || line.contains(" on ")
            || line.contains("cross"),
        "no place in: {line}"
    );
}
