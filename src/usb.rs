use crate::device::{
    BusStatus, DeviceAddress, DeviceDetails, Subsystem, TuxBus, TuxDevice, UsbInterface,
    UsbProperties,
};
use crate::config::{UsbExpectation};
use anyhow::Result;
use colored::*;
use std::collections::HashMap;
use udev::Enumerator;

/// Scans the USB subsystem and returns a list of TuxBus controllers,
/// each containing a recursive tree of TuxDevices.
pub fn audit_usb_subsystem() -> Result<Vec<TuxBus>> {
    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem("usb")?;

    // Collect all USB related udev entries (devices and interfaces)
    let all_entries: Vec<_> = enumerator.scan_devices()?.collect();
    let mut buses = Vec::new();

    // Identify Root Hubs to initialize the TuxBuses
    for dev in &all_entries {
        let sysname = dev.sysname().to_str().unwrap_or("");
        let dev_type = dev.devtype().and_then(|t| t.to_str()).unwrap_or("");

        // Root hubs should be named like 'usb1', 'usb2'
        if dev_type == "usb_device" && sysname.starts_with("usb") {
            let bus_id_str = dev
                .attribute_value("busnum")
                .and_then(|v| v.to_str())
                .ok_or_else(|| anyhow::anyhow!("Device {} is missing busnum", sysname))?;

            // Build the recursive tree starting from this "root" hub
            let root_device_tree = build_usb_tree(dev, &all_entries)?;

            buses.push(TuxBus {
                name: format!("USB Controller (Bus {})", bus_id_str),
                subsystem: Subsystem::Usb,
                id: bus_id_str.to_string(),
                devices: vec![root_device_tree],
                status: BusStatus::Active,
                metadata: HashMap::new(),
            });
        }
    }

    buses.sort_by_key(|bus| bus.id.parse::<u8>().unwrap_or(0));

    Ok(buses)
}

/// Recursively builds the TuxDevice tree, capturing physical children and logical interfaces.
fn build_usb_tree(current_udev: &udev::Device, pool: &[udev::Device]) -> Result<TuxDevice> {
    // Create the base TuxDevice
    let mut tux_dev = TuxDevice::from_udev(current_udev).ok_or_else(|| {
        anyhow::anyhow!(
            "Failed to parse device at {}",
            current_udev.syspath().to_string_lossy()
        )
    })?;

    let mut interfaces = Vec::new();
    let mut children = Vec::new();

    // Iterate through the pool to find logical interfaces and physical children
    for potential in pool {
        if let Some(parent) = potential.parent() {
            // Check if this entry's parent is the one we are currently processing
            if parent.syspath() == current_udev.syspath() {
                let dev_type = potential.devtype().and_then(|t| t.to_str()).unwrap_or("");

                match dev_type {
                    "usb_device" => {
                        // Continue recursive search if a physical child (e.g., a device plugged into a hub)
                        children.push(build_usb_tree(potential, pool)?);
                    }
                    "usb_interface" => {
                        // Capture logical function info
                        interfaces.push(UsbInterface {
                            if_num: potential
                                .attribute_value("bInterfaceNumber")
                                .and_then(|v| v.to_str()?.parse().ok())
                                .unwrap_or(0),
                            class: potential
                                .attribute_value("bInterfaceClass")
                                .and_then(|v| v.to_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            driver: potential
                                .driver()
                                .and_then(|s| s.to_str())
                                .map(|s| s.to_string()),
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    // Populate the 'details' property slot with USB-specific data
    // TODO: is default value for 'devnum' a good idea?
    let dev_num = current_udev
        .attribute_value("devnum")
        .and_then(|v| v.to_str()?.parse().ok())
        .unwrap_or(0);
    let speed = current_udev
        .attribute_value("speed")
        .and_then(|v| v.to_str())
        .unwrap_or("Unknown")
        .to_string();
    let serial_id = current_udev
        .property_value("ID_SERIAL")
        .and_then(|v| v.to_str())
        .unwrap_or("Unknown")
        .to_string();

    tux_dev.details = DeviceDetails::Usb(UsbProperties {
        speed,
        interfaces,
        dev_num,
        serial_id,
    });
    tux_dev.status.hw_responding = true;
    tux_dev.children = children;

    Ok(tux_dev)
}

/// Prints USB device tree for each bus, and verifies parameters against provided blueprint.
///
/// Optionally prints ID_SERIAL property from udev.
pub fn print_and_verify_usb(buses: &[TuxBus], blueprint: &[UsbExpectation], serial: bool) {
    for bus in buses {
        println!("\n{} (Bus {})", "Bus Controller".bold(), bus.id.yellow());
        for device in &bus.devices {
            audit_recursive(device, 0, blueprint, serial);
        }
    }
}

/// Recursively prints details for nested USB devices and their interfaces.
///
/// Dependent on whether actual data matches provided expectations or not.
fn audit_recursive(dev: &TuxDevice, depth: usize, blueprint: &[UsbExpectation], serial: bool) {
    let indent = "  ".repeat(depth);

    // Attempt to match device against blueprint
    if let DeviceDetails::Usb(props) = &dev.details
        && let DeviceAddress::Usb {
            vid,
            pid,
            port_path,
            ..
        } = &dev.address
    {
        let expectation = blueprint.iter().find(|e| &e.vid == vid && &e.pid == pid);

        // Print Device Header
        match expectation {
            Some(exp) => match &exp.min_speed {
                Some(exp_speed) if verify_speed(&props.speed, exp_speed) => {
                    println!(
                        "{}{} {} [{}:{}] at {} ({}M - {})",
                        indent,
                        "★".yellow(),
                        dev.name.cyan(),
                        vid,
                        pid,
                        port_path.dimmed(),
                        props.speed.green().bold(),
                        "expected".green()
                    );
                }
                Some(_) => {
                    println!(
                        "{}{} {} [{}:{}] at {} ({}M - {})",
                        indent,
                        "★".yellow(),
                        dev.name.cyan(),
                        vid,
                        pid,
                        port_path.dimmed(),
                        props.speed.red().bold(),
                        "unexpected".red()
                    );
                }
                None => {
                    println!(
                        "{}{} {} [{}:{}] at {} ({}M)",
                        indent,
                        "★".yellow(),
                        dev.name.cyan(),
                        vid,
                        pid,
                        port_path.dimmed(),
                        props.speed.blue().bold()
                    );
                }
            },
            None => {
                println!(
                    "{}{} {} [{}:{}] at {} ({}M)",
                    indent,
                    "•".white(),
                    dev.name.cyan(),
                    vid,
                    pid,
                    port_path.dimmed(),
                    props.speed.blue().bold()
                );
            }
        }

        // Optionally print ID_SERIAL property
        if serial {
            println!(
                "{}    {} {}",
                indent,
                "ID:".dimmed(),
                props.serial_id.dimmed()
            );
        }

        // Check Interfaces
        for iface in &props.interfaces {
            verify_interface(iface, &indent, expectation);
        }
    }

    // Recurse into hub children
    for child in &dev.children {
        audit_recursive(child, depth + 1, blueprint, serial);
    }
}

/// Checks if expected USB speed is equal or larger than the actual one.
fn verify_speed(actual: &str, expected_min: &str) -> bool {
    let speed_to_val = |s: &str| -> u32 {
        // Strip everything that isn't a digit (like "M" or "Mbps")
        let cleaned = s.trim_end_matches(|c: char| !c.is_numeric());

        // Parse directly to u32. If it fails, return 0.
        cleaned.parse::<u32>().unwrap_or(0)
    };

    speed_to_val(actual) >= speed_to_val(expected_min)
}

/// Prints USB interface details, dependent on whether it matches expectations or not.
fn verify_interface(iface: &UsbInterface, indent: &str, expectation: Option<&UsbExpectation>) {
    let class_name = match iface.class.as_str() {
        "01" => "Audio",
        "09" => "Hub",
        "0e" => "Video",
        "03" => "HID",
        "ff" => "Vendor-Specific",
        _ => &iface.class,
    };
    let driver = iface.driver.as_deref().unwrap_or("none");

    match expectation {
        Some(exp) if exp.required_driver != driver => {
            println!(
                "{}  ┗━ If {:02} [{}]: Driver {} - expected {}",
                indent,
                iface.if_num,
                class_name,
                driver.red().bold(),
                exp.required_driver.red()
            );
        }
        Some(_) => {
            println!(
                "{}  ┗━ If {:02} [{}]: Driver {} - {}",
                indent,
                iface.if_num,
                class_name,
                driver.green().bold(),
                "expected".green()
            );
        }
        None => {
            println!(
                "{}  ┗━ If {:02} [{}]: Driver {}",
                indent,
                iface.if_num,
                class_name,
                driver.blue().bold()
            );
        }
    }
}
