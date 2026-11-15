//! Plain-words places for news lines. Roads have no names, so a spot is described by the
//! district it is in, or else by the two kinds of road that meet nearest to it.

use crate::*;

/// How a kind of road reads in a news line, with its article.
fn road_words(c: RoadClass) -> &'static str {
    match c {
        RoadClass::Highway => "a highway",
        RoadClass::Arterial => "a main road",
        RoadClass::Collector => "an avenue",
        RoadClass::Local => "a side street",
        RoadClass::Alley => "an alley",
        RoadClass::Rail => "the railway",
    }
}

fn dist_to_run(p: Pt, pts: &[Pt]) -> f64 {
    let mut best = f64::MAX;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (abx, aby) = (b.x - a.x, b.y - a.y);
        let l2 = abx * abx + aby * aby;
        let t = if l2 < 1e-9 {
            0.0
        } else {
            (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
        };
        best = best.min(p.dist(pt(a.x + abx * t, a.y + aby * t)));
    }
    best
}

/// Storeys of a tall building at a level, as the models are built.
fn storeys(kind: LotKind, level: u8) -> Option<usize> {
    let i = (level as usize).checked_sub(1)?;
    match kind {
        LotKind::Office => [3usize, 5, 7, 10, 14].get(i).copied(),
        LotKind::Apartments => [3usize, 4, 5, 7, 9].get(i).copied(),
        _ => None,
    }
}

impl Sim {
    /// Where lot `i` is, in words: "in Old Town" when it is in a district, otherwise "at the
    /// corner of a main road and a side street" from the nearest open roads.
    pub fn place_phrase(&self, i: usize) -> String {
        if let Some(d) = self.district_of_lot(i)
            && let Some(d) = self.districts().get(d)
        {
            return format!("in {}", d.name);
        }
        let p = self.lots[i].pos;
        let mut first: Option<(f64, usize)> = None;
        for (r, run) in self.runs.iter().enumerate() {
            if run.class == RoadClass::Rail
                || run.class == RoadClass::Alley
                || !self.road_visible(r)
            {
                continue;
            }
            let d = dist_to_run(p, &run.points);
            if d < 90.0 && first.is_none_or(|f| d < f.0) {
                first = Some((d, r));
            }
        }
        let Some((_, r1)) = first else {
            return "on the edge of town".into();
        };
        let a = &self.runs[r1];
        let mut second: Option<(f64, usize)> = None;
        for (r, run) in self.runs.iter().enumerate() {
            if r == r1
                || run.street == a.street
                || run.class == RoadClass::Rail
                || run.class == RoadClass::Alley
                || !self.road_visible(r)
            {
                continue;
            }
            let d = dist_to_run(p, &run.points);
            if d < 70.0 && second.is_none_or(|s| d < s.0) {
                second = Some((d, r));
            }
        }
        match second {
            Some((_, r2)) => {
                let (c1, c2) = (a.class, self.runs[r2].class);
                if c1 == c2 {
                    let w = road_words(c1)
                        .trim_start_matches("a ")
                        .trim_start_matches("an ");
                    let plural = if w == "main road" {
                        "main roads"
                    } else if w == "avenue" {
                        "avenues"
                    } else if w == "side street" {
                        "side streets"
                    } else {
                        "roads"
                    };
                    format!("where two {plural} cross")
                } else {
                    format!("at the corner of {} and {}", road_words(c1), road_words(c2))
                }
            }
            None => format!("on {}", road_words(a.class)),
        }
    }

    /// News for a big job starting on lot `i` (a tall office or apartment building going up),
    /// naming the place.
    pub(crate) fn log_big_job(&mut self, i: usize, level: u8) {
        let kind = self.lots[i].kind;
        if level < 4 {
            return;
        }
        let Some(n) = storeys(kind, level) else {
            return;
        };
        let what = if kind == LotKind::Office {
            "office"
        } else {
            "apartment building"
        };
        let article = if n == 8 || n == 11 || n == 18 {
            "An"
        } else {
            "A"
        };
        let place = self.place_phrase(i);
        let at = self.lots[i].pos;
        self.log(
            EventKind::Upgrade,
            format!("{article} {n} storey {what} is going up {place}."),
            Some(at),
        );
    }
}
