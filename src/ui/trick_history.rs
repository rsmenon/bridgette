use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::types::Seat;

use super::palette::*;
use super::AppState;

pub fn render_trick_history(f: &mut Frame, area: Rect, state: &AppState) {
    let play = match &state.game.play_state {
        Some(p) => p,
        None => return,
    };

    if play.tricks.is_empty() {
        return;
    }

    let trump = state
        .game
        .contract
        .as_ref()
        .and_then(|c| {
            use crate::engine::bidding::BidSuit;
            if c.suit == BidSuit::NoTrump {
                None
            } else {
                Some(c.suit)
            }
        });

    let header = Row::new(vec![
        Cell::from("#"),
        Cell::from("N"),
        Cell::from("E"),
        Cell::from("S"),
        Cell::from("W"),
        Cell::from("Win"),
    ])
    .style(
        Style::default()
            .fg(TEXT_LIGHT_MUTED)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    );

    let mut rows: Vec<Row> = Vec::new();

    for (i, trick) in play.tricks.iter().enumerate() {
        let winner = trick.winner(trump).unwrap_or(Seat::North);
        let mut cards_by_seat: [Option<crate::engine::card::Card>; 4] = [None; 4];
        for &(seat, card) in &trick.cards {
            cards_by_seat[seat.index()] = Some(card);
        }

        let mut cells = Vec::new();
        cells.push(Cell::from(format!("{}", i + 1)).style(Style::default().fg(TEXT_LIGHT_DISABLED).add_modifier(Modifier::DIM)));

        for &seat in &Seat::ALL {
            if let Some(card) = cards_by_seat[seat.index()] {
                let suit_color = if card.suit.is_red() {
                    SUIT_RED_ON_TABLE
                } else {
                    SUIT_BLACK_ON_TABLE
                };
                let rank_color = TEXT_LIGHT;
                let modifier = if seat == winner { Modifier::BOLD } else { Modifier::empty() };
                let line = Line::from(vec![
                    Span::styled(
                        card.rank.short().to_string(),
                        Style::default().fg(rank_color).add_modifier(modifier),
                    ),
                    Span::styled(
                        card.suit.symbol().to_string(),
                        Style::default().fg(suit_color).add_modifier(modifier),
                    ),
                ]);
                cells.push(Cell::from(line));
            } else {
                cells.push(Cell::from(""));
            }
        }

        cells.push(
            Cell::from(winner.short()).style(
                Style::default()
                    .fg(ACCENT_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        );

        rows.push(Row::new(cells));
    }

    // Title + table layout
    let sections = Layout::vertical([
        Constraint::Length(1), // Title
        Constraint::Min(3),   // Table
    ])
    .split(area);

    let title = Paragraph::new(Span::styled(
        "Tricks",
        Style::default()
            .fg(TEXT_LIGHT_MUTED)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(title, sections[0]);

    let table_area = sections[1];
    let inner_height = table_area.height.saturating_sub(2) as usize;
    let max_scroll = rows.len().saturating_sub(inner_height);
    let scroll = state.trick_scroll.min(max_scroll);
    let visible_rows: Vec<Row> = rows.into_iter().skip(scroll).collect();

    let table = Table::new(
        visible_rows,
        [
            Constraint::Length(2),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(3),
        ],
    )
    .header(header);

    let constrained = Rect {
        width: 21u16.min(table_area.width),
        ..table_area
    };
    f.render_widget(table, constrained);
}
