use tux_validation::os_release;

fn main() -> anyhow::Result<()> {
    let osr = os_release::parse_os_release("/etc/os-release").expect("Failed to read os-release");

    println!(
        "ID={}",
        osr.get("ID").map(String::as_str).unwrap_or("<missing>")
    );
    println!(
        "VERSION_CODENAME={}",
        osr.get("VERSION_CODENAME")
            .map(String::as_str)
            .unwrap_or("<missing>")
    );

    Ok(())
}
