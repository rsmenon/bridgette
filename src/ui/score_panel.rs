use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::engine::scoring::Score;
use crate::types::Phase;

use super::palette::*;
use super::AppState;

pub fn render_score_panel(f: &mut Frame, area: Rect, state: &AppState) {
    let mut lines: Vec<Line> = Vec::new();

    if let Some(play) = &state.game.play_state {
        lines.push(Line::from(vec![
            Span::styled(
                "N/S: ",
                Style::default()
                    .fg(ACCENT_TEAL)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}", play.ns_tricks),
                Style::default().fg(TEXT_LIGHT),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(
                "E/W: ",
                Style::default()
                    .fg(ACCENT_TEAL)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}", play.ew_tricks),
                Style::default().fg(TEXT_LIGHT),
            ),
        ]));

        if let Some(contract) = &state.game.contract {
            let needed = contract.level as usize + 6;
            let declarer_tricks = if contract.declarer.is_ns() {
                play.ns_tricks
            } else {
                play.ew_tricks
            };
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Need: ", Style::default().fg(TEXT_LIGHT_MUTED)),
                Span::styled(format!("{}", needed), Style::default().fg(TEXT_LIGHT)),
                Span::styled("  Have: ", Style::default().fg(TEXT_LIGHT_MUTED)),
                Span::styled(format!("{}", declarer_tricks), Style::default().fg(TEXT_LIGHT)),
            ]));

            if state.game.phase == Phase::Finished {
                let result = declarer_tricks as i32 - needed as i32;
                let msg = if result >= 0 {
                    format!("Made +{}", result)
                } else {
                    format!("Down {}", -result)
                };
                let color = if result >= 0 {
                    ACCENT_GREEN
                } else {
                    ACCENT_RED
                };
                lines.push(Line::from(Span::styled(
                    msg,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                )));

                if let Some(score) = &state.game.score {
                    match score {
                        Score::Made {
                            total,
                            contract_points,
                            overtrick_points,
                            game_bonus,
                            slam_bonus,
                            insult_bonus,
                        } => {
                            lines.push(Line::from(Span::styled(
                                format!("Score: +{}", total),
                                Style::default()
                                    .fg(ACCENT_GREEN)
                                    .add_modifier(Modifier::BOLD),
                            )));
                            let mut parts = vec![format!("{}ctr", contract_points)];
                            if *overtrick_points > 0 {
                                parts.push(format!("{}ot", overtrick_points));
                            }
                            parts.push(format!(
                                "{}{}",
                                game_bonus,
                                if *game_bonus >= 300 { "gm" } else { "ps" }
                            ));
                            if *slam_bonus > 0 {
                                parts.push(format!("{}sl", slam_bonus));
                            }
                            if *insult_bonus > 0 {
                                parts.push(format!("{}ins", insult_bonus));
                            }
                            lines.push(Line::from(Span::styled(
                                parts.join("+"),
                                Style::default().fg(TEXT_LIGHT_MUTED),
                            )));
                        }
                        Score::Defeated { penalty, .. } => {
                            lines.push(Line::from(Span::styled(
                                format!("Penalty: {}", penalty),
                                Style::default()
                                    .fg(ACCENT_RED)
                                    .add_modifier(Modifier::BOLD),
                            )));
                        }
                        Score::PassedOut => {}
                    }
                }
            }
        }
    } else if state.game.passed_out {
        lines.push(Line::from(Span::styled(
            "Passed out — no play",
            Style::default().fg(TEXT_LIGHT_MUTED),
        )));
    }

    // Center vertically
    let content_height = lines.len() as u16;
    let top_pad = area.height.saturating_sub(content_height) / 2;
    let centered = Layout::vertical([
        Constraint::Length(top_pad),
        Constraint::Min(0),
    ])
    .split(area);

    let para = Paragraph::new(lines).alignment(Alignment::Center);
    f.render_widget(para, centered[1]);
}
