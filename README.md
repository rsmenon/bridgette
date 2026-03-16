# ♠️♥️ BRIDGETTE: Bridge Terminal Tutor Engine ♦️♣️

A terminal-based Contract Bridge app where you play as South with an AI partner at North and two AI opponents at East/West. All three AI players bid and play using Standard American Yellow Card (SAYC) conventions. 

This app is targeted at people who don't have access to a local Bridge club, but want to practice and improve their skills 
* Play the game start to finish without any additional hints/guidance and review it play-by-play in the library with an AI tutor
* Play the game with an in-game tutor to get guidance on the best bid/play for the current stage of the game. 
* Activate a Monte Carlo sampling engine that predicts HCP card probabilities and distribution for hidden hands to improve your own intuition and card counting skills.

> [!NOTE]
> These features are meant as a learning tool to build up the underlying skills. These tools will not be available in a real game at a club/tournament, so do not over rely on it to play the game.
>
> This is not a bridge game engine or a hand solver -- see other projects like [Ben](https://github.com/lorserker/ben) or [DDS](https://privat.bahnhof.se/wb758135/bridge/index.html) for that. This uses Anthropic LLMs with tuned prompta for AI bidding and gameplay, so usual caveats with LLMs apply.

## Setup

You will need:
* Rust 1.8+
* Anthropic Claude Code -- either the `claude` CLI installed or an API key

This app was built and tested only on macOS but it should work on linux with appropriate modifications to the commands below 

```bash
brew install rust
cargo install --git https://github.com/rsmenon/bridgette.git
```

On first run, Bridgette creates a config file at `~/.config/bridgette/settings.yaml`. Set your API key there or export it as an environment variable:

```bash
export ANTHROPIC_API_KEY=sk-ant-...
```

The settings file also lets you pick models for each seat and the tutor:

```yaml
api:
  provider: anthropic
  api_key: "${ANTHROPIC_API_KEY}"
agents:
  north:
    model: claude-opus-4-6
  east:
    model: claude-opus-4-6
  west:
    model: claude-opus-4-6
review:
  model: claude-opus-4-6
  enabled: true
```

Backend auto-detection: if an API key is present, Bridgette uses the Anthropic API directly. Otherwise it looks for the `claude` CLI. If neither is available, it won't start.

## Controls

| Key | Action |
|-----|--------|
| <kbd>N</kbd> | New game |
| <kbd>L</kbd> | Open game library |
| <kbd>B</kbd> | Toggle probabilities | 
| <kbd>T</kbd> | Toggle tutor panel |
| <kbd>?</kbd> | Help |
| <kbd>Q</kbd> | Quit |
| Arrow keys | Navigate hand / bid selector |
| <kbd>Enter</kbd> | Play card / place bid |

When N/S is declaring, you control both hands — South as declarer or dummy, and North as the other. East/West always play themselves.

## How it works

The game engine is a standalone state machine that handles dealing, auction validation, trick-taking, and scoring independently of the AI layer. The agent system runs LLM calls in background threads so the UI stays responsive. Each AI turn gets up to 3 retries; if all fail, it falls back to a safe default (Pass during bidding, first legal card during play). Errors show up in the bottom-left panel so you can see what happened.

Prompts are structured with a system message containing the full SAYC reference, plus a turn-specific message with the hand, auction history, valid moves, and situational reminders. The engine keeps the AI honest — it can only make legal bids and play eligible cards.

The probability display (toggle with `B`) estimates where each hidden card likely lives. It works by reading the auction — each bid implies constraints on HCP and suit lengths per SAYC — then running thousands of random deals that satisfy those constraints and the cards already played. The fraction of deals where a given card lands with a given opponent is that card's displayed probability. For small endgame positions it switches to exact enumeration instead of sampling.

Games are saved as JSON in `~/.config/bridgette/data/`. The review system reconstructs game state at any point by replaying the recorded bids and cards through the engine.

## Screenshots

Feature/Screen | Screenshot
----|----
Gameplay | <img width="600" src="https://github.com/user-attachments/assets/606fb810-3a32-48e6-af6d-a1896085cc59" />
Live Tutor | <img width="600" src="https://github.com/user-attachments/assets/ea10ee97-4a65-4ea0-910a-ff30ffa8c78f" />
Estimate HCP Cards & Distribution | <img width="600" src="https://github.com/user-attachments/assets/2518e27a-f6a9-4262-b540-95b5b3d18e06" />
Library | <img width="600" src="https://github.com/user-attachments/assets/c586e920-1736-4efe-b7f2-d47ace6de7fa" />
Post-game review | <img width="600" src="https://github.com/user-attachments/assets/50e1853f-3767-4d6c-96d2-e4e904369ae3" />


