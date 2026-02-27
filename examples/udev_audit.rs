use clap::Parser;
use colored::*;
use std::fs;
use tux_validation::config::Config;
use tux_validation::i2c::{audit_all_i2c_buses, print_and_verify_i2c};
use tux_validation::usb::{audit_usb_subsystem, print_and_verify_usb};

#[derive(Parser)]
#[command(author, version, about = "udev Subsystems Audit")]
struct Args {
    /// Path to expected configuration (optional)
    config: Option<String>,

    /// Audit USB Subsystem
    #[arg(long)]
    usb: bool,

    /// Audit I2C Subsystem
    #[arg(long)]
    i2c: bool,

    /// Perform hardware probe for I2C (smbus_write_quick)
    #[arg(long)]
    hw_probe: bool,

    /// Print serial IDs (USB)
    #[arg(long)]
    serial: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Load configuration (or use empty default)
    let config = if let Some(path) = args.config {
        let config_str = fs::read_to_string(path)?;
        toml::from_str::<Config>(&config_str)?
    } else {
        Config::default()
    };

    // If no specific subsystem flag is provided, we can default to scanning all
    let scan_all = !args.usb && !args.i2c;

    println!("\n{}", "===== UDEV-AUDIT =====".bold().cyan());

    // I2C Audit
    if args.i2c || scan_all {
        let i2c_buses = audit_all_i2c_buses(args.hw_probe)?;
        print_and_verify_i2c(&i2c_buses, &config.i2c_devices);
    }

    // USB Audit
    if args.usb || scan_all {
        let usb_buses = audit_usb_subsystem()?;
        print_and_verify_usb(&usb_buses, &config.usb_devices, args.serial);
    }

    Ok(())
}
