use clap::Parser;
use std::fs;
use tux_validation::usb::{Config, audit_usb_subsystem, print_and_verify_usb};

#[derive(Parser)]
#[command(author, version, about = "Performs USB subsystem audit.")]
struct Args {
    /// Path to expected configuration
    config: String,

    /// Print serial ID
    #[arg(long)]
    serial: bool,

    /// Print debug info
    #[arg(long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Load the blueprint from a file
    let config_str = fs::read_to_string(args.config)?;
    let config: Config = toml::from_str(&config_str)?;

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
    print_and_verify_usb(&buses, &config.usb_devices, args.serial);

    Ok(())
}
