# Bridgette — Architecture & Probability System

Detailed reference for the inference engine, bid constraints, and architecture decisions. For quick reference see CLAUDE.md.

## Data Flow

```
User Input (Keyboard)
    ↓
App::handle_key() → Game state mutations
    ↓
tick_agents()     → AgentController::dispatch() → background LLM thread (3 retries → fallback)
                  → try_recv() → apply AgentResult (Bid/Card)

tick_inference()  → dispatch_inference() → background thread → run_inference()
                  → try_recv() → AppState::inference updated

tick_tutor()      → TutorController::query() → background LLM thread
                  → try_recv() → TutorState updated

draw()            → render_board() / render_library() / render_review_panel()
```

All agent/inference/tutor calls are non-blocking: work is dispatched to background threads and results polled on each tick.

---

## Engine Modules

### engine/game.rs — Central State Machine
`Game` struct is the single source of truth. Key fields:
- `hands[4]` / `dealt_hands[4]` — current + original hands (for review/display)
- `auction: Auction` — bidding history
- `play_state: Option<PlayState>` — trick management during play
- `contract: Option<Contract>` — resolved after bidding
- `phase: Phase` — Bidding / Playing / Finished
- `dealer: Seat`, `vulnerability: Vulnerability`
- `score: Option<Score>` — populated at game end

Phase transitions:
1. `place_bid()` → validates via `Auction`, transitions to Playing when 3 passes follow a suit bid
2. `play_card()` → validates follow-suit, manages tricks, scores after trick 13
3. `agent_view(seat)` → `AgentGameView` snapshot (visible to AI)

`Game::from_hands()` creates a game with predetermined cards (used by review system).

### engine/bidding.rs — Auction
- `Bid` enum: `Suit(level, BidSuit)`, `Pass`, `Double`, `Redouble`
- `Auction::place_bid()` validates: must outrank previous suit bid; Double only after opponent suit bid; Redouble only after Double
- `Auction::resolve_contract()` determines declarer (first player on declaring side to bid the trump suit) and dummy

### engine/play.rs — PlayState
- `eligible_cards(hand)` enforces follow-suit: if led suit present, must play it; else any card
- `play_card(seat, card)` validates turn order, adds card, completes trick at 4 cards
- Tracks `ns_tricks` / `ew_tricks`

### engine/scoring.rs — Chicago Scoring
`calculate_score(contract, tricks_won, vulnerable) -> Score`

Score variants: `Made { contract_points, overtrick_points, bonus, total }` or `Defeated { undertricks, penalty }`.

Bonus breakdown:
- Part-score: 50 always
- Game: NV 300 / V 500 (when contract points ≥ 100)
- Small slam: NV 500 / V 750
- Grand slam: NV 1000 / V 1500
- Insult: 50 (doubled), 100 (redoubled)

Undertricks (doubled, NV): 100 / 200 / 300 progressive (first / second / third+)
Undertricks (doubled, V): 200 / 300 / 300

---

## Agent System

### agent/mod.rs — Backends & Controller

`AgentBackend` trait:
```rust
fn query(&self, system: &str, user: &str) -> Result<String>
```

`ClaudeApiBackend`: blocking reqwest, `max_tokens: 512`, hits `https://api.anthropic.com/v1/messages`.
`ClaudeCliBackend`: spawns `claude` subprocess.

`AgentController::dispatch(game, seat)`:
1. Builds system + user prompt via `prompt.rs`
2. Spawns background thread
3. Retries up to 3 times with error feedback appended to prompt
4. Falls back: `Pass` (bidding) or first eligible card (play)
5. Sends `AgentResult { action, errors }` on channel

`AgentResult.errors` accumulates transient failures — displayed in bottom-left error panel.

### agent/prompt.rs — Prompt Construction

`AgentGameView` captures everything an agent needs:
- Own hand, dummy hand (if visible)
- Dealer, vulnerability, bidding history, valid bids
- Contract, current trick, completed tricks, eligible cards

`build_system_prompt(seat, contract)` assigns role:
- Bidding → "You are bidding for {seat}"
- Declarer → "You are declarer playing {contract}"
- Dummy → "You are playing dummy for {contract}"
- Defender → "You are defending {contract}"

Includes full `SAYC_REFERENCE` constant (~300 lines of SAYC conventions).

`build_situational_reminder()` injects hand-aware nudges when specific auction patterns are detected:
- Partner opened 1NT → Stayman / Jacoby transfer reminders
- Blackwood response → ace-count context
- Weak two / preempt → appropriate response guidance

### agent/parse.rs — Response Parsing
Flexible matching against valid bids or eligible cards:
- Tries exact match, then rank+suit combos, then "Ace of Spades" etc.
- Case-insensitive; handles suit symbols as well as ASCII

---

## Probability Inference System

The inference system estimates the probability each unknown card is held by each unknown seat, using bid constraint priors updated by play history.

### Bid Constraint Extraction (engine/bid_constraints.rs, ~1300 lines)

`extract_constraints(auction, seat) -> HandConstraints`

`HandConstraints`:
```rust
struct HandConstraints {
    hcp_range: (u32, u32),          // estimated HCP bounds
    suit_lengths: [(u32, u32); 4],  // (min, max) per suit: C/D/H/S
    void_suits: Vec<Suit>,          // confirmed voids
    ace_range: (u32, u32),          // aces (from Blackwood)
}
```

Pattern-matching covers:
- Opening bids (1C = 3+ clubs, 1D = 4+ diamonds, 1H/1S = 5+ major, 1NT = 15–17 HCP balanced, 2C strong)
- Weak twos / preempts (6+ cards in suit, limited HCP)
- Stayman responses, Jacoby transfers
- Blackwood 4NT + responses (ace counts → `ace_range`)
- Raises, overcalls, negative doubles

### Inference Engine (engine/inference.rs, ~1800 lines)

`run_inference(game, known_seat) -> CardProbabilities`

`CardProbabilities`:
```rust
struct CardProbabilities {
    probs: [[[f64; 13]; 4]; 4],           // [seat][suit][rank] probability 0.0–1.0
    suit_histograms: [[[[f64; 14]; 4]; 4]], // [seat][suit][length] histogram
    played_cards: HashSet<Card>,
    sample_count: u32,                     // u32::MAX for exact enumeration
    unknown_seat_count: usize,             // 2 or 3 (affects UI neutral midpoint)
    remaining_per_seat: [u32; 4],
}
```

#### Algorithm: `run_inference()`

**Step 1: Setup**
- Identify unknown seats (not South, not dummy if revealed)
- Build pool: all unplayed cards not in known hands
- Detect voids from trick history: if seat didn't follow a led suit, it's void in that suit

**Step 2: Adjust constraints for play**
- Subtract played cards' HCP/suit-counts from bid-inferred constraints
- Clamp to valid ranges (floor 0, suit max = remaining cards)

**Step 3: Route to exact or Monte Carlo**

**Exact enumeration** when: 2 unknown seats AND pool size ≤ 20
- Enumerate all ways to split pool between two unknown seats
- Check all constraints (HCP, suit-length, voids, aces)
- Average accepted distributions → exact probabilities + histograms
- Sets `sample_count = u32::MAX`

**Monte Carlo sampling** otherwise:

```
Phase 1 (probe): attempt 200 samples with original constraints
  → if acceptance rate < 10%: widen all HCP ranges by ±2, discard Phase 1 samples
Phase 2: fill remaining sample budget with possibly-relaxed constraints
```

`generate_constrained_deal(pool, constraints, voids, targets)`:
1. Force-assign cards where only one eligible seat remains (singleton constraint)
2. Satisfy suit-length minimums from free pool for each unknown seat
3. Shuffle remaining free cards; distribute to seats to hit target card counts
4. Reject if any void constraint violated after shuffling

**Fallback** (if < 50 samples accepted):
- `constraint_weighted_fallback()`: uses bid-constraint suit demand as weights per seat
- Normalizes so each seat's total = expected card count

**Step 4: Post-processing**
- Zero out void suits
- Redistribute zeroed mass proportionally to surviving suits (preserving row sums)
- Compute suit-length histograms from accepted samples

#### Confidence Classification

`suit_confidence(seat, suit) -> Confidence` (Low/Med/High):
- High: histogram variance < 0.5 (very constrained)
- Med: variance < 1.5
- Low: otherwise

`suit_length_range(seat, suit) -> (u32, u32)`:
- 80% highest-density interval (HDI) from histogram if available
- Falls back to marginal estimate from constraints

### Probability Grid UI (ui/probability_grid.rs)

Rendered when `show_probabilities = true` (toggled with `M`).

For each unknown seat:
- **Honor grid**: A/K/Q/J for each suit (bridge order: S/H/D/C), colored by probability (green=high, red=low, dim=impossible)
- **Distribution bar**: ASCII histogram of suit-length distribution
- **Range**: 80% HDI as "(min–max cards)"
- **Confidence**: Low/Med/High label

Played cards shown as 'x'. Color midpoint adjusted by `unknown_seat_count` (2 vs 3 unknowns have different neutral points).

### Inference Lifecycle in app.rs

```
dispatch_inference(game, state):
  → fingerprint = hash(hands + play history)
  → skip if fingerprint unchanged or pending
  → spawn background thread: run_inference() → send CardProbabilities
  → set state.inference.pending = true

tick_inference(state):
  → try_recv() probs
  → update state.inference.probs, clear pending, update fingerprint
```

---

## UI Architecture

### AppState (ui/mod.rs)

Central UI state, passed by `&mut` through render functions:
```rust
struct AppState {
    game: Game,
    selected_card_index: usize,
    selected_bid_index: usize,
    status_message: Option<(String, u32)>,   // message + decay ticks
    trick_scroll: usize,
    show_help: bool,
    show_probabilities: bool,
    agent_thinking: Option<(Seat, Instant)>,
    agent_info: AgentInfo,
    agent_errors: Vec<String>,
    tutor: TutorState,
    inference: InferenceState,
    game_started_at: Option<Instant>,
    game_ended_at: Option<Instant>,
}
```

`InferenceState`: `{ probs: Option<CardProbabilities>, fingerprint: u64, pending: bool }`

### Board Layout (ui/board.rs)

3×3 grid with right sidebar:
```
┌─────────────┬──────────────┬──────────────┐
│ Stats       │ North hand   │ Auction      │
├─────────────┼──────────────┼──────────────┤
│ West hand   │ Trick area   │ East hand    │
├─────────────┼──────────────┼──────────────┤
│ Errors      │ South hand   │ Score        │
└─────────────┴──────────────┴──────────────┘
                                    │ Sidebar (38 chars)
                                    │ - Bid selector (bidding)
                                    │ - Trick history (playing/finished)
                                    │ - Splits 50/50 with tutor pane when active
```

Hand cells include: cards (high-to-low), selection highlight, HCP/Dist score line, agent spinner, model name. When `show_probabilities`, hands are overlaid with probability grid data.

### AppMode (app.rs)
```rust
enum AppMode {
    Game,
    Library,
    Review,
    ConfirmQuit,
    ConfirmLeaveGame,
}
```

Draw and input dispatch based on mode. Library and Review are layered on top of the game state (game is suspended but preserved).

---

## Key Conventions & Gotchas

- `Borders::ROUNDED` doesn't exist in ratatui 0.29 — use `BorderType::Rounded` with `Borders::ALL`
- Bid grid indices: 0–34 for 7×5 level/suit grid, 35–37 for Pass/Double/Redouble
- `Seat::index()` returns 0–3 (N=0, E=1, S=2, W=3) for array access
- Cards stored low-to-high internally, displayed high-to-low; arrow key direction inverted
- Only suit symbols colored (red/black); rank characters use text foreground color
- `Hand::hcp()` is single source of HCP — do not duplicate
- Library screen: muted blue palette (`ACCENT_MUTED_BLUE`, `BG_SELECTED_BLUE`), not gold
- Library status: default color for "In Progress", `ACCENT_GREEN` only for "✓ Done"
- Stats panel values: non-bold
- Pass/Dbl/Rdbl: foreground text color, not accent colors
- Hand scoring (HCP/Dist): `TEXT_DARK_MUTED` uniformly
- `SavedGame` uses `#[serde(default)]` on `vulnerability` for backward compatibility with old saves
