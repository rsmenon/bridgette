use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding, Paragraph};
use ratatui::Frame;

use crate::config::{HandsRecord, SavedGame, TrickCardRecord};
use crate::engine::bidding::Bid;
use crate::engine::card::{Card, Suit};
use crate::engine::game::Game;
use crate::engine::hand::Hand;
use crate::types::{Phase, Seat};

use super::bidding_panel::render_bidding_history;
use super::board::{card_on_table_spans, render_hand_score};
use super::palette::*;
use super::tutor::render_response_with_hint;
use super::TutorState;

/// State for the review mode.
pub struct ReviewState {
    pub saved: SavedGame,
    pub step: usize,
    pub total_steps: usize,
    pub game: Game,
    pub tutor: TutorState,
}

impl ReviewState {
    pub fn new(saved: SavedGame) -> Self {
        let total_steps = compute_total_steps(&saved);
        let game = reconstruct_game_at_step(&saved, 0);
        Self {
            saved,
            step: 0,
            total_steps,
            game,
            tutor: TutorState::new(),
        }
    }

    pub fn step_forward(&mut self) {
        if self.total_steps > 0 && self.step < self.total_steps - 1 {
            self.step += 1;
            self.game = reconstruct_game_at_step(&self.saved, self.step);
            self.tutor = TutorState::new();
        }
    }

    pub fn step_backward(&mut self) {
        if self.step > 0 {
            self.step -= 1;
            self.game = reconstruct_game_at_step(&self.saved, self.step);
            self.tutor = TutorState::new();
        }
    }
}

fn compute_total_steps(saved: &SavedGame) -> usize {
    let card_count: usize = saved.play.iter().map(|t| t.cards.len()).sum();
    saved.bidding.len() + card_count
}

/// Find the trick card record at a given play-phase card index.
fn card_at_play_index(saved: &SavedGame, card_index: usize) -> Option<&TrickCardRecord> {
    let mut idx = 0;
    for trick in &saved.play {
        for tc in &trick.cards {
            if idx == card_index {
                return Some(tc);
            }
            idx += 1;
        }
    }
    None
}

fn parse_hands_record(hr: &HandsRecord) -> [Hand; 4] {
    let parse_hand = |cards: &[String]| -> Hand {
        let parsed: Vec<Card> = cards.iter().filter_map(|s| Card::from_ascii(s)).collect();
        Hand::new(parsed)
    };
    [
        parse_hand(&hr.north),
        parse_hand(&hr.east),
        parse_hand(&hr.south),
        parse_hand(&hr.west),
    ]
}

/// Reconstruct a Game at a given step index.
/// Step 0..B-1 = bids, B..B+P-1 = plays.
fn reconstruct_game_at_step(saved: &SavedGame, step: usize) -> Game {
    let dealer = saved.dealer.parse::<Seat>().unwrap_or(Seat::North);
    let hands = parse_hands_record(&saved.hands);
    let vulnerability = saved.vulnerability();
    let mut game = Game::from_hands(dealer, hands, vulnerability);

    let num_bids = saved.bidding.len();

    // Replay bids (step 0 = first bid, step B-1 = last bid)
    let bids_to_replay = (step + 1).min(num_bids);
    for i in 0..bids_to_replay {
        if let Some(bid) = Bid::from_ascii(&saved.bidding[i].bid) {
            let _ = game.place_bid(bid);
        }
    }

    if step < num_bids {
        return game;
    }

    // Replay cards
    let cards_to_replay = step - num_bids + 1;
    let mut cards_replayed = 0;
    for trick in &saved.play {
        for tc in &trick.cards {
            if cards_replayed >= cards_to_replay {
                return game;
            }
            if let Some(card) = Card::from_ascii(&tc.card) {
                let _ = game.play_card(card);
            }
            cards_replayed += 1;
        }
    }

    game
}

/// Get the game state just before the action at `step`, plus the acting seat.
/// This gives the tutor the correct POV for analyzing whether the action was right.
pub fn pre_step_game_and_seat(saved: &SavedGame, step: usize) -> (Game, Seat) {
    let game = if step > 0 {
        reconstruct_game_at_step(saved, step - 1)
    } else {
        let dealer = saved.dealer.parse::<Seat>().unwrap_or(Seat::North);
        let hands = parse_hands_record(&saved.hands);
        let vulnerability = saved.vulnerability();
        Game::from_hands(dealer, hands, vulnerability)
    };
    let seat = game.current_seat();
    (game, seat)
}

/// Build a review question for the tutor based on the current step.
pub fn build_review_question(saved: &SavedGame, step: usize) -> Option<String> {
    let num_bids = saved.bidding.len();

    if step < num_bids {
        let bid_rec = &saved.bidding[step];
        return Some(format!(
            "{} bid {}. Was this a reasonable bid given the auction so far? \
             What alternatives should they have considered? Be concise (2-3 sentences).",
            bid_rec.seat, bid_rec.bid
        ));
    }

    // Play phase
    card_at_play_index(saved, step - num_bids).map(|tc| {
        format!(
            "{} played {}. Was this the best card to play? \
             What alternatives were there? Be concise (2-3 sentences).",
            tc.seat, tc.card
        )
    })
}

/// Render the full review panel (compact board + tutor analysis).
pub fn render_review_panel(f: &mut Frame, area: Rect, review: &ReviewState) {
    let sections = Layout::vertical([
        Constraint::Ratio(3, 5), // Compact board
        Constraint::Ratio(2, 5), // Tutor analysis
    ])
    .split(area);

    render_compact_board(f, sections[0], review);
    render_review_tutor(f, sections[1], review);
}

fn render_compact_board(f: &mut Frame, area: Rect, review: &ReviewState) {
    // Paint background
    let bg = Block::default().style(Style::default().bg(BG_TABLE));
    f.render_widget(bg, area);

    let rows = Layout::vertical([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(area);

    let top_cols = Layout::horizontal([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(rows[0]);

    let mid_cols = Layout::horizontal([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(rows[1]);

    let bot_cols = Layout::horizontal([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(rows[2]);

    let game = &review.game;

    // Top-left: Auction history
    render_review_auction(f, top_cols[0], game);

    // Top-center: North's hand
    render_review_hand(f, top_cols[1], &game.dealt_hands[Seat::North.index()], Seat::North, game);

    // Top-right: Step indicator
    render_step_indicator(f, top_cols[2], review);

    // Mid-left: West's hand
    render_review_hand(f, mid_cols[0], &game.dealt_hands[Seat::West.index()], Seat::West, game);

    // Mid-center: Current trick / bridge title
    render_review_center(f, mid_cols[1], game);

    // Mid-right: East's hand
    render_review_hand(f, mid_cols[2], &game.dealt_hands[Seat::East.index()], Seat::East, game);

    // Bot-left: Contract info
    render_review_info(f, bot_cols[0], game);

    // Bot-center: South's hand
    render_review_hand(f, bot_cols[1], &game.dealt_hands[Seat::South.index()], Seat::South, game);

    // Bot-right: empty
    let empty = Block::default().style(Style::default().bg(BG_FRAME));
    f.render_widget(empty, bot_cols[2]);
}

fn render_review_hand(f: &mut Frame, area: Rect, dealt_hand: &Hand, seat: Seat, game: &Game) {
    // During play, show the *current* hand (cards removed as played)
    let hand = if game.phase == Phase::Playing || game.phase == Phase::Finished {
        &game.hands[seat.index()]
    } else {
        dealt_hand
    };

    let show_score = game.phase == Phase::Bidding;

    let bg = BG_CONTENT;
    let block = Block::default()
        .padding(Padding::new(1, 0, 0, 0))
        .style(Style::default().bg(bg));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    // Seat label
    let title = Paragraph::new(Span::styled(
        format!("{}", seat),
        Style::default()
            .fg(TEXT_DARK)
            .add_modifier(Modifier::BOLD),
    ));
    let title_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    f.render_widget(title, title_area);

    // Cards + optional score
    let content_area = Rect {
        x: inner.x,
        y: inner.y + 1,
        width: inner.width,
        height: inner.height.saturating_sub(1),
    };

    let bottom_height: u16 = if show_score { 2 } else { 0 };
    let sections = Layout::vertical([
        Constraint::Min(4),
        Constraint::Length(bottom_height),
    ])
    .split(content_area);

    let lines = review_hand_lines(hand);
    let para = Paragraph::new(lines);
    f.render_widget(para, sections[0]);

    if show_score {
        render_hand_score(f, sections[1], dealt_hand);
    }
}

/// Render hand cards grouped by suit, no selection highlighting.
fn review_hand_lines(hand: &Hand) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for &suit in Suit::ALL.iter().rev() {
        let cards = hand.cards_of_suit(suit);
        let suit_color = if suit.is_red() { SUIT_RED } else { SUIT_BLACK };

        let mut spans = vec![Span::styled(
            format!("{}  ", suit.symbol()),
            Style::default().fg(suit_color).add_modifier(Modifier::BOLD),
        )];

        if cards.is_empty() {
            spans.push(Span::styled(
                "—",
                Style::default()
                    .fg(TEXT_DARK_DISABLED)
                    .add_modifier(Modifier::DIM),
            ));
        } else {
            for card in cards.iter().rev() {
                spans.push(Span::styled(
                    format!("{:<3}", card.rank.short()),
                    Style::default().fg(TEXT_DARK),
                ));
            }
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn render_review_auction(f: &mut Frame, area: Rect, game: &Game) {
    let block = Block::default()
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(BG_FRAME));
    let inner = block.inner(area);
    f.render_widget(block, area);

    render_bidding_history(f, inner, &game.auction);
}

fn render_step_indicator(f: &mut Frame, area: Rect, review: &ReviewState) {
    let block = Block::default()
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(BG_FRAME));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let num_bids = review.saved.bidding.len();
    let phase_label = if review.step < num_bids {
        "Bidding"
    } else {
        "Play"
    };

    let mut lines = vec![
        Line::from(Span::styled(
            format!("Step {}/{}", review.step + 1, review.total_steps),
            Style::default()
                .fg(TEXT_LIGHT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            phase_label,
            Style::default().fg(TEXT_LIGHT_MUTED),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "← →  Step",
            Style::default().fg(TEXT_LIGHT_MUTED),
        )),
    ];

    // Show what action was just taken
    if review.step < num_bids {
        let bid_rec = &review.saved.bidding[review.step];
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{}: {}", bid_rec.seat, bid_rec.bid),
            Style::default().fg(ACCENT_TEAL),
        )));
    } else if let Some(tc) = card_at_play_index(&review.saved, review.step - num_bids) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{}: {}", tc.seat, tc.card),
            Style::default().fg(ACCENT_TEAL),
        )));
    }

    // Show contract if in play phase
    if let Some(ref contract) = review.game.contract {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{}", contract),
            Style::default().fg(TEXT_LIGHT),
        )));
    }

    let v_center = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(lines.len() as u16),
        Constraint::Min(0),
    ])
    .split(inner);

    let para = Paragraph::new(lines);
    f.render_widget(para, v_center[1]);
}

fn render_review_center(f: &mut Frame, area: Rect, game: &Game) {
    let inner = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    if game.phase != Phase::Playing {
        return;
    }

    let label_style = Style::default()
        .fg(TEXT_LIGHT)
        .bg(BG_TABLE)
        .add_modifier(Modifier::BOLD);
    let label_dim = Style::default()
        .fg(TEXT_LIGHT)
        .bg(BG_TABLE)
        .add_modifier(Modifier::DIM);

    let play = game.play_state.as_ref();
    let seat_cards = if let Some(play) = play {
        // In review, when the 4th card completes a trick, current_trick is
        // already empty (engine moves it to tricks vec). Show the last
        // completed trick so the 4th card is visible.
        let cards = if play.current_trick.cards.is_empty() {
            play.tricks.last().map(|t| &t.cards[..]).unwrap_or(&[])
        } else {
            &play.current_trick.cards[..]
        };
        let mut map: [Option<&Card>; 4] = [None; 4];
        for (seat, card) in cards {
            map[seat.index()] = Some(card);
        }
        map
    } else {
        [None; 4]
    };

    let center_v = Layout::vertical([
        Constraint::Length(1), // N label
        Constraint::Length(1), // N card
        Constraint::Min(0),
        Constraint::Length(1), // W / E row
        Constraint::Min(0),
        Constraint::Length(1), // S card
        Constraint::Length(1), // S label
    ])
    .split(inner);

    let n_style = if seat_cards[Seat::North.index()].is_some() {
        label_style
    } else {
        label_dim
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("N", n_style))).alignment(Alignment::Center),
        center_v[0],
    );
    if let Some(card) = seat_cards[Seat::North.index()] {
        f.render_widget(
            Paragraph::new(Line::from(card_on_table_spans(card))).alignment(Alignment::Center),
            center_v[1],
        );
    }

    let w = center_v[3];
    let w_style = if seat_cards[Seat::West.index()].is_some() {
        label_style
    } else {
        label_dim
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("W", w_style))),
        Rect {
            x: w.x,
            y: w.y,
            width: 1,
            height: 1,
        },
    );
    if let Some(card) = seat_cards[Seat::West.index()] {
        let mut spans = vec![Span::styled(" ", Style::default().bg(BG_TABLE))];
        spans.extend(card_on_table_spans(card));
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: w.x + 1,
                y: w.y,
                width: 4,
                height: 1,
            },
        );
    }
    let e_x = w.x + w.width.saturating_sub(1);
    let e_style = if seat_cards[Seat::East.index()].is_some() {
        label_style
    } else {
        label_dim
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("E", e_style))),
        Rect {
            x: e_x,
            y: w.y,
            width: 1,
            height: 1,
        },
    );
    if let Some(card) = seat_cards[Seat::East.index()] {
        let mut spans = card_on_table_spans(card);
        spans.push(Span::styled(" ", Style::default().bg(BG_TABLE)));
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect {
                x: e_x.saturating_sub(4),
                y: w.y,
                width: 4,
                height: 1,
            },
        );
    }

    if let Some(card) = seat_cards[Seat::South.index()] {
        f.render_widget(
            Paragraph::new(Line::from(card_on_table_spans(card))).alignment(Alignment::Center),
            center_v[5],
        );
    }
    let s_style = if seat_cards[Seat::South.index()].is_some() {
        label_style
    } else {
        label_dim
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("S", s_style))).alignment(Alignment::Center),
        center_v[6],
    );
}

fn render_review_info(f: &mut Frame, area: Rect, game: &Game) {
    let block = Block::default()
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(BG_FRAME));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = Vec::new();

    if let Some(contract) = &game.contract {
        lines.push(Line::from(vec![
            Span::styled("Contract ", Style::default().fg(TEXT_LIGHT_MUTED)),
            Span::styled(format!("{}", contract), Style::default().fg(TEXT_LIGHT)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Declarer ", Style::default().fg(TEXT_LIGHT_MUTED)),
            Span::styled(
                format!("{}", contract.declarer),
                Style::default().fg(TEXT_LIGHT),
            ),
        ]));
    }

    if let Some(play) = &game.play_state {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("N/S: ", Style::default().fg(TEXT_LIGHT_MUTED)),
            Span::styled(
                format!("{}", play.ns_tricks),
                Style::default().fg(TEXT_LIGHT),
            ),
            Span::styled("  E/W: ", Style::default().fg(TEXT_LIGHT_MUTED)),
            Span::styled(
                format!("{}", play.ew_tricks),
                Style::default().fg(TEXT_LIGHT),
            ),
        ]));
    }

    if let Some(score) = &game.score {
        let pts = score.total_points();
        let (text, color) = if pts >= 0 {
            (format!("+{}", pts), ACCENT_GREEN)
        } else {
            (format!("{}", pts), ACCENT_RED)
        };
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(text, Style::default().fg(color))));
    }

    let v_center = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(lines.len() as u16),
        Constraint::Min(0),
    ])
    .split(inner);

    let para = Paragraph::new(lines);
    f.render_widget(para, v_center[1]);
}

fn render_review_tutor(f: &mut Frame, area: Rect, review: &ReviewState) {
    let block = Block::default()
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(BG_FRAME));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Title line
    let sections = Layout::vertical([
        Constraint::Length(1), // Title
        Constraint::Min(3),   // Response area
    ])
    .split(inner);

    let title = Paragraph::new(Span::styled(
        "Tutor",
        Style::default()
            .fg(TEXT_LIGHT_MUTED)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(title, sections[0]);

    render_response_with_hint(
        f,
        sections[1],
        &review.tutor,
        "Press T to explain each play from the perspective of that player",
    );
}

