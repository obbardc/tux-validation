use crate::device::{DeviceAddress, DeviceDetails, Subsystem, TuxBus, TuxDevice, UsbInterface};
use crate::validation::{AuditStatus, TargetId, ValidationResult};
use colored::{ColoredString, Colorize};
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

/// Returns the bullet point icon for the printed results
fn test_status_symbol(result: Option<&ValidationResult>) -> ColoredString {
    match result {
        Some(r) if matches!(r.status, AuditStatus::Pass) => "★".green(),
        Some(_) => "✖".red().bold(),
        None => "•".white(),
    }
}

/// Returns a colored string representing a property based on a validation result check status.
/// Accepts a lambda as a parameter to style the resulting colored string (e.g. bold, dimmed, italic etc.).
fn property_colored_custom(
    property: Option<&str>,
    result: Option<&ValidationResult>,
    check_name: &str,
    style: impl Fn(ColoredString) -> ColoredString,
) -> ColoredString {
    let property_name = property.unwrap_or("none");

    if let Some(r) = result {
        if let Some(check) = r.checks.iter().find(|c| c.name == check_name) {
            return if check.passed {
                style(property_name.green())
            } else {
                style(property_name.red())
            };
        }
    }

    style(property_name.blue())
}

/// A specific version of `property_colored_custom`, printing string in bold.
fn property_colored_bold(
    property: Option<&str>,
    result: Option<&ValidationResult>,
    check_name: &str,
) -> ColoredString {
    // Calls the custom function with your standard blue default
    property_colored_custom(property, result, check_name, |s| s.bold())
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
        let res = results.iter().find(|r| match &r.target_id {
            TargetId::Usb {
                vid: res_vid,
                pid: res_pid,
            } => res_vid == vid && res_pid == pid,
            _ => false,
        });

        let status_symbol = test_status_symbol(res);

        let speed_colored = property_colored_bold(Some(&dev_props.speed), res, "Speed");
        let port_colored = property_colored_custom(Some(&port_path), res, "Port", |s| s.dimmed());

        println!(
            "{}{} {} [{}:{}] at {} ({}M)",
            indent,
            status_symbol,
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
            print_usb_interface(iface, &indent, res);
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

    let driver_colored = property_colored_bold(iface.driver.as_deref(), result, "Driver");

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
        let res = results.iter().find(|res| match &res.target_id {
            TargetId::I2c {
                bus: res_bus,
                address: res_address,
            } => res_bus == bus && res_address == address,
            _ => false,
        });

        let status_symbol = test_status_symbol(res);

        // Header
        println!(
            "  {} {} [{}]",
            status_symbol,
            dev.name.cyan(),
            format!("0x{:02x}", address).blue().dimmed()
        );

        // Hardware ACK/NACK and driver
        let hw_resp = match dev.status.hw_responding {
            Some(true) => format!(" (HW: {})", "ACK".green().bold()),
            Some(false) => format!(" (HW: {})", "NACK".red().bold()),
            None => "".to_string(), // Silently omit if we didn't probe
        };

        let driver_colored =
            property_colored_bold(dev.status.driver_bound.as_deref(), res, "Driver");

        println!("    ┗━ Driver: {}{}", driver_colored, hw_resp);
    }
}

pub fn print_annotated_network(buses: &[TuxBus], results: &[ValidationResult]) {
    println!("\n{}", "=== NETWORK SUBSYSTEM ===".bold().cyan());

    //TODO: should apply filter to other subsystems too...
    for bus in buses.iter().filter(|b| b.subsystem == Subsystem::Net) {
        if bus.devices.is_empty() {
            println!(
                "  {}",
                "No network interfaces detected (excluding loopback).".yellow()
            );
            continue;
        }

        for dev in &bus.devices {
            match &dev.details {
                DeviceDetails::Ethernet(props) => {
                    print_network_interface_details(
                        dev,
                        props.link_detected,
                        &props.ipv4_address,
                        &props.ipv6_address,
                        Some(props.speed),
                        Some(&props.duplex),
                        results,
                    );
                }
                DeviceDetails::Wifi(props) => {
                    print_network_interface_details(
                        dev,
                        props.link_detected,
                        &props.ipv4_address,
                        &props.ipv6_address,
                        None,
                        None,
                        results,
                    );
                }
                _ => continue,
            }
        }
    }
}

// Helper printing function for Eth/Wifi
fn print_network_interface_details(
    dev: &TuxDevice,
    link_detected: bool,
    ipv4: &[String],
    ipv6: &[String],
    speed: Option<u32>,
    duplex: Option<&String>,
    results: &[ValidationResult],
) {
    // Use speed.map_or(...) to hide speed for Wifi devices
    // Find the validation record for this interface
    let res = results.iter().find(|r| {
        if let TargetId::Network { interface } = &r.target_id {
            interface == &dev.name
        } else {
            false
        }
    });

    let status_symbol = test_status_symbol(res);

    // 1. Header: Interface Name and Physical Link Status
    let link_text = if link_detected {
        "UP".green().bold()
    } else {
        "DOWN".red().bold()
    };
    println!("  {} {} [{}]", status_symbol, dev.name.cyan(), link_text);

    // 2. MAC Address Line
    if let DeviceAddress::Network { mac, .. } = &dev.address {
        let mac_colored = property_colored_custom(Some(mac), res, "Mac Address", |s| s.dimmed());
        println!("    ┣━ MAC:    {}", mac_colored);
    }

    // 3. Driver
    let driver_colored = property_colored_bold(dev.status.driver_bound.as_deref(), res, "Driver");
    println!("    ┣━ Driver: {}", driver_colored);

    // 4. IP Addresses
    if !ipv4.is_empty() {
        let ipv4_str = ipv4.join(", ");
        let ipv4_colored =
            property_colored_custom(Some(&ipv4_str), res, "IPv4 Check", |s| s.dimmed());
        println!("    ┣━ IPv4:   {}", ipv4_colored);
    } else {
        println!("    ┣━ IPv4:   {}", "none".yellow().dimmed());
    }

    if !ipv6.is_empty() {
        let ipv6_str = ipv6.join(", ");
        println!("    ┣━ IPv6:   {}", ipv6_str.blue().dimmed());
    }

    // If wifi, SSID, Signal and Frequency
    if let DeviceDetails::Wifi(props) = &dev.details {
        // SSID with Validation Coloring
        if let Some(ssid) = &props.ssid {
            let ssid_colored = property_colored_bold(Some(ssid), res, "SSID");
            println!("    ┣━ SSID:   {}", ssid_colored);
        }

        // Signal Level (Color-coded by strength)
        if props.signal_level != 0 {
            let signal_color = match props.signal_level {
                s if s > -50 => s.to_string().green(),
                s if s > -70 => s.to_string().yellow(),
                _ => props.signal_level.to_string().red(),
            };
            println!("    ┣━ Signal: {} dBm", signal_color);
        }

        // Frequency (Converting MHz to GHz)
        if props.frequency != 0 {
            // Convert e.g., 2412 MHz to 2.412 GHz
            let ghz = props.frequency as f32 / 1000.0;
            println!("    ┣━ Freq:   {:.3} GHz", ghz.to_string().dimmed());
        }
    }

    // Speed & Duplex
    if link_detected {
        let speed_str = speed.map_or("Connected".to_string(), |s| format!("{} Mbps", s));
        let speed_colored = property_colored_bold(Some(&speed_str), res, "Speed");
        let duplex_colored = duplex
            .map_or("".to_string(), |d| format!("({})", d))
            .yellow();
        println!("    ┗━ Config: {} {}", speed_colored, duplex_colored);
    } else {
        println!("    ┗━ Config: {}", "No Carrier".red().dimmed());
    }
}

// Prints annotated PCI devices report
pub fn print_annotated_pci(buses: &[TuxBus], results: &[ValidationResult]) {
    println!("\n{}", "=== PCIe SUBSYSTEM ===".bold().cyan());

    for bus in buses.iter().filter(|b| b.subsystem == Subsystem::Pci) {
        if bus.devices.is_empty() {
            continue;
        }

        // TODO:need sorting? Kinda expensive...
        let mut devices = bus.devices.clone();
        devices.sort_by(|a, b| {
            if let (
                DeviceAddress::Pci {
                    domain: d1,
                    bus: b1,
                    device: dev1,
                    function: f1,
                },
                DeviceAddress::Pci {
                    domain: d2,
                    bus: b2,
                    device: dev2,
                    function: f2,
                },
            ) = (&a.address, &b.address)
            {
                (d1, b1, dev1, f1).cmp(&(d2, b2, dev2, f2))
            } else {
                std::cmp::Ordering::Equal
            }
        });
        for dev in &devices {
            if let DeviceDetails::Pci(props) = &dev.details {
                // Reconstruct the BDF address from the structured enum
                let bdf_addr = if let DeviceAddress::Pci {
                    domain,
                    bus,
                    device,
                    function,
                } = &dev.address
                {
                    format!("{:04x}:{:02x}:{:02x}.{:x}", domain, bus, device, function)
                } else {
                    "Unknown".to_string()
                };

                // Find the validation record for this specific BDF slot
                let res = results.iter().find(|r| {
                    if let TargetId::Pci { address } = &r.target_id {
                        address == &bdf_addr
                    } else {
                        false
                    }
                });

                let status_symbol = test_status_symbol(res);

                // 1. Header: Interface Name and BDF Address
                println!(
                    "  {} {} [{}]",
                    status_symbol,
                    dev.name.cyan(),
                    bdf_addr.dimmed()
                );

                // 2. Hardware ID
                let hw_id = format!("{}:{}", props.vendor_id, props.device_id);
                println!("    ┣━ HW ID:  {}", hw_id.blue().dimmed());

                // 3. Driver
                let driver_colored =
                    property_colored_bold(dev.status.driver_bound.as_deref(), res, "Driver");
                println!("    ┣━ Driver: {}", driver_colored);

                // 4. Link Statistics (Handling True PCIe vs Integrated SoC blocks)
                if let (Some(cur_speed), Some(cur_width)) =
                    (&props.cur_link_speed, props.cur_link_width)
                {
                    let link_str = format!("{} @ x{}", cur_speed, cur_width);

                    // Highlight if the device is electrically bottlenecked
                    let is_bottlenecked =
                        props.max_link_width.is_some_and(|max_w| cur_width < max_w);
                    let mut link_colored = if is_bottlenecked {
                        format!("{} (Bottlenecked!)", link_str).yellow()
                    } else {
                        link_str.blue()
                    };

                    // Override color if link parameters were tested:
                    if let Some(r) = res {
                        if r.checks
                            .iter()
                            .any(|c| c.name.contains("Link") && !c.passed)
                        {
                            link_colored = link_str.red().bold();
                        } else if r.checks.iter().any(|c| c.name.contains("Link") && c.passed) {
                            link_colored = link_str.green().bold();
                        }
                    }

                    println!("    ┗━ Link:   {}", link_colored);
                } else {
                    // This hits devices without link data (integrated SoC?)
                    let link_colored = if let Some(r) = res
                        && r.checks.iter().any(|c| c.name.contains("Link"))
                    {
                        "Integrated/Internal".red().dimmed()
                    } else {
                        "Integrated/Internal".dimmed()
                    };
                    println!("    ┗━ Link:   {}", link_colored);
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
