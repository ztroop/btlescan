use std::collections::HashMap;

use ratatui::layout::Rect;

use crate::{company_codes::get_company_name, structs::ManufacturerData};

/// Extracts manufacturer data for display. When multiple entries exist, uses the
/// smallest company ID for deterministic display; hex data is joined from all entries.
pub fn extract_manufacturer_data(manufacturer_data: &HashMap<u16, Vec<u8>>) -> ManufacturerData {
    let mut c = None;
    let mut m = manufacturer_data
        .iter()
        .map(|(&key, value)| {
            c = c.map_or(Some(key), |prev: u16| Some(prev.min(key)));
            bytes_to_hex(value)
        })
        .collect::<Vec<String>>()
        .join(" ");
    m = if m.is_empty() { "n/a".to_string() } else { m };
    match c {
        Some(code) => ManufacturerData {
            company_code: get_company_name(code).to_string(),
            data: m,
        },
        None => ManufacturerData {
            company_code: "n/a".to_string(),
            data: m,
        },
    }
}

pub fn centered_rect(percent_x: u16, percent_y: u16, size: Rect) -> Rect {
    let w = (size.width as u32 * percent_x as u32 / 100).min(u16::MAX as u32) as u16;
    let h = (size.height as u32 * percent_y as u32 / 100).min(u16::MAX as u32) as u16;
    let popup_size = Rect {
        width: w,
        height: h,
        ..Rect::default()
    };
    Rect {
        x: size.width.saturating_sub(w) / 2,
        y: size.height.saturating_sub(h) / 2,
        ..popup_size
    }
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Maximum hex input length (characters). Limits allocation to prevent DoS.
const MAX_HEX_INPUT_LEN: usize = 16 * 1024;

/// Converts a hex string to bytes. Input is limited to MAX_HEX_INPUT_LEN characters.
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }
    if cleaned.len() > MAX_HEX_INPUT_LEN {
        return Err(format!(
            "Hex string exceeds maximum length of {} characters",
            MAX_HEX_INPUT_LEN
        ));
    }
    if !cleaned.len().is_multiple_of(2) {
        return Err("Hex string must have even number of characters".to_string());
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|e| format!("Invalid hex at position {i}: {e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bytes_to_hex_empty() {
        assert_eq!(bytes_to_hex(&[]), "");
    }

    #[test]
    fn test_bytes_to_hex_single() {
        assert_eq!(bytes_to_hex(&[0x00]), "00");
        assert_eq!(bytes_to_hex(&[0xFF]), "FF");
        assert_eq!(bytes_to_hex(&[0x0A]), "0A");
    }

    #[test]
    fn test_bytes_to_hex_multiple() {
        assert_eq!(bytes_to_hex(&[0x00, 0x50]), "00 50");
        assert_eq!(bytes_to_hex(&[0xDE, 0xAD, 0xBE, 0xEF]), "DE AD BE EF");
    }

    #[test]
    fn test_hex_to_bytes_empty() {
        assert_eq!(hex_to_bytes("").unwrap(), Vec::<u8>::new());
        assert_eq!(hex_to_bytes("  ").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn test_hex_to_bytes_valid() {
        assert_eq!(hex_to_bytes("00").unwrap(), vec![0x00]);
        assert_eq!(hex_to_bytes("FF").unwrap(), vec![0xFF]);
        assert_eq!(hex_to_bytes("0050").unwrap(), vec![0x00, 0x50]);
        assert_eq!(
            hex_to_bytes("DEADBEEF").unwrap(),
            vec![0xDE, 0xAD, 0xBE, 0xEF]
        );
    }

    #[test]
    fn test_hex_to_bytes_with_spaces() {
        assert_eq!(hex_to_bytes("00 50").unwrap(), vec![0x00, 0x50]);
        assert_eq!(
            hex_to_bytes("DE AD BE EF").unwrap(),
            vec![0xDE, 0xAD, 0xBE, 0xEF]
        );
    }

    #[test]
    fn test_hex_to_bytes_case_insensitive() {
        assert_eq!(hex_to_bytes("ff").unwrap(), vec![0xFF]);
        assert_eq!(hex_to_bytes("Ff").unwrap(), vec![0xFF]);
    }

    #[test]
    fn test_hex_to_bytes_odd_length() {
        assert!(hex_to_bytes("F").is_err());
        assert!(hex_to_bytes("ABC").is_err());
    }

    #[test]
    fn test_hex_to_bytes_invalid_chars() {
        assert!(hex_to_bytes("GG").is_err());
        assert!(hex_to_bytes("ZZZZ").is_err());
    }

    #[test]
    fn test_hex_to_bytes_exceeds_max_length() {
        let long = "00".repeat(MAX_HEX_INPUT_LEN / 2 + 1);
        assert!(hex_to_bytes(&long).is_err());
        let at_limit = "00".repeat(MAX_HEX_INPUT_LEN / 2);
        assert!(hex_to_bytes(&at_limit).is_ok());
    }

    #[test]
    fn test_roundtrip() {
        let original = vec![0x00, 0x50, 0xDE, 0xAD];
        let hex = bytes_to_hex(&original);
        let decoded = hex_to_bytes(&hex).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_centered_rect() {
        let size = Rect::new(0, 0, 100, 50);
        let result = centered_rect(60, 60, size);
        assert_eq!(result.width, 60);
        assert_eq!(result.height, 30);
        assert_eq!(result.x, 20);
        assert_eq!(result.y, 10);
    }

    #[test]
    fn test_centered_rect_full() {
        let size = Rect::new(0, 0, 100, 50);
        let result = centered_rect(100, 100, size);
        assert_eq!(result.width, 100);
        assert_eq!(result.height, 50);
        assert_eq!(result.x, 0);
        assert_eq!(result.y, 0);
    }

    #[test]
    fn test_extract_manufacturer_data_empty() {
        let data: HashMap<u16, Vec<u8>> = HashMap::new();
        let result = extract_manufacturer_data(&data);
        assert_eq!(result.company_code, "n/a");
        assert_eq!(result.data, "n/a");
    }

    #[test]
    fn test_extract_manufacturer_data_single() {
        let mut data = HashMap::new();
        data.insert(0x004C, vec![0x02, 0x15]); // Apple
        let result = extract_manufacturer_data(&data);
        assert_eq!(result.company_code, "Apple, Inc.");
        assert_eq!(result.data, "02 15");
    }

    #[test]
    fn test_extract_manufacturer_data_multiple_deterministic() {
        let mut data = HashMap::new();
        data.insert(0x004C, vec![0x01]); // Apple
        data.insert(0x0000, vec![0x02]); // Ericsson
        data.insert(0x0002, vec![0x03]); // Intel
        let result = extract_manufacturer_data(&data);
        assert_eq!(result.company_code, "Ericsson AB"); // smallest ID
        assert!(result.data.contains("01"));
        assert!(result.data.contains("02"));
        assert!(result.data.contains("03"));
    }
}
