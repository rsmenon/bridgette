use serde::{Deserialize, Serialize};

use crate::types::Seat;

use super::bidding::BidSuit;
use super::card::{Card, Suit};
use super::hand::Hand;
use super::trick::Trick;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayState {
    pub tricks: Vec<Trick>,
    pub current_trick: Trick,
    pub ns_tricks: usize,
    pub ew_tricks: usize,
    trump: Option<BidSuit>,
}

impl PlayState {
    pub fn new(leader: Seat, trump: Option<BidSuit>) -> Self {
        Self {
            tricks: Vec::new(),
            current_trick: Trick::new(leader),
            ns_tricks: 0,
            ew_tricks: 0,
            trump,
        }
    }

    pub fn current_player(&self) -> Seat {
        let leader = self.current_trick.leader;
        let played = self.current_trick.cards.len();
        let mut seat = leader;
        for _ in 0..played {
            seat = seat.next();
        }
        seat
    }

    pub fn eligible_cards(&self, hand: &Hand) -> Vec<Card> {
        if let Some(led_suit) = self.current_trick.led_suit() {
            let suited = hand.cards_of_suit(led_suit);
            if suited.is_empty() {
                hand.cards().to_vec()
            } else {
                suited
            }
        } else {
            // Leading: any card is eligible
            hand.cards().to_vec()
        }
    }

    pub fn play_card(&mut self, seat: Seat, card: Card) -> anyhow::Result<Option<Seat>> {
        if self.current_player() != seat {
            anyhow::bail!("Not {}'s turn", seat);
        }

        self.current_trick.cards.push((seat, card));

        if self.current_trick.is_complete() {
            let winner = self.current_trick.winner(self.trump)
                .expect("complete trick must have a winner");
            if winner.is_ns() {
                self.ns_tricks += 1;
            } else {
                self.ew_tricks += 1;
            }
            self.tricks.push(self.current_trick.clone());
            self.current_trick = Trick::new(winner);
            Ok(Some(winner))
        } else {
            Ok(None)
        }
    }

    pub fn is_complete(&self) -> bool {
        self.tricks.len() == 13
    }

    #[allow(dead_code)]
    pub fn led_suit(&self) -> Option<Suit> {
        self.current_trick.led_suit()
    }

    pub fn tricks_played(&self) -> usize {
        self.tricks.len()
    }
}
