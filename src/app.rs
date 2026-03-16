use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::symbols::border;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap};
use ratatui::Frame;

use crate::agent::tutor::TutorController;
use crate::agent::{AgentAction, AgentController};
use crate::config::{self, GameStatus, SavedGame, Settings};
use crate::engine::bid_constraints::{self, HandConstraints};
use crate::engine::bidding::Bid;
use crate::engine::card::{Card, Rank};
use crate::engine::game::Game;
use crate::engine::inference::{self, CardProbabilities};
use crate::types::{Phase, Seat, Vulnerability};
use crate::ui::bid_selector::{bid_at_index, render_bid_selector, BID_GRID_SIZE};
use crate::ui::board::render_board;
use crate::ui::controls::render_controls;
use crate::ui::dialog::{render_confirm_leave, render_confirm_quit};
use crate::ui::library::{render_library, LibraryState};
use crate::ui::palette::*;
use crate::ui::review::{build_review_question, pre_step_game_and_seat, render_review_panel, ReviewState};
use crate::ui::trick_history::render_trick_history;
use crate::ui::tutor::render_tutor_pane;
use crate::ui::{AgentInfo, AppState, InferenceState, TutorState};

const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Game,
    Library,
    Review,
    ConfirmQuit,
    ConfirmLeaveGame,
}

pub struct App {
    pub state: AppState,
    pub should_quit: bool,
    pub agent: AgentController,
    #[allow(dead_code)]
    pub settings: Settings,
    pub game_saved: bool,
    pub mode: AppMode,
    pub library_state: Option<LibraryState>,
    /// The id of the in-progress save file for the current game, if any.
    pub current_save_id: Option<String>,
    /// Whether a game is actively being played (false at startup until [N] or resume).
    pub game_active: bool,
    /// Tutor LLM controller (None if tutor is unavailable).
    pub tutor_controller: Option<TutorController>,
    /// Review mode state (Some when reviewing a completed game).
    pub review_state: Option<ReviewState>,
    /// Chicago-style deal number (0-indexed, increments each new game).
    pub deal_number: u32,
    /// Channel for receiving inference results from background threads.
    inference_rx: Receiver<CardProbabilities>,
    inference_tx: Sender<CardProbabilities>,
    /// Fingerprint of the last dispatched inference computation.
    inference_fingerprint: Option<usize>,
    /// Whether an inference computation is currently running.
    inference_pending: bool,
}

fn format_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Build the common snapshot fields shared by save_game_record and save_in_progress.
fn build_game_snapshot(
    game: &Game,
) -> (
    config::HandsRecord,
    Vec<config::BidRecord>,
    Option<config::ContractRecord>,
    Vec<config::TrickRecord>,
) {
    use crate::agent::prompt::{bid_ascii, card_ascii};
    use crate::config::{BidRecord, ContractRecord, HandsRecord, TrickCardRecord, TrickRecord};
    use crate::engine::bidding::BidSuit;

    let hand_cards = |seat: Seat| -> Vec<String> {
        game.dealt_hands[seat.index()]
            .cards()
            .iter()
            .rev()
            .map(card_ascii)
            .collect()
    };

    let hands = HandsRecord {
        north: hand_cards(Seat::North),
        east: hand_cards(Seat::East),
        south: hand_cards(Seat::South),
        west: hand_cards(Seat::West),
    };

    let bidding: Vec<BidRecord> = game
        .auction
        .bids
        .iter()
        .map(|(seat, bid)| BidRecord {
            seat: format!("{}", seat),
            bid: bid_ascii(bid),
        })
        .collect();

    let contract = game.contract.map(|c| {
        let suit_name = match c.suit {
            BidSuit::Clubs => "Clubs",
            BidSuit::Diamonds => "Diamonds",
            BidSuit::Hearts => "Hearts",
            BidSuit::Spades => "Spades",
            BidSuit::NoTrump => "NoTrump",
        };
        ContractRecord {
            level: c.level,
            suit: suit_name.to_string(),
            doubled: c.doubled,
            redoubled: c.redoubled,
            declarer: format!("{}", c.declarer),
        }
    });

    let trump = game.contract.as_ref().and_then(|c| {
        if c.suit == BidSuit::NoTrump {
            None
        } else {
            Some(c.suit)
        }
    });

    let play: Vec<TrickRecord> = if let Some(ref ps) = game.play_state {
        ps.tricks
            .iter()
            .enumerate()
            .map(|(i, trick)| {
                let cards: Vec<TrickCardRecord> = trick
                    .cards
                    .iter()
                    .map(|(s, c)| TrickCardRecord {
                        seat: format!("{}", s),
                        card: card_ascii(c),
                    })
                    .collect();
                let winner = trick
                    .winner(trump)
                    .expect("completed trick must have a winner");
                TrickRecord {
                    trick_number: i + 1,
                    cards,
                    winner: format!("{}", winner),
                }
            })
            .collect()
    } else {
        vec![]
    };

    (hands, bidding, contract, play)
}

impl App {
    pub fn new(
        agent: AgentController,
        settings: Settings,
        tutor_controller: Option<TutorController>,
    ) -> Self {
        let agent_info = AgentInfo {
            seat_models: [
                settings.agents.north.model.clone(),
                settings.agents.east.model.clone(),
                settings.agents.west.model.clone(),
            ],
        };
        let (inference_tx, inference_rx) = mpsc::channel();
        Self {
            state: AppState {
                game: Game::new(Seat::random(), Vulnerability::None),
                selected_card_index: Some(0),
                selected_bid_index: 35,
                status_message: None,
                trick_scroll: 0,
                show_help: false,
                agent_thinking: None,
                agent_info,
                game_started_at: format_timestamp(),
                game_ended_at: None,
                bidding_system: "SAYC".to_string(),
                agent_errors: Vec::new(),
                tutor: None,
                inference: None,
                show_probabilities: false,
            },
            should_quit: false,
            agent,
            settings,
            game_saved: false,
            mode: AppMode::Game,
            library_state: None,
            game_active: false,
            current_save_id: None,
            tutor_controller,
            review_state: None,
            deal_number: 0,
            inference_rx,
            inference_tx,
            inference_fingerprint: None,
            inference_pending: false,
        }
    }

    pub fn new_game(&mut self) {
        // Clean up old in-progress save if we're starting fresh
        if let Some(ref id) = self.current_save_id {
            let _ = config::delete_game(id);
        }
        let dealer = Seat::random();
        let vulnerability = Vulnerability::chicago(self.deal_number, dealer);
        self.state.game = Game::new(dealer, vulnerability);
        self.deal_number += 1;
        self.state.selected_card_index = Some(0);
        self.state.selected_bid_index = 35;
        self.state.status_message = None;
        self.state.trick_scroll = 0;
        self.state.show_help = false;
        self.state.agent_thinking = None;
        self.state.agent_errors = Vec::new();
        self.state.tutor = None;
        self.state.inference = None;
        self.state.game_started_at = format_timestamp();
        self.state.game_ended_at = None;
        self.game_saved = false;
        self.current_save_id = None;
        self.mode = AppMode::Game;
        self.game_active = true;
        self.agent.reset();
        if let Some(ref mut tc) = self.tutor_controller {
            tc.reset();
        }
        self.inference_fingerprint = None;
        self.inference_pending = false;
        // Drain stale inference results
        while self.inference_rx.try_recv().is_ok() {}
        self.state.set_status("New game started");
    }

    /// Save a record of the finished game to ~/.config/bridgette/data/.
    fn save_game_record(&mut self) {
        use crate::config::{ResultRecord, ScoreBreakdown};
        use crate::engine::scoring::Score;

        if self.game_saved || self.state.game.phase != Phase::Finished {
            return;
        }
        self.game_saved = true;

        if self.state.game_ended_at.is_none() {
            self.state.game_ended_at = Some(format_timestamp());
        }

        let game = &self.state.game;
        let now = chrono::Local::now();
        let id = self
            .current_save_id
            .take()
            .unwrap_or_else(|| now.format("game_%Y%m%d_%H%M%S").to_string());
        let timestamp = now.format("%Y-%m-%dT%H:%M:%S%:z").to_string();

        let (hands, bidding, contract, play) = build_game_snapshot(game);

        let (ns_tricks, ew_tricks) = game
            .play_state
            .as_ref()
            .map(|p| (p.ns_tricks, p.ew_tricks))
            .unwrap_or((0, 0));

        let (score_ns, breakdown) = match game.score {
            Some(Score::Made {
                contract_points,
                overtrick_points,
                game_bonus,
                slam_bonus,
                insult_bonus,
                total,
            }) => {
                let declarer_is_ns = game
                    .contract
                    .as_ref()
                    .is_some_and(|c| c.declarer.is_ns());
                let ns_score = if declarer_is_ns { total } else { -total };
                (
                    ns_score,
                    ScoreBreakdown {
                        contract_points,
                        overtrick_points,
                        game_bonus,
                        slam_bonus,
                        insult_bonus,
                    },
                )
            }
            Some(Score::Defeated { penalty, .. }) => {
                let declarer_is_ns = game
                    .contract
                    .as_ref()
                    .is_some_and(|c| c.declarer.is_ns());
                let ns_score = if declarer_is_ns { penalty } else { -penalty };
                (
                    ns_score,
                    ScoreBreakdown {
                        contract_points: 0,
                        overtrick_points: 0,
                        game_bonus: 0,
                        slam_bonus: 0,
                        insult_bonus: 0,
                    },
                )
            }
            _ => (
                0,
                ScoreBreakdown {
                    contract_points: 0,
                    overtrick_points: 0,
                    game_bonus: 0,
                    slam_bonus: 0,
                    insult_bonus: 0,
                },
            ),
        };

        let result = ResultRecord {
            tricks_won_ns: ns_tricks,
            tricks_won_ew: ew_tricks,
            score_ns,
            score_ew: -score_ns,
            breakdown,
        };

        let saved = SavedGame {
            id,
            timestamp,
            status: GameStatus::Completed,
            favorite: false,
            dealer: format!("{}", game.dealer),
            hands,
            bidding,
            contract,
            play,
            current_trick: None,
            result: Some(result),
            game_state: None,
            ended_at: self.state.game_ended_at.clone(),
            vulnerability: game.vulnerability,
        };

        if let Err(e) = saved.save() {
            eprintln!("Failed to save game record: {}", e);
        }
    }

    /// Save the current in-progress game for later resume.
    fn save_in_progress(&mut self) {
        use crate::agent::prompt::card_ascii;
        use crate::config::TrickCardRecord;

        let game = &self.state.game;
        if game.phase == Phase::Finished {
            return;
        }

        let now = chrono::Local::now();
        let id = self
            .current_save_id
            .clone()
            .unwrap_or_else(|| now.format("game_%Y%m%d_%H%M%S").to_string());
        let timestamp = now.format("%Y-%m-%dT%H:%M:%S%:z").to_string();

        let (hands, bidding, contract, play) = build_game_snapshot(game);

        let current_trick = game.play_state.as_ref().and_then(|ps| {
            if ps.current_trick.cards.is_empty() {
                None
            } else {
                Some(
                    ps.current_trick
                        .cards
                        .iter()
                        .map(|(s, c)| TrickCardRecord {
                            seat: format!("{}", s),
                            card: card_ascii(c),
                        })
                        .collect(),
                )
            }
        });

        let game_state = serde_json::to_string(game).ok();

        let saved = SavedGame {
            id: id.clone(),
            timestamp,
            status: GameStatus::InProgress,
            favorite: false,
            dealer: format!("{}", game.dealer),
            hands,
            bidding,
            contract,
            play,
            current_trick,
            result: None,
            game_state,
            ended_at: None,
            vulnerability: game.vulnerability,
        };

        if let Err(e) = saved.save() {
            eprintln!("Failed to save in-progress game: {}", e);
        }
        self.current_save_id = Some(id);
    }

    /// Resume a saved in-progress game.
    fn resume_game(&mut self, saved: &SavedGame) {
        if saved.status != GameStatus::InProgress {
            return;
        }
        if let Some(ref state_json) = saved.game_state {
            match serde_json::from_str::<Game>(state_json) {
                Ok(game) => {
                    self.state.game = game;
                    self.state.selected_card_index = Some(0);
                    self.state.selected_bid_index = 35;
                    self.state.status_message = None;
                    self.state.trick_scroll = 0;
                    self.state.show_help = false;
                    self.state.agent_thinking = None;
                    self.state.tutor = None;
                    self.state.inference = None;
                    self.state.game_started_at = format_timestamp();
                    self.state.game_ended_at = None;
                    self.game_saved = false;
                    self.current_save_id = Some(saved.id.clone());
                    self.mode = AppMode::Game;
                    self.game_active = true;
                    self.agent.reset();
                    if let Some(ref mut tc) = self.tutor_controller {
                        tc.reset();
                    }
                    self.inference_fingerprint = None;
                    self.inference_pending = false;
                    while self.inference_rx.try_recv().is_ok() {}
                    self.state.set_status("Game resumed");
                }
                Err(e) => {
                    self.state
                        .set_status(format!("Failed to resume: {}", e));
                }
            }
        } else {
            self.state.set_status("No game state to resume");
        }
    }

    fn enter_library(&mut self) {
        let games = config::load_all_games();
        self.library_state = Some(LibraryState::new(games));
        self.mode = AppMode::Library;
    }

    fn enter_review(&mut self, saved: &SavedGame) {
        self.review_state = Some(ReviewState::new(saved.clone()));
        self.mode = AppMode::Review;
    }

    /// Check if it's an AI agent's turn. Returns the seat that should act.
    fn is_agent_turn(&self) -> Option<Seat> {
        let game = &self.state.game;
        if game.phase == Phase::Finished {
            return None;
        }

        let current = game.current_seat();

        // During bidding, only South is human
        if game.phase == Phase::Bidding {
            if current == Seat::South {
                return None;
            }
            return Some(current);
        }

        // During play: South is always human. When N/S declares, human also plays North.
        if let Some(contract) = &game.contract {
            // South is always human-controlled
            if current == Seat::South {
                return None;
            }
            if contract.declarer.is_ns() {
                // N/S declaring — human plays for both declarer and dummy
                if current == contract.declarer || current == contract.dummy {
                    return None;
                }
                return Some(current);
            } else {
                // E/W declaring — agent controls E/W (declarer plays from dummy)
                if current == contract.dummy {
                    return Some(contract.declarer);
                }
                return Some(current);
            }
        }

        Some(current)
    }

    /// Called each frame to manage agent dispatch and result processing.
    pub fn tick_agents(&mut self) {
        // Only tick agents while in Game mode with an active game
        if self.mode != AppMode::Game || !self.game_active {
            return;
        }

        // Check for completed agent action
        if let Some(result) = self.agent.try_recv() {
            self.state.agent_thinking = None;
            // Surface any agent errors to the UI; clear on clean run
            if !result.errors.is_empty() {
                self.state.agent_errors.extend(result.errors);
                // Keep only the most recent errors to prevent unbounded growth
                const MAX_ERRORS: usize = 50;
                if self.state.agent_errors.len() > MAX_ERRORS {
                    let drain = self.state.agent_errors.len() - MAX_ERRORS;
                    self.state.agent_errors.drain(..drain);
                }
            } else {
                self.state.agent_errors.clear();
            }
            // Clear any previous error/status when agent acts successfully
            self.state.status_message = None;
            match result.action {
                AgentAction::Bid(bid) => {
                    let bidder = self.state.game.current_seat();
                    let bid_display = format!("{}", bid);
                    if let Err(e) = self.state.game.place_bid(bid) {
                        self.state.set_status(format!("Agent bid error: {}", e));
                        return;
                    }
                    if self.state.game.phase == Phase::Playing {
                        self.state.selected_card_index = Some(0);
                        if let Some(contract) = &self.state.game.contract {
                            self.state.set_status(format!(
                                "Contract: {} — {} leads",
                                contract,
                                contract.declarer.next()
                            ));
                        }
                    } else if self.state.game.phase == Phase::Finished {
                        self.state.set_status("Passed out — no play");
                    } else {
                        self.state.set_status(format!("{} bid {}", bidder, bid_display));
                    }
                }
                AgentAction::PlayCard(card) => {
                    // If AI is playing from dummy (South), notify human
                    let playing_as_dummy = self.state.game.contract.as_ref().is_some_and(|c| {
                        c.dummy == Seat::South
                            && self.state.game.current_seat() == Seat::South
                    });
                    self.try_play_card(card);
                    if playing_as_dummy {
                        self.state.set_status(format!(
                            "Declarer plays {} from dummy",
                            card
                        ));
                    }
                }
            }
        }

        // Save game record if just finished
        if self.state.game.phase == Phase::Finished && !self.game_saved {
            if self.state.game_ended_at.is_none() {
                self.state.game_ended_at = Some(format_timestamp());
            }
            self.save_game_record();
        }

        // Deal cards on first tick (so the initial frame shows empty hands)
        if self.state.game.hands.iter().all(|h| h.is_empty()) && self.state.game.phase == Phase::Bidding {
            self.state.game.deal_cards();
        }

        // Check if we should dispatch a new agent request
        if !self.agent.pending {
            if let Some(seat) = self.is_agent_turn() {
                self.state.agent_thinking = Some((self.state.game.current_seat(), Instant::now()));
                self.agent.dispatch(&self.state.game, seat);
            }
        }
    }

    /// Called each frame to poll inference results and dispatch new computations.
    pub fn tick_inference(&mut self) {
        if self.mode != AppMode::Game || !self.game_active {
            return;
        }
        if !self.state.show_probabilities {
            return;
        }
        if self.state.game.phase == Phase::Finished {
            return;
        }

        // Poll for completed inference result
        if let Ok(probs) = self.inference_rx.try_recv() {
            self.inference_pending = false;
            let fp = self.inference_fingerprint.unwrap_or(0);
            self.state.inference = Some(InferenceState {
                probabilities: Some(probs),
                fingerprint: fp,
                pending: false,
            });
        }

        // Compute fingerprint: bids + cards played
        let cards_played = self.state.game.play_state.as_ref()
            .map(|p| p.tricks.iter().map(|t| t.cards.len()).sum::<usize>() + p.current_trick.cards.len())
            .unwrap_or(0);
        let fingerprint = self.state.game.auction.bids.len() + cards_played;

        // Only dispatch if fingerprint changed and no computation pending
        if self.inference_pending {
            return;
        }
        if self.inference_fingerprint == Some(fingerprint) {
            return;
        }
        // Don't run inference before cards are dealt
        if self.state.game.hands.iter().all(|h| h.is_empty()) {
            return;
        }

        self.inference_fingerprint = Some(fingerprint);
        self.inference_pending = true;

        // Build constraints from bidding
        let game = &self.state.game;
        let constraints: [HandConstraints; 4] = std::array::from_fn(|i| {
            let seat = Seat::ALL[i];
            bid_constraints::constraints_from_bids(seat, &game.auction.bids, game.vulnerability)
        });

        let south_hand = game.hand(Seat::South).clone();
        let (dummy_hand, dummy_seat) = if let Some(contract) = &game.contract {
            if game.dummy_revealed {
                if contract.declarer.is_ns() {
                    // N/S declaring: South is already known via south_hand.
                    // Pass North's hand as the "dummy" so inference treats it
                    // as known too — the human controls and sees both hands.
                    (Some(game.hand(Seat::North).clone()), Some(Seat::North))
                } else {
                    // E/W declaring: dummy is North (partner of declarer).
                    (Some(game.hand(contract.dummy).clone()), Some(contract.dummy))
                }
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        let tricks = game.play_state.as_ref()
            .map(|p| p.tricks.clone())
            .unwrap_or_default();
        let current_trick = game.play_state.as_ref()
            .map(|p| p.current_trick.clone())
            .unwrap_or_else(|| crate::engine::trick::Trick::new(Seat::North));

        let tx = self.inference_tx.clone();
        thread::spawn(move || {
            let result = inference::run_inference(
                &south_hand,
                dummy_hand.as_ref(),
                dummy_seat,
                &constraints,
                &tricks,
                &current_trick,
                2000,
            );
            let _ = tx.send(result);
        });
    }

    /// Called each frame to poll tutor LLM responses and auto-dispatch recommendations.
    pub fn tick_tutor(&mut self) {
        // Don't consume tutor responses while in Review mode — tick_review handles that
        if self.mode == AppMode::Review {
            return;
        }
        let has_response = if let Some(ref mut tc) = self.tutor_controller {
            tc.try_recv()
        } else {
            None
        };
        if let Some(result) = has_response {
            if let Some(ref mut tutor) = self.state.tutor {
                match result {
                    Ok(text) => {
                        tutor.response = text;
                    }
                    Err(e) => {
                        tutor.response = format!("Error: {}", e);
                    }
                }
                tutor.pending = false;
                tutor.pending_since = None;
                tutor.scroll = 0;
            }
        }

        // Auto-dispatch a recommendation when it becomes the human's turn
        let is_human_turn = self.game_active
            && self.state.game.phase != Phase::Finished
            && self.is_agent_turn().is_none();

        if is_human_turn {
            // Compute a turn fingerprint: number of bids + cards played
            let turn_id = self.state.game.auction.bids.len()
                + self
                    .state
                    .game
                    .play_state
                    .as_ref()
                    .map(|p| p.tricks.len() * 4 + p.current_trick.cards.len())
                    .unwrap_or(0);

            let should_dispatch = self
                .state
                .tutor
                .as_ref()
                .is_some_and(|t| t.last_auto_turn != Some(turn_id))
                && self
                    .tutor_controller
                    .as_ref()
                    .is_some_and(|tc| !tc.pending);

            if should_dispatch {
                if let Some(ref mut tutor) = self.state.tutor {
                    tutor.pending = true;
                    tutor.pending_since = Some(Instant::now());
                    tutor.last_auto_turn = Some(turn_id);
                }
                if let Some(ref mut tc) = self.tutor_controller {
                    tc.dispatch(&self.state.game, None);
                }
            }
        }
    }

    /// Activate the tutor panel and request a recommendation.
    fn activate_tutor(&mut self) {
        if self.tutor_controller.is_none() {
            self.state.set_status("Tutor requires an API key");
            return;
        }
        if !self.game_active {
            return;
        }

        let mut tutor = TutorState::new();
        tutor.pending = true;
        tutor.pending_since = Some(Instant::now());
        self.state.tutor = Some(tutor);

        if let Some(ref mut tc) = self.tutor_controller {
            tc.dispatch(&self.state.game, None);
        }
    }

    /// Deactivate the tutor panel.
    fn deactivate_tutor(&mut self) {
        self.state.tutor = None;
        if let Some(ref mut tc) = self.tutor_controller {
            tc.reset();
        }
    }

    /// Play a card and handle all resulting state transitions.
    fn try_play_card(&mut self, card: Card) {
        let will_complete = self
            .state
            .game
            .play_state
            .as_ref()
            .is_some_and(|p| p.current_trick.cards.len() == 3);

        match self.state.game.play_card(card) {
            Ok(()) => {
                self.state.selected_card_index = Some(0);
                if will_complete {
                    if let Some(play) = &self.state.game.play_state {
                        let winner = play.current_trick.leader;
                        self.state.set_status(format!("Trick won by {}", winner));
                    }
                }
                if self.state.game.phase == Phase::Finished {
                    self.state.set_status("Hand complete!");
                }
            }
            Err(e) => {
                self.state.set_status(format!("Cannot play {}: {}", card, e));
            }
        }
    }

    pub fn draw(&self, f: &mut Frame) {
        let area = f.area();

        // Fill entire background with dim gray
        f.render_widget(Block::default().style(Style::default().bg(BG_FRAME)), area);

        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            let msg = format!(
                "Terminal too small ({}x{})\nNeed at least {}x{}",
                area.width, area.height, MIN_WIDTH, MIN_HEIGHT
            );
            let para = Paragraph::new(msg)
                .alignment(Alignment::Center)
                .style(Style::default().fg(ACCENT_RED).add_modifier(Modifier::BOLD));
            let centered = centered_rect(40, 4, area);
            f.render_widget(para, centered);
            return;
        }

        match self.mode {
            AppMode::Library => {
                if let Some(ref lib) = self.library_state {
                    render_library(f, area, lib);
                }
            }
            AppMode::Review => {
                self.draw_review(f, area);
            }
            AppMode::Game | AppMode::ConfirmQuit | AppMode::ConfirmLeaveGame => {
                self.draw_game(f, area);

                // Render dialog overlays
                match self.mode {
                    AppMode::ConfirmQuit => render_confirm_quit(f, area),
                    AppMode::ConfirmLeaveGame => render_confirm_leave(f, area),
                    _ => {}
                }
            }
        }
    }

    fn draw_game(&self, f: &mut Frame, area: Rect) {
        let outer = Layout::vertical([
            Constraint::Min(20),  // Main content
            Constraint::Length(3), // Separator + status + controls
        ])
        .split(area);

        // Main content: board + separator + right panel
        let main = Layout::horizontal([
            Constraint::Min(40),   // Board
            Constraint::Length(1), // Separator
            Constraint::Length(38), // Right panel
        ])
        .split(outer[0]);

        render_board(f, main[0], &self.state);

        // Thin vertical separator line
        let sep_area = main[1];
        let mut sep_buf = String::new();
        for _ in 0..sep_area.height {
            sep_buf.push('│');
            sep_buf.push('\n');
        }
        f.render_widget(
            Paragraph::new(sep_buf.trim_end().to_string())
                .style(Style::default().fg(BORDER_DARK).bg(BG_FRAME)),
            sep_area,
        );

        self.render_right_panel(f, main[2]);

        render_controls(f, outer[1], &self.state);

        if self.state.show_help {
            self.render_help_popup(f, area);
        }
    }

    fn render_right_panel(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .padding(Padding::new(1, 1, 1, 0))
            .style(Style::default().bg(BG_FRAME));
        let inner = block.inner(area);
        f.render_widget(block, area);

        let tutor_active = self.state.tutor.is_some();

        // Split panel: top half for game content, bottom half for tutor (when active)
        let (game_area, tutor_area) = if tutor_active {
            let halves = Layout::vertical([
                Constraint::Ratio(1, 2),
                Constraint::Ratio(1, 2),
            ])
            .split(inner);
            (halves[0], Some(halves[1]))
        } else {
            (inner, None)
        };

        // Right sidebar: bid selector (auction) or trick history (play/finished)
        if self.game_active && self.state.game.phase == Phase::Bidding {
            let sections = Layout::vertical([
                Constraint::Length(1), // Title
                Constraint::Min(8),    // Bid selector
            ])
            .split(game_area);

            self.render_section_title(f, sections[0], "Make a Bid");
            render_bid_selector(f, sections[1], &self.state);
        } else {
            render_trick_history(f, game_area, &self.state);
        }

        // Render tutor pane if active
        if let (Some(area), Some(tutor)) = (tutor_area, &self.state.tutor) {
            render_tutor_pane(f, area, tutor);
        }
    }

    fn render_section_title(&self, f: &mut Frame, area: Rect, title: &str) {
        let para = Paragraph::new(Span::styled(
            title,
            Style::default()
                .fg(TEXT_LIGHT_MUTED)
                .add_modifier(Modifier::BOLD),
        ));
        f.render_widget(para, area);
    }


    fn render_help_popup(&self, f: &mut Frame, area: Rect) {
        let popup = centered_rect(50, 20, area);
        f.render_widget(Clear, popup);

        let phase = self.state.game.phase;
        let mut lines = vec![
            Line::from(Span::styled(
                " Keyboard Controls ",
                Style::default()
                    .fg(ACCENT_MUTED_BLUE)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        lines.push(Line::from(Span::styled(
            "Global:",
            Style::default().fg(TEXT_LIGHT).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from("  Q         Quit"));
        lines.push(Line::from("  N         New Game"));
        lines.push(Line::from("  L         Library"));
        lines.push(Line::from("  T         Toggle Tutor"));
        lines.push(Line::from("  H / ?     Toggle Help"));
        lines.push(Line::from("  Esc       Close Popup"));
        lines.push(Line::from(""));

        match phase {
            Phase::Bidding => {
                lines.push(Line::from(Span::styled(
                    "Bidding:",
                    Style::default().fg(TEXT_LIGHT).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from("  ↑↓←→      Navigate Grid"));
                lines.push(Line::from("  Enter     Place Bid"));
                lines.push(Line::from("  1-7       Jump to Level"));
                lines.push(Line::from("  C D H S N Jump to Suit"));
                lines.push(Line::from("  P         Pass"));
                lines.push(Line::from("  X         Double"));
            }
            Phase::Playing => {
                lines.push(Line::from(Span::styled(
                    "Playing:",
                    Style::default().fg(TEXT_LIGHT).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from("  ←→        Select Card"));
                lines.push(Line::from("  ↑↓        Change Suit"));
                lines.push(Line::from("  Enter     Play Card"));
                lines.push(Line::from("  A K Q J T Select by Rank"));
                lines.push(Line::from("  2-9       Select by Rank"));
            }
            Phase::Finished => {
                lines.push(Line::from(Span::styled(
                    "Finished:",
                    Style::default().fg(TEXT_LIGHT).add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from("  N         New Game"));
                lines.push(Line::from("  L         Library"));
                lines.push(Line::from("  Q         Quit"));
            }
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_set(border::ROUNDED)
            .border_style(Style::default().fg(BORDER_DARK))
            .title(" Help ")
            .style(Style::default().bg(BG_FRAME_ALT).fg(TEXT_LIGHT_MUTED));
        let para = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
        f.render_widget(para, popup);
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match self.mode {
            AppMode::Game => self.handle_game_key(key),
            AppMode::Library => self.handle_library_key(key),
            AppMode::Review => self.handle_review_key(key),
            AppMode::ConfirmQuit => self.handle_confirm_quit_key(key),
            AppMode::ConfirmLeaveGame => self.handle_confirm_leave_key(key),
        }
    }

    fn draw_review(&self, f: &mut Frame, area: Rect) {
        if let (Some(lib), Some(review)) = (&self.library_state, &self.review_state) {
            let outer = Layout::vertical([
                Constraint::Min(20),  // Main content
                Constraint::Length(1), // Controls
            ])
            .split(area);

            // Library keeps its full size; review panel fills the remaining space
            let cols = Layout::horizontal([
                Constraint::Min(75),  // Library table (unchanged)
                Constraint::Length(1), // Separator
                Constraint::Min(40),  // Review panel
            ])
            .split(outer[0]);

            render_library(f, cols[0], lib);

            // Separator
            let sep_area = cols[1];
            let mut sep_buf = String::new();
            for _ in 0..sep_area.height {
                sep_buf.push('│');
                sep_buf.push('\n');
            }
            f.render_widget(
                Paragraph::new(sep_buf.trim_end().to_string())
                    .style(Style::default().fg(BORDER_DARK).bg(BG_FRAME)),
                sep_area,
            );

            render_review_panel(f, cols[2], review);

            // Review controls footer
            self.render_review_controls(f, outer[1]);
        }
    }

    fn render_review_controls(&self, f: &mut Frame, area: Rect) {
        let controls = [
            ("←→", "Step"),
            ("T", "Tutor"),
            ("PgUp/Dn", "Scroll"),
            ("Esc", "Close"),
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
        let para = Paragraph::new(Line::from(spans)).style(Style::default().bg(BG_CONTROLS));
        f.render_widget(para, area);
    }

    fn handle_review_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Right => {
                if let Some(ref mut rs) = self.review_state {
                    rs.step_forward();
                }
            }
            KeyCode::Left => {
                if let Some(ref mut rs) = self.review_state {
                    rs.step_backward();
                }
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                self.review_auto_analyze();
            }
            KeyCode::PageUp => {
                if let Some(ref mut rs) = self.review_state {
                    rs.tutor.scroll = rs.tutor.scroll.saturating_sub(5);
                }
            }
            KeyCode::PageDown => {
                if let Some(ref mut rs) = self.review_state {
                    rs.tutor.scroll += 5;
                }
            }
            KeyCode::Esc => {
                self.review_state = None;
                self.mode = AppMode::Library;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.mode = AppMode::ConfirmQuit;
            }
            _ => {}
        }
    }

    fn review_auto_analyze(&mut self) {
        let review_data = self
            .review_state
            .as_ref()
            .and_then(|rs| {
                build_review_question(&rs.saved, rs.step)
                    .map(|q| {
                        let (game, seat) = pre_step_game_and_seat(&rs.saved, rs.step);
                        (q, game, seat)
                    })
            });

        if let (Some(rs), Some(tc)) = (&mut self.review_state, &mut self.tutor_controller) {
            if rs.tutor.pending {
                return;
            }
            if let Some((question, game, seat)) = review_data {
                tc.reset();
                tc.dispatch_review(&game, seat, question);
                rs.tutor.pending = true;
                rs.tutor.pending_since = Some(Instant::now());
            }
        }
    }

    /// Poll tutor responses while in review mode.
    pub fn tick_review(&mut self) {
        if self.mode != AppMode::Review {
            return;
        }
        let has_response = if let Some(ref mut tc) = self.tutor_controller {
            tc.try_recv()
        } else {
            None
        };
        if let Some(result) = has_response {
            if let Some(ref mut rs) = self.review_state {
                match result {
                    Ok(text) => rs.tutor.response = text,
                    Err(e) => rs.tutor.response = format!("Error: {}", e),
                }
                rs.tutor.pending = false;
                rs.tutor.pending_since = None;
                rs.tutor.scroll = 0;
            }
        }
    }

    fn handle_game_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc && self.state.show_help {
            self.state.show_help = false;
            return;
        }

        if key.code == KeyCode::Char('?') {
            self.state.show_help = !self.state.show_help;
            return;
        }

        if self.state.show_help {
            return;
        }

        // Handle tutor scroll when tutor is active but input not focused
        let tutor_active = self.state.tutor.is_some();

        // During bidding when it's the human's turn, route most keys to bidding handler.
        // Only Q/?/L/T remain global so suit letters (S, H, D, C, N) work for bid selection.
        let human_bidding = self.state.game.phase == Phase::Bidding
            && self.state.game.current_seat() == Seat::South;

        if human_bidding {
            match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.mode = AppMode::ConfirmQuit;
                }
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    self.handle_library_request();
                }
                KeyCode::Char('t') | KeyCode::Char('T') => {
                    if tutor_active {
                        self.deactivate_tutor();
                    } else {
                        self.activate_tutor();
                    }
                }
                KeyCode::Char('b') | KeyCode::Char('B') => {
                    self.state.show_probabilities = !self.state.show_probabilities;
                }
                KeyCode::Esc if tutor_active => self.deactivate_tutor(),
                _ => self.handle_bidding_key(key.code),
            }
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.mode = AppMode::ConfirmQuit;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.new_game();
            }
            KeyCode::Char('l') | KeyCode::Char('L') => {
                self.handle_library_request();
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                if tutor_active {
                    self.deactivate_tutor();
                } else {
                    self.activate_tutor();
                }
            }
            KeyCode::Char('b') | KeyCode::Char('B') => {
                self.state.show_probabilities = !self.state.show_probabilities;
            }
            KeyCode::Esc if tutor_active => self.deactivate_tutor(),
            _ => match self.state.game.phase {
                Phase::Bidding => self.handle_bidding_key(key.code),
                Phase::Playing => self.handle_playing_key(key.code),
                Phase::Finished => {}
            },
        }
    }

    /// Handle pressing L — go to library, with confirmation if game is active.
    fn handle_library_request(&mut self) {
        let game_active = self.game_active && self.state.game.phase != Phase::Finished;
        if game_active {
            self.mode = AppMode::ConfirmLeaveGame;
        } else {
            self.enter_library();
        }
    }

    fn handle_library_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = AppMode::Game;
                self.library_state = None;
            }
            KeyCode::Up => {
                if let Some(ref mut lib) = self.library_state {
                    lib.move_up();
                }
            }
            KeyCode::Down => {
                if let Some(ref mut lib) = self.library_state {
                    lib.move_down();
                }
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                if let Some(ref mut lib) = self.library_state {
                    lib.toggle_favorite();
                }
            }
            KeyCode::Char('/') => {
                if let Some(ref mut lib) = self.library_state {
                    lib.cycle_filter();
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.library_state = None;
                self.new_game();
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.mode = AppMode::ConfirmQuit;
            }
            KeyCode::Enter => {
                // Open selected game
                let selected = self
                    .library_state
                    .as_ref()
                    .and_then(|lib| lib.selected_game().cloned());
                if let Some(game) = selected {
                    if game.status == GameStatus::InProgress {
                        self.resume_game(&game);
                        self.library_state = None;
                    } else {
                        self.enter_review(&game);
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_confirm_quit_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                // Auto-save if game is in progress
                if self.state.game.phase != Phase::Finished && !self.game_saved {
                    self.save_in_progress();
                }
                self.should_quit = true;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                if self.review_state.is_some() {
                    self.mode = AppMode::Review;
                } else if self.library_state.is_some() {
                    self.mode = AppMode::Library;
                } else {
                    self.mode = AppMode::Game;
                }
            }
            _ => {}
        }
    }

    fn handle_confirm_leave_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.save_in_progress();
                self.state.set_status("Game saved");
                self.enter_library();
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                // Abandon — delete save file if exists
                if let Some(ref id) = self.current_save_id.take() {
                    let _ = config::delete_game(id);
                }
                self.enter_library();
            }
            KeyCode::Char('g') | KeyCode::Char('G') | KeyCode::Esc => {
                self.mode = AppMode::Game;
            }
            _ => {}
        }
    }

    fn handle_bidding_key(&mut self, code: KeyCode) {
        // Only allow human to bid when it's South's turn
        let current = self.state.game.current_seat();
        if current != Seat::South {
            return;
        }
        match code {
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.state.selected_bid_index = 35; // Pass
            }
            KeyCode::Char('x') | KeyCode::Char('X') => {
                self.state.selected_bid_index = 36; // Double
            }
            KeyCode::Char(c @ '1'..='7') => {
                let level = c as u8 - b'0';
                let valid_bids = self.state.game.auction.valid_bids();
                let base = (level as usize - 1) * 5;
                if let Some(col) = (0..5).find(|&si| {
                    let bid = bid_at_index(base + si);
                    bid.is_some_and(|b| valid_bids.contains(&b))
                }) {
                    self.state.selected_bid_index = base + col;
                }
            }
            KeyCode::Right => {
                if self.state.selected_bid_index < BID_GRID_SIZE - 1 {
                    self.state.selected_bid_index += 1;
                }
            }
            KeyCode::Left => {
                if self.state.selected_bid_index > 0 {
                    self.state.selected_bid_index -= 1;
                }
            }
            KeyCode::Down => {
                let idx = self.state.selected_bid_index;
                if idx < 35 {
                    let new = idx + 5;
                    if new < 35 {
                        self.state.selected_bid_index = new;
                    } else {
                        self.state.selected_bid_index = 35;
                    }
                }
            }
            KeyCode::Up => {
                let idx = self.state.selected_bid_index;
                if idx >= 35 {
                    self.state.selected_bid_index = 30;
                } else if idx >= 5 {
                    self.state.selected_bid_index = idx - 5;
                }
            }
            KeyCode::Enter => {
                if let Some(bid) = bid_at_index(self.state.selected_bid_index) {
                    self.try_place_bid(bid);
                }
            }
            _ => {}
        }
    }

    /// Move selection to the given suit column in the current bid row.
    fn try_place_bid(&mut self, bid: Bid) {
        if !self.state.game.auction.is_valid_bid(&bid) {
            self.state.set_status(format!("Invalid bid: {}", bid));
            return;
        }
        let bidder = self.state.game.current_seat();
        let bid_display = format!("{}", bid);
        if let Err(e) = self.state.game.place_bid(bid) {
            self.state.set_status(format!("Bid error: {}", e));
            return;
        }
        self.state.selected_bid_index = 35;

        if self.state.game.phase == Phase::Playing {
            self.state.selected_card_index = Some(0);
            if let Some(contract) = &self.state.game.contract {
                self.state
                    .set_status(format!("Contract: {} — {} leads", contract, contract.declarer.next()));
            }
        } else if self.state.game.phase == Phase::Finished {
            self.state.set_status("Passed out — no play");
        } else {
            self.state
                .set_status(format!("{} bid {}", bidder, bid_display));
        }
    }

    fn handle_playing_key(&mut self, code: KeyCode) {
        let current = self.state.game.current_seat();
        // Human always plays South; also plays North when N/S declares
        let human_can_play = self.state.game.contract.as_ref().is_some_and(|c| {
            current == Seat::South
                || (c.declarer.is_ns() && (current == c.declarer || current == c.dummy))
        });
        if !human_can_play {
            return;
        }
        let eligible = self.state.game.eligible_cards();
        if eligible.is_empty() {
            return;
        }

        match code {
            KeyCode::Right => {
                if let Some(idx) = self.state.selected_card_index {
                    if idx > 0 {
                        self.state.selected_card_index = Some(idx - 1);
                    }
                }
            }
            KeyCode::Left => {
                if let Some(idx) = self.state.selected_card_index {
                    if idx < eligible.len() - 1 {
                        self.state.selected_card_index = Some(idx + 1);
                    }
                }
            }
            KeyCode::Char(c) => {
                let rank = match c {
                    'a' | 'A' => Some(Rank::Ace),
                    'k' | 'K' => Some(Rank::King),
                    'q' | 'Q' => return,
                    'j' | 'J' => Some(Rank::Jack),
                    't' | 'T' => Some(Rank::Ten),
                    '9' => Some(Rank::Nine),
                    '8' => Some(Rank::Eight),
                    '7' => Some(Rank::Seven),
                    '6' => Some(Rank::Six),
                    '5' => Some(Rank::Five),
                    '4' => Some(Rank::Four),
                    '3' => Some(Rank::Three),
                    '2' => Some(Rank::Two),
                    _ => None,
                };
                if let Some(rank) = rank {
                    if let Some(pos) = eligible.iter().position(|c| c.rank == rank) {
                        self.state.selected_card_index = Some(pos);
                    }
                }
            }
            KeyCode::Up => {
                if let Some(idx) = self.state.selected_card_index {
                    if idx < eligible.len() {
                        let current_suit = eligible[idx].suit;
                        if let Some(pos) = eligible.iter().position(|c| c.suit > current_suit) {
                            self.state.selected_card_index = Some(pos);
                        }
                    }
                }
            }
            KeyCode::Down => {
                if let Some(idx) = self.state.selected_card_index {
                    if idx < eligible.len() {
                        let current_suit = eligible[idx].suit;
                        if let Some(pos) = eligible.iter().rposition(|c| c.suit < current_suit) {
                            self.state.selected_card_index = Some(pos);
                        }
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(idx) = self.state.selected_card_index {
                    if idx < eligible.len() {
                        let card = eligible[idx];
                        self.try_play_card(card);
                    }
                }
            }
            _ => {}
        }
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}
