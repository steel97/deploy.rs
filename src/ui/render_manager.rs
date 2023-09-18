use crate::{
    core::constants::VERSION,
    states::ui_state::{UIScreen, UIStore},
};
use futures::lock::Mutex;
use ratatui::{
    prelude::{Backend, Constraint, Corner, Direction, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, LineGauge, List, ListItem, Paragraph, Scrollbar},
    Frame,
};
use std::sync::Arc;

pub async fn render_ui<'a, B: 'a + Backend>(
    frame: &mut Frame<'a, B>,
    ui_state: Arc<Mutex<UIStore>>,
) -> () {
    let mut ui_read = ui_state.lock().await;
    let mut chunk = 0;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Min(2)].as_ref())
        .split(frame.size());

    let paragraph = format!("DEPLOY.RS {}", VERSION);
    let motd = Paragraph::new(String::from("State: starting deployment"))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(paragraph)
                .style(Style::default().fg(Color::LightGreen)),
        )
        .style(Style::default().fg(Color::Yellow)); // Lightgreen?
    frame.render_widget(motd, chunks[chunk]);
    chunk += 1;

    /*if matches!(ui_read.screen, UIScreen::TARGET_START) {
        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Yellow))
            .percent(30);
        frame.render_widget(gauge, chunks[chunk]);
        chunk += 1;
    }*/

    let create_block = |title| {
        Block::default()
            .borders(Borders::ALL)
            .gray()
            .title(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            ))
    };
    ui_read.vertical_scroll_state = ui_read.vertical_scroll_state.content_length(100);
    let paragraph = Paragraph::new("State\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nS3tate\nStat2e\nState\nState\nState\nState\nState\nState\nState1\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\n")
        .gray()
        .block(create_block("Deployment targets"))
        .scroll((ui_read.vertical_scroll as u16, 0));
    frame.render_widget(paragraph, chunks[chunk]);
    frame.render_stateful_widget(
        Scrollbar::default()
            .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        chunks[chunk],
        &mut ui_read.vertical_scroll_state,
    );

    /*let events: Vec<ListItem> = vec![];
    let gaugert = LineGauge::default()
        .block(Block::default().borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Yellow))
        .ratio(0.5);
    events.push(ListItem::new(vec![
        Line::from("-".repeat(chunks[1].width as usize)),
        Line::from(""),
        gaugert,
    ]));

    let events_list = List::new(events)
        .block(Block::default().borders(Borders::ALL).title("Targets"))
        .start_corner(Corner::BottomLeft);
    frame.render_widget(events_list, chunks[chunk]);*/

    /*let label = format!("{}/100", 50);
    let gauge = Gauge::default()
        .block(Block::default().title("Gauge2").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::Green))
        .percent(50)
        .label(label);
    f.render_widget(gauge, chunks[2]);*/
}
