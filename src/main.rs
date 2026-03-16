mod agent;
mod app;
mod config;
mod engine;
mod types;
mod ui;

use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use agent::tutor::TutorController;
use agent::AgentController;
use app::App;
use config::Settings;

fn main() -> anyhow::Result<()> {
    let settings = Settings::load();

    let api_key = settings
        .resolve_api_key()
        .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok());
    let model = Some(settings.agents.north.model.clone());

    let backend = agent::detect_backend(api_key.clone(), model);

    eprintln!("Agent backend: {}", backend.name());

    let controller = AgentController::new(backend);

    // Build tutor controller: prefer API, fall back to CLI
    let tutor_controller = if settings.review.enabled {
        if let Some(key) = api_key {
            Some(TutorController::new_api(key, settings.review.model.clone()))
        } else if agent::cli_available() {
            Some(TutorController::new_cli(settings.review.model.clone()))
        } else {
            None
        }
    } else {
        None
    };
    if let Some(ref tc) = tutor_controller {
        eprintln!("Tutor: {} ({})", settings.review.model, tc.backend_name());
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, controller, settings, tutor_controller);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    agent: AgentController,
    settings: Settings,
    tutor_controller: Option<TutorController>,
) -> anyhow::Result<()> {
    let mut app = App::new(agent, settings, tutor_controller);
    let tick_rate = Duration::from_millis(33); // ~30fps
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| app.draw(f))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                app.handle_key(key);
            }
        }

        if last_tick.elapsed() >= tick_rate {
            // Tick agents
            app.tick_agents();

            // Tick inference
            app.tick_inference();

            // Tick tutor
            app.tick_tutor();

            // Tick review
            app.tick_review();

            // Tick status message decay
            app.state.tick_status();

            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
