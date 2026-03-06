use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::engine::game::Game;
use crate::types::{Phase, Seat};

use super::prompt::{hand_ascii, bid_ascii, card_ascii, SAYC_REFERENCE};

/// Backend variant for tutor queries.
#[derive(Clone)]
enum TutorBackend {
    Api { api_key: String, model: String },
    Cli { model: String },
}

impl TutorBackend {
    fn name(&self) -> &str {
        match self {
            TutorBackend::Api { .. } => "Claude API",
            TutorBackend::Cli { .. } => "Claude CLI",
        }
    }
}

/// A single message in the tutor conversation history.
#[derive(Clone)]
struct TutorMessage {
    role: &'static str, // "user" or "assistant"
    content: String,
}

fn query_api(
    api_key: &str,
    model: &str,
    system_prompt: &str,
    messages: &[TutorMessage],
) -> Result<String, String> {
    let client = reqwest::blocking::Client::new();
    let msgs: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
        .collect();
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 1024,
        "system": system_prompt,
        "messages": msgs
    });

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("Tutor API request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        return Err(format!("Tutor API error {}: {}", status, text));
    }

    let json: serde_json::Value = resp
        .json()
        .map_err(|e| format!("Failed to parse tutor response: {}", e))?;

    json["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No text in tutor API response".to_string())
}

fn query_cli(
    model: &str,
    system_prompt: &str,
    messages: &[TutorMessage],
) -> Result<String, String> {
    // CLI doesn't support multi-turn natively, so concatenate history into a single prompt
    let mut combined = String::new();
    for msg in messages {
        match msg.role {
            "user" => {
                combined.push_str(&msg.content);
                combined.push('\n');
            }
            "assistant" => {
                combined.push_str("\n[Tutor's previous response]\n");
                combined.push_str(&msg.content);
                combined.push_str("\n\n");
            }
            _ => {}
        }
    }

    let output = std::process::Command::new("claude")
        .arg("-p")
        .arg("--output-format")
        .arg("text")
        .arg("--system-prompt")
        .arg(system_prompt)
        .arg("--model")
        .arg(model)
        .arg(combined.trim())
        .output()
        .map_err(|e| format!("Failed to run claude CLI: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("claude CLI error: {}", stderr));
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        Err("Empty response from claude CLI".to_string())
    } else {
        Ok(text)
    }
}

/// Controls background tutor LLM calls with conversation history.
pub struct TutorController {
    backend: TutorBackend,
    tx: Sender<Result<String, String>>,
    rx: Receiver<Result<String, String>>,
    pub pending: bool,
    /// Conversation history for multi-turn context.
    history: Vec<TutorMessage>,
    /// The user prompt from the most recent dispatch (to record in history on response).
    last_user_prompt: Option<String>,
}

impl TutorController {
    pub fn new_api(api_key: String, model: String) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            backend: TutorBackend::Api { api_key, model },
            tx,
            rx,
            pending: false,
            history: Vec::new(),
            last_user_prompt: None,
        }
    }

    pub fn new_cli(model: String) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            backend: TutorBackend::Cli { model },
            tx,
            rx,
            pending: false,
            history: Vec::new(),
            last_user_prompt: None,
        }
    }

    pub fn backend_name(&self) -> &str {
        self.backend.name()
    }

    /// Try to receive a completed tutor response.
    /// On success, records the exchange in conversation history.
    pub fn try_recv(&mut self) -> Option<Result<String, String>> {
        match self.rx.try_recv() {
            Ok(result) => {
                self.pending = false;
                // Record the exchange in history
                if let Some(user_prompt) = self.last_user_prompt.take() {
                    self.history.push(TutorMessage {
                        role: "user",
                        content: user_prompt,
                    });
                    if let Ok(ref text) = result {
                        self.history.push(TutorMessage {
                            role: "assistant",
                            content: text.clone(),
                        });
                    }
                }
                Some(result)
            }
            Err(_) => None,
        }
    }

    /// Dispatch a tutor query for the current game state.
    /// If `question` is None, asks for the best next move recommendation.
    /// Includes prior conversation history for multi-turn context.
    pub fn dispatch(&mut self, game: &Game, question: Option<String>) {
        if self.pending {
            return;
        }
        self.pending = true;

        let north_declares = game.contract.as_ref().is_some_and(|c| c.declarer == Seat::North);
        let system = build_tutor_system_prompt(north_declares);
        let user = build_tutor_user_prompt(game, question);

        // Build full message list: history + current user prompt
        let mut messages = self.history.clone();
        messages.push(TutorMessage {
            role: "user",
            content: user.clone(),
        });

        self.last_user_prompt = Some(user);

        let backend = self.backend.clone();
        let tx = self.tx.clone();

        thread::spawn(move || {
            let result = match &backend {
                TutorBackend::Api { api_key, model } => {
                    query_api(api_key, model, &system, &messages)
                }
                TutorBackend::Cli { model } => {
                    query_cli(model, &system, &messages)
                }
            };
            let _ = tx.send(result);
        });
    }

    /// Dispatch a review-mode tutor query from a specific player's perspective.
    /// `game` should be the state *before* the action being analyzed.
    pub fn dispatch_review(&mut self, game: &Game, seat: Seat, question: String) {
        if self.pending {
            return;
        }
        self.pending = true;

        let system = "You are an expert bridge tutor reviewing a completed game.\n\
            You follow Standard American Yellow Card (SAYC) conventions.\n\n\
            Guidelines:\n\
            - Be VERY concise. 2-3 sentences max.\n\
            - Analyze from the perspective of the player whose action is being reviewed.\n\
            - Only use information that would be visible to that player at that moment.\n\
            - Format cards as rank+suit ASCII: AS, KH, TD, 4C.\n\
            - Do not use markdown formatting. Use plain text only."
            .to_string();

        let view = game.agent_view(seat);
        let mut lines = Vec::new();

        lines.push(format!("Reviewing from {}'s perspective.", seat));
        lines.push(format!("Dealer: {}", view.dealer));
        lines.push(String::new());
        lines.push(format!("{}'s hand:", seat));
        lines.push(hand_ascii(&view.hand));
        lines.push(format!("HCP: {}", view.hand.hcp()));

        if let Some(dummy) = &view.dummy_hand {
            lines.push(String::new());
            lines.push("Dummy's hand:".to_string());
            lines.push(hand_ascii(dummy));
        }

        if !view.bidding_history.is_empty() {
            lines.push(String::new());
            lines.push("Bidding:".to_string());
            for (s, bid) in &view.bidding_history {
                lines.push(format!("  {}: {}", s, bid_ascii(bid)));
            }
        }

        if let Some(contract) = &view.contract {
            lines.push(String::new());
            lines.push(format!("Contract: {} by {}", contract, contract.declarer));
        }

        if !view.completed_tricks.is_empty() {
            lines.push(String::new());
            lines.push(format!("Tricks — N/S: {}, E/W: {}", view.ns_tricks, view.ew_tricks));
            for (i, (winner, cards)) in view.completed_tricks.iter().enumerate() {
                let card_strs: Vec<String> = cards
                    .iter()
                    .map(|(s, c)| format!("{}:{}", s.short(), card_ascii(c)))
                    .collect();
                lines.push(format!("  Trick {}: {} -> won by {}", i + 1, card_strs.join(", "), winner));
            }
        }

        if !view.current_trick_cards.is_empty() {
            lines.push(String::new());
            lines.push("Current trick:".to_string());
            for (s, card) in &view.current_trick_cards {
                lines.push(format!("  {}: {}", s, card_ascii(card)));
            }
        }

        if game.phase == Phase::Playing {
            let eligible_strs: Vec<String> = view.eligible_cards.iter().map(card_ascii).collect();
            lines.push(String::new());
            lines.push(format!("Eligible cards: {}", eligible_strs.join(", ")));
        } else if game.phase == Phase::Bidding {
            let valid_strs: Vec<String> = view.valid_bids.iter().map(bid_ascii).collect();
            lines.push(String::new());
            lines.push(format!("Valid bids: {}", valid_strs.join(", ")));
        }

        lines.push(String::new());
        lines.push(question);

        let user = lines.join("\n");

        let messages = vec![TutorMessage {
            role: "user",
            content: user.clone(),
        }];
        self.last_user_prompt = Some(user);

        let backend = self.backend.clone();
        let tx = self.tx.clone();

        thread::spawn(move || {
            let result = match &backend {
                TutorBackend::Api { api_key, model } => {
                    query_api(api_key, model, &system, &messages)
                }
                TutorBackend::Cli { model } => {
                    query_cli(model, &system, &messages)
                }
            };
            let _ = tx.send(result);
        });
    }

    /// Reset pending state and clear conversation history.
    pub fn reset(&mut self) {
        self.pending = false;
        self.history.clear();
        self.last_user_prompt = None;
        while self.rx.try_recv().is_ok() {}
    }
}

fn build_tutor_system_prompt(north_declares: bool) -> String {
    let role = if north_declares {
        "You are an expert bridge tutor advising the human player during a game.\n\
         North is the declarer and South is dummy. The human controls both hands."
    } else {
        "You are an expert bridge tutor advising the South player during a game."
    };
    format!(
        "{role}\n\
         You follow Standard American Yellow Card (SAYC) conventions.\n\
         \n\
         {SAYC_REFERENCE}\n\
         \n\
         Guidelines:\n\
         - Be VERY concise. 2-3 sentences max for recommendations. Keep explanations short.\n\
         - Format cards as rank+suit ASCII: AS, KH, TD, 4C.\n\
         - Only use information visible to the declaring side at the current point in the game.\n\
         - When recommending a bid or play, state the action first, then a brief reason.\n\
         - If asked about a past bid that doesn't conform to SAYC, say so honestly.\n\
         - Do not use markdown formatting. Use plain text only.\n\
         - Do not repeat or restate the hand or game state. The player can already see it."
    )
}

fn build_tutor_user_prompt(game: &Game, question: Option<String>) -> String {
    let north_declares = game.contract.as_ref().is_some_and(|c| c.declarer == Seat::North);
    let current = game.current_seat();

    // When North declares, use North's view so eligible_cards reflect the current player correctly
    let view_seat = if north_declares && game.phase == Phase::Playing {
        Seat::North
    } else {
        Seat::South
    };
    let view = game.agent_view(view_seat);
    let mut lines = Vec::new();

    lines.push(format!("Dealer: {}", view.dealer));

    if north_declares && game.phase == Phase::Playing {
        // Show both hands with clear labels
        lines.push(String::new());
        lines.push("North's hand (declarer):".to_string());
        lines.push(hand_ascii(&game.hands[Seat::North.index()]));
        lines.push(String::new());
        lines.push("South's hand (dummy):".to_string());
        lines.push(hand_ascii(&game.hands[Seat::South.index()]));
    } else {
        lines.push(String::new());
        lines.push("South's hand:".to_string());
        lines.push(hand_ascii(&view.hand));
        lines.push(format!("HCP: {}", view.hand.hcp()));

        if let Some(dummy) = &view.dummy_hand {
            lines.push(String::new());
            lines.push("Dummy's hand:".to_string());
            lines.push(hand_ascii(dummy));
        }
    }

    // Auction history
    if !view.bidding_history.is_empty() {
        lines.push(String::new());
        lines.push("Bidding:".to_string());
        for (seat, bid) in &view.bidding_history {
            lines.push(format!("  {}: {}", seat, bid_ascii(bid)));
        }
    }

    // Contract and play info
    if let Some(contract) = &view.contract {
        lines.push(String::new());
        lines.push(format!("Contract: {} by {}", contract, contract.declarer));
    }

    if !view.completed_tricks.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Tricks — N/S: {}, E/W: {}",
            view.ns_tricks, view.ew_tricks
        ));
        lines.push("Completed tricks:".to_string());
        for (i, (winner, cards)) in view.completed_tricks.iter().enumerate() {
            let card_strs: Vec<String> = cards
                .iter()
                .map(|(s, c)| format!("{}:{}", s.short(), card_ascii(c)))
                .collect();
            lines.push(format!(
                "  Trick {}: {} -> won by {}",
                i + 1,
                card_strs.join(", "),
                winner
            ));
        }
    }

    if !view.current_trick_cards.is_empty() {
        lines.push(String::new());
        lines.push("Current trick:".to_string());
        for (seat, card) in &view.current_trick_cards {
            lines.push(format!("  {}: {}", seat, card_ascii(card)));
        }
    }

    lines.push(String::new());

    match game.phase {
        Phase::Bidding => {
            let valid_strs: Vec<String> = view.valid_bids.iter().map(bid_ascii).collect();
            lines.push(format!("Valid bids: {}", valid_strs.join(", ")));
            lines.push(String::new());
            match question {
                Some(q) => {
                    lines.push(format!("The player asks: \"{}\"", q));
                    lines.push(String::new());
                    lines.push(
                        "If this question is about a past bid, only consider information \
                         that was available at that point — do not use future information."
                            .to_string(),
                    );
                }
                None => {
                    lines.push(
                        "What is the best bid for South and why? Be concise.".to_string(),
                    );
                }
            }
        }
        Phase::Playing => {
            let eligible_strs: Vec<String> =
                view.eligible_cards.iter().map(card_ascii).collect();
            let playing_seat = if north_declares {
                current
            } else {
                Seat::South
            };
            lines.push(format!(
                "It is {}'s turn. Eligible cards: {}",
                playing_seat,
                eligible_strs.join(", ")
            ));
            lines.push(String::new());
            match question {
                Some(q) => {
                    lines.push(format!("The player asks: \"{}\"", q));
                    lines.push(String::new());
                    lines.push(
                        "If this question is about a past play, only consider information \
                         that was available at that point — do not use future information."
                            .to_string(),
                    );
                }
                None => {
                    let seat_label = if north_declares {
                        format!("{}", playing_seat)
                    } else {
                        "South".to_string()
                    };
                    lines.push(format!(
                        "What is the best card for {} to play and why? Be concise.",
                        seat_label
                    ));
                }
            }
        }
        Phase::Finished => {
            let label = if north_declares { "N/S" } else { "South" };
            match question {
                Some(q) => lines.push(format!("The player asks: \"{}\"", q)),
                None => lines.push(format!(
                    "Give a brief summary of how {} played this hand.",
                    label
                )),
            }
        }
    }

    lines.join("\n")
}
