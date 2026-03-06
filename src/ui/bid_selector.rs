use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::engine::bidding::{Bid, BidSuit};
use crate::types::Phase;

use super::palette::*;
use super::AppState;

pub fn render_bid_selector(f: &mut Frame, area: Rect, state: &AppState) {
    if state.game.phase != Phase::Bidding {
        return;
    }

    let valid_bids = state.game.auction.valid_bids();

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let selected = state.selected_bid_index;

    let mut grid_index = 0;
    for level in 1..=7u8 {
        let cols = Layout::horizontal([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
        ])
        .split(rows[(level - 1) as usize]);

        for (si, &suit) in BidSuit::ALL.iter().enumerate() {
            let bid = Bid::Suit(level, suit);
            let is_valid = valid_bids.contains(&bid);
            let is_selected = selected == grid_index;

            let line = if is_selected && is_valid {
                let style = Style::default()
                    .fg(BG_FRAME)
                    .bg(ACCENT_TEAL)
                    .add_modifier(Modifier::BOLD);
                Line::from(vec![
                    Span::styled(format!("{}", level), style),
                    Span::styled(suit.short().to_string(), style),
                ])
            } else if !is_valid {
                let style = Style::default().fg(TEXT_LIGHT_DISABLED).add_modifier(Modifier::DIM);
                Line::from(vec![
                    Span::styled(format!("{}", level), style),
                    Span::styled(suit.short().to_string(), style),
                ])
            } else {
                let suit_fg = match suit {
                    BidSuit::Hearts | BidSuit::Diamonds => SUIT_RED_ON_TABLE,
                    _ => SUIT_BLACK_ON_TABLE,
                };
                Line::from(vec![
                    Span::styled(format!("{}", level), Style::default().fg(TEXT_LIGHT)),
                    Span::styled(suit.short().to_string(), Style::default().fg(suit_fg)),
                ])
            };

            let para = Paragraph::new(line);
            f.render_widget(para, cols[si]);
            grid_index += 1;
        }
    }

    // Special bids row
    let special_cols = Layout::horizontal([
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(6),
    ])
    .split(rows[7]);

    let specials = [Bid::Pass, Bid::Double, Bid::Redouble];
    for (i, bid) in specials.iter().enumerate() {
        let is_valid = valid_bids.contains(bid);
        let is_selected = selected == 35 + i;

        let label = format!("{}", bid);
        let style = if is_selected && is_valid {
            Style::default()
                .fg(BG_FRAME)
                .bg(ACCENT_TEAL)
                .add_modifier(Modifier::BOLD)
        } else if !is_valid {
            Style::default().fg(TEXT_LIGHT_DISABLED).add_modifier(Modifier::DIM)
        } else {
            Style::default().fg(TEXT_LIGHT)
        };

        let para = Paragraph::new(Span::styled(label, style));
        f.render_widget(para, special_cols[i]);
    }
}

pub fn bid_at_index(index: usize) -> Option<Bid> {
    if index < 35 {
        let level = (index / 5) as u8 + 1;
        let suit_idx = index % 5;
        Some(Bid::Suit(level, BidSuit::ALL[suit_idx]))
    } else {
        match index - 35 {
            0 => Some(Bid::Pass),
            1 => Some(Bid::Double),
            2 => Some(Bid::Redouble),
            _ => None,
        }
    }
}

pub const BID_GRID_SIZE: usize = 38;
