use crate::engine::bidding::{Bid, BidSuit};
use crate::engine::card::{Card, Rank, Suit};
use crate::engine::contract::Contract;
use crate::engine::hand::Hand;
use crate::types::{Seat, Vulnerability};

/// Owned snapshot of the game state visible to a particular seat.
#[derive(Debug, Clone)]
pub struct AgentGameView {
    pub seat: Seat,
    pub hand: Hand,
    pub dummy_hand: Option<Hand>,
    pub dealer: Seat,
    pub vulnerability: Vulnerability,
    pub bidding_history: Vec<(Seat, Bid)>,
    pub valid_bids: Vec<Bid>,
    pub contract: Option<Contract>,
    pub current_trick_cards: Vec<(Seat, Card)>,
    pub completed_tricks: Vec<(Seat, Vec<(Seat, Card)>)>, // (winner, cards)
    pub ns_tricks: usize,
    pub ew_tricks: usize,
    pub eligible_cards: Vec<Card>,
    pub playing_from_dummy: bool,
}

pub fn card_ascii(card: &Card) -> String {
    let suit = match card.suit {
        Suit::Spades => "S",
        Suit::Hearts => "H",
        Suit::Diamonds => "D",
        Suit::Clubs => "C",
    };
    let rank = card.rank.short();
    format!("{}{}", rank, suit)
}

pub fn bid_ascii(bid: &Bid) -> String {
    match bid {
        Bid::Pass => "Pass".to_string(),
        Bid::Double => "Double".to_string(),
        Bid::Redouble => "Redouble".to_string(),
        Bid::Suit(level, suit) => {
            let s = match suit {
                BidSuit::Clubs => "C",
                BidSuit::Diamonds => "D",
                BidSuit::Hearts => "H",
                BidSuit::Spades => "S",
                BidSuit::NoTrump => "NT",
            };
            format!("{}{}", level, s)
        }
    }
}

pub fn hand_ascii(hand: &Hand) -> String {
    let mut parts = Vec::new();
    for &suit in Suit::ALL.iter().rev() {
        let suit_char = match suit {
            Suit::Spades => "S",
            Suit::Hearts => "H",
            Suit::Diamonds => "D",
            Suit::Clubs => "C",
        };
        let cards = hand.cards_of_suit(suit);
        let ranks: Vec<&str> = cards.iter().rev().map(|c| c.rank.short()).collect();
        if ranks.is_empty() {
            parts.push(format!("{}: -", suit_char));
        } else {
            parts.push(format!("{}: {}", suit_char, ranks.join(" ")));
        }
    }
    parts.join("\n")
}


pub fn build_system_prompt(seat: Seat, contract: Option<&Contract>) -> String {
    let role = match contract {
        Some(c) if c.declarer == seat => "declarer",
        Some(c) if c.dummy == seat => "dummy",
        Some(_) => "defender",
        None => "bidder",
    };

    let reference = if contract.is_some() {
        format!("{SAYC_REFERENCE}\n\n{CARD_PLAY_REFERENCE}")
    } else {
        SAYC_REFERENCE.to_string()
    };

    format!(
        "You are an expert bridge player sitting {seat}. You are the {role}.\n\
         You MUST strictly follow Standard American Yellow Card (SAYC) conventions.\n\
         Think hard and consider strategies to win the game before responding.\n\
         Respond with ONLY your action — a bid (e.g., 1S, 2NT, Pass, Double, Redouble) or a card (e.g., AS, TH, 4D).\n\
         Do not explain your reasoning. Just output the action.\n\
         \n\
         {reference}"
    )
}

pub const SAYC_REFERENCE: &str = "\
=== SAYC BIDDING CONVENTIONS (follow strictly) ===

OPENING BIDS:
- 1C: 12-21 HCP, 3+ clubs (bid 1C with 3-3 minors)
- 1D: 12-21 HCP, 4+ diamonds (bid 1D with 4-4 minors)
- 1H: 12-21 HCP, 5+ hearts
- 1S: 12-21 HCP, 5+ spades
- 1NT: 15-17 HCP, balanced hand (no 5-card major)
- 2C: 22+ HCP or 9+ playing tricks; artificial, strong, forcing
- 2D/2H/2S: 5-11 HCP, good 6-card suit (weak two)
- 2NT: 20-21 HCP, balanced
- 3-level: 5-10 HCP, 7-card suit (preemptive)
- 3NT: 25-27 HCP, balanced
- With 5-5+ two suits, open the higher-ranking. With 4-4 minors, open 1D. With 3-3 minors, open 1C.
- Rule of 20 for borderline: HCP + lengths of two longest suits >= 20.

RESPONSES TO 1NT (15-17):
- Pass: 0-7 HCP, no long suit
- 2C = STAYMAN: 8+ HCP, asks for 4-card major. Opener bids 2D (no major), 2H, or 2S.
- 2D = JACOBY TRANSFER to hearts: 5+ hearts, any strength. Opener MUST bid 2H.
- 2H = JACOBY TRANSFER to spades: 5+ spades, any strength. Opener MUST bid 2S.
- 2S: forces 3C; responder passes (clubs) or corrects to 3D (diamonds) — weak hand
- 2NT: 8-9 HCP, invitational
- 3NT: 10-15 HCP, no 4-card major, game values
- 4C = GERBER: ace-asking (4D=0/4, 4H=1, 4S=2, 4NT=3)
- 4NT: quantitative slam invite (NOT Blackwood over NT)

RESPONSES TO 1H/1S OPENING:
- New suit at 1-level: 6+ HCP, 4+ cards, forcing one round
- 1NT: 6-10 HCP, no fit, no biddable suit at 1-level
- Single raise (2H/2S): 6-10 support points, 3+ card support
- 2NT = JACOBY 2NT: 13+ support points, 4+ card support, game forcing
- Limit raise (3H/3S): 10-12 support points, 3+ card support, invitational
- New suit at 2-level: 10+ HCP, 4+ cards, forcing one round
- Jump shift: 18-19+ HCP, strong suit, slam interest
- Game raise (4H/4S): weak with 5+ trumps + distribution, or strong

RESPONSES TO 1C/1D OPENING:
- Same structure as majors, but always bid a 4-card major at 1-level before raising a minor
- Need 5 cards to raise 1C, 4 cards to raise 1D
- 2NT: 13-15 HCP, game forcing; 3NT: 16-17 HCP

STRONG 2C OPENING:
- 2D response: waiting/artificial (any hand without a positive response)
- 2H/2S/3C/3D: natural positive, 5+ cards, 8+ HCP
- 2NT: 8+ HCP, balanced
- Auction is game-forcing unless opener rebids 2NT (22-24)

WEAK TWO BIDS (2D/2H/2S):
- 2NT response: forcing inquiry; opener shows feature (outside A or guarded K) or rebids suit (minimum)
- Raise: preemptive/to play
- New suit: one-round force, 5+ cards

SLAM CONVENTIONS:
- BLACKWOOD (4NT): asks for aces. 5C=0/4, 5D=1, 5H=2, 5S=3. Then 5NT asks kings.
- GERBER (4C over NT opening/rebid): asks for aces. 4D=0/4, 4H=1, 4S=2, 4NT=3.

COMPETITIVE BIDDING:
- Simple overcall: 8-16 HCP, 5+ card suit. Do NOT overcall with only 4 cards in the suit.
- 1NT overcall: 15-18 HCP, balanced, stopper in opener's suit
- Takeout double: support for unbid suits, 12+ HCP (or any 18+)
- Negative double (by responder after overcall): 7+ HCP, 4+ in unbid major(s)
- Michaels cuebid: 5-5+ two-suiter. Over minor: both majors. Over major: other major + a minor.
- Unusual 2NT: 5-5+ in two lowest unbid suits

OPENER'S REBIDS (after partner responds):
- After partner's 1NT response (6-10): pass with 12-14 balanced; raise to 2NT with 18-19; bid new suit or rebid own suit
- After single raise (2H/2S): pass with minimum (12-14); bid game with 18-19; invite with 15-17 (bid 3 of suit)
- After limit raise (3H/3S): pass with minimum; bid game with 14+
- After Jacoby 2NT (game forcing): show shortness (singleton/void) by bidding that suit at 3-level; bid 3NT with 18+ balanced; rebid trump suit at 4-level with minimum
- After new suit at 1-level: raise with 4-card support; rebid own suit with 6+ cards; bid new suit; bid 1NT (12-14 balanced); jump rebid own suit (15-17, 6+ cards); jump to 2NT (18-19 balanced)
- After new suit at 2-level (10+ HCP): same priorities; forcing one round; must not pass
- After Stayman (2C over 1NT): bid 2D (no 4-card major), 2H (4+ hearts), 2S (4+ spades, no hearts)
- After Jacoby Transfer (2D/2H over 1NT): MUST complete the transfer (bid 2H/2S respectively)

RESPONDING TO PARTNER'S OVERCALL:
- Single raise: 8-10 support points, 3+ card support. Do NOT raise with fewer than 3 cards.
- Jump raise: 11-12 support points, 3+ card support, invitational
- New suit: 10+ HCP, 5+ cards, forcing one round
- 1NT response: 8-12 HCP, stopper in opener's suit
- Cuebid of opener's suit: 12+ HCP, 3+ card support, game-forcing raise
- Pass: default with no fit and no suit to bid. Do not stretch to raise with a singleton or doubleton.";

pub const CARD_PLAY_REFERENCE: &str = "\
=== CARD PLAY PRINCIPLES ===

These are strong defaults. Violate them only with a clear reason (entry management,
endplay, deception, etc.). When in doubt, follow the principle.

FUNDAMENTAL RULES:
- Do not waste winners: do not discard an ace or established winner when you have losers to discard.
  Exception: overtaking partner's winner to gain the lead for an important continuation.
- When your side has already won the current trick, play your lowest card.
  Exception: overtake to gain the lead when you have a critical continuation.
- When trumping (ruffing), use a trump HIGH ENOUGH to prevent an overtrump. If an opponent
  might overtrump, ruff with a high trump — not your lowest.
- Second hand low, third hand high (general guideline for defenders).
  Exceptions: split honors in second seat, or third hand finesse when appropriate.
- Fourth hand (last to play): win the trick with the cheapest card that wins. You can see
  all cards played — if you can beat what's on the table, do so. Do not duck.
- Cover an honor with an honor when it can promote a card in your hand or partner's.

DECLARER PLAY:
- Count your winners and losers before playing to trick 1.
- Draw trumps early unless you need to ruff in the short hand first or need dummy entries.
- When playing from dummy, win with the CHEAPEST card that wins the trick.
- Win tricks you can win unless you have a specific plan (hold-up, duck to maintain
  communication, endplay). Do not duck without a reason.
- Play aces to capture opposing high cards, not on tricks with only low cards.
- Manage entries between your hand and dummy. Plan the order of play to preserve them.

DEFENDER PLAY:
- Lead partner's bid suit unless you have a clearly better alternative.
- Return partner's led suit when you gain the lead, unless you have a clearly better plan.
- Lead top of a sequence (KQJ, QJT, etc.).
- Lead 4th best from your longest suit against NT contracts.
- Against trump contracts, consider leading a singleton for a ruff.
- Signal honestly: high card = encouraging, low card = discouraging.

DISCARDING:
- Do not discard winners (aces, established cards, guarded kings) unless necessary for
  an endplay or to create an entry. Discard from your weakest suit first.
- Keep length parity with dummy: if dummy has 3 cards in a suit, try to keep at least 3.
- Protect your high cards: do not bare a king or unguard a queen without reason.

DUMMY PLAY (declarer playing from dummy):
- Win with the cheapest winning card from dummy.
- If your hand already wins the trick, play LOW from dummy unless overtaking for entry.
- Use dummy's entries carefully — do not strand winners in dummy.";

pub fn build_bidding_prompt(view: &AgentGameView) -> String {
    let mut lines = Vec::new();
    lines.push(format!("You are {} (dealer: {})", view.seat, view.dealer));
    let vul_str = match view.vulnerability {
        Vulnerability::None => "None vulnerable".to_string(),
        Vulnerability::Both => "Both vulnerable".to_string(),
        vul => {
            let who = if vul.is_vulnerable(view.seat) { "You are" } else { "Opponents are" };
            format!("{} vulnerable ({})", who, vul)
        }
    };
    lines.push(format!("Vulnerability: {}", vul_str));
    lines.push(String::new());
    lines.push("Your hand:".to_string());
    lines.push(hand_ascii(&view.hand));
    lines.push(format!("HCP: {}", view.hand.hcp()));
    lines.push(String::new());

    if view.bidding_history.is_empty() {
        lines.push("No bids yet.".to_string());
    } else {
        lines.push("Bidding so far:".to_string());
        for (seat, bid) in &view.bidding_history {
            lines.push(format!("  {}: {}", seat, bid_ascii(bid)));
        }
    }

    // Add situational SAYC reminders based on auction context
    let reminder = build_situational_reminder(view);
    if !reminder.is_empty() {
        lines.push(String::new());
        lines.push(reminder);
    }

    lines.push(String::new());
    let valid_strs: Vec<String> = view.valid_bids.iter().map(bid_ascii).collect();
    lines.push(format!("Valid bids: {}", valid_strs.join(", ")));
    lines.push(String::new());
    lines.push("Your bid:".to_string());

    lines.join("\n")
}

/// Build a situational reminder about applicable SAYC conventions given the
/// current auction state. This nudges the LLM toward correct conventional bids
/// rather than relying on its general bridge knowledge.
fn build_situational_reminder(view: &AgentGameView) -> String {
    let partner = view.seat.partner();
    let partner_bid = last_suit_bid_by(&view.bidding_history, partner);
    let my_bid = last_suit_bid_by(&view.bidding_history, view.seat);

    // Partner opened 1NT — remind about Stayman, Jacoby, Gerber
    // But suppress if an opponent bid between partner's 1NT and our turn (systems off)
    let opponent_interfered_over_1nt = if partner_bid == Some((1, BidSuit::NoTrump)) {
        // Find partner's 1NT bid index, then check if any opponent bid a suit after it
        let partner_1nt_idx = view.bidding_history.iter().position(|(s, b)| {
            *s == partner && *b == Bid::Suit(1, BidSuit::NoTrump)
        });
        match partner_1nt_idx {
            Some(idx) => view.bidding_history[idx + 1..].iter().any(|(s, b)| {
                *s != view.seat && *s != partner && !matches!(b, Bid::Pass)
            }),
            None => false,
        }
    } else {
        false
    };

    if partner_bid == Some((1, BidSuit::NoTrump)) && my_bid.is_none() && !opponent_interfered_over_1nt {
        let h_count = view.hand.cards_of_suit(Suit::Hearts).len();
        let s_count = view.hand.cards_of_suit(Suit::Spades).len();
        let hcp_val = view.hand.hcp();
        let mut hints = vec![
            "REMINDER — Partner opened 1NT (15-17 HCP). Apply these SAYC conventions:".to_string(),
        ];
        if hcp_val >= 8 && (h_count >= 4 || s_count >= 4) && h_count < 5 && s_count < 5 {
            hints.push("- You have 8+ HCP and a 4-card major: bid 2C (Stayman) to find a major fit.".to_string());
        }
        if h_count >= 5 {
            hints.push("- You have 5+ hearts: bid 2D (Jacoby Transfer). Partner MUST respond 2H.".to_string());
        }
        if s_count >= 5 {
            hints.push("- You have 5+ spades: bid 2H (Jacoby Transfer). Partner MUST respond 2S.".to_string());
        }
        if hcp_val <= 7 && h_count < 5 && s_count < 5 {
            hints.push("- With 0-7 HCP and no 5-card major: Pass.".to_string());
        }
        if (10..=15).contains(&hcp_val) && h_count < 4 && s_count < 4 {
            hints.push("- With 10-15 HCP and no 4-card major: bid 3NT directly.".to_string());
        }
        return hints.join("\n");
    }

    // I opened 1NT and partner bid 2C (Stayman) — I must respond correctly
    if my_bid == Some((1, BidSuit::NoTrump))
        && last_bid_is(&view.bidding_history, partner, &Bid::Suit(2, BidSuit::Clubs))
    {
        let h_count = view.hand.cards_of_suit(Suit::Hearts).len();
        let s_count = view.hand.cards_of_suit(Suit::Spades).len();
        let mut hints = vec![
            "REMINDER — Partner bid 2C (Stayman) asking for a 4-card major. You MUST respond:".to_string(),
        ];
        if h_count >= 4 && s_count >= 4 {
            hints.push("- You have 4+ hearts AND 4+ spades: bid 2H (bid hearts first with both).".to_string());
        } else if h_count >= 4 {
            hints.push("- You have 4+ hearts: bid 2H.".to_string());
        } else if s_count >= 4 {
            hints.push("- You have 4+ spades: bid 2S.".to_string());
        } else {
            hints.push("- You have no 4-card major: bid 2D (denies a 4-card major).".to_string());
        }
        return hints.join("\n");
    }

    // I opened 1NT and partner bid 2D (Jacoby Transfer to hearts)
    if my_bid == Some((1, BidSuit::NoTrump))
        && last_bid_is(&view.bidding_history, partner, &Bid::Suit(2, BidSuit::Diamonds))
    {
        return "REMINDER — Partner bid 2D (Jacoby Transfer). You MUST bid 2H to complete the transfer.".to_string();
    }

    // I opened 1NT and partner bid 2H (Jacoby Transfer to spades)
    if my_bid == Some((1, BidSuit::NoTrump))
        && last_bid_is(&view.bidding_history, partner, &Bid::Suit(2, BidSuit::Hearts))
    {
        return "REMINDER — Partner bid 2H (Jacoby Transfer). You MUST bid 2S to complete the transfer.".to_string();
    }

    // Partner opened 1H or 1S — remind about responses including Jacoby 2NT
    if let Some((1, suit)) = partner_bid {
        if (suit == BidSuit::Hearts || suit == BidSuit::Spades) && my_bid.is_none() {
            let support_suit = if suit == BidSuit::Hearts { Suit::Hearts } else { Suit::Spades };
            let support_count = view.hand.cards_of_suit(support_suit).len();
            let hcp_val = view.hand.hcp();
            let suit_name = if suit == BidSuit::Hearts { "hearts" } else { "spades" };
            let mut hints = vec![
                format!("REMINDER — Partner opened 1{}. Apply SAYC responses:", if suit == BidSuit::Hearts { "H" } else { "S" }),
            ];
            if support_count >= 4 && hcp_val >= 13 {
                hints.push(format!("- You have 4+ {} and 13+ support points: bid 2NT (Jacoby 2NT, game forcing).", suit_name));
            }
            if support_count >= 3 && (6..=10).contains(&hcp_val) {
                hints.push(format!("- You have 3+ {} and 6-10 points: make a single raise.", suit_name));
            }
            if support_count >= 3 && (10..=12).contains(&hcp_val) {
                hints.push(format!("- You have 3+ {} and 10-12 points: make a limit raise (jump raise).", suit_name));
            }
            return hints.join("\n");
        }
    }

    // Partner opened 2C (strong) — remind about responses
    if partner_bid == Some((2, BidSuit::Clubs)) && my_bid.is_none() {
        let hcp_val = view.hand.hcp();
        let mut hints = vec![
            "REMINDER — Partner opened 2C (strong, artificial, forcing). Respond:".to_string(),
        ];
        if hcp_val < 8 {
            hints.push("- With 0-7 HCP: bid 2D (waiting, artificial).".to_string());
        } else {
            hints.push("- With 8+ HCP and a good 5+ card suit: bid that suit as a natural positive.".to_string());
            hints.push("- With 8+ HCP and balanced: bid 2NT.".to_string());
        }
        return hints.join("\n");
    }

    // Blackwood — partner bid 4NT (not over NT opening)
    if last_bid_is(&view.bidding_history, partner, &Bid::Suit(4, BidSuit::NoTrump))
        && my_bid != Some((1, BidSuit::NoTrump))
        && my_bid != Some((2, BidSuit::NoTrump))
    {
        let aces = count_aces(&view.hand);
        let response = match aces {
            0 | 4 => "5C",
            1 => "5D",
            2 => "5H",
            3 => "5S",
            _ => "5C",
        };
        return format!(
            "REMINDER — Partner bid 4NT (Blackwood), asking for aces. You have {} ace(s). Respond {}.",
            aces, response
        );
    }

    String::new()
}

/// Find the last suit bid (not Pass/Double/Redouble) made by a given seat.
fn last_suit_bid_by(history: &[(Seat, Bid)], seat: Seat) -> Option<(u8, BidSuit)> {
    history
        .iter()
        .rev()
        .find_map(|(s, b)| {
            if *s == seat {
                if let Bid::Suit(level, suit) = b {
                    return Some((*level, *suit));
                }
            }
            None
        })
}

/// Check if the last bid by a given seat is exactly the specified bid.
fn last_bid_is(history: &[(Seat, Bid)], seat: Seat, target: &Bid) -> bool {
    history
        .iter()
        .rev()
        .find(|(s, _)| *s == seat)
        .map(|(_, b)| b == target)
        .unwrap_or(false)
}

fn count_aces(hand: &Hand) -> u32 {
    hand.cards().iter().filter(|c| c.rank == Rank::Ace).count() as u32
}

pub fn build_play_prompt(view: &AgentGameView) -> String {
    let mut lines = Vec::new();

    if let Some(contract) = &view.contract {
        let suit_str = match contract.suit {
            BidSuit::Clubs => "Clubs",
            BidSuit::Diamonds => "Diamonds",
            BidSuit::Hearts => "Hearts",
            BidSuit::Spades => "Spades",
            BidSuit::NoTrump => "No Trump",
        };
        let doubled = if contract.redoubled {
            " Redoubled"
        } else if contract.doubled {
            " Doubled"
        } else {
            ""
        };
        lines.push(format!(
            "Contract: {}{} {} by {}",
            contract.level, doubled, suit_str, contract.declarer
        ));
        let vul_str = if view.vulnerability.is_vulnerable(contract.declarer) {
            "Declarer vulnerable"
        } else {
            "Declarer not vulnerable"
        };
        lines.push(format!("Vulnerability: {}", vul_str));
    }

    // Bidding sequence
    if !view.bidding_history.is_empty() {
        lines.push(String::new());
        lines.push("Bidding:".to_string());
        for (seat, bid) in &view.bidding_history {
            lines.push(format!("  {}: {}", seat.short(), bid_ascii(bid)));
        }
    }

    if view.playing_from_dummy {
        lines.push(String::new());
        lines.push("It is DUMMY's turn. You must play a card from dummy's hand.".to_string());
        lines.push(String::new());
        lines.push("Dummy's hand (play from here):".to_string());
        lines.push(hand_ascii(&view.hand));

        if let Some(own) = &view.dummy_hand {
            lines.push(String::new());
            lines.push("Your hand:".to_string());
            lines.push(hand_ascii(own));
        }
    } else {
        lines.push(String::new());
        lines.push("Your hand:".to_string());
        lines.push(hand_ascii(&view.hand));

        if let Some(dummy) = &view.dummy_hand {
            lines.push(String::new());
            lines.push("Dummy's hand:".to_string());
            lines.push(hand_ascii(dummy));
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "Tricks — N/S: {}, E/W: {}",
        view.ns_tricks, view.ew_tricks
    ));

    // Add "tricks needed" context — the most fundamental strategic framing
    if let Some(contract) = &view.contract {
        let declarer_is_ns = contract.declarer.is_ns();
        let agent_is_declarer_side = view.seat.is_ns() == declarer_is_ns;
        let declarer_tricks = if declarer_is_ns { view.ns_tricks } else { view.ew_tricks };
        let defender_tricks = if declarer_is_ns { view.ew_tricks } else { view.ns_tricks };
        let target = contract.level as usize + 6; // tricks declarer needs total
        if agent_is_declarer_side {
            let still_need = target.saturating_sub(declarer_tricks);
            if still_need > 0 {
                lines.push(format!("You need {} more trick(s) to make the contract.", still_need));
            } else {
                lines.push("Contract is already made. Play for overtricks.".to_string());
            }
        } else {
            let defeat_target = 13 - target + 1; // tricks defenders need total to set
            let still_need = defeat_target.saturating_sub(defender_tricks);
            if still_need > 0 {
                lines.push(format!("You need {} more trick(s) to defeat the contract.", still_need));
            } else {
                lines.push("Contract is already defeated.".to_string());
            }
        }
    }

    if !view.completed_tricks.is_empty() {
        lines.push(String::new());
        lines.push("Completed tricks:".to_string());
        for (i, (winner, cards)) in view.completed_tricks.iter().enumerate() {
            let card_strs: Vec<String> = cards
                .iter()
                .map(|(s, c)| format!("{}:{}", s.short(), card_ascii(c)))
                .collect();
            lines.push(format!(
                "  Trick {}: {} → won by {}",
                i + 1,
                card_strs.join(", "),
                winner
            ));
        }
    }

    lines.push(String::new());
    if view.current_trick_cards.is_empty() {
        lines.push("You are leading this trick.".to_string());
    } else {
        lines.push("Current trick:".to_string());
        for (seat, card) in &view.current_trick_cards {
            lines.push(format!("  {}: {}", seat, card_ascii(card)));
        }
    }

    // Add situational play reminder
    let play_reminder = build_play_reminder(view);
    if !play_reminder.is_empty() {
        lines.push(String::new());
        lines.push(play_reminder);
    }

    lines.push(String::new());
    let eligible_strs: Vec<String> = view.eligible_cards.iter().map(card_ascii).collect();
    lines.push(format!("Eligible cards: {}", eligible_strs.join(", ")));
    lines.push(String::new());
    lines.push("Your play:".to_string());

    lines.join("\n")
}

/// Build a situational reminder for card play based on the current trick state.
/// Detects scenarios where the LLM commonly makes mistakes and injects targeted nudges.
fn build_play_reminder(view: &AgentGameView) -> String {
    if view.current_trick_cards.is_empty() {
        return String::new(); // Leading — no reminder needed
    }

    let contract = match &view.contract {
        Some(c) => c,
        None => return String::new(),
    };

    let trump_suit: Option<Suit> = match contract.suit {
        BidSuit::Clubs => Some(Suit::Clubs),
        BidSuit::Diamonds => Some(Suit::Diamonds),
        BidSuit::Hearts => Some(Suit::Hearts),
        BidSuit::Spades => Some(Suit::Spades),
        BidSuit::NoTrump => None,
    };

    let led_suit = view.current_trick_cards[0].1.suit;

    let (winning_seat, winning_card) = current_trick_winner(
        &view.current_trick_cards, trump_suit,
    );

    let cards_played = view.current_trick_cards.len();
    let is_last = cards_played == 3;
    let winner_is_partner = is_partner(view.seat, winning_seat);

    // Check if we're void in the led suit
    let void_in_led = !view.eligible_cards.iter().any(|c| c.suit == led_suit);

    // Cards that can beat the current winner
    let can_win: Vec<&Card> = view.eligible_cards.iter()
        .filter(|c| beats_card(c, &winning_card, led_suit, trump_suit))
        .collect();

    let mut hints: Vec<String> = Vec::new();

    if is_last {
        // --- 4th seat logic ---
        if winner_is_partner {
            hints.push(format!(
                "REMINDER — You play last. Partner ({}) is winning with {}. Consider playing low to save your winners for later.",
                winning_seat, card_ascii(&winning_card),
            ));
        } else if !can_win.is_empty() {
            let cheapest = can_win.iter()
                .min_by_key(|c| card_strength(c, led_suit, trump_suit))
                .unwrap();
            hints.push(format!(
                "REMINDER — You play last. {} is winning with {}. Consider beating it with {} — the cheapest card that wins.",
                winning_seat, card_ascii(&winning_card), card_ascii(cheapest),
            ));
        } else {
            hints.push(format!(
                "REMINDER — You play last. {} is winning with {} and you cannot beat it. Consider playing your lowest card to minimize losses.",
                winning_seat, card_ascii(&winning_card),
            ));
        }
    } else if void_in_led {
        // --- 2nd/3rd seat, void in led suit ---
        if let Some(ts) = trump_suit {
            let has_trump = view.eligible_cards.iter().any(|c| c.suit == ts);
            let current_winner_trumped = winning_card.suit == ts && winning_card.suit != led_suit;

            if winner_is_partner {
                hints.push(format!(
                    "REMINDER — Partner ({}) is currently winning with {}. You are void in the led suit. Consider discarding a low card rather than trumping partner's winner.",
                    winning_seat, card_ascii(&winning_card),
                ));
            } else if has_trump && current_winner_trumped {
                let over_trumps: Vec<&Card> = view.eligible_cards.iter()
                    .filter(|c| c.suit == ts && c.rank > winning_card.rank)
                    .collect();
                if !over_trumps.is_empty() {
                    let cheapest = over_trumps.iter().min_by_key(|c| c.rank).unwrap();
                    hints.push(format!(
                        "REMINDER — {} trumped with {}. Consider over-trumping with {} to win this trick.",
                        winning_seat, card_ascii(&winning_card), card_ascii(cheapest),
                    ));
                }
            } else if has_trump && !current_winner_trumped {
                hints.push("REMINDER — You are void in the led suit. Consider trumping to win this trick.".to_string());
            }
        }
    } else {
        // --- 2nd/3rd seat, following suit ---
        if winner_is_partner {
            hints.push(format!(
                "REMINDER — Partner ({}) is currently winning with {}. Consider playing low to save your winners for later.",
                winning_seat, card_ascii(&winning_card),
            ));
        } else if cards_played == 1 {
            // 2nd seat: "second hand low" applies to defenders, not declarer
            let is_defender = contract.declarer != view.seat && contract.dummy != view.seat;
            if is_defender {
                hints.push(format!(
                    "REMINDER — You play second. {} led {}. Second hand low is a strong default — consider saving your high cards unless you have a specific reason to play high.",
                    winning_seat, card_ascii(&winning_card),
                ));
            }
        } else if cards_played == 2 && !can_win.is_empty() {
            // 3rd seat: third hand high
            let cheapest = can_win.iter()
                .min_by_key(|c| card_strength(c, led_suit, trump_suit))
                .unwrap();
            hints.push(format!(
                "REMINDER — You play third. {} is currently winning with {}. Third hand high — consider playing {} to try to win this trick for your side.",
                winning_seat, card_ascii(&winning_card), card_ascii(cheapest),
            ));
        }
    }

    hints.join("\n")
}

/// Determine who is currently winning the trick.
fn current_trick_winner(
    cards: &[(Seat, Card)],
    trump_suit: Option<Suit>,
) -> (Seat, Card) {
    let led_suit = cards[0].1.suit;
    let mut best_seat = cards[0].0;
    let mut best_card = cards[0].1;

    for &(seat, card) in &cards[1..] {
        if card_beats_in_trick(&card, &best_card, led_suit, trump_suit) {
            best_seat = seat;
            best_card = card;
        }
    }

    (best_seat, best_card)
}

/// Does `new_card` beat `current_best` in the context of a trick?
fn card_beats_in_trick(new: &Card, best: &Card, led_suit: Suit, trump_suit: Option<Suit>) -> bool {
    if let Some(ts) = trump_suit {
        match (new.suit == ts, best.suit == ts) {
            (true, false) => return true,   // New is trump, best isn't
            (false, true) => return false,  // Best is trump, new isn't
            (true, true) => return new.rank > best.rank, // Both trump
            (false, false) => {}            // Neither trump — fall through
        }
    }
    // Led suit comparison: on-suit beats off-suit, higher rank beats lower
    if new.suit == led_suit && best.suit != led_suit {
        true
    } else if new.suit == led_suit && best.suit == led_suit {
        new.rank > best.rank
    } else {
        false
    }
}

/// Does `candidate` beat `current_winner` in this trick context?
/// Unlike `card_beats_in_trick`, this also handles the case where the winner
/// is off-suit (e.g., an off-suit discard that's nominally "winning" because
/// no one followed suit — any on-suit card or trump beats it).
fn beats_card(candidate: &Card, winner: &Card, led_suit: Suit, trump_suit: Option<Suit>) -> bool {
    if let Some(ts) = trump_suit {
        match (candidate.suit == ts, winner.suit == ts) {
            (true, false) => return true,
            (false, true) => return false,
            (true, true) => return candidate.rank > winner.rank,
            (false, false) => {}
        }
    }
    // Candidate on led suit beats anything not on led suit (and not trump)
    if candidate.suit == led_suit && winner.suit != led_suit {
        return true;
    }
    candidate.suit == led_suit && winner.suit == led_suit && candidate.rank > winner.rank
}

/// Numeric strength of a card for comparison (higher = stronger).
fn card_strength(card: &Card, led_suit: Suit, trump_suit: Option<Suit>) -> u8 {
    let rank_val = card.rank as u8;
    if Some(card.suit) == trump_suit {
        100 + rank_val // Trumps are strongest
    } else if card.suit == led_suit {
        50 + rank_val // Led suit next
    } else {
        rank_val // Off-suit weakest
    }
}

fn is_partner(seat: Seat, other: Seat) -> bool {
    seat.partner() == other
}
