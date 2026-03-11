use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Table};
use ratatui::symbols::border;
use ratatui::Frame;

use crate::config::{GameStatus, SavedGame};
use crate::types::Vulnerability;

use super::palette::*;

pub struct LibraryState {
    pub games: Vec<SavedGame>,
    pub selected_index: usize,
    pub filter_status: Option<GameStatus>,
}

impl LibraryState {
    pub fn new(games: Vec<SavedGame>) -> Self {
        Self {
            games,
            selected_index: 0,
            filter_status: None,
        }
    }

    pub fn filtered_games(&self) -> Vec<&SavedGame> {
        self.games
            .iter()
            .filter(|g| {
                if let Some(status) = &self.filter_status {
                    g.status == *status
                } else {
                    true
                }
            })
            .collect()
    }

    pub fn move_up(&mut self) {
        let count = self.filtered_games().len();
        if count > 0 && self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let count = self.filtered_games().len();
        if count > 0 && self.selected_index < count - 1 {
            self.selected_index += 1;
        }
    }

    pub fn selected_game(&self) -> Option<&SavedGame> {
        let filtered = self.filtered_games();
        filtered.get(self.selected_index).copied()
    }

    pub fn toggle_favorite(&mut self) {
        if let Some(game) = self.selected_game() {
            let id = game.id.clone();
            let new_fav = !game.favorite;
            let _ = crate::config::update_favorite(&id, new_fav);
            // Update local state
            if let Some(g) = self.games.iter_mut().find(|g| g.id == id) {
                g.favorite = new_fav;
            }
        }
    }

    pub fn cycle_filter(&mut self) {
        self.filter_status = match self.filter_status {
            None => Some(GameStatus::Completed),
            Some(GameStatus::Completed) => Some(GameStatus::InProgress),
            Some(GameStatus::InProgress) => None,
        };
        self.selected_index = 0;
    }
}

pub fn render_library(f: &mut Frame, area: Rect, state: &LibraryState) {
    let outer = Layout::vertical([
        Constraint::Length(1), // Title bar
        Constraint::Min(5),   // Table
        Constraint::Length(1), // Footer controls
    ])
    .split(area);

    // Title bar
    let filter_text = match state.filter_status {
        None => String::new(),
        Some(GameStatus::Completed) => " [Filter: Completed]".to_string(),
        Some(GameStatus::InProgress) => " [Filter: In Progress]".to_string(),
    };
    let title = Paragraph::new(Line::from(vec![
        Span::styled(
            " Library ",
            Style::default()
                .fg(TEXT_LIGHT)
                .bg(BG_FRAME_ALT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {} games{} ", state.games.len(), filter_text),
            Style::default().fg(TEXT_LIGHT_MUTED).bg(BG_FRAME),
        ),
    ]))
    .style(Style::default().bg(BG_FRAME));
    f.render_widget(title, outer[0]);

    // Table
    let filtered = state.filtered_games();

    let header = Row::new(vec!["★", "Status", "Date", "Dealer", "Vul", "Declarer", "Contract", "Score"])
        .style(
            Style::default()
                .fg(ACCENT_MUTED_BLUE)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .bottom_margin(0);

    let rows: Vec<Row> = filtered
        .iter()
        .enumerate()
        .map(|(i, game)| {
            let is_selected = i == state.selected_index;

            // Per-cell coloring
            let fav_style = if is_selected {
                Style::default().fg(ACCENT_MUTED_BLUE).bg(BG_SELECTED_BLUE).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(ACCENT_MUTED_BLUE)
            };
            let fav = if game.favorite { "★" } else { " " };

            let (status_text, status_color) = match game.status {
                GameStatus::InProgress => {
                    if game.contract.is_some() {
                        ("▶ Play", None)
                    } else {
                        ("≡ Auction", None)
                    }
                }
                GameStatus::Completed => ("✓ Done", Some(ACCENT_GREEN)),
            };

            let score_str = game.score_display();
            let score_color = if score_str.starts_with('+') {
                ACCENT_GREEN
            } else if score_str.starts_with('-') {
                ACCENT_RED
            } else {
                TEXT_DARK_MUTED
            };

            let vul_str = match game.vulnerability {
                Vulnerability::None => "—",
                Vulnerability::NorthSouth => "N/S",
                Vulnerability::EastWest => "E/W",
                Vulnerability::Both => "Both",
            };

            if is_selected {
                let base = Style::default().fg(TEXT_LIGHT).bg(BG_SELECTED_BLUE).add_modifier(Modifier::BOLD);
                let vul_style = if game.vulnerability == Vulnerability::None {
                    base
                } else {
                    Style::default().fg(ACCENT_RED).bg(BG_SELECTED_BLUE).add_modifier(Modifier::BOLD)
                };
                Row::new(vec![
                    Cell::from(fav.to_string()).style(fav_style),
                    Cell::from(status_text.to_string()).style(match status_color {
                        Some(c) => Style::default().fg(c).bg(BG_SELECTED_BLUE).add_modifier(Modifier::BOLD),
                        None => base,
                    }),
                    Cell::from(game.timestamp_display()).style(base),
                    Cell::from(game.dealer.clone()).style(base),
                    Cell::from(vul_str).style(vul_style),
                    Cell::from(game.declarer().to_string()).style(base),
                    Cell::from(game.contract_display()).style(base),
                    Cell::from(score_str).style(base),
                ])
            } else {
                let (bg, fg) = if i % 2 == 0 {
                    (BG_FRAME, TEXT_LIGHT)
                } else {
                    (BG_FRAME_ALT, TEXT_LIGHT)
                };
                let base = Style::default().fg(fg).bg(bg);
                let vul_style = if game.vulnerability == Vulnerability::None {
                    Style::default().fg(TEXT_LIGHT_MUTED).bg(bg)
                } else {
                    Style::default().fg(ACCENT_RED).bg(bg)
                };
                Row::new(vec![
                    Cell::from(fav.to_string()).style(fav_style.bg(bg)),
                    Cell::from(status_text.to_string()).style(match status_color {
                        Some(c) => Style::default().fg(c).bg(bg),
                        None => base,
                    }),
                    Cell::from(game.timestamp_display()).style(base),
                    Cell::from(game.dealer.clone()).style(base),
                    Cell::from(vul_str).style(vul_style),
                    Cell::from(game.declarer().to_string()).style(base),
                    Cell::from(game.contract_display()).style(base),
                    Cell::from(score_str).style(Style::default().fg(score_color).bg(bg)),
                ])
            }
        })
        .collect();

    let widths = [
        Constraint::Length(2),  // ★
        Constraint::Length(12), // Status
        Constraint::Length(18), // Date
        Constraint::Length(8),  // Dealer
        Constraint::Length(5),  // Vul
        Constraint::Length(10), // Declarer
        Constraint::Length(10), // Contract
        Constraint::Length(8),  // Score
    ];

    let table_block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(BORDER_DARK))
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(BG_FRAME));

    if filtered.is_empty() {
        let empty_msg = Paragraph::new(Line::from(Span::styled(
            "No games found. Press [N] to start a new game.",
            Style::default().fg(TEXT_LIGHT_MUTED),
        )))
        .block(table_block)
        .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(empty_msg, outer[1]);
    } else {
        let table = Table::new(rows, widths)
            .header(header)
            .block(table_block);
        f.render_widget(table, outer[1]);
    }

    // Footer controls with pill styling
    let controls = [
        ("Enter", "Open"),
        ("F", "Favorite"),
        ("/", "Filter"),
        ("N", "New Game"),
        ("Esc", "Back"),
        ("↑↓", "Navigate"),
        ("Q", "Quit"),
    ];

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

    let footer = Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_FRAME));
    f.render_widget(footer, outer[2]);
}
