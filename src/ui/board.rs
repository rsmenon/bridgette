use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Padding, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use crate::engine::card::Suit;
use crate::engine::hand::Hand;
use crate::types::{Phase, Seat, Vulnerability};

use super::bidding_panel::render_bidding_history;
use super::palette::*;
use super::score_panel::render_score_panel;
use super::AppState;

pub fn render_board(f: &mut Frame, area: Rect, state: &AppState) {
    // Paint the entire board area green
    let board_bg = Block::default().style(Style::default().bg(BG_TABLE));
    f.render_widget(board_bg, area);

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

    // Corner panels (gray background)
    render_stats_panel(f, top_cols[0], state);
    render_auction_corner(f, top_cols[2], state);
    render_error_corner(f, bot_cols[0], state);
    render_score_corner(f, bot_cols[2], state);

    // Player hands
    render_hand_cell(f, top_cols[1], Seat::North, state);
    render_hand_cell(f, mid_cols[0], Seat::West, state);
    render_hand_cell(f, mid_cols[2], Seat::East, state);
    render_hand_cell(f, bot_cols[1], Seat::South, state);

    render_center(f, mid_cols[1], state);
}

/// Top-left corner: game info as a centered table (gray bg)
fn render_stats_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(BG_FRAME));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let label_style = Style::default().fg(TEXT_LIGHT_MUTED);
    let value_style = Style::default().fg(TEXT_LIGHT);

    let (status_str, status_color) = match state.game.phase {
        Phase::Finished => ("Completed", ACCENT_GREEN),
        Phase::Playing => ("Play", ACCENT_TEAL),
        Phase::Bidding => {
            if state.game.hands.iter().all(|h| h.is_empty()) {
                ("Not Started", TEXT_LIGHT_MUTED)
            } else {
                ("Auction", ACCENT_TEAL)
            }
        }
    };

    let vul = state.game.vulnerability;
    let vul_str = match vul {
        Vulnerability::None => "None",
        Vulnerability::NorthSouth => "N/S",
        Vulnerability::EastWest => "E/W",
        Vulnerability::Both => "Both",
    };
    let vul_color = if vul == Vulnerability::None { TEXT_LIGHT_MUTED } else { ACCENT_RED };

    let mut table_rows = vec![
        Row::new(vec![
            Cell::from("Dealer").style(label_style),
            Cell::from(format!("{}", state.game.dealer)).style(value_style),
        ]),
        Row::new(vec![
            Cell::from("Vul").style(label_style),
            Cell::from(vul_str).style(Style::default().fg(vul_color)),
        ]),
        Row::new(vec![
            Cell::from("Status").style(label_style),
            Cell::from(status_str).style(Style::default().fg(status_color)),
        ]),
    ];

    if let Some(contract) = &state.game.contract {
        table_rows.push(Row::new(vec![
            Cell::from("Contract").style(label_style),
            Cell::from(format!("{}", contract)).style(value_style),
        ]));
    } else if state.game.passed_out {
        table_rows.push(Row::new(vec![
            Cell::from("Contract").style(label_style),
            Cell::from("Passed Out").style(Style::default().fg(TEXT_LIGHT_MUTED)),
        ]));
    }

    // Show opening lead card
    if let Some(play) = &state.game.play_state {
        let lead_card = if !play.tricks.is_empty() {
            play.tricks[0].cards.first().map(|(_, c)| *c)
        } else {
            play.current_trick.cards.first().map(|(_, c)| *c)
        };
        if let Some(card) = lead_card {
            table_rows.push(Row::new(vec![
                Cell::from("Lead").style(label_style),
                Cell::from(format!("{}", card)).style(value_style),
            ]));
        }
    }

    if let Some(ref ended) = state.game_ended_at {
        table_rows.push(Row::new(vec![
            Cell::from("Ended").style(label_style),
            Cell::from(ended.as_str()).style(value_style),
        ]));
    }

    table_rows.push(Row::new(vec![
        Cell::from("System").style(label_style),
        Cell::from(state.bidding_system.as_str()).style(value_style),
    ]));

    let not_started = state.game.phase == Phase::Bidding && state.game.hands.iter().all(|h| h.is_empty());
    if state.game.phase != Phase::Finished && !not_started {
        table_rows.push(Row::new(vec![
            Cell::from("Turn").style(label_style),
            Cell::from(format!("{}", state.game.current_seat())).style(value_style),
        ]));
    }


    // Center the table vertically and horizontally
    let table_width: u16 = 24;
    let table_height = table_rows.len() as u16;
    let left_pad = inner.width.saturating_sub(table_width) / 2;
    let top_pad = inner.height.saturating_sub(table_height) / 2;
    let centered = Rect {
        x: inner.x + left_pad,
        y: inner.y + top_pad,
        width: table_width.min(inner.width),
        height: table_height.min(inner.height),
    };

    let table = Table::new(
        table_rows,
        [Constraint::Length(9), Constraint::Min(8)],
    );
    f.render_widget(table, centered);
}

/// Top-right corner: auction history centered (gray bg)
fn render_auction_corner(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(BG_FRAME));
    let inner = block.inner(area);
    f.render_widget(block, area);

    render_bidding_history(f, inner, &state.game.auction);
}

/// Bottom-left corner: agent errors (gray bg)
fn render_error_corner(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(BG_FRAME));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if state.agent_errors.is_empty() {
        return;
    }

    let max_lines = inner.height as usize;
    let errors = &state.agent_errors;
    let skip = errors.len().saturating_sub(max_lines);
    let lines: Vec<Line> = errors
        .iter()
        .skip(skip)
        .map(|e| {
            Line::from(Span::styled(
                e.clone(),
                Style::default().fg(ACCENT_RED),
            ))
        })
        .collect();

    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(para, inner);
}

/// Bottom-right corner: score panel (gray bg)
fn render_score_corner(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .padding(Padding::new(1, 1, 0, 0))
        .style(Style::default().bg(BG_FRAME));
    let inner = block.inner(area);
    f.render_widget(block, area);

    render_score_panel(f, inner, state);
}

fn render_hand_cell(f: &mut Frame, area: Rect, seat: Seat, state: &AppState) {
    let game = &state.game;
    let is_current = game.phase != Phase::Finished && game.current_seat() == seat;
    let visible = game.is_hand_visible(seat) || game.phase == Phase::Finished;
    let is_agent = seat != Seat::South
        && !(game.phase == Phase::Playing
            && seat == Seat::North
            && game.contract.as_ref().is_some_and(|c| c.declarer.is_ns()));

    let bg = if is_current { BG_CONTENT_ACTIVE } else { BG_CONTENT };
    let block = Block::default()
        .padding(Padding::new(1, 1, 1, 1))
        .style(Style::default().bg(bg));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Player location label inside the pane
    let title_color = TEXT_DARK;
    let title_line = Line::from(Span::styled(
        format!("{}", seat),
        Style::default()
            .fg(title_color)
            .add_modifier(Modifier::BOLD),
    ));
    if inner.height > 0 {
        let title_area = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(title_line), title_area);
    }

    // Content area below title + 1 line spacing
    let content = Rect {
        x: inner.x,
        y: inner.y + 2,
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };

    // Determine what to show below cards
    let is_finished = game.phase == Phase::Finished;
    let show_hand_score = is_finished || (seat == Seat::South && game.phase == Phase::Bidding);
    let show_agent_status = is_agent && !is_finished;

    let bottom_height: u16 = if show_hand_score || show_agent_status { 2 } else { 0 };
    let sections = Layout::vertical([
        Constraint::Min(4),              // Cards area
        Constraint::Length(bottom_height), // Score or agent info
    ])
    .split(content);

    if visible {
        let hand = if is_finished {
            &game.dealt_hands[seat.index()]
        } else {
            game.hand(seat)
        };
        let lines = hand_lines(hand, seat, state);
        let para = Paragraph::new(lines);
        f.render_widget(para, sections[0]);

        if show_hand_score {
            render_hand_score(f, sections[1], hand);
        }
    } else {
        let count = game.hand(seat).len();
        let text = format!("[{} cards]", count);
        let v_center = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(sections[0]);
        let para = Paragraph::new(text)
            .style(Style::default().fg(TEXT_DARK_MUTED).bg(bg).add_modifier(Modifier::DIM));
        f.render_widget(para, v_center[1]);
    }

    if show_agent_status {
        render_agent_status(f, sections[1], seat, state);
    }
}

/// Render agent model + thinking/ready status in the bottom of agent player boxes.
fn render_agent_status(f: &mut Frame, area: Rect, seat: Seat, state: &AppState) {
    let info = &state.agent_info;

    let model = info.model_for(seat);
    let agent_line = Line::from(vec![
        Span::styled("Agent: ", Style::default().fg(TEXT_DARK_MUTED)),
        Span::styled(
            model.to_string(),
            Style::default().fg(TEXT_DARK_MUTED).add_modifier(Modifier::ITALIC),
        ),
    ]);

    let is_thinking = state
        .agent_thinking
        .as_ref()
        .is_some_and(|(s, _)| *s == seat);

    let status_line = if is_thinking {
        let elapsed = state.agent_thinking.as_ref().unwrap().1.elapsed();
        thinking_braille_line(elapsed)
    } else {
        Line::from(Span::styled(
            "Ready",
            Style::default().fg(ACCENT_SAGE),
        ))
    };

    let lines = vec![agent_line, status_line];
    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

fn hand_lines(hand: &Hand, seat: Seat, state: &AppState) -> Vec<Line<'static>> {
    let game = &state.game;
    let is_current_player = game.phase == Phase::Playing && game.current_seat() == seat;
    let eligible = if is_current_player {
        game.eligible_cards()
    } else {
        vec![]
    };

    let mut lines = Vec::new();
    for &suit in Suit::ALL.iter().rev() {
        let cards = hand.cards_of_suit(suit);

        let suit_color = if suit.is_red() {
            SUIT_RED
        } else {
            SUIT_BLACK
        };

        let mut spans = vec![Span::styled(
            format!("{}  ", suit.symbol()),
            Style::default().fg(suit_color).add_modifier(Modifier::BOLD),
        )];

        if cards.is_empty() {
            spans.push(Span::styled("—", Style::default().fg(TEXT_DARK_DISABLED).add_modifier(Modifier::DIM)));
        } else {
            for card in cards.iter().rev() {
                let is_eligible = eligible.contains(card);
                let is_selected = is_current_player
                    && state.selected_card_index.is_some()
                    && is_eligible
                    && eligible
                        .iter()
                        .position(|c| c == card)
                        .is_some_and(|pos| pos == state.selected_card_index.unwrap());

                let card_style = if is_selected {
                    Style::default()
                        .fg(TEXT_DARK)
                        .bg(ACCENT_TEAL)
                        .add_modifier(Modifier::BOLD)
                } else if is_current_player && !is_eligible {
                    Style::default().fg(TEXT_DARK_DISABLED)
                } else {
                    Style::default().fg(TEXT_DARK)
                };

                spans.push(Span::styled(
                    format!("{:<3}", card.rank.short()),
                    card_style,
                ));
            }
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn render_center(f: &mut Frame, area: Rect, state: &AppState) {
    let inner = Rect {
        x: area.x + 1,
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };

    // "BRIDGE" title centered in the green box
    let title_v = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(inner);

    let title_para = Paragraph::new(Line::from(Span::styled(
        "B R I D G E",
        Style::default()
            .fg(TEXT_LIGHT)
            .bg(BG_TABLE)
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center);
    f.render_widget(title_para, title_v[1]);

    if state.game.phase == Phase::Finished {
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

    let play = state.game.play_state.as_ref();
    let seat_cards = if let Some(play) = play {
        let cards = &play.current_trick.cards;
        let mut map: [Option<&crate::engine::card::Card>; 4] = [None; 4];
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

    let n_style = if seat_cards[Seat::North.index()].is_some() { label_style } else { label_dim };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("N", n_style))).alignment(Alignment::Center),
        center_v[0],
    );
    if let Some(card) = seat_cards[Seat::North.index()] {
        f.render_widget(
            Paragraph::new(Line::from(card_on_table_spans(card)))
                .alignment(Alignment::Center),
            center_v[1],
        );
    }

    let w = center_v[3];
    let w_style = if seat_cards[Seat::West.index()].is_some() { label_style } else { label_dim };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("W", w_style))),
        Rect { x: w.x, y: w.y, width: 1, height: 1 },
    );
    if let Some(card) = seat_cards[Seat::West.index()] {
        let mut spans = vec![Span::styled(" ", Style::default().bg(BG_TABLE))];
        spans.extend(card_on_table_spans(card));
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect { x: w.x + 1, y: w.y, width: 4, height: 1 },
        );
    }
    let e_x = w.x + w.width.saturating_sub(1);
    let e_style = if seat_cards[Seat::East.index()].is_some() { label_style } else { label_dim };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("E", e_style))),
        Rect { x: e_x, y: w.y, width: 1, height: 1 },
    );
    if let Some(card) = seat_cards[Seat::East.index()] {
        let mut spans = card_on_table_spans(card);
        spans.push(Span::styled(" ", Style::default().bg(BG_TABLE)));
        f.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect { x: e_x.saturating_sub(4), y: w.y, width: 4, height: 1 },
        );
    }

    if let Some(card) = seat_cards[Seat::South.index()] {
        f.render_widget(
            Paragraph::new(Line::from(card_on_table_spans(card)))
                .alignment(Alignment::Center),
            center_v[5],
        );
    }
    let s_style = if seat_cards[Seat::South.index()].is_some() { label_style } else { label_dim };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled("S", s_style))).alignment(Alignment::Center),
        center_v[6],
    );
}

/// Braille six-double spinner animation for "Thinking..."
fn thinking_braille_line(elapsed: std::time::Duration) -> Line<'static> {
    let symbols = throbber_widgets_tui::BRAILLE_SIX_DOUBLE.symbols;
    let ms = elapsed.as_millis() as usize;
    let idx = (ms / 100) % symbols.len();
    let symbol = symbols[idx];

    Line::from(vec![
        Span::styled(
            format!("{} ", symbol),
            Style::default()
                .fg(ACCENT_TEAL)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Thinking...",
            Style::default().fg(ACCENT_TEAL).add_modifier(Modifier::BOLD),
        ),
    ])
}

pub fn card_on_table_spans(card: &crate::engine::card::Card) -> Vec<Span<'static>> {
    let suit_color = if card.suit.is_red() {
        SUIT_RED_ON_TABLE
    } else {
        SUIT_BLACK_ON_TABLE
    };
    vec![
        Span::styled(
            card.rank.short().to_string(),
            Style::default().fg(TEXT_LIGHT).bg(BG_TABLE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            card.suit.symbol().to_string(),
            Style::default().fg(suit_color).bg(BG_TABLE).add_modifier(Modifier::BOLD),
        ),
    ]
}

/// Render HCP/Dist scoring line below a hand.
pub fn render_hand_score(f: &mut Frame, area: Rect, hand: &Hand) {
    let muted = Style::default().fg(TEXT_DARK_MUTED);
    let score_line = if !hand.is_empty() {
        let (hcp, dist, total) = sayc_score(hand);
        Line::from(vec![
            Span::styled("HCP:", muted),
            Span::styled(format!("{} ", hcp), muted),
            Span::styled("Dist:", muted),
            Span::styled(format!("{} ", dist), muted),
            Span::styled(format!("= {}", total), muted),
        ])
    } else {
        Line::from(vec![
            Span::styled("HCP:", muted),
            Span::styled("-- ", muted),
            Span::styled("Dist:", muted),
            Span::styled("-- ", muted),
            Span::styled("= --", muted),
        ])
    };
    let para = Paragraph::new(vec![Line::from(""), score_line]);
    f.render_widget(para, area);
}

fn sayc_score(hand: &Hand) -> (u32, u32, u32) {
    let hcp = hand.hcp();
    let dist = hand.dist_points();
    (hcp, dist, hcp + dist)
}
