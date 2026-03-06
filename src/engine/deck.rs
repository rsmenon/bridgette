use rand::seq::SliceRandom;
use rand::thread_rng;

use super::card::{Card, Rank, Suit};
use super::hand::Hand;

pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    pub fn new() -> Self {
        let mut cards = Vec::with_capacity(52);
        for &suit in &Suit::ALL {
            for &rank in &Rank::ALL {
                cards.push(Card::new(suit, rank));
            }
        }
        Self { cards }
    }

    pub fn shuffle(&mut self) {
        self.cards.shuffle(&mut thread_rng());
    }

    /// Deal into 4 hands of 13 cards each (North, East, South, West order).
    pub fn deal(&self) -> [Hand; 4] {
        let mut hands: [Vec<Card>; 4] = [vec![], vec![], vec![], vec![]];
        for (i, card) in self.cards.iter().enumerate() {
            hands[i % 4].push(*card);
        }
        [
            Hand::new(hands[0].clone()),
            Hand::new(hands[1].clone()),
            Hand::new(hands[2].clone()),
            Hand::new(hands[3].clone()),
        ]
    }
}
