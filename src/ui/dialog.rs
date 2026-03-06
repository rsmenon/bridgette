use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use super::palette::*;

fn centered_popup(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

pub fn render_confirm_quit(f: &mut Frame, area: Rect) {
    let popup = centered_popup(36, 7, area);
    f.render_widget(Clear, popup);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Quit the game?",
            Style::default()
                .fg(TEXT_LIGHT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [", Style::default().fg(TEXT_LIGHT_DISABLED)),
            Span::styled(
                "Y",
                Style::default()
                    .fg(BG_FRAME)
                    .bg(TEXT_LIGHT_MUTED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] ", Style::default().fg(TEXT_LIGHT_DISABLED)),
            Span::styled("Yes  ", Style::default().fg(TEXT_LIGHT_MUTED)),
            Span::styled(" [", Style::default().fg(TEXT_LIGHT_DISABLED)),
            Span::styled(
                "N",
                Style::default()
                    .fg(BG_FRAME)
                    .bg(TEXT_LIGHT_MUTED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] ", Style::default().fg(TEXT_LIGHT_DISABLED)),
            Span::styled("No", Style::default().fg(TEXT_LIGHT_MUTED)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(TEXT_LIGHT_MUTED))
        .title(" Quit ")
        .style(Style::default().bg(BG_FRAME_ALT).fg(TEXT_LIGHT_MUTED));
    let para = Paragraph::new(lines)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(Wrap { trim: false });
    f.render_widget(para, popup);
}

pub fn render_confirm_leave(f: &mut Frame, area: Rect) {
    let popup = centered_popup(44, 8, area);
    f.render_widget(Clear, popup);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "Leave current game?",
            Style::default()
                .fg(TEXT_LIGHT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [", Style::default().fg(TEXT_LIGHT_DISABLED)),
            Span::styled(
                "S",
                Style::default()
                    .fg(BG_FRAME)
                    .bg(TEXT_LIGHT_MUTED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] ", Style::default().fg(TEXT_LIGHT_DISABLED)),
            Span::styled("Save  ", Style::default().fg(TEXT_LIGHT_MUTED)),
            Span::styled(" [", Style::default().fg(TEXT_LIGHT_DISABLED)),
            Span::styled(
                "A",
                Style::default()
                    .fg(BG_FRAME)
                    .bg(TEXT_LIGHT_MUTED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] ", Style::default().fg(TEXT_LIGHT_DISABLED)),
            Span::styled("Abandon  ", Style::default().fg(TEXT_LIGHT_MUTED)),
            Span::styled(" [", Style::default().fg(TEXT_LIGHT_DISABLED)),
            Span::styled(
                "G",
                Style::default()
                    .fg(BG_FRAME)
                    .bg(TEXT_LIGHT_MUTED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] ", Style::default().fg(TEXT_LIGHT_DISABLED)),
            Span::styled("Go Back", Style::default().fg(TEXT_LIGHT_MUTED)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(border::ROUNDED)
        .border_style(Style::default().fg(TEXT_LIGHT_MUTED))
        .title(" Leave Game ")
        .style(Style::default().bg(BG_FRAME_ALT).fg(TEXT_LIGHT_MUTED));
    let para = Paragraph::new(lines)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(Wrap { trim: false });
    f.render_widget(para, popup);
}
