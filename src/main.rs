use core::time::Duration;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use deploy::{
    deployment::deployment,
    serialization::config::Config,
    states::{
        base_state::BaseState,
        ui_state::{self, UIScreen, UIStore},
    },
    ui::render_manager::render_ui,
};
use futures::lock::Mutex;
use ratatui::{prelude::CrosstermBackend, Terminal};
use std::{env, error::Error, io::Stdout};
use std::{io, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // load deploy configuration
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        panic!("Error! No config specified.");
    }

    if !std::path::Path::new(&args[1]).exists() {
        panic!("Error! Config {} doesn't exists", &args[0]);
    }

    let config = Arc::new(Mutex::new(Config::read_config(&args[1]).unwrap()));
    //println!("test {}", config.read().unwrap().use_sudo.unwrap_or(false));

    // create states
    let ui_state: Arc<Mutex<UIStore>>;
    {
        let config_locked = config.lock().await;
        ui_state = Arc::new(Mutex::new(
            ui_state::UIStore::new()
                .set_targets_count(config_locked.targets.len() as u32)
                .set_packages_count(config_locked.packages.len() as u32)
                .set_deployed_count(0)
                .finalize(),
        ));
    }

    // run deployment thread
    tokio::spawn(deployment::begin_deployment(config, ui_state.clone()));

    // create cool UI
    let term_res = setup_terminal();
    match term_res {
        Ok(mut terminal) => {
            run(&mut terminal, ui_state.clone()).await?;
            restore_terminal(&mut terminal)?;
        }
        _ => {
            println!("Error occured!");
        }
    }
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, Box<dyn Error>> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn Error>> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    Ok(terminal.show_cursor()?)
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    ui_state: Arc<Mutex<UIStore>>,
) -> Result<(), Box<dyn Error>> {
    loop {
        let cur_frame = &mut terminal.get_frame();
        let frame = render_ui(cur_frame, ui_state.clone()).await;
        terminal.draw(|_| frame)?;
        let mut ui_read = ui_state.lock().await;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match key {
                    KeyEvent {
                        modifiers: KeyModifiers::CONTROL,
                        code: KeyCode::Char('c'),
                        ..
                    } => {
                        break;
                    }
                    KeyEvent {
                        code: KeyCode::Esc,
                        kind: KeyEventKind::Press,
                        ..
                    } => {
                        break;
                    }
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    } => {
                        ui_read.vertical_scroll = ui_read.vertical_scroll.saturating_add(1);
                        ui_read.vertical_scroll_state = ui_read
                            .vertical_scroll_state
                            .position(ui_read.vertical_scroll);
                    }
                    KeyEvent {
                        code: KeyCode::Up, ..
                    } => {
                        ui_read.vertical_scroll = ui_read.vertical_scroll.saturating_sub(1);
                        ui_read.vertical_scroll_state = ui_read
                            .vertical_scroll_state
                            .position(ui_read.vertical_scroll);
                    }
                    KeyEvent {
                        code: KeyCode::Char('a'),
                        kind: KeyEventKind::Press,
                        ..
                    } => {
                        let mut ui_state_wr = ui_state.lock().await;
                        let new_state = if matches!(ui_state_wr.screen, UIScreen::CONFIG) {
                            UIScreen::TARGET_START
                        } else {
                            UIScreen::CONFIG
                        };
                        ui_state_wr.set_screen(new_state);
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
