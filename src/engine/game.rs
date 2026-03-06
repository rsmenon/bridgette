use serde::{Deserialize, Serialize};

use crate::agent::prompt::AgentGameView;
use crate::types::{Phase, Seat};

use super::bidding::{Auction, Bid, BidSuit};
use super::card::Card;
use super::contract::Contract;
use super::deck::Deck;
use super::hand::Hand;
use super::play::PlayState;
use super::scoring::{self, Score};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub hands: [Hand; 4],
    pub dealt_hands: [Hand; 4],
    pub auction: Auction,
    pub play_state: Option<PlayState>,
    pub contract: Option<Contract>,
    pub phase: Phase,
    pub dealer: Seat,
    pub dummy_revealed: bool,
    pub passed_out: bool,
    pub score: Option<Score>,
}

impl Game {
    pub fn new(dealer: Seat) -> Self {
        let empty: [Hand; 4] = std::array::from_fn(|_| Hand::new(vec![]));
        Self {
            hands: empty.clone(),
            dealt_hands: empty,
            auction: Auction::new(dealer),
            play_state: None,
            contract: None,
            phase: Phase::Bidding,
            dealer,
            dummy_revealed: false,
            passed_out: false,
            score: None,
        }
    }

    /// Create a game with pre-determined hands (for review/replay).
    pub fn from_hands(dealer: Seat, hands: [Hand; 4]) -> Self {
        let mut game = Self::new(dealer);
        game.dealt_hands = hands.clone();
        game.hands = hands;
        game
    }

    /// Shuffle and deal cards to all four hands.
    pub fn deal_cards(&mut self) {
        let mut deck = Deck::new();
        deck.shuffle();
        let hands = deck.deal();
        self.dealt_hands = hands.clone();
        self.hands = hands;
    }

    pub fn current_seat(&self) -> Seat {
        match self.phase {
            Phase::Bidding => self.auction.current_bidder(),
            Phase::Playing => self
                .play_state
                .as_ref()
                .expect("play_state must exist during Playing phase")
                .current_player(),
            Phase::Finished => self.dealer, // doesn't matter
        }
    }

    pub fn place_bid(&mut self, bid: Bid) -> anyhow::Result<()> {
        if self.phase != Phase::Bidding {
            anyhow::bail!("Not in bidding phase");
        }
        self.auction.place_bid(bid)?;

        if self.auction.is_complete() {
            if let Some(contract) = self.auction.resolve_contract() {
                self.contract = Some(contract);
                let leader = contract.declarer.next(); // left of declarer leads
                let trump = if contract.suit == BidSuit::NoTrump {
                    None
                } else {
                    Some(contract.suit)
                };
                self.play_state = Some(PlayState::new(leader, trump));
                self.phase = Phase::Playing;
            } else {
                // Passed out
                self.passed_out = true;
                self.score = Some(Score::PassedOut);
                self.phase = Phase::Finished;
            }
        }
        Ok(())
    }

    pub fn play_card(&mut self, card: Card) -> anyhow::Result<()> {
        if self.phase != Phase::Playing {
            anyhow::bail!("Not in playing phase");
        }

        let play = self
            .play_state
            .as_mut()
            .expect("play_state must exist during Playing phase");
        let seat = play.current_player();
        let hand = &self.hands[seat.index()];

        // Validate card is eligible
        let eligible = play.eligible_cards(hand);
        if !eligible.contains(&card) {
            anyhow::bail!("Card {} is not eligible to play", card);
        }

        self.hands[seat.index()].remove_card(card);
        let play = self
            .play_state
            .as_mut()
            .expect("play_state must exist during Playing phase");
        play.play_card(seat, card)?;

        // Reveal dummy after opening lead
        if !self.dummy_revealed
            && play.tricks_played() == 0
            && play.current_trick.cards.len() == 1
        {
            self.dummy_revealed = true;
        }

        if play.is_complete() {
            let contract = self
                .contract
                .as_ref()
                .expect("contract must exist during Playing phase");
            let declarer_tricks = if contract.declarer.is_ns() {
                play.ns_tricks
            } else {
                play.ew_tricks
            };
            self.score = Some(scoring::calculate_score(contract, declarer_tricks as u8));
            self.phase = Phase::Finished;
        }

        Ok(())
    }

    pub fn hand(&self, seat: Seat) -> &Hand {
        &self.hands[seat.index()]
    }

    pub fn is_hand_visible(&self, seat: Seat) -> bool {
        // South is always visible (home player)
        if seat == Seat::South {
            return true;
        }
        // When N/S declares, human controls both hands — North is always visible
        if self.phase == Phase::Playing {
            if let Some(contract) = &self.contract {
                if contract.declarer.is_ns() && seat == Seat::North {
                    return true;
                }
            }
        }
        // Dummy is visible after reveal
        if self.dummy_revealed {
            if let Some(contract) = &self.contract {
                if seat == contract.dummy {
                    return true;
                }
            }
        }
        // All hands visible when finished
        if self.phase == Phase::Finished {
            return true;
        }
        false
    }

    pub fn eligible_cards(&self) -> Vec<Card> {
        if self.phase != Phase::Playing {
            return vec![];
        }
        let play = self
            .play_state
            .as_ref()
            .expect("play_state must exist during Playing phase");
        let seat = play.current_player();
        play.eligible_cards(&self.hands[seat.index()])
    }

    /// Build an owned snapshot of the game visible to the given seat.
    pub fn agent_view(&self, seat: Seat) -> AgentGameView {
        let play = self.play_state.as_ref();
        let actual_seat = if let Some(contract) = &self.contract {
            // If it's dummy's turn, the declarer plays for dummy
            if let Some(p) = play {
                if p.current_player() == contract.dummy && contract.declarer == seat {
                    contract.dummy
                } else {
                    seat
                }
            } else {
                seat
            }
        } else {
            seat
        };

        let playing_from_dummy = actual_seat != seat;
        let hand = self.hands[actual_seat.index()].clone();

        let dummy_hand = if let Some(contract) = &self.contract {
            if self.dummy_revealed {
                if playing_from_dummy {
                    // Declarer is playing from dummy — show declarer's own hand as "dummy_hand"
                    Some(self.hands[seat.index()].clone())
                } else {
                    Some(self.hands[contract.dummy.index()].clone())
                }
            } else {
                None
            }
        } else {
            None
        };

        let eligible_cards = if self.phase == Phase::Playing {
            if let Some(p) = play {
                p.eligible_cards(&self.hands[p.current_player().index()])
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let trump_suit = self.contract.as_ref().and_then(|c| {
            if c.suit == BidSuit::NoTrump {
                None
            } else {
                Some(c.suit)
            }
        });

        let current_trick_cards = play
            .map(|p| p.current_trick.cards.clone())
            .unwrap_or_default();

        let completed_tricks = play
            .map(|p| {
                p.tricks
                    .iter()
                    .map(|t| {
                        let winner = t.winner(trump_suit)
                            .expect("completed trick must have a winner");
                        (winner, t.cards.clone())
                    })
                    .collect()
            })
            .unwrap_or_default();

        let (ns_tricks, ew_tricks) = play
            .map(|p| (p.ns_tricks, p.ew_tricks))
            .unwrap_or((0, 0));

        AgentGameView {
            seat,
            hand,
            dummy_hand,
            dealer: self.dealer,
            bidding_history: self.auction.bids.clone(),
            valid_bids: if self.phase == Phase::Bidding {
                self.auction.valid_bids()
            } else {
                vec![]
            },
            contract: self.contract,
            current_trick_cards,
            completed_tricks,
            ns_tricks,
            ew_tricks,
            eligible_cards,
            playing_from_dummy,
        }
    }
}
