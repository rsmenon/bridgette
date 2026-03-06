use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row, Table};
use ratatui::Frame;

use crate::engine::bidding::{Auction, Bid};

use super::palette::*;

pub fn render_bidding_history(f: &mut Frame, area: Rect, auction: &Auction) {

    // Auction table — light text on gray background, centered
    let header = Row::new(vec![
        Cell::from("N"),
        Cell::from("E"),
        Cell::from("S"),
        Cell::from("W"),
    ])
    .style(
        Style::default()
            .fg(TEXT_LIGHT_MUTED)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    );

    let dealer_col = auction.dealer.index();
    let mut rows: Vec<Row> = Vec::new();
    let mut current_row = vec![Cell::from(""); 4];

    for (i, (_, bid)) in auction.bids.iter().enumerate() {
        let col = (dealer_col + i) % 4;
        if i > 0 && col == 0 {
            rows.push(Row::new(std::mem::replace(
                &mut current_row,
                vec![Cell::from(""); 4],
            )));
        }
        let cell = match bid {
            Bid::Pass => Cell::from("Pass").style(Style::default().fg(TEXT_LIGHT)),
            Bid::Double => Cell::from("Dbl").style(Style::default().fg(TEXT_LIGHT)),
            Bid::Redouble => Cell::from("Rdbl").style(Style::default().fg(TEXT_LIGHT)),
            Bid::Suit(level, suit) => {
                use crate::engine::bidding::BidSuit;
                let suit_fg = match suit {
                    BidSuit::Hearts | BidSuit::Diamonds => SUIT_RED_ON_TABLE,
                    _ => SUIT_BLACK_ON_TABLE,
                };
                Cell::from(Line::from(vec![
                    Span::styled(format!("{}", level), Style::default().fg(TEXT_LIGHT)),
                    Span::styled(suit.short().to_string(), Style::default().fg(suit_fg)),
                ]))
            }
        };
        current_row[col] = cell;
    }

    if !auction.bids.is_empty() {
        rows.push(Row::new(current_row));
    }

    // Center the table: 4 cols * 5 chars = 20 chars wide
    let table_width: u16 = 20;
    let table_height = (rows.len() as u16 + 1).min(area.height); // +1 for header
    let left_pad = area.width.saturating_sub(table_width) / 2;
    let top_pad = area.height.saturating_sub(table_height) / 2;
    let centered = Rect {
        x: area.x + left_pad,
        y: area.y + top_pad,
        width: table_width.min(area.width),
        height: area.height.saturating_sub(top_pad),
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
        ],
    )
    .header(header);

    f.render_widget(table, centered);
}
