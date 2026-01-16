use std::collections::HashMap;
use std::fs;
use anyhow::Result;

pub fn parse_os_release(path: &str) -> Result<HashMap<String, String>> {
    let text = fs::read_to_string(path)?;
    let mut map = HashMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            let v = v.trim().trim_matches('"').trim_matches('\'');
            map.insert(k.trim().to_string(), v.to_string());
        }
    }
    Ok(map)
}