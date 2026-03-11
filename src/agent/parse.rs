use crate::engine::bidding::{Bid, BidSuit};
use crate::engine::card::{Card, Rank, Suit};

fn parse_rank(ch: char) -> Option<Rank> {
    match ch.to_ascii_uppercase() {
        'A' => Some(Rank::Ace),
        'K' => Some(Rank::King),
        'Q' => Some(Rank::Queen),
        'J' => Some(Rank::Jack),
        'T' | '0' => Some(Rank::Ten),
        '9' => Some(Rank::Nine),
        '8' => Some(Rank::Eight),
        '7' => Some(Rank::Seven),
        '6' => Some(Rank::Six),
        '5' => Some(Rank::Five),
        '4' => Some(Rank::Four),
        '3' => Some(Rank::Three),
        '2' => Some(Rank::Two),
        _ => None,
    }
}

fn parse_suit(ch: char) -> Option<Suit> {
    match ch.to_ascii_uppercase() {
        'S' => Some(Suit::Spades),
        'H' => Some(Suit::Hearts),
        'D' => Some(Suit::Diamonds),
        'C' => Some(Suit::Clubs),
        _ => None,
    }
}

fn parse_bid_suit(ch: char) -> Option<BidSuit> {
    match ch.to_ascii_uppercase() {
        'S' => Some(BidSuit::Spades),
        'H' => Some(BidSuit::Hearts),
        'D' => Some(BidSuit::Diamonds),
        'C' => Some(BidSuit::Clubs),
        'N' => Some(BidSuit::NoTrump),
        _ => None,
    }
}

/// Parse a bid from LLM response text. Tries to find a valid bid in the text.
pub fn parse_bid(text: &str, valid_bids: &[Bid]) -> Result<Bid, String> {
    let text = text.trim();
    let upper = text.to_uppercase();

    // Check for pass
    if upper.contains("PASS") && valid_bids.contains(&Bid::Pass) {
        return Ok(Bid::Pass);
    }

    // Check for redouble (before double to avoid false match)
    if (upper.contains("REDOUBLE") || upper.contains("RDBL") || upper.contains("XX"))
        && valid_bids.contains(&Bid::Redouble)
    {
        return Ok(Bid::Redouble);
    }

    // Check for double
    if (upper.contains("DOUBLE") || upper.contains("DBL"))
        && valid_bids.contains(&Bid::Double)
    {
        return Ok(Bid::Double);
    }

    // Try to find suit bid patterns like "1S", "2NT", "3H", "1 Spade", etc.
    let chars: Vec<char> = upper.chars().collect();
    for i in 0..chars.len() {
        let ch = chars[i];
        if ch.is_ascii_digit() && ch != '0' {
            let level = ch as u8 - b'0';
            if !(1..=7).contains(&level) {
                continue;
            }

            // Check for NT right after
            if i + 2 < chars.len() && chars[i + 1] == 'N' && chars[i + 2] == 'T' {
                let bid = Bid::Suit(level, BidSuit::NoTrump);
                if valid_bids.contains(&bid) {
                    return Ok(bid);
                }
            }

            // Check for suit letter right after
            if i + 1 < chars.len() {
                if let Some(suit) = parse_bid_suit(chars[i + 1]) {
                    let bid = Bid::Suit(level, suit);
                    if valid_bids.contains(&bid) {
                        return Ok(bid);
                    }
                }
            }

            // Check for spelled-out suit names
            let rest = &upper[i + 1..];
            let rest = rest.trim_start();
            let suit = if rest.starts_with("NO TRUMP") || rest.starts_with("NOTRUMP") || rest.starts_with("NO-TRUMP") {
                Some(BidSuit::NoTrump)
            } else if rest.starts_with("SPADE") {
                Some(BidSuit::Spades)
            } else if rest.starts_with("HEART") {
                Some(BidSuit::Hearts)
            } else if rest.starts_with("DIAMOND") {
                Some(BidSuit::Diamonds)
            } else if rest.starts_with("CLUB") {
                Some(BidSuit::Clubs)
            } else {
                None
            };

            if let Some(suit) = suit {
                let bid = Bid::Suit(level, suit);
                if valid_bids.contains(&bid) {
                    return Ok(bid);
                }
            }
        }
    }

    Err(format!("Could not parse bid from: {}", text))
}

/// Parse a card from LLM response text. Tries formats like "SA", "AS", "Ace of Spades", etc.
pub fn parse_card(text: &str, eligible_cards: &[Card]) -> Result<Card, String> {
    let text = text.trim();
    let upper = text.to_uppercase();

    // Try to find card patterns
    let chars: Vec<char> = upper.chars().collect();

    // Try rank-suit format (AS, TH, 4D, etc.) — canonical format
    for i in 0..chars.len().saturating_sub(1) {
        if let Some(rank) = parse_rank(chars[i]) {
            if let Some(suit) = parse_suit(chars[i + 1]) {
                let card = Card::new(suit, rank);
                if eligible_cards.contains(&card) {
                    return Ok(card);
                }
            }
        }
    }

    // Try suit-rank format (SA, HT, etc.) — fallback for LLM variation
    for i in 0..chars.len().saturating_sub(1) {
        if let Some(suit) = parse_suit(chars[i]) {
            if let Some(rank) = parse_rank(chars[i + 1]) {
                let card = Card::new(suit, rank);
                if eligible_cards.contains(&card) {
                    return Ok(card);
                }
            }
        }
    }

    // Try spelled-out patterns: "Ace of Spades", "King of Hearts", etc.
    let rank_names = [
        ("ACE", Rank::Ace),
        ("KING", Rank::King),
        ("QUEEN", Rank::Queen),
        ("JACK", Rank::Jack),
        ("TEN", Rank::Ten),
        ("NINE", Rank::Nine),
        ("EIGHT", Rank::Eight),
        ("SEVEN", Rank::Seven),
        ("SIX", Rank::Six),
        ("FIVE", Rank::Five),
        ("FOUR", Rank::Four),
        ("THREE", Rank::Three),
        ("TWO", Rank::Two),
        ("DEUCE", Rank::Two),
    ];
    let suit_names = [
        ("SPADE", Suit::Spades),
        ("HEART", Suit::Hearts),
        ("DIAMOND", Suit::Diamonds),
        ("CLUB", Suit::Clubs),
    ];

    for &(rn, rank) in &rank_names {
        if upper.contains(rn) {
            for &(sn, suit) in &suit_names {
                if upper.contains(sn) {
                    let card = Card::new(suit, rank);
                    if eligible_cards.contains(&card) {
                        return Ok(card);
                    }
                }
            }
        }
    }

    Err(format!("Could not parse card from: {}", text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bid_pass() {
        let valid = vec![Bid::Pass, Bid::Suit(1, BidSuit::Clubs)];
        assert_eq!(parse_bid("Pass", &valid).unwrap(), Bid::Pass);
        assert_eq!(parse_bid("I'll pass", &valid).unwrap(), Bid::Pass);
    }

    #[test]
    fn test_parse_bid_suit() {
        let valid = vec![Bid::Pass, Bid::Suit(1, BidSuit::Spades), Bid::Suit(2, BidSuit::NoTrump)];
        assert_eq!(parse_bid("1S", &valid).unwrap(), Bid::Suit(1, BidSuit::Spades));
        assert_eq!(parse_bid("2NT", &valid).unwrap(), Bid::Suit(2, BidSuit::NoTrump));
        assert_eq!(parse_bid("I bid 1S", &valid).unwrap(), Bid::Suit(1, BidSuit::Spades));
    }

    #[test]
    fn test_parse_card_suit_rank() {
        let eligible = vec![
            Card::new(Suit::Spades, Rank::Ace),
            Card::new(Suit::Hearts, Rank::Ten),
        ];
        assert_eq!(parse_card("SA", &eligible).unwrap(), Card::new(Suit::Spades, Rank::Ace));
        assert_eq!(parse_card("HT", &eligible).unwrap(), Card::new(Suit::Hearts, Rank::Ten));
    }

    #[test]
    fn test_parse_card_rank_suit() {
        let eligible = vec![Card::new(Suit::Spades, Rank::Ace)];
        assert_eq!(parse_card("AS", &eligible).unwrap(), Card::new(Suit::Spades, Rank::Ace));
    }

    #[test]
    fn test_parse_card_spelled_out() {
        let eligible = vec![Card::new(Suit::Spades, Rank::Ace)];
        assert_eq!(
            parse_card("Ace of Spades", &eligible).unwrap(),
            Card::new(Suit::Spades, Rank::Ace)
        );
    }

}
