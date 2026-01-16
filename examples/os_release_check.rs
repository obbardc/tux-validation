use clap::Parser;
use tux_validation::os_release;

#[derive(Parser)]
#[command(author, version, about = "Validates OS release info")]
struct Args {
    /// Expected OS ID (e.g., debian)
    #[arg(short, long)]
    id: String,

    /// Expected Version Codename (e.g., forky)
    #[arg(short, long)]
    codename: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("--- Starting OS Validation ---");
    let osr = os_release::parse_os_release("/etc/os-release")?;

    let actual_id = osr.get("ID").map(|s| s.as_str()).unwrap_or("unknown");
    let actual_code = osr
        .get("VERSION_CODENAME")
        .map(|s| s.as_str())
        .unwrap_or("unknown");

    if args.id.eq_ignore_ascii_case(actual_id) && args.codename.eq_ignore_ascii_case(actual_code) {
        println!("Validation Passed: {} ({})", actual_id, actual_code);
    } else {
        println!("Validation Failed!");
        println!("Expected: {} ({})", args.id, args.codename);
        println!("Actual:   {} ({})", actual_id, actual_code);
        std::process::exit(1); // Return non-zero to signify failure
    }

    Ok(())
}
