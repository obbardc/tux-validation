use clap::Parser;
use colored::*;
use std::fs;
use std::time::{Duration, Instant};
use tux_validation::config::Config;
use tux_validation::i2c::audit_all_i2c_buses;
use tux_validation::network::audit_network_subsystem;
use tux_validation::report::{
    generate_junit_xml, print_annotated_i2c, print_annotated_network, print_annotated_usb_tree,
    print_xml_summary,
};
use tux_validation::usb::audit_usb_subsystem;
use tux_validation::validation::{
    ValidationResult, evaluate_i2c_blueprint, evaluate_network_blueprint, evaluate_usb_blueprint,
};

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

    /// Audit Network Subsystem
    #[arg(long)]
    net: bool,

    /// Perform hardware probe for I2C (smbus_write_quick)
    #[arg(long)]
    i2c_hw_probe: bool,

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
    let scan_all = !args.usb && !args.i2c && !args.net;

    println!("\n{}", "===== UDEV-AUDIT =====".bold().cyan());

    // I2C Audit
    let mut i2c_results: Vec<ValidationResult> = Vec::new();
    let mut i2c_scan_duration = Duration::ZERO;
    if args.i2c || scan_all {
        let i2c_start = Instant::now();
        let i2c_buses = audit_all_i2c_buses(args.i2c_hw_probe)?;
        i2c_scan_duration = i2c_start.elapsed();
        i2c_results = evaluate_i2c_blueprint(&i2c_buses, &config.i2c_devices);
        print_annotated_i2c(&i2c_buses, &i2c_results);
    }

    // USB Audit
    let mut usb_results: Vec<ValidationResult> = Vec::new();
    let mut usb_scan_duration = Duration::ZERO;
    if args.usb || scan_all {
        let usb_start = Instant::now();
        let usb_buses = audit_usb_subsystem()?;
        usb_scan_duration = usb_start.elapsed();
        usb_results = evaluate_usb_blueprint(&usb_buses, &config.usb_devices);
        print_annotated_usb_tree(&usb_buses, &usb_results, args.serial);
    }

    // Network Audit
    let mut network_results: Vec<ValidationResult> = Vec::new();
    let mut net_scan_duration = Duration::ZERO;
    if args.net || scan_all {
        let net_start = Instant::now();
        let net_buses = audit_network_subsystem()?;
        net_scan_duration = net_start.elapsed();
        network_results = evaluate_network_blueprint(&net_buses, &config.network_devices);
        print_annotated_network(&net_buses, &network_results);
    }
    let all_results: Vec<ValidationResult> = usb_results
        .into_iter()
        .chain(i2c_results)
        .chain(network_results)
        .collect();

    if let Some(filepath) = args.xml_report.clone() {
        let total_scan_duration = usb_scan_duration + i2c_scan_duration + net_scan_duration;
        generate_junit_xml(&all_results, &filepath, Some(total_scan_duration))?;
        if args.xml_summary {
            print_xml_summary(&filepath)?;
        }
    }

    Ok(())
}
