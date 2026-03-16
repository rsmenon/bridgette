use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::types::{Phase, Seat};


use super::palette::*;
use super::AppState;

pub fn render_controls(f: &mut Frame, area: Rect, state: &AppState) {
    let rows = Layout::vertical([
        Constraint::Length(1), // separator
        Constraint::Length(1), // status message
        Constraint::Length(1), // controls
    ])
    .split(area);

    // Separator line
    let sep = "─".repeat(area.width as usize);
    f.render_widget(
        Paragraph::new(Span::styled(&sep, Style::default().fg(BORDER_DARK))),
        rows[0],
    );

    if let Some((ref msg, _)) = state.status_message {
        let para = Paragraph::new(Line::from(vec![
            Span::styled(" ● ", Style::default().fg(ACCENT_MUTED_BLUE)),
            Span::styled(msg.as_str(), Style::default().fg(ACCENT_MUTED_BLUE)),
        ]))
        .style(Style::default().bg(BG_CONTROLS));
        f.render_widget(para, rows[1]);
    } else {
        // Fill empty status row with background
        f.render_widget(
            Paragraph::new("").style(Style::default().bg(BG_CONTROLS)),
            rows[1],
        );
    }

    let current = state.game.current_seat();
    let human_turn = match state.game.phase {
        Phase::Bidding => current == Seat::South,
        Phase::Playing => {
            current == Seat::South
                || state.game.contract.as_ref().is_some_and(|c| {
                    c.declarer.is_ns() && (current == c.declarer || current == c.dummy)
                })
        }
        Phase::Finished => false,
    };

    // Standard shortcuts always shown, plus optional phase-specific extras after a separator.
    let standard: &[(&str, &str)] = &[
        ("N", "New"),
        ("L", "Library"),
        ("B", "Probability"),
        ("T", "Tutor"),
        ("?", "Help"),
        ("Q", "Quit"),
    ];

    let bid_extras: &[(&str, &str)] = if matches!(state.game.phase, Phase::Bidding) && human_turn {
        &[
            ("↑↓←→", "Navigate"),
            ("Enter", "Bid"),
            ("P", "Pass"),
            ("X", "Dbl"),
            ("1-7", "Level"),
        ]
    } else {
        &[]
    };

    fn key_spans(pairs: &[(&str, &str)]) -> Vec<Span<'static>> {
        pairs
            .iter()
            .flat_map(|(key, desc)| {
                vec![
                    Span::styled(" [", Style::default().fg(TEXT_LIGHT_DISABLED)),
                    Span::styled(
                        key.to_string(),
                        Style::default()
                            .fg(BG_FRAME)
                            .bg(TEXT_LIGHT_MUTED)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("] ", Style::default().fg(TEXT_LIGHT_DISABLED)),
                    Span::styled(
                        format!("{} ", desc),
                        Style::default().fg(TEXT_LIGHT_MUTED),
                    ),
                ]
            })
            .collect()
    }

    let mut spans = key_spans(&standard);
    if !bid_extras.is_empty() {
        spans.push(Span::styled(
            " │ ",
            Style::default().fg(TEXT_LIGHT_DISABLED),
        ));
        spans.extend(key_spans(bid_extras));
    }

    let para = Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_CONTROLS));
    f.render_widget(para, rows[2]);
}
