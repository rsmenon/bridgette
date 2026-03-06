pub mod parse;
pub mod prompt;
pub mod tutor;

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use crate::engine::bidding::Bid;
use crate::engine::card::Card;
use crate::engine::game::Game;
use crate::types::{Phase, Seat};

use self::prompt::{bid_ascii, build_bidding_prompt, build_play_prompt, build_system_prompt, card_ascii};

/// Result of an agent's decision.
#[derive(Debug)]
pub enum AgentAction {
    Bid(Bid),
    PlayCard(Card),
}

/// Result sent back from agent thread, including any errors encountered.
#[derive(Debug)]
pub struct AgentResult {
    pub action: AgentAction,
    /// Errors encountered before arriving at the action (e.g. retries, fallback).
    pub errors: Vec<String>,
}

/// Trait for different AI backends.
pub trait AgentBackend: Send + Sync {
    fn query(&self, system_prompt: &str, user_prompt: &str) -> Result<String, String>;
    fn name(&self) -> &str;
}

/// Claude API backend using reqwest blocking client.
pub struct ClaudeApiBackend {
    client: reqwest::blocking::Client,
    api_key: String,
    model: String,
}

impl ClaudeApiBackend {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::blocking::Client::new(),
            api_key,
            model,
        }
    }
}

impl AgentBackend for ClaudeApiBackend {
    fn query(&self, system_prompt: &str, user_prompt: &str) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 64,
            "system": system_prompt,
            "messages": [
                {"role": "user", "content": user_prompt}
            ]
        });

        let resp = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| format!("API request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("API error {}: {}", status, text));
        }

        let json: serde_json::Value = resp
            .json()
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        json["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "No text in API response".to_string())
    }

    fn name(&self) -> &str {
        "Claude API"
    }
}

/// Claude CLI backend using subprocess.
pub struct ClaudeCliBackend {
    model: String,
}

impl ClaudeCliBackend {
    pub fn new(model: String) -> Self {
        Self { model }
    }
}

impl AgentBackend for ClaudeCliBackend {
    fn query(&self, system_prompt: &str, user_prompt: &str) -> Result<String, String> {
        let output = std::process::Command::new("claude")
            .arg("-p")
            .arg("--output-format")
            .arg("text")
            .arg("--system-prompt")
            .arg(system_prompt)
            .arg("--model")
            .arg(&self.model)
            .arg(user_prompt)
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

    fn name(&self) -> &str {
        "Claude CLI"
    }
}

/// Controls agent background threads and communication.
pub struct AgentController {
    backend: Arc<dyn AgentBackend>,
    tx: Sender<AgentResult>,
    rx: Receiver<AgentResult>,
    pub pending: bool,
    pub pending_seat: Option<Seat>,
    /// Custom system instructions for North. When non-empty, replaces the
    /// default SAYC system prompt for the North agent only.
    north_custom_instructions: Option<String>,
}

impl AgentController {
    pub fn new(backend: Arc<dyn AgentBackend>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            backend,
            tx,
            rx,
            pending: false,
            pending_seat: None,
            north_custom_instructions: None,
        }
    }

    pub fn set_north_custom_instructions(&mut self, instructions: String) {
        if instructions.is_empty() {
            self.north_custom_instructions = None;
        } else {
            self.north_custom_instructions = Some(instructions);
        }
    }

    /// Try to receive a completed agent result.
    pub fn try_recv(&mut self) -> Option<AgentResult> {
        match self.rx.try_recv() {
            Ok(result) => {
                self.pending = false;
                self.pending_seat = None;
                Some(result)
            }
            Err(_) => None,
        }
    }

    /// Dispatch a background request for the given seat.
    pub fn dispatch(&mut self, game: &Game, seat: Seat) {
        if self.pending || game.phase == Phase::Finished {
            return;
        }
        self.pending = true;
        self.pending_seat = Some(seat);

        let view = game.agent_view(seat);

        let backend = Arc::clone(&self.backend);
        let tx = self.tx.clone();
        let phase = game.phase;
        let contract = game.contract;
        let custom = if seat == Seat::North {
            self.north_custom_instructions.clone()
        } else {
            None
        };

        thread::spawn(move || {
            let system = build_system_prompt(seat, contract.as_ref(), custom.as_deref());
            let base_prompt = match phase {
                Phase::Bidding => build_bidding_prompt(&view),
                Phase::Playing => build_play_prompt(&view),
                Phase::Finished => unreachable!(),
            };

            let mut errors: Vec<String> = Vec::new();
            let mut correction: Option<String> = None;

            for attempt in 0..3 {
                let user = match &correction {
                    None => base_prompt.clone(),
                    Some(feedback) => format!("{}\n\n{}", base_prompt, feedback),
                };

                match backend.query(&system, &user) {
                    Ok(response) => {
                        let action = match phase {
                            Phase::Bidding => {
                                parse::parse_bid(&response, &view.valid_bids)
                                    .map(AgentAction::Bid)
                            }
                            Phase::Playing => {
                                parse::parse_card(&response, &view.eligible_cards)
                                    .map(AgentAction::PlayCard)
                            }
                            Phase::Finished => unreachable!(),
                        };

                        match action {
                            Ok(a) => {
                                // Retry succeeded — discard transient errors
                                let _ = tx.send(AgentResult { action: a, errors: Vec::new() });
                                return;
                            }
                            Err(_) => {
                                // Build correction feedback for next attempt
                                let options = match phase {
                                    Phase::Bidding => {
                                        let strs: Vec<String> = view.valid_bids.iter().map(bid_ascii).collect();
                                        format!("Valid bids: {}", strs.join(", "))
                                    }
                                    Phase::Playing => {
                                        let strs: Vec<String> = view.eligible_cards.iter().map(|c| card_ascii(c)).collect();
                                        format!("Eligible cards: {}", strs.join(", "))
                                    }
                                    Phase::Finished => unreachable!(),
                                };
                                correction = Some(format!(
                                    "CORRECTION: Your previous response \"{}\" could not be parsed \
                                     or was not a legal choice. You MUST respond with exactly one \
                                     of these options: {}. Reply with ONLY the bid/card, nothing else.",
                                    response.trim(),
                                    options
                                ));
                                errors.push(format!(
                                    "[{}] Bad response (attempt {}): \"{}\"",
                                    seat.short(), attempt + 1, response.trim()
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        errors.push(format!("[{}] API error (attempt {}): {}", seat.short(), attempt + 1, e));
                    }
                }
            }

            // After 3 failures, play a safe default (Pass / first eligible card)
            errors.push(format!("[{}] All retries failed, using default", seat.short()));
            let action = match phase {
                Phase::Bidding => AgentAction::Bid(Bid::Pass),
                Phase::Playing => AgentAction::PlayCard(view.eligible_cards[0]),
                Phase::Finished => unreachable!(),
            };
            let _ = tx.send(AgentResult { action, errors });
        });
    }

    /// Reset pending state (e.g., on new game).
    pub fn reset(&mut self) {
        self.pending = false;
        self.pending_seat = None;
        // Drain any remaining messages
        while self.rx.try_recv().is_ok() {}
    }
}

/// Detect the best available backend. Panics if neither API key nor CLI is available.
pub fn detect_backend(
    api_key: Option<String>,
    model: Option<String>,
) -> Arc<dyn AgentBackend> {
    let model = model.unwrap_or_else(|| "claude-opus-4-6".to_string());

    // Auto-detect: try API key first, then CLI
    if let Some(key) = api_key {
        Arc::new(ClaudeApiBackend::new(key, model))
    } else if cli_available() {
        Arc::new(ClaudeCliBackend::new(model))
    } else {
        panic!(
            "No agent backend available. Set ANTHROPIC_API_KEY or install the `claude` CLI."
        );
    }
}

pub fn cli_available() -> bool {
    std::process::Command::new("claude")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
}
