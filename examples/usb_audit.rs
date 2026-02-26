use clap::Parser;
use std::fs;
use tux_validation::usb::{audit_usb_subsystem, print_and_verify_usb};
use tux_validation::config::Config;

#[derive(Parser)]
#[command(author, version, about = "Performs USB subsystem audit.")]
struct Args {
    /// Path to expected configuration (optional)
    config: Option<String>,

    /// Print serial ID
    #[arg(long)]
    serial: bool,

    /// Print debug info
    #[arg(long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Optionally load the blueprint from a file
    let blueprint = if let Some(path) = args.config {
        let config_str = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&config_str)?;
        config.usb_devices
    } else {
        Vec::new() // Default to an empty list if no config provided
    };

    // Perform the scan
    let buses = audit_usb_subsystem()?;

    if args.verbose {
        println!("DEBUG INFORMATION");
        for bus in &buses {
            println!("--- BUS {} ---", bus.id);
            for dev in &bus.devices {
                dev.print_json()?;
            }
        }
        println!("");
    }

    // Pass the config.usb_devices to your audit function
    print_and_verify_usb(&buses, &blueprint, args.serial);

    Ok(())
}
