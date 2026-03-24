use clap::Parser;
use colored::Colorize;
use std::fs;
use std::time::Instant;

use tux_validation::config::Config;
use tux_validation::report::{generate_junit_xml, print_annotated_systemd, print_xml_summary};
use tux_validation::systemd::audit_systemd_services;
use tux_validation::validation::evaluate_systemd_blueprint;

#[derive(Parser, Debug)]
#[command(author, version, about = "Standalone Systemd Service Auditor")]
struct Args {
    /// Path to the TOML configuration blueprint
    #[arg(short, long, conflicts_with = "services")]
    config: Option<String>,

    /// Path to output JUnit XML report (only valid when using --config)
    #[arg(long, requires = "config")]
    xml_report: Option<String>,

    /// Print a summary of the XML report (only valid when using --xml-report)
    #[arg(long, requires = "xml_report")]
    xml_summary: bool,

    /// Ad-hoc list of services to query directly (e.g., NetworkManager.service sshd.service)
    #[arg(conflicts_with = "config")]
    services: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // =========================================================================
    // MODE 1: CI / BLUEPRINT VALIDATION MODE (--config)
    // =========================================================================
    if let Some(config_path) = args.config {
        println!(
            "Mode: {} ({})",
            "Blueprint Validation".yellow(),
            config_path
        );

        // Load the TOML
        let config_str = fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&config_str)?;

        if config.systemd_services.is_empty() {
            println!(
                "{} No [[systemd_services]] found in blueprint.",
                "⚠".yellow().bold()
            );
            return Ok(());
        }

        // Extract names to query
        let service_names: Vec<String> = config
            .systemd_services
            .iter()
            .map(|exp| exp.name.clone())
            .collect();

        // Audit (Scan via D-Bus)
        let start_time = Instant::now();
        let scanned_services = audit_systemd_services(&service_names)?;
        let scan_duration = start_time.elapsed();

        // Evaluate against the blueprint configuration
        let results = evaluate_systemd_blueprint(&scanned_services, &config.systemd_services);

        // Report to terminal
        print_annotated_systemd(&scanned_services, &results);

        // Generate XML (if requested)
        if let Some(xml_path) = args.xml_report {
            generate_junit_xml(
                &results,
                &xml_path,
                Some(scan_duration),
                "Systemd Audit",
                "Systemd: Service Discovery",
            )?;
            if args.xml_summary {
                print_xml_summary(&xml_path)?;
            }
        }

    // =========================================================================
    // MODE 2: AD-HOC QUERY MODE (Pass service names as args)
    // =========================================================================
    } else if !args.services.is_empty() {
        println!("Mode: {}", "Ad-Hoc Query".yellow());

        let scanned_services = audit_systemd_services(&args.services)?;

        print_annotated_systemd(&scanned_services, &[]);
    } else {
        println!(
            "{} Please provide either a --config file or a list of services.",
            "⚠".yellow().bold()
        );
        println!("Run with --help for usage details.");
    }

    Ok(())
}
