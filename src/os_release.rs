//! OS release and environment parsing utilities.
//!
//! This module contains a lightweight parser for standard Linux `/etc/os-release`
//! key-value files. While originally built as a foundational exercise for the
//! framework, it remains a robust utility for extracting host OS metadata
//! (like OS name, version, and build IDs).

use anyhow::Result;
use std::collections::HashMap;
use std::io::BufRead;

/// Opens and parses a standard Linux `os-release` file into a key-value dictionary.
///
/// # Arguments
/// * `path` - The file path to read (typically `"/etc/os-release"` or `"/usr/lib/os-release"`).
///
/// # Returns
/// A `Result` containing a `HashMap` of the parsed key-value pairs.
pub fn parse_os_release(path: &str) -> Result<HashMap<String, String>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    parse_os_release_from_reader(reader)
}

/// Core parsing logic for `os-release` formatted data.
///
/// Iterates line-by-line over the provided reader, skipping empty lines and `#` comments.
/// It splits valid lines on the first `=` character and safely strips any surrounding
/// single or double quotes from the value.
///
/// # Arguments
/// * `reader` - Any type implementing the `BufRead` trait (such as a `BufReader` over a file,
///   or a `Cursor` over a string in unit tests).
///
/// # Returns
/// A `Result` containing a `HashMap` of the parsed key-value pairs representing the OS metadata.
pub fn parse_os_release_from_reader<R: BufRead>(reader: R) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();

    for line_result in reader.lines() {
        let raw = line_result?;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().trim_matches('"').trim_matches('\'');
            map.insert(k.trim().to_string(), v.to_string());
        }
    }
    Ok(map)
}
