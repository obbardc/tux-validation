use crate::device::{
    TuxBus, TuxDevice, DeviceDetails, UsbProperties, 
    UsbInterface, Subsystem, BusStatus
};
use anyhow::Result;
use udev::Enumerator;
use std::collections::HashMap;

/// Scans the USB subsystem and returns a list of TuxBus controllers,
/// each containing a recursive tree of TuxDevices.
pub fn audit_usb_subsystem() -> Result<Vec<TuxBus>> {
    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem("usb")?;

    // Collect all USB related udev entries (devices and interfaces)
    let all_entries: Vec<_>= enumerator.scan_devices()?.collect();
    let mut buses = Vec::new();

    // Identify Root Hubs to initialize the TuxBuses
    for dev in &all_entries {
        let sysname = dev.sysname().to_str().unwrap_or("");
        let dev_type = dev.devtype().and_then(|t| t.to_str()).unwrap_or("");

        // Root hubs should be named like 'usb1', 'usb2'
        if dev_type == "usb_device" && sysname.starts_with("usb") {
            let bus_id_str = dev.attribute_value("busnum")
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
    let mut tux_dev = TuxDevice::from_udev(current_udev)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse device at {}", current_udev.syspath().to_string_lossy()))?;

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
                            if_num: potential.attribute_value("bInterfaceNumber")
                                .and_then(|v| v.to_str()?.parse().ok())
                                .unwrap_or(0),
                            class: potential.attribute_value("bInterfaceClass")
                                .and_then(|v| v.to_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            driver: potential.driver().and_then(|s| s.to_str()).map(|s| s.to_string()),
                        });
                    }
                    _ => {}
                }
            }
        }
    }

    // Populate the 'details' property slot with USB-specific data
    // TODO: is default value for 'devnum' a good idea? 
    let dev_num = current_udev.attribute_value("devnum")
        .and_then(|v| v.to_str()?.parse().ok())
        .unwrap_or(0);
    let speed = current_udev.attribute_value("speed")
        .and_then(|v| v.to_str())
        .unwrap_or("Unknown")
        .to_string();
    let serial_id = current_udev.property_value("ID_SERIAL")
        .and_then(|v| v.to_str())
        .unwrap_or("Unknown")
        .to_string();

    tux_dev.details = DeviceDetails::Usb(UsbProperties {
        speed,
        interfaces,
        dev_num,
        serial_id
    });
    tux_dev.status.hw_responding = true;
    tux_dev.children = children;

    Ok(tux_dev)
}