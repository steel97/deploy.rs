use crate::{
    core::constants::VERSION,
    states::ui_state::{UIScreen, UIStore, UITargetState},
};
use futures::lock::Mutex;
use ratatui::{
    prelude::{Backend, Constraint, Corner, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, LineGauge, List, ListItem, Paragraph, Scrollbar},
    Frame,
};
use std::{cmp::min, sync::Arc};

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
    let mut state = "State: loading config";
    let mut state_color = Color::Yellow;
    match ui_read.screen {
        UIScreen::TARGET_START => {
            state = "State: starting deployment";
        }
        UIScreen::FINISHED => {
            state = "State: deployment finished";
            state_color = Color::LightGreen;
        }
        _ => {}
    }

    let motd = Paragraph::new(String::from(state))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(paragraph)
                .style(Style::default().fg(Color::LightGreen)),
        )
        .style(Style::default().fg(state_color)); // Lightgreen?
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

    /*let create_block = |title| {
        Block::default()
            .borders(Borders::ALL)
            .gray()
            .title(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            ))
    };*/

    let area = chunks[chunk];
    let block = Block::default()
        .borders(Borders::ALL)
        .gray()
        .title("Deployment targets");
    //.scroll((ui_read.vertical_scroll as u16, 0));
    frame.render_widget(block, area);

    let element_height = 1;
    let y_area_margin = 1;
    let el_per_scroll = element_height + y_area_margin;
    let start_from = (ui_read.vertical_scroll / el_per_scroll) as u32;
    let max_elements = (area.height - (y_area_margin * 2 * 2)) / (element_height + y_area_margin);

    ui_read.vertical_scroll_state = ui_read
        .vertical_scroll_state
        .content_length(el_per_scroll * max_elements);
    ui_read.vertical_scroll_max = el_per_scroll * max_elements;
    /*let paragraph = Paragraph::new("State\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nS3tate\nStat2e\nState\nState\nState\nState\nState\nState\nState1\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\nState\n")
    .gray()
    .block(create_block("Deployment targets"))
    .scroll((ui_read.vertical_scroll as u16, 0));*/

    //frame.render_widget(paragraph, chunks_inner[0]);
    frame.render_stateful_widget(
        Scrollbar::default()
            .orientation(ratatui::widgets::ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        chunks[chunk],
        &mut ui_read.vertical_scroll_state,
    );

    if area.height < 10 {
        return;
    }

    let mut render_index = 0;
    let mut el_index = 0;
    for render_entry in ui_read.deployment_targets.iter() {
        el_index = el_index + 1;
        let y_offset = y_area_margin * 2 + render_index * (element_height + y_area_margin);
        //println!("{}", render_entry.0);
        if render_entry.0.clone() < start_from {
            //&start_from {
            continue;
        }

        if render_index >= max_elements {
            break;
        }

        // basic container helpers
        let area_margin = 2;
        let element_count = 3;
        let mut element_index = 0;
        let element_width = min(
            area.width / element_count - area_margin * (element_count + 1),
            40,
        );

        // target name
        let target_name = Paragraph::new(format!("{}. {}", el_index, render_entry.1.name)).gray();
        frame.render_widget(
            target_name,
            Rect::new(
                area.x + (area_margin * (element_index + 1)) + element_width * element_index,
                area.y + y_offset,
                element_width,
                element_height,
            ),
        );

        // state label
        element_index = element_index + 1;
        let mut state_str = String::new();
        match render_entry.1.state {
            UITargetState::TARGET_START => {
                state_str = "[1/5] starting deployment".to_string();
            }
            UITargetState::TARGET_CHECKSUM => {
                state_str = format!("[2/5] computing checksum {}", render_entry.1.upload_package);
            }
            UITargetState::TARGET_UPLOADING => {
                state_str = format!(
                    "[3/5] uploading {} ({}/{})",
                    render_entry.1.upload_package,
                    render_entry.1.upload_pos,
                    render_entry.1.upload_len
                );
            }
            UITargetState::TARGET_NO_CHANGES => {
                state_str = format!("[3/5] no changes for {}", render_entry.1.upload_package);
            }
            UITargetState::TARGET_FINISHING => {
                state_str = "[4/5] finishing deployment".to_string();
            }
            UITargetState::TARGET_FINISHED => {
                state_str = "[5/5] finished".to_string();
            }
            _ => {}
        }
        let mut state_label = Paragraph::new(state_str).gray();
        if matches!(render_entry.1.state, UITargetState::TARGET_FINISHED) {
            state_label = state_label.light_green();
        }

        frame.render_widget(
            state_label,
            Rect::new(
                area.x + (area_margin * (element_index + 1)) + element_width * element_index,
                area.y + y_offset,
                element_width,
                element_height,
            ),
        );

        // progress bar for uploading
        if matches!(render_entry.1.state, UITargetState::TARGET_UPLOADING) {
            element_index = element_index + 1;
            let percent = render_entry.1.upload_pos / render_entry.1.upload_len;
            let gauge = Gauge::default()
                .block(Block::default())
                .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
                .percent(percent.try_into().unwrap_or(0));

            frame.render_widget(
                gauge,
                Rect::new(
                    area.x + (area_margin * (element_index + 1)) + element_width * element_index,
                    area.y + y_offset,
                    element_width,
                    element_height,
                ),
            );
        }

        render_index = render_index + 1;
    }

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
