# Bridgette

A terminal-based contract bridge game in Rust where a human plays South against three AI agents (North, East, West). Built with Ratatui + Crossterm.

## Quick Reference

```bash
cargo build                  # Build
cargo test                   # Run tests (116 passing)
cargo run                    # Run (backend auto-detected from settings.yaml)
```

Config: `~/.config/bridgette/settings.yaml` (API key, agent models, backend auto-detection)
Game data: `~/.config/bridgette/data/`

Backend auto-detection priority: API key present → Claude API, `claude` CLI available → CLI, otherwise panics.

## Architecture

```
src/
├── main.rs              # Entry point, terminal setup, event loop (~30fps polling)
├── app.rs               # App struct: game state, agent controller, mode management (Game/Library/Review), draw/input
├── types.rs             # Seat (N/E/S/W), Phase (Bidding/Playing/Finished)
├── config.rs            # Settings (YAML), NorthAgent/SeatAgent, SavedGame/GameRecord (JSON), file I/O
├── engine/
│   ├── mod.rs           # Module re-exports
│   ├── card.rs          # Suit, Rank, Card (Ord, Hash, Serialize), Card::from_ascii()
│   ├── deck.rs          # Deck: shuffle, deal into 4 Hands
│   ├── hand.rs          # Hand: sorted Vec<Card>, suit filtering, removal, HCP/dist-point calculation
│   ├── trick.rs         # Trick: cards played, leader, winner(trump)
│   ├── bidding.rs       # BidSuit, Bid, Auction: validation, completion, contract resolution, Bid::from_ascii()
│   ├── contract.rs      # Contract: level, suit, doubled, declarer, dummy
│   ├── play.rs          # PlayState: trick management, eligible cards, follow-suit rules
│   ├── scoring.rs       # Score: contract/overtrick/bonus points, undertrick penalties
│   ├── game.rs          # Game: full state machine (deal → bid → play → score), Game::from_hands()
│   └── tests.rs         # 96 tests covering all engine logic
├── agent/
│   ├── mod.rs           # AgentBackend trait, ClaudeApi/ClaudeCli backends, AgentController
│   ├── prompt.rs        # AgentGameView, system/bidding/play prompt construction (SAYC)
│   ├── parse.rs         # Parse LLM text → Bid or Card (flexible format matching)
│   └── tutor.rs         # TutorController: background LLM tutor (API/CLI backends, conversation history)
└── ui/
    ├── mod.rs           # AppState: game + UI state (selection indices, status messages, agent_errors)
    ├── board.rs         # 3×3 grid layout: hands at compass positions, center trick area, panels
    ├── bid_selector.rs  # 7×5 bid grid (levels × suits) + Pass/Double/Redouble row
    ├── bidding_panel.rs # Auction history as 4-column table (N/E/S/W)
    ├── trick_history.rs # Scrollable trick log with winner highlights
    ├── tutor.rs         # TutorState + render_tutor_pane: input, response, markdown/card parsing
    ├── review.rs        # ReviewState + compact board: step-through review of completed games with tutor
    ├── score_panel.rs   # Score breakdown panel (trick counts, result, points)
    ├── controls.rs      # Status bar + phase-specific keyboard hints
    ├── dialog.rs        # Confirm quit/leave popups
    ├── library.rs       # LibraryState: saved game browser with filtering, favorites
    └── palette.rs       # Truecolor constants (warm paper-on-gray theme)
```

## Key Design Patterns

### Game State Machine
`Game` (engine/game.rs) is the central type. Phase transitions:
1. **Bidding**: `place_bid()` validates via `Auction`, transitions to Playing when 3 passes follow a suit bid
2. **Playing**: `play_card()` validates follow-suit, manages tricks, calculates score after trick 13
3. **Finished**: Score computed, game saveable

`Game` is fully serializable (serde) for save/resume.

### Agent System
`AgentController` (agent/mod.rs) manages background threads via mpsc channels. The channel sends `AgentResult` which includes the action plus any errors encountered during retries.

Two backends implement `AgentBackend` trait:
- **ClaudeApiBackend**: blocking reqwest to Anthropic Messages API (64 max_tokens)
- **ClaudeCliBackend**: `claude` CLI subprocess

LLM calls run in background threads with 3 retries; after exhausting retries, falls back to a safe default (Pass during bidding, first eligible card during play). Errors are surfaced to the UI via `AppState.agent_errors` and displayed in the bottom-left panel. `detect_backend()` auto-selects: API key present → API, `claude` CLI available → CLI, otherwise panics. No CLI arguments — all configuration via `settings.yaml` and env vars.

### Prompt Structure
- System prompt sets role (bidder/declarer/defender) and includes a full SAYC reference (`SAYC_REFERENCE` constant in prompt.rs).
- Bidding prompt includes hand, HCP, history, valid bids, plus **situational reminders** — `build_situational_reminder()` detects auction triggers (partner opened 1NT, Stayman, Jacoby Transfer, Blackwood, etc.) and injects hand-aware nudges.
- Play prompt includes contract, hands (own + visible dummy), tricks, eligible cards.

### Human Control and Turn Logic
`is_agent_turn()` (app.rs) is the central gate for who acts. During **bidding**, only South is human. During **play**, South is always human-controlled. When N/S declares, the human also controls North:
- N/S declaring → human plays for both declarer and dummy turns; E/W turns are AI
- E/W declaring → human plays South; E/W and North are AI-controlled

`handle_playing_key()` mirrors this: `human_can_play = current == South || (c.declarer.is_ns() && (current == c.declarer || current == c.dummy))`. Card selection highlighting in `hand_lines()` (board.rs) works generically via `game.current_seat()` + `game.eligible_cards()`, so it naturally highlights the correct hand.

When North is declarer, North's hand cell hides the "Agent: ..." status during play since the human controls it.

### Tutor System
In-game AI tutor panel in the right sidebar. Activated with `T`, provides bidding/play advice.

**Backend** (`agent/tutor.rs`): `TutorController` with `TutorBackend` enum (API or CLI). Separate from `AgentController` — uses `max_tokens: 1024` (vs 64 for agents), returns free-text, independent lifecycle. Backend selection in `main.rs`: API key → API, `claude` CLI available → CLI, otherwise tutor unavailable. Multi-turn conversation history via `Vec<TutorMessage>` — exchanges recorded on `try_recv()`, cleared on `reset()` (new game/reactivation).

**UI** (`ui/tutor.rs`): `TutorState` holds input, cursor, response, scroll, pending state, `last_auto_turn` fingerprint. Right panel splits 50/50 when tutor active (top: bid selector/tricks, bottom: tutor pane). Tutor pane: title + scrollable response + 4-line bordered input box. Response text is parsed for `**bold**`/`*italic*` markdown and ASCII card mentions (e.g., `AS` → `A♠` with colored suit symbol).

**Auto-dispatch**: `tick_tutor()` fires a recommendation query when it becomes the human's turn. A turn fingerprint (`bids.len() + cards_played`) prevents re-dispatching for the same decision point.

**Prompt adaptation**: When North is declarer, system prompt notes "North is declarer, South is dummy, human controls both hands." User prompt shows both hands with labels, uses `agent_view(Seat::North)` for correct eligible cards, and asks "What is the best card for {North/South} to play?"

**Keys**: `T` toggle tutor, `Tab` focus input, `Esc` close/unfocus, `Enter` submit question, `PageUp/PageDown` scroll response. When input is focused, all character keys route to text input (including `?` which otherwise opens help).

### Custom Instructions (North only)
`NorthAgent` in config.rs has a `custom_instructions` field. When non-empty, it **replaces** the default SAYC system prompt, letting the user fine-tune their partner's style. East/West (`SeatAgent`) have model only — they always use the standard SAYC prompt.

### Card Display Format
Cards use rank-first format: `A♠`, `T❤`, `4♦` (display with unicode suits) / `AS`, `TH`, `4D` (ASCII in agent prompts). Cards stored low-to-high internally, displayed high-to-low. Arrow keys inverted accordingly (Right = idx-1, Left = idx+1). Suit symbols use standard Unicode characters (♣♦♥♠). Only the suit symbol is colored (red/black); rank characters use the text foreground color.

### UI Layout
3×3 grid with right sidebar (38 chars wide):
- **Top-left**: Stats panel (dealer, status, contract, lead, system, turn)
- **Top-center**: North's hand
- **Top-right**: Auction history
- **Mid-left/center/right**: West, center trick area, East
- **Bottom-left**: Agent error panel (errors only, shown in red)
- **Bottom-center**: South's hand (with HCP/Dist scoring line; all hands show HCP/Dist when finished)
- **Bottom-right**: Score panel (trick counts, need/have, result, score breakdown via `score_panel.rs`)
- **Right sidebar**: Bid selector (bidding) or trick history (playing/finished). When tutor active, splits 50/50 with tutor pane below.
- **Bottom bar**: Controls/status

### Review System
Step-through review of completed games from the Library. Selecting a completed game opens a split view: library table (left, `Min(75)`) + review panel (right, `Min(40)`).

**State** (`ui/review.rs`): `ReviewState` holds the `SavedGame`, current step index, total step count, a reconstructed `Game` at the current step, and a `TutorState` for the tutor panel.

**Game reconstruction**: `reconstruct_game_at_step()` replays bids/cards from `SavedGame` records through `Game::from_hands()` to produce valid game state at any step. Step 0 = first bid; steps 0..B-1 = bids; steps B..B+P-1 = card plays. `Card::from_ascii()` and `Bid::from_ascii()` parse saved record strings.

**Layout**: Review panel splits 60/40 vertically — compact 3×3 board (top) and tutor pane (bottom). The compact board shows all four hands at compass positions, auction history (top-left), step/action indicator (top-right), trick cards in center (during play), contract/trick info (bottom-left). During bidding phase, all hands show HCP/Dist scoring (`HCP:X Dist:Y = Z`) matching the game board format.

**Tutor integration**: Reuses the same `TutorController` from the game. Tutor is on-demand only — user presses `T` to get an explanation of the current step's action. `tick_tutor()` returns early when in Review mode so `tick_review()` can receive channel responses instead.

**Keys**: `←/→` step through, `T` ask tutor, `PageUp/PageDown` scroll tutor response, `Esc` close review.

### Persistence
- `SavedGame` (config.rs) wraps full serialized `Game` state + metadata (timestamps, favorite flag)
- In-progress games auto-save; completed games get a `GameRecord` with score breakdown
- Library UI supports filtering (in-progress/completed), favorites, resume, delete

## Scoring Rules
Chicago-style deals with vulnerability tracked per deal.
- Contract points: minors 20/trick, majors 30/trick, NT 40 first + 30 subsequent
- Doubled/redoubled multiply contract points 2×/4×
- Game bonus: NV 300 / V 500 if contract points ≥ 100, else 50 (part-score)
- Slam bonus: NV 500/1000 (small/grand), V 750/1500
- Insult bonus: 50 (doubled made), 100 (redoubled made)
- Overtricks: undoubled = trick value; doubled NV 100 / V 200 each; redoubled NV 200 / V 400 each
- Undertricks undoubled: NV 50 / V 100 each
- Undertricks doubled: NV 100/200/300 progressive; V 200/300/300
- Undertricks redoubled: NV 200/400/600 progressive; V 400/600/600

## Style Conventions
- Suit colors apply only to suit symbols, not rank characters
- Stats panel values are non-bold
- Pass/Dbl/Rdbl use foreground text color (not accent colors)
- Hand scoring (HCP/Dist) uses `TEXT_DARK_MUTED` uniformly
- `Borders::ROUNDED` doesn't exist in ratatui 0.29 — use `BorderType::Rounded` with `Borders::ALL`
- Bid grid indices: 0–34 for 7×5 level/suit grid, 35–37 for Pass/Double/Redouble
- `Seat::index()` returns 0–3 (N=0, E=1, S=2, W=3) for array access
- Library screen uses muted blue palette (`ACCENT_MUTED_BLUE`, `BG_SELECTED_BLUE`) — not gold
- Library status column uses default row text color; only "✓ Done" uses `ACCENT_GREEN`
- `Hand::hcp()` is the single source of HCP calculation — do not duplicate

## Vulnerability (Chicago-style)
- `Vulnerability` enum in `types.rs`: `None`, `NorthSouth`, `EastWest`, `Both`
- `Vulnerability::chicago(deal_number, dealer)` computes per-deal vulnerability:
  - Deal 0: None, Deal 1: Dealer's side, Deal 2: Both, Deal 3: Non-dealer's side (repeats)
- `App.deal_number` tracks the current deal (0-indexed, increments each `new_game()`)
- `Game.vulnerability` field stores the deal's vulnerability (serialized with `#[serde(default)]` for backward compat)
- Scoring uses `vulnerability.is_vulnerable(declarer)` to determine penalties/bonuses
- Vulnerable differences: game bonus 500 (vs 300), slam 750/1500 (vs 500/1000), undertricks 100 (vs 50), doubled/redoubled penalties higher
- Stats panel shows "Vul: None/N-S/E-W/Both" with red accent when anyone is vulnerable
- Agent prompts include vulnerability status in both bidding and play phases
- `SavedGame.vulnerability` persists vulnerability; old saves default to `None`
