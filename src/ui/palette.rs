//! Bridge TUI color palette — warm paper-on-gray aesthetic with truecolor RGB.

#![allow(dead_code)]

use ratatui::style::Color;

// ── Frame (app background, title bar, controls bar) ──────────────────
pub const BG_FRAME: Color = Color::Rgb(42, 40, 38);
pub const BG_FRAME_ALT: Color = Color::Rgb(52, 50, 47);

// ── Content surfaces (player boxes, right panel) ─────────────────────
pub const BG_CONTENT: Color = Color::Rgb(240, 235, 220);
pub const BG_SELECTED: Color = Color::Rgb(210, 160, 30);
pub const BG_SELECTED_BLUE: Color = Color::Rgb(70, 100, 140);

// ── Center table ─────────────────────────────────────────────────────
pub const BG_TABLE: Color = Color::Rgb(25, 82, 50);
pub const BG_TABLE_EDGE: Color = Color::Rgb(20, 62, 38);

// ── Text on light backgrounds (content areas) ────────────────────────
pub const TEXT_DARK: Color = Color::Rgb(30, 28, 26);
pub const TEXT_DARK_MUTED: Color = Color::Rgb(110, 105, 95);
pub const TEXT_DARK_DISABLED: Color = Color::Rgb(180, 175, 165);

// ── Text on dark backgrounds (frame, table, popups) ──────────────────
pub const TEXT_LIGHT: Color = Color::Rgb(210, 205, 190);
pub const TEXT_LIGHT_MUTED: Color = Color::Rgb(160, 156, 148);
pub const TEXT_LIGHT_DISABLED: Color = Color::Rgb(80, 78, 72);

// ── Suit colors ──────────────────────────────────────────────────────
pub const SUIT_BLACK: Color = Color::Rgb(20, 18, 16);
pub const SUIT_RED: Color = Color::Rgb(185, 40, 35);
pub const SUIT_BLACK_ON_TABLE: Color = Color::Rgb(220, 215, 200);
pub const SUIT_RED_ON_TABLE: Color = Color::Rgb(240, 90, 75);
// ── Accent colors ────────────────────────────────────────────────────
pub const ACCENT_GOLD: Color = Color::Rgb(210, 170, 50);
pub const ACCENT_TEAL: Color = Color::Rgb(100, 135, 185);
pub const ACCENT_GREEN: Color = Color::Rgb(50, 160, 90);
pub const ACCENT_RED: Color = Color::Rgb(210, 60, 45);
pub const ACCENT_BLUE: Color = Color::Rgb(80, 170, 160);
pub const ACCENT_SAGE: Color = Color::Rgb(130, 160, 120);
pub const ACCENT_MUTED_BLUE: Color = Color::Rgb(110, 140, 180);

// ── Active player content tint ───────────────────────────────────────
pub const BG_CONTENT_ACTIVE: Color = Color::Rgb(248, 243, 228);

// ── Controls bar ─────────────────────────────────────────────────────
pub const BG_CONTROLS: Color = Color::Rgb(32, 30, 28);

// ── Borders ──────────────────────────────────────────────────────────
pub const BORDER_DARK: Color = Color::Rgb(75, 72, 66);
pub const BORDER_LIGHT: Color = Color::Rgb(200, 195, 180);
pub const BORDER_ACTIVE: Color = Color::Rgb(185, 150, 50);
