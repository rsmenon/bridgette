use std::fmt;

use serde::{Deserialize, Serialize};

use crate::types::Seat;

use super::bidding::BidSuit;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Contract {
    pub level: u8,
    pub suit: BidSuit,
    pub doubled: bool,
    pub redoubled: bool,
    pub declarer: Seat,
    pub dummy: Seat,
}

impl fmt::Display for Contract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.level, self.suit)?;
        if self.redoubled {
            write!(f, "XX")?;
        } else if self.doubled {
            write!(f, "X")?;
        }
        write!(f, " by {}", self.declarer)
    }
}
