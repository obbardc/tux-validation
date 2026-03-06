use clap::Parser;
use colored::*;
use std::fs;
use std::time::Instant;
use tux_validation::config::Config;
use tux_validation::i2c::audit_all_i2c_buses;
use tux_validation::report::{
    evaluate_i2c_blueprint, evaluate_usb_blueprint, generate_junit_xml, print_annotated_i2c,
    print_annotated_usb_tree, print_xml_summary,
};
use tux_validation::usb::audit_usb_subsystem;

#[derive(Parser)]
#[command(author, version, about = "udev Subsystems Audit")]
struct Args {
    /// Path to expected configuration (optional)
    config: Option<String>,

    /// Audit USB Subsystem
    #[arg(long)]
    xml_report: Option<String>,

    #[arg(long)]
    xml_summary: bool,

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
        let i2c_start = Instant::now();
        let i2c_buses = audit_all_i2c_buses(args.hw_probe)?;
        let i2c_scan_duration = i2c_start.elapsed();
        let i2c_results = evaluate_i2c_blueprint(&i2c_buses, &config.i2c_devices);
        print_annotated_i2c(&i2c_buses, &i2c_results);
        if let Some(filepath) = args.xml_report.clone() {
            generate_junit_xml(&i2c_results, &filepath, Some(i2c_scan_duration))?;
            if args.xml_summary {
                print_xml_summary(&filepath)?;
            }
        }
        //print_and_verify_i2c(&i2c_buses, &config.i2c_devices);
    }

    // USB Audit
    if args.usb || scan_all {
        let usb_start = Instant::now();
        let usb_buses = audit_usb_subsystem()?;
        let usb_scan_duration = usb_start.elapsed();
        let usb_results = evaluate_usb_blueprint(&usb_buses, &config.usb_devices);
        print_annotated_usb_tree(&usb_buses, &usb_results, args.serial);
        if let Some(filepath) = args.xml_report {
            generate_junit_xml(&usb_results, &filepath, Some(usb_scan_duration))?;
            if args.xml_summary {
                print_xml_summary(&filepath)?;
            }
        }
    }

    Ok(())
}
