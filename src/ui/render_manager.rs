use crate::{
    core::constants::VERSION,
    states::ui_state::{UIScreen, UIStore},
};
use futures::lock::Mutex;
use ratatui::{
    prelude::{Backend, Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};
use std::sync::Arc;

pub async fn render_ui<'a, B: 'a + Backend>(
    frame: &mut Frame<'a, B>,
    ui_state: Arc<Mutex<UIStore>>,
) -> () {
    let ui_read = &ui_state.lock().await;
    let mut chunk = 0;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Min(0),
            ]
            .as_ref(),
        )
        .split(frame.size());

    let motd = Paragraph::new(format!("DEPLOY.RS ver. {}", VERSION))
        .style(Style::default().fg(Color::LightGreen));
    frame.render_widget(motd, chunks[chunk]);
    chunk += 1;

    let info_targets = Paragraph::new(format!("Loaded {} targets", ui_read.targets_count));
    frame.render_widget(info_targets, chunks[chunk]);
    chunk += 1;

    let info_packages = Paragraph::new(format!("Loaded {} packages", ui_read.packages_count));
    frame.render_widget(info_packages, chunks[chunk]);
    chunk += 1;

    let deploy_state = Paragraph::new(String::from("Starting deployment"))
        .style(Style::default().fg(Color::LightGreen));
    frame.render_widget(deploy_state, chunks[chunk]);
    chunk += 1;

    let deploy_state = Paragraph::new(String::from(format!(
        "Deploying target {}",
        ui_read.deployment_target
    )))
    .style(Style::default().fg(Color::Yellow));
    frame.render_widget(deploy_state, chunks[chunk]);
    chunk += 1;

    let deploy_state = Paragraph::new(String::from("[1/4] computing checksum"))
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(deploy_state, chunks[chunk]);
    chunk += 1;

    if matches!(ui_read.screen, UIScreen::TARGET_START) {
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Yellow))
            .percent(30);
        frame.render_widget(gauge, chunks[chunk]);
        //chunk += 1;
    }

    /*let label = format!("{}/100", 50);
    let gauge = Gauge::default()
        .block(Block::default().title("Gauge2").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::Green))
        .percent(50)
        .label(label);
    f.render_widget(gauge, chunks[2]);*/
}
