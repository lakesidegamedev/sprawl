//! A town that does things on its own: fires where fire cover is weak, a rival town that sets
//! its own taxes and draws people and firms away, and seasons that sway demand.

use crate::*;

/// Days a fire burns before the lot is cleared and rebuilt.
pub const FIRE_DAYS: i32 = 6;
pub(crate) const RIVAL_NAMES: [&str; 4] = ["Ashford", "Bellmont", "Corvale", "Dunmore"];

pub(crate) fn rival_code(name: &str) -> u8 {
    RIVAL_NAMES.iter().position(|&n| n == name).unwrap_or(0) as u8
}

pub(crate) fn rival_name(code: u8) -> Option<&'static str> {
    RIVAL_NAMES.get(code as usize).copied()
}

#[derive(Clone, Debug)]
pub struct Rival {
    pub name: &'static str,
    /// Its tax rates in tenths of a percent.
    pub tax: ByClass<u32>,
}

impl Rival {
    pub(crate) fn new(seed: u64) -> Rival {
        Rival {
            name: RIVAL_NAMES[(mix(seed, 0x61, 0) % 4) as usize],
            tax: ByClass::all(70),
        }
    }
}

/// Demand swing by month, percent of normal, January first.
fn season_table(c: Class) -> [u32; 12] {
    match c {
        Class::Homes => [96, 96, 98, 101, 104, 106, 106, 105, 102, 99, 97, 96],
        Class::Shops => [92, 94, 97, 99, 101, 102, 102, 103, 100, 103, 108, 112],
        Class::Industry => [98, 99, 101, 102, 103, 102, 100, 100, 102, 103, 100, 97],
    }
}

impl Sim {
    /// The seasonal demand factor now, in percent of normal.
    pub fn season_pct(&self, c: Class) -> u32 {
        season_table(c)[self.date().month as usize - 1]
    }

    pub fn rival(&self) -> &Rival {
        &self.rival
    }
}
