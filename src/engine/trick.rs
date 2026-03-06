use serde::{Deserialize, Serialize};

use crate::types::Seat;

use super::bidding::BidSuit;
use super::card::{Card, Suit};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trick {
    pub cards: Vec<(Seat, Card)>,
    pub leader: Seat,
}

impl Trick {
    pub fn new(leader: Seat) -> Self {
        Self {
            cards: Vec::with_capacity(4),
            leader,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.cards.len() == 4
    }

    pub fn led_suit(&self) -> Option<Suit> {
        self.cards.first().map(|(_, c)| c.suit)
    }

    pub fn winner(&self, trump: Option<BidSuit>) -> Option<Seat> {
        if self.cards.is_empty() {
            return None;
        }

        let trump_suit = trump.and_then(|bs| match bs {
            BidSuit::Clubs => Some(Suit::Clubs),
            BidSuit::Diamonds => Some(Suit::Diamonds),
            BidSuit::Hearts => Some(Suit::Hearts),
            BidSuit::Spades => Some(Suit::Spades),
            BidSuit::NoTrump => None,
        });

        let led = self.led_suit().unwrap();
        let mut best_seat = self.cards[0].0;
        let mut best_card = self.cards[0].1;
        let mut best_is_trump = trump_suit == Some(best_card.suit);

        for &(seat, card) in &self.cards[1..] {
            let is_trump = trump_suit == Some(card.suit);
            let beats = if is_trump && !best_is_trump {
                true
            } else if !is_trump && best_is_trump {
                false
            } else if is_trump && best_is_trump {
                card.rank > best_card.rank
            } else {
                // Both non-trump: only led suit matters
                card.suit == led && (best_card.suit != led || card.rank > best_card.rank)
            };
            if beats {
                best_seat = seat;
                best_card = card;
                best_is_trump = is_trump;
            }
        }
        Some(best_seat)
    }
}
