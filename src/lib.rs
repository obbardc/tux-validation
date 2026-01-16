use std::collections::HashMap;
use std::fs;

fn parse_os_release(path: &str) -> anyhow::Result<HashMap<String, String>> {
    let text = fs::read_to_string(path)?;
    let mut map = HashMap::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };

        let v = v.trim().trim_matches('"').trim_matches('\'');
        map.insert(k.trim().to_string(), v.to_string());
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_release_id_and_codename() -> anyhow::Result<()> {
        let osr = parse_os_release("/etc/os-release")?;

        println!("ID={}", osr.get("ID").map(String::as_str).unwrap_or("<missing>"));
        println!(
            "VERSION_CODENAME={}",
            osr.get("VERSION_CODENAME").map(String::as_str).unwrap_or("<missing>")
        );

        assert_eq!(osr.get("ID").map(String::as_str), Some("debian"));
        assert_eq!(osr.get("VERSION_CODENAME").map(String::as_str), Some("forky"));

        Ok(())
    }
}
