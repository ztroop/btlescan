use ratatui::{
    layout::Constraint,
    style::{Color, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::{
    structs::DeviceInfo,
    utils::{extract_manufacturer_data, extract_remote_id},
};

pub fn detail_table(selected_device: Option<&DeviceInfo>, is_connected: bool) -> Table<'_> {
    let rows = match selected_device {
        Some(device) => {
            let services_binding = device.services.len().to_string();
            let manufacturer_data = extract_manufacturer_data(&device.manufacturer_data);
            let status = if is_connected {
                ("Connected ●".to_string(), Color::Green)
            } else {
                ("Disconnected".to_string(), Color::DarkGray)
            };
            let mut rows = vec![
                Row::new(vec!["Status:".to_owned(), status.0.clone()])
                    .style(Style::default().fg(status.1)),
                Row::new(vec!["Detected At:".to_owned(), device.detected_at.clone()]),
                Row::new(vec!["Services:".to_owned(), services_binding]),
                Row::new(vec![
                    "Company Code ID:".to_owned(),
                    manufacturer_data.company_code,
                ]),
                Row::new(vec![
                    "Manufacturer Data:".to_owned(),
                    manufacturer_data.data,
                ]),
            ];
            if let Some(remote_id) = extract_remote_id(&device.service_data) {
                rows.push(Row::new(vec!["Remote ID:".to_owned(), remote_id]));
            }
            rows
        }
        None => vec![
            Row::new(vec!["Status:".to_owned(), "No devices detected".to_owned()])
                .style(Style::default().fg(Color::DarkGray)),
        ],
    };

    Table::new(rows, [Constraint::Length(20), Constraint::Length(80)])
        .block(Block::default().title("Details").borders(Borders::ALL))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detail_table_no_devices() {
        let _table = detail_table(None, false);
    }

    #[test]
    fn test_detail_table_with_device() {
        let device = DeviceInfo::default();
        let _table = detail_table(Some(&device), false);
    }
}
