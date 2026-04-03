use clap::Parser;
use std::fs;
use std::time::{Duration, Instant};
use tux_validation::config::Config;
use tux_validation::i2c::audit_all_i2c_buses;
use tux_validation::network::audit_network_subsystem;
use tux_validation::pcie::audit_pci_subsystem;
use tux_validation::report::{
    generate_junit_xml, print_annotated_i2c, print_annotated_network, print_annotated_pci,
    print_annotated_systemd, print_annotated_usb_tree, print_xml_summary,
};
use tux_validation::systemd::audit_systemd_services;
use tux_validation::usb::audit_usb_subsystem;
use tux_validation::validation::{
    ValidationResult, evaluate_i2c_blueprint, evaluate_network_blueprint, evaluate_pci_blueprint,
    evaluate_systemd_blueprint, evaluate_usb_blueprint,
};

/// A generic pipeline runner that executes a single subsystem audit phase.
///
/// This function acts as the orchestrator for the validation pipeline. It takes
/// the raw scanning function and the validation closures as parameters, ensuring
/// that hardware is only probed if the configuration blueprint actually requested it.
///
/// # Arguments
/// * `should_run` - If `false`, skips the phase entirely (e.g., if the TOML had no USB rules).
/// * `scan` - A closure that executes the OS-level hardware discovery.
/// * `evaluate` - A closure that cross-references the scanned hardware with the TOML blueprint.
/// * `print` - A closure that renders the final colorized output to the terminal.
///
/// # Returns
/// A tuple containing the evaluated `ValidationResult`s and the exact `Duration`
/// it took to perform the OS-level `scan` phase.
fn run_audit_phase<Item>(
    should_run: bool,
    scan: impl FnOnce() -> anyhow::Result<Vec<Item>>,
    evaluate: impl FnOnce(&[Item]) -> Vec<ValidationResult>,
    print: impl FnOnce(&[Item], &[ValidationResult]),
) -> anyhow::Result<(Vec<ValidationResult>, Duration)> {
    if !should_run {
        return Ok((Vec::new(), Duration::ZERO));
    }

    let start = Instant::now();
    let scanned_data = scan()?;
    let duration = start.elapsed();

    let results = evaluate(&scanned_data);
    print(&scanned_data, &results);

    Ok((results, duration))
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Tux Validation: Embedded Linux System Auditor"
)]
struct Args {
    /// Path to the TOML configuration blueprint (e.g., board_config.toml)
    #[arg(short, long)]
    config: String,

    /// Path to output the CI-compatible JUnit XML report
    #[arg(short = 'x', long)]
    xml_report: Option<String>,

    /// Print a high-level summary of the generated XML report to the console
    #[arg(long, requires = "xml_report")]
    xml_summary: bool,

    /// Perform live hardware probing for I2C (requires sudo and i2c-dev module)
    #[arg(long)]
    i2c_hw_probe: bool,

    /// Include internal USB serial IDs in the terminal output
    #[arg(long)]
    usb_print_serial: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("Loading blueprint from: {}", args.config);
    let config_str = fs::read_to_string(&args.config)?;
    let config: Config = toml::from_str(&config_str)?;

    // =========================================================================
    // EXECUTE AUDIT PHASES
    // =========================================================================

    let service_names: Vec<String> = config
        .systemd_services
        .iter()
        .map(|s| s.name.clone())
        .collect();

    let (sys_results, sys_dur) = run_audit_phase(
        !service_names.is_empty(),
        || audit_systemd_services(&service_names),
        |services| evaluate_systemd_blueprint(services, &config.systemd_services),
        print_annotated_systemd,
    )?;

    let (i2c_results, i2c_dur) = run_audit_phase(
        !config.i2c_devices.is_empty(),
        || audit_all_i2c_buses(args.i2c_hw_probe),
        |buses| evaluate_i2c_blueprint(buses, &config.i2c_devices),
        print_annotated_i2c,
    )?;

    let (usb_results, usb_dur) = run_audit_phase(
        !config.usb_devices.is_empty(),
        audit_usb_subsystem,
        |buses| evaluate_usb_blueprint(buses, &config.usb_devices),
        |buses, res| print_annotated_usb_tree(buses, res, args.usb_print_serial),
    )?;

    let (net_results, net_dur) = run_audit_phase(
        !config.network_devices.is_empty(),
        audit_network_subsystem,
        |buses| evaluate_network_blueprint(buses, &config.network_devices),
        print_annotated_network,
    )?;

    let (pci_results, pci_dur) = run_audit_phase(
        !config.pci_devices.is_empty(),
        audit_pci_subsystem,
        |buses| evaluate_pci_blueprint(buses, &config.pci_devices),
        print_annotated_pci,
    )?;

    // =========================================================================
    // AGGREGATE & REPORT
    // =========================================================================

    let all_results: Vec<ValidationResult> = sys_results
        .into_iter()
        .chain(i2c_results)
        .chain(usb_results)
        .chain(net_results)
        .chain(pci_results)
        .collect();

    if let Some(filepath) = args.xml_report {
        let total_scan_duration = sys_dur + i2c_dur + usb_dur + net_dur + pci_dur;

        generate_junit_xml(
            &all_results,
            &filepath,
            Some(total_scan_duration),
            "System Audit",
            "System: Full Discovery (Scan)",
        )?;

        if args.xml_summary {
            print_xml_summary(&filepath)?;
        }
    }

    Ok(())
}
