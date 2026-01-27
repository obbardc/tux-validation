use clap::Parser;
use tux_validation::i2c::{LinuxI2cScanner, validate_bus};

#[derive(Parser)]
#[command(author, version, about = "Verifies I2C device addresses")]
struct Args {
    /// I2C BUS ID (e.g., 0)
    #[arg(short, long)]
    bus_id: u8,

    /// One or more device addresses (e.g., 0x1b 0x50)
    #[arg(short, long, value_parser = parse_hex, num_args = 1..)]
    addresses: Vec<u16>,
}

/// Helper to parse hex strings into u16
fn parse_hex(s: &str) -> Result<u16, String> {
    u16::from_str_radix(s.trim_start_matches("0x"), 16)
        .map_err(|e| format!("Invalid hex address '{}': {}", s, e))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let scanner = LinuxI2cScanner { bus_id: args.bus_id };

    println!("Checking I2C Bus {}...", args.bus_id.to_string());
    let report = validate_bus(&scanner, &args.addresses)?;

    for addr in &report.present {
        println!("Found expected device at 0x{:02x}", addr);
    }
    
    for addr in &report.missing {
        println!("FAILED: Expected device at 0x{:02x} not found!", addr);
    }

    if !report.unexpected.is_empty() {
        println!("Found extra/unknown devices: {:02x?}", report.unexpected);
    }

    for addr in &report.probed {
        println!("Device at 0x{:02x} answered smbus quick_write", addr);
    }

    //if report.missing.is_empty() {
    //    println!("Bus {}: HEALTHY", args.bus_id.to_string());
    //} else {
    //    std::process::exit(1);
    //}

    Ok(())
}