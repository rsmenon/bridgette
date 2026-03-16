pub mod bid_selector;
pub mod bidding_panel;
pub mod board;
pub mod controls;
pub mod dialog;
pub mod library;
pub mod palette;
pub mod probability_grid;
pub mod score_panel;
pub mod trick_history;
pub mod review;
pub mod tutor;

use std::time::Instant;

use crate::engine::game::Game;
use crate::engine::inference::CardProbabilities;
use crate::types::Seat;

pub use self::tutor::TutorState;

/// State for the Monte Carlo probability inference display.
#[allow(dead_code)]
pub struct InferenceState {
    pub probabilities: Option<CardProbabilities>,
    pub fingerprint: usize,
    pub pending: bool,
}

/// Agent backend info for display in player boxes.
pub struct AgentInfo {
    /// Model name per seat: [North, East, West]
    pub seat_models: [String; 3],
}

impl AgentInfo {
    pub fn model_for(&self, seat: Seat) -> &str {
        match seat {
            Seat::North => &self.seat_models[0],
            Seat::East => &self.seat_models[1],
            Seat::West => &self.seat_models[2],
            Seat::South => "",
        }
    }
}

pub struct AppState {
    pub game: Game,
    pub selected_card_index: Option<usize>,
    pub selected_bid_index: usize,
    pub status_message: Option<(String, u16)>,
    pub trick_scroll: usize,
    pub show_help: bool,
    pub agent_thinking: Option<(Seat, Instant)>,
    pub agent_info: AgentInfo,
    pub game_started_at: String,
    pub game_ended_at: Option<String>,
    pub bidding_system: String,
    /// Agent errors to display in the bottom-right panel.
    pub agent_errors: Vec<String>,
    /// Tutor panel state (Some when tutor is active).
    pub tutor: Option<TutorState>,
    /// Monte Carlo inference state.
    pub inference: Option<InferenceState>,
    /// Whether to show probability grids (toggled with M).
    pub show_probabilities: bool,
}

impl AppState {
    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_message = Some((msg.into(), 90)); // ~3 seconds at 30fps
    }

    pub fn tick_status(&mut self) {
        if let Some((_, ref mut ticks)) = self.status_message {
            if *ticks == 0 {
                self.status_message = None;
            } else {
                *ticks -= 1;
            }
        }
    }
}
