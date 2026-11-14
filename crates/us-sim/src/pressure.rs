//! Pressure on the mayor: approval, elections every four years, bankruptcy, and the game modes
//! that switch them off.

use crate::*;

/// Days between elections (four years).
pub const ELECTION_PERIOD_DAYS: i32 = 1461;
/// Months with an overdrawn treasury and no credit left before the state takes over.
pub const BANKRUPT_MONTHS: u32 = 6;
/// The treasury an unlimited-money game keeps.
pub const UNLIMITED_TREASURY: i64 = 1_000_000_000_000;
/// Share of the vote needed to stay in office, percent.
pub const WIN_SHARE: i32 = 50;

const FIRST_NAMES: [&str; 12] = [
    "Imogen", "Teodor", "Marisol", "Anselm", "Odalys", "Corwin", "Petra", "Lucan", "Ilse",
    "Dorian", "Yara", "Fenwick",
];
const LAST_NAMES: [&str; 12] = [
    "Ashcombe",
    "Brightwater",
    "Calloway",
    "Dunstan",
    "Eastlake",
    "Fairweather",
    "Garrity",
    "Holloway",
    "Ingleby",
    "Juniper",
    "Kestrel",
    "Lockhart",
];

/// Which rules are on. A sandbox game cannot be lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// The state takes over a town that stays broke.
    pub bankruptcy: bool,
    /// Voters hold an election every four years.
    pub elections: bool,
    /// The treasury never runs out.
    pub unlimited_money: bool,
}

impl Rules {
    pub fn standard() -> Rules {
        Rules {
            bankruptcy: true,
            elections: true,
            unlimited_money: false,
        }
    }

    /// No bankruptcy and no elections.
    pub fn sandbox() -> Rules {
        Rules {
            bankruptcy: false,
            elections: false,
            unlimited_money: false,
        }
    }

    /// A sandbox with money that never runs out.
    pub fn sandbox_unlimited() -> Rules {
        Rules {
            unlimited_money: true,
            ..Rules::sandbox()
        }
    }

    pub fn is_sandbox(&self) -> bool {
        !self.bankruptcy && !self.elections
    }
}

/// What voters care about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Issue {
    Taxes,
    /// The least covered service.
    Services(Service),
    Jobs,
    Traffic,
    Debt,
    Fires,
}

impl Issue {
    /// A short phrase for the news feed and the campaign.
    pub fn text(self) -> String {
        match self {
            Issue::Taxes => "high taxes".into(),
            Issue::Services(s) => format!("weak {} cover", s.label().to_lowercase()),
            Issue::Jobs => "too few jobs".into(),
            Issue::Traffic => "traffic jams".into(),
            Issue::Debt => "the town's debt".into(),
            Issue::Fires => "the fires".into(),
        }
    }
}

/// How voters feel, in points. `total` is 0 to 100; each part says how much it moved the total
/// from the starting 70.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Approval {
    pub total: i32,
    pub taxes: i32,
    pub services: i32,
    pub jobs: i32,
    pub traffic: i32,
    pub debt: i32,
    pub fires: i32,
    /// The issue pulling the total down hardest.
    pub worst: Issue,
}

pub const APPROVAL_BASE: i32 = 70;

/// One held election.
#[derive(Clone, Debug)]
pub struct ElectionResult {
    pub day: i32,
    pub approval: i32,
    /// The incumbent's share of the vote, percent.
    pub share: i32,
    pub won: bool,
    /// An invented person who ran against the mayor.
    pub challenger: String,
    /// What the challenger ran on.
    pub issue: Issue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameOverReason {
    /// The treasury stayed empty with no credit left and the state took over.
    Bankrupt,
    /// The voters chose the challenger.
    LostElection {
        challenger: String,
        issue: Issue,
        share: i32,
    },
}

/// How the mayor's career went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CareerSummary {
    pub days: i32,
    pub years: i32,
    pub population: i64,
    pub peak_population: i64,
    pub buildings: u32,
    pub jobs: i64,
    pub treasury: i64,
    pub debt: i64,
    pub elections_won: u32,
    pub fires: u32,
    pub departures: u32,
    pub approval: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameOver {
    pub day: i32,
    pub reason: GameOverReason,
    pub summary: CareerSummary,
}

impl GameOver {
    pub fn headline(&self) -> String {
        match &self.reason {
            GameOverReason::Bankrupt => {
                "The state has taken over the town: it ran out of money and credit.".into()
            }
            GameOverReason::LostElection {
                challenger, share, ..
            } => format!("You lost the election to {challenger} with {share}% of the vote."),
        }
    }
}

impl Sim {
    // ----- rules -----

    pub fn rules(&self) -> Rules {
        self.rules
    }

    pub fn set_rules(&mut self, rules: Rules) {
        self.rules = rules;
        if rules.unlimited_money {
            self.treasury = self.treasury.max(UNLIMITED_TREASURY);
        }
        if !rules.bankruptcy {
            self.broke_months = 0;
        }
    }

    /// A new game with these rules from the first day.
    pub fn new_with_rules(
        town: &Town,
        hall: Pt,
        seed: u64,
        initial_reveals: usize,
        rules: Rules,
    ) -> Sim {
        let mut s = Sim::new_with(town, hall, seed, initial_reveals);
        s.set_rules(rules);
        s
    }

    // ----- game over -----

    /// Set once the game is lost. The clock stops: `step_day` does nothing after this.
    pub fn game_over(&self) -> Option<&GameOver> {
        self.game_over.as_ref()
    }

    pub fn is_game_over(&self) -> bool {
        self.game_over.is_some()
    }

    pub fn career_summary(&self) -> CareerSummary {
        CareerSummary {
            days: self.day,
            years: self.day / 365,
            population: self.population(),
            peak_population: self.peak_population.max(self.population()),
            buildings: self.built_count() as u32,
            jobs: self.jobs(),
            treasury: self.treasury,
            debt: self.debt(),
            elections_won: self.elections_won,
            fires: self.fires_total,
            departures: self.departures,
            approval: self.approval.total,
        }
    }

    fn end_game(&mut self, reason: GameOverReason) {
        let over = GameOver {
            day: self.day,
            reason,
            summary: self.career_summary(),
        };
        let text = over.headline();
        self.log(EventKind::GameOver, text, None);
        self.game_over = Some(over);
    }

    // ----- bankruptcy -----

    /// Months in a row the town has been overdrawn with no credit left.
    pub fn broke_months(&self) -> u32 {
        self.broke_months
    }

    pub(crate) fn check_bankruptcy(&mut self) {
        if !self.rules.bankruptcy {
            self.broke_months = 0;
            return;
        }
        // an overdrawn town is lent money while it has credit left
        while self.treasury < 0 && self.debt() + BOND_SIZE <= self.credit_limit() {
            if self.take_loan().is_err() {
                break;
            }
            self.log(
                EventKind::Loan,
                "The treasury was overdrawn, so the town sold an emergency bond.".into(),
                None,
            );
        }
        let stuck = self.treasury < 0 && self.debt() + BOND_SIZE > self.credit_limit();
        if !stuck {
            self.broke_months = 0;
            return;
        }
        self.broke_months += 1;
        if self.broke_months >= BANKRUPT_MONTHS {
            self.end_game(GameOverReason::Bankrupt);
        } else if self.broke_months == 1 || self.broke_months == BANKRUPT_MONTHS - 2 {
            let left = BANKRUPT_MONTHS - self.broke_months;
            self.log(
                EventKind::Budget,
                format!(
                    "The state auditor is watching: the treasury is overdrawn and credit is used up. {left} months to fix it."
                ),
                None,
            );
        }
    }

    // ----- approval -----

    /// How voters feel today, with what drives it.
    pub fn approval(&self) -> Approval {
        self.compute_approval()
    }

    /// The approval taken at the last monthly close (0 before the first).
    pub fn approval_pct(&self) -> i32 {
        self.approval.total
    }

    pub(crate) fn compute_approval(&self) -> Approval {
        // taxes: homes weigh double, since most voters pay them
        let eff = (2 * self.tax.homes + self.tax.shops + self.tax.industry) as f64 / 4.0 / 10.0;
        let taxes = if eff <= 7.0 {
            ((7.0 - eff) * 1.5).min(8.0)
        } else {
            -((eff - 7.0) * 4.0).min(40.0)
        };
        // services: mean cover of the five budget services over homes
        let mut sums = [0.0f64; 5];
        let mut n = 0.0f64;
        for (i, l) in self.lots.iter().enumerate() {
            if matches!(l.phase, Phase::Built { .. })
                && !l.exempt
                && class_of(l.kind) == Some(Class::Homes)
            {
                n += 1.0;
                for (k, s) in sums.iter_mut().enumerate() {
                    *s += self.coverage[k].get(i).copied().unwrap_or(0.0) as f64;
                }
            }
        }
        let means: Vec<f64> = sums
            .iter()
            .map(|s| if n > 0.0 { s / n } else { 0.5 })
            .collect();
        let mean = means.iter().sum::<f64>() / 5.0;
        let services = (mean - 0.45) * 40.0;
        let mut weakest = 0;
        for k in 1..5 {
            if means[k] < means[weakest] {
                weakest = k;
            }
        }
        let weakest = Service::ALL[weakest];
        // jobs: about one job for every two residents is fine
        let pop = self.population();
        let jobs = if pop > 0 {
            let ratio = self.jobs() as f64 / (pop as f64 * 0.5);
            (ratio.min(1.3) - 0.8) * 12.0
        } else {
            0.0
        };
        let traffic = -(self.avg_cong as f64) * 25.0;
        let limit = self.credit_limit().max(1);
        let mut debt = -(self.debt() as f64 / limit as f64) * 15.0;
        if self.treasury < 0 {
            debt -= 8.0;
        }
        let recent = self
            .fire_days
            .iter()
            .filter(|&&d| self.day - d <= 365)
            .count();
        let fires = -(recent.min(12) as f64) * 0.75;
        let round = |v: f64| libm::floor(v + 0.5) as i32;
        let parts = [
            (Issue::Taxes, round(taxes)),
            (Issue::Services(weakest), round(services)),
            (Issue::Jobs, round(jobs)),
            (Issue::Traffic, round(traffic)),
            (Issue::Debt, round(debt)),
            (Issue::Fires, round(fires)),
        ];
        let sum: i32 = parts.iter().map(|p| p.1).sum();
        let mut worst = parts[0];
        for p in &parts[1..] {
            if p.1 < worst.1 {
                worst = *p;
            }
        }
        Approval {
            total: (APPROVAL_BASE + sum).clamp(0, 100),
            taxes: parts[0].1,
            services: parts[1].1,
            jobs: parts[2].1,
            traffic: parts[3].1,
            debt: parts[4].1,
            fires: parts[5].1,
            worst: worst.0,
        }
    }

    // ----- elections -----

    /// The day of the next election.
    pub fn next_election_day(&self) -> i32 {
        self.next_election
    }

    pub fn next_election_date(&self) -> Date {
        Date::from_days(self.next_election)
    }

    pub fn days_to_election(&self) -> i32 {
        (self.next_election - self.day).max(0)
    }

    pub fn elections_won(&self) -> u32 {
        self.elections_won
    }

    /// The most recent election, if one has been held.
    pub fn last_election(&self) -> Option<&ElectionResult> {
        self.last_election.as_ref()
    }

    pub(crate) fn hold_election_if_due(&mut self) {
        if !self.rules.elections || self.day < self.next_election {
            return;
        }
        let n = self.elections_won as u64 + 1;
        let ap = self.compute_approval();
        let noise = (roll(self.seed ^ 0xe1ec, n, 1) * 9.0) as i32 - 4;
        let share = (ap.total + noise).clamp(0, 100);
        let won = share >= WIN_SHARE;
        let challenger = format!(
            "{} {}",
            FIRST_NAMES[(mix(self.seed, 0xca11, n) % 12) as usize],
            LAST_NAMES[(mix(self.seed, 0xca12, n) % 12) as usize]
        );
        self.next_election += ELECTION_PERIOD_DAYS;
        self.last_election = Some(ElectionResult {
            day: self.day,
            approval: ap.total,
            share,
            won,
            challenger: challenger.clone(),
            issue: ap.worst,
        });
        let issue = ap.worst.text();
        if won {
            self.elections_won += 1;
            self.log(
                EventKind::Election,
                format!(
                    "You won re-election with {share}% of the vote. {challenger} ran on {issue}."
                ),
                None,
            );
        } else {
            self.log(
                EventKind::Election,
                format!("{challenger} ran on {issue} and won the election."),
                None,
            );
            self.end_game(GameOverReason::LostElection {
                challenger,
                issue: ap.worst,
                share,
            });
        }
    }

    /// Called at each monthly close, after the books.
    pub(crate) fn pressure_month(&mut self) {
        self.peak_population = self.peak_population.max(self.population());
        let day = self.day;
        self.fire_days.retain(|&d| day - d <= 400);
        self.approval = self.compute_approval();
        if self.rules.unlimited_money {
            self.treasury = self.treasury.max(UNLIMITED_TREASURY);
        }
        self.check_bankruptcy();
        if self.game_over.is_none() {
            self.hold_election_if_due();
        }
    }
}
