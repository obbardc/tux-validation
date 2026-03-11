use crate::device::{DeviceAddress, DeviceDetails, TuxBus, TuxDevice, UsbInterface, Subsystem};
use crate::validation::{AuditStatus, TargetId, ValidationResult};
use colored::Colorize;
use junit_report::{Duration as JunitDuration, Report, TestCase, TestSuite};
use roxmltree::Document;
use std::fs;
use std::time::Duration as CoreDuration;

/// Generates a junit report object and writes it in XML format.
pub fn generate_junit_xml(
    results: &[ValidationResult],
    filepath: &str,
    scan_duration: Option<CoreDuration>,
) -> anyhow::Result<()> {
    let mut report = Report::new();
    let mut suite = TestSuite::new("Hardware Audit");

    if let Some(duration) = scan_duration {
        let scan_time =
            JunitDuration::try_from(duration).unwrap_or_else(|_| JunitDuration::milliseconds(1));
        suite.add_testcase(TestCase::success(
            "System: Hardware Discovery (Scan)",
            scan_time,
        ));
    }

    for res in results {
        let test_name = format!("[{}] {}", res.subsystem, res.item_name);

        let test_time = JunitDuration::try_from(res.duration)
            .unwrap_or_else(|_| JunitDuration::milliseconds(1));

        let case = match &res.status {
            AuditStatus::Pass => TestCase::success(&test_name, test_time),
            AuditStatus::Fail { reason, .. } | AuditStatus::Missing { reason } => {
                TestCase::failure(&test_name, test_time, "HardwareError", reason)
            }
        };
        suite.add_testcase(case);
    }

    report.add_testsuite(suite);
    let mut file = std::fs::File::create(filepath)?;
    report.write_xml(&mut file)?;
    Ok(())
}

///Prints annotated USB tree with detected devices and interfaces
pub fn print_annotated_usb_tree(
    buses: &[TuxBus],
    results: &[ValidationResult],
    print_serial: bool,
) {
    println!("\n{}", "=== USB SUBSYSTEM ===".bold().cyan());
    for bus in buses {
        println!("\n{} (Bus {})", "Bus Controller".bold(), bus.id.yellow());
        for dev in &bus.devices {
            print_recursive_node(dev, results, 0, print_serial);
        }
    }
}

fn print_recursive_node(
    dev: &TuxDevice,
    results: &[ValidationResult],
    depth: usize,
    print_serial: bool,
) {
    let indent = "  ".repeat(depth);
    // Check dev is a USB device and extract necessary parameters
    if let DeviceDetails::Usb(dev_props) = &dev.details
        && let DeviceAddress::Usb {
            vid,
            pid,
            port_path,
            ..
        } = &dev.address
    {
        let test_result = results.iter().find(|res| match &res.target_id {
            TargetId::Usb {
                vid: res_vid,
                pid: res_pid,
            } => res_vid == vid && res_pid == pid,
            _ => false,
        });

        let mut speed_colored = dev_props.speed.blue().bold();
        let mut port_colored = port_path.blue().dimmed();
        let mut newline_colored = "•".white();
        if let Some(res) = test_result {
            // Select PASS/FAIL/DEFAULT colors based on checks results
            let speed_check = res.checks.iter().find(|check| check.name == "Speed");
            speed_colored = match speed_check {
                Some(speed_checked) => {
                    if speed_checked.passed {
                        dev_props.speed.green().bold()
                    } else {
                        dev_props.speed.red().bold()
                    }
                }
                None => dev_props.speed.blue().bold(),
            };
            let port_check = res.checks.iter().find(|check| check.name == "Port");
            port_colored = match port_check {
                Some(port_checked) => {
                    if port_checked.passed {
                        port_path.green().dimmed()
                    } else {
                        port_path.red().dimmed()
                    }
                }
                None => port_path.blue().dimmed(),
            };
            newline_colored = "★".yellow();
        }
        println!(
            "{}{} {} [{}:{}] at {} ({}M)",
            indent,
            newline_colored,
            dev.name.cyan(),
            vid,
            pid,
            port_colored,
            speed_colored,
        );

        // Optionally print ID_SERIAL property
        if print_serial {
            println!(
                "{}    {} {}",
                indent,
                "ID:".dimmed(),
                dev_props.serial_id.dimmed()
            );
        }
        // Check and print interfaces data
        for iface in &dev_props.interfaces {
            print_usb_interface(iface, &indent, test_result);
        }
    }

    // Recurse into hub children
    for child in &dev.children {
        print_recursive_node(child, results, depth + 1, print_serial);
    }
}

fn print_usb_interface(iface: &UsbInterface, indent: &str, result: Option<&ValidationResult>) {
    let class_name = match iface.class.as_str() {
        "01" => "Audio",
        "09" => "Hub",
        "0e" => "Video",
        "03" => "HID",
        "ff" => "Vendor-Specific",
        _ => &iface.class,
    };
    let dev_driver = iface.driver.as_deref().unwrap_or("none");
    let mut driver_colored = dev_driver.blue().bold();
    if let Some(res) = result {
        let driver_check = res.checks.iter().find(|check| check.name == "Driver");
        driver_colored = match driver_check {
            Some(driver_checked) => {
                if driver_checked.passed {
                    dev_driver.green().bold()
                } else {
                    dev_driver.red().bold()
                }
            }
            None => dev_driver.blue().bold(),
        };
    }

    println!(
        "{}  ┗━ If {:02} [{}]: Driver {}",
        indent, iface.if_num, class_name, driver_colored,
    );
}

pub fn print_annotated_i2c(buses: &[TuxBus], results: &[ValidationResult]) {
    println!("\n{}", "=== I2C SUBSYSTEM ===".bold().cyan());
    for bus in buses {
        // Skip empty buses for cleaner output
        if bus.devices.is_empty() {
            continue;
        }

        println!("\n{} (Bus {})", "I2C Bus".bold(), bus.id.yellow());
        for dev in &bus.devices {
            print_i2c_device(dev, results);
        }
    }
}

fn print_i2c_device(dev: &TuxDevice, results: &[ValidationResult]) {
    // Check dev is a I2C device and extract necessary parameters
    if let DeviceAddress::I2c { bus, address } = &dev.address {
        let test_result = results.iter().find(|res| match &res.target_id {
            TargetId::I2c {
                bus: res_bus,
                address: res_address,
            } => res_bus == bus && res_address == address,
            _ => false,
        });

        let addr_str = format!("0x{:02x}", address);
        let driver = dev.status.driver_bound.as_deref().unwrap_or("none");
        let hw_resp = match dev.status.hw_responding {
            Some(true) => format!(" (HW: {})", "ACK".green().bold()),
            Some(false) => format!(" (HW: {})", "NACK".red().bold()),
            None => "".to_string(), // Silently omit if we didn't probe
        };

        let addr_colored = addr_str.blue().dimmed();
        let dev_name_colored = dev.name.cyan();
        let (driver_colored, newline_colored) = if let Some(res) = test_result {
            let driver_check = res.checks.iter().find(|check| check.name == "Driver");
            let d_color = match driver_check {
                Some(check) if check.passed => driver.green().bold(),
                Some(_) => driver.red().bold(),
                None => driver.blue().bold(),
            };

            (d_color, "★".yellow())
        } else {
            // The defaults if no test result is found
            (driver.blue().bold(), "•".white())
        };
        println!(
            "  {} {} [{}]",
            newline_colored, dev_name_colored, addr_colored
        );
        println!("    ┗━ Driver: {}{}", driver_colored, hw_resp);
    }
}

pub fn print_annotated_network(buses: &[TuxBus], results: &[ValidationResult]) {
    println!("\n{}", "=== NETWORK SUBSYSTEM ===".bold().cyan());

    //TODO: should apply filter to other subsystems too...
    for bus in buses.iter().filter(|b| b.subsystem == Subsystem::Net) {
        if bus.devices.is_empty() {
            println!("  {}", "No network interfaces detected (excluding loopback).".yellow());
            continue;
        }

        for dev in &bus.devices {
            if let DeviceDetails::Ethernet(props) = &dev.details {
                // Find the validation record for this interface
                let res = results.iter().find(|r| {
                    if let TargetId::Ethernet { interface } = &r.target_id {
                        interface == &dev.name
                    } else {
                        false
                    }
                });

                // Determine the bullet point icon and color based on overall status
                let status_symbol = match res {
                    Some(r) if matches!(r.status, AuditStatus::Pass) => "★".yellow(),
                    Some(_) => "✖".red().bold(), // TODO: should I do the same for other devices?
                    None => "•".white(),
                };

                // 1. Header: Interface Name and Physical Link Status
                let link_text = if props.link_detected { "UP".green().bold() } else { "DOWN".red().bold() };
                println!("  {} {} [{}]", status_symbol, dev.name.cyan(), link_text);

                // 2. MAC Address Line
                if let DeviceAddress::Ethernet { mac, .. } = &dev.address {
                    let mut mac_colored = mac.blue().dimmed();
                    if let Some(r) = res {
                        if let Some(check) = r.checks.iter().find(|c| c.name == "Mac Address") {
                            mac_colored = if check.passed { mac.green().dimmed() } else { mac.red().dimmed() };
                        }
                    }
                    println!("    ┣━ MAC:    {}", mac_colored);
                }

                // 3. Driver
                let driver_name = dev.status.driver_bound.as_deref().unwrap_or("none");
                let mut driver_colored = driver_name.blue();
                if let Some(r) = res {
                    if let Some(check) = r.checks.iter().find(|c| c.name == "Driver") {
                        driver_colored = if check.passed { driver_name.green().bold() } else { driver_name.red().bold() };
                    }
                }
                println!("    ┣━ Driver: {}", driver_colored);

                // 4. IP Addresses
                if !props.ipv4_address.is_empty() {
                    let ip_str = props.ipv4_address.join(", ");
                    let mut ip_colored = ip_str.blue().dimmed();
                    if let Some(r) = res {
                        if let Some(check) = r.checks.iter().find(|c| c.name == "IPv4 Check") {
                            ip_colored = if check.passed { ip_str.green().dimmed() } else { ip_str.red().dimmed() };
                        }
                    }
                    println!("    ┣━ IPv4:   {}", ip_colored);
                } else {
                    println!("    ┣━ IPv4:   {}", "none".yellow().dimmed());
                }

                // 5. Speed & Duplex
                if props.link_detected {
                    let speed_str = format!("{} Mbps", props.speed);
                    let mut speed_colored = speed_str.blue();
                    let duplex_colored = props.duplex.yellow();
                    if let Some(r) = res {
                        if let Some(check) = r.checks.iter().find(|c| c.name == "Speed") {
                            speed_colored = if check.passed { speed_str.green().bold() } else { speed_str.red().bold() };
                        }
                    }
                    println!("    ┗━ Config: {} ({})", speed_colored, duplex_colored);
                } else {
                    println!("    ┗━ Config: {}", "No Carrier".red().dimmed());
                }
            }
        }
    }
}

/// Reads a JUnit XML file and prints a high-level summary to the terminal.
pub fn print_xml_summary(filepath: &str) -> anyhow::Result<()> {
    // Read the file into a string
    let xml_str = fs::read_to_string(filepath)
        .map_err(|e| anyhow::anyhow!("Failed to read XML file '{}': {}", filepath, e))?;

    // Parse the XML stringm into a DOM tree representation
    let doc =
        Document::parse(&xml_str).map_err(|e| anyhow::anyhow!("Failed to parse XML: {}", e))?;

    let mut total_tests = 0;
    let mut total_failures = 0;

    // Walk the tree looking for <testcase> nodes
    for node in doc.descendants() {
        if node.has_tag_name("testcase") {
            total_tests += 1;

            // Check if this testcase has a <failure> or <error> child
            let has_failure = node
                .children()
                .any(|c| c.has_tag_name("failure") || c.has_tag_name("error"));

            if has_failure {
                total_failures += 1;
            }
        }
    }

    let passed = total_tests - total_failures;

    // Print the condensed summary
    println!("\n{}", "=== XML REPORT SUMMARY ===".bold().cyan());
    println!("File:  {}", filepath.dimmed());
    println!("Total: {}", total_tests);

    if passed > 0 {
        println!("Pass:  {}", passed.to_string().green().bold());
    } else {
        println!("Pass:  {}", passed);
    }

    if total_failures > 0 {
        println!("Fail:  {}", total_failures.to_string().red().bold());
    } else {
        println!("Fail:  {}", total_failures);
    }

    // Add a final status banner
    if total_failures == 0 && total_tests > 0 {
        println!("\n{}", "ALL TESTS PASSED".green().bold());
    } else if total_failures > 0 {
        println!("\n{}", "SOME TESTS FAILED".red().bold());
    }

    Ok(())
}
