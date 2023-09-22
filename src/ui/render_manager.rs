use crate::{
    core::constants::VERSION,
    states::ui_state::{UIScreen, UIStore, UITargetState},
};
use futures::lock::Mutex;
use ratatui::{
    prelude::{Backend, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Gauge, Paragraph, Scrollbar},
    Frame,
};
use std::{cmp::min, sync::Arc};

pub fn convert_target_state_to_str(
    state: UITargetState,
    upload_package: String,
    upload_pos: u64,
    upload_len: u64,
) -> Result<String, ()> {
    Ok(match state {
        UITargetState::TARGET_START => "[1/5] starting deployment".to_string(),
        UITargetState::TARGET_CHECKSUM => {
            format!("[2/5] computing checksum {}", upload_package)
        }
        UITargetState::TARGET_UPLOADING => {
            format!(
                "[3/5] uploading {} ({}/{})",
                upload_package, upload_pos, upload_len
            )
        }
        UITargetState::TARGET_NO_CHANGES => {
            format!("[3/5] no changes for {}", upload_package)
        }
        UITargetState::TARGET_FINISHING => "[4/5] finishing deployment".to_string(),
        UITargetState::TARGET_FINISHED => "[5/5] finished".to_string(),
    })
}

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
    let mut state_color = Color::LightYellow;
    match ui_read.screen {
        UIScreen::TARGET_START => {
            state = "State: starting deployment";
        }
        UIScreen::FINISHED | UIScreen::FINISHED_END => {
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

    let spacing = 4;
    let mut render_index = 0;
    let mut el_index = 0;
    let mut first_el_width = 0;
    let mut second_el_width = 0;

    // calculate column widths
    for render_entry in ui_read.deployment_targets.iter() {
        el_index = el_index + 1;
        //println!("{}", render_entry.0);
        if render_entry.0.clone() < start_from {
            //&start_from {
            continue;
        }

        if render_index >= max_elements {
            break;
        }
        // target name
        let target_name = format!("{}. {}", el_index, render_entry.1.name);

        // state label
        let state_str = convert_target_state_to_str(
            render_entry.1.state, //UITargetState::TARGET_FINISHING,
            render_entry.1.upload_package.to_string(),
            render_entry.1.upload_pos,
            render_entry.1.upload_len,
        )
        .unwrap();

        let f_len = target_name.len() + spacing;
        let s_len = state_str.len() + spacing;

        if first_el_width < f_len {
            first_el_width = f_len;
        }

        if second_el_width < s_len {
            second_el_width = s_len;
        }

        render_index = render_index + 1;
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
        let mut element_index = 0;
        /*
            let element_count = 3;
            let element_width = min(
            area.width / element_count - area_margin * (element_count + 1),
            40,
        );*/
        let element_width = first_el_width as u16;

        // target name
        let target_name = Paragraph::new(format!("{}. {}", el_index, render_entry.1.name)).gray();
        frame.render_widget(
            target_name,
            Rect::new(
                area.x + (area_margin * (element_index + 1)) + element_width * element_index,
                area.y + y_offset,
                first_el_width as u16,
                element_height,
            ),
        );

        // state label
        element_index = element_index + 1;
        let element_width: u16 = second_el_width as u16;
        let state_str = convert_target_state_to_str(
            render_entry.1.state,
            render_entry.1.upload_package.to_string(),
            render_entry.1.upload_pos,
            render_entry.1.upload_len,
        )
        .unwrap();
        let mut state_label = Paragraph::new(state_str).gray();
        if matches!(render_entry.1.state, UITargetState::TARGET_FINISHED) {
            state_label = state_label.light_green();
        }

        frame.render_widget(
            state_label,
            Rect::new(
                area.x
                    + (area_margin * (element_index + 1))
                    + (first_el_width as u16) * element_index,
                area.y + y_offset,
                element_width,
                element_height,
            ),
        );

        // progress bar for uploading
        if matches!(render_entry.1.state, UITargetState::TARGET_UPLOADING) {
            element_index = element_index + 1;
            let percent = render_entry.1.upload_pos as f64 / render_entry.1.upload_len as f64;
            let gauge = Gauge::default()
                .block(Block::default())
                .gauge_style(Style::default().fg(Color::LightYellow).bg(Color::DarkGray))
                .percent((percent * 100.0) as u16);

            frame.render_widget(
                gauge,
                Rect::new(
                    area.x
                        + (area_margin * (element_index + 1))
                        + element_width
                        + (first_el_width as u16),
                    area.y + y_offset,
                    element_width,
                    element_height,
                ),
            );
        }

        render_index = render_index + 1;
    }
}
