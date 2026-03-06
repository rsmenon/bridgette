# ♠️♥️ Bridgette ♦️♣️

A terminal-based contract bridge game where you play South with an AI partner at North and two AI opponents at East/West.

All three AI players bid and play using Standard American Yellow Card (SAYC) conventions. An in-game tutor watches over your shoulder and explains what's happening.

<img width="1460" height="914" alt="Screenshot 2026-03-06 at 00 12 09" src="https://github.com/user-attachments/assets/c3ab12b1-4967-4f13-b937-6959e83dd110" />

## Features

- **Full contract bridge**: Bidding, play, and scoring with proper follow-suit rules, trick-taking, and contract evaluation
- **AI opponents and partner**: Each seat runs its own Claude prompt with SAYC knowledge, including situational awareness for conventions like Stayman, Jacoby Transfers, and Blackwood
- **In-game tutor**: Press `T` to open the tutor panel. It automatically offers advice when it's your turn — explaining what to bid or which card to play and why
- **Game library**: Games auto-save as you play. Browse, resume, favorite, and delete from the library screen
- **Post-game review**: Step through completed games bid-by-bid and card-by-card, with the tutor available to explain each decision
- **Customizable partner**: Give North custom instructions in `settings.yaml` to change your partner's bidding style or personality


## Setup

You need a Claude API key or the `claude` CLI installed.

```bash
cargo build --release
./target/release/bridgette
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
    custom_instructions: ""
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
| Arrow keys | Navigate hand / bid selector |
| Enter | Play card / place bid |
| `T` | Toggle tutor panel |
| `PageUp/Down` | Scroll tutor response |
| `L` | Open game library |
| `N` | New game |
| `?` | Help |
| `Q` | Quit |

When N/S is declaring, you control both hands — South as declarer or dummy, and North as the other. East/West always play themselves.

## How it works

The game engine is a standalone state machine that handles dealing, auction validation, trick-taking, and scoring independently of the AI layer. The agent system runs LLM calls in background threads so the UI stays responsive. Each AI turn gets up to 3 retries; if all fail, it falls back to a safe default (Pass during bidding, first legal card during play). Errors show up in the bottom-left panel so you can see what happened.

Prompts are structured with a system message containing the full SAYC reference, plus a turn-specific message with the hand, auction history, valid moves, and situational reminders. The engine keeps the AI honest — it can only make legal bids and play eligible cards.

Games are saved as JSON in `~/.config/bridgette/data/`. The review system reconstructs game state at any point by replaying the recorded bids and cards through the engine.

## Limitations

- No vulnerability tracking (always scores as not vulnerable)
- Requires a Claude backend (no offline play)

