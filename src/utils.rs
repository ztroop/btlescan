use std::collections::HashMap;

use crate::company_codes::COMPANY_CODE;

/// Extracts the manufacturer data from a `HashMap<u16, Vec<u8>>` and returns a tuple with the company name and the manufacturer data as a string.
/// If the manufacturer data is empty, it returns "n/a" as the company name and the manufacturer data.
/// If the company code is not found in the `company_codes` module, it returns "n/a" as the company name.
pub fn extract_manufacturer_data(manufacturer_data: &HashMap<u16, Vec<u8>>) -> (String, String) {
    let mut c = None;
    let mut m = manufacturer_data
        .iter()
        .map(|(&key, value)| {
            c = Some(key);
            let hex_string = value
                .iter()
                .map(|byte| format!("{:02X}", byte))
                .collect::<Vec<String>>()
                .join(" ");
            hex_string.to_string()
        })
        .collect::<Vec<String>>()
        .join(" ");
    m = if m.is_empty() { "n/a".to_string() } else { m };
    match c {
        Some(code) => (COMPANY_CODE.get(&code).unwrap_or(&"n/a").to_string(), m),
        None => ("n/a".to_string(), m),
    }
}
