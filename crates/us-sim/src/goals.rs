//! The first-year goal list, and the signs that growth is blocked or about to be.
//!
//! The goals teach the loop of the game. Each one checks itself off from the state of the town
//! at the monthly check and pays a small reward: a grant that shows as its own line in the money
//! panel, or a burst of people at the gate. State is saved in the `GOAL` section; a save from
//! before goals starts with every goal the town has already met ticked off and no reward paid.

use crate::*;

/// The goals, in the order they are shown. Only ever append.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Goal {
    Settle,
    Water,
    School,
    Budget,
    Grow,
    Policy,
}

impl Goal {
    pub const ALL: [Goal; 6] = [
        Goal::Settle,
        Goal::Water,
        Goal::School,
        Goal::Budget,
        Goal::Grow,
        Goal::Policy,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn title(self) -> &'static str {
        match self {
            Goal::Settle => "Reach 300 people",
            Goal::Water => "Build a water tower",
            Goal::School => "Build a school",
            Goal::Budget => "Change a tax rate or a budget",
            Goal::Grow => "Reach 1,000 people",
            Goal::Policy => "Pass an ordinance or draw a district",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Goal::Settle => "Builders start on their own. Keep taxes fair and give them jobs.",
            Goal::Water => "Place a water tower by an open road to make room for more growth.",
            Goal::School => "Homes stop growing when the school seats run out.",
            Goal::Budget => "Adjust department funding or a tax rate and watch the monthly net.",
            Goal::Grow => "Water, power, schools and clinics each hold growth back when full.",
            Goal::Policy => "Unlocks as the town grows. Open Districts or Ordinances.",
        }
    }

    /// The building that gets the goal done, for a shortcut button.
    pub fn civic(self) -> Option<CivicKind> {
        match self {
            Goal::Water => Some(CivicKind::Water),
            Goal::School => Some(CivicKind::School),
            _ => None,
        }
    }

    /// Treasury grant paid when the goal is met.
    fn grant(self) -> i64 {
        match self {
            Goal::Settle => 15_000,
            Goal::Water => 0,
            Goal::School => 20_000,
            Goal::Budget => 5_000,
            Goal::Grow => 40_000,
            Goal::Policy => 25_000,
        }
    }

    /// People added to the arrival queue when the goal is met.
    fn people(self) -> f64 {
        match self {
            Goal::Water => 60.0,
            Goal::Grow => 40.0,
            _ => 0.0,
        }
    }

    pub fn reward_text(self) -> String {
        let mut parts = Vec::new();
        if self.grant() > 0 {
            parts.push(format!("${} grant", dollars(self.grant())));
        }
        if self.people() > 0.0 {
            parts.push(format!("{} more settlers", self.people() as i64));
        }
        parts.join(" and ")
    }
}

/// One line of the list as the player sees it.
#[derive(Clone, Debug)]
pub struct GoalView {
    pub goal: Goal,
    pub done: bool,
    pub title: &'static str,
    pub hint: &'static str,
    pub reward: String,
}

/// What the goals and the growth check remember.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GoalState {
    /// Bit per `Goal`: met.
    pub done: u32,
    /// The player closed the card.
    pub dismissed: bool,
    /// Months in a row some capacity has held growth back.
    pub blocked_months: u32,
    /// Grants paid out at the last monthly check, shown in the money panel.
    pub last_grant: i64,
    /// Grants waiting for the next monthly ledger.
    pub grant_due: i64,
}

/// A capacity that is full or nearly full, with what to build.
#[derive(Clone, Debug)]
pub struct GrowthAlert {
    pub capacity: Capacity,
    /// Full: growth that needs it has stopped. Otherwise it is only close to full.
    pub blocked: bool,
    pub demand: i64,
    pub supply: i64,
    /// The building that adds capacity, and what it costs now.
    pub build: CivicKind,
    pub cost: i64,
    /// One is already going up.
    pub under_construction: bool,
    pub can_afford: bool,
    pub headline: String,
    pub advice: String,
}

impl Sim {
    fn goal_met(&self, g: Goal) -> bool {
        let count = |k: CivicKind| {
            self.lots
                .iter()
                .filter(|l| matches!(l.phase, Phase::Built { .. }) && l.civic == Some(k))
                .count()
        };
        match g {
            Goal::Settle => self.population() >= 300,
            Goal::Water => count(CivicKind::Water) > 0,
            Goal::School => count(CivicKind::School) > 0,
            Goal::Budget => {
                self.budget != [BUDGET_DEFAULT_PCT; 6]
                    || Class::ALL
                        .iter()
                        .any(|&c| self.tax.get(c) != TAX_DEFAULT_X10)
            }
            Goal::Grow => self.population() >= 1000,
            Goal::Policy => !self.dist.is_default(),
        }
    }

    /// The goal list, with what is done.
    pub fn goals(&self) -> Vec<GoalView> {
        Goal::ALL
            .into_iter()
            .map(|g| GoalView {
                goal: g,
                done: self.goal.done & (1 << g.index()) != 0,
                title: g.title(),
                hint: g.hint(),
                reward: g.reward_text(),
            })
            .collect()
    }

    /// Whether the card should still be shown: not closed and not all done.
    pub fn goals_visible(&self) -> bool {
        !self.goal.dismissed && self.goal.done != (1 << Goal::ALL.len()) - 1
    }

    pub fn dismiss_goals(&mut self) {
        self.goal.dismissed = true;
    }

    /// Dollars of goal grants paid in the last monthly ledger.
    pub fn goal_grants_last_month(&self) -> i64 {
        self.goal.last_grant
    }

    /// Months in a row growth has been held back by a full capacity.
    pub fn blocked_months(&self) -> u32 {
        self.goal.blocked_months
    }

    /// Monthly: tick off goals and pay rewards; count the months growth has been blocked.
    pub(crate) fn step_goals(&mut self) {
        if self.stalled.iter().any(|&b| b) {
            self.goal.blocked_months = self.goal.blocked_months.saturating_add(1);
        } else {
            self.goal.blocked_months = 0;
        }
        for g in Goal::ALL {
            let bit = 1u32 << g.index();
            if self.goal.done & bit != 0 || !self.goal_met(g) {
                continue;
            }
            self.goal.done |= bit;
            self.goal.grant_due += g.grant();
            if g.people() > 0.0 {
                let q = self.queue.get(Class::Homes) + g.people();
                self.queue.set(Class::Homes, q);
            }
            self.log(
                EventKind::Milestone,
                format!(
                    "Goal met: {}. Reward: {}.",
                    g.title().to_lowercase(),
                    g.reward_text()
                ),
                None,
            );
        }
    }

    /// Called by the monthly ledger: the grants to pay into the treasury now.
    pub(crate) fn take_goal_grants(&mut self) -> i64 {
        let g = self.goal.grant_due;
        self.goal.grant_due = 0;
        self.goal.last_grant = g;
        g
    }

    /// Approval lost because residents see no new homes while a capacity stays full.
    pub(crate) fn blocked_approval(&self) -> f64 {
        -(self.goal.blocked_months.min(8) as f64) * 0.5
    }

    /// Capacities that are full (blocked) or close to it, worst first.
    pub fn growth_alerts(&self) -> Vec<GrowthAlert> {
        let mut out = Vec::new();
        for c in Capacity::ALL {
            let st = self.committed_status(c);
            let blocked = self.stalled[c.index()];
            if !blocked && st.ratio() < crate::limits::WARN_AT {
                continue;
            }
            let build = c.civic();
            let cost = build.cost();
            let under_construction = self
                .lots
                .iter()
                .any(|l| l.civic == Some(build) && matches!(l.phase, Phase::Building { .. }));
            let what = match c {
                Capacity::Water => "water",
                Capacity::Power => "power",
                Capacity::Schools => "school seats",
                Capacity::Health => "clinic space",
            };
            let (headline, advice) = if blocked {
                (
                    format!("Growth stopped: {what} is full"),
                    if under_construction {
                        format!(
                            "A {} is going up. New {} wait until it opens.",
                            build.label().to_lowercase(),
                            waiting(c)
                        )
                    } else {
                        format!(
                            "New {} wait. Build a {} for ${}.",
                            waiting(c),
                            build.label().to_lowercase(),
                            dollars(cost)
                        )
                    },
                )
            } else {
                (
                    format!("{} is {}% used", cap_name(c), (st.ratio() * 100.0) as i64),
                    if under_construction {
                        format!("A {} is going up.", build.label().to_lowercase())
                    } else {
                        format!(
                            "Build a {} (${}) before growth stops.",
                            build.label().to_lowercase(),
                            dollars(cost)
                        )
                    },
                )
            };
            out.push(GrowthAlert {
                capacity: c,
                blocked,
                demand: st.demand,
                supply: st.supply,
                build,
                cost,
                under_construction,
                can_afford: self.treasury >= cost,
                headline,
                advice,
            });
        }
        out.sort_by_key(|a| !a.blocked);
        out
    }

    // ----- save -----

    pub(crate) fn save_goals(&self, w: &mut crate::save::W) {
        w.u32(self.goal.done);
        w.bool(self.goal.dismissed);
        w.u32(self.goal.blocked_months);
        w.i64(self.goal.last_grant);
        w.i64(self.goal.grant_due);
    }

    pub(crate) fn load_goals(&mut self, r: &mut crate::save::R) -> crate::save::Res<()> {
        self.goal.done = r.u32()?;
        self.goal.dismissed = r.bool()?;
        self.goal.blocked_months = r.u32()?;
        self.goal.last_grant = r.i64()?;
        self.goal.grant_due = r.i64()?;
        Ok(())
    }

    /// A save from before goals: what the town has already met is ticked off with no reward,
    /// and a town past its first year or its first thousand people is not nagged at all.
    pub(crate) fn goals_for_old_save(&mut self) {
        for g in Goal::ALL {
            if self.goal_met(g) {
                self.goal.done |= 1 << g.index();
            }
        }
        if self.day > 365 || self.population() >= 1000 {
            self.goal.done = (1 << Goal::ALL.len()) - 1;
            self.goal.dismissed = true;
        }
    }

    pub(crate) fn goal_hash(&self, f: &mut dyn FnMut(u64)) {
        f(self.goal.done as u64);
        f(self.goal.dismissed as u64);
        f(self.goal.blocked_months as u64);
        f(self.goal.last_grant as u64);
        f(self.goal.grant_due as u64);
    }
}

/// 60000 as 60,000.
pub fn dollars(v: i64) -> String {
    let digits = v.abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{}{out}", if v < 0 { "-" } else { "" })
}

fn waiting(c: Capacity) -> &'static str {
    match c {
        Capacity::Water | Capacity::Power => "homes, shops and firms",
        Capacity::Schools | Capacity::Health => "homes",
    }
}

fn cap_name(c: Capacity) -> &'static str {
    match c {
        Capacity::Water => "Water",
        Capacity::Power => "Power",
        Capacity::Schools => "School seats",
        Capacity::Health => "Clinic space",
    }
}
