use serde::{Deserialize, Serialize};

use crate::types::Seat;

use super::contract::Contract;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BidSuit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
    NoTrump,
}

impl BidSuit {
    pub const ALL: [BidSuit; 5] = [
        BidSuit::Clubs,
        BidSuit::Diamonds,
        BidSuit::Hearts,
        BidSuit::Spades,
        BidSuit::NoTrump,
    ];

    pub fn short(self) -> &'static str {
        match self {
            BidSuit::Clubs => "♣",
            BidSuit::Diamonds => "♦",
            BidSuit::Hearts => "❤",
            BidSuit::Spades => "♠",
            BidSuit::NoTrump => "NT",
        }
    }
}

impl std::fmt::Display for BidSuit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.short())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bid {
    Suit(u8, BidSuit), // level 1-7, suit
    Pass,
    Double,
    Redouble,
}

impl Bid {
    /// Parse ASCII bid string like "1C", "3NT", "Pass", "Double", "Redouble".
    pub fn from_ascii(s: &str) -> Option<Bid> {
        match s {
            "Pass" => Some(Bid::Pass),
            "Double" | "Dbl" => Some(Bid::Double),
            "Redouble" | "Rdbl" => Some(Bid::Redouble),
            _ => {
                let level = s.chars().next()?.to_digit(10)? as u8;
                if !(1..=7).contains(&level) {
                    return None;
                }
                let suit_str = &s[1..];
                let suit = match suit_str {
                    "C" => BidSuit::Clubs,
                    "D" => BidSuit::Diamonds,
                    "H" => BidSuit::Hearts,
                    "S" => BidSuit::Spades,
                    "NT" => BidSuit::NoTrump,
                    _ => return None,
                };
                Some(Bid::Suit(level, suit))
            }
        }
    }
}

impl std::fmt::Display for Bid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Bid::Suit(level, suit) => write!(f, "{}{}", level, suit),
            Bid::Pass => write!(f, "Pass"),
            Bid::Double => write!(f, "Dbl"),
            Bid::Redouble => write!(f, "Rdbl"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Auction {
    pub dealer: Seat,
    pub bids: Vec<(Seat, Bid)>,
    current_bidder: Seat,
}

impl Auction {
    pub fn new(dealer: Seat) -> Self {
        Self {
            dealer,
            bids: Vec::new(),
            current_bidder: dealer,
        }
    }

    pub fn current_bidder(&self) -> Seat {
        self.current_bidder
    }

    /// Find the last non-pass suit bid.
    fn last_suit_bid(&self) -> Option<(usize, Seat, u8, BidSuit)> {
        for (i, (seat, bid)) in self.bids.iter().enumerate().rev() {
            if let Bid::Suit(level, suit) = bid {
                return Some((i, *seat, *level, *suit));
            }
        }
        None
    }

    /// Check if the last action was a double (by opponents of the current bidder).
    fn last_action_is_double(&self) -> bool {
        for (_, bid) in self.bids.iter().rev() {
            match bid {
                Bid::Pass => continue,
                Bid::Double => return true,
                _ => return false,
            }
        }
        false
    }

    /// Check if the last action is a suit bid (ignoring passes).
    fn last_action_is_suit_bid(&self) -> bool {
        for (_, bid) in self.bids.iter().rev() {
            match bid {
                Bid::Pass => continue,
                Bid::Suit(_, _) => return true,
                _ => return false,
            }
        }
        false
    }

    pub fn is_valid_bid(&self, bid: &Bid) -> bool {
        match bid {
            Bid::Pass => true,
            Bid::Suit(level, suit) => {
                if *level < 1 || *level > 7 {
                    return false;
                }
                match self.last_suit_bid() {
                    None => true,
                    Some((_, _, prev_level, prev_suit)) => {
                        *level > prev_level
                            || (*level == prev_level && *suit > prev_suit)
                    }
                }
            }
            Bid::Double => {
                // Can only double an opponent's suit bid
                if let Some((_, seat, _, _)) = self.last_suit_bid() {
                    // Must be an opponent's bid and last non-pass action must be a suit bid
                    seat.is_ns() != self.current_bidder.is_ns()
                        && self.last_action_is_suit_bid()
                } else {
                    false
                }
            }
            Bid::Redouble => {
                // Can only redouble a double on our side's bid
                if let Some((_, seat, _, _)) = self.last_suit_bid() {
                    seat.is_ns() == self.current_bidder.is_ns()
                        && self.last_action_is_double()
                } else {
                    false
                }
            }
        }
    }

    pub fn place_bid(&mut self, bid: Bid) -> anyhow::Result<()> {
        if !self.is_valid_bid(&bid) {
            anyhow::bail!("Invalid bid: {:?}", bid);
        }
        self.bids.push((self.current_bidder, bid));
        self.current_bidder = self.current_bidder.next();
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        if self.bids.len() < 4 {
            return false;
        }
        // Four initial passes
        if self.bids.len() == 4 && self.bids.iter().all(|(_, b)| *b == Bid::Pass) {
            return true;
        }
        // Three consecutive passes after at least one suit bid
        let len = self.bids.len();
        if len >= 4 {
            let last_three = &self.bids[len - 3..];
            if last_three.iter().all(|(_, b)| *b == Bid::Pass) {
                // Check there was at least one suit bid
                return self.last_suit_bid().is_some();
            }
        }
        false
    }

    pub fn resolve_contract(&self) -> Option<Contract> {
        if !self.is_complete() {
            return None;
        }
        let (_, _, level, bid_suit) = self.last_suit_bid()?;

        // Find declarer: the first player on the winning side to bid that denomination
        let winning_seat = {
            let (_, seat, _, _) = self
                .bids
                .iter()
                .rev()
                .find_map(|(s, b)| {
                    if let Bid::Suit(l, bs) = b {
                        Some((0, *s, *l, *bs))
                    } else {
                        None
                    }
                })
                .unwrap();
            seat
        };

        let declarer = self
            .bids
            .iter()
            .find_map(|(seat, bid)| {
                if let Bid::Suit(_, bs) = bid {
                    if *bs == bid_suit && seat.is_ns() == winning_seat.is_ns() {
                        return Some(*seat);
                    }
                }
                None
            })
            .unwrap();

        // Check doubled/redoubled state: look at non-pass bids after the last suit bid
        let (last_suit_idx, _, _, _) = self.last_suit_bid().unwrap();
        let mut doubled = false;
        let mut redoubled = false;
        for (_, bid) in &self.bids[last_suit_idx + 1..] {
            match bid {
                Bid::Double => {
                    doubled = true;
                    redoubled = false;
                }
                Bid::Redouble => {
                    redoubled = true;
                    doubled = false;
                }
                _ => {}
            }
        }

        Some(Contract {
            level,
            suit: bid_suit,
            doubled,
            redoubled,
            declarer,
            dummy: declarer.partner(),
        })
    }

    /// Return list of valid bids the current bidder can make.
    pub fn valid_bids(&self) -> Vec<Bid> {
        let mut bids = vec![Bid::Pass];
        for level in 1..=7u8 {
            for &suit in &BidSuit::ALL {
                let bid = Bid::Suit(level, suit);
                if self.is_valid_bid(&bid) {
                    bids.push(bid);
                }
            }
        }
        if self.is_valid_bid(&Bid::Double) {
            bids.push(Bid::Double);
        }
        if self.is_valid_bid(&Bid::Redouble) {
            bids.push(Bid::Redouble);
        }
        bids
    }
}
