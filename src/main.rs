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
use std::{env, error::Error, io::Stdout, process::ExitCode};
use std::{io, sync::Arc};

#[tokio::main]
async fn main() -> ExitCode {
    // load deploy configuration
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Error! No config specified.");
        return ExitCode::from(1);
    }

    if !std::path::Path::new(&args[1]).exists() {
        println!("Error! Config {} doesn't exists", &args[1]);
        return ExitCode::from(2);
    }

    let config = Arc::new(Mutex::new(Config::read_config(&args[1]).unwrap()));
    //println!("test {}", config.read().unwrap().use_sudo.unwrap_or(false));

    // create states
    let mut exit_on_finish = false;
    if args.len() > 2 {
        exit_on_finish = args[2].parse().unwrap_or(false);
    }

    let ui_state: Arc<Mutex<UIStore>>;
    {
        let config_locked = config.lock().await;
        ui_state = Arc::new(Mutex::new(
            ui_state::UIStore::new()
                .set_targets_count(config_locked.targets.len() as u32)
                .set_packages_count(config_locked.packages.len() as u32)
                .set_deployed_count(0)
                .set_exit_on_finish(exit_on_finish)
                .finalize(),
        ));
    }

    // run deployment thread
    {
        let mut ui_state_wr = ui_state.lock().await;
        ui_state_wr.set_screen(UIScreen::TARGET_START);
    }

    tokio::spawn(deployment::begin_deployment(config, ui_state.clone()));

    // create cool UI
    let term_res = setup_terminal();
    match term_res {
        Ok(mut terminal) => {
            let _ = run(&mut terminal, ui_state.clone()).await;
            let _ = restore_terminal(&mut terminal);
        }
        _ => {
            println!("Error occured!");
        }
    }

    ExitCode::from(0)
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
        {
            let ui_read_l = ui_state.lock().await;
            if matches!(ui_read_l.screen, UIScreen::FINISHED_END) && ui_read_l.exit_on_finish {
                break;
            }
        }
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
                        code: KeyCode::Char('q'),
                        kind: KeyEventKind::Press,
                        ..
                    } => {
                        break;
                    }
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    } => {
                        let mut ui_read = ui_state.lock().await;
                        if ui_read.vertical_scroll < ui_read.vertical_scroll_max {
                            ui_read.vertical_scroll = ui_read.vertical_scroll.saturating_add(1);
                            ui_read.vertical_scroll_state = ui_read
                                .vertical_scroll_state
                                .position(ui_read.vertical_scroll as usize);
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Up, ..
                    } => {
                        let mut ui_read = ui_state.lock().await;
                        ui_read.vertical_scroll = ui_read.vertical_scroll.saturating_sub(1);
                        ui_read.vertical_scroll_state = ui_read
                            .vertical_scroll_state
                            .position(ui_read.vertical_scroll as usize);
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
