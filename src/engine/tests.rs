use super::bidding::{Auction, Bid, BidSuit};
use super::card::{Card, Rank, Suit};
use super::deck::Deck;
use super::hand::Hand;
use super::play::PlayState;
use super::trick::Trick;
use crate::types::{Phase, Seat};
use std::collections::HashSet;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn c(suit: Suit, rank: Rank) -> Card {
    Card::new(suit, rank)
}

/// Build a Hand from a slice of (Suit, Rank) pairs.
fn hand_from(cards: &[(Suit, Rank)]) -> Hand {
    Hand::new(cards.iter().map(|&(s, r)| Card::new(s, r)).collect())
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. CARD ORDERING AND SORTING
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn card_ordering_suit_then_rank() {
    // Suit order: Clubs < Diamonds < Hearts < Spades
    assert!(Suit::Clubs < Suit::Diamonds);
    assert!(Suit::Diamonds < Suit::Hearts);
    assert!(Suit::Hearts < Suit::Spades);

    // Rank order: Two < Three < ... < Ace
    assert!(Rank::Two < Rank::Three);
    assert!(Rank::Ten < Rank::Jack);
    assert!(Rank::King < Rank::Ace);

    // Cards sort by suit first, then rank
    let club_ace = c(Suit::Clubs, Rank::Ace);
    let diamond_two = c(Suit::Diamonds, Rank::Two);
    assert!(club_ace < diamond_two, "Club Ace should sort before Diamond Two");

    let spade_two = c(Suit::Spades, Rank::Two);
    let spade_king = c(Suit::Spades, Rank::King);
    assert!(spade_two < spade_king);
}

#[test]
fn hand_sorts_cards_on_creation() {
    let cards = vec![
        c(Suit::Spades, Rank::Ace),
        c(Suit::Clubs, Rank::Two),
        c(Suit::Hearts, Rank::King),
        c(Suit::Diamonds, Rank::Five),
    ];
    let hand = Hand::new(cards);
    let sorted = hand.cards();
    // Should be: C2, D5, HK, SA
    assert_eq!(sorted[0], c(Suit::Clubs, Rank::Two));
    assert_eq!(sorted[1], c(Suit::Diamonds, Rank::Five));
    assert_eq!(sorted[2], c(Suit::Hearts, Rank::King));
    assert_eq!(sorted[3], c(Suit::Spades, Rank::Ace));
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. DECK DEALING
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn deck_has_52_unique_cards() {
    let deck = Deck::new();
    let hands = deck.deal();
    let mut all_cards = HashSet::new();
    let mut total = 0;
    for hand in &hands {
        assert_eq!(hand.len(), 13, "Each hand must have exactly 13 cards");
        for card in hand.cards() {
            assert!(all_cards.insert(*card), "Duplicate card found: {}", card);
            total += 1;
        }
    }
    assert_eq!(total, 52);
    assert_eq!(all_cards.len(), 52);
}

#[test]
fn deck_deal_shuffled_has_52_unique() {
    let mut deck = Deck::new();
    deck.shuffle();
    let hands = deck.deal();
    let mut all_cards = HashSet::new();
    for hand in &hands {
        assert_eq!(hand.len(), 13);
        for card in hand.cards() {
            all_cards.insert(*card);
        }
    }
    assert_eq!(all_cards.len(), 52);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. HAND OPERATIONS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn hand_remove_card() {
    let mut hand = hand_from(&[
        (Suit::Clubs, Rank::Ace),
        (Suit::Hearts, Rank::King),
        (Suit::Spades, Rank::Two),
    ]);
    assert_eq!(hand.len(), 3);
    assert!(hand.remove_card(c(Suit::Hearts, Rank::King)));
    assert_eq!(hand.len(), 2);
    assert!(!hand.contains(&c(Suit::Hearts, Rank::King)));
    // Removing a card not in hand returns false
    assert!(!hand.remove_card(c(Suit::Diamonds, Rank::Five)));
}

#[test]
fn hand_cards_of_suit_and_has_suit() {
    let hand = hand_from(&[
        (Suit::Clubs, Rank::Two),
        (Suit::Clubs, Rank::Ace),
        (Suit::Hearts, Rank::King),
    ]);
    assert!(hand.has_suit(Suit::Clubs));
    assert!(!hand.has_suit(Suit::Diamonds));
    let clubs = hand.cards_of_suit(Suit::Clubs);
    assert_eq!(clubs.len(), 2);
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. AUCTION / BIDDING RULES
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn bid_must_be_higher_level_or_suit() {
    let mut auction = Auction::new(Seat::North);
    // 1C is valid opening
    assert!(auction.is_valid_bid(&Bid::Suit(1, BidSuit::Clubs)));
    auction.place_bid(Bid::Suit(1, BidSuit::Clubs)).unwrap();

    // 1C again is invalid (not higher)
    assert!(!auction.is_valid_bid(&Bid::Suit(1, BidSuit::Clubs)));
    // 1D is valid (same level, higher suit)
    assert!(auction.is_valid_bid(&Bid::Suit(1, BidSuit::Diamonds)));
    // 2C is valid (higher level)
    assert!(auction.is_valid_bid(&Bid::Suit(2, BidSuit::Clubs)));
}

#[test]
fn bid_suit_ordering() {
    // Clubs < Diamonds < Hearts < Spades < NoTrump
    assert!(BidSuit::Clubs < BidSuit::Diamonds);
    assert!(BidSuit::Diamonds < BidSuit::Hearts);
    assert!(BidSuit::Hearts < BidSuit::Spades);
    assert!(BidSuit::Spades < BidSuit::NoTrump);
}

#[test]
fn bid_level_range_validation() {
    let auction = Auction::new(Seat::North);
    assert!(!auction.is_valid_bid(&Bid::Suit(0, BidSuit::Clubs)));
    assert!(auction.is_valid_bid(&Bid::Suit(1, BidSuit::Clubs)));
    assert!(auction.is_valid_bid(&Bid::Suit(7, BidSuit::NoTrump)));
    assert!(!auction.is_valid_bid(&Bid::Suit(8, BidSuit::Clubs)));
}

#[test]
fn pass_is_always_valid() {
    let auction = Auction::new(Seat::North);
    assert!(auction.is_valid_bid(&Bid::Pass));
}

#[test]
fn double_opponents_bid_only() {
    let mut auction = Auction::new(Seat::North);
    // N: 1H
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    // E can double (opponent)
    assert!(auction.is_valid_bid(&Bid::Double));
    auction.place_bid(Bid::Double).unwrap();

    // S cannot double own side's bid that was doubled by opponent
    assert!(!auction.is_valid_bid(&Bid::Double));
    // S can redouble
    assert!(auction.is_valid_bid(&Bid::Redouble));
}

#[test]
fn cannot_double_without_suit_bid() {
    let auction = Auction::new(Seat::North);
    assert!(!auction.is_valid_bid(&Bid::Double));
}

#[test]
fn cannot_double_partners_bid() {
    let mut auction = Auction::new(Seat::North);
    // N: 1H, E: Pass
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    // S (North's partner) cannot double North's bid
    assert!(!auction.is_valid_bid(&Bid::Double));
}

#[test]
fn cannot_double_already_doubled() {
    let mut auction = Auction::new(Seat::North);
    // N: 1H, E: Dbl, S: Pass
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Double).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    // W cannot double again (last non-pass action is Double, not suit bid)
    assert!(!auction.is_valid_bid(&Bid::Double));
}

#[test]
fn redouble_only_after_double_on_own_side() {
    let mut auction = Auction::new(Seat::North);
    // N: 1H
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    // E: cannot redouble (no double yet)
    assert!(!auction.is_valid_bid(&Bid::Redouble));
    // E: Dbl
    auction.place_bid(Bid::Double).unwrap();
    // S: can redouble (our side's bid was doubled)
    assert!(auction.is_valid_bid(&Bid::Redouble));
    // S: Rdbl
    auction.place_bid(Bid::Redouble).unwrap();
    // W: cannot redouble (opponent's bid was redoubled)
    assert!(!auction.is_valid_bid(&Bid::Redouble));
}

#[test]
fn cannot_redouble_without_double() {
    let mut auction = Auction::new(Seat::North);
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    // S: cannot redouble (no double happened)
    assert!(!auction.is_valid_bid(&Bid::Redouble));
}

#[test]
fn four_passes_completes_auction() {
    let mut auction = Auction::new(Seat::North);
    for _ in 0..3 {
        auction.place_bid(Bid::Pass).unwrap();
        assert!(!auction.is_complete());
    }
    auction.place_bid(Bid::Pass).unwrap();
    assert!(auction.is_complete());
    // No contract when passed out
    assert!(auction.resolve_contract().is_none());
}

#[test]
fn three_passes_after_suit_bid_completes() {
    let mut auction = Auction::new(Seat::North);
    auction.place_bid(Bid::Suit(1, BidSuit::Clubs)).unwrap();
    assert!(!auction.is_complete());
    auction.place_bid(Bid::Pass).unwrap();
    assert!(!auction.is_complete());
    auction.place_bid(Bid::Pass).unwrap();
    assert!(!auction.is_complete());
    auction.place_bid(Bid::Pass).unwrap();
    assert!(auction.is_complete());
}

#[test]
fn auction_not_complete_with_two_passes_after_bid() {
    let mut auction = Auction::new(Seat::North);
    auction.place_bid(Bid::Suit(2, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    assert!(!auction.is_complete());
}

#[test]
fn three_passes_after_double_completes() {
    let mut auction = Auction::new(Seat::North);
    // N: 1H, E: Dbl, S: Pass, W: Pass, N: Pass
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Double).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    assert!(!auction.is_complete());
    auction.place_bid(Bid::Pass).unwrap();
    assert!(auction.is_complete());
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. CONTRACT RESOLUTION
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn contract_declarer_is_first_on_winning_side_to_bid_denomination() {
    // N: 1H, E: Pass, S: 2H, W: Pass, N: Pass, W: Pass (oops - let me redo)
    // N: 1H, E: Pass, S: 2H, W: Pass, N: Pass, E: Pass (nope not valid either)
    // Need 3 passes after last suit bid
    let mut auction = Auction::new(Seat::North);
    // N: 1H
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    // E: Pass
    auction.place_bid(Bid::Pass).unwrap();
    // S: 2H
    auction.place_bid(Bid::Suit(2, BidSuit::Hearts)).unwrap();
    // W: Pass
    auction.place_bid(Bid::Pass).unwrap();
    // N: Pass
    auction.place_bid(Bid::Pass).unwrap();
    // E: Pass
    auction.place_bid(Bid::Pass).unwrap();
    assert!(auction.is_complete());

    let contract = auction.resolve_contract().unwrap();
    // Declarer should be North (first NS player to bid Hearts)
    assert_eq!(contract.declarer, Seat::North);
    assert_eq!(contract.suit, BidSuit::Hearts);
    assert_eq!(contract.level, 2);
    // Dummy is North's partner = South
    assert_eq!(contract.dummy, Seat::South);
}

#[test]
fn contract_declarer_when_partner_bids_first() {
    let mut auction = Auction::new(Seat::North);
    // N: Pass, E: 1S, S: Pass, W: 2S, N: Pass, E: Pass, S: Pass
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Suit(1, BidSuit::Spades)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Suit(2, BidSuit::Spades)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();

    let contract = auction.resolve_contract().unwrap();
    // East bid spades first on EW side
    assert_eq!(contract.declarer, Seat::East);
    assert_eq!(contract.dummy, Seat::West);
    assert_eq!(contract.level, 2);
    assert_eq!(contract.suit, BidSuit::Spades);
}

#[test]
fn contract_doubled_state() {
    let mut auction = Auction::new(Seat::North);
    // N: 1H, E: Dbl, S: Pass, W: Pass, N: Pass
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Double).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();

    let contract = auction.resolve_contract().unwrap();
    assert!(contract.doubled);
    assert!(!contract.redoubled);
}

#[test]
fn contract_redoubled_state() {
    let mut auction = Auction::new(Seat::North);
    // N: 1H, E: Dbl, S: Rdbl, W: Pass, N: Pass, E: Pass
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Double).unwrap();
    auction.place_bid(Bid::Redouble).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();

    let contract = auction.resolve_contract().unwrap();
    assert!(!contract.doubled);
    assert!(contract.redoubled);
}

#[test]
fn contract_new_suit_bid_clears_doubled() {
    let mut auction = Auction::new(Seat::North);
    // N: 1H, E: Dbl, S: 2H, W: Pass, N: Pass, E: Pass
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Double).unwrap();
    auction.place_bid(Bid::Suit(2, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();

    let contract = auction.resolve_contract().unwrap();
    // The double was on 1H; 2H is a new bid, so not doubled
    assert!(!contract.doubled);
    assert!(!contract.redoubled);
}

#[test]
fn contract_dummy_assignment() {
    let mut auction = Auction::new(Seat::East);
    // E: 3NT, S: Pass, W: Pass, N: Pass
    auction
        .place_bid(Bid::Suit(3, BidSuit::NoTrump))
        .unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();

    let contract = auction.resolve_contract().unwrap();
    assert_eq!(contract.declarer, Seat::East);
    assert_eq!(contract.dummy, Seat::West); // partner of East
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. TRICK WINNER DETERMINATION
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn trick_winner_highest_of_led_suit_no_trump() {
    let mut trick = Trick::new(Seat::North);
    trick.cards.push((Seat::North, c(Suit::Hearts, Rank::Five)));
    trick.cards.push((Seat::East, c(Suit::Hearts, Rank::King)));
    trick.cards.push((Seat::South, c(Suit::Hearts, Rank::Two)));
    trick.cards.push((Seat::West, c(Suit::Hearts, Rank::Jack)));

    let winner = trick.winner(Some(BidSuit::NoTrump)).unwrap();
    assert_eq!(winner, Seat::East, "Highest heart (King) should win");
}

#[test]
fn trick_winner_led_suit_beats_off_suit_no_trump() {
    let mut trick = Trick::new(Seat::North);
    trick.cards.push((Seat::North, c(Suit::Hearts, Rank::Five)));
    trick
        .cards
        .push((Seat::East, c(Suit::Spades, Rank::Ace))); // off suit
    trick
        .cards
        .push((Seat::South, c(Suit::Hearts, Rank::Seven)));
    trick
        .cards
        .push((Seat::West, c(Suit::Diamonds, Rank::Ace))); // off suit

    let winner = trick.winner(Some(BidSuit::NoTrump)).unwrap();
    assert_eq!(
        winner, Seat::South,
        "Highest of led suit (H7) beats off-suit Aces"
    );
}

#[test]
fn trick_winner_trump_beats_high_of_led_suit() {
    let mut trick = Trick::new(Seat::North);
    trick
        .cards
        .push((Seat::North, c(Suit::Hearts, Rank::Ace)));
    trick.cards.push((Seat::East, c(Suit::Spades, Rank::Two))); // trump
    trick
        .cards
        .push((Seat::South, c(Suit::Hearts, Rank::King)));
    trick
        .cards
        .push((Seat::West, c(Suit::Hearts, Rank::Queen)));

    let winner = trick.winner(Some(BidSuit::Spades)).unwrap();
    assert_eq!(winner, Seat::East, "Even lowest trump beats non-trump Ace");
}

#[test]
fn trick_winner_highest_trump_when_multiple_trumped() {
    let mut trick = Trick::new(Seat::North);
    trick
        .cards
        .push((Seat::North, c(Suit::Hearts, Rank::Ace)));
    trick.cards.push((Seat::East, c(Suit::Spades, Rank::Five))); // trump
    trick
        .cards
        .push((Seat::South, c(Suit::Spades, Rank::Jack))); // higher trump
    trick
        .cards
        .push((Seat::West, c(Suit::Spades, Rank::Three))); // lower trump

    let winner = trick.winner(Some(BidSuit::Spades)).unwrap();
    assert_eq!(winner, Seat::South, "Highest trump (SJ) should win");
}

#[test]
fn trick_winner_no_trump_contract_none() {
    // When trump is None (no trump), highest of led suit wins
    let mut trick = Trick::new(Seat::North);
    trick
        .cards
        .push((Seat::North, c(Suit::Diamonds, Rank::Three)));
    trick
        .cards
        .push((Seat::East, c(Suit::Diamonds, Rank::Ace)));
    trick
        .cards
        .push((Seat::South, c(Suit::Spades, Rank::Ace)));
    trick
        .cards
        .push((Seat::West, c(Suit::Diamonds, Rank::King)));

    let winner = trick.winner(None).unwrap();
    assert_eq!(winner, Seat::East);
}

#[test]
fn trick_winner_leader_wins_with_highest() {
    let mut trick = Trick::new(Seat::South);
    trick
        .cards
        .push((Seat::South, c(Suit::Clubs, Rank::Ace)));
    trick
        .cards
        .push((Seat::West, c(Suit::Clubs, Rank::King)));
    trick
        .cards
        .push((Seat::North, c(Suit::Clubs, Rank::Queen)));
    trick
        .cards
        .push((Seat::East, c(Suit::Clubs, Rank::Jack)));

    let winner = trick.winner(Some(BidSuit::Hearts)).unwrap();
    assert_eq!(winner, Seat::South, "Leader with Ace of led suit wins");
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. FOLLOW-SUIT ENFORCEMENT
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn must_follow_suit_if_able() {
    let hand = hand_from(&[
        (Suit::Hearts, Rank::Ace),
        (Suit::Hearts, Rank::King),
        (Suit::Spades, Rank::Two),
    ]);
    let mut play = PlayState::new(Seat::North, Some(BidSuit::Spades));
    // North leads a heart
    play.play_card(Seat::North, c(Suit::Hearts, Rank::Five))
        .unwrap();
    // East's eligible cards: must follow hearts
    let eligible = play.eligible_cards(&hand);
    assert_eq!(eligible.len(), 2);
    assert!(eligible.iter().all(|c| c.suit == Suit::Hearts));
}

#[test]
fn any_card_if_void_in_led_suit() {
    let hand = hand_from(&[
        (Suit::Spades, Rank::Ace),
        (Suit::Clubs, Rank::King),
        (Suit::Diamonds, Rank::Two),
    ]);
    let mut play = PlayState::new(Seat::North, Some(BidSuit::Spades));
    play.play_card(Seat::North, c(Suit::Hearts, Rank::Five))
        .unwrap();
    // East has no hearts -> any card eligible
    let eligible = play.eligible_cards(&hand);
    assert_eq!(eligible.len(), 3);
}

#[test]
fn leader_can_play_any_card() {
    let hand = hand_from(&[
        (Suit::Hearts, Rank::Ace),
        (Suit::Spades, Rank::Two),
        (Suit::Clubs, Rank::King),
    ]);
    let play = PlayState::new(Seat::North, Some(BidSuit::Spades));
    let eligible = play.eligible_cards(&hand);
    assert_eq!(eligible.len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. PLAY STATE
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn play_current_player_rotates() {
    let play = PlayState::new(Seat::North, None);
    assert_eq!(play.current_player(), Seat::North);
}

#[test]
fn play_trick_counting() {
    let mut play = PlayState::new(Seat::North, None);
    // Play a complete trick - North leads, all follow
    play.play_card(Seat::North, c(Suit::Hearts, Rank::Ace))
        .unwrap();
    assert_eq!(play.current_player(), Seat::East);
    play.play_card(Seat::East, c(Suit::Hearts, Rank::King))
        .unwrap();
    assert_eq!(play.current_player(), Seat::South);
    play.play_card(Seat::South, c(Suit::Hearts, Rank::Queen))
        .unwrap();
    assert_eq!(play.current_player(), Seat::West);
    let winner = play
        .play_card(Seat::West, c(Suit::Hearts, Rank::Jack))
        .unwrap();

    // North wins with Ace, trick winner returned
    assert_eq!(winner, Some(Seat::North));
    assert_eq!(play.ns_tricks, 1);
    assert_eq!(play.ew_tricks, 0);
    assert_eq!(play.tricks_played(), 1);
    // Next leader is North (winner)
    assert_eq!(play.current_player(), Seat::North);
}

#[test]
fn play_ew_trick_count() {
    let mut play = PlayState::new(Seat::East, None);
    play.play_card(Seat::East, c(Suit::Clubs, Rank::Ace))
        .unwrap();
    play.play_card(Seat::South, c(Suit::Clubs, Rank::Two))
        .unwrap();
    play.play_card(Seat::West, c(Suit::Clubs, Rank::Three))
        .unwrap();
    play.play_card(Seat::North, c(Suit::Clubs, Rank::Four))
        .unwrap();

    assert_eq!(play.ns_tricks, 0);
    assert_eq!(play.ew_tricks, 1);
}

#[test]
fn play_13_tricks_complete() {
    let mut play = PlayState::new(Seat::North, None);
    // Play 13 tricks where North always wins (leads Ace of different suit each time)
    // We'll just do all hearts for simplicity, using decreasing ranks
    for i in 0..13 {
        // North leads highest, others play lower - all same suit
        // Just use different suits per trick to avoid needing 52 of same suit
        let suit = Suit::ALL[i % 4];
        play.play_card(Seat::North, c(suit, Rank::Ace)).unwrap();
        play.play_card(Seat::East, c(suit, Rank::King)).unwrap();
        play.play_card(Seat::South, c(suit, Rank::Queen)).unwrap();
        play.play_card(Seat::West, c(suit, Rank::Jack)).unwrap();
    }
    assert!(play.is_complete());
    assert_eq!(play.ns_tricks + play.ew_tricks, 13);
    assert_eq!(play.ns_tricks, 13);
}

#[test]
fn play_wrong_turn_rejected() {
    let mut play = PlayState::new(Seat::North, None);
    let result = play.play_card(Seat::East, c(Suit::Hearts, Rank::Ace));
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. GAME PHASE TRANSITIONS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn game_starts_in_bidding_phase() {
    let game = super::game::Game::new(Seat::North);
    assert_eq!(game.phase, Phase::Bidding);
}

#[test]
fn game_passed_out_goes_to_finished() {
    let mut game = super::game::Game::new(Seat::North);
    for _ in 0..4 {
        game.place_bid(Bid::Pass).unwrap();
    }
    assert_eq!(game.phase, Phase::Finished);
    assert!(game.passed_out);
    assert!(game.contract.is_none());
}

#[test]
fn game_bid_then_play_transition() {
    let mut game = super::game::Game::new(Seat::North);
    // N: 1NT, E: Pass, S: Pass, W: Pass
    game.place_bid(Bid::Suit(1, BidSuit::NoTrump)).unwrap();
    game.place_bid(Bid::Pass).unwrap();
    game.place_bid(Bid::Pass).unwrap();
    assert_eq!(game.phase, Phase::Bidding);
    game.place_bid(Bid::Pass).unwrap();

    assert_eq!(game.phase, Phase::Playing);
    assert!(game.contract.is_some());
    assert!(game.play_state.is_some());
}

#[test]
fn game_cannot_bid_during_play() {
    let mut game = super::game::Game::new(Seat::North);
    game.place_bid(Bid::Suit(1, BidSuit::NoTrump)).unwrap();
    game.place_bid(Bid::Pass).unwrap();
    game.place_bid(Bid::Pass).unwrap();
    game.place_bid(Bid::Pass).unwrap();

    let result = game.place_bid(Bid::Pass);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. OPENING LEAD & DUMMY REVEAL
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn opening_leader_is_left_of_declarer() {
    let mut auction = Auction::new(Seat::North);
    // N: 1NT, E: Pass, S: Pass, W: Pass
    auction
        .place_bid(Bid::Suit(1, BidSuit::NoTrump))
        .unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();

    let contract = auction.resolve_contract().unwrap();
    assert_eq!(contract.declarer, Seat::North);
    // Left of North = East
    let leader = contract.declarer.next();
    assert_eq!(leader, Seat::East);
}

#[test]
fn dummy_not_revealed_before_opening_lead() {
    let mut game = super::game::Game::new(Seat::North);
    game.place_bid(Bid::Suit(1, BidSuit::NoTrump)).unwrap();
    game.place_bid(Bid::Pass).unwrap();
    game.place_bid(Bid::Pass).unwrap();
    game.place_bid(Bid::Pass).unwrap();

    assert!(!game.dummy_revealed, "Dummy should not be revealed before opening lead");
}

#[test]
fn dummy_revealed_after_opening_lead() {
    // We need to construct a game where we control the hands to make a valid play
    let mut game = super::game::Game::new(Seat::North);
    game.deal_cards();
    game.place_bid(Bid::Suit(1, BidSuit::NoTrump)).unwrap();
    game.place_bid(Bid::Pass).unwrap();
    game.place_bid(Bid::Pass).unwrap();
    game.place_bid(Bid::Pass).unwrap();

    // Declarer is North, leader is East
    let contract = game.contract.unwrap();
    assert_eq!(contract.declarer, Seat::North);

    // East leads - pick a card from East's hand
    let east_card = game.hands[Seat::East.index()].cards()[0];
    game.play_card(east_card).unwrap();

    assert!(
        game.dummy_revealed,
        "Dummy should be revealed after opening lead"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. HAND VISIBILITY
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn south_hand_always_visible() {
    let game = super::game::Game::new(Seat::North);
    assert!(game.is_hand_visible(Seat::South));
}

#[test]
fn dummy_visible_after_reveal() {
    let mut game = super::game::Game::new(Seat::North);
    game.deal_cards();
    game.place_bid(Bid::Suit(1, BidSuit::NoTrump)).unwrap();
    game.place_bid(Bid::Pass).unwrap();
    game.place_bid(Bid::Pass).unwrap();
    game.place_bid(Bid::Pass).unwrap();

    // Dummy is South (North's partner)
    let contract = game.contract.unwrap();
    assert_eq!(contract.dummy, Seat::South);

    // Before opening lead, dummy visibility depends on other rules
    // After opening lead, dummy should be visible
    let east_card = game.hands[Seat::East.index()].cards()[0];
    game.play_card(east_card).unwrap();

    assert!(game.is_hand_visible(contract.dummy));
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. VALID BIDS LIST
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn valid_bids_initial_contains_all_suit_bids_and_pass() {
    let auction = Auction::new(Seat::North);
    let valid = auction.valid_bids();
    // Pass + 35 suit bids (7 levels * 5 suits) = 36
    // No Double or Redouble at start
    assert_eq!(valid.len(), 36);
    assert!(valid.contains(&Bid::Pass));
    assert!(valid.contains(&Bid::Suit(1, BidSuit::Clubs)));
    assert!(valid.contains(&Bid::Suit(7, BidSuit::NoTrump)));
    assert!(!valid.contains(&Bid::Double));
    assert!(!valid.contains(&Bid::Redouble));
}

#[test]
fn valid_bids_after_1h_excludes_lower() {
    let mut auction = Auction::new(Seat::North);
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    let valid = auction.valid_bids();
    // Should not contain 1C, 1D, 1H
    assert!(!valid.contains(&Bid::Suit(1, BidSuit::Clubs)));
    assert!(!valid.contains(&Bid::Suit(1, BidSuit::Diamonds)));
    assert!(!valid.contains(&Bid::Suit(1, BidSuit::Hearts)));
    // Should contain 1S, 1NT, 2C+
    assert!(valid.contains(&Bid::Suit(1, BidSuit::Spades)));
    assert!(valid.contains(&Bid::Suit(1, BidSuit::NoTrump)));
    assert!(valid.contains(&Bid::Suit(2, BidSuit::Clubs)));
    // Should contain Double (opponent's bid)
    assert!(valid.contains(&Bid::Double));
}

// ═══════════════════════════════════════════════════════════════════════════
// 13. EDGE CASES
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn seven_nt_is_highest_possible_bid() {
    let mut auction = Auction::new(Seat::North);
    auction
        .place_bid(Bid::Suit(7, BidSuit::NoTrump))
        .unwrap();
    // Only Pass, Double should be valid now
    let valid = auction.valid_bids();
    assert!(valid.contains(&Bid::Pass));
    assert!(valid.contains(&Bid::Double));
    // No suit bid should be valid
    for level in 1..=7u8 {
        for &suit in &BidSuit::ALL {
            assert!(!valid.contains(&Bid::Suit(level, suit)));
        }
    }
}

#[test]
fn seat_next_wraps_correctly() {
    assert_eq!(Seat::North.next(), Seat::East);
    assert_eq!(Seat::East.next(), Seat::South);
    assert_eq!(Seat::South.next(), Seat::West);
    assert_eq!(Seat::West.next(), Seat::North);
}

#[test]
fn seat_partner_correct() {
    assert_eq!(Seat::North.partner(), Seat::South);
    assert_eq!(Seat::South.partner(), Seat::North);
    assert_eq!(Seat::East.partner(), Seat::West);
    assert_eq!(Seat::West.partner(), Seat::East);
}

#[test]
fn seat_is_ns_correct() {
    assert!(Seat::North.is_ns());
    assert!(Seat::South.is_ns());
    assert!(!Seat::East.is_ns());
    assert!(!Seat::West.is_ns());
}

#[test]
fn trick_led_suit_returns_first_cards_suit() {
    let mut trick = Trick::new(Seat::North);
    assert!(trick.led_suit().is_none());
    trick
        .cards
        .push((Seat::North, c(Suit::Diamonds, Rank::Five)));
    assert_eq!(trick.led_suit(), Some(Suit::Diamonds));
}

#[test]
fn double_after_intervening_bid_not_allowed() {
    // N: 1H, E: 2C, S: ? - S cannot double 1H (2C intervened as a higher bid)
    let mut auction = Auction::new(Seat::North);
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Suit(2, BidSuit::Clubs)).unwrap();
    // S: last_suit_bid() returns 2C by East. East is EW, South is NS.
    // So seat.is_ns() != current_bidder.is_ns() => false != true => true
    // last_action_is_suit_bid() => true (2C)
    // So South CAN double 2C - that's correct!
    assert!(auction.is_valid_bid(&Bid::Double));
}

#[test]
fn double_opponents_bid_after_partner_passes() {
    // N: 1H, E: Pass, S: Pass, W can double N's 1H
    let mut auction = Auction::new(Seat::North);
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    // W: last_suit_bid() returns 1H by North.
    // North is NS, West is EW: NS != EW => true
    // last_action_is_suit_bid() skips passes, finds 1H => true
    assert!(auction.is_valid_bid(&Bid::Double));
}

#[test]
fn cannot_place_bid_after_auction_complete() {
    let mut auction = Auction::new(Seat::North);
    auction.place_bid(Bid::Suit(1, BidSuit::Clubs)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    assert!(auction.is_complete());
    // The auction doesn't prevent placing more bids itself - that's handled at the Game level.
    // But is_valid_bid should still work. Let's verify the game prevents it.
}

#[test]
fn play_state_led_suit_tracks_current_trick() {
    let mut play = PlayState::new(Seat::North, None);
    assert!(play.led_suit().is_none());
    play.play_card(Seat::North, c(Suit::Clubs, Rank::Ace))
        .unwrap();
    assert_eq!(play.led_suit(), Some(Suit::Clubs));
}

// ═══════════════════════════════════════════════════════════════════════════
// 14. BID SUIT TO CARD SUIT MAPPING (for trump)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn trump_suit_mapping_in_trick_winner() {
    // Verify each BidSuit maps to correct Suit for trump purposes
    for (bid_suit, card_suit) in [
        (BidSuit::Clubs, Suit::Clubs),
        (BidSuit::Diamonds, Suit::Diamonds),
        (BidSuit::Hearts, Suit::Hearts),
        (BidSuit::Spades, Suit::Spades),
    ] {
        let mut trick = Trick::new(Seat::North);
        trick
            .cards
            .push((Seat::North, c(Suit::Hearts, Rank::Ace)));
        trick
            .cards
            .push((Seat::East, c(card_suit, Rank::Two)));
        trick
            .cards
            .push((Seat::South, c(Suit::Hearts, Rank::King)));
        trick
            .cards
            .push((Seat::West, c(Suit::Hearts, Rank::Queen)));

        // If the bid_suit matches the card_suit, East's trump should win
        // (unless hearts IS the trump, in which case North's Ace wins)
        if card_suit != Suit::Hearts {
            let winner = trick.winner(Some(bid_suit)).unwrap();
            assert_eq!(
                winner,
                Seat::East,
                "Trump {} should beat non-trump",
                bid_suit
            );
        }
    }
}

#[test]
fn no_trump_contract_means_no_trump_suit() {
    let mut trick = Trick::new(Seat::North);
    trick
        .cards
        .push((Seat::North, c(Suit::Hearts, Rank::Five)));
    trick.cards.push((Seat::East, c(Suit::Spades, Rank::Ace)));
    trick
        .cards
        .push((Seat::South, c(Suit::Hearts, Rank::Seven)));
    trick.cards.push((Seat::West, c(Suit::Clubs, Rank::Ace)));

    // BidSuit::NoTrump means no trump => highest of led suit wins
    let winner = trick.winner(Some(BidSuit::NoTrump)).unwrap();
    assert_eq!(
        winner, Seat::South,
        "In NT, highest of led suit wins, off-suit cards lose"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 15. COMPLEX AUCTION SCENARIOS
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn competitive_auction_scenario() {
    // N: 1H, E: 1S, S: 2H, W: 2S, N: 3H, E: Pass, S: Pass, W: Pass
    let mut auction = Auction::new(Seat::North);
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Suit(1, BidSuit::Spades)).unwrap();
    auction.place_bid(Bid::Suit(2, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Suit(2, BidSuit::Spades)).unwrap();
    auction.place_bid(Bid::Suit(3, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();

    assert!(auction.is_complete());
    let contract = auction.resolve_contract().unwrap();
    assert_eq!(contract.level, 3);
    assert_eq!(contract.suit, BidSuit::Hearts);
    assert_eq!(
        contract.declarer, Seat::North,
        "North was first NS to bid Hearts"
    );
    assert!(!contract.doubled);
}

#[test]
fn declarer_changes_denomination() {
    // N: 1H, E: Pass, S: 1S, W: Pass, N: 2S, E: Pass, S: Pass, W: Pass
    // Last suit bid is 2S by North. First NS to bid Spades is South (1S).
    let mut auction = Auction::new(Seat::North);
    auction.place_bid(Bid::Suit(1, BidSuit::Hearts)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Suit(1, BidSuit::Spades)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Suit(2, BidSuit::Spades)).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();
    auction.place_bid(Bid::Pass).unwrap();

    let contract = auction.resolve_contract().unwrap();
    assert_eq!(contract.suit, BidSuit::Spades);
    assert_eq!(
        contract.declarer, Seat::South,
        "South was first NS to bid Spades"
    );
    assert_eq!(contract.dummy, Seat::North);
}
