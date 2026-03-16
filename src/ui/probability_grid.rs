use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::engine::card::{Rank, Suit};
use crate::engine::inference::{CardProbabilities, Confidence};
use crate::types::Seat;

use super::palette::*;

const CHAR_PROB: &str = "■";
const CHAR_IMPOSSIBLE: &str = "·";
const CHAR_PLAYED: &str = "x";

/// Honor ranks displayed high-to-low.
const HONOR_RANKS: [Rank; 4] = [Rank::Ace, Rank::King, Rank::Queen, Rank::Jack];

/// Suits displayed in bridge order: Spades, Hearts, Diamonds, Clubs.
const DISPLAY_SUITS: [Suit; 4] = [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs];

/// Render the probability display for a hidden hand: honor grid + distribution bars + range + confidence.
pub fn render_probability_grid(
    f: &mut Frame,
    area: Rect,
    seat: Seat,
    probs: &CardProbabilities,
    bg: ratatui::style::Color,
) {
    let midpoint = 1.0 / probs.unknown_seat_count.max(1) as f32;

    let mut lines: Vec<Line> = Vec::with_capacity(6);

    let sep_style = Style::default().fg(TEXT_DARK_DISABLED).bg(bg);
    let label_col = 13usize; // distribution bar width
    let range_col = 4usize;  // width for range labels (e.g. "0-13")
    let conf_col = 4usize;   // width for confidence label
    let honor_cols = HONOR_RANKS.len() * 2; // "A K Q J " = 8 chars

    // Header row: " │A K Q J│Distribution  0-13│Conf."
    let hdr = Style::default().fg(TEXT_DARK_MUTED).bg(bg);
    let mut header_spans: Vec<Span> = vec![
        Span::styled(" ", hdr),
        Span::styled("│", sep_style),
    ];
    for rank in &HONOR_RANKS {
        header_spans.push(Span::styled(format!("{} ", rank.short()), hdr));
    }
    header_spans.push(Span::styled("│", sep_style));
    let dist_label = "Distribution";
    header_spans.push(Span::styled(dist_label.to_string(), hdr));
    let dist_pad = label_col.saturating_sub(dist_label.len());
    if dist_pad > 0 {
        header_spans.push(Span::styled(" ".repeat(dist_pad), Style::default().bg(bg)));
    }
    // Range header (no leading vline)
    let range_hdr = " ".repeat(range_col);
    header_spans.push(Span::styled(range_hdr, hdr));
    header_spans.push(Span::styled("│", sep_style));
    // Confidence header
    let conf_hdr = format!("{:<width$}", "Conf", width = conf_col);
    header_spans.push(Span::styled(conf_hdr, hdr));
    lines.push(Line::from(header_spans));

    // Horizontal separator: "─┼────────┼─────────────────┼────"
    let sep_line = format!(
        "─┼{}┼{}{}┼{}",
        "─".repeat(honor_cols),
        "─".repeat(label_col),
        "─".repeat(range_col),
        "─".repeat(conf_col),
    );
    lines.push(Line::from(Span::styled(sep_line, sep_style)));

    // Suit rows: honor cells + distribution bar + range + confidence
    for &suit in &DISPLAY_SUITS {
        let suit_color = if suit.is_red() { SUIT_RED } else { SUIT_BLACK };
        let mut spans: Vec<Span> = vec![
            Span::styled(
                suit.symbol().to_string(),
                Style::default().fg(suit_color).bg(bg).add_modifier(Modifier::BOLD),
            ),
            Span::styled("│", sep_style),
        ];

        // Honor cells (A K Q J)
        for &rank in &HONOR_RANKS {
            let p = probs.prob(seat, suit, rank);
            let is_played = probs.played_cards.iter().any(|(_, c)| {
                c.suit == suit && c.rank == rank
            });

            let (ch, style) = if is_played {
                (CHAR_PLAYED, Style::default().fg(TEXT_DARK_DISABLED).bg(bg).add_modifier(Modifier::DIM))
            } else {
                cell_style(p, bg, midpoint)
            };

            spans.push(Span::styled(format!("{} ", ch), style));
        }

        // Separator between honor grid and distribution bar
        spans.push(Span::styled("│", sep_style));

        // Distribution bar
        let expected = probs.expected_suit_length(seat, suit);
        let (range_min, range_max) = probs.suit_length_range(seat, suit);

        let bar_len = ((expected / 13.0) * label_col as f32).round().max(0.0).min(label_col as f32) as usize;

        let bar_color = ratatui::style::Color::Rgb(180, 175, 165);
        let bar_str: String = "█".repeat(bar_len);
        spans.push(Span::styled(
            bar_str,
            Style::default().fg(bar_color).bg(bg),
        ));

        // Pad to the fixed label column
        let pad_to_label = label_col.saturating_sub(bar_len);
        if pad_to_label > 0 {
            spans.push(Span::styled(
                " ".repeat(pad_to_label),
                Style::default().bg(bg),
            ));
        }

        // Range label (no leading vline, pad to range_col)
        let range_label = if range_min == range_max {
            format!("{:<width$}", range_min, width = range_col)
        } else {
            format!("{:<width$}", format!("{}-{}", range_min, range_max), width = range_col)
        };
        spans.push(Span::styled(
            range_label,
            Style::default().fg(TEXT_DARK_MUTED).bg(bg),
        ));

        // Separator + per-suit confidence
        spans.push(Span::styled("│", sep_style));
        let suit_conf = probs.suit_confidence(seat, suit);
        let (conf_label, conf_color) = match suit_conf {
            Confidence::High => ("High", ACCENT_GREEN),
            Confidence::Med => ("Med", ratatui::style::Color::Rgb(200, 170, 50)),
            Confidence::Low => ("Low", ratatui::style::Color::Rgb(200, 80, 60)),
        };
        spans.push(Span::styled(
            format!("{:<width$}", conf_label, width = conf_col),
            Style::default().fg(conf_color).bg(bg),
        ));

        lines.push(Line::from(spans));
    }

    let para = Paragraph::new(lines);
    f.render_widget(para, area);
}

/// Determine the character and style for a probability value.
fn cell_style(p: f32, bg: ratatui::style::Color, midpoint: f32) -> (&'static str, Style) {
    if p <= 0.0 {
        (CHAR_IMPOSSIBLE, Style::default().fg(TEXT_DARK_DISABLED).bg(bg))
    } else {
        let color = prob_color(p, midpoint);
        (CHAR_PROB, Style::default().fg(color).bg(bg))
    }
}

/// RGB lerp for probability coloring.
/// Below neutral: lightest gray (0%) → darker gray (midpoint).
/// Above neutral: gray (midpoint) → green (100%).
pub fn prob_color(p: f32, midpoint: f32) -> ratatui::style::Color {
    let p = p.clamp(0.0, 1.0);

    // 0%: lightest gray — nearly invisible (matches TEXT_DARK_DISABLED)
    let (r0, g0, b0) = (200u8, 195u8, 185u8);
    // Neutral (midpoint): medium gray (between TEXT_DARK_MUTED and TEXT_DARK_DISABLED)
    let (rn, gn, bn) = (130u8, 125u8, 115u8);
    // 100%: dark green (from ACCENT_GREEN family)
    let (r1, g1, b1) = (40u8, 140u8, 75u8);

    if p <= midpoint {
        // Lerp from lightest gray (0%) → medium gray (midpoint)
        let t = p / midpoint; // 0 at 0%, 1 at neutral
        let r = lerp_u8(r0, rn, t);
        let g = lerp_u8(g0, gn, t);
        let b = lerp_u8(b0, bn, t);
        ratatui::style::Color::Rgb(r, g, b)
    } else {
        // Lerp from medium gray (midpoint) → dark green (100%)
        let t = (p - midpoint) / (1.0 - midpoint); // 0 at neutral, 1 at 100%
        let t = t.powf(0.7); // gentle amplification near neutral
        let r = lerp_u8(rn, r1, t);
        let g = lerp_u8(gn, g1, t);
        let b = lerp_u8(bn, b1, t);
        ratatui::style::Color::Rgb(r, g, b)
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let a = a as f32;
    let b = b as f32;
    (a + (b - a) * t).round() as u8
}
