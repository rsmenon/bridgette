use crate::types::Seat;

use super::bidding::{Bid, BidSuit};
use super::card::Suit;

/// Constraints on a hand inferred from bidding.
#[derive(Debug, Clone)]
pub struct HandConstraints {
    pub hcp_range: (u32, u32),
    pub suit_lengths: [(u32, u32); 4], // min/max per suit (C, D, H, S order matching Suit::ALL)
    #[allow(dead_code)]
    pub void_suits: Vec<Suit>,
    /// Min/max number of aces (0-4). Set by Blackwood responses.
    pub ace_range: (u32, u32),
}

impl Default for HandConstraints {
    fn default() -> Self {
        Self {
            hcp_range: (0, 40),
            suit_lengths: [(0, 13); 4],
            void_suits: Vec::new(),
            ace_range: (0, 4),
        }
    }
}

impl HandConstraints {
    fn suit_idx(suit: Suit) -> usize {
        match suit {
            Suit::Clubs => 0,
            Suit::Diamonds => 1,
            Suit::Hearts => 2,
            Suit::Spades => 3,
        }
    }

    fn set_hcp(&mut self, min: u32, max: u32) {
        self.hcp_range = (min, max);
    }

    fn set_suit_min(&mut self, suit: Suit, min: u32) {
        let idx = Self::suit_idx(suit);
        self.suit_lengths[idx].0 = self.suit_lengths[idx].0.max(min);
    }

    fn set_suit_max(&mut self, suit: Suit, max: u32) {
        let idx = Self::suit_idx(suit);
        self.suit_lengths[idx].1 = self.suit_lengths[idx].1.min(max);
    }

    fn narrow_hcp(&mut self, min: u32, max: u32) {
        self.hcp_range.0 = self.hcp_range.0.max(min);
        self.hcp_range.1 = self.hcp_range.1.min(max);
    }
}

fn bid_suit_to_suit(bs: BidSuit) -> Option<Suit> {
    match bs {
        BidSuit::Clubs => Some(Suit::Clubs),
        BidSuit::Diamonds => Some(Suit::Diamonds),
        BidSuit::Hearts => Some(Suit::Hearts),
        BidSuit::Spades => Some(Suit::Spades),
        BidSuit::NoTrump => None,
    }
}

/// Classify what role a seat's first bid plays in the auction.
#[derive(Debug, PartialEq)]
enum BidRole {
    Opener,
    Responder,   // Partner opened before us
    Overcaller,  // Opponent opened before us (and partner hasn't bid a suit)
}

/// Derive hand constraints for a seat from the auction so far.
pub fn constraints_from_bids(
    seat: Seat,
    bids: &[(Seat, Bid)],
    _vulnerability: crate::types::Vulnerability,
) -> HandConstraints {
    let mut constraints = HandConstraints::default();

    // Find this seat's bids
    let seat_bids: Vec<&Bid> = bids.iter()
        .filter(|(s, _)| *s == seat)
        .map(|(_, b)| b)
        .collect();

    if seat_bids.is_empty() {
        return constraints;
    }

    let first_bid = seat_bids[0];
    let partner = seat.partner();

    // Classify this seat's role: opener, responder, or overcaller
    let role = classify_bid_role(seat, bids);

    // Find partner's first suit bid (if any) to detect raises
    let partner_suit: Option<BidSuit> = bids.iter()
        .find_map(|(s, b)| {
            if *s == partner {
                if let Bid::Suit(_, bs) = b {
                    return Some(*bs);
                }
            }
            None
        });

    match first_bid {
        Bid::Pass => {
            // Pass as first action: typically 0-11 HCP (didn't open)
            if role == BidRole::Opener {
                constraints.set_hcp(0, 11);
            }
            // Pass as overcaller: opponent opened and this seat didn't overcall.
            // Implies sub-overcall strength — likely 0-7 HCP or no 5-card suit.
            // Use a conservative 0-15 cap (they might have a trap pass with 12+
            // but no suit to overcall in, so we can't go as tight as 0-7).
            if role == BidRole::Overcaller {
                constraints.set_hcp(0, 15);
            }
        }
        Bid::Suit(level, suit) => {
            match role {
                BidRole::Opener => {
                    apply_opening_constraints(&mut constraints, *level, *suit);
                }
                BidRole::Responder => {
                    // Skip generic response constraints for conventional bids
                    // (Stayman 2♣, transfers 2♦/2♥ after partner's 1NT)
                    if !is_conventional_response(seat, bids) {
                        let is_raise = partner_suit == Some(*suit);
                        apply_response_constraints(&mut constraints, *level, *suit, is_raise, partner_suit);
                    }
                }
                BidRole::Overcaller => {
                    apply_overcall_constraints(&mut constraints, *level, *suit);
                }
            }
        }
        Bid::Double => {
            apply_double_constraints(&mut constraints, seat, bids);
        }
        Bid::Redouble => {}
    }

    // Detect conventional sequences (multi-bid patterns)
    apply_blackwood_constraints(&mut constraints, seat, bids);
    apply_gerber_constraints(&mut constraints, seat, bids);
    apply_stayman_constraints(&mut constraints, seat, bids);
    apply_transfer_constraints(&mut constraints, seat, bids);
    apply_stayman_reply_constraints(&mut constraints, seat, bids);
    apply_strong_2c_response_constraints(&mut constraints, seat, bids);
    apply_rebid_constraints(&mut constraints, seat, bids);
    apply_doubler_suit_constraints(&mut constraints, seat, bids);
    apply_advance_constraints(&mut constraints, seat, bids);

    constraints
}

/// Determine whether a seat is the opener, responder to partner, or overcaller.
fn classify_bid_role(seat: Seat, bids: &[(Seat, Bid)]) -> BidRole {
    let partner = seat.partner();
    let mut partner_acted = false;
    let mut opponent_bid_suit = false;

    for (s, b) in bids {
        if *s == seat {
            break;
        }
        if *s == partner && matches!(b, Bid::Suit(_, _) | Bid::Double) {
            partner_acted = true;
        } else if matches!(b, Bid::Suit(_, _)) {
            opponent_bid_suit = true;
        }
    }

    if !partner_acted && !opponent_bid_suit {
        BidRole::Opener
    } else if partner_acted {
        BidRole::Responder
    } else {
        BidRole::Overcaller
    }
}

fn apply_opening_constraints(constraints: &mut HandConstraints, level: u8, suit: BidSuit) {
    match (level, suit) {
        // 1-level openings
        (1, BidSuit::Clubs) => {
            constraints.set_hcp(12, 21);
            constraints.set_suit_min(Suit::Clubs, 3);
        }
        (1, BidSuit::Diamonds) => {
            constraints.set_hcp(12, 21);
            constraints.set_suit_min(Suit::Diamonds, 4);
        }
        (1, BidSuit::Hearts) => {
            constraints.set_hcp(12, 21);
            constraints.set_suit_min(Suit::Hearts, 5);
        }
        (1, BidSuit::Spades) => {
            constraints.set_hcp(12, 21);
            constraints.set_suit_min(Suit::Spades, 5);
        }
        (1, BidSuit::NoTrump) => {
            constraints.set_hcp(15, 17);
            // Balanced: no suit shorter than 2
            for &s in &Suit::ALL {
                constraints.set_suit_min(s, 2);
            }
        }
        // Weak twos
        (2, BidSuit::Diamonds) => {
            constraints.set_hcp(5, 11);
            constraints.set_suit_min(Suit::Diamonds, 6);
        }
        (2, BidSuit::Hearts) => {
            constraints.set_hcp(5, 11);
            constraints.set_suit_min(Suit::Hearts, 6);
        }
        (2, BidSuit::Spades) => {
            constraints.set_hcp(5, 11);
            constraints.set_suit_min(Suit::Spades, 6);
        }
        // Strong 2C
        (2, BidSuit::Clubs) => {
            constraints.set_hcp(22, 40);
        }
        // 2NT
        (2, BidSuit::NoTrump) => {
            constraints.set_hcp(20, 21);
            for &s in &Suit::ALL {
                constraints.set_suit_min(s, 2);
            }
        }
        // 3-level preempts (suit bids only)
        (3, BidSuit::NoTrump) => {
            // 3NT opening: gambling or 25-27 balanced (SAYC uses 25-27)
            constraints.set_hcp(25, 27);
            for &s in &Suit::ALL {
                constraints.set_suit_min(s, 2);
            }
        }
        (3, suit) => {
            if let Some(s) = bid_suit_to_suit(suit) {
                constraints.set_hcp(5, 11);
                constraints.set_suit_min(s, 7);
            }
        }
        _ => {}
    }
}

fn apply_response_constraints(
    constraints: &mut HandConstraints,
    level: u8,
    suit: BidSuit,
    is_raise: bool,
    partner_suit: Option<BidSuit>,
) {
    if is_raise {
        // Raising partner's suit: 3+ support
        if let Some(s) = bid_suit_to_suit(suit) {
            constraints.set_suit_min(s, 3);
        }
        match level {
            2 => constraints.set_hcp(6, 10),       // Simple raise
            3 => constraints.set_hcp(10, 12),      // Limit raise
            4 => constraints.set_hcp(6, 9),        // Preemptive game raise (5+ support typical)
            _ => constraints.set_hcp(6, 40),
        }
        // Game raise or higher often implies 4+ support
        if level >= 4 {
            if let Some(s) = bid_suit_to_suit(suit) {
                constraints.set_suit_min(s, 4);
            }
        }
    } else {
        // New suit or NT response
        // NOTE: NT arms must come before generic suit arms to avoid being shadowed
        match (level, suit) {
            // 1NT response: 6-10 HCP, denies fit for partner's major
            (1, BidSuit::NoTrump) => {
                constraints.set_hcp(6, 10);
            }
            // 2NT response: Jacoby 2NT if partner opened a major (13+, 4+ support, game forcing)
            //               Otherwise 13-15 HCP invitational
            (2, BidSuit::NoTrump) => {
                let partner_opened_major = matches!(
                    partner_suit,
                    Some(BidSuit::Hearts) | Some(BidSuit::Spades)
                );
                if partner_opened_major {
                    // Jacoby 2NT: 13+ support points, 4+ card support
                    constraints.set_hcp(13, 40);
                    if let Some(ps) = partner_suit {
                        if let Some(s) = bid_suit_to_suit(ps) {
                            constraints.set_suit_min(s, 4);
                        }
                    }
                } else {
                    constraints.set_hcp(13, 15);
                }
            }
            // 3NT response: 16-18 HCP (or 16-17 over minor)
            (3, BidSuit::NoTrump) => {
                constraints.set_hcp(16, 18);
            }
            // New suit at 1-level: 6+ HCP, 4+ cards
            (1, suit) => {
                constraints.set_hcp(6, 40);
                if let Some(s) = bid_suit_to_suit(suit) {
                    constraints.set_suit_min(s, 4);
                }
            }
            // New suit at 2-level: 10+ HCP, 4+ cards (forcing one round)
            (2, suit) => {
                constraints.set_hcp(10, 40);
                if let Some(s) = bid_suit_to_suit(suit) {
                    constraints.set_suit_min(s, 4);
                }
            }
            // Jump shift (e.g. 1H -> 3C): 18-19+ HCP, strong suit, slam interest
            // New suit at 3+ level
            (_, BidSuit::NoTrump) => {
                constraints.set_hcp(10, 40);
            }
            (_, suit) => {
                // Jump shift or new suit at high level: strong hand
                constraints.set_hcp(10, 40);
                if let Some(s) = bid_suit_to_suit(suit) {
                    constraints.set_suit_min(s, 4);
                }
            }
        }
    }
}

/// Detect Blackwood 4NT and constrain ace count from the response.
/// Standard Blackwood responses: 5♣=0/4, 5♦=1, 5♥=2, 5♠=3 aces.
fn apply_blackwood_constraints(
    constraints: &mut HandConstraints,
    seat: Seat,
    bids: &[(Seat, Bid)],
) {
    let partner = seat.partner();

    // Look for partner bidding 4NT followed by this seat's response
    for (i, (s, b)) in bids.iter().enumerate() {
        if *s == partner && *b == Bid::Suit(4, BidSuit::NoTrump) {
            // Find this seat's next bid after the 4NT
            for (_, (rs, rb)) in bids.iter().enumerate().skip(i + 1) {
                if *rs == seat {
                    if let Bid::Suit(5, resp_suit) = rb {
                        let aces = match resp_suit {
                            BidSuit::Clubs => 0,    // 0 or 4 — contextually almost always 0
                            BidSuit::Diamonds => 1,
                            BidSuit::Hearts => 2,
                            BidSuit::Spades => 3,
                            _ => break,
                        };
                        constraints.ace_range = (aces, aces);
                    }
                    break;
                }
            }
        }
    }
}

/// Detect Gerber 4♣ over a NT opening and constrain ace count from the response.
/// Gerber responses: 4♦=0, 4♥=1, 4♠=2, 4NT=3 aces.
fn apply_gerber_constraints(
    constraints: &mut HandConstraints,
    seat: Seat,
    bids: &[(Seat, Bid)],
) {
    let partner = seat.partner();

    // Look for partner bidding 4♣ after this seat opened NT
    for (i, (s, b)) in bids.iter().enumerate() {
        if *s != partner || *b != Bid::Suit(4, BidSuit::Clubs) {
            continue;
        }
        // Check that this seat opened with NT (1NT, 2NT, or 3NT)
        let seat_opened_nt = bids.iter().any(|(bs, bb)| {
            *bs == seat && matches!(bb, Bid::Suit(1 | 2 | 3, BidSuit::NoTrump))
        });
        if !seat_opened_nt {
            continue;
        }
        // Find this seat's response after partner's 4♣
        for (_, (rs, rb)) in bids.iter().enumerate().skip(i + 1) {
            if *rs == seat {
                if let Bid::Suit(4, resp_suit) = rb {
                    let aces = match resp_suit {
                        BidSuit::Diamonds => 0,
                        BidSuit::Hearts => 1,
                        BidSuit::Spades => 2,
                        BidSuit::NoTrump => 3,
                        _ => break,
                    };
                    constraints.ace_range = (aces, aces);
                }
                break;
            }
        }
    }
}

/// Check if a seat's first bid is a conventional response (Stayman, transfer, or Gerber)
/// after partner's NT opening, so the generic response handler should be skipped.
fn is_conventional_response(seat: Seat, bids: &[(Seat, Bid)]) -> bool {
    let partner = seat.partner();

    // Find this seat's first suit bid
    let first_bid = bids.iter().find_map(|(s, b)| {
        if *s == seat { if let Bid::Suit(lvl, suit) = b { return Some((*lvl, *suit)); } }
        None
    });

    let (level, suit) = match first_bid {
        Some(v) => v,
        None => return false,
    };

    let first_bid_idx = bids.iter().position(|(s, b)| {
        *s == seat && *b == Bid::Suit(level, suit)
    }).unwrap_or(0);

    // Gerber: 4♣ after partner opened any NT (1NT, 2NT, 3NT)
    if level == 4 && suit == BidSuit::Clubs {
        let partner_opened_nt = bids[..first_bid_idx].iter().rev().any(|(bs, bb)| {
            *bs == partner && matches!(bb, Bid::Suit(1 | 2 | 3, BidSuit::NoTrump))
        });
        if partner_opened_nt {
            return true;
        }
    }

    // Must be a 2-level bid after partner's 1NT for Stayman/transfers
    if level != 2 {
        return false;
    }

    let partner_opened_1nt = bids[..first_bid_idx].iter().rev().any(|(bs, bb)| {
        *bs == partner && *bb == Bid::Suit(1, BidSuit::NoTrump)
    });

    if !partner_opened_1nt {
        return false;
    }

    // 2♣ = Stayman, 2♦ = transfer to hearts, 2♥ = transfer to spades
    matches!(suit, BidSuit::Clubs | BidSuit::Diamonds | BidSuit::Hearts)
}

/// Detect Stayman convention: 2♣ response to partner's 1NT opening.
/// Implies responder has 4+ cards in at least one major and 8+ HCP (invitational+).
fn apply_stayman_constraints(
    constraints: &mut HandConstraints,
    seat: Seat,
    bids: &[(Seat, Bid)],
) {
    let partner = seat.partner();

    // Check if this seat bid 2♣ and partner opened 1NT
    for (i, (s, b)) in bids.iter().enumerate() {
        if *s != seat || *b != Bid::Suit(2, BidSuit::Clubs) {
            continue;
        }
        // Was partner's most recent bid before this 1NT?
        let partner_opened_1nt = bids[..i].iter().rev().any(|(bs, bb)| {
            *bs == partner && *bb == Bid::Suit(1, BidSuit::NoTrump)
        });
        if partner_opened_1nt {
            constraints.hcp_range.0 = constraints.hcp_range.0.max(8);
            // Has 4+ in at least one major — we can't express OR in constraints,
            // but we know they have some major length. No suit constraint applied
            // since we don't know which major.
            return;
        }
    }
}

/// Detect Jacoby Transfer: 2♦ or 2♥ response to partner's 1NT opening.
/// 2♦ = 5+ hearts, 2♥ = 5+ spades.
fn apply_transfer_constraints(
    constraints: &mut HandConstraints,
    seat: Seat,
    bids: &[(Seat, Bid)],
) {
    let partner = seat.partner();

    for (i, (s, b)) in bids.iter().enumerate() {
        if *s != seat {
            continue;
        }
        let (transfer_suit, level_check) = match b {
            Bid::Suit(2, BidSuit::Diamonds) => (Suit::Hearts, true),
            Bid::Suit(2, BidSuit::Hearts) => (Suit::Spades, true),
            _ => continue,
        };
        if !level_check {
            continue;
        }
        // Was partner's most recent bid before this 1NT?
        let partner_opened_1nt = bids[..i].iter().rev().any(|(bs, bb)| {
            *bs == partner && *bb == Bid::Suit(1, BidSuit::NoTrump)
        });
        if partner_opened_1nt {
            constraints.set_suit_min(transfer_suit, 5);
            return;
        }
    }
}

/// Constraints for an overcall (opponent opened, this seat bids a new suit).
/// Constraints for takeout doubles and negative doubles.
fn apply_double_constraints(
    constraints: &mut HandConstraints,
    seat: Seat,
    bids: &[(Seat, Bid)],
) {
    let partner = seat.partner();
    // Only check if partner bid a suit BEFORE this seat's double
    let double_idx = bids.iter().position(|(s, b)| *s == seat && *b == Bid::Double)
        .unwrap_or(0);
    let partner_bid_suit_before = bids[..double_idx].iter().any(|(s, b)| {
        *s == partner && matches!(b, Bid::Suit(_, _))
    });

    if partner_bid_suit_before {
        // Negative double: partner opened, opponent overcalled, we double
        // 7+ HCP, typically 4+ in unbid major(s)
        constraints.narrow_hcp(7, 40);
    } else {
        // Takeout double: opponent opened, we double
        // 12+ HCP (or any shape with 18+), support for unbid suits
        constraints.narrow_hcp(12, 40);
    }
}

/// Stayman opener reply: after partner bid 2♣ (Stayman) over our 1NT.
/// 2♦ = no 4-card major, 2♥ = 4+ hearts, 2♠ = 4+ spades (no 4 hearts).
fn apply_stayman_reply_constraints(
    constraints: &mut HandConstraints,
    seat: Seat,
    bids: &[(Seat, Bid)],
) {
    let partner = seat.partner();

    // Did this seat open 1NT?
    let opened_1nt = bids.iter().any(|(s, b)| {
        *s == seat && *b == Bid::Suit(1, BidSuit::NoTrump)
    });
    if !opened_1nt {
        return;
    }

    // Did partner bid 2♣ (Stayman)?
    let partner_stayman = bids.iter().any(|(s, b)| {
        *s == partner && *b == Bid::Suit(2, BidSuit::Clubs)
    });
    if !partner_stayman {
        return;
    }

    // Find this seat's reply after partner's Stayman
    for (i, (s, b)) in bids.iter().enumerate() {
        if *s == partner && *b == Bid::Suit(2, BidSuit::Clubs) {
            // Find our next bid
            for (_, (rs, rb)) in bids.iter().enumerate().skip(i + 1) {
                if *rs == seat {
                    match rb {
                        Bid::Suit(2, BidSuit::Diamonds) => {
                            // Denies 4-card major
                            constraints.set_suit_max(Suit::Hearts, 3);
                            constraints.set_suit_max(Suit::Spades, 3);
                        }
                        Bid::Suit(2, BidSuit::Hearts) => {
                            constraints.set_suit_min(Suit::Hearts, 4);
                        }
                        Bid::Suit(2, BidSuit::Spades) => {
                            constraints.set_suit_min(Suit::Spades, 4);
                            // Bid spades = has spades but NOT hearts
                            constraints.set_suit_max(Suit::Hearts, 3);
                        }
                        _ => {}
                    }
                    return;
                }
            }
        }
    }
}

/// Strong 2♣ response constraints.
/// 2♦ = waiting (0-7 HCP), 2H/2S/3C/3D = positive (8+ HCP, 5+ suit),
/// 2NT = positive balanced (8+ HCP).
fn apply_strong_2c_response_constraints(
    constraints: &mut HandConstraints,
    seat: Seat,
    bids: &[(Seat, Bid)],
) {
    let partner = seat.partner();

    // Did partner open 2♣?
    let partner_opened_2c = bids.iter().any(|(s, b)| {
        *s == partner && *b == Bid::Suit(2, BidSuit::Clubs)
    });
    if !partner_opened_2c {
        return;
    }

    // Find this seat's first bid after partner's 2♣
    for (i, (s, b)) in bids.iter().enumerate() {
        if *s == partner && *b == Bid::Suit(2, BidSuit::Clubs) {
            for (_, (rs, rb)) in bids.iter().enumerate().skip(i + 1) {
                if *rs == seat {
                    match rb {
                        Bid::Suit(2, BidSuit::Diamonds) => {
                            // Waiting/negative: 0-7 HCP
                            constraints.narrow_hcp(0, 7);
                        }
                        Bid::Suit(2, BidSuit::NoTrump) => {
                            // Positive balanced
                            constraints.narrow_hcp(8, 40);
                        }
                        Bid::Suit(2, suit) | Bid::Suit(3, suit) => {
                            // Positive with a suit
                            constraints.narrow_hcp(8, 40);
                            if let Some(s) = bid_suit_to_suit(*suit) {
                                constraints.set_suit_min(s, 5);
                            }
                        }
                        _ => {}
                    }
                    return;
                }
            }
        }
    }
}

/// Opener rebid constraints — refine the opener's hand based on their second bid.
fn apply_rebid_constraints(
    constraints: &mut HandConstraints,
    seat: Seat,
    bids: &[(Seat, Bid)],
) {
    // Only applies to openers who bid twice
    let seat_bids: Vec<(usize, &Bid)> = bids.iter()
        .enumerate()
        .filter(|(_, (s, _))| *s == seat)
        .map(|(i, (_, b))| (i, b))
        .collect();

    if seat_bids.len() < 2 {
        return;
    }

    let first = seat_bids[0].1;
    let second = seat_bids[1].1;

    // Only handle openers with suit openings
    let (open_level, open_suit) = match first {
        Bid::Suit(l, s) => (*l, *s),
        _ => return,
    };

    // Check that this seat was the opener
    let role = classify_bid_role(seat, bids);
    if role != BidRole::Opener {
        return;
    }

    match second {
        Bid::Suit(rebid_level, rebid_suit) => {
            let rebid_level = *rebid_level;
            let rebid_suit = *rebid_suit;

            // Rebid own suit: 6+ cards (unless at 1-level)
            if rebid_suit == open_suit && rebid_level > open_level {
                if let Some(s) = bid_suit_to_suit(open_suit) {
                    constraints.set_suit_min(s, 6);
                }
                // Jump rebid own suit = 15-17 HCP
                if rebid_level >= open_level + 2 {
                    constraints.narrow_hcp(15, 17);
                }
            }

            // Rebid 1NT after 1-suit opening: 12-14 balanced
            if rebid_level == 1 && rebid_suit == BidSuit::NoTrump {
                constraints.narrow_hcp(12, 14);
                for &s in &Suit::ALL {
                    constraints.set_suit_min(s, 2);
                }
            }

            // Jump to 2NT: 18-19 balanced
            if rebid_level == 2 && rebid_suit == BidSuit::NoTrump && open_level == 1 {
                constraints.narrow_hcp(18, 19);
                for &s in &Suit::ALL {
                    constraints.set_suit_min(s, 2);
                }
            }

            // New suit at rebid: 4+ cards in that suit
            if rebid_suit != open_suit && rebid_suit != BidSuit::NoTrump {
                if let Some(s) = bid_suit_to_suit(rebid_suit) {
                    constraints.set_suit_min(s, 4);
                }
            }
        }
        _ => {}
    }
}

/// Constraints for advancing partner's takeout double.
/// Responding to a double is different from normal responses — can be very weak.
fn apply_advance_constraints(
    constraints: &mut HandConstraints,
    seat: Seat,
    bids: &[(Seat, Bid)],
) {
    let partner = seat.partner();

    // Check if partner doubled before this seat's first bid
    let seat_first_bid_idx = bids.iter().position(|(s, _)| *s == seat);
    let partner_doubled_before = match seat_first_bid_idx {
        Some(idx) => bids[..idx].iter().any(|(s, b)| *s == partner && *b == Bid::Double),
        None => false,
    };

    if !partner_doubled_before {
        return;
    }

    let first_bid = bids.iter().find(|(s, _)| *s == seat).map(|(_, b)| b);
    match first_bid {
        Some(Bid::Suit(1, BidSuit::NoTrump)) => {
            constraints.set_hcp(6, 10);
        }
        Some(Bid::Suit(level, suit)) => {
            if let Some(s) = bid_suit_to_suit(*suit) {
                constraints.set_suit_min(s, 4);
            }
            // Advancing at minimum level can be 0+ HCP (forced); jump = invitational
            match level {
                1 | 2 => constraints.set_hcp(0, 40),  // cheapest level could be weak
                _ => constraints.set_hcp(10, 40),      // jump advance = invitational
            }
        }
        _ => {}
    }
}

/// When a seat's first bid is a Double and they later bid a suit, constrain that suit.
/// E.g., double then 3♠ shows 4+ spades (5+ if at the 3-level or higher).
fn apply_doubler_suit_constraints(
    constraints: &mut HandConstraints,
    seat: Seat,
    bids: &[(Seat, Bid)],
) {
    let seat_bids: Vec<&Bid> = bids.iter()
        .filter(|(s, _)| *s == seat)
        .map(|(_, b)| b)
        .collect();

    if seat_bids.is_empty() || *seat_bids[0] != Bid::Double {
        return;
    }

    // Look at subsequent bids for suit information
    for &bid in &seat_bids[1..] {
        if let Bid::Suit(level, suit) = bid {
            if let Some(s) = bid_suit_to_suit(*suit) {
                // At 3+ level, likely 5+ cards; at 2-level or 1-level, 4+
                let min_length = if *level >= 3 { 5 } else { 4 };
                constraints.set_suit_min(s, min_length);
            }
            break; // Only first suit bid after the double
        }
    }
}

/// Constraints for an overcall (opponent opened, this seat bids a new suit).
fn apply_overcall_constraints(constraints: &mut HandConstraints, level: u8, suit: BidSuit) {
    match (level, suit) {
        // 1NT overcall: 15-18 HCP, balanced, stopper in opener's suit
        (1, BidSuit::NoTrump) => {
            constraints.set_hcp(15, 18);
            for &s in &Suit::ALL {
                constraints.set_suit_min(s, 2);
            }
        }
        // Simple overcall at 1-level: 8-16 HCP, 5+ card suit
        (1, suit) => {
            constraints.set_hcp(8, 16);
            if let Some(s) = bid_suit_to_suit(suit) {
                constraints.set_suit_min(s, 5);
            }
        }
        // Simple overcall at 2-level: 10-16 HCP, 5+ card suit (needs more values)
        (2, BidSuit::NoTrump) => {
            // Unusual 2NT: 5-5+ in two lowest unbid suits, typically 5-11 HCP
            constraints.set_hcp(5, 15);
        }
        (2, suit) => {
            constraints.set_hcp(10, 16);
            if let Some(s) = bid_suit_to_suit(suit) {
                constraints.set_suit_min(s, 5);
            }
        }
        // Jump overcall (preemptive): weak hand, long suit
        (3, suit) => {
            if let Some(s) = bid_suit_to_suit(suit) {
                constraints.set_hcp(5, 11);
                constraints.set_suit_min(s, 6);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Vulnerability;

    #[test]
    fn pass_constrains_hcp() {
        let bids = vec![(Seat::North, Bid::Pass)];
        let c = constraints_from_bids(Seat::North, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (0, 11));
    }

    #[test]
    fn one_heart_opening() {
        let bids = vec![(Seat::North, Bid::Suit(1, BidSuit::Hearts))];
        let c = constraints_from_bids(Seat::North, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (12, 21));
        // Hearts (index 2) should have min 5
        assert_eq!(c.suit_lengths[2].0, 5);
    }

    #[test]
    fn one_nt_opening() {
        let bids = vec![(Seat::East, Bid::Suit(1, BidSuit::NoTrump))];
        let c = constraints_from_bids(Seat::East, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (15, 17));
        // All suits min 2 (balanced)
        for i in 0..4 {
            assert!(c.suit_lengths[i].0 >= 2);
        }
    }

    #[test]
    fn strong_two_clubs() {
        let bids = vec![(Seat::West, Bid::Suit(2, BidSuit::Clubs))];
        let c = constraints_from_bids(Seat::West, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (22, 40));
    }

    #[test]
    fn weak_two_spades() {
        let bids = vec![(Seat::South, Bid::Suit(2, BidSuit::Spades))];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (5, 11));
        assert_eq!(c.suit_lengths[3].0, 6); // Spades
    }

    #[test]
    fn response_new_suit() {
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(1, BidSuit::Spades)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (6, 40));
        assert_eq!(c.suit_lengths[3].0, 4); // Spades min 4
    }

    #[test]
    fn raise_partners_suit() {
        // South opens 1H, West passes, North raises to 3H (limit raise)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(3, BidSuit::Hearts)),
        ];
        let c = constraints_from_bids(Seat::North, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (10, 12)); // Limit raise
        assert_eq!(c.suit_lengths[2].0, 3); // Hearts min 3 (support)
    }

    #[test]
    fn simple_raise() {
        // North opens 1S, East passes, South raises to 2S
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Spades)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(2, BidSuit::Spades)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (6, 10)); // Simple raise
        assert_eq!(c.suit_lengths[3].0, 3); // Spades min 3
    }

    #[test]
    fn no_bids_gives_default() {
        let bids = vec![];
        let c = constraints_from_bids(Seat::North, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (0, 40));
    }

    #[test]
    fn overcall_at_one_level() {
        // North opens 1H, East overcalls 1S
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::East, Bid::Suit(1, BidSuit::Spades)),
        ];
        let c = constraints_from_bids(Seat::East, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (8, 16));
        assert_eq!(c.suit_lengths[3].0, 5); // Spades min 5
    }

    #[test]
    fn overcall_at_two_level() {
        // North opens 1S, East overcalls 2H
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Spades)),
            (Seat::East, Bid::Suit(2, BidSuit::Hearts)),
        ];
        let c = constraints_from_bids(Seat::East, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (10, 16));
        assert_eq!(c.suit_lengths[2].0, 5); // Hearts min 5
    }

    #[test]
    fn one_nt_overcall() {
        // South opens 1D, West overcalls 1NT
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::Diamonds)),
            (Seat::West, Bid::Suit(1, BidSuit::NoTrump)),
        ];
        let c = constraints_from_bids(Seat::West, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (15, 18));
        for i in 0..4 {
            assert!(c.suit_lengths[i].0 >= 2); // Balanced
        }
    }

    #[test]
    fn jacoby_2nt_response() {
        // North opens 1H, East passes, South bids 2NT (Jacoby)
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(2, BidSuit::NoTrump)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (13, 40)); // 13+ support points
        assert_eq!(c.suit_lengths[2].0, 4); // Hearts min 4 (support)
    }

    #[test]
    fn one_nt_response_to_major() {
        // North opens 1H, East passes, South bids 1NT
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(1, BidSuit::NoTrump)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (6, 10)); // 6-10 HCP, denies fit
    }

    #[test]
    fn three_nt_opening() {
        let bids = vec![(Seat::North, Bid::Suit(3, BidSuit::NoTrump))];
        let c = constraints_from_bids(Seat::North, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (25, 27));
        for i in 0..4 {
            assert!(c.suit_lengths[i].0 >= 2); // Balanced
        }
    }

    #[test]
    fn west_overcall_after_south_opens() {
        // South opens 1H, West overcalls 2C
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::West, Bid::Suit(2, BidSuit::Clubs)),
        ];
        let c = constraints_from_bids(Seat::West, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (10, 16));
        assert_eq!(c.suit_lengths[0].0, 5); // Clubs min 5
    }

    #[test]
    fn blackwood_response_zero_aces() {
        // E bids 4NT (Blackwood), W responds 5C (0 aces)
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::East, Bid::Double),
            (Seat::South, Bid::Pass),
            (Seat::West, Bid::Suit(2, BidSuit::Clubs)),
            (Seat::North, Bid::Suit(2, BidSuit::Hearts)),
            (Seat::East, Bid::Suit(3, BidSuit::Spades)),
            (Seat::South, Bid::Pass),
            (Seat::West, Bid::Suit(4, BidSuit::Spades)),
            (Seat::North, Bid::Pass),
            (Seat::East, Bid::Suit(4, BidSuit::NoTrump)),
            (Seat::South, Bid::Pass),
            (Seat::West, Bid::Suit(5, BidSuit::Clubs)),
        ];
        let c = constraints_from_bids(Seat::West, &bids, Vulnerability::None);
        assert_eq!(c.ace_range, (0, 0));
    }

    #[test]
    fn blackwood_response_one_ace() {
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::Spades)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(4, BidSuit::NoTrump)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(5, BidSuit::Diamonds)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert_eq!(c.ace_range, (1, 1));
    }

    #[test]
    fn gerber_response_zero_aces() {
        // South opens 1NT, North bids 4♣ (Gerber), South responds 4♦ (0 aces)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::NoTrump)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(4, BidSuit::Clubs)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(4, BidSuit::Diamonds)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert_eq!(c.ace_range, (0, 0));
    }

    #[test]
    fn gerber_response_two_aces() {
        // North opens 2NT, South bids 4♣ (Gerber), North responds 4♠ (2 aces)
        let bids = vec![
            (Seat::North, Bid::Suit(2, BidSuit::NoTrump)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(4, BidSuit::Clubs)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(4, BidSuit::Spades)),
        ];
        let c = constraints_from_bids(Seat::North, &bids, Vulnerability::None);
        assert_eq!(c.ace_range, (2, 2));
    }

    #[test]
    fn stayman_implies_major_and_hcp() {
        // South opens 1NT, North responds 2♣ (Stayman)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::NoTrump)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(2, BidSuit::Clubs)),
        ];
        let c = constraints_from_bids(Seat::North, &bids, Vulnerability::None);
        assert!(c.hcp_range.0 >= 8, "Stayman requires 8+ HCP, got min {}", c.hcp_range.0);
    }

    #[test]
    fn jacoby_transfer_to_hearts() {
        // South opens 1NT, North bids 2♦ (transfer to hearts)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::NoTrump)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(2, BidSuit::Diamonds)),
        ];
        let c = constraints_from_bids(Seat::North, &bids, Vulnerability::None);
        assert_eq!(c.suit_lengths[2].0, 5, "Transfer to hearts: 5+ hearts");
    }

    #[test]
    fn jacoby_transfer_to_spades() {
        // South opens 1NT, North bids 2♥ (transfer to spades)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::NoTrump)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(2, BidSuit::Hearts)),
        ];
        let c = constraints_from_bids(Seat::North, &bids, Vulnerability::None);
        assert_eq!(c.suit_lengths[3].0, 5, "Transfer to spades: 5+ spades");
    }

    #[test]
    fn two_diamonds_not_transfer_without_1nt() {
        // North opens 1♥, South responds 2♦ — NOT a transfer
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(2, BidSuit::Diamonds)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        // Should be treated as a new suit response (4+ diamonds), not a transfer
        assert_eq!(c.suit_lengths[1].0, 4, "Not a transfer: 4+ diamonds as new suit");
        assert_eq!(c.suit_lengths[2].0, 0, "No heart constraint implied");
    }

    #[test]
    fn takeout_double_hcp() {
        // North opens 1H, East doubles (takeout)
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::East, Bid::Double),
        ];
        let c = constraints_from_bids(Seat::East, &bids, Vulnerability::None);
        assert!(c.hcp_range.0 >= 12, "Takeout double: 12+ HCP, got min {}", c.hcp_range.0);
    }

    #[test]
    fn negative_double_hcp() {
        // South opens 1H, West overcalls 2C, North doubles (negative)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::West, Bid::Suit(2, BidSuit::Clubs)),
            (Seat::North, Bid::Double),
        ];
        let c = constraints_from_bids(Seat::North, &bids, Vulnerability::None);
        assert!(c.hcp_range.0 >= 7, "Negative double: 7+ HCP, got min {}", c.hcp_range.0);
    }

    #[test]
    fn stayman_reply_denies_major() {
        // South opens 1NT, North bids 2♣ (Stayman), South replies 2♦ (no major)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::NoTrump)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(2, BidSuit::Clubs)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(2, BidSuit::Diamonds)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert!(c.suit_lengths[2].1 <= 3, "2D reply: max 3 hearts, got max {}", c.suit_lengths[2].1);
        assert!(c.suit_lengths[3].1 <= 3, "2D reply: max 3 spades, got max {}", c.suit_lengths[3].1);
    }

    #[test]
    fn stayman_reply_shows_hearts() {
        // South opens 1NT, North bids 2♣, South replies 2♥ (4+ hearts)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::NoTrump)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(2, BidSuit::Clubs)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(2, BidSuit::Hearts)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert!(c.suit_lengths[2].0 >= 4, "2H reply: 4+ hearts, got min {}", c.suit_lengths[2].0);
    }

    #[test]
    fn stayman_reply_shows_spades_denies_hearts() {
        // South opens 1NT, North bids 2♣, South replies 2♠
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::NoTrump)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(2, BidSuit::Clubs)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(2, BidSuit::Spades)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert!(c.suit_lengths[3].0 >= 4, "2S reply: 4+ spades, got min {}", c.suit_lengths[3].0);
        assert!(c.suit_lengths[2].1 <= 3, "2S reply: max 3 hearts, got max {}", c.suit_lengths[2].1);
    }

    #[test]
    fn strong_2c_waiting_response() {
        // North opens 2♣, South responds 2♦ (waiting)
        let bids = vec![
            (Seat::North, Bid::Suit(2, BidSuit::Clubs)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(2, BidSuit::Diamonds)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert!(c.hcp_range.1 <= 7, "2D waiting: max 7 HCP, got max {}", c.hcp_range.1);
    }

    #[test]
    fn strong_2c_positive_response() {
        // North opens 2♣, South responds 2♥ (positive, 5+ hearts)
        let bids = vec![
            (Seat::North, Bid::Suit(2, BidSuit::Clubs)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(2, BidSuit::Hearts)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert!(c.hcp_range.0 >= 8, "Positive response: 8+ HCP, got min {}", c.hcp_range.0);
        assert!(c.suit_lengths[2].0 >= 5, "Positive 2H: 5+ hearts, got min {}", c.suit_lengths[2].0);
    }

    #[test]
    fn opener_rebids_own_suit() {
        // South opens 1H, North responds 1S, South rebids 2H (6+ hearts)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(1, BidSuit::Spades)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(2, BidSuit::Hearts)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert!(c.suit_lengths[2].0 >= 6, "Rebid own suit: 6+ hearts, got min {}", c.suit_lengths[2].0);
    }

    #[test]
    fn opener_rebids_1nt() {
        // South opens 1D, North responds 1H, South rebids 1NT (12-14 balanced)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::Diamonds)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(1, BidSuit::NoTrump)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert_eq!(c.hcp_range, (12, 14));
        for i in 0..4 {
            assert!(c.suit_lengths[i].0 >= 2, "1NT rebid: balanced, suit {} min {}", i, c.suit_lengths[i].0);
        }
    }

    #[test]
    fn opener_rebids_new_suit() {
        // South opens 1H, North responds 1NT, South rebids 2C (4+ clubs)
        let bids = vec![
            (Seat::South, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::West, Bid::Pass),
            (Seat::North, Bid::Suit(1, BidSuit::NoTrump)),
            (Seat::East, Bid::Pass),
            (Seat::South, Bid::Suit(2, BidSuit::Clubs)),
        ];
        let c = constraints_from_bids(Seat::South, &bids, Vulnerability::None);
        assert!(c.suit_lengths[0].0 >= 4, "New suit rebid: 4+ clubs, got min {}", c.suit_lengths[0].0);
        assert!(c.suit_lengths[2].0 >= 5, "Original opening: 5+ hearts still holds, got min {}", c.suit_lengths[2].0);
    }

    #[test]
    fn doubler_then_bids_suit() {
        // N opens 1H, E doubles, S passes, W bids 2C, N bids 2H, E bids 3S
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::East, Bid::Double),
            (Seat::South, Bid::Pass),
            (Seat::West, Bid::Suit(2, BidSuit::Clubs)),
            (Seat::North, Bid::Suit(2, BidSuit::Hearts)),
            (Seat::East, Bid::Suit(3, BidSuit::Spades)),
        ];
        let c = constraints_from_bids(Seat::East, &bids, Vulnerability::None);
        assert!(c.hcp_range.0 >= 12, "Doubler: 12+ HCP, got min {}", c.hcp_range.0);
        assert!(c.suit_lengths[3].0 >= 5, "Doubler bids 3S: 5+ spades, got min {}", c.suit_lengths[3].0);
    }

    #[test]
    fn advance_partners_double() {
        // N opens 1H, E doubles, S passes, W bids 2C (advancing)
        let bids = vec![
            (Seat::North, Bid::Suit(1, BidSuit::Hearts)),
            (Seat::East, Bid::Double),
            (Seat::South, Bid::Pass),
            (Seat::West, Bid::Suit(2, BidSuit::Clubs)),
        ];
        let c = constraints_from_bids(Seat::West, &bids, Vulnerability::None);
        // Advancing at 2-level can be weak (0+ HCP)
        assert_eq!(c.hcp_range.0, 0, "Advance can be weak, got min {}", c.hcp_range.0);
        assert!(c.suit_lengths[0].0 >= 4, "Advance 2C: 4+ clubs, got min {}", c.suit_lengths[0].0);
    }
}
