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
            Span::styled(" ● ", Style::default().fg(ACCENT_TEAL)),
            Span::styled(msg.as_str(), Style::default().fg(ACCENT_TEAL)),
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

    let tutor_active = state.tutor.is_some();
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

    let controls: Vec<(&str, &str)> = match state.game.phase {
        Phase::Bidding if human_turn => {
            let mut v: Vec<(&str, &str)> = vec![
                ("↑↓←→", "Navigate"),
                ("Enter", "Bid"),
                ("P", "Pass"),
                ("D", "Dbl"),
                ("R", "Rdbl"),
                ("1-7", "Level"),
            ];
            if tutor_active {
                v.push(("Tab", "Ask"));
                v.push(("T/Esc", "Close Tutor"));
            } else {
                v.push(("T", "Tutor"));
            }
            v.extend_from_slice(&[("H", "Help"), ("Q", "Quit")]);
            v
        }
        Phase::Bidding => vec![
            ("N", "New Game"),
            ("L", "Library"),
            ("?", "Help"),
            ("Q", "Quit"),
        ],
        Phase::Playing if human_turn => {
            let mut v: Vec<(&str, &str)> = vec![
                ("←→", "Select"),
                ("↑↓", "Suit"),
                ("Enter", "Play"),
                ("A-9", "By Rank"),
            ];
            if tutor_active {
                v.push(("Tab", "Ask"));
                v.push(("T/Esc", "Close Tutor"));
            } else {
                v.push(("T", "Tutor"));
            }
            v.extend_from_slice(&[("H", "Help"), ("Q", "Quit")]);
            v
        }
        Phase::Playing => vec![
            ("N", "New Game"),
            ("L", "Library"),
            ("?", "Help"),
            ("Q", "Quit"),
        ],
        Phase::Finished => vec![
            ("N", "New Game"),
            ("L", "Library"),
            ("T", "Tutor"),
            ("Q", "Quit"),
            ("H", "Help"),
        ],
    };

    let spans: Vec<Span> = controls
        .iter()
        .flat_map(|(key, desc)| {
            vec![
                Span::styled(" [", Style::default().fg(TEXT_LIGHT_DISABLED)),
                Span::styled(
                    *key,
                    Style::default()
                        .fg(BG_FRAME)
                        .bg(TEXT_LIGHT_MUTED)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("] ", Style::default().fg(TEXT_LIGHT_DISABLED)),
                Span::styled(format!("{} ", desc), Style::default().fg(TEXT_LIGHT_MUTED)),
            ]
        })
        .collect();

    let para = Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_CONTROLS));
    f.render_widget(para, rows[2]);
}
