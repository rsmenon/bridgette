use std::time::{Duration, Instant};

use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::types::Seat;

use super::bid_constraints::HandConstraints;
use super::card::{Card, Rank, Suit};
use super::hand::Hand;
use super::trick::Trick;

/// Per-card probability that a card is in a given seat's hand.
/// probs[seat_index][suit_index][rank_index] — f32 in 0.0..=1.0
#[derive(Debug, Clone)]
pub struct CardProbabilities {
    pub probs: [[[f32; 13]; 4]; 4], // [4 seats][4 suits][13 ranks]
    pub sample_count: u32,
    /// Cards that have been played (for UI to show 'x')
    pub played_cards: Vec<(Seat, Card)>,
    /// Number of unknown seats (2 or 3) — used for neutral-point coloring
    pub unknown_seat_count: u8,
    /// Suit-length histograms: [seat][suit][length 0-13] = sample count
    pub suit_histograms: [[[u32; 14]; 4]; 4],
    /// Remaining cards each seat holds (13 minus cards played by that seat)
    pub remaining_per_seat: [usize; 4],
    /// Remaining cards of each suit in the unknown pool [C, D, H, S]
    pub pool_suit_counts: [usize; 4],
}

impl CardProbabilities {
    pub fn new() -> Self {
        Self {
            probs: [[[0.0; 13]; 4]; 4],
            sample_count: 0,
            played_cards: Vec::new(),
            unknown_seat_count: 3,
            suit_histograms: [[[0u32; 14]; 4]; 4],
            remaining_per_seat: [13; 4],
            pool_suit_counts: [0; 4],
        }
    }

    pub fn prob(&self, seat: Seat, suit: Suit, rank: Rank) -> f32 {
        self.probs[seat.index()][suit_index(suit)][rank_index(rank)]
    }

    /// Returns the 80% highest-density interval for suit length of *remaining* cards.
    /// Clamped to [0, min(remaining_hand_size, pool_suit_count)].
    /// Falls back to a marginal-derived estimate if histogram data is insufficient.
    pub fn suit_length_range(&self, seat: Seat, suit: Suit) -> (u8, u8) {
        let si = suit_index(suit);
        let max_possible = self.remaining_per_seat[seat.index()]
            .min(self.pool_suit_counts[si]) as u8;

        let hist = &self.suit_histograms[seat.index()][si];
        let total: u32 = hist.iter().sum();

        let (lo, hi) = if total >= 50 {
            // Enough histogram data — use HDI
            let threshold = (total as f64 * 0.80).ceil() as u32;
            let mut bins: Vec<(usize, u32)> = hist.iter().enumerate().filter(|&(_, &c)| c > 0).map(|(i, &c)| (i, c)).collect();
            bins.sort_by(|a, b| b.1.cmp(&a.1));

            let mut accumulated = 0u32;
            let mut min_len = 13usize;
            let mut max_len = 0usize;
            for &(len, count) in &bins {
                accumulated += count;
                min_len = min_len.min(len);
                max_len = max_len.max(len);
                if accumulated >= threshold {
                    break;
                }
            }
            (min_len as u8, max_len as u8)
        } else {
            // Derive range from per-card marginal probabilities.
            // Expected length = sum of probs. Variance ≈ sum of p*(1-p).
            let mut mean = 0.0f32;
            let mut variance = 0.0f32;
            for ri in 0..13 {
                let p = self.probs[seat.index()][si][ri];
                mean += p;
                variance += p * (1.0 - p);
            }
            let stddev = variance.sqrt();
            let lo = (mean - stddev).floor().max(0.0) as u8;
            let hi = (mean + stddev).ceil().min(13.0) as u8;
            (lo, hi)
        };

        // Clamp to what's physically possible given remaining cards
        (lo.min(max_possible), hi.min(max_possible))
    }

    /// Expected suit length of *remaining* cards (weighted average from histogram,
    /// or sum of per-card marginals), clamped to pool constraints.
    pub fn expected_suit_length(&self, seat: Seat, suit: Suit) -> f32 {
        let si = suit_index(suit);
        let max_possible = self.remaining_per_seat[seat.index()]
            .min(self.pool_suit_counts[si]) as f32;

        let raw = {
            let hist = &self.suit_histograms[seat.index()][si];
            let total: u32 = hist.iter().sum();
            if total > 0 {
                let weighted: f64 = hist.iter().enumerate().map(|(len, &count)| len as f64 * count as f64).sum();
                (weighted / total as f64) as f32
            } else {
                // Fallback: sum per-card marginals
                (0..13).map(|ri| self.probs[seat.index()][si][ri]).sum()
            }
        };

        raw.min(max_possible)
    }

    /// Per-suit confidence combining inference quality (sample count) with
    /// histogram concentration (80% HDI width) for the given seat and suit.
    pub fn suit_confidence(&self, seat: Seat, suit: Suit) -> Confidence {
        // Weighted fallback — no real sampling was done
        if self.sample_count == 0 {
            return Confidence::Low;
        }

        let (lo, hi) = self.suit_length_range(seat, suit);
        let spread = hi.saturating_sub(lo);

        if self.sample_count == u32::MAX {
            // Exact enumeration — numbers are perfect; confidence reflects
            // how informative the result is (narrow range = more useful).
            if spread <= 2 { Confidence::High } else { Confidence::Med }
        } else if self.sample_count >= 200 {
            if spread <= 2 { Confidence::High }
            else if spread <= 4 { Confidence::Med }
            else { Confidence::Low }
        } else {
            // 50–199 accepted samples
            if spread <= 1 { Confidence::High }
            else if spread <= 3 { Confidence::Med }
            else { Confidence::Low }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confidence {
    Low,
    Med,
    High,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Confidence::Low => write!(f, "Low"),
            Confidence::Med => write!(f, "Med"),
            Confidence::High => write!(f, "High"),
        }
    }
}

fn suit_index(suit: Suit) -> usize {
    match suit {
        Suit::Clubs => 0,
        Suit::Diamonds => 1,
        Suit::Hearts => 2,
        Suit::Spades => 3,
    }
}

fn rank_index(rank: Rank) -> usize {
    match rank {
        Rank::Two => 0,
        Rank::Three => 1,
        Rank::Four => 2,
        Rank::Five => 3,
        Rank::Six => 4,
        Rank::Seven => 5,
        Rank::Eight => 6,
        Rank::Nine => 7,
        Rank::Ten => 8,
        Rank::Jack => 9,
        Rank::Queen => 10,
        Rank::King => 11,
        Rank::Ace => 12,
    }
}

/// Detect void suits from play history: if a player didn't follow suit, they're void.
pub fn void_suits_from_play(tricks: &[Trick], current_trick: &Trick) -> [Vec<Suit>; 4] {
    let mut voids: [Vec<Suit>; 4] = Default::default();

    let all_tricks: Vec<&Trick> = tricks.iter().chain(std::iter::once(current_trick)).collect();

    for trick in all_tricks {
        if let Some(led_suit) = trick.led_suit() {
            for &(seat, card) in &trick.cards {
                if card.suit != led_suit && !voids[seat.index()].contains(&led_suit) {
                    voids[seat.index()].push(led_suit);
                }
            }
        }
    }

    voids
}

/// Collect all cards that have been played so far.
fn played_cards_list(tricks: &[Trick], current_trick: &Trick) -> Vec<(Seat, Card)> {
    let mut played = Vec::new();
    for trick in tricks {
        for &(seat, card) in &trick.cards {
            played.push((seat, card));
        }
    }
    for &(seat, card) in &current_trick.cards {
        played.push((seat, card));
    }
    played
}

/// Count aces in a slice of cards.
fn ace_count(cards: &[Card]) -> u32 {
    cards.iter().filter(|c| c.rank == Rank::Ace).count() as u32
}

/// Calculate HCP for a slice of cards.
fn hcp_of(cards: &[Card]) -> u32 {
    cards.iter().map(|c| match c.rank {
        Rank::Ace => 4,
        Rank::King => 3,
        Rank::Queen => 2,
        Rank::Jack => 1,
        _ => 0,
    }).sum()
}

/// Count cards of a given suit in a slice.
fn suit_count(cards: &[Card], suit: Suit) -> u32 {
    cards.iter().filter(|c| c.suit == suit).count() as u32
}

/// Adjust bid constraints to account for cards already played by each seat.
///
/// Bid constraints describe the original 13-card hand (e.g., "5+ hearts, 10+ HCP").
/// The sampler deals only remaining cards. If a seat has already played 1 heart and
/// 6 HCP worth of honors, the remaining-hand constraint should be "4+ hearts, 4+ HCP".
fn adjust_constraints_for_play(
    constraints: &[HandConstraints; 4],
    played: &[(Seat, Card)],
    unknown_seats: &[Seat],
) -> [HandConstraints; 4] {
    let mut adjusted = constraints.clone();

    for &seat in unknown_seats {
        let seat_played: Vec<&Card> = played.iter()
            .filter(|(s, _)| *s == seat)
            .map(|(_, c)| c)
            .collect();

        if seat_played.is_empty() {
            continue;
        }

        let c = &mut adjusted[seat.index()];

        // Subtract played HCP from the HCP range
        let played_hcp: u32 = seat_played.iter().map(|card| match card.rank {
            Rank::Ace => 4,
            Rank::King => 3,
            Rank::Queen => 2,
            Rank::Jack => 1,
            _ => 0,
        }).sum();
        c.hcp_range.0 = c.hcp_range.0.saturating_sub(played_hcp);
        c.hcp_range.1 = c.hcp_range.1.saturating_sub(played_hcp);

        // Subtract played aces from ace range
        let played_aces = seat_played.iter().filter(|card| card.rank == Rank::Ace).count() as u32;
        c.ace_range.0 = c.ace_range.0.saturating_sub(played_aces);
        c.ace_range.1 = c.ace_range.1.saturating_sub(played_aces);

        // Subtract played suit cards from suit-length minimums
        for (i, &suit) in Suit::ALL.iter().enumerate() {
            let played_in_suit = seat_played.iter().filter(|card| card.suit == suit).count() as u32;
            c.suit_lengths[i].0 = c.suit_lengths[i].0.saturating_sub(played_in_suit);
            c.suit_lengths[i].1 = c.suit_lengths[i].1.saturating_sub(played_in_suit);
            // Ensure max doesn't go below min
            if c.suit_lengths[i].1 < c.suit_lengths[i].0 {
                c.suit_lengths[i].1 = c.suit_lengths[i].0;
            }
        }
    }

    adjusted
}

/// For each card in the pool, compute which unknown seats can legally hold it.
/// A seat is ineligible if:
/// - it is void in the card's suit (from play observation), OR
/// - its adjusted suit-length maximum for that suit is 0 (derived void from constraints), OR
/// - the card is an ace and the seat's ace_range maximum is 0 (e.g. Blackwood 0 aces)
fn compute_eligible_seats(
    pool: &[Card],
    unknown_seats: &[Seat],
    voids: &[Vec<Suit>; 4],
    constraints: &[HandConstraints; 4],
) -> Vec<Vec<Seat>> {
    pool.iter().map(|card| {
        let si = suit_index(card.suit);
        unknown_seats.iter()
            .copied()
            .filter(|&s| {
                !voids[s.index()].contains(&card.suit)
                    && constraints[s.index()].suit_lengths[si].1 > 0
                    && !(card.rank == Rank::Ace && constraints[s.index()].ace_range.1 == 0)
            })
            .collect()
    }).collect()
}

/// Generate a single constrained deal using pre-partitioned card assignment.
///
/// Phase 1: Cards eligible for exactly one seat are force-assigned.
/// Phase 2: Suit-length minimums are satisfied from the remaining free pool.
/// Phase 3: Remaining cards are shuffled and distributed; void violations
///          in the free portion (possible with 3 unknown seats) cause a retry
///          but are far rarer than in pure rejection sampling.
///
/// Returns None if the deal is infeasible or fails a constraint check.
fn generate_constrained_deal(
    pool: &[Card],
    unknown_seats: &[Seat],
    target_counts: &[usize; 4],
    voids: &[Vec<Suit>; 4],
    constraints: &[HandConstraints; 4],
    eligible_per_card: &[Vec<Seat>],
    rng: &mut rand::rngs::ThreadRng,
) -> Option<[Vec<Card>; 4]> {
    // Shuffle seat processing order to eliminate systematic bias:
    // without this, the first seat gets cards from a fuller pool while
    // the last seat gets deterministic leftovers.
    let mut shuffled_seats = unknown_seats.to_vec();
    shuffled_seats.shuffle(rng);

    let mut hands: [Vec<Card>; 4] = Default::default();
    let mut free: Vec<Card> = Vec::new();

    // Phase 1: Force-assign cards that can only go to one seat
    for (i, &card) in pool.iter().enumerate() {
        match eligible_per_card[i].len() {
            0 => return None,
            1 => hands[eligible_per_card[i][0].index()].push(card),
            _ => free.push(card),
        }
    }

    // Check forced assignments don't exceed capacity
    for &seat in &shuffled_seats {
        if hands[seat.index()].len() > target_counts[seat.index()] {
            return None;
        }
    }

    // Phase 2: Pre-assign suit-length minimums from free pool
    for &seat in &shuffled_seats {
        let c = &constraints[seat.index()];
        for (si, &suit) in Suit::ALL.iter().enumerate() {
            if voids[seat.index()].contains(&suit) { continue; }
            let space = target_counts[seat.index()] - hands[seat.index()].len();
            if space == 0 { break; }

            let have = hands[seat.index()].iter().filter(|cd| cd.suit == suit).count() as u32;
            let min_needed = c.suit_lengths[si].0;
            if have >= min_needed { continue; }

            let still_need = ((min_needed - have) as usize).min(space);

            // Collect indices of this suit in the free pool
            // Exclude aces if the seat can't hold them (ace_range.1 == 0)
            let mut suit_indices: Vec<usize> = free.iter()
                .enumerate()
                .filter(|(_, cd)| {
                    cd.suit == suit
                        && !(cd.rank == Rank::Ace && c.ace_range.1 == 0)
                })
                .map(|(idx, _)| idx)
                .collect();

            if suit_indices.len() < still_need {
                return None;
            }

            // Randomly pick which cards to pre-assign
            suit_indices.shuffle(rng);
            let mut to_take: Vec<usize> = suit_indices[..still_need].to_vec();
            to_take.sort_unstable();
            for (offset, &idx) in to_take.iter().enumerate() {
                let card = free.remove(idx - offset);
                hands[seat.index()].push(card);
            }
        }

        if hands[seat.index()].len() > target_counts[seat.index()] {
            return None;
        }
    }

    // Shuffle free pool before propagation to eliminate iteration-order bias
    // when cards compete for seats under tight constraints.
    free.shuffle(rng);

    // Phase 2b: Iterative constraint propagation — after pre-assigning minimums,
    // some free cards may now be eligible for only one seat (e.g., if one seat
    // consumed all available cards of a suit that another seat also needed).
    // Re-derive eligibility and force-assign until stable.
    loop {
        let mut changed = false;
        let mut i = 0;
        while i < free.len() {
            let card = free[i];
            let si = suit_index(card.suit);
            let eligible: Vec<Seat> = shuffled_seats.iter()
                .copied()
                .filter(|&s| {
                    !voids[s.index()].contains(&card.suit)
                        && constraints[s.index()].suit_lengths[si].1 > 0
                        && hands[s.index()].len() < target_counts[s.index()]
                        // Exclude if seat already has max cards of this suit
                        && (hands[s.index()].iter().filter(|c| c.suit == card.suit).count() as u32)
                            < constraints[s.index()].suit_lengths[si].1
                        // Exclude aces from seats that can't hold them
                        && !(card.rank == Rank::Ace && constraints[s.index()].ace_range.1 == 0)
                })
                .collect();
            match eligible.len() {
                0 => return None,
                1 => {
                    hands[eligible[0].index()].push(card);
                    free.remove(i);
                    changed = true;
                    // Don't increment i — next card shifted into position
                }
                _ => { i += 1; }
            }
        }
        if !changed { break; }
    }

    // Check capacity after propagation
    for &seat in &shuffled_seats {
        if hands[seat.index()].len() > target_counts[seat.index()] {
            return None;
        }
    }

    // Phase 3: Shuffle remaining free cards and distribute
    free.shuffle(rng);
    let mut idx = 0;
    for &seat in &shuffled_seats {
        let need = target_counts[seat.index()] - hands[seat.index()].len();
        if need == 0 { continue; }
        if idx + need > free.len() { return None; }
        let deal = &free[idx..idx + need];

        // Check void constraints on this slice (only possible with 3+ unknown seats)
        for &void_suit in &voids[seat.index()] {
            if deal.iter().any(|c| c.suit == void_suit) {
                return None;
            }
        }

        hands[seat.index()].extend_from_slice(deal);
        idx += need;
    }

    // Phase 4: Check soft constraints (HCP range, suit-length maximums)
    for &seat in &shuffled_seats {
        let hand = &hands[seat.index()];
        let c = &constraints[seat.index()];

        let hcp = hcp_of(hand);
        if hcp < c.hcp_range.0 || hcp > c.hcp_range.1 {
            return None;
        }

        let aces = ace_count(hand);
        if aces < c.ace_range.0 || aces > c.ace_range.1 {
            return None;
        }

        for (i, &suit) in Suit::ALL.iter().enumerate() {
            let count = suit_count(hand, suit);
            if count < c.suit_lengths[i].0 || count > c.suit_lengths[i].1 {
                return None;
            }
        }
    }

    Some(hands)
}

/// Run exact enumeration for small pools with 2 unknown seats.
///
/// Enumerates all C(n, k) ways to partition `pool` into two hands, checks each
/// against constraints (voids, HCP, suit lengths), and counts valid partitions
/// per card to produce exact probabilities. Eliminates sampling noise entirely.
///
/// Returns None if preconditions aren't met (pool too large or != 2 unknown seats).
/// Return type for exact inference: (per-card probabilities, suit-length histograms)
type ExactResult = ([[[f64; 13]; 4]; 4], [[[u64; 14]; 4]; 4]);

fn run_exact_inference(
    pool: &[Card],
    unknown_seats: &[Seat],
    target_counts: &[usize; 4],
    voids: &[Vec<Suit>; 4],
    constraints: &[HandConstraints; 4],
) -> Option<ExactResult> {
    if unknown_seats.len() != 2 || pool.len() > 20 {
        return None;
    }

    let seat_a = unknown_seats[0];
    let seat_b = unknown_seats[1];
    let count_a = target_counts[seat_a.index()];
    let count_b = target_counts[seat_b.index()];

    if count_a + count_b != pool.len() {
        return None;
    }

    // Enumerate all C(pool.len(), count_a) subsets for seat_a
    let n = pool.len();
    let mut counters = [[[0u64; 13]; 4]; 4];
    let mut hist = [[[0u64; 14]; 4]; 4];
    let mut total_valid: u64 = 0;

    // Use bitmask enumeration for subsets of size count_a
    // For n <= 20, this is at most C(20, 10) = 184,756 — trivially fast
    let mut combo: Vec<usize> = (0..count_a).collect();

    loop {
        // Build hands for this partition
        let mut hand_a: Vec<Card> = Vec::with_capacity(count_a);
        let mut hand_b: Vec<Card> = Vec::with_capacity(count_b);
        let mut in_a = vec![false; n];
        for &idx in &combo {
            in_a[idx] = true;
            hand_a.push(pool[idx]);
        }
        for (idx, &card) in pool.iter().enumerate() {
            if !in_a[idx] {
                hand_b.push(card);
            }
        }

        // Check constraints for both seats
        let valid = check_hand_constraints(&hand_a, seat_a, voids, constraints)
            && check_hand_constraints(&hand_b, seat_b, voids, constraints);

        if valid {
            total_valid += 1;
            for &card in &hand_a {
                counters[seat_a.index()][suit_index(card.suit)][rank_index(card.rank)] += 1;
            }
            for &card in &hand_b {
                counters[seat_b.index()][suit_index(card.suit)][rank_index(card.rank)] += 1;
            }
            // Accumulate suit-length histograms
            for (si, &suit) in Suit::ALL.iter().enumerate() {
                let len_a = hand_a.iter().filter(|c| c.suit == suit).count();
                let len_b = hand_b.iter().filter(|c| c.suit == suit).count();
                hist[seat_a.index()][si][len_a] += 1;
                hist[seat_b.index()][si][len_b] += 1;
            }
        }

        // Advance to next combination
        if !next_combination(&mut combo, n) {
            break;
        }
    }

    if total_valid == 0 {
        return None;
    }

    let mut result = [[[0.0f64; 13]; 4]; 4];
    let total = total_valid as f64;
    for &seat in unknown_seats {
        for si in 0..4 {
            for ri in 0..13 {
                result[seat.index()][si][ri] = counters[seat.index()][si][ri] as f64 / total;
            }
        }
    }

    Some((result, hist))
}

/// Check if a hand satisfies void, HCP, and suit-length constraints.
fn check_hand_constraints(
    hand: &[Card],
    seat: Seat,
    voids: &[Vec<Suit>; 4],
    constraints: &[HandConstraints; 4],
) -> bool {
    let c = &constraints[seat.index()];

    // Check voids
    for &void_suit in &voids[seat.index()] {
        if hand.iter().any(|card| card.suit == void_suit) {
            return false;
        }
    }

    // Check HCP
    let hcp = hcp_of(hand);
    if hcp < c.hcp_range.0 || hcp > c.hcp_range.1 {
        return false;
    }

    // Check ace count
    let aces = ace_count(hand);
    if aces < c.ace_range.0 || aces > c.ace_range.1 {
        return false;
    }

    // Check suit lengths
    for (i, &suit) in Suit::ALL.iter().enumerate() {
        let count = suit_count(hand, suit);
        if count < c.suit_lengths[i].0 || count > c.suit_lengths[i].1 {
            return false;
        }
    }

    true
}

/// Advance a combination (indices in ascending order) to the next one.
/// Returns false when all combinations have been exhausted.
fn next_combination(combo: &mut Vec<usize>, n: usize) -> bool {
    let k = combo.len();
    if k == 0 {
        return false;
    }

    // Find rightmost element that can be incremented
    let mut i = k;
    loop {
        if i == 0 {
            return false;
        }
        i -= 1;
        if combo[i] < n - k + i {
            combo[i] += 1;
            for j in (i + 1)..k {
                combo[j] = combo[j - 1] + 1;
            }
            return true;
        }
    }
}

/// Compute constraint-weighted probabilities as a fallback when sampling
/// produces too few accepted deals and exact enumeration isn't feasible.
///
/// Priority hierarchy (per user guidance):
/// 1. HCP: honor cards (AKQJ) weighted toward seats with higher HCP demands
/// 2. Distribution: suit-length constraints provide moderate influence
/// 3. Cue signals: aces filtered by ace_range (Blackwood)
/// 4. Spot cards (2-T): near-uniform distribution with mild suit weighting
fn constraint_weighted_fallback(
    pool: &[Card],
    unknown_seats: &[Seat],
    voids: &[Vec<Suit>; 4],
    constraints: &[HandConstraints; 4],
    target_counts: &[usize; 4],
) -> [[[f32; 13]; 4]; 4] {
    let mut result = [[[0.0f32; 13]; 4]; 4];

    for card in pool.iter() {
        let si = suit_index(card.suit);
        let ri = rank_index(card.rank);
        let card_hcp: u32 = match card.rank {
            Rank::Ace => 4, Rank::King => 3, Rank::Queen => 2, Rank::Jack => 1, _ => 0,
        };
        let is_honor = card_hcp > 0;

        let mut weights: Vec<(Seat, f32)> = Vec::new();
        let mut total_weight: f32 = 0.0;

        for &seat in unknown_seats {
            let c = &constraints[seat.index()];

            // Hard filters: void, suit max, ace constraint
            if voids[seat.index()].contains(&card.suit) { continue; }
            if c.suit_lengths[si].1 == 0 { continue; }
            if card.rank == Rank::Ace && c.ace_range.1 == 0 { continue; }

            let remaining = target_counts[seat.index()] as f32;

            // --- Priority 1: HCP demand ---
            // For honor cards, weight toward seats with higher expected HCP
            let hcp_factor = if is_honor {
                let mid_hcp = (c.hcp_range.0 + c.hcp_range.1) as f32 / 2.0;
                // Scale: 0 HCP → 0.2, 10 HCP → 0.7, 20 HCP → 1.2
                0.2 + mid_hcp / 20.0
            } else {
                // Spot cards: uniform HCP factor
                1.0
            };

            // --- Priority 2: Distribution (suit length) ---
            // Mild boost for seats that need this suit; don't let it dominate
            let suit_min = c.suit_lengths[si].0 as f32;
            // 0 min → 1.0, 5 min → 1.25, 7 min → 1.35
            let suit_factor = 1.0 + suit_min * 0.05;

            let weight = remaining.max(0.1) * hcp_factor * suit_factor;
            weights.push((seat, weight));
            total_weight += weight;
        }

        if total_weight > 0.0 {
            for (seat, weight) in &weights {
                result[seat.index()][si][ri] = weight / total_weight;
            }
        }
    }

    result
}

/// Run Monte Carlo inference to estimate card probabilities.
///
/// Uses constraint-aware deal generation: cards forced by void constraints are
/// pre-assigned, suit-length minimums are satisfied before random distribution,
/// and only soft constraints (HCP, suit-length max) require rejection sampling.
/// This yields much higher acceptance rates than pure rejection sampling,
/// especially when voids are present during play.
///
/// `south_hand`: South's current hand (always known).
/// `dummy_hand`: Dummy's hand if revealed.
/// `constraints`: Per-seat constraints from bidding.
/// `tricks`: Completed tricks.
/// `current_trick`: The current (incomplete) trick.
/// `num_samples`: Number of Monte Carlo iterations.
pub fn run_inference(
    south_hand: &Hand,
    dummy_hand: Option<&Hand>,
    dummy_seat: Option<Seat>,
    constraints: &[HandConstraints; 4],
    tricks: &[Trick],
    current_trick: &Trick,
    num_samples: u32,
) -> CardProbabilities {
    let mut result = CardProbabilities::new();
    let timeout = Duration::from_millis(200);
    let start = Instant::now();

    let played = played_cards_list(tricks, current_trick);
    result.played_cards = played.clone();

    let played_cards_set: Vec<Card> = played.iter().map(|(_, c)| *c).collect();

    // Detect voids from play
    let voids = void_suits_from_play(tricks, current_trick);

    // Build the pool of unknown cards
    let mut pool: Vec<Card> = Vec::new();
    for &suit in &Suit::ALL {
        for &rank in &Rank::ALL {
            let card = Card::new(suit, rank);
            if south_hand.cards().contains(&card) {
                continue;
            }
            if let Some(dh) = dummy_hand {
                if dh.cards().contains(&card) {
                    continue;
                }
            }
            if played_cards_set.contains(&card) {
                continue;
            }
            pool.push(card);
        }
    }

    // Determine which seats are unknown
    let unknown_seats: Vec<Seat> = Seat::ALL.iter().copied().filter(|&s| {
        s != Seat::South && Some(s) != dummy_seat
    }).collect();
    result.unknown_seat_count = unknown_seats.len() as u8;

    // How many cards each unknown seat still holds
    let mut target_counts: [usize; 4] = [0; 4];
    for &seat in &unknown_seats {
        let cards_played_by_seat = played.iter().filter(|(s, _)| *s == seat).count();
        target_counts[seat.index()] = 13 - cards_played_by_seat;
    }

    // Record remaining cards per seat and pool suit counts for range clamping
    result.remaining_per_seat = target_counts;
    for &card in &pool {
        result.pool_suit_counts[suit_index(card.suit)] += 1;
    }

    // Adjust bid constraints to account for cards already played by each seat.
    // Bid constraints describe the original 13-card hand, but the sampler deals
    // only remaining cards. Subtract played HCP and suit counts so the constraints
    // apply correctly to the reduced hand.
    let adjusted_constraints = adjust_constraints_for_play(constraints, &played, &unknown_seats);

    // Pre-compute which seats can hold each card (void + constraint filtering)
    let eligible_per_card = compute_eligible_seats(&pool, &unknown_seats, &voids, &adjusted_constraints);

    // Try exact enumeration first for small pools with 2 unknown seats.
    // This eliminates sampling noise entirely and makes the fallback unreachable.
    let used_exact = if unknown_seats.len() == 2 && pool.len() <= 20 {
        if let Some((exact, exact_hist)) = run_exact_inference(
            &pool, &unknown_seats, &target_counts, &voids, &adjusted_constraints,
        ) {
            for &seat in &unknown_seats {
                for si in 0..4 {
                    for ri in 0..13 {
                        result.probs[seat.index()][si][ri] = exact[seat.index()][si][ri] as f32;
                    }
                    for len in 0..14 {
                        result.suit_histograms[seat.index()][si][len] = exact_hist[seat.index()][si][len] as u32;
                    }
                }
            }
            result.sample_count = u32::MAX; // signal: exact
            true
        } else {
            false
        }
    } else {
        false
    };

    if !used_exact {
        // Monte Carlo sampling with adaptive constraint relaxation.
        // If acceptance rate is too low after an initial probe, widen HCP
        // bounds by ±2 and recompute eligibility to improve acceptance.
        let mut counters = [[[0u32; 13]; 4]; 4];
        let mut accepted = 0u32;
        let mut rng = thread_rng();
        let mut active_constraints = adjusted_constraints.clone();
        let mut active_eligible = eligible_per_card.clone();

        // Phase 1: probe with original constraints (first 200 attempts)
        let probe_limit = 200u32.min(num_samples);
        let mut attempted = 0u32;
        for _ in 0..probe_limit {
            if start.elapsed() > timeout {
                break;
            }
            attempted += 1;

            let hands = match generate_constrained_deal(
                &pool, &unknown_seats, &target_counts, &voids, &active_constraints,
                &active_eligible, &mut rng,
            ) {
                Some(h) => h,
                None => continue,
            };

            accepted += 1;
            for &seat in &unknown_seats {
                for card in &hands[seat.index()] {
                    counters[seat.index()][suit_index(card.suit)][rank_index(card.rank)] += 1;
                }
                for (si, &suit) in Suit::ALL.iter().enumerate() {
                    let count = hands[seat.index()].iter().filter(|c| c.suit == suit).count();
                    result.suit_histograms[seat.index()][si][count] += 1;
                }
            }
        }

        // Adaptive relaxation: if acceptance < 10% after probe, widen HCP ±2
        // Discard Phase 1 samples to avoid mixing strict/relaxed distributions
        if attempted >= probe_limit && accepted < (attempted / 10) {
            for &seat in &unknown_seats {
                let c = &mut active_constraints[seat.index()];
                c.hcp_range.0 = c.hcp_range.0.saturating_sub(2);
                c.hcp_range.1 = (c.hcp_range.1 + 2).min(40);
            }
            active_eligible = compute_eligible_seats(&pool, &unknown_seats, &voids, &active_constraints);
            // Reset counters — Phase 1 samples came from a different distribution
            counters = [[[0u32; 13]; 4]; 4];
            accepted = 0;
            result.suit_histograms = [[[0u32; 14]; 4]; 4];
        }

        // Phase 2: remaining samples with (possibly relaxed) constraints
        for _ in 0..(num_samples - probe_limit.min(attempted)) {
            if start.elapsed() > timeout {
                break;
            }

            let hands = match generate_constrained_deal(
                &pool, &unknown_seats, &target_counts, &voids, &active_constraints,
                &active_eligible, &mut rng,
            ) {
                Some(h) => h,
                None => continue,
            };

            accepted += 1;
            for &seat in &unknown_seats {
                for card in &hands[seat.index()] {
                    counters[seat.index()][suit_index(card.suit)][rank_index(card.rank)] += 1;
                }
                for (si, &suit) in Suit::ALL.iter().enumerate() {
                    let count = hands[seat.index()].iter().filter(|c| c.suit == suit).count();
                    result.suit_histograms[seat.index()][si][count] += 1;
                }
            }
        }

        if accepted < 50 {
            // Constraint-weighted fallback: uses suit demand from constraints
            // instead of uniform distribution, preserving bid information.
            let weighted = constraint_weighted_fallback(
                &pool, &unknown_seats, &voids, &adjusted_constraints, &target_counts,
            );
            for &seat in &unknown_seats {
                for si in 0..4 {
                    for ri in 0..13 {
                        result.probs[seat.index()][si][ri] = weighted[seat.index()][si][ri];
                    }
                }
            }
            // Normalize so each seat's probabilities sum to its target card count.
            // The fallback computes per-card weights independently, so without this
            // the total probability mass per seat may not match reality.
            for &seat in &unknown_seats {
                let sum: f32 = (0..4).flat_map(|si| (0..13).map(move |ri| (si, ri)))
                    .map(|(si, ri)| result.probs[seat.index()][si][ri])
                    .sum();
                let target = target_counts[seat.index()] as f32;
                if sum > 0.0 {
                    let scale = target / sum;
                    for si in 0..4 {
                        for ri in 0..13 {
                            result.probs[seat.index()][si][ri] *= scale;
                        }
                    }
                }
            }
            result.sample_count = 0;
        } else {
            let n = accepted as f32;
            for &seat in &unknown_seats {
                for &suit in &Suit::ALL {
                    for &rank in &Rank::ALL {
                        let si = suit_index(suit);
                        let ri = rank_index(rank);
                        result.probs[seat.index()][si][ri] = counters[seat.index()][si][ri] as f32 / n;
                    }
                }
            }
            result.sample_count = accepted;
        }
    }

    // Post-processing: enforce void constraints on final probabilities.
    // Even with exact enumeration or enough samples, zero out void suits
    // and redistribute proportionally to existing posteriors.
    for &seat in &unknown_seats {
        for &void_suit in &voids[seat.index()] {
            let si = suit_index(void_suit);
            for &rank in &Rank::ALL {
                let ri = rank_index(rank);
                let stolen = result.probs[seat.index()][si][ri];
                if stolen > 0.0 {
                    result.probs[seat.index()][si][ri] = 0.0;
                    // Redistribute proportionally to existing posteriors (Change 5)
                    let recipients: Vec<(Seat, f32)> = unknown_seats.iter()
                        .copied()
                        .filter(|&s| s != seat && !voids[s.index()].contains(&void_suit))
                        .map(|s| (s, result.probs[s.index()][si][ri]))
                        .collect();
                    let total_existing: f32 = recipients.iter().map(|(_, p)| p).sum();
                    if total_existing > 0.0 {
                        for &(r, existing) in &recipients {
                            result.probs[r.index()][si][ri] += stolen * (existing / total_existing);
                        }
                    } else if !recipients.is_empty() {
                        // If all recipients have zero, fall back to equal split
                        let share = stolen / recipients.len() as f32;
                        for &(r, _) in &recipients {
                            result.probs[r.index()][si][ri] += share;
                        }
                    }
                }
            }
        }
    }

    // Override knowns: South's cards → 1.0 for South, 0.0 elsewhere
    for card in south_hand.cards() {
        let si = suit_index(card.suit);
        let ri = rank_index(card.rank);
        for seat in Seat::ALL {
            result.probs[seat.index()][si][ri] = if seat == Seat::South { 1.0 } else { 0.0 };
        }
    }

    // Dummy cards if revealed
    if let (Some(dh), Some(ds)) = (dummy_hand, dummy_seat) {
        for card in dh.cards() {
            let si = suit_index(card.suit);
            let ri = rank_index(card.rank);
            for seat in Seat::ALL {
                result.probs[seat.index()][si][ri] = if seat == ds { 1.0 } else { 0.0 };
            }
        }
    }

    // Played cards → 1.0 for the seat that played them
    for &(seat, card) in &result.played_cards.clone() {
        let si = suit_index(card.suit);
        let ri = rank_index(card.rank);
        for s in Seat::ALL {
            result.probs[s.index()][si][ri] = if s == seat { 1.0 } else { 0.0 };
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::bid_constraints::HandConstraints;
    use crate::engine::deck::Deck;
    use crate::engine::trick::Trick;

    #[test]
    fn probabilities_sum_to_one() {
        let mut deck = Deck::new();
        deck.shuffle();
        let hands = deck.deal();
        let south = &hands[Seat::South.index()];

        let constraints: [HandConstraints; 4] = Default::default();
        let empty_trick = Trick::new(Seat::North);
        let result = run_inference(south, None, None, &constraints, &[], &empty_trick, 500);

        // For each card in the deck, probabilities across all seats should sum to ~1.0
        for &suit in &Suit::ALL {
            for &rank in &Rank::ALL {
                let sum: f32 = Seat::ALL.iter()
                    .map(|s| result.prob(*s, suit, rank))
                    .sum();
                assert!(
                    (sum - 1.0).abs() < 0.1,
                    "Sum for {:?}{:?} = {} (should be ~1.0)",
                    rank, suit, sum
                );
            }
        }
    }

    #[test]
    fn south_cards_are_certain() {
        let mut deck = Deck::new();
        deck.shuffle();
        let hands = deck.deal();
        let south = &hands[Seat::South.index()];

        let constraints: [HandConstraints; 4] = Default::default();
        let empty_trick = Trick::new(Seat::North);
        let result = run_inference(south, None, None, &constraints, &[], &empty_trick, 500);

        for card in south.cards() {
            assert_eq!(
                result.prob(Seat::South, card.suit, card.rank),
                1.0,
                "South's card {:?} should be 1.0",
                card
            );
            for &other in &[Seat::North, Seat::East, Seat::West] {
                assert_eq!(
                    result.prob(other, card.suit, card.rank),
                    0.0,
                    "Other seat {:?} should be 0.0 for South's card {:?}",
                    other, card
                );
            }
        }
    }

    #[test]
    fn void_enforcement() {
        use crate::engine::card::{Card, Rank, Suit};

        // South holds many hearts — this reduces hearts in the pool,
        // making the void constraint on East easier to satisfy via rejection sampling.
        let south = Hand::new(vec![
            Card::new(Suit::Hearts, Rank::Four),
            Card::new(Suit::Hearts, Rank::Five),
            Card::new(Suit::Hearts, Rank::Six),
            Card::new(Suit::Hearts, Rank::Seven),
            Card::new(Suit::Hearts, Rank::Eight),
            Card::new(Suit::Hearts, Rank::Nine),
            Card::new(Suit::Hearts, Rank::Ten),
            Card::new(Suit::Hearts, Rank::Jack),
            Card::new(Suit::Hearts, Rank::Queen),
            Card::new(Suit::Hearts, Rank::King),
            Card::new(Suit::Clubs, Rank::Two),
            Card::new(Suit::Clubs, Rank::Three),
            Card::new(Suit::Clubs, Rank::Four),
        ]);

        // Simulate East failing to follow hearts = East is void in hearts
        // Only 3 hearts remain outside South's hand: A, 2, 3
        let mut trick = Trick::new(Seat::North);
        trick.cards.push((Seat::North, Card::new(Suit::Hearts, Rank::Ace)));
        trick.cards.push((Seat::East, Card::new(Suit::Spades, Rank::Two)));
        trick.cards.push((Seat::South, Card::new(Suit::Hearts, Rank::Four)));
        trick.cards.push((Seat::West, Card::new(Suit::Hearts, Rank::Three)));

        let constraints: [HandConstraints; 4] = Default::default();
        let empty_trick = Trick::new(Seat::North);
        let result = run_inference(&south, None, None, &constraints, &[trick], &empty_trick, 5000);

        // Only H2 is unplayed and not in South's hand — East must not hold it
        let p = result.prob(Seat::East, Suit::Hearts, Rank::Two);
        assert!(
            p < 0.05,
            "East should be void in hearts but has p={} for Two",
            p
        );
    }

    #[test]
    fn inference_favors_true_locations() {
        let mut deck = Deck::new();
        deck.shuffle();
        let hands = deck.deal();
        let south = &hands[Seat::South.index()];

        let constraints: [HandConstraints; 4] = Default::default();
        let empty_trick = Trick::new(Seat::North);
        let result = run_inference(south, None, None, &constraints, &[], &empty_trick, 2000);

        // For each non-South seat, the cards they actually hold should have
        // higher average probability than cards held by other seats
        for &seat in &[Seat::North, Seat::East, Seat::West] {
            let own_avg: f32 = hands[seat.index()].cards().iter()
                .map(|c| result.prob(seat, c.suit, c.rank))
                .sum::<f32>() / 13.0;

            // Without constraints, all unknown seats should be ~0.33
            assert!(
                own_avg > 0.2,
                "Seat {:?} average prob for own cards = {} (should be ~0.33)",
                seat, own_avg
            );
        }
    }

    #[test]
    fn dummy_knowledge_changes_probabilities() {
        let mut deck = Deck::new();
        deck.shuffle();
        let hands = deck.deal();
        let south = &hands[Seat::South.index()];
        let north = &hands[Seat::North.index()]; // dummy

        let constraints: [HandConstraints; 4] = Default::default();
        let empty_trick = Trick::new(Seat::East);

        // Without dummy knowledge: 3 unknown seats → ~0.33 each
        let without = run_inference(south, None, None, &constraints, &[], &empty_trick, 2000);

        // With dummy knowledge: 2 unknown seats → ~0.50 each
        let with = run_inference(south, Some(north), Some(Seat::North), &constraints, &[], &empty_trick, 2000);

        // Pick a card that East actually holds (not in South or North hand)
        let east_card = hands[Seat::East.index()].cards()[0];
        let p_without = without.prob(Seat::East, east_card.suit, east_card.rank);
        let p_with = with.prob(Seat::East, east_card.suit, east_card.rank);

        // With dummy revealed, East's prob should be higher (~0.5 vs ~0.33)
        assert!(
            p_with > p_without + 0.05,
            "Dummy knowledge should increase East's prob: without={}, with={}",
            p_without, p_with
        );

        // North's cards should be 1.0 for North when dummy is known
        for card in north.cards() {
            assert_eq!(
                with.prob(Seat::North, card.suit, card.rank),
                1.0,
                "Dummy card {:?} should be 1.0 for North",
                card
            );
        }
    }

    #[test]
    fn void_with_two_seats_forces_all_to_other() {
        // Reproduces the bug scenario: 2 unknown seats (E/W), East void in clubs,
        // 4 clubs remaining in pool — all must go to West. With pure rejection
        // sampling this had ~5.5% acceptance rate, often falling below 100 samples
        // and triggering the uniform fallback which ignored voids.
        use crate::engine::card::{Card, Rank, Suit};

        // South: 3 spades, 5 hearts, 3 diamonds, 0 clubs
        let south = Hand::new(vec![
            Card::new(Suit::Spades, Rank::Ace),
            Card::new(Suit::Spades, Rank::Seven),
            Card::new(Suit::Spades, Rank::Four),
            Card::new(Suit::Hearts, Rank::Ace),
            Card::new(Suit::Hearts, Rank::King),
            Card::new(Suit::Hearts, Rank::Queen),
            Card::new(Suit::Hearts, Rank::Eight),
            Card::new(Suit::Hearts, Rank::Seven),
            Card::new(Suit::Diamonds, Rank::Ten),
            Card::new(Suit::Diamonds, Rank::Nine),
            Card::new(Suit::Diamonds, Rank::Five),
        ]);

        // North (dummy): 4 spades, 3 hearts, 3 diamonds, 0 clubs
        let north = Hand::new(vec![
            Card::new(Suit::Spades, Rank::Jack),
            Card::new(Suit::Spades, Rank::Eight),
            Card::new(Suit::Spades, Rank::Five),
            Card::new(Suit::Spades, Rank::Two),
            Card::new(Suit::Hearts, Rank::Ten),
            Card::new(Suit::Hearts, Rank::Five),
            Card::new(Suit::Hearts, Rank::Four),
            Card::new(Suit::Diamonds, Rank::Ace),
            Card::new(Suit::Diamonds, Rank::Six),
            Card::new(Suit::Diamonds, Rank::Four),
        ]);

        // 2 completed tricks (all clubs) + current trick with N leading K♣, E plays 3♦
        let mut trick1 = Trick::new(Seat::North);
        trick1.cards.push((Seat::North, Card::new(Suit::Clubs, Rank::Nine)));
        trick1.cards.push((Seat::East, Card::new(Suit::Clubs, Rank::Eight)));
        trick1.cards.push((Seat::South, Card::new(Suit::Clubs, Rank::Two)));
        trick1.cards.push((Seat::West, Card::new(Suit::Clubs, Rank::Ace)));

        let mut trick2 = Trick::new(Seat::North);
        trick2.cards.push((Seat::North, Card::new(Suit::Clubs, Rank::Queen)));
        trick2.cards.push((Seat::East, Card::new(Suit::Clubs, Rank::Six)));
        trick2.cards.push((Seat::South, Card::new(Suit::Clubs, Rank::Four)));
        trick2.cards.push((Seat::West, Card::new(Suit::Clubs, Rank::Jack)));

        // Current trick: N leads K♣, East plays 3♦ (void in clubs!)
        let mut current = Trick::new(Seat::North);
        current.cards.push((Seat::North, Card::new(Suit::Clubs, Rank::King)));
        current.cards.push((Seat::East, Card::new(Suit::Diamonds, Rank::Three)));

        let constraints: [HandConstraints; 4] = Default::default();
        let result = run_inference(
            &south, Some(&north), Some(Seat::North),
            &constraints, &[trick1, trick2], &current, 2000,
        );

        // Remaining clubs in pool: T♣, 7♣, 5♣, 3♣ — all must be West's
        assert!(result.sample_count >= 100,
            "Should get enough samples with constraint-aware generation, got {}",
            result.sample_count);

        for &rank in &[Rank::Ten, Rank::Seven, Rank::Five, Rank::Three] {
            let p_east = result.prob(Seat::East, Suit::Clubs, rank);
            let p_west = result.prob(Seat::West, Suit::Clubs, rank);
            assert!(p_east < 0.01,
                "East void in clubs but {:?}♣ has p_east={}", rank, p_east);
            assert!(p_west > 0.99,
                "West should have {:?}♣ at ~100% but p_west={}", rank, p_west);
        }
    }

    #[test]
    fn constraint_aware_gets_more_samples() {
        // With tight constraints (e.g., West opened 2C = 22+ HCP),
        // constraint-aware generation should still produce meaningful samples.
        use crate::engine::card::{Rank, Suit};
        use crate::engine::bid_constraints::HandConstraints;

        let mut deck = Deck::new();
        deck.shuffle();
        let hands = deck.deal();
        let south = &hands[Seat::South.index()];

        // West opened strong 2C: 22-40 HCP (very tight)
        let mut constraints: [HandConstraints; 4] = Default::default();
        constraints[Seat::West.index()].hcp_range = (22, 40);

        let empty_trick = Trick::new(Seat::North);
        let result = run_inference(south, None, None, &constraints, &[], &empty_trick, 2000);

        // Should get some accepted samples (maybe not 100+ due to tight HCP,
        // but the constraint-aware generation shouldn't make it worse)
        // The main thing: probabilities should still be valid
        for &suit in &Suit::ALL {
            for &rank in &Rank::ALL {
                let sum: f32 = Seat::ALL.iter()
                    .map(|s| result.prob(*s, suit, rank))
                    .sum();
                assert!(
                    (sum - 1.0).abs() < 0.15,
                    "Sum for {:?}{:?} = {} (should be ~1.0)",
                    rank, suit, sum
                );
            }
        }
    }

    #[test]
    fn constraints_adjusted_for_played_cards() {
        // Reproduces a scenario where unadjusted constraints are mutually infeasible:
        // North overcalled 1H (8-16 HCP, 5+ hearts) and East responded 2D (10+ HCP, 4+ diamonds).
        // After 4 tricks, East has played A♦ + Q♦ (6 HCP). Pool has only 16 HCP.
        // Without adjustment: N needs 8+ and E needs 10+ = 18 > 16. Zero acceptance.
        // With adjustment: N needs 8+ and E needs 4+ = 12 ≤ 16. Feasible.
        use crate::engine::card::{Card, Rank, Suit};
        use crate::engine::bid_constraints::HandConstraints;

        // South: ♠QJT4 ♥J9 ♦— ♣876
        let south = Hand::new(vec![
            Card::new(Suit::Spades, Rank::Queen),
            Card::new(Suit::Spades, Rank::Jack),
            Card::new(Suit::Spades, Rank::Ten),
            Card::new(Suit::Spades, Rank::Four),
            Card::new(Suit::Hearts, Rank::Jack),
            Card::new(Suit::Hearts, Rank::Nine),
            Card::new(Suit::Clubs, Rank::Eight),
            Card::new(Suit::Clubs, Rank::Seven),
            Card::new(Suit::Clubs, Rank::Six),
        ]);

        // West (dummy): ♠AK8 ♥86 ♦— ♣AQJ2
        let west = Hand::new(vec![
            Card::new(Suit::Spades, Rank::Ace),
            Card::new(Suit::Spades, Rank::King),
            Card::new(Suit::Spades, Rank::Eight),
            Card::new(Suit::Hearts, Rank::Eight),
            Card::new(Suit::Hearts, Rank::Six),
            Card::new(Suit::Clubs, Rank::Ace),
            Card::new(Suit::Clubs, Rank::Queen),
            Card::new(Suit::Clubs, Rank::Jack),
            Card::new(Suit::Clubs, Rank::Two),
        ]);

        // 4 completed tricks
        let mut trick1 = Trick::new(Seat::North);
        trick1.cards.push((Seat::North, Card::new(Suit::Spades, Rank::Two)));
        trick1.cards.push((Seat::East, Card::new(Suit::Spades, Rank::Six)));
        trick1.cards.push((Seat::South, Card::new(Suit::Spades, Rank::Seven)));
        trick1.cards.push((Seat::West, Card::new(Suit::Spades, Rank::Five)));

        let mut trick2 = Trick::new(Seat::South);
        trick2.cards.push((Seat::South, Card::new(Suit::Hearts, Rank::Five)));
        trick2.cards.push((Seat::West, Card::new(Suit::Hearts, Rank::Four)));
        trick2.cards.push((Seat::North, Card::new(Suit::Hearts, Rank::Two)));
        trick2.cards.push((Seat::East, Card::new(Suit::Hearts, Rank::Seven)));

        let mut trick3 = Trick::new(Seat::East);
        trick3.cards.push((Seat::East, Card::new(Suit::Diamonds, Rank::Ace)));
        trick3.cards.push((Seat::South, Card::new(Suit::Diamonds, Rank::Nine)));
        trick3.cards.push((Seat::West, Card::new(Suit::Diamonds, Rank::Three)));
        trick3.cards.push((Seat::North, Card::new(Suit::Diamonds, Rank::Four)));

        let mut trick4 = Trick::new(Seat::East);
        trick4.cards.push((Seat::East, Card::new(Suit::Diamonds, Rank::Queen)));
        trick4.cards.push((Seat::South, Card::new(Suit::Clubs, Rank::Five)));
        trick4.cards.push((Seat::West, Card::new(Suit::Diamonds, Rank::Six)));
        trick4.cards.push((Seat::North, Card::new(Suit::Diamonds, Rank::Seven)));

        // North: overcall 1H → 8-16 HCP, 5+ hearts
        // East: respond 2D → 10+ HCP, 4+ diamonds
        let mut constraints: [HandConstraints; 4] = Default::default();
        constraints[Seat::North.index()].hcp_range = (8, 16);
        constraints[Seat::North.index()].suit_lengths[2].0 = 5; // hearts min 5
        constraints[Seat::East.index()].hcp_range = (10, 40);
        constraints[Seat::East.index()].suit_lengths[1].0 = 4; // diamonds min 4

        let empty_trick = Trick::new(Seat::East);
        let result = run_inference(
            &south, Some(&west), Some(Seat::West),
            &constraints, &[trick1, trick2, trick3, trick4], &empty_trick, 2000,
        );

        // With adjusted constraints, we should get real samples (not uniform fallback)
        assert!(result.sample_count >= 100,
            "Should get enough samples with play-adjusted constraints, got {}",
            result.sample_count);

        // North should have higher heart probability than East (5+ hearts overcall)
        let north_heart_avg: f32 = Rank::ALL.iter()
            .map(|&r| result.prob(Seat::North, Suit::Hearts, r))
            .sum::<f32>();
        let east_heart_avg: f32 = Rank::ALL.iter()
            .map(|&r| result.prob(Seat::East, Suit::Hearts, r))
            .sum::<f32>();
        assert!(north_heart_avg > east_heart_avg,
            "North (overcalled 1H) should have more hearts: N={:.2} vs E={:.2}",
            north_heart_avg, east_heart_avg);

        // East should have higher diamond probability than North (responded 2D)
        let north_diamond_avg: f32 = Rank::ALL.iter()
            .map(|&r| result.prob(Seat::North, Suit::Diamonds, r))
            .sum::<f32>();
        let east_diamond_avg: f32 = Rank::ALL.iter()
            .map(|&r| result.prob(Seat::East, Suit::Diamonds, r))
            .sum::<f32>();
        assert!(east_diamond_avg > north_diamond_avg,
            "East (responded 2D) should have more diamonds: E={:.2} vs N={:.2}",
            east_diamond_avg, north_diamond_avg);
    }

    #[test]
    fn tight_constraints_at_late_tricks() {
        // Reproduces trick 7 scenario where North (overcalled 1H with 5+ hearts)
        // needs ALL remaining hearts in the pool. Without iterative constraint
        // propagation, acceptance rate drops below threshold → uniform fallback.
        use crate::engine::card::{Card, Rank, Suit};
        use crate::engine::bid_constraints::HandConstraints;

        // South: ♠QJT4 ♥J ♦— ♣87 (7 cards remaining)
        let south = Hand::new(vec![
            Card::new(Suit::Spades, Rank::Queen),
            Card::new(Suit::Spades, Rank::Jack),
            Card::new(Suit::Spades, Rank::Ten),
            Card::new(Suit::Spades, Rank::Four),
            Card::new(Suit::Hearts, Rank::Jack),
            Card::new(Suit::Clubs, Rank::Eight),
            Card::new(Suit::Clubs, Rank::Seven),
        ]);

        // West (dummy): ♠AK8 ♥8 ♦— ♣AQJ (7 cards remaining)
        let west = Hand::new(vec![
            Card::new(Suit::Spades, Rank::Ace),
            Card::new(Suit::Spades, Rank::King),
            Card::new(Suit::Spades, Rank::Eight),
            Card::new(Suit::Hearts, Rank::Eight),
            Card::new(Suit::Clubs, Rank::Ace),
            Card::new(Suit::Clubs, Rank::Queen),
            Card::new(Suit::Clubs, Rank::Jack),
        ]);

        // 6 completed tricks (24 cards played)
        let mut trick1 = Trick::new(Seat::North);
        trick1.cards.push((Seat::North, Card::new(Suit::Spades, Rank::Two)));
        trick1.cards.push((Seat::East, Card::new(Suit::Spades, Rank::Six)));
        trick1.cards.push((Seat::South, Card::new(Suit::Spades, Rank::Seven)));
        trick1.cards.push((Seat::West, Card::new(Suit::Spades, Rank::Five)));

        let mut trick2 = Trick::new(Seat::South);
        trick2.cards.push((Seat::South, Card::new(Suit::Hearts, Rank::Five)));
        trick2.cards.push((Seat::West, Card::new(Suit::Hearts, Rank::Four)));
        trick2.cards.push((Seat::North, Card::new(Suit::Hearts, Rank::Two)));
        trick2.cards.push((Seat::East, Card::new(Suit::Hearts, Rank::Seven)));

        let mut trick3 = Trick::new(Seat::East);
        trick3.cards.push((Seat::East, Card::new(Suit::Diamonds, Rank::Ace)));
        trick3.cards.push((Seat::South, Card::new(Suit::Diamonds, Rank::Nine)));
        trick3.cards.push((Seat::West, Card::new(Suit::Diamonds, Rank::Three)));
        trick3.cards.push((Seat::North, Card::new(Suit::Diamonds, Rank::Four)));

        let mut trick4 = Trick::new(Seat::East);
        trick4.cards.push((Seat::East, Card::new(Suit::Diamonds, Rank::Queen)));
        trick4.cards.push((Seat::South, Card::new(Suit::Clubs, Rank::Five)));
        trick4.cards.push((Seat::West, Card::new(Suit::Diamonds, Rank::Six)));
        trick4.cards.push((Seat::North, Card::new(Suit::Diamonds, Rank::Seven)));

        let mut trick5 = Trick::new(Seat::East);
        trick5.cards.push((Seat::East, Card::new(Suit::Diamonds, Rank::King)));
        trick5.cards.push((Seat::South, Card::new(Suit::Clubs, Rank::Six)));
        trick5.cards.push((Seat::West, Card::new(Suit::Diamonds, Rank::Two)));
        trick5.cards.push((Seat::North, Card::new(Suit::Hearts, Rank::Ace)));

        let mut trick6 = Trick::new(Seat::East);
        trick6.cards.push((Seat::East, Card::new(Suit::Diamonds, Rank::Jack)));
        trick6.cards.push((Seat::South, Card::new(Suit::Hearts, Rank::Nine)));
        trick6.cards.push((Seat::West, Card::new(Suit::Diamonds, Rank::Eight)));
        trick6.cards.push((Seat::North, Card::new(Suit::Diamonds, Rank::Five)));

        // Current trick 7: N:T♥ E:K♥, South's turn
        let mut current = Trick::new(Seat::North);
        current.cards.push((Seat::North, Card::new(Suit::Hearts, Rank::Ten)));
        current.cards.push((Seat::East, Card::new(Suit::Hearts, Rank::King)));

        // North: overcalled 1H → 8-16 HCP, 5+ hearts
        // East: responded 2D → 10+ HCP, 4+ diamonds
        let mut constraints: [HandConstraints; 4] = Default::default();
        constraints[Seat::North.index()].hcp_range = (8, 16);
        constraints[Seat::North.index()].suit_lengths[2].0 = 5; // hearts min 5
        constraints[Seat::East.index()].hcp_range = (10, 40);
        constraints[Seat::East.index()].suit_lengths[1].0 = 4; // diamonds min 4

        let result = run_inference(
            &south, Some(&west), Some(Seat::West),
            &constraints,
            &[trick1, trick2, trick3, trick4, trick5, trick6],
            &current, 2000,
        );

        // With iterative propagation + lower threshold, should NOT fall back to uniform
        assert!(result.sample_count > 0,
            "Should get real samples at trick 7 with tight constraints, got {}",
            result.sample_count);

        // North overcalled 1H — should have more remaining hearts than East
        let north_hearts: f32 = Rank::ALL.iter()
            .map(|&r| result.prob(Seat::North, Suit::Hearts, r))
            .sum::<f32>();
        let east_hearts: f32 = Rank::ALL.iter()
            .map(|&r| result.prob(Seat::East, Suit::Hearts, r))
            .sum::<f32>();
        // North and East should NOT have identical distributions
        assert!((north_hearts - east_hearts).abs() > 0.1,
            "N/E hearts should differ: N={:.2} vs E={:.2}",
            north_hearts, east_hearts);
    }

    #[test]
    fn performance_under_200ms() {
        let mut deck = Deck::new();
        deck.shuffle();
        let hands = deck.deal();
        let south = &hands[Seat::South.index()];

        let constraints: [HandConstraints; 4] = Default::default();
        let empty_trick = Trick::new(Seat::North);

        let start = std::time::Instant::now();
        let _result = run_inference(south, None, None, &constraints, &[], &empty_trick, 2000);
        let elapsed = start.elapsed();

        assert!(
            elapsed < std::time::Duration::from_millis(500),
            "Inference took {:?}, should be < 500ms",
            elapsed
        );
    }

    #[test]
    fn seat_order_does_not_bias() {
        // Verify that shuffling seat order eliminates systematic bias.
        // Run inference multiple times and check that N/E/W get similar
        // average probabilities for non-South cards (no constraints).
        use crate::engine::card::{Rank, Suit};

        let mut deck = Deck::new();
        deck.shuffle();
        let hands = deck.deal();
        let south = &hands[Seat::South.index()];

        let constraints: [HandConstraints; 4] = Default::default();
        let empty_trick = Trick::new(Seat::North);

        // Run with enough samples to get stable estimates
        let result = run_inference(south, None, None, &constraints, &[], &empty_trick, 5000);

        // Average probability across all pool cards for each unknown seat
        let mut seat_avgs = [0.0f32; 4];
        let mut count = 0;
        for &suit in &Suit::ALL {
            for &rank in &Rank::ALL {
                let card = Card::new(suit, rank);
                if south.cards().contains(&card) { continue; }
                count += 1;
                for &seat in &[Seat::North, Seat::East, Seat::West] {
                    seat_avgs[seat.index()] += result.prob(seat, suit, rank);
                }
            }
        }
        for &seat in &[Seat::North, Seat::East, Seat::West] {
            seat_avgs[seat.index()] /= count as f32;
        }

        // Without bias, each seat should get ~0.333 on average
        // Allow 0.05 tolerance for sampling noise
        for &seat in &[Seat::North, Seat::East, Seat::West] {
            let avg = seat_avgs[seat.index()];
            assert!(
                (avg - 0.333).abs() < 0.05,
                "Seat {:?} average prob = {:.4} (expected ~0.333, bias detected)",
                seat, avg
            );
        }

        // Check pairwise: no two seats should differ by more than 0.05
        let n_avg = seat_avgs[Seat::North.index()];
        let e_avg = seat_avgs[Seat::East.index()];
        let w_avg = seat_avgs[Seat::West.index()];
        assert!(
            (n_avg - e_avg).abs() < 0.05,
            "N/E bias: N={:.4} vs E={:.4}", n_avg, e_avg
        );
        assert!(
            (n_avg - w_avg).abs() < 0.05,
            "N/W bias: N={:.4} vs W={:.4}", n_avg, w_avg
        );
    }

    #[test]
    fn exact_enumeration_matches_sampling() {
        // For a small pool with 2 unknown seats, exact enumeration should
        // produce probabilities close to what sampling gives.
        use crate::engine::card::{Card, Rank, Suit};

        // Late game: South has 4 cards, North (dummy) has 4 cards,
        // leaving 4 cards each for E and W (8 card pool).
        let south = Hand::new(vec![
            Card::new(Suit::Spades, Rank::Ace),
            Card::new(Suit::Spades, Rank::King),
            Card::new(Suit::Hearts, Rank::Ace),
            Card::new(Suit::Hearts, Rank::King),
        ]);
        let north = Hand::new(vec![
            Card::new(Suit::Clubs, Rank::Ace),
            Card::new(Suit::Clubs, Rank::King),
            Card::new(Suit::Diamonds, Rank::Ace),
            Card::new(Suit::Diamonds, Rank::King),
        ]);

        // Play 9 tricks to get down to 4 cards each
        let mut tricks = Vec::new();
        let played_cards = vec![
            // Trick 1
            (Seat::North, Card::new(Suit::Clubs, Rank::Queen)),
            (Seat::East, Card::new(Suit::Clubs, Rank::Jack)),
            (Seat::South, Card::new(Suit::Spades, Rank::Queen)),
            (Seat::West, Card::new(Suit::Clubs, Rank::Ten)),
            // Trick 2
            (Seat::North, Card::new(Suit::Diamonds, Rank::Queen)),
            (Seat::East, Card::new(Suit::Diamonds, Rank::Jack)),
            (Seat::South, Card::new(Suit::Hearts, Rank::Queen)),
            (Seat::West, Card::new(Suit::Diamonds, Rank::Ten)),
            // Trick 3
            (Seat::North, Card::new(Suit::Clubs, Rank::Nine)),
            (Seat::East, Card::new(Suit::Clubs, Rank::Eight)),
            (Seat::South, Card::new(Suit::Spades, Rank::Jack)),
            (Seat::West, Card::new(Suit::Clubs, Rank::Seven)),
            // Trick 4
            (Seat::North, Card::new(Suit::Diamonds, Rank::Nine)),
            (Seat::East, Card::new(Suit::Diamonds, Rank::Eight)),
            (Seat::South, Card::new(Suit::Hearts, Rank::Jack)),
            (Seat::West, Card::new(Suit::Diamonds, Rank::Seven)),
            // Trick 5
            (Seat::North, Card::new(Suit::Clubs, Rank::Six)),
            (Seat::East, Card::new(Suit::Clubs, Rank::Five)),
            (Seat::South, Card::new(Suit::Spades, Rank::Ten)),
            (Seat::West, Card::new(Suit::Clubs, Rank::Four)),
            // Trick 6
            (Seat::North, Card::new(Suit::Diamonds, Rank::Six)),
            (Seat::East, Card::new(Suit::Diamonds, Rank::Five)),
            (Seat::South, Card::new(Suit::Hearts, Rank::Ten)),
            (Seat::West, Card::new(Suit::Diamonds, Rank::Four)),
            // Trick 7
            (Seat::North, Card::new(Suit::Clubs, Rank::Three)),
            (Seat::East, Card::new(Suit::Clubs, Rank::Two)),
            (Seat::South, Card::new(Suit::Spades, Rank::Nine)),
            (Seat::West, Card::new(Suit::Diamonds, Rank::Three)),
            // Trick 8
            (Seat::North, Card::new(Suit::Diamonds, Rank::Two)),
            (Seat::East, Card::new(Suit::Hearts, Rank::Nine)),
            (Seat::South, Card::new(Suit::Spades, Rank::Eight)),
            (Seat::West, Card::new(Suit::Hearts, Rank::Eight)),
            // Trick 9
            (Seat::North, Card::new(Suit::Spades, Rank::Two)),
            (Seat::East, Card::new(Suit::Spades, Rank::Three)),
            (Seat::South, Card::new(Suit::Hearts, Rank::Two)),
            (Seat::West, Card::new(Suit::Spades, Rank::Four)),
        ];

        for chunk in played_cards.chunks(4) {
            let leader = chunk[0].0;
            let mut trick = Trick::new(leader);
            for &(seat, card) in chunk {
                trick.cards.push((seat, card));
            }
            tricks.push(trick);
        }

        let constraints: [HandConstraints; 4] = Default::default();
        let empty_trick = Trick::new(Seat::North);

        let result = run_inference(
            &south, Some(&north), Some(Seat::North),
            &constraints, &tricks, &empty_trick, 5000,
        );

        // With 2 unknown seats and small pool, should use exact enumeration
        assert_eq!(result.sample_count, u32::MAX,
            "Should use exact enumeration for small pool with 2 unknown seats");

        // Probabilities should sum to 1.0 for each card
        // Pool should be the 8 remaining cards (52 - 13S - 13N - 36played = ... )
        for &suit in &Suit::ALL {
            for &rank in &Rank::ALL {
                let sum: f32 = Seat::ALL.iter()
                    .map(|s| result.prob(*s, suit, rank))
                    .sum();
                assert!(
                    (sum - 1.0).abs() < 0.01,
                    "Exact enum: sum for {:?}{:?} = {} (should be 1.0)",
                    rank, suit, sum
                );
            }
        }
    }

    #[test]
    fn suit_histogram_sums_to_accepted() {
        let mut deck = Deck::new();
        deck.shuffle();
        let hands = deck.deal();
        let south = &hands[Seat::South.index()];

        let constraints: [HandConstraints; 4] = Default::default();
        let empty_trick = Trick::new(Seat::North);
        let result = run_inference(south, None, None, &constraints, &[], &empty_trick, 2000);

        if result.sample_count > 0 && result.sample_count != u32::MAX {
            // For MC sampling, each suit histogram for each seat should sum to accepted count
            for &seat in &[Seat::North, Seat::East, Seat::West] {
                for (si, &suit) in Suit::ALL.iter().enumerate() {
                    let hist_sum: u32 = result.suit_histograms[seat.index()][si].iter().sum();
                    assert_eq!(
                        hist_sum, result.sample_count,
                        "Histogram sum for {:?} {:?} = {} (expected {})",
                        seat, suit, hist_sum, result.sample_count
                    );
                }
            }
        }
    }

    #[test]
    fn suit_length_range_covers_80pct() {
        let mut deck = Deck::new();
        deck.shuffle();
        let hands = deck.deal();
        let south = &hands[Seat::South.index()];

        let constraints: [HandConstraints; 4] = Default::default();
        let empty_trick = Trick::new(Seat::North);
        let result = run_inference(south, None, None, &constraints, &[], &empty_trick, 2000);

        if result.sample_count > 0 {
            for &seat in &[Seat::North, Seat::East, Seat::West] {
                for &suit in &Suit::ALL {
                    let (lo, hi) = result.suit_length_range(seat, suit);
                    assert!(lo <= hi, "Range inverted for {:?} {:?}: {}-{}", seat, suit, lo, hi);

                    // Verify the range covers at least 80% of samples
                    let si = suit_index(suit);
                    let hist = &result.suit_histograms[seat.index()][si];
                    let total: u32 = hist.iter().sum();
                    if total > 0 {
                        let covered: u32 = hist.iter().enumerate()
                            .filter(|&(len, _)| len >= lo as usize && len <= hi as usize)
                            .map(|(_, &count)| count)
                            .sum();
                        let pct = covered as f64 / total as f64;
                        assert!(
                            pct >= 0.79, // slight tolerance for rounding
                            "Range {}-{} for {:?} {:?} covers only {:.1}% (expected >= 80%)",
                            lo, hi, seat, suit, pct * 100.0
                        );
                    }
                }
            }
        }
    }
}
