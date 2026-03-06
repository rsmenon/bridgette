use serde::{Deserialize, Serialize};

use super::card::{Card, Rank, Suit};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hand {
    cards: Vec<Card>,
}

impl Hand {
    pub fn new(mut cards: Vec<Card>) -> Self {
        cards.sort();
        Self { cards }
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn remove_card(&mut self, card: Card) -> bool {
        if let Some(pos) = self.cards.iter().position(|c| *c == card) {
            self.cards.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn cards_of_suit(&self, suit: Suit) -> Vec<Card> {
        self.cards.iter().filter(|c| c.suit == suit).copied().collect()
    }

    #[allow(dead_code)]
    pub fn has_suit(&self, suit: Suit) -> bool {
        self.cards.iter().any(|c| c.suit == suit)
    }

    #[allow(dead_code)]
    pub fn contains(&self, card: &Card) -> bool {
        self.cards.contains(card)
    }

    /// Calculate high card points (A=4, K=3, Q=2, J=1).
    pub fn hcp(&self) -> u32 {
        self.cards
            .iter()
            .map(|c| match c.rank {
                Rank::Ace => 4,
                Rank::King => 3,
                Rank::Queen => 2,
                Rank::Jack => 1,
                _ => 0,
            })
            .sum()
    }

    /// Calculate distribution points (void=3, singleton=2, doubleton=1).
    pub fn dist_points(&self) -> u32 {
        Suit::ALL
            .iter()
            .map(|&s| match self.cards_of_suit(s).len() {
                0 => 3,
                1 => 2,
                2 => 1,
                _ => 0,
            })
            .sum()
    }
}
