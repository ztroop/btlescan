use std::collections::HashSet;

use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
};
use uuid::Uuid;

use crate::{structs::Characteristic, utils::bytes_to_hex};

pub fn characteristic_panel<'a>(
    characteristics: &[Characteristic],
    selected: Option<usize>,
    char_values: &std::collections::HashMap<Uuid, Vec<u8>>,
    subscribed: &HashSet<Uuid>,
    focused: bool,
) -> Table<'a> {
    let border_color = if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let rows: Vec<Row> = characteristics
        .iter()
        .enumerate()
        .map(|(i, ch)| {
            let props: Vec<String> = ch
                .properties
                .iter_names()
                .map(|x| x.0.to_string())
                .collect();
            let props_str = props.join(",");

            let value_str = char_values
                .get(&ch.uuid)
                .map(|v| bytes_to_hex(v))
                .unwrap_or_default();

            let sub_indicator = if subscribed.contains(&ch.uuid) {
                " ●"
            } else {
                ""
            };

            let uuid_str = ch.uuid.to_string();
            let uuid_short: &str = uuid_str.get(..8).unwrap_or(&uuid_str);

            let style = if selected == Some(i) && focused {
                Style::default().add_modifier(Modifier::REVERSED)
            } else if selected == Some(i) {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };

            Row::new(vec![
                format!("{}{}", uuid_short, sub_indicator),
                props_str,
                value_str,
            ])
            .style(style)
        })
        .collect();

    Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(20),
            Constraint::Fill(1),
        ],
    )
    .header(Row::new(vec!["UUID", "Properties", "Value"]).style(Style::default().fg(Color::Yellow)))
    .block(
        Block::default()
            .title("Characteristics")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use btleplug::api::CharPropFlags;
    use std::collections::HashMap;

    fn make_char(uuid_str: &str, props: CharPropFlags) -> Characteristic {
        Characteristic {
            uuid: Uuid::parse_str(uuid_str).unwrap(),
            properties: props,
            descriptors: vec![],
            service: Uuid::parse_str("00001800-0000-1000-8000-00805f9b34fb").unwrap(),
            handle: None,
        }
    }

    #[test]
    fn test_characteristic_panel_empty() {
        let chars: Vec<Characteristic> = vec![];
        let values = HashMap::new();
        let subs = HashSet::new();
        let _table = characteristic_panel(&chars, None, &values, &subs, false);
    }

    #[test]
    fn test_characteristic_panel_with_data() {
        let chars = vec![
            make_char("00002a29-0000-1000-8000-00805f9b34fb", CharPropFlags::READ),
            make_char(
                "00002a37-0000-1000-8000-00805f9b34fb",
                CharPropFlags::NOTIFY,
            ),
        ];
        let mut values = HashMap::new();
        values.insert(
            Uuid::parse_str("00002a29-0000-1000-8000-00805f9b34fb").unwrap(),
            vec![0x41, 0x70],
        );
        let mut subs = HashSet::new();
        subs.insert(Uuid::parse_str("00002a37-0000-1000-8000-00805f9b34fb").unwrap());

        let _table = characteristic_panel(&chars, Some(0), &values, &subs, true);
    }
}
