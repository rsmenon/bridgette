use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use crate::types::Vulnerability;

fn config_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("bridgette")
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.yaml")
}

fn data_dir() -> PathBuf {
    config_dir().join("data")
}

// --- Settings ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub api: ApiSettings,
    pub agents: AgentSeats,
    pub review: ReviewSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiSettings {
    pub provider: String,
    /// Supports env var reference like "${ANTHROPIC_API_KEY}"
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeatAgent {
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NorthAgent {
    pub model: String,
    /// Custom system instructions for North (your partner). When set, these
    /// replace the default SAYC system prompt, letting you fine-tune your
    /// partner's playing style.
    #[serde(default)]
    pub custom_instructions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSeats {
    pub north: NorthAgent,
    pub east: SeatAgent,
    pub west: SeatAgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSettings {
    pub model: String,
    pub enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            api: ApiSettings {
                provider: "anthropic".to_string(),
                api_key: "${ANTHROPIC_API_KEY}".to_string(),
            },
            agents: AgentSeats {
                north: NorthAgent {
                    model: "claude-opus-4-6".to_string(),
                    custom_instructions: String::new(),
                },
                east: SeatAgent {
                    model: "claude-opus-4-6".to_string(),
                },
                west: SeatAgent {
                    model: "claude-opus-4-6".to_string(),
                },
            },
            review: ReviewSettings {
                model: "claude-opus-4-6".to_string(),
                enabled: true,
            },
        }
    }
}

impl Settings {
    /// Load settings from ~/.config/bridgette/settings.yaml, creating defaults if absent.
    pub fn load() -> Self {
        let path = settings_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => match serde_yaml::from_str(&contents) {
                    Ok(settings) => return settings,
                    Err(e) => {
                        eprintln!("Warning: failed to parse {}: {}", path.display(), e);
                        eprintln!("Using default settings");
                    }
                },
                Err(e) => {
                    eprintln!("Warning: failed to read {}: {}", path.display(), e);
                    eprintln!("Using default settings");
                }
            }
        } else {
            let settings = Settings::default();
            if let Err(e) = settings.save() {
                eprintln!("Warning: failed to create settings: {}", e);
            }
            return settings;
        }
        Settings::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = config_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("Create dir error: {}", e))?;
        let yaml = serde_yaml::to_string(self).map_err(|e| format!("Serialize error: {}", e))?;
        fs::write(settings_path(), yaml).map_err(|e| format!("Write error: {}", e))
    }

    /// Resolve the API key, expanding env var references like "${ANTHROPIC_API_KEY}".
    pub fn resolve_api_key(&self) -> Option<String> {
        let key = &self.api.api_key;
        if key.starts_with("${") && key.ends_with('}') {
            let var_name = &key[2..key.len() - 1];
            std::env::var(var_name).ok()
        } else if key.is_empty() {
            None
        } else {
            Some(key.clone())
        }
    }

}

// --- Game Record (JSON) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameRecord {
    pub id: String,
    pub timestamp: String,
    pub dealer: String,
    pub hands: HandsRecord,
    pub bidding: Vec<BidRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<ContractRecord>,
    pub play: Vec<TrickRecord>,
    pub result: ResultRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandsRecord {
    pub north: Vec<String>,
    pub east: Vec<String>,
    pub south: Vec<String>,
    pub west: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidRecord {
    pub seat: String,
    pub bid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractRecord {
    pub level: u8,
    pub suit: String,
    pub doubled: bool,
    pub redoubled: bool,
    pub declarer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrickRecord {
    pub trick_number: usize,
    pub cards: Vec<TrickCardRecord>,
    pub winner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrickCardRecord {
    pub seat: String,
    pub card: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultRecord {
    pub tricks_won_ns: usize,
    pub tricks_won_ew: usize,
    pub score_ns: i32,
    pub score_ew: i32,
    pub breakdown: ScoreBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub contract_points: i32,
    pub overtrick_points: i32,
    pub game_bonus: i32,
    pub slam_bonus: i32,
    pub insult_bonus: i32,
}


// --- SavedGame (unified format for library) ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameStatus {
    InProgress,
    Completed,
}

impl std::fmt::Display for GameStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameStatus::InProgress => write!(f, "In Progress"),
            GameStatus::Completed => write!(f, "Completed"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedGame {
    pub id: String,
    pub timestamp: String,
    pub status: GameStatus,
    #[serde(default)]
    pub favorite: bool,
    pub dealer: String,
    pub hands: HandsRecord,
    pub bidding: Vec<BidRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contract: Option<ContractRecord>,
    pub play: Vec<TrickRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_trick: Option<Vec<TrickCardRecord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultRecord>,
    /// Serialized Game struct for resuming in-progress games.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_state: Option<String>,
    /// When the game was completed (HH:MM:SS format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// Vulnerability for this deal. Defaults to None for old saves.
    #[serde(default)]
    pub vulnerability: Vulnerability,
}

impl SavedGame {
    pub fn save(&self) -> Result<PathBuf, String> {
        let dir = data_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create data dir: {}", e))?;
        let filename = format!("{}.json", self.id);
        let path = dir.join(&filename);
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Serialize error: {}", e))?;
        fs::write(&path, json).map_err(|e| format!("Write error: {}", e))?;
        Ok(path)
    }

    /// Display-friendly declarer from the contract record.
    pub fn declarer(&self) -> &str {
        self.contract
            .as_ref()
            .map(|c| c.declarer.as_str())
            .unwrap_or("—")
    }

    /// Display-friendly contract string.
    pub fn contract_display(&self) -> String {
        match &self.contract {
            Some(c) => {
                let suit_short = match c.suit.as_str() {
                    "Clubs" => "C",
                    "Diamonds" => "D",
                    "Hearts" => "H",
                    "Spades" => "S",
                    "NoTrump" => "NT",
                    other => other,
                };
                let modifier = if c.redoubled {
                    "XX"
                } else if c.doubled {
                    "X"
                } else {
                    ""
                };
                format!("{}{}{}", c.level, suit_short, modifier)
            }
            None => {
                // In-progress games without a contract are still bidding
                if self.status == GameStatus::InProgress {
                    String::new()
                } else {
                    "Passed Out".to_string()
                }
            }
        }
    }

    /// Display-friendly score (NS perspective).
    pub fn score_display(&self) -> String {
        match &self.result {
            Some(r) => {
                if r.score_ns >= 0 {
                    format!("+{}", r.score_ns)
                } else {
                    format!("{}", r.score_ns)
                }
            }
            None => "—".to_string(),
        }
    }

    /// Get vulnerability for this game.
    pub fn vulnerability(&self) -> Vulnerability {
        self.vulnerability
    }

    /// Display-friendly local timestamp.
    pub fn timestamp_display(&self) -> String {
        // Try to parse ISO 8601 and format as local
        chrono::DateTime::parse_from_rfc3339(&self.timestamp)
            .or_else(|_| chrono::DateTime::parse_from_str(&self.timestamp, "%Y-%m-%dT%H:%M:%SZ"))
            .map(|dt| {
                dt.with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|_| self.timestamp.clone())
    }
}

/// Load all saved games from the data directory.
pub fn load_all_games() -> Vec<SavedGame> {
    let dir = data_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut games: Vec<SavedGame> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(contents) = fs::read_to_string(&path) {
                // Try SavedGame format first
                if let Ok(saved) = serde_json::from_str::<SavedGame>(&contents) {
                    games.push(saved);
                    continue;
                }
                // Fall back to old GameRecord format and migrate
                if let Ok(record) = serde_json::from_str::<GameRecord>(&contents) {
                    let saved = SavedGame {
                        id: record.id,
                        timestamp: record.timestamp,
                        status: GameStatus::Completed,
                        favorite: false,
                        dealer: record.dealer,
                        hands: record.hands,
                        bidding: record.bidding,
                        contract: record.contract,
                        play: record.play,
                        current_trick: None,
                        result: Some(record.result),
                        game_state: None,
                        ended_at: None,
                        vulnerability: Vulnerability::None,
                    };
                    games.push(saved);
                }
            }
        }
    }

    // Sort by timestamp descending (newest first)
    games.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    games
}

/// Delete a saved game by its id.
pub fn delete_game(id: &str) -> Result<(), String> {
    let path = data_dir().join(format!("{}.json", id));
    fs::remove_file(path).map_err(|e| format!("Failed to delete game: {}", e))
}

/// Toggle the favorite status of a saved game.
pub fn update_favorite(id: &str, fav: bool) -> Result<(), String> {
    let path = data_dir().join(format!("{}.json", id));
    let contents =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read game: {}", e))?;
    let mut saved: SavedGame =
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse game: {}", e))?;
    saved.favorite = fav;
    let json =
        serde_json::to_string_pretty(&saved).map_err(|e| format!("Serialize error: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("Write error: {}", e))
}
