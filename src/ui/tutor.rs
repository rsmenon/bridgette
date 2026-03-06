use std::time::Instant;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use super::palette::*;

/// State for the in-game tutor panel.
pub struct TutorState {
    /// The tutor's latest response.
    pub response: String,
    /// Scroll offset for the response area.
    pub scroll: u16,
    /// Whether a tutor query is in-flight.
    pub pending: bool,
    /// Timestamp when the pending query started (for animation).
    pub pending_since: Option<Instant>,
    /// Turn counter for the last auto-dispatched recommendation.
    /// Prevents re-dispatching on the same turn.
    pub last_auto_turn: Option<usize>,
}

impl TutorState {
    pub fn new() -> Self {
        Self {
            response: String::new(),
            scroll: 0,
            pending: false,
            pending_since: None,
            last_auto_turn: None,
        }
    }
}

pub fn render_tutor_pane(f: &mut Frame, area: Rect, state: &TutorState) {
    let sections = Layout::vertical([
        Constraint::Length(1), // Title
        Constraint::Min(3),   // Response area
    ])
    .split(area);

    // Title
    let title = Paragraph::new(Span::styled(
        "Tutor",
        Style::default()
            .fg(TEXT_LIGHT_MUTED)
            .add_modifier(Modifier::BOLD),
    ));
    f.render_widget(title, sections[0]);

    // Response area
    render_response(f, sections[1], state);
}

pub fn render_response(f: &mut Frame, area: Rect, state: &TutorState) {
    render_response_with_hint(f, area, state, "Tutor recommendations appear here automatically");
}

pub fn render_response_with_hint(f: &mut Frame, area: Rect, state: &TutorState, hint: &str) {
    if state.pending {
        // Show thinking animation
        let elapsed = state
            .pending_since
            .map(|t| t.elapsed())
            .unwrap_or_default();
        let symbols = throbber_widgets_tui::BRAILLE_SIX_DOUBLE.symbols;
        let ms = elapsed.as_millis() as usize;
        let idx = (ms / 100) % symbols.len();
        let symbol = symbols[idx];

        let line = Line::from(vec![
            Span::styled(
                format!("{} ", symbol),
                Style::default()
                    .fg(ACCENT_TEAL)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Thinking...",
                Style::default()
                    .fg(ACCENT_TEAL)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        let para = Paragraph::new(line);
        f.render_widget(para, area);
    } else if !state.response.is_empty() {
        let lines: Vec<Line> = state
            .response
            .lines()
            .map(|l| Line::from(parse_styled_line(l)))
            .collect();
        let para = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((state.scroll, 0));
        f.render_widget(para, area);
    } else {
        let hint_para = Paragraph::new(Span::styled(
            hint.to_string(),
            Style::default().fg(TEXT_LIGHT_DISABLED),
        ));
        f.render_widget(hint_para, area);
    }
}

/// A text segment with an optional style modifier (bold/italic) from markdown parsing.
struct StyledSegment {
    text: String,
    modifier: Modifier,
}

/// Parse markdown-style **bold** and *italic* from a line of text into segments.
fn parse_markdown_segments(line: &str) -> Vec<StyledSegment> {
    let mut segments = Vec::new();
    let mut buf = String::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Check for ** (bold)
        if i + 1 < len && chars[i] == '*' && chars[i + 1] == '*' {
            // Flush plain buffer
            if !buf.is_empty() {
                segments.push(StyledSegment {
                    text: buf.clone(),
                    modifier: Modifier::empty(),
                });
                buf.clear();
            }
            // Find closing **
            i += 2;
            let mut bold_text = String::new();
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '*') {
                bold_text.push(chars[i]);
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip closing **
            }
            if !bold_text.is_empty() {
                segments.push(StyledSegment {
                    text: bold_text,
                    modifier: Modifier::BOLD,
                });
            }
            continue;
        }

        // Check for single * (italic) — but not **
        if chars[i] == '*' && (i + 1 >= len || chars[i + 1] != '*') {
            // Flush plain buffer
            if !buf.is_empty() {
                segments.push(StyledSegment {
                    text: buf.clone(),
                    modifier: Modifier::empty(),
                });
                buf.clear();
            }
            // Find closing *
            i += 1;
            let mut italic_text = String::new();
            while i < len && chars[i] != '*' {
                italic_text.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1; // skip closing *
            }
            if !italic_text.is_empty() {
                segments.push(StyledSegment {
                    text: italic_text,
                    modifier: Modifier::ITALIC,
                });
            }
            continue;
        }

        buf.push(chars[i]);
        i += 1;
    }

    if !buf.is_empty() {
        segments.push(StyledSegment {
            text: buf,
            modifier: Modifier::empty(),
        });
    }

    segments
}

/// Check if a character is a valid card rank character.
fn is_rank_char(c: char) -> bool {
    matches!(c, 'A' | 'K' | 'Q' | 'J' | 'T' | '2'..='9')
}

/// Map suit letter to (symbol, is_red).
fn suit_info(c: char) -> Option<(&'static str, bool)> {
    match c {
        'S' => Some(("♠", false)),
        'H' => Some(("❤", true)),
        'D' => Some(("♦", true)),
        'C' => Some(("♣", false)),
        _ => None,
    }
}

/// Parse a line: first extract markdown bold/italic, then convert card mentions
/// within each segment.
pub fn parse_styled_line(line: &str) -> Vec<Span<'static>> {
    let segments = parse_markdown_segments(line);
    let mut spans = Vec::new();

    for seg in segments {
        let base_style = Style::default().fg(TEXT_LIGHT).add_modifier(seg.modifier);
        parse_card_mentions_styled(&seg.text, base_style, &mut spans);
    }

    if spans.is_empty() {
        spans.push(Span::styled(String::new(), Style::default().fg(TEXT_LIGHT)));
    }

    spans
}

/// Parse card mentions in text, applying `base_style` to normal text and
/// preserving the modifier on rank characters. Suit symbols get their own color
/// but inherit the modifier.
fn parse_card_mentions_styled(text: &str, base_style: Style, spans: &mut Vec<Span<'static>>) {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut buf = String::new();
    let mut i = 0;

    while i < len {
        if i + 1 < len && is_rank_char(chars[i]) {
            if let Some((symbol, is_red)) = suit_info(chars[i + 1]) {
                let before_ok = i == 0 || !chars[i - 1].is_alphanumeric();
                let after_ok = i + 2 >= len || !chars[i + 2].is_alphanumeric();
                if before_ok && after_ok {
                    if !buf.is_empty() {
                        spans.push(Span::styled(buf.clone(), base_style));
                        buf.clear();
                    }
                    spans.push(Span::styled(chars[i].to_string(), base_style));
                    let suit_color = if is_red { SUIT_RED_ON_TABLE } else { SUIT_BLACK_ON_TABLE };
                    spans.push(Span::styled(
                        symbol.to_string(),
                        Style::default()
                            .fg(suit_color)
                            .add_modifier(base_style.add_modifier),
                    ));
                    i += 2;
                    continue;
                }
            }
        }
        buf.push(chars[i]);
        i += 1;
    }

    if !buf.is_empty() {
        spans.push(Span::styled(buf, base_style));
    }
}
